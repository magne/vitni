//! Media use-cases (ADR 0006): create, set path/checksum, assert date, add attribute/citation,
//! attach note, tag, show, and list.
//!
//! Setting a file path also records the file's checksum (#359): when the path names a file in the
//! workspace media library, its bytes are hashed here — the app layer owns the I/O the pure core may
//! not do — and a `ChecksumSet` follows whenever the digest differs from the recorded one.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

use vitni_core::citation::CitationView;
use vitni_core::date::GenealogicalDate;
use vitni_core::enums::Restriction;
use vitni_core::ids::{AssertionId, CitationId, HumanId, MediaId, NoteId, TagId};
use vitni_core::media::MediaView;
use vitni_core::media::command::{MediaCommand, MediaCommandEnvelope};
use vitni_core::media_path::{MediaPath, media_root_relative};
use vitni_core::provenance::EvidenceRef;
use vitni_core::text::{Attribute, Url};
use vitni_db::Store;

use crate::citation::TagRef;
use crate::dto::{AggRef, AttachedRef, CitationRef, UsingRecordRef, citation_refs, tag_refs};
use crate::error::AppError;
use crate::event::{DateParts, gregorian_date};
use crate::identity::{self, IdentityDecision, MediaClusters, PairDecision};
use crate::media_usage::MediaUsage;
use crate::session::Session;
use crate::use_case::{self, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// A frontend-neutral summary of a media object (the DTO the CLI renders), carrying its stable id and
/// the joined views the detail tabs render (the cross-aggregate-joins dependency note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSummary {
    /// The user-facing identifier (e.g. `O0001`).
    pub human_id: String,
    /// The stable `MediaId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The media's location rendered for display, if set.
    pub path: Option<String>,
    /// The media's filesystem path, if its location is a local file (mutually exclusive with
    /// [`Self::web_path`]) — the raw value the whole-record editor seeds its File-path field from.
    pub file_path: Option<String>,
    /// The media's web reference, if its location is a URL (mutually exclusive with
    /// [`Self::file_path`]) — the raw value the whole-record editor seeds its Web-path field from.
    pub web_path: Option<String>,
    /// The media's MIME type (e.g. `image/jpeg`), if set.
    pub mime: Option<String>,
    /// The media's checksum, if set.
    pub checksum: Option<String>,
    /// The media's date, if asserted. Structured so the frontend localizes it (ADR 0003).
    pub date: Option<GenealogicalDate>,
    /// The recorded attributes (the File card's typed metadata).
    pub attributes: Vec<MediaAttributeRef>,
    /// The citations backing the media's claims (the Citations tab).
    pub citations: Vec<CitationRef>,
    /// The attached notes (the Notes tab), with the attach `AssertionId` (the Detach target).
    pub notes: Vec<AttachedRef>,
    /// The applied tags (the Tags tab), by name/colour/priority.
    pub tags: Vec<TagRef>,
    /// The records that reference this media (the Overview "Used by" card).
    pub used_by: Vec<UsingRecordRef>,
    /// The media's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
    /// Every media object record merged into this media object's cluster, directly or through another member
    /// (ADR 0039 §4), in id order.
    pub merged: Vec<AggRef>,
    /// The `human_id` of the member each member row came from, by the row's `AssertionId`: its edit or
    /// retraction is written to that member's stream (ADR 0039 §5). A row absent here is the
    /// media object's own. Never rendered as a key; see [`owner_of`](Self::owner_of).
    pub claim_owners: BTreeMap<String, String>,
}

impl MediaSummary {
    /// The `human_id` of the record that owns the row introduced by `assertion_id`: the member it came
    /// from, or this media object for its own rows.
    #[must_use]
    pub fn owner_of(&self, assertion_id: &str) -> &str {
        self.claim_owners
            .get(assertion_id)
            .map_or(self.human_id.as_str(), String::as_str)
    }
}

/// A typed attribute on a media object (the File card's metadata rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAttributeRef {
    /// The attribute name/type.
    pub attribute_type: String,
    /// The attribute value.
    pub value: String,
    /// The `AssertionId` (a UUID string) that introduced this attribute — the target a per-row Edit
    /// supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// What to create a media object with (the auto/override `human_id` and an optional file path).
#[derive(Debug, Clone)]
pub struct NewMedia {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// An optional file path for an initial `SetPath`.
    pub path: Option<String>,
}

/// Creates a media object, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::MediaDomain`] if a domain rule
/// rejects the command, or a workspace/store error.
pub async fn create_media(
    workspace: &Workspace,
    session: &Session,
    new: NewMedia,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
    let follow_up = provenance.follow_up();
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_media(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_media_human_id(&workspace.media_id_format()?).await?,
    };
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;

    let media_id = session.new_media_id();
    let aggregate_id = media_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        MediaCommand::CreateMedia {
            media_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        citation_refs,
    )
    .await?;

    if let Some(path) = new.path {
        let checksum = library_file_checksum(workspace, &path);
        execute(
            store,
            session,
            &aggregate_id,
            MediaCommand::SetPath {
                media_id,
                path: MediaPath::File(path),
            },
            follow_up,
            Vec::new(),
        )
        .await?;
        record_checksum(store, session, media_id, checksum, None).await?;
    }

    Ok(human_id)
}

/// Sets (or changes) a media object's file path, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_media_file_path(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    path: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    set_media_path(workspace, session, human_id, MediaPath::File(path), meta).await
}

/// Sets (or changes) a media object's web reference, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_media_web_path(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    href: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let path = MediaPath::Web(Url {
        url_type: None,
        href,
        description: None,
    });
    set_media_path(workspace, session, human_id, path, meta).await
}

/// Sets (or changes) a media object's checksum, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_media_checksum(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    checksum: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute_media_mutation(
        store,
        session,
        media_id,
        MediaCommand::SetChecksum { media_id, checksum },
        meta,
    )
    .await
}

/// Sets (or changes) a media object's MIME type, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_media_mime(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    mime: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute_media_mutation(store, session, media_id, MediaCommand::SetMime { media_id, mime }, meta).await
}

/// Asserts a media object's date, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn assert_media_date(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    parts: DateParts,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute_media_mutation(
        store,
        session,
        media_id,
        MediaCommand::AssertDate {
            media_id,
            date: gregorian_date(parts),
        },
        meta,
    )
    .await
}

/// Asserts a media object's date from an already-built [`GenealogicalDate`] (the full GEDCOM date
/// grammar, via [`build_genealogical_date`](crate::event::build_genealogical_date)).
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn assert_media_date_value(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    date: GenealogicalDate,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute_media_mutation(
        store,
        session,
        media_id,
        MediaCommand::AssertDate { media_id, date },
        meta,
    )
    .await
}

/// Adds a typed attribute to a media object, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn add_media_attribute(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    attribute_type: String,
    value: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute_media_mutation(
        store,
        session,
        media_id,
        MediaCommand::AddAttribute {
            media_id,
            attribute: Attribute { attribute_type, value },
        },
        meta,
    )
    .await
}

/// Adds a citation (by its `human_id`) backing a media object's claims.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] / [`AppError::CitationNotFound`] if either is unknown, or a
/// workspace/store error.
pub async fn add_media_citation(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    citation_human_id: &str,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    let citation_id = resolve_citation_id(store, citation_human_id).await?;
    execute_media_mutation(
        store,
        session,
        media_id,
        MediaCommand::AddCitation { media_id, citation_id },
        meta,
    )
    .await
}

/// Attaches a note (by note aggregate id) to a media object, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn attach_media_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute_media_mutation(
        store,
        session,
        media_id,
        MediaCommand::AttachNote { media_id, note_id },
        meta,
    )
    .await
}

/// Applies (or removes) a tag on a media object, identified by `human_id`. A removed tag is untagged on
/// every record of the cluster that holds it (ADR 0039 §5).
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn tag_media(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    if !remove {
        let command = MediaCommand::Tag { media_id, tag_id };
        return execute_media_mutation(store, session, media_id, command, meta).await;
    }
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    for record in identity::cluster_records(store, media_id).await? {
        let Some(id) = record.media_id() else { continue };
        if id != media_id && record.tags().contains(&tag_id) {
            let command = MediaCommand::Untag { media_id: id, tag_id };
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
    let command = MediaCommand::Untag { media_id, tag_id };
    execute_media_mutation(store, session, media_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Attaches a note (by its `human_id`) to a media object — the importer-facing wrapper.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] / [`AppError::NoteNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_media_note(
    workspace: &Workspace,
    session: &Session,
    media_human_id: &str,
    note_human_id: &str,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = use_case::resolve_id(
        store.find_note(note_human_id).await?,
        vitni_core::note::NoteView::note_id,
        || AppError::NoteNotFound(note_human_id.to_owned()),
    )?;
    attach_media_note(workspace, session, media_human_id, note_id, meta).await
}

/// The outcome of [`merge_media`]: the survivor's refreshed summary and the merged media object's
/// `human_id`.
///
/// The merge is a same-as link on the survivor (ADR 0039 §1): no record that names the merged media object
/// is rewritten. Every reader resolves those references to the cluster's root instead (ADR 0039 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaMergeResult {
    /// The survivor's summary after the merge, composed with every record now in its cluster.
    pub survivor: MediaSummary,
    /// The merged media object's `human_id` (its own record/stream is untouched).
    pub merged_human_id: String,
}

/// Merges `merged_human_id`'s cluster into `surviving_human_id`'s, recording a same-as link
/// (ADR 0039 §1). Both records resolve to their cluster roots first (§4), and one `MediaMerged`
/// event is emitted on the surviving root's stream, carrying the decision's provenance and assessment.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if either `human_id` does not resolve; [`AppError::MediaDomain`] with
/// `MergeConflict` if they resolve to the same media object, or `IdentityDecided` if the two are already one
/// cluster or a record of one cluster is distinguished from a record of the other; or a store error.
pub async fn merge_media(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<MediaMergeResult, AppError> {
    let pair = identity::merge::<MediaView>(workspace, session, surviving_human_id, merged_human_id, decision).await?;
    media_merge_result(workspace, &pair.first_human_id, merged_human_id).await
}

/// Undoes every live distinction between the two media object clusters, then merges them (ADR 0039 §4).
///
/// # Errors
///
/// As [`merge_media`], except that a distinction between the clusters no longer refuses the merge.
pub async fn undo_media_distinction_and_merge(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<MediaMergeResult, AppError> {
    let pair = identity::undo_distinction_and_merge::<MediaView>(
        workspace,
        session,
        surviving_human_id,
        merged_human_id,
        decision,
    )
    .await?;
    media_merge_result(workspace, &pair.first_human_id, merged_human_id).await
}

/// The survivor's composed summary after a merge.
async fn media_merge_result(
    workspace: &Workspace,
    survivor_human_id: &str,
    merged_human_id: &str,
) -> Result<MediaMergeResult, AppError> {
    let survivor = show_media(workspace, survivor_human_id)
        .await?
        .ok_or_else(|| AppError::MediaNotFound(survivor_human_id.to_owned()))?;
    Ok(MediaMergeResult {
        survivor,
        merged_human_id: merged_human_id.to_owned(),
    })
}

/// Records that the two media objects are different (ADR 0039 §1), so neither cluster is proposed as a
/// duplicate of the other again. One `MediaDistinguished` event is emitted on the first root's
/// stream; undoing it lifts the decision.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if either `human_id` does not resolve; [`AppError::MediaDomain`] with
/// `DistinctFromItself` if they resolve to the same media object, or `IdentityDecided` if the two are already
/// one cluster or already distinguished; or a store error.
pub async fn distinguish_media(
    workspace: &Workspace,
    session: &Session,
    media_human_id: &str,
    other_human_id: &str,
    decision: IdentityDecision,
) -> Result<(), AppError> {
    identity::distinguish::<MediaView>(workspace, session, media_human_id, other_human_id, decision).await
}

/// The live identity decision between the clusters of two media objects, or `None` when the pair is
/// undecided (ADR 0039 §4).
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if either `human_id` does not resolve, or a store error.
pub async fn media_pair_decision(
    workspace: &Workspace,
    first_human_id: &str,
    other_human_id: &str,
) -> Result<Option<PairDecision>, AppError> {
    identity::pair_decision::<MediaView>(workspace, first_human_id, other_human_id).await
}

/// The `human_id` of the record of `human_id`'s media object cluster whose stream holds the live assertion
/// `assertion_id` — where an edit or retraction of that row is written (ADR 0039 §5).
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if `human_id` is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or a store error.
pub async fn media_claim_owner(workspace: &Workspace, human_id: &str, assertion_id: &str) -> Result<String, AppError> {
    identity::claim_owner::<MediaView>(workspace, human_id, assertion_id).await
}

/// Loads a single media object's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_media(workspace: &Workspace, human_id: &str) -> Result<Option<MediaSummary>, AppError> {
    let store = workspace.store();
    let Some(media_id) = store.find_media(human_id).await?.and_then(|view| view.media_id()) else {
        return Ok(None);
    };
    let views = identity::cluster_records(store, media_id).await?;
    let lookups = MediaLookups::load(workspace).await?;
    Ok(summarize_cluster(&views.iter().collect::<Vec<_>>(), &lookups))
}

/// Lists every media object's summary, ordered by `human_id`. A merged record is listed once, as its cluster's
/// root (ADR 0039 §5).
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_media(workspace: &Workspace) -> Result<Vec<MediaSummary>, AppError> {
    let store = workspace.store();
    let views = store.list_media().await?;
    let clusters = MediaClusters::load(store).await?;
    let lookups = MediaLookups::load(workspace).await?;
    let mut summaries = Vec::with_capacity(views.len());
    for cluster in identity::group_clusters(&views, &clusters) {
        summaries.extend(summarize_cluster(&cluster, &lookups));
    }
    Ok(summaries)
}

/// The lookups `summarize` needs to join a media object's attachments and back-references to the
/// other projections without a per-row query (the cross-aggregate join lives here — the app/db layer).
struct MediaLookups {
    citations: HashMap<CitationId, CitationRef>,
    notes: HashMap<NoteId, use_case::NoteLookup>,
    tags: HashMap<TagId, TagRef>,
    usage: MediaUsage,
}

impl MediaLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let store = workspace.store();
        Ok(Self {
            citations: citation_refs(store).await?,
            notes: use_case::note_lookups(store).await?,
            tags: tag_refs(store).await?,
            usage: MediaUsage::load(workspace).await?,
        })
    }
}

/// Sets the media path through the store.
async fn set_media_path(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    path: MediaPath,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media = store.find_media(human_id).await?;
    let recorded = media.as_ref().and_then(|view| view.checksum().map(str::to_owned));
    let media_id = use_case::resolve_id(media, MediaView::media_id, || {
        AppError::MediaNotFound(human_id.to_owned())
    })?;
    let checksum = match &path {
        MediaPath::File(file) => library_file_checksum(workspace, file),
        MediaPath::Web(_) => None,
    };
    execute_media_mutation(store, session, media_id, MediaCommand::SetPath { media_id, path }, meta).await?;
    record_checksum(store, session, media_id, checksum, recorded.as_deref()).await
}

/// Records `checksum` on the media object when it is known and differs from the `recorded` one. A
/// mechanical setter, so it carries no provenance (data-model §8).
async fn record_checksum(
    store: &Store,
    session: &Session,
    media_id: MediaId,
    checksum: Option<String>,
    recorded: Option<&str>,
) -> Result<(), AppError> {
    let Some(checksum) = checksum else {
        return Ok(());
    };
    if recorded == Some(checksum.as_str()) {
        return Ok(());
    }
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::SetChecksum { media_id, checksum },
        Provenance::default(),
        Vec::new(),
    )
    .await
}

/// The `"sha256:<hex>"` digest of the media-library file `stored` names, or `None` when it names no
/// library file (an absolute path, a `..` escape) or the file is not on this machine — both legitimate
/// for a media record, so neither fails the write. Any other read failure is logged and skipped.
fn library_file_checksum(workspace: &Workspace, stored: &str) -> Option<String> {
    let path = workspace.media_root().join(media_root_relative(stored)?);
    match file_checksum(&path) {
        Ok(checksum) => Some(checksum),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "not recording a checksum: the media file is unreadable");
            None
        }
    }
}

/// Streams the file at `path` through SHA-256, in the `"sha256:<lowercase hex>"` form the plugin host's
/// `media-store` reports.
pub(crate) fn file_checksum(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let mut checksum = String::from("sha256:");
    for byte in hasher.finalize() {
        let _ = write!(checksum, "{byte:02x}");
    }
    Ok(checksum)
}

/// Sets a media object's privacy restrictions (GEDCOM `RESN` — data-model §6). A merged member restricted
/// beyond the new set is narrowed to it first, so the cluster reads with exactly `restrictions`
/// (ADR 0039 §5).
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    for record in identity::cluster_records(store, media_id).await? {
        let Some(id) = record.media_id() else { continue };
        if id == media_id || record.restrictions().is_subset(&restrictions) {
            continue;
        }
        let narrowed = record.restrictions().intersection(&restrictions).copied().collect();
        let command = MediaCommand::SetRestrictions {
            media_id: id,
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
    execute_media_mutation(
        store,
        session,
        media_id,
        MediaCommand::SetRestrictions { media_id, restrictions },
        meta,
    )
    .await
}

/// Sets (or changes) a media object's user-facing identifier, identified by its current `human_id`,
/// returning the effective new id.
///
/// A supplied non-blank `new` id is dup-checked (a collision with a *different* record is
/// [`AppError::HumanIdTaken`]); a blank/absent `new` allocates the next free id from the workspace's
/// configured format (the regenerate case).
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if the media object is unknown, [`AppError::HumanIdTaken`] if the
/// requested id is already in use, or a workspace/store error.
pub async fn set_media_human_id(
    workspace: &Workspace,
    session: &Session,
    current_human_id: &str,
    new: Option<String>,
    provenance: Provenance,
) -> Result<String, AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, current_human_id).await?;
    let human_id = match use_case::requested_human_id(new) {
        Some(id) => {
            if id != current_human_id && store.find_media(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_media_human_id(&workspace.media_id_format()?).await?,
    };
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::SetHumanId {
            media_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        Vec::new(),
    )
    .await?;
    Ok(human_id)
}

/// Executes one command through the store, stamping it with `provenance` (the operator's surety and
/// rationale) and `citations` (`EventContext.citations` — data-model §8), and maps the outcome to
/// [`AppError`].
pub(crate) async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: MediaCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = MediaCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    let Some(envelope) = crate::origin_gate::gate(store, session, aggregate_id, envelope).await? else {
        return Ok(());
    };
    store
        .execute_media(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create media mutation, applying the operator-intent [`MutationMeta`]: resolves
/// the backing citations, and — when `meta.supersedes` is set — wraps `command` in a
/// [`MediaCommand::SupersedeAssertion`] so the new assertion replaces the named one (ADR 0004 §2).
async fn execute_media_mutation(
    store: &Store,
    session: &Session,
    media_id: MediaId,
    command: MediaCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(media_id, command, target);
    execute(
        store,
        session,
        &media_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in a [`MediaCommand::SupersedeAssertion`] against `target` when superseding, or
/// returns it unchanged for a plain assertion.
fn superseded(media_id: MediaId, command: MediaCommand, target: Option<AssertionId>) -> MediaCommand {
    match target {
        Some(target) => MediaCommand::SupersedeAssertion {
            media_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
}

/// Resolves a `human_id` to its aggregate [`MediaId`], or [`AppError::MediaNotFound`].
async fn resolve_media_id(store: &Store, human_id: &str) -> Result<MediaId, AppError> {
    use_case::resolve_id(store.find_media(human_id).await?, MediaView::media_id, || {
        AppError::MediaNotFound(human_id.to_owned())
    })
}

/// Resolves a citation `human_id` to its aggregate [`CitationId`], or [`AppError::CitationNotFound`].
async fn resolve_citation_id(store: &Store, human_id: &str) -> Result<CitationId, AppError> {
    use_case::resolve_id(store.find_citation(human_id).await?, CitationView::citation_id, || {
        AppError::CitationNotFound(human_id.to_owned())
    })
}

/// Renders the media location for display.
fn render_path(path: &MediaPath) -> String {
    match path {
        MediaPath::File(file) => file.clone(),
        MediaPath::Web(url) => url.href.clone(),
    }
}

/// The media location split into its file-path and web-path fields (only one is ever `Some`, since a
/// media object holds a single, last-writer-wins location) — the raw seeds for the edit form.
fn split_path(path: Option<&MediaPath>) -> (Option<String>, Option<String>) {
    match path {
        Some(MediaPath::File(file)) => (Some(file.clone()), None),
        Some(MediaPath::Web(url)) => (None, Some(url.href.clone())),
        None => (None, None),
    }
}

/// Renders a [`MediaView`] into the frontend DTO, joining its attachments and back-references to the
/// other projections via `lookups`.
fn summarize(view: &MediaView, lookups: &MediaLookups) -> MediaSummary {
    let attributes = view
        .attributes_with_assertions()
        .iter()
        .map(|attributed| MediaAttributeRef {
            attribute_type: attributed.value.attribute_type.clone(),
            value: attributed.value.value.clone(),
            assertion_id: attributed.assertion_id.to_string(),
        })
        .collect();
    let citations = view
        .citations_with_assertions()
        .iter()
        .filter_map(|attributed| {
            lookups.citations.get(&attributed.value).cloned().map(|mut citation| {
                citation.assertion_id = Some(attributed.assertion_id.to_string());
                citation
            })
        })
        .collect();
    let notes = view
        .notes_with_assertions()
        .iter()
        .filter_map(|attributed| {
            lookups.notes.get(&attributed.value).map(|note| AttachedRef {
                human_id: note.human_id.clone(),
                id: note.id.clone(),
                note_type: note.note_type.clone(),
                text: note.text.clone(),
                language: note.language.clone(),
                assertion_id: attributed.assertion_id.to_string(),
            })
        })
        .collect();
    let tags = view
        .tags()
        .into_iter()
        .filter_map(|id| lookups.tags.get(&id).cloned())
        .collect();
    let used_by = view.media_id().map(|id| lookups.usage.used_by(id)).unwrap_or_default();
    let (file_path, web_path) = split_path(view.path());
    MediaSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        id: view.media_id().map(|id| id.to_string()).unwrap_or_default(),
        path: view.path().map(render_path),
        file_path,
        web_path,
        mime: view.mime().map(ToOwned::to_owned),
        checksum: view.checksum().map(ToOwned::to_owned),
        date: view.date().cloned(),
        attributes,
        citations,
        notes,
        tags,
        used_by,
        restrictions: view.restrictions().clone(),
        merged: Vec::new(),
        claim_owners: BTreeMap::new(),
    }
}

/// Summarises a cluster — its root first, then its members — as one media object (ADR 0039 §5): the root's
/// summary with every member's rows appended, each member-owned row recorded in `claim_owners`.
/// `None` for an empty slice.
fn summarize_cluster(views: &[&MediaView], lookups: &MediaLookups) -> Option<MediaSummary> {
    let (root, members) = views.split_first()?;
    let mut summary = summarize(root, lookups);
    for view in members {
        let member = summarize(view, lookups);
        summary.merged.push(AggRef {
            human_id: member.human_id.clone(),
            id: view.media_id().map(|id| id.to_string()).unwrap_or_default(),
        });
        adopt(&mut summary, member);
    }
    summary.merged.sort_by(|x, y| x.id.cmp(&y.id));
    Some(summary)
}

/// Appends a member's rows to its root's summary, recording the member each row came from, and fills
/// what the root lacks from the member.
fn adopt(root: &mut MediaSummary, member: MediaSummary) {
    let owner = member.human_id.clone();
    let mut owned: Vec<String> = Vec::new();
    owned.extend(member.attributes.iter().map(|row| row.assertion_id.clone()));
    owned.extend(member.notes.iter().map(|row| row.assertion_id.clone()));
    owned.extend(member.citations.iter().filter_map(|row| row.assertion_id.clone()));
    for assertion_id in owned {
        root.claim_owners.insert(assertion_id, owner.clone());
    }
    if root.path.is_none() {
        root.path = member.path;
        root.file_path = member.file_path;
        root.web_path = member.web_path;
    }
    root.mime = root.mime.take().or(member.mime);
    root.checksum = root.checksum.take().or(member.checksum);
    root.date = root.date.take().or(member.date);
    root.attributes.extend(member.attributes);
    root.citations.extend(member.citations);
    root.notes.extend(member.notes);
    for user in member.used_by {
        if !root.used_by.iter().any(|held| held.id == user.id) {
            root.used_by.push(user);
        }
    }
    for tag in member.tags {
        if !root.tags.iter().any(|held| held.id == tag.id) {
            root.tags.push(tag);
        }
    }
    root.restrictions.extend(member.restrictions);
}
