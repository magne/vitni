//! The DNA-test change-set use-case (Phase 5): a deferred create that commits a DNA test's anchoring
//! person (required), provider, test type, genome build, and kit id in a single operator action.
//!
//! Mirrors [`crate::source_change_set`], with a required person resolved to a `PersonId` **before any
//! write** (an unknown person rejects the whole change-set). Deferred fields (account, date tested,
//! SNP count) stay out (review-findings). Provenance follows the shared change-set rule
//! ([`crate::change_set`]). Editing an existing test is the per-field `dispatch_dna_test_edit` path
//! (PR27), not this create.

use genealogy_core::dna::{DnaGenomeBuild, DnaProvider, DnaTestType};
use genealogy_core::dna_test::command::{DnaTestCommand, DnaTestCommandEnvelope};
use genealogy_core::ids::HumanId;
use genealogy_core::provenance::CitationRef;
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// The desired end state of a new DNA test, committed as one operator action. The anchoring person is
/// required; the rest is optional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaTestChangeSet {
    /// A caller-supplied `human_id` override (dup-checked before any write); `None` auto-allocates.
    pub human_id: Option<String>,
    /// The anchoring person's `human_id` (required; resolved before any write).
    pub person: String,
    /// The testing provider.
    pub provider: Option<DnaProvider>,
    /// The test type (autosomal, Y-DNA, …).
    pub test_type: Option<DnaTestType>,
    /// The reference genome build.
    pub genome_build: Option<DnaGenomeBuild>,
    /// The kit id.
    pub kit_id: Option<String>,
    /// The operator intent stamped on every emitted command (`record-editing.html` §5b).
    pub provenance: Provenance,
    /// Citation `human_id`s backing every non-`Create*` command; resolved before any write.
    pub citations: Vec<String>,
}

/// Commits a [`DnaTestChangeSet`]: creates the test anchored to its person and emits a setter for each
/// filled field.
///
/// Returns the test's `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied `human_id` is in use, [`AppError::PersonNotFound`] if the
/// anchoring person is unknown (both validated before any write, so nothing commits),
/// [`AppError::CitationNotFound`] if a backing citation is unknown, [`AppError::DnaTestDomain`] on a
/// domain rejection, or a workspace/store error.
pub async fn commit_dna_test_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: DnaTestChangeSet,
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match &change_set.human_id {
        Some(id) => {
            if store.find_dna_test(id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id.clone()));
            }
            id.clone()
        }
        None => store.next_dna_test_human_id(&workspace.dna_test_id_format()?).await?,
    };
    let person_id = crate::person::resolve_person_id_public(store, &change_set.person).await?;
    let block = use_case::resolve_citation_refs(store, &change_set.citations).await?;

    let dna_test_id = session.new_dna_test_id();
    let aggregate_id = dna_test_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        DnaTestCommand::CreateDnaTest {
            dna_test_id,
            human_id: HumanId::new(&human_id),
            person_id,
        },
        change_set.provenance.clone(),
        Vec::new(),
    )
    .await?;

    if let Some(provider) = change_set.provider {
        execute(
            store,
            session,
            &aggregate_id,
            DnaTestCommand::SetProvider { dna_test_id, provider },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    if let Some(test_type) = change_set.test_type {
        execute(
            store,
            session,
            &aggregate_id,
            DnaTestCommand::SetTestType { dna_test_id, test_type },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    if let Some(genome_build) = change_set.genome_build {
        execute(
            store,
            session,
            &aggregate_id,
            DnaTestCommand::SetGenomeBuild {
                dna_test_id,
                genome_build,
            },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    if let Some(kit_id) = change_set.kit_id {
        execute(
            store,
            session,
            &aggregate_id,
            DnaTestCommand::SetKitId { dna_test_id, kit_id },
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
    command: DnaTestCommand,
    provenance: Provenance,
    citations: Vec<CitationRef>,
) -> Result<(), AppError> {
    let envelope = DnaTestCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_dna_test(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

#[cfg(test)]
mod tests {
    use super::{DnaTestChangeSet, commit_dna_test_change_set};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::dna_test::{list_dna_tests, show_dna_test};
    use crate::person::{NewPerson, create_person};
    use crate::session::Session;
    use crate::use_case::Provenance;
    use crate::workspace::Workspace;
    use genealogy_core::dna::{DnaProvider, DnaTestType};
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

    fn draft(person: String) -> DnaTestChangeSet {
        DnaTestChangeSet {
            human_id: None,
            person,
            provider: None,
            test_type: None,
            genome_build: None,
            kit_id: None,
            provenance: Provenance::default(),
            citations: Vec::new(),
        }
    }

    #[tokio::test]
    async fn create_commits_the_person_provider_and_kit_id() {
        let (workspace, session, _dir) = setup().await;
        let person = person(&workspace, &session).await;
        let human_id = commit_dna_test_change_set(
            &workspace,
            &session,
            DnaTestChangeSet {
                provider: Some(DnaProvider::AncestryDna),
                test_type: Some(DnaTestType::Autosomal),
                kit_id: Some("AB-1234".to_owned()),
                ..draft(person.clone())
            },
        )
        .await
        .expect("create");

        let test = show_dna_test(&workspace, &human_id).await.expect("show").expect("test");
        assert_eq!(test.provider, Some(DnaProvider::AncestryDna));
        assert_eq!(test.test_type, Some(DnaTestType::Autosomal));
        assert_eq!(test.kit_id.as_deref(), Some("AB-1234"));
        assert_eq!(test.person.map(|p| p.human_id), Some(person));
    }

    #[tokio::test]
    async fn a_person_only_draft_creates_a_bare_test() {
        let (workspace, session, _dir) = setup().await;
        let person = person(&workspace, &session).await;
        let human_id = commit_dna_test_change_set(&workspace, &session, draft(person))
            .await
            .expect("create");
        let test = show_dna_test(&workspace, &human_id).await.expect("show").expect("test");
        assert_eq!(test.provider, None);
    }

    #[tokio::test]
    async fn an_unknown_person_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let result = commit_dna_test_change_set(&workspace, &session, draft("I9999".to_owned())).await;
        assert!(matches!(result, Err(crate::error::AppError::PersonNotFound(_))));
        let tests = list_dna_tests(&workspace).await.expect("tests");
        assert!(tests.is_empty(), "nothing commits when the person is unknown");
    }

    #[tokio::test]
    async fn create_stamps_block_provenance_on_every_command() {
        let (workspace, session, _dir) = setup().await;
        let person = person(&workspace, &session).await;
        let human_id = commit_dna_test_change_set(
            &workspace,
            &session,
            DnaTestChangeSet {
                provider: Some(DnaProvider::AncestryDna),
                provenance: Provenance {
                    confidence: Confidence::High,
                    rationale: Some("test kit".to_owned()),
                    evidence_analysis: None,
                },
                ..draft(person)
            },
        )
        .await
        .expect("create");
        let log = crate::history::change_log_for_dna_test(&workspace, &human_id)
            .await
            .expect("log");
        assert!(!log.is_empty());
        for entry in &log {
            assert_eq!(entry.confidence, Confidence::High);
            assert_eq!(entry.rationale.as_deref(), Some("test kit"));
        }
    }
}
