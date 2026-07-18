//! The person change-set use-case (Phase 5): a deferred create/edit that commits a small graph of
//! aggregates — one Person (name + gender inline), the tags applied to it, and optionally one new
//! Source and/or Citation the person's assertions cite — in a single operator action.
//!
//! # Why a change-set
//!
//! The Dioxus person dialog buffers every edit locally and persists nothing until OK (see
//! `docs/mockups/edit-patterns.html`). On OK it hands the app a [`PersonChangeSet`] describing the
//! *desired* end state; this module turns that into the minimal set of commands and commits them.
//! A citation created inside the dialog is not yet saved, yet several assertions may cite it (create
//! a person from a baptism record: the name, the date of birth, and the baptism all cite the one new
//! citation). The UI names that not-yet-saved target with a [`PlaceholderRef`]; this module mints the
//! real UUID once and resolves every placeholder to it, so each referencing assertion carries the
//! same [`EvidenceRef`] in its `EventContext` (ADR 0004 §1).
//!
//! # Commit semantics (storage constraint, ADR 0002)
//!
//! cqrs-es 0.5 commits each aggregate's events in that aggregate's own transaction; there is no
//! public seam to share one transaction across the per-aggregate frameworks. So the change-set is a
//! **sequenced** commit, not a single-transaction atomic one: every up-front validation (a
//! supplied `human_id` must be free) runs *before any write*, so the common rejection commits
//! nothing; then dependencies are written in dependency order (Source → Citation → Person) so a
//! reference never dangles. A mid-sequence infrastructure failure can leave a new Source/Citation
//! written without the Person — those are inert orphans (nothing references them), not corruption,
//! and the event log stays append-only. Full cross-aggregate atomicity is a documented follow-up
//! (it needs a bespoke `PersistedEventRepository` sharing a transaction handle).

use std::collections::BTreeSet;

use genealogy_core::enums::{EvidenceLevel, Sex};
use genealogy_core::ids::{AssertionId, CitationId, HumanId, TagId};
use genealogy_core::person::command::PersonCommand;
use genealogy_core::provenance::EvidenceRef;
use genealogy_db::Store;
use uuid::Uuid;

use crate::change_set::{
    CitationRefInput, NewCitationEntry, NewSourceEntry, Resolution, commit_pending_sources_and_citations,
};
use crate::error::AppError;
use crate::person::{PersonNameParts, PersonSummary, build_name, execute_person_command, show_person};
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// Whether the change-set creates a new person or edits an existing one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonTarget {
    /// Create a new person; `human_id` is auto-allocated when `None`, else validated unique.
    New {
        /// A caller-supplied `human_id` override; `None` auto-allocates the next free one.
        human_id: Option<String>,
    },
    /// Edit the existing person with this `human_id`; only the diff is committed.
    Existing {
        /// The `human_id` of the person to edit.
        human_id: String,
    },
}

/// The desired end state of a person and the aggregates its vital assertions cite, committed as one
/// operator action. For a create this is the whole person; for an edit it is diffed against the
/// person's current projection so only changed assertions are emitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonChangeSet {
    /// Create a new person or edit an existing one.
    pub target: PersonTarget,
    /// The preferred name, or `None` to leave the name unchanged (edit) / unset (create).
    pub name: Option<PersonNameParts>,
    /// The citation backing the preferred name, if the operator attached one.
    pub name_citation: Option<CitationRefInput>,
    /// The person's sex, or `None` to leave it unchanged (edit) / default to `Unknown` (create).
    pub sex: Option<Sex>,
    /// The tags that should be applied when the commit finishes (the desired set), by tag aggregate
    /// id (a UUID string — tags carry no `human_id`, data-model §9). On edit the commit diffs this
    /// against the currently-applied tags and emits only the add/remove events.
    pub tags: Vec<String>,
    /// New sources to create in this set (referenced by pending citations).
    pub new_sources: Vec<NewSourceEntry>,
    /// New citations to create in this set (referenced by the name citation).
    pub new_citations: Vec<NewCitationEntry>,
    /// The operator intent (confidence · rationale · evidence analysis) captured once in the save's
    /// provenance block and stamped on every emitted command (`record-editing.html` §5b).
    pub provenance: Provenance,
    /// Citation `human_id`s from the provenance block, backing every non-`Create*` assertion. Resolved
    /// before any write; an unknown id rejects the whole change-set.
    pub citations: Vec<String>,
}

/// Commits a [`PersonChangeSet`]: creates/edits the person and any new Source/Citation it cites, in
/// one operator action, resolving intra-set placeholder references to the ids it mints.
///
/// Returns the person's `human_id` (the assigned one on create).
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied `human_id` is in use (validated before any write, so
/// nothing commits), [`AppError::PersonNotFound`] if an edited person is gone,
/// [`AppError::CitationNotFound`]/[`AppError::SourceNotFound`] if a referenced existing record is
/// unknown, [`AppError::Domain`] on a domain rejection (e.g. an empty name), or a workspace/store
/// error.
pub async fn commit_person_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: PersonChangeSet,
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = resolve_person_human_id(workspace, store, &change_set.target).await?;
    // Resolve the provenance block's backing citations before any write, so an unknown id rejects
    // the whole change-set (nothing commits).
    let block_citations = use_case::resolve_citation_refs(store, &change_set.citations).await?;

    let resolution = commit_pending_sources_and_citations(
        workspace,
        session,
        store,
        &change_set.new_sources,
        &change_set.new_citations,
        &change_set.provenance,
    )
    .await?;
    let name_citations = resolve_name_citations(change_set.name_citation.as_ref(), &resolution)?;

    match &change_set.target {
        PersonTarget::New { .. } => {
            create_person_graph(
                session,
                store,
                &human_id,
                &change_set,
                &name_citations,
                &block_citations,
            )
            .await?;
        }
        PersonTarget::Existing { .. } => {
            let current = show_person(workspace, &human_id)
                .await?
                .ok_or_else(|| AppError::PersonNotFound(human_id.clone()))?;
            edit_person_graph(session, store, &current, &change_set, &name_citations, &block_citations).await?;
        }
    }
    Ok(human_id)
}

/// Merges the name's specific citations with the provenance block's backing citations, dropping any
/// duplicate so an assertion never cites the same citation twice.
fn merge_citations(specific: &[EvidenceRef], block: &[EvidenceRef]) -> Vec<EvidenceRef> {
    let mut merged = specific.to_vec();
    for reference in block {
        if !merged.iter().any(|existing| existing == reference) {
            merged.push(*reference);
        }
    }
    merged
}

/// Validates/allocates the person's `human_id` before any write, so a duplicate-id rejection commits
/// nothing.
async fn resolve_person_human_id(
    workspace: &Workspace,
    store: &Store,
    target: &PersonTarget,
) -> Result<String, AppError> {
    match target {
        PersonTarget::New { human_id: Some(id) } => {
            if store.find_person(id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id.clone()));
            }
            Ok(id.clone())
        }
        PersonTarget::New { human_id: None } => Ok(store.next_person_human_id(&workspace.person_id_format()?).await?),
        PersonTarget::Existing { human_id } => Ok(human_id.clone()),
    }
}

/// Resolves the name's citation reference (existing `human_id` or a pending placeholder) to the
/// [`EvidenceRef`]s recorded in the name assertion's provenance envelope.
fn resolve_name_citations(
    reference: Option<&CitationRefInput>,
    resolution: &Resolution,
) -> Result<Vec<EvidenceRef>, AppError> {
    let Some(reference) = reference else {
        return Ok(Vec::new());
    };
    let citation_id = match reference {
        CitationRefInput::Existing(human_id) => parse_citation_id(human_id)?,
        CitationRefInput::Pending(placeholder) => resolution
            .citation(placeholder)
            .ok_or_else(|| AppError::CitationNotFound(placeholder.0.clone()))?,
    };
    Ok(vec![EvidenceRef::Citation(citation_id)])
}

/// Parses a citation reference expressed as a raw aggregate-id UUID string (the pending path resolves
/// to a minted id; an existing reference resolved earlier is passed by id).
fn parse_citation_id(id: &str) -> Result<CitationId, AppError> {
    Uuid::parse_str(id)
        .map(CitationId::from_uuid)
        .map_err(|_| AppError::CitationNotFound(id.to_owned()))
}

/// Parses the primary name assertion's id (a UUID string surfaced on [`PersonSummary`]); a malformed
/// id yields `None`, so the edit falls back to a plain `AssertName` rather than failing.
fn parse_assertion_id(id: &str) -> Option<AssertionId> {
    Uuid::parse_str(id).ok().map(AssertionId::from_uuid)
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`]. The id
/// is resolved from a tag the user picked by name; it is never shown to the user (data-model §9).
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Emits the create-person command graph: `CreatePerson`, then the name (with its citations), sex,
/// and tags — the person's full initial state.
async fn create_person_graph(
    session: &Session,
    store: &Store,
    human_id: &str,
    change_set: &PersonChangeSet,
    name_citations: &[EvidenceRef],
    block_citations: &[EvidenceRef],
) -> Result<(), AppError> {
    let person_id = session.new_person_id();
    let aggregate_id = person_id.to_string();
    execute_person_command(
        store,
        session,
        &aggregate_id,
        PersonCommand::CreatePerson {
            person_id,
            human_id: HumanId::new(human_id),
            evidence_level: EvidenceLevel::Conclusion,
        },
        change_set.provenance.clone(),
        Vec::new(),
    )
    .await?;

    if let Some(parts) = change_set.name.clone().filter(|parts| !parts.is_empty()) {
        let name = build_name(parts);
        execute_person_command(
            store,
            session,
            &aggregate_id,
            PersonCommand::AssertName { person_id, name },
            change_set.provenance.clone(),
            merge_citations(name_citations, block_citations),
        )
        .await?;
    }

    if let Some(sex) = &change_set.sex {
        execute_person_command(
            store,
            session,
            &aggregate_id,
            PersonCommand::AssertSex {
                person_id,
                sex: sex.clone(),
            },
            change_set.provenance.clone(),
            block_citations.to_vec(),
        )
        .await?;
    }

    for raw in &change_set.tags {
        let tag_id = parse_tag_id(raw)?;
        execute_person_command(
            store,
            session,
            &aggregate_id,
            PersonCommand::Tag { person_id, tag_id },
            change_set.provenance.clone(),
            block_citations.to_vec(),
        )
        .await?;
    }
    Ok(())
}

/// Emits only the assertions that differ from the person's current projection: a changed name, a
/// changed sex, and the tag add/remove diff.
async fn edit_person_graph(
    session: &Session,
    store: &Store,
    current: &PersonSummary,
    change_set: &PersonChangeSet,
    name_citations: &[EvidenceRef],
    block_citations: &[EvidenceRef],
) -> Result<(), AppError> {
    let person_id = crate::person::resolve_person_id_public(store, &current.human_id).await?;
    let aggregate_id = person_id.to_string();

    if let Some(parts) = change_set.name.clone().filter(|parts| !parts.is_empty())
        && (name_changed(current, &parts) || !name_citations.is_empty())
    {
        let name = build_name(parts);
        let assert = PersonCommand::AssertName { person_id, name };
        // Supersede the current primary name so the changed preferred name replaces it (rather than
        // becoming a buried second name — the projection's primary is the first-asserted).
        let command = match current.primary_name_assertion.as_deref().and_then(parse_assertion_id) {
            Some(target) => PersonCommand::SupersedeAssertion {
                person_id,
                target,
                replacement: Box::new(assert),
            },
            None => assert,
        };
        execute_person_command(
            store,
            session,
            &aggregate_id,
            command,
            change_set.provenance.clone(),
            merge_citations(name_citations, block_citations),
        )
        .await?;
    }

    if let Some(sex) = &change_set.sex
        && current.sex.as_ref() != Some(sex)
    {
        execute_person_command(
            store,
            session,
            &aggregate_id,
            PersonCommand::AssertSex {
                person_id,
                sex: sex.clone(),
            },
            change_set.provenance.clone(),
            block_citations.to_vec(),
        )
        .await?;
    }

    commit_tag_diff(
        session,
        store,
        &aggregate_id,
        person_id,
        current,
        change_set,
        block_citations,
    )
    .await
}

/// Emits `Tag`/`Untag` for the difference between the person's current tags and the desired set.
async fn commit_tag_diff(
    session: &Session,
    store: &Store,
    aggregate_id: &str,
    person_id: genealogy_core::ids::PersonId,
    current: &PersonSummary,
    change_set: &PersonChangeSet,
    block_citations: &[EvidenceRef],
) -> Result<(), AppError> {
    let desired = &change_set.tags;
    let current_tags: BTreeSet<&str> = current.tags.iter().map(String::as_str).collect();
    let desired_tags: BTreeSet<&str> = desired.iter().map(String::as_str).collect();
    for raw in desired {
        if !current_tags.contains(raw.as_str()) {
            let tag_id = parse_tag_id(raw)?;
            execute_person_command(
                store,
                session,
                aggregate_id,
                PersonCommand::Tag { person_id, tag_id },
                change_set.provenance.clone(),
                block_citations.to_vec(),
            )
            .await?;
        }
    }
    for current_tag in &current.tags {
        if !desired_tags.contains(current_tag.as_str()) {
            let tag_id = parse_tag_id(current_tag)?;
            execute_person_command(
                store,
                session,
                aggregate_id,
                PersonCommand::Untag { person_id, tag_id },
                change_set.provenance.clone(),
                block_citations.to_vec(),
            )
            .await?;
        }
    }
    Ok(())
}

/// Whether the desired name differs from the person's current primary name (its given/surname and the
/// structured parts the dialog edits).
fn name_changed(current: &PersonSummary, parts: &PersonNameParts) -> bool {
    current.given != parts.given
        || current.surname != parts.surname
        || current.surname_prefix != parts.surname_prefix
        || current.nickname != parts.nickname
        || current.name_prefix != parts.prefix
        || current.name_suffix != parts.suffix
        || current.name_type.as_ref() != Some(&parts.name_type)
}

#[cfg(test)]
mod tests {
    use super::{PersonChangeSet, PersonTarget, commit_person_change_set};
    use crate::change_set::{CitationRefInput, NewCitationEntry, NewSourceEntry, PlaceholderRef, SourceRefInput};
    use crate::config::{AppDefaults, IdFormats, OperatorConfig, WorkspaceDefaults};
    use crate::history::change_log_for_person;
    use crate::person::{PersonNameParts, show_person};
    use crate::session::Session;
    use crate::tag::create_tag;
    use crate::use_case::Provenance;
    use crate::workspace::Workspace;
    use genealogy_core::enums::Sex;
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

    fn defaults() -> WorkspaceDefaults {
        WorkspaceDefaults {
            id_formats: IdFormats {
                person: "I%04d".to_owned(),
                family: "F%04d".to_owned(),
                place: "P%04d".to_owned(),
                source: "S%04d".to_owned(),
                citation: "C%04d".to_owned(),
                event: "E%04d".to_owned(),
                dna_test: "D%04d".to_owned(),
                dna_match: "X%04d".to_owned(),
                repository: "R%04d".to_owned(),
                note: "N%04d".to_owned(),
                media: "O%04d".to_owned(),
            },
            ..Default::default()
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
        let workspace = Workspace::open(&ws, &operator(), &defaults()).await.expect("open");
        (workspace, session(), dir)
    }

    fn name(given: &str, surname: &str) -> PersonNameParts {
        PersonNameParts::simple(Some(given.to_owned()), Some(surname.to_owned()))
    }

    #[tokio::test]
    async fn create_with_name_gender_and_a_new_shared_citation_commits_once() {
        let (workspace, session, _dir) = setup().await;
        let change_set = PersonChangeSet {
            target: PersonTarget::New { human_id: None },
            name: Some(name("John", "Smith")),
            name_citation: Some(CitationRefInput::Pending(PlaceholderRef("c1".to_owned()))),
            sex: Some(Sex::Male),
            tags: Vec::new(),
            new_sources: vec![NewSourceEntry {
                placeholder: PlaceholderRef("s1".to_owned()),
                title: Some("Baptism register".to_owned()),
            }],
            new_citations: vec![NewCitationEntry {
                placeholder: PlaceholderRef("c1".to_owned()),
                source: SourceRefInput::Pending(PlaceholderRef("s1".to_owned())),
                page: Some("p. 14".to_owned()),
            }],
            provenance: Provenance::default(),
            citations: Vec::new(),
        };
        let human_id = commit_person_change_set(&workspace, &session, change_set)
            .await
            .expect("commit");

        let summary = show_person(&workspace, &human_id).await.expect("show").expect("person");
        assert_eq!(summary.given.as_deref(), Some("John"));
        assert_eq!(summary.surname.as_deref(), Some("Smith"));
        assert_eq!(summary.sex, Some(Sex::Male));
        // The name assertion cites the newly-minted citation (its source-count is 1).
        assert_eq!(summary.names.len(), 1);
        assert_eq!(summary.names[0].source_count, 1, "the pending citation backs the name");
    }

    #[tokio::test]
    async fn a_pending_citation_is_minted_once_and_shared_by_every_referencing_assertion() {
        // The name references the pending citation `c1`; the citation itself references pending source
        // `s1`. Both placeholders resolve to a single minted aggregate id.
        let (workspace, session, _dir) = setup().await;
        let change_set = PersonChangeSet {
            target: PersonTarget::New { human_id: None },
            name: Some(name("Mary", "Doe")),
            name_citation: Some(CitationRefInput::Pending(PlaceholderRef("c1".to_owned()))),
            sex: None,
            tags: Vec::new(),
            new_sources: vec![NewSourceEntry {
                placeholder: PlaceholderRef("s1".to_owned()),
                title: None,
            }],
            new_citations: vec![NewCitationEntry {
                placeholder: PlaceholderRef("c1".to_owned()),
                source: SourceRefInput::Pending(PlaceholderRef("s1".to_owned())),
                page: None,
            }],
            provenance: Provenance::default(),
            citations: Vec::new(),
        };
        let human_id = commit_person_change_set(&workspace, &session, change_set)
            .await
            .expect("commit");

        // Exactly one citation and one source exist after the commit (the pending pair, minted once).
        let citations = crate::citation::list_citations(&workspace).await.expect("citations");
        let sources = crate::source::list_sources(&workspace).await.expect("sources");
        assert_eq!(citations.len(), 1, "the pending citation is minted exactly once");
        assert_eq!(sources.len(), 1, "the pending source is minted exactly once");
        let summary = show_person(&workspace, &human_id).await.expect("show").expect("person");
        // The citation is carried in the name assertion's provenance envelope (its source-count),
        // not as a separate INDI.SOUR attachment.
        assert_eq!(summary.names.len(), 1);
        assert_eq!(
            summary.names[0].source_count, 1,
            "the name's citation resolves to the minted one"
        );
    }

    #[tokio::test]
    async fn a_duplicate_human_id_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        // Take I0001 with a first person.
        let taken = commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::New { human_id: None },
                name: Some(name("First", "Person")),
                name_citation: None,
                sex: None,
                tags: Vec::new(),
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("first commit");

        // A second create supplying the taken id, plus a pending source, must reject before any write.
        let result = commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::New {
                    human_id: Some(taken.clone()),
                },
                name: Some(name("Clash", "Person")),
                name_citation: None,
                sex: None,
                tags: Vec::new(),
                new_sources: vec![NewSourceEntry {
                    placeholder: PlaceholderRef("s1".to_owned()),
                    title: Some("Would-be orphan".to_owned()),
                }],
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await;

        assert!(matches!(result, Err(crate::error::AppError::HumanIdTaken(_))));
        // No source was written — the id check runs before any aggregate is created.
        let sources = crate::source::list_sources(&workspace).await.expect("sources");
        assert!(sources.is_empty(), "nothing commits when the human_id is taken");
        let persons = crate::person::list_persons(&workspace).await.expect("persons");
        assert_eq!(persons.len(), 1, "only the original person exists");
    }

    #[tokio::test]
    async fn an_edit_emits_only_the_diff() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::New { human_id: None },
                name: Some(name("Jane", "Doe")),
                name_citation: None,
                sex: Some(Sex::Female),
                tags: Vec::new(),
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create");

        // Edit: keep the same name and sex, only change the surname. Expect one NameAsserted, no
        // SexAsserted (sex unchanged), no name re-assert if unchanged.
        commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::Existing {
                    human_id: human_id.clone(),
                },
                name: Some(name("Jane", "Smith")),
                name_citation: None,
                sex: Some(Sex::Female),
                tags: Vec::new(),
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("edit");

        let summary = show_person(&workspace, &human_id).await.expect("show").expect("person");
        assert_eq!(
            summary.surname.as_deref(),
            Some("Smith"),
            "the changed name is now primary"
        );
        assert_eq!(summary.given.as_deref(), Some("Jane"));
        assert_eq!(summary.sex, Some(Sex::Female));
        // The edit supersedes the old primary name (not append), so exactly one live name remains.
        assert_eq!(
            summary.names.len(),
            1,
            "the edit supersedes the primary name rather than adding"
        );
    }

    #[tokio::test]
    async fn an_edit_with_no_changes_emits_nothing() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::New { human_id: None },
                name: Some(name("Sam", "Vimes")),
                name_citation: None,
                sex: Some(Sex::Male),
                tags: Vec::new(),
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create");

        commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::Existing {
                    human_id: human_id.clone(),
                },
                name: Some(name("Sam", "Vimes")),
                name_citation: None,
                sex: Some(Sex::Male),
                tags: Vec::new(),
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("no-op edit");

        let summary = show_person(&workspace, &human_id).await.expect("show").expect("person");
        assert_eq!(summary.names.len(), 1, "an unchanged name is not re-asserted");
    }

    #[tokio::test]
    async fn tags_diff_on_edit_applies_and_removes() {
        let (workspace, session, _dir) = setup().await;
        let tag_a = create_tag(&workspace, &session, "Ancestor".to_owned(), Provenance::default(), &[])
            .await
            .expect("tag a");
        let tag_b = create_tag(&workspace, &session, "Verified".to_owned(), Provenance::default(), &[])
            .await
            .expect("tag b");
        let human_id = commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::New { human_id: None },
                name: Some(name("Tag", "Test")),
                name_citation: None,
                sex: None,
                tags: vec![tag_a.clone()],
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create with tag a");
        let created = show_person(&workspace, &human_id).await.expect("show").expect("person");
        assert_eq!(created.tags, vec![tag_a.clone()]);

        // Edit to tags = {b}: removes a, adds b.
        commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::Existing {
                    human_id: human_id.clone(),
                },
                name: None,
                name_citation: None,
                sex: None,
                tags: vec![tag_b.clone()],
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("retag");

        let summary = show_person(&workspace, &human_id).await.expect("show").expect("person");
        assert_eq!(summary.tags, vec![tag_b.clone()], "tag a removed, tag b added");
    }

    #[tokio::test]
    async fn create_with_empty_name_and_no_sex_creates_a_bare_person() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::New { human_id: None },
                name: Some(PersonNameParts::simple(None, None)),
                name_citation: None,
                sex: None,
                tags: Vec::new(),
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create bare");

        let summary = show_person(&workspace, &human_id).await.expect("show").expect("person");
        assert!(summary.names.is_empty(), "an all-empty name asserts no name");
        assert_eq!(summary.sex, None, "no sex asserted defaults to none in the projection");
    }

    #[tokio::test]
    async fn create_stamps_block_provenance_on_every_assertion_and_citations_on_non_creates() {
        let (workspace, session, _dir) = setup().await;
        // Seed a citation to reference from the provenance block.
        commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::New { human_id: None },
                name: Some(name("Seed", "Person")),
                name_citation: None,
                sex: None,
                tags: Vec::new(),
                new_sources: vec![NewSourceEntry {
                    placeholder: PlaceholderRef("s".to_owned()),
                    title: Some("Register".to_owned()),
                }],
                new_citations: vec![NewCitationEntry {
                    placeholder: PlaceholderRef("c".to_owned()),
                    source: SourceRefInput::Pending(PlaceholderRef("s".to_owned())),
                    page: None,
                }],
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("seed");
        let cite = crate::citation::list_citations(&workspace).await.expect("citations")[0]
            .human_id
            .clone();

        let human_id = commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::New { human_id: None },
                name: Some(name("John", "Smith")),
                name_citation: None,
                sex: Some(Sex::Male),
                tags: Vec::new(),
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance {
                    confidence: Some(Confidence::High),
                    rationale: Some("baptism record".to_owned()),
                    evidence_analysis: None,
                },
                citations: vec![cite],
            },
        )
        .await
        .expect("commit");

        let log = change_log_for_person(&workspace, &human_id).await.expect("log");
        assert!(!log.is_empty(), "the create emits assertions");
        for entry in &log {
            assert_eq!(
                entry.confidence,
                Some(Confidence::High),
                "every assertion carries the block confidence"
            );
            assert_eq!(entry.rationale.as_deref(), Some("baptism record"));
        }
        let non_creates: Vec<_> = log.iter().filter(|entry| entry.event_type != "PersonCreated").collect();
        assert!(!non_creates.is_empty(), "name and sex assertions exist");
        for entry in non_creates {
            assert_eq!(
                entry.citations.len(),
                1,
                "non-create assertions carry the block citation"
            );
        }
        let create = log
            .iter()
            .find(|entry| entry.event_type == "PersonCreated")
            .expect("create");
        assert!(
            create.citations.is_empty(),
            "the create command carries no backing citation"
        );
    }

    #[tokio::test]
    async fn create_with_an_unknown_block_citation_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let result = commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::New { human_id: None },
                name: Some(name("Ghost", "Citation")),
                name_citation: None,
                sex: None,
                tags: Vec::new(),
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: vec!["C9999".to_owned()],
            },
        )
        .await;

        assert!(matches!(result, Err(crate::error::AppError::CitationNotFound(_))));
        let persons = crate::person::list_persons(&workspace).await.expect("persons");
        assert!(persons.is_empty(), "nothing commits when a block citation is unknown");
    }

    async fn create_bare(workspace: &Workspace, session: &Session) -> String {
        commit_person_change_set(
            workspace,
            session,
            PersonChangeSet {
                target: PersonTarget::New { human_id: None },
                name: Some(name("John", "Smith")),
                name_citation: None,
                sex: None,
                tags: Vec::new(),
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create")
    }

    #[tokio::test]
    async fn set_person_human_id_renames_and_the_old_id_no_longer_resolves() {
        let (workspace, session, _dir) = setup().await;
        let human_id = create_bare(&workspace, &session).await;

        let renamed = crate::person::set_person_human_id(
            &workspace,
            &session,
            &human_id,
            Some("I0777".to_owned()),
            Provenance::default(),
        )
        .await
        .expect("rename");
        assert_eq!(renamed, "I0777");

        assert!(show_person(&workspace, &human_id).await.expect("show").is_none());
        let person = show_person(&workspace, "I0777").await.expect("show").expect("person");
        assert_eq!(person.human_id, "I0777");
        assert_eq!(
            person.given.as_deref(),
            Some("John"),
            "the rename leaves the other claims intact"
        );
    }

    #[tokio::test]
    async fn set_person_human_id_rejects_an_id_already_in_use() {
        let (workspace, session, _dir) = setup().await;
        let first = create_bare(&workspace, &session).await;
        let second = create_bare(&workspace, &session).await;

        let taken = crate::person::set_person_human_id(
            &workspace,
            &session,
            &second,
            Some(first.clone()),
            Provenance::default(),
        )
        .await;
        assert!(matches!(taken, Err(crate::error::AppError::HumanIdTaken(id)) if id == first));
    }

    #[tokio::test]
    async fn set_person_human_id_with_a_blank_id_regenerates_from_the_format() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_person_change_set(
            &workspace,
            &session,
            PersonChangeSet {
                target: PersonTarget::New {
                    human_id: Some("I9000".to_owned()),
                },
                name: Some(name("Mary", "Doe")),
                name_citation: None,
                sex: None,
                tags: Vec::new(),
                new_sources: Vec::new(),
                new_citations: Vec::new(),
                provenance: Provenance::default(),
                citations: Vec::new(),
            },
        )
        .await
        .expect("create");

        let renamed = crate::person::set_person_human_id(&workspace, &session, &human_id, None, Provenance::default())
            .await
            .expect("regenerate");
        assert_eq!(
            renamed, "I9001",
            "the next id from the I%04d format follows the existing max"
        );
    }
}
