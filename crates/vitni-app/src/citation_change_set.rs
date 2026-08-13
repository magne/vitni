//! The citation change-set use-case (Phase 5): a deferred create that commits a citation's source
//! (required), page, confidence, and Evidence Explained analysis in a single operator action —
//! including a source created inline (a §6b cascade).
//!
//! The source reference is either an existing source (validated before any write) or a source created
//! in this same set (a [`PlaceholderRef`] resolved through the shared pending-commit helper). No date
//! rides here — the cited-record date is asserted afterwards via [`crate::citation::assert_citation_date_value`].
//! The record-level confidence + evidence analysis are the citation's own
//! surety/analysis (distinct from the provenance block, which describes who/why asserted them).
//! Provenance follows the shared change-set rule.

use vitni_core::citation::command::{CitationCommand, CitationCommandEnvelope};
use vitni_core::ids::{CitationId, HumanId};
use vitni_core::provenance::{Confidence, EvidenceAnalysis, EvidenceRef};
use vitni_db::Store;

use crate::change_set::{NewSourceEntry, SourceRefInput, commit_pending_sources_and_citations};
use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// The desired end state of a new citation, committed as one operator action. The source is required
/// (existing or created inline); the rest is optional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationChangeSet {
    /// A caller-supplied `human_id` override (dup-checked before any write); `None` auto-allocates.
    pub human_id: Option<String>,
    /// The cited source (existing or pending in this set) — required.
    pub source: SourceRefInput,
    /// The page / locator within the source.
    pub page: Option<String>,
    /// The citation's own confidence in the cited claim.
    pub confidence: Option<Confidence>,
    /// The citation's Evidence Explained analysis.
    pub evidence: Option<EvidenceAnalysis>,
    /// New sources to create in this set (referenced by `source`).
    pub new_sources: Vec<NewSourceEntry>,
    /// The operator intent stamped on every emitted command (`record-editing.html` §5b).
    pub provenance: Provenance,
    /// Citation `human_id`s backing every non-`Create*` command; resolved before any write.
    pub citations: Vec<String>,
}

/// Commits a [`CitationChangeSet`]: creates any pending source, then the citation against its source,
/// and emits a setter for each filled field.
///
/// Returns the citation's `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied `human_id` is in use, [`AppError::SourceNotFound`] if the
/// cited source is unknown (both validated before any write), [`AppError::CitationNotFound`] if a
/// backing citation is unknown, [`AppError::CitationDomain`]/[`AppError::SourceDomain`] on a domain
/// rejection, or a workspace/store error.
pub async fn commit_citation_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: CitationChangeSet,
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match &change_set.human_id {
        Some(id) => {
            if store.find_citation(id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id.clone()));
            }
            id.clone()
        }
        None => store.next_citation_human_id(&workspace.citation_id_format()?).await?,
    };
    let block = use_case::resolve_citation_refs(store, &change_set.citations).await?;

    // Create any pending source (a no-op when the citation cites an existing source), then resolve the
    // cited source — an unknown existing source rejects before the citation is written.
    let resolution = commit_pending_sources_and_citations(
        workspace,
        session,
        store,
        &change_set.new_sources,
        &[],
        &change_set.provenance,
    )
    .await?;
    let source_id = match &change_set.source {
        SourceRefInput::Existing(source_human_id) => {
            crate::citation::resolve_source_id_public(store, source_human_id).await?
        }
        SourceRefInput::Pending(placeholder) => resolution
            .source(placeholder)
            .ok_or_else(|| AppError::SourceNotFound(placeholder.0.clone()))?,
    };

    let citation_id = session.new_citation_id();
    let aggregate_id = citation_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        CitationCommand::CreateCitation {
            citation_id,
            human_id: HumanId::new(&human_id),
            source_id,
        },
        change_set.provenance.clone(),
        Vec::new(),
    )
    .await?;
    for command in field_commands(citation_id, &change_set) {
        execute(
            store,
            session,
            &aggregate_id,
            command,
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    Ok(human_id)
}

/// The setter commands for the fields the operator filled, in a stable order.
fn field_commands(citation_id: CitationId, change_set: &CitationChangeSet) -> Vec<CitationCommand> {
    let mut commands = Vec::new();
    if let Some(page) = change_set.page.clone() {
        commands.push(CitationCommand::SetPage { citation_id, page });
    }
    if let Some(confidence) = change_set.confidence {
        commands.push(CitationCommand::SetConfidence {
            citation_id,
            confidence,
        });
    }
    if let Some(analysis) = change_set.evidence {
        commands.push(CitationCommand::SetEvidenceAnalysis { citation_id, analysis });
    }
    commands
}

/// Executes one command through the store, stamping the operator `provenance` and backing
/// `citations`, and mapping the command outcome to [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: CitationCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = CitationCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_citation(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

#[cfg(test)]
mod tests {
    use super::{CitationChangeSet, commit_citation_change_set};
    use crate::change_set::{NewSourceEntry, PlaceholderRef, SourceRefInput};
    use crate::citation::{list_citations, show_citation};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::session::Session;
    use crate::source::{NewSource, create_source, list_sources};
    use crate::use_case::Provenance;
    use crate::workspace::Workspace;
    use tempfile::TempDir;
    use uuid::Uuid;
    use vitni_core::ids::AgentId;
    use vitni_core::provenance::{Agent, AgentKind, Confidence};

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

    async fn source(ws: &Workspace, session: &Session) -> String {
        create_source(
            ws,
            session,
            NewSource {
                human_id: None,
                title: Some("Register".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("source")
    }

    #[tokio::test]
    async fn create_against_an_existing_source_commits_page_and_confidence() {
        let (workspace, session, _dir) = setup().await;
        let source = source(&workspace, &session).await;
        let human_id = commit_citation_change_set(
            &workspace,
            &session,
            CitationChangeSet {
                human_id: None,
                source: SourceRefInput::Existing(source),
                page: Some("f. 22".to_owned()),
                confidence: Some(Confidence::High),
                evidence: None,
                new_sources: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create");

        let citation = show_citation(&workspace, &human_id)
            .await
            .expect("show")
            .expect("citation");
        assert_eq!(citation.page.as_deref(), Some("f. 22"));
        assert_eq!(citation.confidence, Some(Confidence::High));
    }

    #[tokio::test]
    async fn a_pending_source_is_created_once_and_cited() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_citation_change_set(
            &workspace,
            &session,
            CitationChangeSet {
                human_id: None,
                source: SourceRefInput::Pending(PlaceholderRef("s1".to_owned())),
                page: None,
                confidence: None,
                evidence: None,
                new_sources: vec![NewSourceEntry {
                    placeholder: PlaceholderRef("s1".to_owned()),
                    title: Some("Parish register".to_owned()),
                }],
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create");

        let sources = list_sources(&workspace).await.expect("sources");
        assert_eq!(sources.len(), 1, "exactly one source is created");
        let citation = show_citation(&workspace, &human_id)
            .await
            .expect("show")
            .expect("citation");
        assert!(citation.source.is_some(), "the citation cites the new source");
    }

    #[tokio::test]
    async fn an_unknown_existing_source_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let result = commit_citation_change_set(
            &workspace,
            &session,
            CitationChangeSet {
                human_id: None,
                source: SourceRefInput::Existing("S9999".to_owned()),
                page: None,
                confidence: None,
                evidence: None,
                new_sources: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await;
        assert!(matches!(result, Err(crate::error::AppError::SourceNotFound(_))));
        let citations = list_citations(&workspace).await.expect("citations");
        assert!(citations.is_empty(), "nothing commits when the cited source is unknown");
    }
}
