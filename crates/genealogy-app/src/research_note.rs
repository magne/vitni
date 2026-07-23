//! `ResearchNote` use-cases (ADR 0028): create, set body, tag, restrict, show, and list — including
//! the reverse-by-subject query ("which arguments exist about this Person/Family/Event/Place").

use std::collections::BTreeSet;

use genealogy_core::enums::Restriction;
use genealogy_core::ids::{AssertionId, HumanId, ResearchNoteId, TagId};
use genealogy_core::name::LanguageTag;
use genealogy_core::provenance::EvidenceRef;
use genealogy_core::research_note::ResearchNoteView;
use genealogy_core::research_note::command::{ResearchNoteCommand, ResearchNoteCommandEnvelope};
use genealogy_core::research_note::subject::SubjectRef;
use genealogy_core::text::{MediaType, RichText};
use genealogy_db::Store;

use crate::citation::TagRef;
use crate::dto::tag_refs;
use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// A frontend-neutral summary of a research note (the DTO the CLI renders), carrying its stable id,
/// its subjects, and the joined tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchNoteSummary {
    /// The user-facing identifier (e.g. `A0001`).
    pub human_id: String,
    /// The stable `ResearchNoteId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The conclusion-bearing entities this argument is about (non-empty). Structured (not a label)
    /// so the frontend resolves each subject's own human-readable id and localizes the kind
    /// (ADR 0003).
    pub subjects: BTreeSet<SubjectRef>,
    /// The optional short title.
    pub title: Option<String>,
    /// The written argument text, if set.
    pub body: Option<String>,
    /// How the argument text is interpreted (Markdown/Plain/HTML).
    pub media_type: Option<MediaType>,
    /// The argument's language (a BCP-47 tag), if recorded.
    pub language: Option<String>,
    /// The applied tags (the Tags tab), by name/colour/priority.
    pub tags: Vec<TagRef>,
    /// The research note's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// Which conclusion-bearing aggregate a research note's subject names, identified by its
/// human-readable id (ADR 0028 §2) — the create-time input a frontend supplies (a CLI/GUI has only
/// `human_id`s to hand); [`resolve_subject`] turns it into the internal-id [`SubjectRef`] the
/// command carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewResearchNoteSubject {
    /// A Person, by its `human_id` (e.g. `I0001`).
    Person(String),
    /// A Family, by its `human_id`.
    Family(String),
    /// An Event, by its `human_id`.
    Event(String),
    /// A Place, by its `human_id`.
    Place(String),
}

/// What to create a research note with: the auto/override `human_id`, the required (non-empty)
/// subjects (by their human-readable ids), and an optional short title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewResearchNote {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// The conclusion-bearing entities this argument is about — must be non-empty (ADR 0028 §2);
    /// an empty list is rejected by `decide`'s `SubjectRequired` (`ResearchNoteError`).
    pub subjects: Vec<NewResearchNoteSubject>,
    /// An optional short title.
    pub title: Option<String>,
}

/// Resolves a [`NewResearchNoteSubject`] (a human-readable id) to the internal-id [`SubjectRef`]
/// the command carries, the same `human_id` → aggregate-id resolution every other cross-aggregate
/// reference goes through (e.g. [`use_case::resolve_citation_refs`]).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] / `FamilyNotFound` / `EventNotFound` / `PlaceNotFound` if the named
/// `human_id` is unknown, or a workspace/store error.
async fn resolve_subject(store: &Store, subject: NewResearchNoteSubject) -> Result<SubjectRef, AppError> {
    match subject {
        NewResearchNoteSubject::Person(human_id) => {
            let view = store
                .find_person(&human_id)
                .await?
                .ok_or_else(|| AppError::PersonNotFound(human_id.clone()))?;
            let id = view.person_id().ok_or(AppError::PersonNotFound(human_id))?;
            Ok(SubjectRef::Person(id))
        }
        NewResearchNoteSubject::Family(human_id) => {
            let view = store
                .find_family(&human_id)
                .await?
                .ok_or_else(|| AppError::FamilyNotFound(human_id.clone()))?;
            let id = view.family_id().ok_or(AppError::FamilyNotFound(human_id))?;
            Ok(SubjectRef::Family(id))
        }
        NewResearchNoteSubject::Event(human_id) => {
            let view = store
                .find_event(&human_id)
                .await?
                .ok_or_else(|| AppError::EventNotFound(human_id.clone()))?;
            let id = view.event_id().ok_or(AppError::EventNotFound(human_id))?;
            Ok(SubjectRef::Event(id))
        }
        NewResearchNoteSubject::Place(human_id) => {
            let view = store
                .find_place(&human_id)
                .await?
                .ok_or_else(|| AppError::PlaceNotFound(human_id.clone()))?;
            let id = view.place_id().ok_or(AppError::PlaceNotFound(human_id))?;
            Ok(SubjectRef::Place(id))
        }
    }
}

/// Creates a research note, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, a `*NotFound` error if a named subject's
/// `human_id` is unknown, [`AppError::ResearchNoteDomain`] with
/// [`genealogy_core::research_note::ResearchNoteError::SubjectRequired`] if `new.subjects` is empty
/// or with `UnknownSubject` on the rare race where a subject is removed between resolution and the
/// aggregate-tax check (ADR 0028 §2), or a workspace/store error.
pub async fn create_research_note(
    workspace: &Workspace,
    session: &Session,
    new: NewResearchNote,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
    let store = workspace.store();
    let mut subjects = BTreeSet::new();
    for subject in new.subjects {
        subjects.insert(resolve_subject(store, subject).await?);
    }
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_research_note(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => {
            store
                .next_research_note_human_id(&workspace.research_note_id_format()?)
                .await?
        }
    };
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;

    let research_note_id = session.new_research_note_id();
    execute(
        store,
        session,
        &research_note_id.to_string(),
        ResearchNoteCommand::CreateResearchNote {
            research_note_id,
            human_id: HumanId::new(&human_id),
            subjects,
            title: new.title,
        },
        provenance,
        citation_refs,
    )
    .await?;

    Ok(human_id)
}

/// Sets (or changes) a research note's written argument and its BCP-47 `language`, identified by
/// `human_id`.
///
/// # Errors
///
/// [`AppError::ResearchNoteNotFound`] if no such research note exists, or a workspace/store error.
pub async fn set_research_note_body(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    text: String,
    language: Option<String>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let research_note_id = resolve_research_note_id(store, human_id).await?;
    let body = RichText {
        text,
        media_type: MediaType::Markdown,
        language: language.map(LanguageTag::new),
        translator: None,
        translations: Vec::new(),
    };
    execute_research_note_mutation(
        store,
        session,
        research_note_id,
        ResearchNoteCommand::SetBody { research_note_id, body },
        meta,
    )
    .await
}

/// Adds a subject to an existing research note, identified by `human_id` — idempotent if the
/// resolved subject is already named (ADR 0028 §2).
///
/// # Errors
///
/// [`AppError::ResearchNoteNotFound`] if no such research note exists, a `*NotFound` error if
/// `subject`'s `human_id` is unknown, [`AppError::ResearchNoteDomain`] with
/// [`genealogy_core::research_note::ResearchNoteError::UnknownSubject`] on the rare race where the
/// subject is removed between resolution and the aggregate-tax check, or a workspace/store error.
pub async fn add_subject_to_research_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    subject: NewResearchNoteSubject,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let research_note_id = resolve_research_note_id(store, human_id).await?;
    let subject = resolve_subject(store, subject).await?;
    execute_research_note_mutation(
        store,
        session,
        research_note_id,
        ResearchNoteCommand::AddSubject {
            research_note_id,
            subject,
        },
        meta,
    )
    .await
}

/// Removes a subject from an existing research note, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::ResearchNoteNotFound`] if no such research note exists, a `*NotFound` error if
/// `subject`'s `human_id` is unknown, [`AppError::ResearchNoteDomain`] with
/// [`genealogy_core::research_note::ResearchNoteError::SubjectRequired`] if `subject` is the note's
/// only remaining one (ADR 0028 §2), or a workspace/store error.
pub async fn remove_subject_from_research_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    subject: NewResearchNoteSubject,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let research_note_id = resolve_research_note_id(store, human_id).await?;
    let subject = resolve_subject(store, subject).await?;
    execute_research_note_mutation(
        store,
        session,
        research_note_id,
        ResearchNoteCommand::RemoveSubject {
            research_note_id,
            subject,
        },
        meta,
    )
    .await
}

/// Applies (or removes) a tag on a research note, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::ResearchNoteNotFound`] if no such research note exists, or a workspace/store error.
pub async fn tag_research_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let research_note_id = resolve_research_note_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    let command = if remove {
        ResearchNoteCommand::Untag {
            research_note_id,
            tag_id,
        }
    } else {
        ResearchNoteCommand::Tag {
            research_note_id,
            tag_id,
        }
    };
    execute_research_note_mutation(store, session, research_note_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Sets a research note's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::ResearchNoteNotFound`] if no such research note exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let research_note_id = resolve_research_note_id(store, human_id).await?;
    execute_research_note_mutation(
        store,
        session,
        research_note_id,
        ResearchNoteCommand::SetRestrictions {
            research_note_id,
            restrictions,
        },
        meta,
    )
    .await
}

/// Loads a single research note's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_research_note(
    workspace: &Workspace,
    human_id: &str,
) -> Result<Option<ResearchNoteSummary>, AppError> {
    let Some(view) = workspace.store().find_research_note(human_id).await? else {
        return Ok(None);
    };
    let tags = tag_refs(workspace.store()).await?;
    Ok(Some(summarize(&view, &tags)))
}

/// Lists every research note's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_research_notes(workspace: &Workspace) -> Result<Vec<ResearchNoteSummary>, AppError> {
    let views = workspace.store().list_research_notes().await?;
    let tags = tag_refs(workspace.store()).await?;
    Ok(views.iter().map(|view| summarize(view, &tags)).collect())
}

/// Lists every research note arguing about `subject` — "which arguments exist about this
/// Person/Family/Event/Place" (ADR 0028 §5: a reverse query over this aggregate's projection, not a
/// field on the subject aggregate), via the `genealogy-db` JSON reverse index over `subjects`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_research_notes_for_subject(
    workspace: &Workspace,
    subject: SubjectRef,
) -> Result<Vec<ResearchNoteSummary>, AppError> {
    let views = workspace.store().list_research_notes_for_subject(subject).await?;
    let tags = tag_refs(workspace.store()).await?;
    Ok(views.iter().map(|view| summarize(view, &tags)).collect())
}

/// Executes one command through the store, stamping it with `provenance` and `citations`
/// (`EventContext.citations` — data-model §8), and maps the outcome to [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: ResearchNoteCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = ResearchNoteCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_research_note(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create research-note mutation, applying the operator-intent [`MutationMeta`]:
/// resolves the backing citations, and — when `meta.supersedes` is set — wraps `command` in a
/// [`ResearchNoteCommand::SupersedeAssertion`] so the new assertion replaces the named one
/// (ADR 0004 §2).
async fn execute_research_note_mutation(
    store: &Store,
    session: &Session,
    research_note_id: ResearchNoteId,
    command: ResearchNoteCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(research_note_id, command, target);
    execute(
        store,
        session,
        &research_note_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in a [`ResearchNoteCommand::SupersedeAssertion`] against `target` when
/// superseding, or returns it unchanged for a plain assertion.
fn superseded(
    research_note_id: ResearchNoteId,
    command: ResearchNoteCommand,
    target: Option<AssertionId>,
) -> ResearchNoteCommand {
    match target {
        Some(target) => ResearchNoteCommand::SupersedeAssertion {
            research_note_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
}

/// Resolves a `human_id` to its aggregate [`ResearchNoteId`], or [`AppError::ResearchNoteNotFound`].
async fn resolve_research_note_id(store: &Store, human_id: &str) -> Result<ResearchNoteId, AppError> {
    use_case::resolve_id(
        store.find_research_note(human_id).await?,
        ResearchNoteView::research_note_id,
        || AppError::ResearchNoteNotFound(human_id.to_owned()),
    )
}

/// Renders a [`ResearchNoteView`] into the frontend DTO, joining its tags via `tags`.
fn summarize(view: &ResearchNoteView, tags: &std::collections::HashMap<TagId, TagRef>) -> ResearchNoteSummary {
    let body = view.body();
    ResearchNoteSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        id: view.research_note_id().map(|id| id.to_string()).unwrap_or_default(),
        subjects: view.subjects().clone(),
        title: view.title().map(str::to_owned),
        body: body.map(|b| b.text.clone()),
        media_type: body.map(|b| b.media_type),
        language: body.and_then(|b| b.language.as_ref().map(|l| l.as_str().to_owned())),
        tags: view
            .tags()
            .into_iter()
            .filter_map(|id| tags.get(&id).cloned())
            .collect(),
        restrictions: view.restrictions().clone(),
    }
}
