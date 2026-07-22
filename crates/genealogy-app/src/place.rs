//! Place use-cases (ADR 0006): create, set type, add name, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`PlaceSummary`] (never a `PlaceView`, cqrs-es, or sqlx type). `human_id` is
//! auto-allocated using the workspace's configured format, or validated when supplied (ADR 0005).

use std::collections::{BTreeSet, HashMap};

use genealogy_core::citation::CitationView;
use genealogy_core::date::GenealogicalDate;
use genealogy_core::enums::{PlaceType, Restriction, SuccessionKind};
use genealogy_core::geo::{GeoCoordinates, PlaceGeometry};
use genealogy_core::ids::{AssertionId, CitationId, HumanId, MediaId, NoteId, PlaceId, TagId};
use genealogy_core::place::PlaceView;
use genealogy_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use genealogy_core::place::error::PlaceError;
use genealogy_core::place_name::PlaceName;
use genealogy_core::place_ref::PlaceRef;
use genealogy_core::provenance::Confidence;
use genealogy_core::provenance::EvidenceRef;
use genealogy_core::text::MediaRef;
use genealogy_db::{PlaceSuccessionRecord, Store};

use crate::citation::TagRef;
use crate::dto::{AttachedRef, CitationRef, MediaLookup, MediaRefSummary, citation_refs, media_lookups, tag_refs};
use crate::error::AppError;
use crate::place_hierarchy::{HierarchyHop, generated_title, hierarchy_chain};
use crate::session::Session;
use crate::use_case::{self, MediaRefInput, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// An asserted place name, with its language/date metadata and the assertion's provenance (the
/// evidence-first cue for the Names tab — data-model §14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceNameRef {
    /// The name text.
    pub text: String,
    /// The BCP-47 language tag the name is in, if recorded.
    pub language: Option<String>,
    /// The date the name was in use (structured so the frontend localizes it — ADR 0003).
    pub date: Option<GenealogicalDate>,
    /// The operator's surety in the name assertion.
    pub confidence: Option<Confidence>,
    /// How many citations back the name assertion.
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) that introduced this name — the target a per-row Edit
    /// supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// An enclosing place, joined to the place projection: its name/type for display, the stable id for
/// navigation, the dated link, and the assertion's surety (the jurisdiction chain — data-model §14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceEnclosingRef {
    /// The enclosing place's user-facing identifier (e.g. `P0001`).
    pub human_id: String,
    /// The enclosing place's stable `PlaceId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The enclosing place's primary name, if resolved.
    pub name: Option<String>,
    /// The enclosing place's type, if resolved.
    pub place_type: Option<PlaceType>,
    /// The date the enclosing relationship was valid (structured so the frontend localizes it).
    pub date: Option<GenealogicalDate>,
    /// The operator's surety in the enclosing-by assertion.
    pub confidence: Option<Confidence>,
    /// The `AssertionId` (a UUID string) that introduced this enclosing-by link — the target a
    /// per-row Edit supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// A succession relation from a place's perspective — a predecessor or a successor, depending on
/// which list it is read from (ADR 0026 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceSuccessionRef {
    /// The counterpart place's user-facing identifier (e.g. `P0001`).
    pub human_id: String,
    /// The counterpart place's stable `PlaceId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The counterpart place's primary name, if resolved.
    pub name: Option<String>,
    /// The kind of identity change.
    pub kind: SuccessionKind,
    /// The date the succession took effect (structured so the frontend localizes it), if known.
    pub date: Option<GenealogicalDate>,
    /// The `AssertionId` (a UUID string) a correction targets. Never rendered.
    pub assertion_id: String,
}

/// A geometry a place had, with its provenance — the read side of a dated shape assertion (ADR
/// 0024). Unlike `coordinates` (last-writer-wins), a place can carry many of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceGeometryRef {
    /// The asserted shape.
    pub geometry: PlaceGeometry,
    /// The date this geometry held, if known (structured so the frontend localizes it — ADR 0003).
    pub date: Option<GenealogicalDate>,
    /// The operator's surety in the geometry assertion.
    pub confidence: Option<Confidence>,
    /// The geometry assertion's citations, joined to the source projection.
    pub citations: Vec<CitationRef>,
    /// The `AssertionId` (a UUID string) that introduced this geometry — the target a per-row Edit
    /// supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// A frontend-neutral summary of a place (the DTO the CLI renders). References to other aggregates
/// carry their stable ids alongside their `human_id`s (the cross-aggregate-joins dependency note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceSummary {
    /// The user-facing identifier (e.g. `P0001`).
    pub human_id: String,
    /// The place's stable `PlaceId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The generated title (e.g. "Saint Petersburg, Russia"): the place's own resolved name
    /// followed by each ancestor's resolved name up the transitive hierarchy walk (`docs/issues.md`;
    /// ADR 0026 §1).
    pub generated_title: String,
    /// The date this summary is resolved **as of** (ADR 0026 §1) — `None` for the current/primary
    /// resolution `show_place`/`list_places` use; `Some` only from `show_place_as_of`.
    pub resolved_as_of: Option<GenealogicalDate>,
    /// The place's type. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub place_type: Option<PlaceType>,
    /// The operator's surety in the place type, if set.
    pub place_type_confidence: Option<Confidence>,
    /// The asserted names, in assertion order, each with its language/date + provenance.
    pub names: Vec<PlaceNameRef>,
    /// The place's code, if set.
    pub code: Option<String>,
    /// The operator's surety in the code, if set.
    pub code_confidence: Option<Confidence>,
    /// The code assertion's citations, joined to the source projection — the evidence behind the code,
    /// for the provenance popover.
    pub code_citations: Vec<CitationRef>,
    /// The place's coordinates rendered as `lat,long` degrees, if asserted.
    pub coordinates: Option<String>,
    /// The operator's surety in the coordinates, if asserted.
    pub coordinates_confidence: Option<Confidence>,
    /// The coordinate assertion's citations, joined to the source projection — the evidence behind
    /// the coordinates, for the provenance popover.
    pub coordinate_citations: Vec<CitationRef>,
    /// The place's dated geometry assertions (ADR 0024), in assertion order — these accumulate
    /// rather than replace, unlike `coordinates` above.
    pub geometries: Vec<PlaceGeometryRef>,
    /// The single geometry in effect **as of** [`Self::resolved_as_of`] (ADR 0026 §1) — the
    /// latest-dated assertion at or before that date, or the first-asserted (undated/primary) one
    /// when none qualifies (or when resolving without a target date). `None` when the place has no
    /// geometry assertions at all. The geography view's map marker (ADR 0025 §1) reads this, never
    /// [`Self::geometries`] directly.
    pub resolved_geometry: Option<PlaceGeometryRef>,
    /// The full transitive jurisdiction chain (nearest first), joined to the place projection — the
    /// `docs/issues.md` "Transitive place-hierarchy walk", date-aware (ADR 0026 §1).
    pub enclosing: Vec<PlaceEnclosingRef>,
    /// Places this place succeeded (what it came from), joined to the place projection. Populated
    /// only by `show_place`/`show_place_as_of` — empty from `list_places` (ADR 0026 §4).
    pub predecessors: Vec<PlaceSuccessionRef>,
    /// Places this place was succeeded by (what it became), joined to the place projection.
    /// Populated only by `show_place`/`show_place_as_of` — empty from `list_places` (ADR 0026 §4).
    pub successors: Vec<PlaceSuccessionRef>,
    /// Citations backing the place's claims, joined to the citation/source projection.
    pub citations: Vec<CitationRef>,
    /// Media attached to the place, in assertion order.
    pub media: Vec<MediaRefSummary>,
    /// Notes attached to the place, with the attach `AssertionId` (the Detach target), in assertion
    /// order.
    pub notes: Vec<AttachedRef>,
    /// Tags applied to the place, by name + colour (never by id — data-model §9).
    pub tags: Vec<TagRef>,
    /// The place's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// What to create a place with (the auto/override `human_id`, its type, and an optional first name).
#[derive(Debug, Clone)]
pub struct NewPlace {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// The place's type.
    pub place_type: PlaceType,
    /// An optional name text for an initial `AssertName`.
    pub name: Option<String>,
}

/// Creates a place, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::PlaceDomain`] if a domain rule
/// rejects the command, or a workspace/store error.
pub async fn create_place(
    workspace: &Workspace,
    session: &Session,
    new: NewPlace,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_place(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_place_human_id(&workspace.place_id_format()?).await?,
    };
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;

    let place_id = session.new_place_id();
    let aggregate_id = place_id.to_string();

    execute(
        store,
        session,
        &aggregate_id,
        PlaceCommand::CreatePlace {
            place_id,
            human_id: HumanId::new(&human_id),
            place_type: new.place_type,
        },
        provenance,
        citation_refs,
    )
    .await?;

    if let Some(text) = new.name {
        execute(
            store,
            session,
            &aggregate_id,
            PlaceCommand::AssertName {
                place_id,
                name: place_name(text),
            },
            Provenance::default(),
            Vec::new(),
        )
        .await?;
    }

    Ok(human_id)
}

/// Creates a place with an already-allocated `human_id`, returning its minted [`PlaceId`].
///
/// The event change-set ([`crate::event_change_set`]) reuses this to create a pending place inside the
/// same operator action and keep the id for the event's `LinkPlace`, mirroring
/// [`crate::source::create_source_returning_id`]. The `human_id` is allocated by the caller before any
/// write.
///
/// # Errors
///
/// [`AppError::PlaceDomain`] on a domain rejection, or a workspace/store error.
pub(crate) async fn create_place_returning_id(
    session: &Session,
    store: &Store,
    human_id: &str,
    place_type: PlaceType,
    name: Option<String>,
    provenance: Provenance,
) -> Result<PlaceId, AppError> {
    let place_id = session.new_place_id();
    let aggregate_id = place_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        PlaceCommand::CreatePlace {
            place_id,
            human_id: HumanId::new(human_id),
            place_type,
        },
        provenance.clone(),
        Vec::new(),
    )
    .await?;
    if let Some(text) = name {
        execute(
            store,
            session,
            &aggregate_id,
            PlaceCommand::AssertName {
                place_id,
                name: place_name(text),
            },
            provenance,
            Vec::new(),
        )
        .await?;
    }
    Ok(place_id)
}

/// Resolves a place `human_id` to its aggregate [`PlaceId`] — the crate-internal accessor the event
/// change-set reuses to validate/link an existing place before any write.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub(crate) async fn resolve_place_id_public(store: &Store, human_id: &str) -> Result<PlaceId, AppError> {
    resolve_place_id(store, human_id).await
}

/// Sets (or changes) an existing place's type, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn set_place_type(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    place_type: PlaceType,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute_place_mutation(
        store,
        session,
        place_id,
        PlaceCommand::SetPlaceType { place_id, place_type },
        meta,
    )
    .await
}

/// Asserts an additional name on an existing place, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, [`AppError::PlaceDomain`] if the name is
/// empty, or a workspace/store error.
pub async fn add_place_name(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    name: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute_place_mutation(
        store,
        session,
        place_id,
        PlaceCommand::AssertName {
            place_id,
            name: place_name(name),
        },
        meta,
    )
    .await
}

/// Asserts that a place is enclosed by another place, identified by their `human_id`s.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if either place is unknown, [`AppError::PlaceDomain`] if the
/// enclosing place is not yet projected (`UnknownPlace`), or a workspace/store error.
pub async fn assert_place_enclosed_by(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    enclosing_human_id: &str,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    let enclosing_id = resolve_place_id(store, enclosing_human_id).await?;
    execute_place_mutation(
        store,
        session,
        place_id,
        PlaceCommand::AssertEnclosedBy {
            place_id,
            enclosed_by: PlaceRef {
                place_id: enclosing_id,
                date: None,
            },
        },
        meta,
    )
    .await
}

/// The operator intent for [`assert_place_succession`]: which places ceased and resulted, and how
/// (ADR 0026 §3). Bundled so the use-case's signature stays within the argument-count lint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceSuccessionInput {
    /// The `human_id`s of the place(s) that ceased (`human_id` must be one of these).
    pub from_human_ids: Vec<String>,
    /// The `human_id`s of the place(s) that resulted.
    pub to_human_ids: Vec<String>,
    /// The kind of identity change.
    pub kind: SuccessionKind,
    /// The date this succession took effect, if known.
    pub date: Option<GenealogicalDate>,
}

/// Asserts an identity-changing succession between places, identified by `human_id`s (ADR 0026 §3):
/// `human_id` is the anchor this assertion is recorded against and must be one of
/// `succession.from_human_ids`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if any named place is unknown, [`AppError::PlaceDomain`] if
/// `human_id` is not among the `from` places, either endpoint list is empty, or a `from`/`to` place
/// is not yet projected (`UnknownPlace`), or a workspace/store error.
pub async fn assert_place_succession(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    succession: PlaceSuccessionInput,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    let mut from = Vec::with_capacity(succession.from_human_ids.len());
    for id in &succession.from_human_ids {
        from.push(resolve_place_id(store, id).await?);
    }
    let mut to = Vec::with_capacity(succession.to_human_ids.len());
    for id in &succession.to_human_ids {
        to.push(resolve_place_id(store, id).await?);
    }
    execute_place_mutation(
        store,
        session,
        place_id,
        PlaceCommand::AssertSuccession {
            place_id,
            from,
            to,
            kind: succession.kind,
            date: succession.date,
        },
        meta,
    )
    .await
}

/// Asserts a place's geographic coordinates, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn assert_place_coordinates(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    coordinates: GeoCoordinates,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute_place_mutation(
        store,
        session,
        place_id,
        PlaceCommand::AssertCoordinates { place_id, coordinates },
        meta,
    )
    .await
}

/// Asserts a (possibly dated) geometry for a place, identified by `human_id` (ADR 0024). Unlike
/// [`assert_place_coordinates`], geometry assertions accumulate rather than replace: a place can
/// hold many dated boundaries over its history.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, [`AppError::PlaceDomain`] if the geometry is
/// invalid (a polygon ring with fewer than 3 points), or a workspace/store error.
pub async fn assert_place_geometry(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    geometry: PlaceGeometry,
    date: Option<GenealogicalDate>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute_place_mutation(
        store,
        session,
        place_id,
        PlaceCommand::AssertGeometry {
            place_id,
            geometry,
            date,
        },
        meta,
    )
    .await
}

/// Sets (or changes) a place's code, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, [`AppError::PlaceDomain`] if the code is
/// empty, or a workspace/store error.
pub async fn set_place_code(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    code: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute_place_mutation(store, session, place_id, PlaceCommand::SetCode { place_id, code }, meta).await
}

/// Adds a citation (by its `human_id`) backing a place's claims.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`]/[`AppError::CitationNotFound`] if either is unknown, or a
/// workspace/store error.
pub async fn add_place_citation(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    citation_human_id: &str,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    let citation_id = resolve_citation_id(store, citation_human_id).await?;
    execute_place_mutation(
        store,
        session,
        place_id,
        PlaceCommand::AddCitation { place_id, citation_id },
        meta,
    )
    .await
}

/// Attaches a media reference (by media aggregate id) to a place, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn attach_place_media(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    media_id: MediaId,
    input: MediaRefInput,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute_place_mutation(
        store,
        session,
        place_id,
        PlaceCommand::AttachMedia {
            place_id,
            media: MediaRef {
                media_id,
                crop: input.crop,
                caption: input.caption,
                citations: Vec::new(),
            },
        },
        meta,
    )
    .await
}

/// Re-edits an existing place media attachment (crop / caption) by the `AssertionId` of the attach
/// assertion — supersedes it with a new `MediaAttached` carrying the same media and citations plus
/// the new crop/caption (the row-Edit correction, ADR 0004 §2).
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, [`AppError::PlaceDomain`] if `assertion_id`
/// names no live media attachment, or a workspace/store error.
pub async fn update_place_media_ref(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
    input: MediaRefInput,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let view = store
        .find_place(human_id)
        .await?
        .ok_or_else(|| AppError::PlaceNotFound(human_id.to_owned()))?;
    let place_id = resolve_place_id(store, human_id).await?;
    let target = use_case::parse_assertion_id(assertion_id)?;
    let existing = view
        .media_with_assertions()
        .iter()
        .find(|attributed| attributed.assertion_id == target)
        .ok_or(AppError::PlaceDomain(PlaceError::SupersedesMissingAssertion(target)))?;
    let media = MediaRef {
        media_id: existing.value.media_id,
        crop: input.crop,
        caption: input.caption,
        citations: existing.value.citations.clone(),
    };
    let meta = MutationMeta {
        supersedes: Some(assertion_id),
        ..meta
    };
    execute_place_mutation(
        store,
        session,
        place_id,
        PlaceCommand::AttachMedia { place_id, media },
        meta,
    )
    .await
}

/// Attaches a note (by note aggregate id) to a place, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn attach_place_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute_place_mutation(
        store,
        session,
        place_id,
        PlaceCommand::AttachNote { place_id, note_id },
        meta,
    )
    .await
}

/// Attaches a media object (by its `human_id`) to a place — the frontend/importer-facing wrapper
/// that resolves the media `human_id` to its id, so a caller never handles UUIDs.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] / [`AppError::MediaNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_place_media(
    workspace: &Workspace,
    session: &Session,
    place_human_id: &str,
    media_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = use_case::resolve_id(
        store.find_media(media_human_id).await?,
        genealogy_core::media::MediaView::media_id,
        || AppError::MediaNotFound(media_human_id.to_owned()),
    )?;
    attach_place_media(
        workspace,
        session,
        place_human_id,
        media_id,
        MediaRefInput::default(),
        MutationMeta::default(),
    )
    .await
}

/// Attaches a note (by its `human_id`) to a place — the frontend/importer-facing wrapper.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] / [`AppError::NoteNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_place_note(
    workspace: &Workspace,
    session: &Session,
    place_human_id: &str,
    note_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = use_case::resolve_id(
        store.find_note(note_human_id).await?,
        genealogy_core::note::NoteView::note_id,
        || AppError::NoteNotFound(note_human_id.to_owned()),
    )?;
    attach_place_note(workspace, session, place_human_id, note_id, MutationMeta::default()).await
}

/// Applies (or removes) a tag on a place, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn tag_place(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    let command = if remove {
        PlaceCommand::Untag { place_id, tag_id }
    } else {
        PlaceCommand::Tag { place_id, tag_id }
    };
    execute_place_mutation(store, session, place_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Loads a single place's summary by `human_id`, with its succession relations (ADR 0026 §4).
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_place(workspace: &Workspace, human_id: &str) -> Result<Option<PlaceSummary>, AppError> {
    show_place_resolved(workspace, human_id, None).await
}

/// Loads a single place's summary resolved **as of** `as_of` (ADR 0026 §1): the name and
/// jurisdiction chain reflect the assertions in effect at that date, not the current/primary ones —
/// the entry point a future time slider (ADR 0025) drives.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_place_as_of(
    workspace: &Workspace,
    human_id: &str,
    as_of: GenealogicalDate,
) -> Result<Option<PlaceSummary>, AppError> {
    show_place_resolved(workspace, human_id, Some(as_of)).await
}

/// Shared implementation: resolves `human_id`'s summary (current/primary when `as_of` is `None`),
/// then joins its succession relations from the cross-aggregate index with a single-place query, so
/// [`list_places`] — which never needs them — does not pay for a workspace-wide bulk join.
async fn show_place_resolved(
    workspace: &Workspace,
    human_id: &str,
    as_of: Option<GenealogicalDate>,
) -> Result<Option<PlaceSummary>, AppError> {
    let Some(view) = workspace.store().find_place(human_id).await? else {
        return Ok(None);
    };
    let lookups = PlaceLookups::load(workspace).await?;
    let mut summary = summarize_as_of(&view, &lookups, as_of.as_ref());
    let Some(place_id) = view.place_id() else {
        return Ok(Some(summary));
    };
    let store = workspace.store();
    let id = place_id.to_string();
    summary.predecessors = succession_refs(&store.place_predecessors(&id).await?, &lookups);
    summary.successors = succession_refs(&store.place_successors(&id).await?, &lookups);
    Ok(Some(summary))
}

/// Joins raw succession-index records to the place projection, skipping (and logging) any row whose
/// JSON-serialized kind/date fails to parse — defensive, since these are our own serializations, not
/// user input, so a failure here signals a real bug rather than bad data.
fn succession_refs(records: &[PlaceSuccessionRecord], lookups: &PlaceLookups) -> Vec<PlaceSuccessionRef> {
    records
        .iter()
        .filter_map(|record| succession_ref(record, lookups))
        .collect()
}

/// Parses and joins one succession-index record; `None` (logged) on a malformed `kind`/`date`.
fn succession_ref(record: &PlaceSuccessionRecord, lookups: &PlaceLookups) -> Option<PlaceSuccessionRef> {
    let kind = match serde_json::from_str::<SuccessionKind>(&record.kind) {
        Ok(kind) => kind,
        Err(error) => {
            tracing::warn!(%error, kind = %record.kind, "unparseable succession kind; skipping");
            return None;
        }
    };
    let date = match record
        .date_json
        .as_deref()
        .map(serde_json::from_str::<GenealogicalDate>)
    {
        Some(Ok(date)) => Some(date),
        Some(Err(error)) => {
            tracing::warn!(%error, "unparseable succession date; skipping");
            return None;
        }
        None => None,
    };
    let info = uuid::Uuid::parse_str(&record.place_id)
        .ok()
        .map(PlaceId::from_uuid)
        .and_then(|id| lookups.places.get(&id));
    Some(PlaceSuccessionRef {
        human_id: info.map_or_else(|| record.place_id.clone(), |i| i.human_id.clone()),
        id: record.place_id.clone(),
        name: info.and_then(|i| i.name.clone()),
        kind,
        date,
        assertion_id: record.assertion_id.clone(),
    })
}

/// Lists every place's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_places(workspace: &Workspace) -> Result<Vec<PlaceSummary>, AppError> {
    let views = workspace.store().list_places().await?;
    let lookups = PlaceLookups::load(workspace).await?;
    Ok(views.iter().map(|view| summarize(view, &lookups)).collect())
}

/// Lists every place's summary, resolved **as of** `as_of` (ADR 0026 §1) — the geography view's feed
/// (`show_geography`) for a chosen time-slider year. Mirrors [`list_places`], resolving each place's
/// name, jurisdiction, and geometry at the same target date rather than the current/primary one.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_places_as_of(workspace: &Workspace, as_of: GenealogicalDate) -> Result<Vec<PlaceSummary>, AppError> {
    let views = workspace.store().list_places().await?;
    let lookups = PlaceLookups::load(workspace).await?;
    Ok(views
        .iter()
        .map(|view| summarize_as_of(view, &lookups, Some(&as_of)))
        .collect())
}

/// An enclosing place joined to the Place projection: the `human_id`, primary name, and type.
struct PlaceInfo {
    human_id: String,
    name: Option<String>,
    place_type: Option<PlaceType>,
}

/// The lookups `summarize` needs to join a place's enclosing chain, succession relations, and
/// attachments to the other projections without a per-row query (the cross-aggregate join lives
/// here — the app/db layer). `views` backs the transitive hierarchy walk (ADR 0026 §1): each hop
/// resolves against another place's own folded state, not just its flat `PlaceInfo` summary.
struct PlaceLookups {
    places: HashMap<PlaceId, PlaceInfo>,
    views: HashMap<PlaceId, PlaceView>,
    citations: HashMap<CitationId, CitationRef>,
    media: HashMap<MediaId, MediaLookup>,
    notes: HashMap<NoteId, String>,
    tags: HashMap<TagId, TagRef>,
}

impl PlaceLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let store = workspace.store();
        let mut places = HashMap::new();
        let mut views = HashMap::new();
        for view in store.list_places().await? {
            if let (Some(id), Some(human_id)) = (view.place_id(), view.human_id()) {
                places.insert(
                    id,
                    PlaceInfo {
                        human_id: human_id.as_str().to_owned(),
                        name: view.names().first().map(|n| n.text.clone()),
                        place_type: view.place_type().cloned(),
                    },
                );
                views.insert(id, view);
            }
        }
        Ok(Self {
            places,
            views,
            citations: citation_refs(store).await?,
            media: media_lookups(store).await?,
            notes: use_case::note_human_ids(store).await?,
            tags: tag_refs(store).await?,
        })
    }
}

/// Sets a place's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute_place_mutation(
        store,
        session,
        place_id,
        PlaceCommand::SetRestrictions { place_id, restrictions },
        meta,
    )
    .await
}

/// Sets (or changes) a place's user-facing identifier, identified by its current `human_id`,
/// returning the effective new id.
///
/// A supplied non-blank `new` id is dup-checked (a collision with a *different* record is
/// [`AppError::HumanIdTaken`]); a blank/absent `new` allocates the next free id from the workspace's
/// configured format (the regenerate case).
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if the place is unknown, [`AppError::HumanIdTaken`] if the requested
/// id is already in use, or a workspace/store error.
pub async fn set_place_human_id(
    workspace: &Workspace,
    session: &Session,
    current_human_id: &str,
    new: Option<String>,
    provenance: Provenance,
) -> Result<String, AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, current_human_id).await?;
    let human_id = match use_case::requested_human_id(new) {
        Some(id) => {
            if id != current_human_id && store.find_place(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_place_human_id(&workspace.place_id_format()?).await?,
    };
    execute(
        store,
        session,
        &place_id.to_string(),
        PlaceCommand::SetHumanId {
            place_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        Vec::new(),
    )
    .await?;
    Ok(human_id)
}

/// Executes one command through the store, stamping the operator `provenance` and backing
/// `citations`, and mapping the command outcome to [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: PlaceCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = PlaceCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_place(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create place mutation, applying the operator-intent [`MutationMeta`]: resolves
/// the backing citations, and — when `meta.supersedes` is set — wraps `command` in a
/// [`PlaceCommand::SupersedeAssertion`] so the new assertion replaces the named one (ADR 0004 §2).
async fn execute_place_mutation(
    store: &Store,
    session: &Session,
    place_id: PlaceId,
    command: PlaceCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(place_id, command, target);
    execute(
        store,
        session,
        &place_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in a [`PlaceCommand::SupersedeAssertion`] against `target` when superseding, or
/// returns it unchanged for a plain assertion.
fn superseded(place_id: PlaceId, command: PlaceCommand, target: Option<AssertionId>) -> PlaceCommand {
    match target {
        Some(target) => PlaceCommand::SupersedeAssertion {
            place_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
}

/// Resolves a `human_id` to its aggregate [`PlaceId`], or [`AppError::PlaceNotFound`].
async fn resolve_place_id(store: &Store, human_id: &str) -> Result<PlaceId, AppError> {
    use_case::resolve_id(store.find_place(human_id).await?, PlaceView::place_id, || {
        AppError::PlaceNotFound(human_id.to_owned())
    })
}

/// Resolves a citation `human_id` to its aggregate [`CitationId`], or [`AppError::CitationNotFound`].
async fn resolve_citation_id(store: &Store, human_id: &str) -> Result<CitationId, AppError> {
    use_case::resolve_id(store.find_citation(human_id).await?, CitationView::citation_id, || {
        AppError::CitationNotFound(human_id.to_owned())
    })
}

/// Builds a [`PlaceName`] from plain text (language/date are not collected by the CLI yet).
fn place_name(text: String) -> PlaceName {
    PlaceName {
        text,
        language: None,
        date: None,
    }
}

/// Renders a [`PlaceView`] into the frontend DTO, joining the enclosing chain and attachments to the
/// other projections via `lookups`.
/// Resolves an assertion's backing evidence to the joined citation [`CitationRef`]s (dropping DNA
/// matches and any citation not in the lookup), for the scalar coordinate/code provenance popovers.
fn resolve_place_citations(evidence: &[EvidenceRef], lookups: &PlaceLookups) -> Vec<CitationRef> {
    evidence
        .iter()
        .filter_map(|reference| reference.as_citation())
        .filter_map(|id| lookups.citations.get(&id).cloned())
        .collect()
}

/// Builds the place's asserted names with their provenance, in assertion order.
fn name_refs(view: &PlaceView) -> Vec<PlaceNameRef> {
    view.names_with_assertions()
        .iter()
        .map(|attributed| {
            let asserted = &attributed.value;
            PlaceNameRef {
                text: asserted.value.text.clone(),
                language: asserted.value.language.as_ref().map(|l| l.as_str().to_owned()),
                date: asserted.value.date.clone(),
                confidence: asserted.confidence,
                source_count: asserted.citation_ids().count(),
                assertion_id: attributed.assertion_id.to_string(),
            }
        })
        .collect()
}

/// Resolves the single geometry a place holds **as of** `as_of_sort_value` (ADR 0026 §1): the
/// latest-dated assertion at or before the target, falling back to the first-asserted
/// (undated/primary) one when none qualifies — the same rule [`resolved_name`] and [`resolve_hop`]
/// apply, now extended to geometry so the geography view's time slider can resolve a place's
/// boundary alongside its name and jurisdiction. `None` resolves the current/primary geometry.
fn resolved_geometry(
    view: &PlaceView,
    lookups: &PlaceLookups,
    as_of_sort_value: Option<i64>,
) -> Option<PlaceGeometryRef> {
    let attributed = match as_of_sort_value {
        Some(target) => view.geometry_as_of(target),
        None => view.geometries_with_assertions().first(),
    };
    attributed.map(|attributed| {
        let asserted = &attributed.value;
        PlaceGeometryRef {
            geometry: asserted.value.geometry.clone(),
            date: asserted.value.date.clone(),
            confidence: asserted.confidence,
            citations: resolve_place_citations(&asserted.citations, lookups),
            assertion_id: attributed.assertion_id.to_string(),
        }
    })
}

/// Builds the place's dated geometry assertions with their provenance, in assertion order (ADR
/// 0024) — these accumulate rather than replace, unlike the scalar `coordinates`.
fn geometry_refs(view: &PlaceView, lookups: &PlaceLookups) -> Vec<PlaceGeometryRef> {
    view.geometries_with_assertions()
        .iter()
        .map(|attributed| {
            let asserted = &attributed.value;
            PlaceGeometryRef {
                geometry: asserted.value.geometry.clone(),
                date: asserted.value.date.clone(),
                confidence: asserted.confidence,
                citations: resolve_place_citations(&asserted.citations, lookups),
                assertion_id: attributed.assertion_id.to_string(),
            }
        })
        .collect()
}

fn summarize(view: &PlaceView, lookups: &PlaceLookups) -> PlaceSummary {
    summarize_as_of(view, lookups, None)
}

/// Resolves the enclosing link a single place's own `enclosed_by` set carries **as of**
/// `as_of_sort_value` (ADR 0026 §1) — the primary (first-asserted) link when `None` — as one
/// [`HierarchyHop`] the walk can continue from. `None` when `place_id` is unknown or has no
/// qualifying link (a top-level place).
fn resolve_hop(
    place_id: PlaceId,
    views: &HashMap<PlaceId, PlaceView>,
    as_of_sort_value: Option<i64>,
) -> Option<HierarchyHop> {
    let view = views.get(&place_id)?;
    let link = match as_of_sort_value {
        Some(target) => view.enclosed_by_as_of(target),
        None => view.primary_enclosed_by(),
    }?;
    Some(HierarchyHop {
        place_id: link.value.value.place_id,
        date: link.value.value.date.clone(),
        confidence: link.value.confidence,
        assertion_id: link.assertion_id,
    })
}

/// The name a place resolves to **as of** `as_of_sort_value` (ADR 0026 §1) — the first-asserted name
/// (today's convention, matching [`PlaceInfo`]) when `None`.
fn resolved_name(view: &PlaceView, as_of_sort_value: Option<i64>) -> Option<String> {
    match as_of_sort_value {
        Some(target) => view.name_as_of(target).map(|a| a.value.value.text.clone()),
        None => view.names().first().map(|n| n.text.clone()),
    }
}

/// Joins one resolved hierarchy hop to the place projection (name/type), for the breadcrumb table.
fn enclosing_ref_from_hop(hop: &HierarchyHop, lookups: &PlaceLookups) -> PlaceEnclosingRef {
    let info = lookups.places.get(&hop.place_id);
    PlaceEnclosingRef {
        human_id: info.map_or_else(|| hop.place_id.to_string(), |i| i.human_id.clone()),
        id: hop.place_id.to_string(),
        name: info.and_then(|i| i.name.clone()),
        place_type: info.and_then(|i| i.place_type.clone()),
        date: hop.date.clone(),
        confidence: hop.confidence,
        assertion_id: hop.assertion_id.to_string(),
    }
}

/// Builds the DTO, resolving the name and the transitive enclosing chain **as of** `as_of`'s
/// `sort_value` when given (ADR 0026 §1) — the current/primary resolution when `None`.
/// `predecessors`/`successors` are always left empty here; `show_place`/`show_place_as_of` fill them
/// in with a separate, single-place succession-index query (avoiding an N+1 bulk join for the list
/// path — data-model §9's read-side counterpart).
fn summarize_as_of(view: &PlaceView, lookups: &PlaceLookups, as_of: Option<&GenealogicalDate>) -> PlaceSummary {
    let as_of_sort_value = as_of.map(|date| date.sort_value);
    let names = name_refs(view);
    let chain = view
        .place_id()
        .map(|place_id| hierarchy_chain(place_id, |id| resolve_hop(id, &lookups.views, as_of_sort_value)))
        .unwrap_or_default();
    let enclosing = chain.iter().map(|hop| enclosing_ref_from_hop(hop, lookups)).collect();
    let geometries = geometry_refs(view, lookups);
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
    let media = view
        .media_with_assertions()
        .iter()
        .filter_map(|attributed| {
            let media = &attributed.value;
            lookups.media.get(&media.media_id).map(|lookup| MediaRefSummary {
                human_id: lookup.human_id.clone(),
                id: lookup.id.clone(),
                caption: media.caption.clone(),
                crop: media.crop,
                path: lookup.path.clone(),
                mime: lookup.mime.clone(),
                assertion_id: attributed.assertion_id.to_string(),
            })
        })
        .collect();
    let notes = view
        .notes_with_assertions()
        .iter()
        .filter_map(|attributed| {
            lookups.notes.get(&attributed.value).map(|human_id| AttachedRef {
                human_id: human_id.clone(),
                id: attributed.value.to_string(),
                assertion_id: attributed.assertion_id.to_string(),
            })
        })
        .collect();
    let tags = view
        .tags()
        .into_iter()
        .filter_map(|id| lookups.tags.get(&id).cloned())
        .collect();
    let human_id = view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default();
    let own_name = resolved_name(view, as_of_sort_value);
    let ancestor_names: Vec<Option<String>> = chain
        .iter()
        .map(|hop| {
            lookups
                .views
                .get(&hop.place_id)
                .and_then(|v| resolved_name(v, as_of_sort_value))
        })
        .collect();
    let title = generated_title(own_name.as_deref(), &human_id, &ancestor_names);
    let resolved_geometry_ref = resolved_geometry(view, lookups, as_of_sort_value);
    PlaceSummary {
        human_id,
        id: view.place_id().map(|id| id.to_string()).unwrap_or_default(),
        generated_title: title,
        resolved_as_of: as_of.cloned(),
        place_type: view.place_type().cloned(),
        place_type_confidence: view.asserted_place_type().and_then(|a| a.confidence),
        names,
        code: view.code().map(ToOwned::to_owned),
        code_confidence: view.asserted_code().and_then(|a| a.confidence),
        code_citations: view
            .asserted_code()
            .map_or_else(Vec::new, |a| resolve_place_citations(&a.citations, lookups)),
        coordinates: view.coordinates().map(|c| format!("{},{}", c.latitude, c.longitude)),
        coordinates_confidence: view.asserted_coordinates().and_then(|a| a.confidence),
        coordinate_citations: view
            .asserted_coordinates()
            .map_or_else(Vec::new, |a| resolve_place_citations(&a.citations, lookups)),
        geometries,
        resolved_geometry: resolved_geometry_ref,
        enclosing,
        predecessors: Vec::new(),
        successors: Vec::new(),
        citations,
        media,
        notes,
        tags,
        restrictions: view.restrictions().clone(),
    }
}
