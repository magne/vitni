//! The family change-set use-case (Phase 5): a deferred create that commits a family and its partners
//! (0..=2) in a single operator action.
//!
//! A family has no scalar form — the draft is just the partners (`family.html`). Every partner's
//! person `human_id` is resolved to a `PersonId` **before any write**, so an unknown partner rejects
//! the whole change-set (nothing commits). Provenance follows the shared change-set rule
//! ([`crate::change_set`]). Adding/removing partners on an existing family is the per-row path (PR30),
//! not this create.

use genealogy_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
use genealogy_core::ids::{HumanId, PersonId};
use genealogy_core::provenance::CitationRef;
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// The desired end state of a new family, committed as one operator action: the partner person
/// `human_id`s (0..=2, the family aggregate enforces the upper bound).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyChangeSet {
    /// The partners' person `human_id`s (resolved before any write).
    pub partners: Vec<String>,
    /// The operator intent stamped on every emitted command (`record-editing.html` §5b).
    pub provenance: Provenance,
    /// Citation `human_id`s backing every non-`Create*` command; resolved before any write.
    pub citations: Vec<String>,
}

/// Commits a [`FamilyChangeSet`]: creates the family and adds each resolved partner.
///
/// Returns the family's `human_id`.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if a partner `human_id` is unknown (validated before any write, so
/// nothing commits), [`AppError::CitationNotFound`] if a backing citation is unknown,
/// [`AppError::FamilyDomain`] on a domain rejection (e.g. a duplicate partner), or a workspace/store
/// error.
pub async fn commit_family_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: FamilyChangeSet,
) -> Result<String, AppError> {
    let store = workspace.store();
    // Resolve every partner and backing citation before any write, so an unknown reference rejects
    // the whole change-set.
    let mut partner_ids: Vec<PersonId> = Vec::with_capacity(change_set.partners.len());
    for human_id in &change_set.partners {
        partner_ids.push(crate::person::resolve_person_id_public(store, human_id).await?);
    }
    let block = use_case::resolve_citation_refs(store, &change_set.citations).await?;

    let human_id = store.next_family_human_id(&workspace.family_id_format()?).await?;
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
    use super::{FamilyChangeSet, commit_family_change_set};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::family::{list_families, show_family};
    use crate::person::{NewPerson, create_person};
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
                partners: vec![a.clone(), b.clone()],
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
                partners: vec!["I9999".to_owned()],
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
    async fn create_stamps_block_provenance_on_the_partner_assertions() {
        let (workspace, session, _dir) = setup().await;
        let a = person(&workspace, &session).await;
        let human_id = commit_family_change_set(
            &workspace,
            &session,
            FamilyChangeSet {
                partners: vec![a],
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
