//! Note use-cases (ADR 0006): create, set type, set rich text, tag, show, and list.

use std::collections::{BTreeSet, HashMap};

use genealogy_core::enums::{NoteType, Restriction};
use genealogy_core::ids::{AssertionId, HumanId, NoteId, TagId};
use genealogy_core::name::LanguageTag;
use genealogy_core::note::NoteView;
use genealogy_core::note::command::{NoteCommand, NoteCommandEnvelope};
use genealogy_core::provenance::CitationRef;
use genealogy_core::text::{MediaType, RichText};
use genealogy_db::Store;

use crate::citation::TagRef;
use crate::dto::{UsingRecordRef, tag_refs};
use crate::error::AppError;
use crate::note_usage::NoteUsage;
use crate::session::Session;
use crate::use_case::{self, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// A frontend-neutral summary of a note (the DTO the CLI renders), carrying its stable id and the
/// joined views the detail tabs render (the cross-aggregate-joins dependency note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSummary {
    /// The user-facing identifier (e.g. `N0001`).
    pub human_id: String,
    /// The stable `NoteId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The note's type. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub note_type: Option<NoteType>,
    /// The note's primary text content, if set.
    pub text: Option<String>,
    /// How the primary text is interpreted (Markdown/Plain/HTML).
    pub media_type: Option<MediaType>,
    /// The primary content's language (a BCP-47 tag), if recorded.
    pub language: Option<String>,
    /// Translations of the primary content into other languages (the Language tab).
    pub translations: Vec<TranslationRef>,
    /// The records that reference this note (the References tab).
    pub references: Vec<UsingRecordRef>,
    /// The applied tags (the Tags tab), by name/colour/priority.
    pub tags: Vec<TagRef>,
    /// The note's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// A translation of a note's primary content into another language (a Language-tab row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRef {
    /// The translation's language (a BCP-47 tag), if recorded.
    pub language: Option<String>,
    /// The translated text.
    pub text: String,
    /// How the translated text is interpreted (Markdown/Plain/HTML).
    pub media_type: MediaType,
    /// Who produced the translation, if recorded.
    pub translator: Option<String>,
}

/// What to create a note with (the auto/override `human_id` and optional initial text).
#[derive(Debug, Clone)]
pub struct NewNote {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// Optional initial Markdown text for an initial `SetRichText`.
    pub text: Option<String>,
}

/// Creates a note, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::NoteDomain`] if a domain rule
/// rejects the command, or a workspace/store error.
pub async fn create_note(
    workspace: &Workspace,
    session: &Session,
    new: NewNote,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_note(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_note_human_id(&workspace.note_id_format()?).await?,
    };
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;

    let note_id = session.new_note_id();
    let aggregate_id = note_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        NoteCommand::CreateNote {
            note_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        citation_refs,
    )
    .await?;

    if let Some(text) = new.text {
        execute(
            store,
            session,
            &aggregate_id,
            NoteCommand::SetRichText {
                note_id,
                text: markdown(text),
            },
            Provenance::default(),
            Vec::new(),
        )
        .await?;
    }

    Ok(human_id)
}

/// Sets (or changes) a note's type, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if no such note exists, or a workspace/store error.
pub async fn set_note_type(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_type: NoteType,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, human_id).await?;
    execute_note_mutation(
        store,
        session,
        note_id,
        NoteCommand::SetNoteType { note_id, note_type },
        meta,
    )
    .await
}

/// Sets (or changes) a note's Markdown text, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if no such note exists, or a workspace/store error.
pub async fn set_note_text(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    text: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, human_id).await?;
    execute_note_mutation(
        store,
        session,
        note_id,
        NoteCommand::SetRichText {
            note_id,
            text: markdown(text),
        },
        meta,
    )
    .await
}

/// Applies (or removes) a tag on a note, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if no such note exists, or a workspace/store error.
pub async fn tag_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    let command = if remove {
        NoteCommand::Untag { note_id, tag_id }
    } else {
        NoteCommand::Tag { note_id, tag_id }
    };
    execute_note_mutation(store, session, note_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Adds (or replaces) a translation of a note's primary content, identified by `human_id`.
///
/// The whole [`RichText`] is re-emitted (a `RichTextSet`): the current primary content is preserved
/// and the translation for `language` is appended or replaced. The read of current state happens here
/// in the app layer (the decision core stays pure — ADR 0004 §3).
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if no such note exists, or a workspace/store error.
pub async fn add_note_translation(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    language: String,
    text: String,
    translator: Option<String>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, human_id).await?;
    let current = store.find_note(human_id).await?.and_then(|view| view.text().cloned());
    let mut rich = current.unwrap_or_else(|| markdown(String::new()));
    let translation = RichText {
        text,
        media_type: MediaType::Markdown,
        language: Some(LanguageTag::new(&language)),
        translator,
        translations: Vec::new(),
    };
    rich.translations
        .retain(|t| t.language.as_ref().map(LanguageTag::as_str) != Some(language.as_str()));
    rich.translations.push(translation);
    execute_note_mutation(
        store,
        session,
        note_id,
        NoteCommand::SetRichText { note_id, text: rich },
        meta,
    )
    .await
}

/// Loads a single note's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_note(workspace: &Workspace, human_id: &str) -> Result<Option<NoteSummary>, AppError> {
    let Some(view) = workspace.store().find_note(human_id).await? else {
        return Ok(None);
    };
    let lookups = NoteLookups::load(workspace).await?;
    Ok(Some(summarize(&view, &lookups)))
}

/// Lists every note's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_notes(workspace: &Workspace) -> Result<Vec<NoteSummary>, AppError> {
    let views = workspace.store().list_notes().await?;
    let lookups = NoteLookups::load(workspace).await?;
    Ok(views.iter().map(|view| summarize(view, &lookups)).collect())
}

/// The lookups `summarize` needs to join a note's tags and back-references to the other projections
/// without a per-row query (the cross-aggregate join lives here — the app/db layer).
struct NoteLookups {
    tags: HashMap<TagId, TagRef>,
    usage: NoteUsage,
}

impl NoteLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        Ok(Self {
            tags: tag_refs(workspace.store()).await?,
            usage: NoteUsage::load(workspace).await?,
        })
    }
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
/// Sets a note's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if no such note exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, human_id).await?;
    execute_note_mutation(
        store,
        session,
        note_id,
        NoteCommand::SetRestrictions { note_id, restrictions },
        meta,
    )
    .await
}

/// Sets (or changes) a note's user-facing identifier, identified by its current `human_id`,
/// returning the effective new id.
///
/// A supplied non-blank `new` id is dup-checked (a collision with a *different* record is
/// [`AppError::HumanIdTaken`]); a blank/absent `new` allocates the next free id from the workspace's
/// configured format (the regenerate case).
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if the note is unknown, [`AppError::HumanIdTaken`] if the requested id
/// is already in use, or a workspace/store error.
pub async fn set_note_human_id(
    workspace: &Workspace,
    session: &Session,
    current_human_id: &str,
    new: Option<String>,
    provenance: Provenance,
) -> Result<String, AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, current_human_id).await?;
    let human_id = match use_case::requested_human_id(new) {
        Some(id) => {
            if id != current_human_id && store.find_note(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_note_human_id(&workspace.note_id_format()?).await?,
    };
    execute(
        store,
        session,
        &note_id.to_string(),
        NoteCommand::SetHumanId {
            note_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        Vec::new(),
    )
    .await?;
    Ok(human_id)
}

/// Executes one command through the store, stamping it with `provenance` and `citations`
/// (`EventContext.citations` — data-model §8), and maps the outcome to [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: NoteCommand,
    provenance: Provenance,
    citations: Vec<CitationRef>,
) -> Result<(), AppError> {
    let envelope = NoteCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_note(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create note mutation, applying the operator-intent [`MutationMeta`]: resolves
/// the backing citations, and — when `meta.supersedes` is set — wraps `command` in a
/// [`NoteCommand::SupersedeAssertion`] so the new assertion replaces the named one (ADR 0004 §2).
async fn execute_note_mutation(
    store: &Store,
    session: &Session,
    note_id: NoteId,
    command: NoteCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(note_id, command, target);
    execute(
        store,
        session,
        &note_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in a [`NoteCommand::SupersedeAssertion`] against `target` when superseding, or
/// returns it unchanged for a plain assertion.
fn superseded(note_id: NoteId, command: NoteCommand, target: Option<AssertionId>) -> NoteCommand {
    match target {
        Some(target) => NoteCommand::SupersedeAssertion {
            note_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
}

/// Resolves a `human_id` to its aggregate [`NoteId`], or [`AppError::NoteNotFound`].
async fn resolve_note_id(store: &Store, human_id: &str) -> Result<NoteId, AppError> {
    use_case::resolve_id(store.find_note(human_id).await?, NoteView::note_id, || {
        AppError::NoteNotFound(human_id.to_owned())
    })
}

/// Builds a Markdown [`RichText`] from plain text (language is not collected by the CLI yet).
fn markdown(text: String) -> RichText {
    RichText {
        text,
        media_type: MediaType::Markdown,
        language: None,
        translator: None,
        translations: Vec::new(),
    }
}

/// Renders a [`NoteView`] into the frontend DTO, joining its tags and back-references to the other
/// projections via `lookups`.
fn summarize(view: &NoteView, lookups: &NoteLookups) -> NoteSummary {
    let text = view.text();
    let translations = text
        .map(|rich| {
            rich.translations
                .iter()
                .map(|t| TranslationRef {
                    language: t.language.as_ref().map(|l| l.as_str().to_owned()),
                    text: t.text.clone(),
                    media_type: t.media_type,
                    translator: t.translator.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let tags = view
        .tags()
        .into_iter()
        .filter_map(|id| lookups.tags.get(&id).cloned())
        .collect();
    let references = view.note_id().map(|id| lookups.usage.used_by(id)).unwrap_or_default();
    NoteSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        id: view.note_id().map(|id| id.to_string()).unwrap_or_default(),
        note_type: view.note_type().cloned(),
        text: text.map(|t| t.text.clone()),
        media_type: text.map(|t| t.media_type),
        language: text.and_then(|t| t.language.as_ref().map(|l| l.as_str().to_owned())),
        translations,
        references,
        tags,
        restrictions: view.restrictions().clone(),
    }
}
