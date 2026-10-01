//! Note use-cases (ADR 0006): create, set type, set rich text, tag, show, and list.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use vitni_core::enums::{NoteType, Restriction};
use vitni_core::ids::{AssertionId, HumanId, NoteId, TagId};
use vitni_core::name::LanguageTag;
use vitni_core::note::NoteView;
use vitni_core::note::command::{NoteCommand, NoteCommandEnvelope};
use vitni_core::provenance::EvidenceRef;
use vitni_core::text::{MediaType, RichText};
use vitni_db::Store;

use crate::citation::TagRef;
use crate::dto::{AggRef, UsingRecordRef, tag_refs};
use crate::error::AppError;
use crate::identity::{self, IdentityDecision, NoteClusters, PairDecision};
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
    /// Every note record merged into this note's cluster, directly or through another member
    /// (ADR 0039 §4), in id order.
    pub merged: Vec<AggRef>,
    /// The `human_id` of the member each member row came from, by the row's `AssertionId`: its edit or
    /// retraction is written to that member's stream (ADR 0039 §5). A row absent here is the
    /// note's own. Never rendered as a key; see [`owner_of`](Self::owner_of).
    pub claim_owners: BTreeMap<String, String>,
}

impl NoteSummary {
    /// The `human_id` of the record that owns the row introduced by `assertion_id`: the member it came
    /// from, or this note for its own rows.
    #[must_use]
    pub fn owner_of(&self, assertion_id: &str) -> &str {
        self.claim_owners
            .get(assertion_id)
            .map_or(self.human_id.as_str(), String::as_str)
    }
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
    /// The `AssertionId` (a UUID string) of the note's text assertion the translation lives in — the
    /// target an Edit supersedes (translations are re-emitted as part of the whole `RichText`;
    /// ADR 0004 §2). Never rendered.
    pub assertion_id: String,
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
    let follow_up = provenance.follow_up();
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
            follow_up,
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

/// Sets (or changes) a note's Markdown text and its BCP-47 `language`, identified by `human_id`.
///
/// Preserves any existing translations of the content (the Language tab) — only the primary text and
/// its language are replaced, so editing the body from the whole-record form never drops translations.
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if no such note exists, or a workspace/store error.
pub async fn set_note_text(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    text: String,
    language: Option<String>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, human_id).await?;
    let translations = store
        .find_note(human_id)
        .await?
        .and_then(|view| view.text().cloned())
        .map(|rich| rich.translations)
        .unwrap_or_default();
    let rich = RichText {
        text,
        media_type: MediaType::Markdown,
        language: language.map(LanguageTag::new),
        translator: None,
        translations,
    };
    execute_note_mutation(
        store,
        session,
        note_id,
        NoteCommand::SetRichText { note_id, text: rich },
        meta,
    )
    .await
}

/// Applies (or removes) a tag on a note, identified by `human_id`. A removed tag is untagged on
/// every record of the cluster that holds it (ADR 0039 §5).
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
    if !remove {
        let command = NoteCommand::Tag { note_id, tag_id };
        return execute_note_mutation(store, session, note_id, command, meta).await;
    }
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    for record in identity::cluster_records(store, note_id).await? {
        let Some(id) = record.note_id() else { continue };
        if id != note_id && record.tags().contains(&tag_id) {
            let command = NoteCommand::Untag { note_id: id, tag_id };
            execute(
                store,
                session,
                &id.to_string(),
                command,
                meta.provenance.clone(),
                citations.clone(),
            )
            .await?;
        }
    }
    let command = NoteCommand::Untag { note_id, tag_id };
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

/// The outcome of [`merge_notes`]: the survivor's refreshed summary and the merged note's
/// `human_id`.
///
/// The merge is a same-as link on the survivor (ADR 0039 §1): no record that names the merged note
/// is rewritten. Every reader resolves those references to the cluster's root instead (ADR 0039 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteMergeResult {
    /// The survivor's summary after the merge, composed with every record now in its cluster.
    pub survivor: NoteSummary,
    /// The merged note's `human_id` (its own record/stream is untouched).
    pub merged_human_id: String,
}

/// Merges `merged_human_id`'s cluster into `surviving_human_id`'s, recording a same-as link
/// (ADR 0039 §1). Both records resolve to their cluster roots first (§4), and one `NotesMerged`
/// event is emitted on the surviving root's stream, carrying the decision's provenance and assessment.
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if either `human_id` does not resolve; [`AppError::NoteDomain`] with
/// `MergeConflict` if they resolve to the same note, or `IdentityDecided` if the two are already one
/// cluster or a record of one cluster is distinguished from a record of the other; or a store error.
pub async fn merge_notes(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<NoteMergeResult, AppError> {
    let pair = identity::merge::<NoteView>(workspace, session, surviving_human_id, merged_human_id, decision).await?;
    note_merge_result(workspace, &pair.first_human_id, merged_human_id).await
}

/// Undoes every live distinction between the two note clusters, then merges them (ADR 0039 §4).
///
/// # Errors
///
/// As [`merge_notes`], except that a distinction between the clusters no longer refuses the merge.
pub async fn undo_note_distinction_and_merge(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<NoteMergeResult, AppError> {
    let pair = identity::undo_distinction_and_merge::<NoteView>(
        workspace,
        session,
        surviving_human_id,
        merged_human_id,
        decision,
    )
    .await?;
    note_merge_result(workspace, &pair.first_human_id, merged_human_id).await
}

/// The survivor's composed summary after a merge.
async fn note_merge_result(
    workspace: &Workspace,
    survivor_human_id: &str,
    merged_human_id: &str,
) -> Result<NoteMergeResult, AppError> {
    let survivor = show_note(workspace, survivor_human_id)
        .await?
        .ok_or_else(|| AppError::NoteNotFound(survivor_human_id.to_owned()))?;
    Ok(NoteMergeResult {
        survivor,
        merged_human_id: merged_human_id.to_owned(),
    })
}

/// Records that the two notes are different (ADR 0039 §1), so neither cluster is proposed as a
/// duplicate of the other again. One `NotesDistinguished` event is emitted on the first root's
/// stream; undoing it lifts the decision.
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if either `human_id` does not resolve; [`AppError::NoteDomain`] with
/// `DistinctFromItself` if they resolve to the same note, or `IdentityDecided` if the two are already
/// one cluster or already distinguished; or a store error.
pub async fn distinguish_notes(
    workspace: &Workspace,
    session: &Session,
    note_human_id: &str,
    other_human_id: &str,
    decision: IdentityDecision,
) -> Result<(), AppError> {
    identity::distinguish::<NoteView>(workspace, session, note_human_id, other_human_id, decision).await
}

/// The live identity decision between the clusters of two notes, or `None` when the pair is
/// undecided (ADR 0039 §4).
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if either `human_id` does not resolve, or a store error.
pub async fn note_pair_decision(
    workspace: &Workspace,
    first_human_id: &str,
    other_human_id: &str,
) -> Result<Option<PairDecision>, AppError> {
    identity::pair_decision::<NoteView>(workspace, first_human_id, other_human_id).await
}

/// The `human_id` of the record of `human_id`'s note cluster whose stream holds the live assertion
/// `assertion_id` — where an edit or retraction of that row is written (ADR 0039 §5).
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if `human_id` is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or a store error.
pub async fn note_claim_owner(workspace: &Workspace, human_id: &str, assertion_id: &str) -> Result<String, AppError> {
    identity::claim_owner::<NoteView>(workspace, human_id, assertion_id).await
}

/// Loads a single note's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_note(workspace: &Workspace, human_id: &str) -> Result<Option<NoteSummary>, AppError> {
    let store = workspace.store();
    let Some(note_id) = store.find_note(human_id).await?.and_then(|view| view.note_id()) else {
        return Ok(None);
    };
    let views = identity::cluster_records(store, note_id).await?;
    let lookups = NoteLookups::load(workspace).await?;
    Ok(summarize_cluster(&views.iter().collect::<Vec<_>>(), &lookups))
}

/// Lists every note's summary, ordered by `human_id`. A merged record is listed once, as its cluster's
/// root (ADR 0039 §5).
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_notes(workspace: &Workspace) -> Result<Vec<NoteSummary>, AppError> {
    let store = workspace.store();
    let views = store.list_notes().await?;
    let clusters = NoteClusters::load(store).await?;
    let lookups = NoteLookups::load(workspace).await?;
    let mut summaries = Vec::with_capacity(views.len());
    for cluster in identity::group_clusters(&views, &clusters) {
        summaries.extend(summarize_cluster(&cluster, &lookups));
    }
    Ok(summaries)
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
/// Sets a note's privacy restrictions (GEDCOM `RESN` — data-model §6). A merged member restricted
/// beyond the new set is narrowed to it first, so the cluster reads with exactly `restrictions`
/// (ADR 0039 §5).
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
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    for record in identity::cluster_records(store, note_id).await? {
        let Some(id) = record.note_id() else { continue };
        if id == note_id || record.restrictions().is_subset(&restrictions) {
            continue;
        }
        let narrowed = record.restrictions().intersection(&restrictions).copied().collect();
        let command = NoteCommand::SetRestrictions {
            note_id: id,
            restrictions: narrowed,
        };
        execute(
            store,
            session,
            &id.to_string(),
            command,
            meta.provenance.clone(),
            citations.clone(),
        )
        .await?;
    }
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
pub(crate) async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: NoteCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = NoteCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    let Some(envelope) = crate::origin_gate::gate(store, session, aggregate_id, envelope).await? else {
        return Ok(());
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
    let text_assertion = view.text_assertion().map(|a| a.to_string()).unwrap_or_default();
    let translations = text
        .map(|rich| {
            rich.translations
                .iter()
                .map(|t| TranslationRef {
                    language: t.language.as_ref().map(|l| l.as_str().to_owned()),
                    text: t.text.clone(),
                    media_type: t.media_type,
                    translator: t.translator.clone(),
                    assertion_id: text_assertion.clone(),
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
        merged: Vec::new(),
        claim_owners: BTreeMap::new(),
    }
}

/// Summarises a cluster — its root first, then its members — as one note (ADR 0039 §5): the root's
/// summary with every member's rows appended, each member-owned row recorded in `claim_owners`.
/// `None` for an empty slice.
fn summarize_cluster(views: &[&NoteView], lookups: &NoteLookups) -> Option<NoteSummary> {
    let (root, members) = views.split_first()?;
    let mut summary = summarize(root, lookups);
    for view in members {
        let member = summarize(view, lookups);
        summary.merged.push(AggRef {
            human_id: member.human_id.clone(),
            id: view.note_id().map(|id| id.to_string()).unwrap_or_default(),
        });
        adopt(&mut summary, member);
    }
    summary.merged.sort_by(|x, y| x.id.cmp(&y.id));
    Some(summary)
}

/// Appends a member's rows to its root's summary, recording the member each row came from, and fills
/// what the root lacks from the member.
fn adopt(root: &mut NoteSummary, member: NoteSummary) {
    let owner = member.human_id.clone();
    let mut owned: Vec<String> = Vec::new();
    owned.extend(member.translations.iter().map(|row| row.assertion_id.clone()));
    for assertion_id in owned {
        root.claim_owners.insert(assertion_id, owner.clone());
    }
    root.note_type = root.note_type.take().or(member.note_type);
    if root.text.is_none() {
        root.text = member.text;
        root.media_type = member.media_type;
        root.language = member.language;
    }
    root.translations.extend(member.translations);
    for reference in member.references {
        if !root.references.iter().any(|held| held.id == reference.id) {
            root.references.push(reference);
        }
    }
    for tag in member.tags {
        if !root.tags.iter().any(|held| held.id == tag.id) {
            root.tags.push(tag);
        }
    }
    root.restrictions.extend(member.restrictions);
}
