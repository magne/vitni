//! The family change-set use-case (Phase 5): a deferred create that commits a family and its partners
//! (0..=2) in a single operator action.
//!
//! A family's only scalar is its user-facing id; the draft is the id override plus the partners
//! (`family.html`). A partner is either an existing person (by `human_id`) or one created inline from
//! the picker's "+ New person". Every existing partner is resolved to a `PersonId` and any id override
//! is checked for a duplicate **before any write**, so a bad reference rejects the whole change-set
//! (nothing commits). New partners are created first (a minimal name-only person change-set carrying
//! the same provenance), so the family's `AddPartner` always references a real person. Provenance
//! follows the shared change-set rule ([`crate::change_set`]). Adding/removing partners on an existing
//! family is the per-row path (PR30), not this create.

use genealogy_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
use genealogy_core::ids::{HumanId, PersonId};
use genealogy_core::provenance::CitationRef;
use genealogy_db::Store;

use crate::error::AppError;
use crate::person::PersonNameParts;
use crate::person_change_set::{PersonChangeSet, PersonTarget, commit_person_change_set};
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// A partner on a family change-set: an existing person (by `human_id`) or one created inline (by its
/// name parts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartnerInput {
    /// An existing person, by their `human_id` (resolved before any write).
    Existing(String),
    /// A person created inline, by their name parts (created before the family).
    New {
        /// The given name, if any.
        given: Option<String>,
        /// The surname, if any.
        surname: Option<String>,
    },
}

/// The desired end state of a new family, committed as one operator action: an optional `human_id`
/// override and the partners (0..=2, the family aggregate enforces the upper bound).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyChangeSet {
    /// A caller-supplied `human_id` override; `None`/blank auto-allocates the next free one.
    pub human_id: Option<String>,
    /// The partners, each existing or created inline (resolved/created before the family).
    pub partners: Vec<PartnerInput>,
    /// The operator intent stamped on every emitted command (`record-editing.html` §5b).
    pub provenance: Provenance,
    /// Citation `human_id`s backing every non-`Create*` command; resolved before any write.
    pub citations: Vec<String>,
}

/// A partner resolved for commit: an existing person's id, or a new person still to create.
enum ResolvedPartner {
    /// An existing person, resolved to their aggregate id.
    Existing(PersonId),
    /// A new person to create before the family.
    New {
        /// The given name, if any.
        given: Option<String>,
        /// The surname, if any.
        surname: Option<String>,
    },
}

/// Commits a [`FamilyChangeSet`]: creates any inline partner persons, then the family, then adds each
/// partner.
///
/// Returns the family's `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if the id override is in use, [`AppError::PersonNotFound`] if an
/// existing partner `human_id` is unknown, [`AppError::CitationNotFound`] if a backing citation is
/// unknown (all validated before any write, so nothing commits), [`AppError::FamilyDomain`] on a
/// domain rejection (e.g. a duplicate partner), or a workspace/store error.
pub async fn commit_family_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: FamilyChangeSet,
) -> Result<String, AppError> {
    let store = workspace.store();
    // Validate the id override and resolve every existing partner + backing citation before any
    // write, so a bad reference rejects the whole change-set.
    let requested = use_case::requested_human_id(change_set.human_id.clone());
    if let Some(id) = &requested
        && store.find_family(id).await?.is_some()
    {
        return Err(AppError::HumanIdTaken(id.clone()));
    }
    let partners = resolve_partners(store, &change_set.partners).await?;
    let block = use_case::resolve_citation_refs(store, &change_set.citations).await?;

    // Create each inline partner person first, so the family's AddPartner references a real id.
    let mut partner_ids: Vec<PersonId> = Vec::with_capacity(partners.len());
    for partner in partners {
        partner_ids.push(match partner {
            ResolvedPartner::Existing(person_id) => person_id,
            ResolvedPartner::New { given, surname } => {
                create_partner_person(workspace, session, given, surname, &change_set.provenance).await?
            }
        });
    }

    let human_id = match requested {
        Some(id) => id,
        None => store.next_family_human_id(&workspace.family_id_format()?).await?,
    };
    let family_id = session.new_family_id();
    let aggregate_id = family_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        FamilyCommand::CreateFamily {
            family_id,
            human_id: HumanId::new(&human_id),
        },
        change_set.provenance.clone(),
        Vec::new(),
    )
    .await?;

    for person_id in partner_ids {
        execute(
            store,
            session,
            &aggregate_id,
            FamilyCommand::AddPartner { family_id, person_id },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    Ok(human_id)
}

/// Resolves each partner input, turning every existing reference into a `PersonId` (an unknown one
/// errors) and passing new partners through untouched — all reads, before any write.
async fn resolve_partners(store: &Store, partners: &[PartnerInput]) -> Result<Vec<ResolvedPartner>, AppError> {
    let mut resolved: Vec<ResolvedPartner> = Vec::with_capacity(partners.len());
    for partner in partners {
        match partner {
            PartnerInput::Existing(human_id) => {
                let person_id = crate::person::resolve_person_id_public(store, human_id).await?;
                resolved.push(ResolvedPartner::Existing(person_id));
            }
            PartnerInput::New { given, surname } => resolved.push(ResolvedPartner::New {
                given: given.clone(),
                surname: surname.clone(),
            }),
        }
    }
    Ok(resolved)
}

/// Creates an inline partner via a minimal name-only person change-set carrying the family's
/// provenance, returning the new person's aggregate id.
async fn create_partner_person(
    workspace: &Workspace,
    session: &Session,
    given: Option<String>,
    surname: Option<String>,
    provenance: &Provenance,
) -> Result<PersonId, AppError> {
    let human_id = commit_person_change_set(
        workspace,
        session,
        PersonChangeSet {
            target: PersonTarget::New { human_id: None },
            name: Some(PersonNameParts::simple(given, surname)),
            name_citation: None,
            sex: None,
            tags: Vec::new(),
            new_sources: Vec::new(),
            new_citations: Vec::new(),
            provenance: provenance.clone(),
            citations: Vec::new(),
        },
    )
    .await?;
    crate::person::resolve_person_id_public(workspace.store(), &human_id).await
}

/// Executes one command through the store, stamping the operator `provenance` and backing
/// `citations`, and mapping the command outcome to [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: FamilyCommand,
    provenance: Provenance,
    citations: Vec<CitationRef>,
) -> Result<(), AppError> {
    let envelope = FamilyCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_family(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

#[cfg(test)]
mod tests {
    use super::{FamilyChangeSet, PartnerInput, commit_family_change_set};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::family::{list_families, show_family};
    use crate::person::{NewPerson, create_person, list_persons, show_person};
    use crate::session::Session;
    use crate::use_case::Provenance;
    use crate::workspace::Workspace;
    use genealogy_core::enums::EvidenceLevel;
    use genealogy_core::ids::AgentId;
    use genealogy_core::provenance::{Agent, AgentKind, Confidence};
    use tempfile::TempDir;
    use uuid::Uuid;

    fn operator() -> OperatorConfig {
        OperatorConfig {
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
            email: None,
        }
    }

    fn session() -> Session {
        Session::new(Agent {
            kind: AgentKind::Human,
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
        })
    }

    async fn setup() -> (Workspace, Session, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
        let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
            .await
            .expect("open");
        (workspace, session(), dir)
    }

    async fn person(ws: &Workspace, session: &Session) -> String {
        create_person(
            ws,
            session,
            NewPerson {
                human_id: None,
                name: None,
                evidence_level: EvidenceLevel::Conclusion,
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("person")
    }

    #[tokio::test]
    async fn create_commits_the_family_and_its_partners() {
        let (workspace, session, _dir) = setup().await;
        let a = person(&workspace, &session).await;
        let b = person(&workspace, &session).await;
        let human_id = commit_family_change_set(
            &workspace,
            &session,
            FamilyChangeSet {
                human_id: None,
                partners: vec![PartnerInput::Existing(a.clone()), PartnerInput::Existing(b.clone())],
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create");

        let family = show_family(&workspace, &human_id).await.expect("show").expect("family");
        assert_eq!(family.partners.len(), 2, "both partners are added");
    }

    #[tokio::test]
    async fn a_partnerless_draft_creates_a_bare_family() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_family_change_set(
            &workspace,
            &session,
            FamilyChangeSet {
                human_id: None,
                partners: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create");
        let family = show_family(&workspace, &human_id).await.expect("show").expect("family");
        assert!(family.partners.is_empty());
    }

    #[tokio::test]
    async fn an_unknown_partner_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let result = commit_family_change_set(
            &workspace,
            &session,
            FamilyChangeSet {
                human_id: None,
                partners: vec![PartnerInput::Existing("I9999".to_owned())],
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await;
        assert!(matches!(result, Err(crate::error::AppError::PersonNotFound(_))));
        let families = list_families(&workspace).await.expect("families");
        assert!(families.is_empty(), "nothing commits when a partner is unknown");
    }

    #[tokio::test]
    async fn an_unknown_existing_partner_rejects_before_any_new_partner_is_created() {
        let (workspace, session, _dir) = setup().await;
        let result = commit_family_change_set(
            &workspace,
            &session,
            FamilyChangeSet {
                human_id: None,
                partners: vec![
                    PartnerInput::New {
                        given: Some("Ada".to_owned()),
                        surname: Some("Lovelace".to_owned()),
                    },
                    PartnerInput::Existing("I9999".to_owned()),
                ],
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await;
        assert!(matches!(result, Err(crate::error::AppError::PersonNotFound(_))));
        let families = list_families(&workspace).await.expect("families");
        assert!(families.is_empty(), "no family is created");
        let persons = list_persons(&workspace).await.expect("persons");
        assert!(
            persons.is_empty(),
            "the inline partner is not created when a sibling is unknown"
        );
    }

    #[tokio::test]
    async fn mixed_existing_and_new_partners_create_the_persons_then_the_family() {
        let (workspace, session, _dir) = setup().await;
        let existing = person(&workspace, &session).await;
        let human_id = commit_family_change_set(
            &workspace,
            &session,
            FamilyChangeSet {
                human_id: None,
                partners: vec![
                    PartnerInput::Existing(existing.clone()),
                    PartnerInput::New {
                        given: Some("Grace".to_owned()),
                        surname: Some("Hopper".to_owned()),
                    },
                ],
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create");

        let family = show_family(&workspace, &human_id).await.expect("show").expect("family");
        assert_eq!(
            family.partners.len(),
            2,
            "both the existing and the new partner are added"
        );
        let persons = list_persons(&workspace).await.expect("persons");
        assert_eq!(persons.len(), 2, "the inline partner person is created");
        let created = persons
            .iter()
            .find(|p| p.human_id != existing)
            .expect("the created partner");
        let created = show_person(&workspace, &created.human_id)
            .await
            .expect("show")
            .expect("person");
        assert_eq!(created.given.as_deref(), Some("Grace"));
        assert_eq!(created.surname.as_deref(), Some("Hopper"));
    }

    #[tokio::test]
    async fn a_new_partner_carries_the_change_set_provenance() {
        let (workspace, session, _dir) = setup().await;
        commit_family_change_set(
            &workspace,
            &session,
            FamilyChangeSet {
                human_id: None,
                partners: vec![PartnerInput::New {
                    given: Some("Ada".to_owned()),
                    surname: Some("Lovelace".to_owned()),
                }],
                provenance: Provenance {
                    confidence: Confidence::High,
                    rationale: Some("marriage record".to_owned()),
                    evidence_analysis: None,
                },
                citations: Vec::new(),
            },
        )
        .await
        .expect("create");
        let created = list_persons(&workspace).await.expect("persons")[0].human_id.clone();
        let log = crate::history::change_log_for_person(&workspace, &created)
            .await
            .expect("log");
        assert!(!log.is_empty());
        for entry in &log {
            assert_eq!(entry.confidence, Confidence::High);
            assert_eq!(entry.rationale.as_deref(), Some("marriage record"));
        }
    }

    #[tokio::test]
    async fn a_human_id_override_is_honoured_and_a_duplicate_is_rejected() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_family_change_set(
            &workspace,
            &session,
            FamilyChangeSet {
                human_id: Some("F0777".to_owned()),
                partners: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create");
        assert_eq!(human_id, "F0777", "the id override is used");

        let clash = commit_family_change_set(
            &workspace,
            &session,
            FamilyChangeSet {
                human_id: Some("F0777".to_owned()),
                partners: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await;
        assert!(matches!(clash, Err(crate::error::AppError::HumanIdTaken(id)) if id == "F0777"));
        let families = list_families(&workspace).await.expect("families");
        assert_eq!(families.len(), 1, "the duplicate id commits nothing");
    }

    #[tokio::test]
    async fn create_stamps_block_provenance_on_the_partner_assertions() {
        let (workspace, session, _dir) = setup().await;
        let a = person(&workspace, &session).await;
        let human_id = commit_family_change_set(
            &workspace,
            &session,
            FamilyChangeSet {
                human_id: None,
                partners: vec![PartnerInput::Existing(a)],
                provenance: Provenance {
                    confidence: Confidence::High,
                    rationale: Some("marriage record".to_owned()),
                    evidence_analysis: None,
                },
                citations: Vec::new(),
            },
        )
        .await
        .expect("create");
        let log = crate::history::change_log_for_family(&workspace, &human_id)
            .await
            .expect("log");
        assert!(!log.is_empty());
        for entry in &log {
            assert_eq!(entry.confidence, Confidence::High);
            assert_eq!(entry.rationale.as_deref(), Some("marriage record"));
        }
    }
}
