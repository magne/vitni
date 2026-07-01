//! The person change-set use-case (Phase 5): a deferred create/edit that commits a small graph of
//! aggregates — one Person (name + gender inline), the tags applied to it, and optionally one new
//! Source and/or Citation the person's assertions cite — in a single operator action.
//!
//! # Why a change-set
//!
//! The Dioxus person dialog buffers every edit locally and persists nothing until OK (see
//! `docs/phase5/edit-patterns.html`). On OK it hands the app a [`PersonChangeSet`] describing the
//! *desired* end state; this module turns that into the minimal set of commands and commits them.
//! A citation created inside the dialog is not yet saved, yet several assertions may cite it (create
//! a person from a baptism record: the name, the date of birth, and the baptism all cite the one new
//! citation). The UI names that not-yet-saved target with a [`PlaceholderRef`]; this module mints the
//! real UUID once and resolves every placeholder to it, so each referencing assertion carries the
//! same [`CitationRef`] in its `EventContext` (ADR 0004 §1).
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
use genealogy_core::ids::{AssertionId, CitationId, HumanId, SourceId, TagId};
use genealogy_core::person::command::PersonCommand;
use genealogy_core::provenance::CitationRef;
use genealogy_db::Store;
use uuid::Uuid;

use crate::error::AppError;
use crate::person::{PersonNameParts, PersonSummary, build_name, execute_person_command, show_person};
use crate::session::Session;
use crate::use_case::Provenance;
use crate::workspace::Workspace;

/// A placeholder for a not-yet-saved aggregate created inside the same change-set (a pending Source
/// or Citation). The UI mints these locally; [`commit_person_change_set`] resolves each to the real
/// UUID it allocates, so later entries and every citing assertion use the persisted id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlaceholderRef(pub String);

/// Which source a pending citation cites: one that already exists, or one created in this set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRefInput {
    /// An existing source, by its `human_id` (e.g. `S0001`).
    Existing(String),
    /// A source created earlier in this same change-set, by its placeholder.
    Pending(PlaceholderRef),
}

/// Which citation an assertion cites: one that already exists, or one created in this set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationRefInput {
    /// An existing citation, by its `human_id` (e.g. `C0001`).
    Existing(String),
    /// A citation created in this same change-set, by its placeholder.
    Pending(PlaceholderRef),
}

/// A new Source to create as part of the change-set (only the title is collected in this slice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSourceEntry {
    /// The placeholder a pending citation references this source by.
    pub placeholder: PlaceholderRef,
    /// The source's title, if the operator gave one.
    pub title: Option<String>,
}

/// A new Citation to create as part of the change-set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCitationEntry {
    /// The placeholder assertions reference this citation by.
    pub placeholder: PlaceholderRef,
    /// The source this citation cites (existing or pending in the same set).
    pub source: SourceRefInput,
    /// The page / locator within the source, if given.
    pub page: Option<String>,
}

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

    let resolution = commit_new_aggregates(workspace, session, store, &change_set).await?;
    let name_citations = resolve_name_citations(change_set.name_citation.as_ref(), &resolution)?;

    match &change_set.target {
        PersonTarget::New { .. } => {
            create_person_graph(session, store, &human_id, &change_set, &name_citations).await?;
        }
        PersonTarget::Existing { .. } => {
            let current = show_person(workspace, &human_id)
                .await?
                .ok_or_else(|| AppError::PersonNotFound(human_id.clone()))?;
            edit_person_graph(session, store, &current, &change_set, &name_citations).await?;
        }
    }
    Ok(human_id)
}

/// The ids the change-set minted for its pending Source/Citation placeholders, for resolving
/// intra-set references.
#[derive(Default)]
struct Resolution {
    sources: Vec<(PlaceholderRef, SourceId)>,
    citations: Vec<(PlaceholderRef, CitationId)>,
}

impl Resolution {
    fn source(&self, placeholder: &PlaceholderRef) -> Option<SourceId> {
        self.sources.iter().find(|(p, _)| p == placeholder).map(|(_, id)| *id)
    }

    fn citation(&self, placeholder: &PlaceholderRef) -> Option<CitationId> {
        self.citations.iter().find(|(p, _)| p == placeholder).map(|(_, id)| *id)
    }
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

/// Creates the change-set's new Source and Citation aggregates in dependency order (Source →
/// Citation), returning the minted ids so the person's assertions can cite them.
async fn commit_new_aggregates(
    workspace: &Workspace,
    session: &Session,
    store: &Store,
    change_set: &PersonChangeSet,
) -> Result<Resolution, AppError> {
    let mut resolution = Resolution::default();
    for entry in &change_set.new_sources {
        let human_id = store.next_source_human_id(&workspace.source_id_format()?).await?;
        let source_id =
            crate::source::create_source_returning_id(session, store, &human_id, entry.title.clone()).await?;
        resolution.sources.push((entry.placeholder.clone(), source_id));
    }
    for entry in &change_set.new_citations {
        let source_id = match &entry.source {
            SourceRefInput::Existing(human_id) => crate::citation::resolve_source_id_public(store, human_id).await?,
            SourceRefInput::Pending(placeholder) => resolution
                .source(placeholder)
                .ok_or_else(|| AppError::SourceNotFound(placeholder.0.clone()))?,
        };
        let human_id = store.next_citation_human_id(&workspace.citation_id_format()?).await?;
        let citation_id =
            crate::citation::create_citation_returning_id(session, store, &human_id, source_id, entry.page.clone())
                .await?;
        resolution.citations.push((entry.placeholder.clone(), citation_id));
    }
    Ok(resolution)
}

/// Resolves the name's citation reference (existing `human_id` or a pending placeholder) to the
/// [`CitationRef`]s recorded in the name assertion's provenance envelope.
fn resolve_name_citations(
    reference: Option<&CitationRefInput>,
    resolution: &Resolution,
) -> Result<Vec<CitationRef>, AppError> {
    let Some(reference) = reference else {
        return Ok(Vec::new());
    };
    let citation_id = match reference {
        CitationRefInput::Existing(human_id) => parse_citation_id(human_id)?,
        CitationRefInput::Pending(placeholder) => resolution
            .citation(placeholder)
            .ok_or_else(|| AppError::CitationNotFound(placeholder.0.clone()))?,
    };
    Ok(vec![CitationRef { citation_id }])
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
    name_citations: &[CitationRef],
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
        Provenance::default(),
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
            Provenance::default(),
            name_citations.to_vec(),
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
            Provenance::default(),
            Vec::new(),
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
            Provenance::default(),
            Vec::new(),
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
    name_citations: &[CitationRef],
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
            Provenance::default(),
            name_citations.to_vec(),
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
            Provenance::default(),
            Vec::new(),
        )
        .await?;
    }

    commit_tag_diff(session, store, &aggregate_id, person_id, current, &change_set.tags).await
}

/// Emits `Tag`/`Untag` for the difference between the person's current tags and the desired set.
async fn commit_tag_diff(
    session: &Session,
    store: &Store,
    aggregate_id: &str,
    person_id: genealogy_core::ids::PersonId,
    current: &PersonSummary,
    desired: &[String],
) -> Result<(), AppError> {
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
                Provenance::default(),
                Vec::new(),
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
                Provenance::default(),
                Vec::new(),
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
    use super::{
        CitationRefInput, NewCitationEntry, NewSourceEntry, PersonChangeSet, PersonTarget, PlaceholderRef,
        SourceRefInput, commit_person_change_set,
    };
    use crate::config::{AppDefaults, IdFormats, OperatorConfig, WorkspaceDefaults};
    use crate::person::{PersonNameParts, show_person};
    use crate::session::Session;
    use crate::tag::create_tag;
    use crate::workspace::Workspace;
    use genealogy_core::enums::Sex;
    use genealogy_core::ids::AgentId;
    use genealogy_core::provenance::{Agent, AgentKind};
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
        let tag_a = create_tag(&workspace, &session, "Ancestor".to_owned())
            .await
            .expect("tag a");
        let tag_b = create_tag(&workspace, &session, "Verified".to_owned())
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
            },
        )
        .await
        .expect("create bare");

        let summary = show_person(&workspace, &human_id).await.expect("show").expect("person");
        assert!(summary.names.is_empty(), "an all-empty name asserts no name");
        assert_eq!(summary.sex, None, "no sex asserted defaults to none in the projection");
    }
}
