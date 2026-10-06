//! Intent dispatch: turns a data-loading [`Intent`] into a `vitni-app` use-case call and returns
//! render-ready view-models.
//!
//! This is the only place the presentation layer touches the application's use-cases. It is async
//! because the use-cases are; a renderer awaits it on its own runtime.

use std::collections::BTreeSet;

use vitni_app::{
    AppError, ChildParentRelationship, ImportedMediaRef, MediaRefInput, NewFact, NewParticipation, Provenance,
    Restriction, Session, Workspace, add_child, add_citation_attribute, add_event_citation, add_family_citation,
    add_media_attribute, add_media_citation, add_name, add_note_translation, add_partner, add_person_citation,
    add_place_citation, add_place_name, add_repository_address, add_repository_url, add_source_attribute,
    assert_association, assert_child_relationship, assert_citation_date_value, assert_event_address, assert_fact,
    assert_participation, assert_place_enclosed_by, assert_sex, attach_citation_media, attach_citation_note,
    attach_family_media, attach_family_note, attach_person_media, attach_person_note, change_log_for_citation,
    change_log_for_event, change_log_for_family, change_log_for_media, change_log_for_note, change_log_for_person,
    change_log_for_place, change_log_for_repository, change_log_for_research_note, change_log_for_source,
    citation_claim_owner, claim_owner, event_claim_owner, families_for_person, family_claim_owner,
    import_attach_event_media, import_attach_event_note, import_attach_media_note, import_attach_place_media,
    import_attach_place_note, import_attach_repository_note, import_attach_source_media, import_attach_source_note,
    link_family_event, link_place, link_source_repository, linked_records, list_citations, list_event_rows,
    list_family_rows, list_media, list_notes, list_person_rows, list_persons, list_places, list_repositories,
    list_sources, media_claim_owner, note_claim_owner, place_claim_owner, recent_activity, record_origin, remove_child,
    repository_claim_owner, set_citation_confidence, set_citation_evidence_analysis, set_citation_restrictions,
    set_event_restrictions, set_family_restrictions, set_media_restrictions, set_note_restrictions, set_note_text,
    set_note_type, set_page, set_place_restrictions, set_repository_restrictions, set_restrictions,
    set_source_restrictions, show_citation, show_event, show_family, show_media, show_note, show_person, show_place,
    show_repository, show_source, source_claim_owner, tag_citation, tag_event, tag_family, tag_media, tag_note,
    tag_person, tag_place, tag_repository, tag_source, undo_assertion, undo_citation_assertion, undo_event_assertion,
    undo_family_assertion, undo_media_assertion, undo_note_assertion, undo_place_assertion, undo_repository_assertion,
    undo_research_note_assertion, undo_source_assertion, unlink_person, workspace_counts,
};
use vitni_app::{
    CitationRefInput, NewCitationEntry, NewSourceEntry, PersonChangeSet, PersonTarget, PlaceholderRef, SourceRefInput,
    commit_person_change_set, set_person_human_id,
};
use vitni_app::{
    DecidableKind, MatchQueueFilter, MatchVerdict, ancestors, assess, check_records, decide_match, descendants,
    list_import_runs, match_pair_decision, match_queue, relationship, undo_match_distinction_and_merge,
};
use vitni_app::{
    DraftRecord, MatchBand, MatchableKind, SimilarRecord, assess_draft, find_similar, find_similar_to_draft,
};
use vitni_app::{
    NewResearchNote, NewResearchNoteSubject, add_subject_to_research_note, create_research_note, list_research_notes,
    list_research_notes_about, remove_subject_from_research_note, set_research_note_body,
    set_research_note_restrictions, show_research_note, tag_research_note,
};
use vitni_app::{
    TagChangeSet, TagTarget, add_dna_match_segment, assert_dna_match_shared_ancestor, assert_dna_test_haplogroup,
    build_shared_ancestor, change_log_for_dna_match, change_log_for_dna_test, change_log_for_tag,
    commit_tag_change_set, import_attach_dna_match_note, import_attach_dna_test_note, list_dna_matches, list_dna_tests,
    list_tags, set_dna_match_restrictions, set_dna_match_status, set_dna_test_restrictions, show_dna_match,
    show_dna_test, show_tag, tag_dna_match, tag_dna_test, undo_dna_match_assertion, undo_dna_test_assertion,
};

use vitni_app::{
    CitationChangeSet, DnaTestChangeSet, EventChangeSet, FamilyChangeSet, MediaChangeSet, NewPlaceEntry, NoteChangeSet,
    PartnerInput, PlaceChangeSet, PlaceRefInput, PlaceSuccessionInput, RepositoryChangeSet, SourceChangeSet,
    assert_event_date_value, assert_media_date_value, assert_place_coordinates, assert_place_geometry,
    assert_place_succession, build_genealogical_date, commit_citation_change_set, commit_dna_test_change_set,
    commit_event_change_set, commit_family_change_set, commit_media_change_set, commit_note_change_set,
    commit_place_change_set, commit_repository_change_set, commit_source_change_set, set_dna_test_genome_build,
    set_dna_test_kit_id, set_dna_test_provider, set_dna_test_type, set_event_description, set_event_type,
    set_media_file_path, set_media_mime, set_media_web_path, set_place_code, set_place_type, set_repository_name,
    set_repository_type, set_source_abbrev, set_source_author, set_source_pub_info, set_title, show_geography,
    year_only_date,
};
use vitni_app::{NewDnaMatch, observe_dna_match};
use vitni_app::{
    set_citation_human_id, set_dna_match_human_id, set_dna_test_human_id, set_event_human_id, set_family_human_id,
    set_media_human_id, set_note_human_id, set_place_human_id, set_repository_human_id, set_source_human_id,
};
use vitni_app::{
    update_citation_media_ref, update_event_media_ref, update_family_media_ref, update_person_media_ref,
    update_place_media_ref, update_source_media_ref,
};

use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::navigation::{
    Category, CitationChangeSetRequest, CitationEdit, CitationSourceRequest, DecideMatch, DnaMatchChangeSetRequest,
    DnaMatchEdit, DnaTestChangeSetRequest, DnaTestEdit, DraftCitationRef, DraftSourceRef, EventChangeSetRequest,
    EventEdit, EventPlaceRequest, FamilyChangeSetRequest, FamilyEdit, Intent, MatchDecision, MediaChangeSetRequest,
    MediaEdit, NewRecordRequest, NoteChangeSetRequest, NoteEdit, PartnerRequest, PersonChangeSetRequest, PersonEdit,
    PlaceChangeSetRequest, PlaceEdit, RepositoryChangeSetRequest, RepositoryEdit, ResearchNoteChangeSetRequest,
    ResearchNoteEdit, SourceChangeSetRequest, SourceEdit, SubjectRequest, TagChangeSetRequest,
};
use crate::view_model::{
    CitationDetail, CompareSide, DASHBOARD_MATCHES, DashboardVm, DataQualityVm, DnaMatchDetail, DnaTestDetail,
    EventDetail, FamilyDetail, FamilyVm, GeographyVm, LISTED_MATCHES, MatchCompareVm, MatchQueueVm, MediaDetail,
    MediaRefVm, NoteDetail, PedigreeVm, PersonDetail, PlaceDetail, ProvenanceDraft, RelationshipVm, RepositoryDetail,
    ResearchNoteDetail, SimilarHitVm, SimilarVm, SourceDetail, TagDetail, citation_row, collapse_history,
    dna_match_row, dna_test_row, event_list_row, event_row, family_list_row, family_row, media_row, note_row,
    person_list_row, place_row, repository_row, research_note_row, source_row, tag_row,
};

/// How many recent changes the dashboard activity feed shows.
const ACTIVITY_LIMIT: u32 = 12;

/// How many quick entry points "Jump back in" shows.
const JUMP_BACK_LIMIT: usize = 4;

/// The data a dispatched [`Intent`] produced.
///
/// Not `Eq`: [`PlaceDetail`]'s map point carries decimal-degree floats, which have no total equality.
#[derive(Debug, Clone, PartialEq)]
pub enum IntentOutcome {
    /// The workspace dashboard.
    Dashboard(Box<DashboardVm>),
    /// The dashboard's data-quality results (filled by the second, slower dashboard load).
    DataQuality(Box<DataQualityVm>),
    /// The list, as generic rows.
    List(Vec<RowVm>),
    /// One person's detail.
    Detail(Box<PersonDetail>),
    /// One citation's detail.
    CitationDetail(Box<CitationDetail>),
    /// One family's detail.
    FamilyDetail(Box<FamilyDetail>),
    /// One event's detail.
    EventDetail(Box<EventDetail>),
    /// One place's detail.
    PlaceDetail(Box<PlaceDetail>),
    /// One source's detail.
    SourceDetail(Box<SourceDetail>),
    /// One repository's detail.
    RepositoryDetail(Box<RepositoryDetail>),
    /// One media object's detail.
    MediaDetail(Box<MediaDetail>),
    /// One note's detail.
    NoteDetail(Box<NoteDetail>),
    /// One research note's detail.
    ResearchNoteDetail(Box<ResearchNoteDetail>),
    /// One tag's detail.
    TagDetail(Box<TagDetail>),
    /// One DNA test's detail.
    DnaTestDetail(Box<DnaTestDetail>),
    /// One DNA match's detail.
    DnaMatchDetail(Box<DnaMatchDetail>),
    /// The Pedigree tool's ancestor + descendant charts for one focus person.
    Pedigree(Box<PedigreeVm>),
    /// The kinship calculator's result for two people.
    Relationship(Box<RelationshipVm>),
    /// The Matches tool's possible-matches table (ADR 0039 §3).
    MatchQueue(Box<MatchQueueVm>),
    /// The shared compare view, loaded for a chosen pair.
    MatchCompare(Box<MatchCompareVm>),
    /// The stored records similar to a record or a record being created (ADR 0038 §8).
    Similar(Box<SimilarVm>),
    /// The Geography tool's markers, event pins, and time-slider resolution (ADR 0025 §1). The map's
    /// tile provider is deliberately absent: `dispatch` has no config access by design (workspace +
    /// localizer only), so the renderer reads the client-scope `[map]` section itself, the same way the
    /// assisted-import `[ai]` config is read outside `dispatch`
    /// (`vitni-ui-dioxus/src/services.rs`).
    Geography(Box<GeographyVm>),
    /// The requested record id was not found.
    NotFound {
        /// The id that was looked up.
        human_id: String,
    },
}

/// Dispatches `intent` against the workspace, building localized view-models via `loc`.
///
/// A missing person is returned as [`IntentOutcome::NotFound`] rather than an error so a renderer can
/// show it gracefully; infrastructure failures surface as [`AppError`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (e.g. a database failure).
pub async fn dispatch(workspace: &Workspace, loc: &Localizer, intent: &Intent) -> Result<IntentOutcome, AppError> {
    match intent {
        Intent::ShowDashboard => show_dashboard(workspace, loc).await,
        Intent::ShowDataQuality => show_data_quality(workspace, loc).await,
        Intent::ShowList => {
            let person_rows = list_person_rows(workspace).await?;
            let mut rows = Vec::with_capacity(person_rows.len());
            for person in &person_rows {
                rows.push(person_list_row(person, loc));
            }
            Ok(IntentOutcome::List(rows))
        }
        Intent::ShowPerson { human_id } => show_person_detail(workspace, loc, human_id).await,
        Intent::ShowCitationList => {
            let summaries = list_citations(workspace).await?;
            let mut rows = Vec::with_capacity(summaries.len());
            for summary in &summaries {
                rows.push(citation_row(summary, loc));
            }
            Ok(IntentOutcome::List(rows))
        }
        Intent::ShowCitation { human_id } => match show_citation(workspace, human_id).await? {
            Some(summary) => {
                let mut detail = CitationDetail::from_summary(&summary, loc);
                let change_log = change_log_for_citation(workspace, human_id).await?;
                detail.history = collapse_history(&change_log, loc);
                Ok(IntentOutcome::CitationDetail(Box::new(detail)))
            }
            None => Ok(IntentOutcome::NotFound {
                human_id: human_id.clone(),
            }),
        },
        Intent::ShowFamilyList => {
            let family_rows = list_family_rows(workspace).await?;
            let mut rows = Vec::with_capacity(family_rows.len());
            for family in &family_rows {
                rows.push(family_list_row(family, loc));
            }
            Ok(IntentOutcome::List(rows))
        }
        Intent::ShowFamily { human_id } => match show_family(workspace, human_id).await? {
            Some(summary) => {
                let mut detail = FamilyDetail::from_summary(&summary, loc);
                detail.research_notes =
                    research_notes_about(workspace, loc, NewResearchNoteSubject::Family(human_id.clone())).await?;
                let change_log = change_log_for_family(workspace, human_id).await?;
                detail.history = collapse_history(&change_log, loc);
                Ok(IntentOutcome::FamilyDetail(Box::new(detail)))
            }
            None => Ok(IntentOutcome::NotFound {
                human_id: human_id.clone(),
            }),
        },
        Intent::ShowEventList => {
            let event_rows = list_event_rows(workspace).await?;
            let mut rows = Vec::with_capacity(event_rows.len());
            for event in &event_rows {
                rows.push(event_list_row(event, loc));
            }
            Ok(IntentOutcome::List(rows))
        }
        Intent::ShowEvent { human_id } => show_event_detail(workspace, loc, human_id).await,
        Intent::ShowPlaceList => {
            let summaries = list_places(workspace).await?;
            let mut rows = Vec::with_capacity(summaries.len());
            for summary in &summaries {
                rows.push(place_row(summary, loc));
            }
            Ok(IntentOutcome::List(rows))
        }
        Intent::ShowPlace { human_id } => show_place_detail(workspace, loc, human_id).await,
        Intent::ShowSourceList => source_list(workspace, loc).await,
        Intent::ShowSource { human_id } => show_source_detail(workspace, loc, human_id).await,
        Intent::ShowRepositoryList => repository_list(workspace, loc).await,
        Intent::ShowRepository { human_id } => show_repository_detail(workspace, loc, human_id).await,
        Intent::ShowMediaList => media_list(workspace, loc).await,
        Intent::ShowMedia { human_id } => show_media_detail(workspace, loc, human_id).await,
        Intent::ShowNoteList => note_list(workspace, loc).await,
        Intent::ShowNote { human_id } => show_note_detail(workspace, loc, human_id).await,
        Intent::ShowResearchNoteList => research_note_list(workspace, loc).await,
        Intent::ShowResearchNote { human_id } => show_research_note_detail(workspace, loc, human_id).await,
        Intent::ShowTagList => tag_list(workspace, loc).await,
        Intent::ShowTag { id } => show_tag_detail(workspace, loc, id).await,
        Intent::ShowDnaTestList => dna_test_list(workspace, loc).await,
        Intent::ShowDnaTest { human_id } => show_dna_test_detail(workspace, loc, human_id).await,
        Intent::ShowDnaMatchList => dna_match_list(workspace, loc).await,
        Intent::ShowDnaMatch { human_id } => show_dna_match_detail(workspace, loc, human_id).await,
        Intent::ShowPedigree { human_id, depth } => show_pedigree(workspace, loc, human_id, *depth).await,
        Intent::ComputeRelationship { human_id_a, human_id_b } => {
            compute_relationship(workspace, loc, human_id_a, human_id_b).await
        }
        Intent::ListMatches { filter } => list_matches(workspace, loc, filter).await,
        Intent::MatchCompare { kind, left, right } => match_compare(workspace, loc, *kind, left, right).await,
        Intent::FindSimilar { kind, human_id } => find_similar_view(workspace, loc, *kind, human_id).await,
        Intent::SimilarToDraft { draft } => similar_to_draft_view(workspace, loc, draft).await,
        Intent::DraftCompare { draft, right } => draft_compare(workspace, loc, draft, right).await,
        Intent::ShowGeography { year } => show_geography_view(workspace, loc, *year).await,
    }
}

/// Loads the Geography tool's markers and event pins, resolved as of `year` (ADR 0025 §1, ADR 0026
/// §1). Carries no tile provider — see [`IntentOutcome::Geography`]'s doc for why `dispatch` cannot
/// resolve the configured one itself.
async fn show_geography_view(
    workspace: &Workspace,
    loc: &Localizer,
    year: Option<i32>,
) -> Result<IntentOutcome, AppError> {
    let summary = show_geography(workspace, year).await?;
    let vm = GeographyVm::from_summary(&summary, loc);
    Ok(IntentOutcome::Geography(Box::new(vm)))
}

/// Loads the fast dashboard: counts, evidence health, recent activity, and jump-back.
///
/// Deliberately does *not* run the whole-workspace data-quality checks — those fill the data-quality
/// card via a separate [`Intent::ShowDataQuality`] load so the dashboard renders without waiting on
/// them. Loads the person projection once (shared by evidence health and activity name resolution).
async fn show_dashboard(workspace: &Workspace, loc: &Localizer) -> Result<IntentOutcome, AppError> {
    let counts = workspace_counts(workspace).await?;
    let persons = list_persons(workspace).await?;
    let activity = recent_activity(workspace, ACTIVITY_LIMIT).await?;
    let dashboard = DashboardVm::build(counts, &persons, &activity, loc, JUMP_BACK_LIMIT);
    Ok(IntentOutcome::Dashboard(Box::new(dashboard)))
}

/// Runs the dashboard's data-quality checks over a single shared person load and groups them into the
/// [`DataQualityVm`] the data-quality card renders.
async fn show_data_quality(workspace: &Workspace, loc: &Localizer) -> Result<IntentOutcome, AppError> {
    let persons = list_persons(workspace).await?;
    let quality = check_records(workspace, &persons, DASHBOARD_MATCHES).await?;
    let data_quality = DataQualityVm::build(&persons, &quality, loc);
    Ok(IntentOutcome::DataQuality(Box::new(data_quality)))
}

/// The possible-matches queue under `filter`, and the import runs it can be narrowed to (the Matches
/// tool's table).
async fn list_matches(
    workspace: &Workspace,
    loc: &Localizer,
    filter: &MatchQueueFilter,
) -> Result<IntentOutcome, AppError> {
    let queue = match_queue(workspace, filter, Some(LISTED_MATCHES)).await?;
    let runs = list_import_runs(workspace).await?;
    Ok(IntentOutcome::MatchQueue(Box::new(MatchQueueVm::build(
        &queue, &runs, loc,
    ))))
}

/// Loads the pair `left`, `right` of `kind` into the shared compare view, with the decision already
/// taken between their clusters (ADR 0039 §4). Like [`show_pedigree`], an unknown `human_id`
/// propagates as an [`AppError`] rather than [`IntentOutcome::NotFound`] — the Matches tool has no
/// per-record detail pane to degrade gracefully into.
async fn match_compare(
    workspace: &Workspace,
    loc: &Localizer,
    kind: DecidableKind,
    left: &str,
    right: &str,
) -> Result<IntentOutcome, AppError> {
    let assessment = assess(workspace, kind.matchable(), left, right).await?;
    let category = Category::from_matchable_kind(kind.matchable());
    let left_label = resolve_record_name(workspace, loc, category, left).await?;
    let right_label = resolve_record_name(workspace, loc, category, right).await?;
    let left_origin = record_origin(workspace, kind.matchable(), left).await?;
    let right_origin = record_origin(workspace, kind.matchable(), right).await?;
    let left_media = record_media(workspace, kind, left).await?;
    let right_media = record_media(workspace, kind, right).await?;
    let left_side = CompareSide {
        human_id: left,
        label: left_label.as_deref().unwrap_or(left),
        origin: left_origin.as_ref(),
        media: &left_media,
    };
    let right_side = CompareSide {
        human_id: right,
        label: right_label.as_deref().unwrap_or(right),
        origin: right_origin.as_ref(),
        media: &right_media,
    };
    let mut vm = MatchCompareVm::build(left_side, right_side, &assessment, loc);
    vm.earlier_decision = match_pair_decision(workspace, kind, left, right).await?;
    Ok(IntentOutcome::MatchCompare(Box::new(vm)))
}

/// The most records *Find similar* lists.
const FIND_SIMILAR_LIMIT: usize = 10;

/// The most records the similar-record hint names.
const DRAFT_HINT_LIMIT: usize = 3;

/// The records like the record `human_id` of `kind` (*Find similar*).
async fn find_similar_view(
    workspace: &Workspace,
    loc: &Localizer,
    kind: MatchableKind,
    human_id: &str,
) -> Result<IntentOutcome, AppError> {
    let similar = find_similar(workspace, kind, human_id, MatchBand::Possible, FIND_SIMILAR_LIMIT).await?;
    similar_view(workspace, loc, kind, similar).await
}

/// The records like `draft`, a record being created (the similar-record hint).
async fn similar_to_draft_view(
    workspace: &Workspace,
    loc: &Localizer,
    draft: &DraftRecord,
) -> Result<IntentOutcome, AppError> {
    let similar = find_similar_to_draft(workspace, draft, MatchBand::Possible, DRAFT_HINT_LIMIT).await?;
    similar_view(workspace, loc, draft.kind(), similar).await
}

/// The records of `kind` in `similar`, each labelled by its live name.
async fn similar_view(
    workspace: &Workspace,
    loc: &Localizer,
    kind: MatchableKind,
    similar: Vec<SimilarRecord>,
) -> Result<IntentOutcome, AppError> {
    let category = Category::from_matchable_kind(kind);
    let mut hits = Vec::with_capacity(similar.len());
    for record in &similar {
        let label = match kind {
            MatchableKind::Tag => None,
            MatchableKind::Person
            | MatchableKind::Family
            | MatchableKind::Event
            | MatchableKind::Place
            | MatchableKind::Source
            | MatchableKind::Repository
            | MatchableKind::Citation
            | MatchableKind::Media
            | MatchableKind::Note => resolve_record_name(workspace, loc, category, &record.record.human_id).await?,
        };
        hits.push(SimilarHitVm::build(kind, record, label, loc));
    }
    Ok(IntentOutcome::Similar(Box::new(SimilarVm { hits })))
}

/// Loads the record being created, `draft`, and the stored record `right` of its kind into the shared
/// compare view. The draft side has no id yet, so it shows the localized "New" in its place.
async fn draft_compare(
    workspace: &Workspace,
    loc: &Localizer,
    draft: &DraftRecord,
    right: &str,
) -> Result<IntentOutcome, AppError> {
    let kind = draft.kind();
    let assessment = assess_draft(workspace, draft, right).await?;
    let right_label = resolve_record_name(workspace, loc, Category::from_matchable_kind(kind), right).await?;
    let right_origin = record_origin(workspace, kind, right).await?;
    let right_media = match DecidableKind::from_matchable(kind) {
        Some(decidable) => record_media(workspace, decidable, right).await?,
        None => Vec::new(),
    };
    let new_id = loc.similar_draft_side();
    let left = CompareSide {
        human_id: &new_id,
        label: &draft_label(draft),
        origin: None,
        media: &[],
    };
    let right = CompareSide {
        human_id: right,
        label: right_label.as_deref().unwrap_or(right),
        origin: right_origin.as_ref(),
        media: &right_media,
    };
    Ok(IntentOutcome::MatchCompare(Box::new(MatchCompareVm::build(
        left,
        right,
        &assessment,
        loc,
    ))))
}

/// How a record being created names itself: a person's name, a place's name, a source's title.
fn draft_label(draft: &DraftRecord) -> String {
    match draft {
        DraftRecord::Person { name, .. } => [name.given.as_deref(), name.surname.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" "),
        DraftRecord::Place { name, .. } | DraftRecord::Repository { name } => name.clone(),
        DraftRecord::Source { title, .. } => title.clone(),
    }
}

/// The media attached to the record `human_id` of `kind`, for its side's evidence snippet; none for a
/// kind that carries no media references.
async fn record_media(workspace: &Workspace, kind: DecidableKind, human_id: &str) -> Result<Vec<MediaRefVm>, AppError> {
    let media = match kind {
        DecidableKind::Person => show_person(workspace, human_id).await?.map(|summary| summary.media),
        DecidableKind::Family => show_family(workspace, human_id).await?.map(|summary| summary.media),
        DecidableKind::Event => show_event(workspace, human_id).await?.map(|summary| summary.media),
        DecidableKind::Place => show_place(workspace, human_id).await?.map(|summary| summary.media),
        DecidableKind::Source => show_source(workspace, human_id).await?.map(|summary| summary.media),
        DecidableKind::Citation => show_citation(workspace, human_id).await?.map(|summary| summary.media),
        DecidableKind::Repository | DecidableKind::Media | DecidableKind::Note => None,
    };
    Ok(media.unwrap_or_default().iter().map(MediaRefVm::from_ref).collect())
}

/// Dispatches a [`DecideMatch`] request to `vitni_app::decide_match` (or, for
/// [`MatchDecision::UndoDistinctionAndSame`], `vitni_app::undo_match_distinction_and_merge`), mutating
/// the workspace. Unlike [`dispatch`] (a read), this emits an event; the renderer bumps its data
/// version to refresh the queue afterwards. Returns the localized confirmation the screen shows.
///
/// # Errors
///
/// Propagates the [`AppError`] from the use-case (either `human_id` not found, a domain refusal —
/// see [`AppError::identity_refusal`] — or a database failure).
pub async fn dispatch_decide_match(
    workspace: &Workspace,
    session: &Session,
    loc: &Localizer,
    request: &DecideMatch,
) -> Result<String, AppError> {
    let DecideMatch {
        kind,
        left,
        right,
        decision,
        judgment,
    } = request;
    let (kind, decided) = (*kind, judgment.decision());
    match decision {
        MatchDecision::Same => {
            decide_match(workspace, session, kind, left, right, MatchVerdict::Same, decided).await?;
            Ok(loc.match_merged_summary(kind, right, left))
        }
        MatchDecision::UndoDistinctionAndSame => {
            undo_match_distinction_and_merge(workspace, session, kind, left, right, decided).await?;
            Ok(loc.match_merged_summary(kind, right, left))
        }
        MatchDecision::Distinct => {
            decide_match(workspace, session, kind, left, right, MatchVerdict::Distinct, decided).await?;
            Ok(loc.match_distinguished_summary(kind, right, left))
        }
    }
}

/// Resolves the current primary display name of the record `(category, human_id)`, or `None` when
/// the record has no name (or does not exist).
///
/// Record links render this so a rename is reflected everywhere the record is linked; the tab-label
/// rule ([`tab_label`](crate::navigation::tab_label)) supplies the human-id fallback for the `None`
/// case. Reuses each aggregate's row builder for its established, localized display label, except a
/// person, whose raw name (absent for an unnamed person) drives the human-id fallback.
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying `show_*` use-case (e.g. a database failure).
pub async fn resolve_record_name(
    workspace: &Workspace,
    loc: &Localizer,
    category: Category,
    human_id: &str,
) -> Result<Option<String>, AppError> {
    let name = match category {
        Category::People => show_person(workspace, human_id)
            .await?
            .and_then(|summary| summary.display_name),
        Category::Families => show_family(workspace, human_id)
            .await?
            .map(|summary| family_row(&summary, loc).title),
        Category::Events => show_event(workspace, human_id)
            .await?
            .map(|summary| event_row(&summary, loc).title),
        Category::Places => show_place(workspace, human_id)
            .await?
            .map(|summary| place_row(&summary, loc).title),
        Category::Sources => show_source(workspace, human_id)
            .await?
            .map(|summary| source_row(&summary, loc).title),
        Category::Citations => show_citation(workspace, human_id)
            .await?
            .map(|summary| citation_row(&summary, loc).title),
        Category::Repositories => show_repository(workspace, human_id)
            .await?
            .map(|summary| repository_row(&summary, loc).title),
        Category::Media => show_media(workspace, human_id)
            .await?
            .map(|summary| media_row(&summary, loc).title),
        Category::Notes => show_note(workspace, human_id)
            .await?
            .map(|summary| note_row(&summary, loc).title),
        Category::ResearchNotes => show_research_note(workspace, human_id)
            .await?
            .map(|summary| research_note_row(&summary, loc).title),
        Category::Tags => show_tag(workspace, human_id)
            .await?
            .map(|summary| tag_row(&summary, loc).title),
        Category::DnaTests => show_dna_test(workspace, human_id)
            .await?
            .map(|summary| dna_test_row(&summary, loc).title),
        Category::DnaMatches => show_dna_match(workspace, human_id)
            .await?
            .map(|summary| dna_match_row(&summary, loc).title),
        Category::Dashboard => None,
    };
    Ok(name)
}

/// Loads the tag list as generic rows.
async fn tag_list(workspace: &Workspace, loc: &Localizer) -> Result<IntentOutcome, AppError> {
    let summaries = list_tags(workspace).await?;
    let mut rows = Vec::with_capacity(summaries.len());
    for summary in &summaries {
        rows.push(tag_row(summary, loc));
    }
    Ok(IntentOutcome::List(rows))
}

/// Loads one tag's detail (summary with usage join + change log), or [`IntentOutcome::NotFound`].
async fn show_tag_detail(workspace: &Workspace, loc: &Localizer, id: &str) -> Result<IntentOutcome, AppError> {
    match show_tag(workspace, id).await? {
        Some(summary) => {
            let mut detail = TagDetail::from_summary(&summary, loc);
            let change_log = change_log_for_tag(workspace, id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::TagDetail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: id.to_owned(),
        }),
    }
}

/// Loads the DNA-test list as generic rows.
async fn dna_test_list(workspace: &Workspace, loc: &Localizer) -> Result<IntentOutcome, AppError> {
    let summaries = list_dna_tests(workspace).await?;
    let mut rows = Vec::with_capacity(summaries.len());
    for summary in &summaries {
        rows.push(dna_test_row(summary, loc));
    }
    Ok(IntentOutcome::List(rows))
}

/// Loads one DNA test's detail (joined summary + change log), or [`IntentOutcome::NotFound`].
async fn show_dna_test_detail(
    workspace: &Workspace,
    loc: &Localizer,
    human_id: &str,
) -> Result<IntentOutcome, AppError> {
    match show_dna_test(workspace, human_id).await? {
        Some(summary) => {
            let mut detail = DnaTestDetail::from_summary(&summary, loc);
            let change_log = change_log_for_dna_test(workspace, human_id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::DnaTestDetail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: human_id.to_owned(),
        }),
    }
}

/// Loads the DNA-match list as generic rows.
async fn dna_match_list(workspace: &Workspace, loc: &Localizer) -> Result<IntentOutcome, AppError> {
    let summaries = list_dna_matches(workspace).await?;
    let mut rows = Vec::with_capacity(summaries.len());
    for summary in &summaries {
        rows.push(dna_match_row(summary, loc));
    }
    Ok(IntentOutcome::List(rows))
}

/// Loads one DNA match's detail (joined summary + change log), or [`IntentOutcome::NotFound`].
async fn show_dna_match_detail(
    workspace: &Workspace,
    loc: &Localizer,
    human_id: &str,
) -> Result<IntentOutcome, AppError> {
    match show_dna_match(workspace, human_id).await? {
        Some(summary) => {
            let mut detail = DnaMatchDetail::from_summary(&summary, loc);
            let change_log = change_log_for_dna_match(workspace, human_id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::DnaMatchDetail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: human_id.to_owned(),
        }),
    }
}

/// Loads the Pedigree tool's ancestor and descendant charts for one focus person, `depth`
/// generations on each side.
///
/// Unlike the `show_*` use-cases above, a missing person surfaces as a propagated
/// [`AppError::PersonNotFound`] rather than [`IntentOutcome::NotFound`] — the generic error surface
/// (`Localizer::error`) already renders it, and the Pedigree tool has no per-record detail pane to
/// degrade gracefully into.
async fn show_pedigree(
    workspace: &Workspace,
    loc: &Localizer,
    human_id: &str,
    depth: u32,
) -> Result<IntentOutcome, AppError> {
    let ancestor_chart = ancestors(workspace, human_id, depth).await?;
    let descendant_chart = descendants(workspace, human_id, depth).await?;
    let depth = usize::try_from(depth).unwrap_or(usize::MAX);
    let vm = PedigreeVm::build(&ancestor_chart, &descendant_chart, depth, loc);
    Ok(IntentOutcome::Pedigree(Box::new(vm)))
}

/// Computes the kinship between two people (the Pedigree tool's Relationships view). As with
/// [`show_pedigree`], an unknown `human_id` propagates as an [`AppError`] rather than
/// [`IntentOutcome::NotFound`].
async fn compute_relationship(
    workspace: &Workspace,
    loc: &Localizer,
    human_id_a: &str,
    human_id_b: &str,
) -> Result<IntentOutcome, AppError> {
    let result = relationship(workspace, human_id_a, human_id_b).await?;
    let vm = RelationshipVm::build(&result, loc);
    Ok(IntentOutcome::Relationship(Box::new(vm)))
}

/// Loads the media list as generic rows.
async fn media_list(workspace: &Workspace, loc: &Localizer) -> Result<IntentOutcome, AppError> {
    let summaries = list_media(workspace).await?;
    let mut rows = Vec::with_capacity(summaries.len());
    for summary in &summaries {
        rows.push(media_row(summary, loc));
    }
    Ok(IntentOutcome::List(rows))
}

/// Loads the note list as generic rows.
async fn note_list(workspace: &Workspace, loc: &Localizer) -> Result<IntentOutcome, AppError> {
    let summaries = list_notes(workspace).await?;
    let mut rows = Vec::with_capacity(summaries.len());
    for summary in &summaries {
        rows.push(note_row(summary, loc));
    }
    Ok(IntentOutcome::List(rows))
}

/// Loads the research-note list as generic rows.
async fn research_note_list(workspace: &Workspace, loc: &Localizer) -> Result<IntentOutcome, AppError> {
    let summaries = list_research_notes(workspace).await?;
    let mut rows = Vec::with_capacity(summaries.len());
    for summary in &summaries {
        rows.push(research_note_row(summary, loc));
    }
    Ok(IntentOutcome::List(rows))
}

/// Loads one research note's detail (joined summary + change log), or [`IntentOutcome::NotFound`].
async fn show_research_note_detail(
    workspace: &Workspace,
    loc: &Localizer,
    human_id: &str,
) -> Result<IntentOutcome, AppError> {
    match show_research_note(workspace, human_id).await? {
        Some(summary) => {
            let mut detail = ResearchNoteDetail::from_summary(&summary, loc);
            let change_log = change_log_for_research_note(workspace, human_id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::ResearchNoteDetail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: human_id.to_owned(),
        }),
    }
}

/// Loads the research notes arguing about one record, as the reverse-lookup tab's rows (ADR 0028 §5) —
/// the Person / Family / Event / Place detail loaders fill their `research_notes` field with this, the
/// same way they fill `history`.
async fn research_notes_about(
    workspace: &Workspace,
    loc: &Localizer,
    subject: NewResearchNoteSubject,
) -> Result<Vec<RowVm>, AppError> {
    let summaries = list_research_notes_about(workspace, subject).await?;
    let mut rows = Vec::with_capacity(summaries.len());
    for summary in &summaries {
        rows.push(research_note_row(summary, loc));
    }
    Ok(rows)
}

/// Maps a UI [`SubjectRequest`] to the app's `human_id`-keyed subject input, or [`AppError`] when the
/// category is not one of the four conclusion-bearing aggregates a subject may name (ADR 0028 §2).
fn subject_input(subject: &SubjectRequest) -> Result<NewResearchNoteSubject, AppError> {
    let human_id = subject.human_id.clone();
    match subject.category {
        Category::People => Ok(NewResearchNoteSubject::Person(human_id)),
        Category::Families => Ok(NewResearchNoteSubject::Family(human_id)),
        Category::Events => Ok(NewResearchNoteSubject::Event(human_id)),
        Category::Places => Ok(NewResearchNoteSubject::Place(human_id)),
        Category::Dashboard
        | Category::Sources
        | Category::Citations
        | Category::Repositories
        | Category::Media
        | Category::Notes
        | Category::ResearchNotes
        | Category::Tags
        | Category::DnaTests
        | Category::DnaMatches => Err(AppError::ResearchNoteDomain(
            vitni_app::ResearchNoteError::UnknownSubject,
        )),
    }
}

/// Loads one media object's detail (joined summary + change log), or [`IntentOutcome::NotFound`].
async fn show_media_detail(workspace: &Workspace, loc: &Localizer, human_id: &str) -> Result<IntentOutcome, AppError> {
    match show_media(workspace, human_id).await? {
        Some(summary) => {
            let mut detail = MediaDetail::from_summary(&summary, loc);
            let change_log = change_log_for_media(workspace, human_id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::MediaDetail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: human_id.to_owned(),
        }),
    }
}

/// Loads one note's detail (joined summary + change log), or [`IntentOutcome::NotFound`].
async fn show_note_detail(workspace: &Workspace, loc: &Localizer, human_id: &str) -> Result<IntentOutcome, AppError> {
    match show_note(workspace, human_id).await? {
        Some(summary) => {
            let mut detail = NoteDetail::from_summary(&summary, loc);
            let change_log = change_log_for_note(workspace, human_id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::NoteDetail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: human_id.to_owned(),
        }),
    }
}

/// Loads one person's detail (summary joined with events/families/citations + the collapsed change
/// log), or [`IntentOutcome::NotFound`].
async fn show_person_detail(workspace: &Workspace, loc: &Localizer, human_id: &str) -> Result<IntentOutcome, AppError> {
    match show_person(workspace, human_id).await? {
        Some(summary) => {
            let mut detail = PersonDetail::from_summary(&summary, loc);
            detail.families = families_for_person(workspace, human_id)
                .await?
                .iter()
                .map(|family| FamilyVm::from_app(family, loc))
                .collect();
            detail.research_notes =
                research_notes_about(workspace, loc, NewResearchNoteSubject::Person(human_id.to_owned())).await?;
            detail.link(&linked_records(workspace, human_id).await?, loc);
            let change_log = change_log_for_person(workspace, human_id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::Detail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: human_id.to_owned(),
        }),
    }
}

/// Loads the source list as generic rows.
async fn source_list(workspace: &Workspace, loc: &Localizer) -> Result<IntentOutcome, AppError> {
    let summaries = list_sources(workspace).await?;
    let mut rows = Vec::with_capacity(summaries.len());
    for summary in &summaries {
        rows.push(source_row(summary, loc));
    }
    Ok(IntentOutcome::List(rows))
}

/// Loads the repository list as generic rows.
async fn repository_list(workspace: &Workspace, loc: &Localizer) -> Result<IntentOutcome, AppError> {
    let summaries = list_repositories(workspace).await?;
    let mut rows = Vec::with_capacity(summaries.len());
    for summary in &summaries {
        rows.push(repository_row(summary, loc));
    }
    Ok(IntentOutcome::List(rows))
}

/// Loads one source's detail (joined summary + collapsed change log), or [`IntentOutcome::NotFound`].
async fn show_source_detail(workspace: &Workspace, loc: &Localizer, human_id: &str) -> Result<IntentOutcome, AppError> {
    match show_source(workspace, human_id).await? {
        Some(summary) => {
            let mut detail = SourceDetail::from_summary(&summary, loc);
            let change_log = change_log_for_source(workspace, human_id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::SourceDetail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: human_id.to_owned(),
        }),
    }
}

/// Loads one repository's detail (joined summary + change log), or [`IntentOutcome::NotFound`].
async fn show_repository_detail(
    workspace: &Workspace,
    loc: &Localizer,
    human_id: &str,
) -> Result<IntentOutcome, AppError> {
    match show_repository(workspace, human_id).await? {
        Some(summary) => {
            let mut detail = RepositoryDetail::from_summary(&summary, loc);
            let change_log = change_log_for_repository(workspace, human_id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::RepositoryDetail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: human_id.to_owned(),
        }),
    }
}

/// Loads one event's detail (joined summary + collapsed change log), or [`IntentOutcome::NotFound`].
async fn show_event_detail(workspace: &Workspace, loc: &Localizer, human_id: &str) -> Result<IntentOutcome, AppError> {
    match show_event(workspace, human_id).await? {
        Some(summary) => {
            let mut detail = EventDetail::from_summary(&summary, loc);
            detail.research_notes =
                research_notes_about(workspace, loc, NewResearchNoteSubject::Event(human_id.to_owned())).await?;
            let change_log = change_log_for_event(workspace, human_id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::EventDetail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: human_id.to_owned(),
        }),
    }
}

/// Loads one place's detail (joined summary + collapsed change log), or [`IntentOutcome::NotFound`].
async fn show_place_detail(workspace: &Workspace, loc: &Localizer, human_id: &str) -> Result<IntentOutcome, AppError> {
    match show_place(workspace, human_id).await? {
        Some(summary) => {
            let mut detail = PlaceDetail::from_summary(&summary, loc);
            detail.research_notes =
                research_notes_about(workspace, loc, NewResearchNoteSubject::Place(human_id.to_owned())).await?;
            let change_log = change_log_for_place(workspace, human_id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::PlaceDetail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: human_id.to_owned(),
        }),
    }
}

/// Commits a [`PersonChangeSetRequest`] (the buffered person dialog) through
/// [`commit_person_change_set`], returning the person's `human_id`.
///
/// Maps the UI-side draft (string tag ids, existing/pending references) to the app-layer
/// [`PersonChangeSet`]; the app mints ids for new aggregates, resolves the intra-set placeholder
/// references, and commits the graph as one operator action (create the person + name + gender +
/// tags + any new source/citation, or, on edit, only the diff). Dispatched only when the operator
/// presses OK — Cancel never reaches here.
///
/// # Errors
///
/// Propagates the [`AppError`] from `commit_person_change_set` (a duplicate `human_id`, a domain
/// rejection such as an empty name, a missing referenced record, or a database failure).
pub async fn dispatch_person_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &PersonChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let target = match &request.existing_human_id {
        Some(human_id) => PersonTarget::Existing {
            human_id: human_id.clone(),
        },
        None => PersonTarget::New {
            human_id: request.human_id_override.clone().filter(|id| !id.is_empty()),
            birth: request.birth,
        },
    };
    let change_set = PersonChangeSet {
        target,
        name: request.name.clone(),
        name_citation: request.name_citation.as_ref().map(map_citation_ref),
        sex: request.sex.clone(),
        tags: request.tags.clone(),
        new_sources: request
            .new_sources
            .iter()
            .map(|source| NewSourceEntry {
                placeholder: PlaceholderRef(source.placeholder.clone()),
                title: source.title.clone(),
            })
            .collect(),
        new_citations: request
            .new_citations
            .iter()
            .map(|citation| NewCitationEntry {
                placeholder: PlaceholderRef(citation.placeholder.clone()),
                source: map_source_ref(&citation.source),
                page: citation.page.clone(),
            })
            .collect(),
        provenance: prov.provenance(),
        citations: prov.citations.clone(),
    };
    let human_id = commit_person_change_set(workspace, session, change_set).await?;
    // On edit the draft carries the (possibly changed) human id in `human_id_override`. The change-set
    // diffs the claims; the identity change is a separate audited command, applied only when the id
    // actually differs (a cleared field regenerates). The rename returns the effective id to reload by.
    match &request.existing_human_id {
        Some(current) if request.human_id_override.as_deref().map(str::trim) != Some(current.as_str()) => {
            set_person_human_id(
                workspace,
                session,
                current,
                request.human_id_override.clone(),
                prov.provenance(),
            )
            .await
        }
        _ => Ok(human_id),
    }
}

/// Maps a draft citation reference to the app-layer [`CitationRefInput`].
fn map_citation_ref(reference: &DraftCitationRef) -> CitationRefInput {
    match reference {
        DraftCitationRef::Existing(human_id) => CitationRefInput::Existing(human_id.clone()),
        DraftCitationRef::Pending(placeholder) => CitationRefInput::Pending(PlaceholderRef(placeholder.clone())),
    }
}

/// Maps a draft source reference to the app-layer [`SourceRefInput`].
fn map_source_ref(reference: &DraftSourceRef) -> SourceRefInput {
    match reference {
        DraftSourceRef::Existing(human_id) => SourceRefInput::Existing(human_id.clone()),
        DraftSourceRef::Pending(placeholder) => SourceRefInput::Pending(PlaceholderRef(placeholder.clone())),
    }
}

/// Dispatches a [`PersonEdit`] to its `vitni-app` command use-case, returning the person's
/// `human_id` — the detail to reload afterwards.
///
/// Unlike [`dispatch`] (a read), this mutates the workspace and is stamped with the session's
/// operator/clock/id. The returned id is always [`PersonEdit::target`]: unlike the other eleven
/// aggregates, `PersonEdit` has no `SetHumanId`, so nothing here can rename the person. A correction of
/// a row a merged member owns is written to that member (ADR 0039 §5), while the target still names
/// the cluster the pane shows.
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_person_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &PersonEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let owner = person_edit_owner(workspace, edit, prov).await?;
    let human_id = &owner;
    let outcome = match edit {
        PersonEdit::AssertName { name, .. } => add_name(workspace, session, human_id, name.clone(), prov.meta()).await,
        PersonEdit::AssertSex { sex, .. } => assert_sex(workspace, session, human_id, sex.clone(), prov.meta()).await,
        PersonEdit::SetRestrictions { restrictions, .. } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_restrictions(workspace, session, human_id, restrictions, prov.meta()).await
        }
        PersonEdit::AssertFact { fact_type, value, .. } => {
            let new = NewFact {
                fact_type: fact_type.clone(),
                value: value.clone(),
                date: None,
            };
            assert_fact(workspace, session, human_id, new, prov.meta()).await
        }
        PersonEdit::AttachCitation { citation_id, .. } => {
            add_person_citation(workspace, session, human_id, citation_id, prov.meta()).await
        }
        PersonEdit::AttachMedia { media_id, .. } => {
            attach_person_media(
                workspace,
                session,
                human_id,
                media_id,
                MediaRefInput::default(),
                prov.meta(),
            )
            .await
        }
        PersonEdit::SetMediaRegion {
            assertion_id,
            crop,
            caption,
            ..
        } => {
            update_person_media_ref(
                workspace,
                session,
                human_id,
                assertion_id,
                MediaRefInput {
                    crop: *crop,
                    caption: caption.clone(),
                },
                prov.meta(),
            )
            .await
        }
        PersonEdit::AttachNote { note_id, .. } => {
            attach_person_note(workspace, session, human_id, note_id, prov.meta()).await
        }
        PersonEdit::AssertAssociation { other_id, role, .. } => {
            assert_association(workspace, session, human_id, other_id, role.clone(), prov.meta()).await
        }
        PersonEdit::AssertParticipation {
            event_id,
            role,
            age,
            attributes,
            notes,
            ..
        } => {
            let new = NewParticipation {
                role: role.clone(),
                age: age.clone(),
                attributes: attributes.clone(),
                notes: notes.clone(),
            };
            assert_participation(workspace, session, human_id, event_id, new, prov.meta()).await
        }
        PersonEdit::Tag { tag_id, remove, .. } => {
            tag_person(workspace, session, human_id, tag_id, *remove, prov.meta()).await
        }
        PersonEdit::UndoAssertion { assertion_id, .. } => {
            undo_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale).await
        }
        PersonEdit::Unlink { member, .. } => {
            unlink_person(workspace, session, human_id, member, prov.provenance().rationale).await
        }
    };
    outcome.map(|()| edit.target().to_owned())
}

/// The record an edit on a person's detail writes to (ADR 0039 §5): a correction — an undo, a media
/// region update, or any edit superseding a row — goes to the cluster record whose stream holds that
/// row; anything else goes to the record the edit names.
async fn person_edit_owner(
    workspace: &Workspace,
    edit: &PersonEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let corrected = match edit {
        PersonEdit::UndoAssertion { assertion_id, .. } | PersonEdit::SetMediaRegion { assertion_id, .. } => {
            Some(assertion_id.as_str())
        }
        PersonEdit::AssertName { .. }
        | PersonEdit::AssertSex { .. }
        | PersonEdit::SetRestrictions { .. }
        | PersonEdit::AssertFact { .. }
        | PersonEdit::AttachCitation { .. }
        | PersonEdit::AttachMedia { .. }
        | PersonEdit::AttachNote { .. }
        | PersonEdit::AssertAssociation { .. }
        | PersonEdit::AssertParticipation { .. }
        | PersonEdit::Tag { .. } => prov.supersedes.as_deref(),
        PersonEdit::Unlink { .. } => None,
    };
    match corrected {
        Some(assertion_id) => claim_owner(workspace, edit.target(), assertion_id).await,
        None => Ok(edit.target().to_owned()),
    }
}

/// Dispatches a [`CitationEdit`] to its `vitni-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected citation ([`CitationEdit::target`]) afterwards. Mirrors
/// [`dispatch_person_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_citation_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &CitationEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let owner = match corrected_assertion(edit_correction_citation(edit), prov) {
        Some(assertion_id) => citation_claim_owner(workspace, edit.target(), assertion_id).await?,
        None => edit.target().to_owned(),
    };
    if owner != edit.target() {
        dispatch_citation_edit_to(workspace, session, &edit.retargeted(&owner), prov).await?;
        return Ok(edit.target().to_owned());
    }
    dispatch_citation_edit_to(workspace, session, edit, prov).await
}

/// The row a citation undo or media-region edit names.
fn edit_correction_citation(edit: &CitationEdit) -> Option<&str> {
    match edit {
        CitationEdit::SetMediaRegion { assertion_id, .. } | CitationEdit::UndoAssertion { assertion_id, .. } => {
            Some(assertion_id)
        }
        CitationEdit::SetHumanId { .. }
        | CitationEdit::SetPage { .. }
        | CitationEdit::SetDate { .. }
        | CitationEdit::SetConfidence { .. }
        | CitationEdit::SetEvidenceAnalysis { .. }
        | CitationEdit::AddAttribute { .. }
        | CitationEdit::AttachMedia { .. }
        | CitationEdit::AttachNote { .. }
        | CitationEdit::Tag { .. }
        | CitationEdit::SetRestrictions { .. } => None,
    }
}

/// Dispatches a [`CitationEdit`] to the record it names.
async fn dispatch_citation_edit_to(
    workspace: &Workspace,
    session: &Session,
    edit: &CitationEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match edit {
        CitationEdit::SetHumanId { human_id, new_human_id } => {
            set_citation_human_id(workspace, session, human_id, new_human_id.clone(), prov.provenance()).await
        }
        CitationEdit::SetPage { human_id, page } => set_page(workspace, session, human_id, page.clone(), prov.meta())
            .await
            .map(|()| human_id.clone()),
        CitationEdit::SetDate { human_id, date } => assert_citation_date_value(
            workspace,
            session,
            human_id,
            build_genealogical_date(date.clone()),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        CitationEdit::SetConfidence { human_id, confidence } => {
            set_citation_confidence(workspace, session, human_id, (*confidence).into(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        CitationEdit::SetEvidenceAnalysis { human_id, analysis } => {
            set_citation_evidence_analysis(workspace, session, human_id, *analysis, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        CitationEdit::AddAttribute {
            human_id,
            attribute_type,
            value,
        } => add_citation_attribute(
            workspace,
            session,
            human_id,
            attribute_type.clone(),
            value.clone(),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        CitationEdit::AttachMedia { human_id, media_id } => attach_citation_media(
            workspace,
            session,
            human_id,
            media_id,
            MediaRefInput::default(),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        CitationEdit::SetMediaRegion {
            human_id,
            assertion_id,
            crop,
            caption,
        } => update_citation_media_ref(
            workspace,
            session,
            human_id,
            assertion_id,
            MediaRefInput {
                crop: *crop,
                caption: caption.clone(),
            },
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        CitationEdit::AttachNote { human_id, note_id } => {
            attach_citation_note(workspace, session, human_id, note_id, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        CitationEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_citation(workspace, session, human_id, tag_id, *remove, prov.meta())
            .await
            .map(|()| human_id.clone()),
        CitationEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_citation_restrictions(workspace, session, human_id, restrictions, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        CitationEdit::UndoAssertion { human_id, assertion_id } => {
            undo_citation_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale)
                .await
                .map(|()| human_id.clone())
        }
    }
}

/// Dispatches a [`FamilyEdit`] to its `vitni-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected family ([`FamilyEdit::target`]) afterwards. Mirrors
/// [`dispatch_person_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_family_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &FamilyEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let owner = match corrected_assertion(edit_correction_family(edit), prov) {
        Some(assertion_id) => family_claim_owner(workspace, edit.target(), assertion_id).await?,
        None => edit.target().to_owned(),
    };
    if owner != edit.target() {
        dispatch_family_edit_to(workspace, session, &edit.retargeted(&owner), prov).await?;
        return Ok(edit.target().to_owned());
    }
    dispatch_family_edit_to(workspace, session, edit, prov).await
}

/// Dispatches a [`FamilyEdit`] to the record it names.
async fn dispatch_family_edit_to(
    workspace: &Workspace,
    session: &Session,
    edit: &FamilyEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match edit {
        FamilyEdit::SetHumanId { human_id, new_human_id } => {
            set_family_human_id(workspace, session, human_id, new_human_id.clone(), prov.provenance()).await
        }
        FamilyEdit::AddPartner { human_id, person_id } => {
            add_partner(workspace, session, human_id, person_id, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        FamilyEdit::AddChild {
            human_id,
            person_id,
            relationships,
        } => {
            let relationships: Vec<(String, ChildParentRelationship)> = relationships.clone();
            add_child(workspace, session, human_id, person_id, relationships, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        FamilyEdit::AssertChildRelationship {
            human_id,
            person_id,
            partner_id,
            relationship,
        } => assert_child_relationship(
            workspace,
            session,
            human_id,
            person_id,
            partner_id,
            relationship.clone(),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        FamilyEdit::RemoveChild { human_id, person_id } => {
            remove_child(workspace, session, human_id, person_id, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        FamilyEdit::LinkFamilyEvent { human_id, event_id } => {
            link_family_event(workspace, session, human_id, event_id, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        FamilyEdit::AttachMedia { human_id, media_id } => attach_family_media(
            workspace,
            session,
            human_id,
            media_id,
            MediaRefInput::default(),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        FamilyEdit::SetMediaRegion { .. } => family_set_region(workspace, session, edit, prov).await,
        FamilyEdit::AttachNote { human_id, note_id } => {
            attach_family_note(workspace, session, human_id, note_id, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        FamilyEdit::AttachCitation { human_id, citation_id } => {
            add_family_citation(workspace, session, human_id, citation_id, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        FamilyEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_family(workspace, session, human_id, tag_id, *remove, prov.meta())
            .await
            .map(|()| human_id.clone()),
        FamilyEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_family_restrictions(workspace, session, human_id, restrictions, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        FamilyEdit::UndoAssertion { human_id, assertion_id } => {
            undo_family_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale)
                .await
                .map(|()| human_id.clone())
        }
    }
}

/// Supersedes a family media reference's crop/caption ([`FamilyEdit::SetMediaRegion`]), returning the
/// family `human_id` to reload. Mirrors [`event_set_region`] — the whole edit comes in so the caller's
/// match arm stays a one-line delegate; the `let else` is unreachable in practice.
async fn family_set_region(
    ws: &Workspace,
    session: &Session,
    edit: &FamilyEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let FamilyEdit::SetMediaRegion {
        human_id,
        assertion_id,
        crop,
        caption,
    } = edit
    else {
        return Ok(String::new());
    };
    let input = MediaRefInput {
        crop: *crop,
        caption: caption.clone(),
    };
    update_family_media_ref(ws, session, human_id, assertion_id, input, prov.meta())
        .await
        .map(|()| human_id.clone())
}

/// Supersedes an event media reference's crop/caption ([`EventEdit::SetMediaRegion`]), returning the
/// event `human_id` to reload. Takes the whole edit so the caller's match arm stays a one-line
/// delegate (the field-by-field destructure lives here); the `let else` is unreachable in practice.
async fn event_set_region(
    ws: &Workspace,
    session: &Session,
    edit: &EventEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let EventEdit::SetMediaRegion {
        human_id,
        assertion_id,
        crop,
        caption,
    } = edit
    else {
        return Ok(String::new());
    };
    let input = MediaRefInput {
        crop: *crop,
        caption: caption.clone(),
    };
    update_event_media_ref(ws, session, human_id, assertion_id, input, prov.meta())
        .await
        .map(|()| human_id.clone())
}

/// Dispatches an [`EventEdit`] to its `vitni-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected event ([`EventEdit::target`]) afterwards. Mirrors
/// [`dispatch_family_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_event_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &EventEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let owner = match corrected_assertion(edit_correction_event(edit), prov) {
        Some(assertion_id) => event_claim_owner(workspace, edit.target(), assertion_id).await?,
        None => edit.target().to_owned(),
    };
    if owner != edit.target() {
        dispatch_event_edit_to(workspace, session, &edit.retargeted(&owner), prov).await?;
        return Ok(edit.target().to_owned());
    }
    dispatch_event_edit_to(workspace, session, edit, prov).await
}

/// The assertion a correction targets: the row an undo or media-region edit names, else the one the
/// provenance supersedes — the row whose owner the edit is written to (ADR 0039 §5).
fn corrected_assertion<'a>(row: Option<&'a str>, prov: &'a ProvenanceDraft) -> Option<&'a str> {
    row.or(prov.supersedes.as_deref())
}

/// The row an event undo or media-region edit names.
fn edit_correction_event(edit: &EventEdit) -> Option<&str> {
    match edit {
        EventEdit::UndoAssertion { assertion_id, .. } | EventEdit::SetMediaRegion { assertion_id, .. } => {
            Some(assertion_id)
        }
        EventEdit::SetHumanId { .. }
        | EventEdit::SetType { .. }
        | EventEdit::SetDate { .. }
        | EventEdit::SetDescription { .. }
        | EventEdit::LinkPlace { .. }
        | EventEdit::AddAddress { .. }
        | EventEdit::AddParticipant { .. }
        | EventEdit::AttachCitation { .. }
        | EventEdit::AttachMedia { .. }
        | EventEdit::AttachNote { .. }
        | EventEdit::Tag { .. }
        | EventEdit::SetRestrictions { .. } => None,
    }
}

/// The row a family undo or media-region edit names.
fn edit_correction_family(edit: &FamilyEdit) -> Option<&str> {
    match edit {
        FamilyEdit::UndoAssertion { assertion_id, .. } | FamilyEdit::SetMediaRegion { assertion_id, .. } => {
            Some(assertion_id)
        }
        FamilyEdit::SetHumanId { .. }
        | FamilyEdit::AddPartner { .. }
        | FamilyEdit::AddChild { .. }
        | FamilyEdit::AssertChildRelationship { .. }
        | FamilyEdit::RemoveChild { .. }
        | FamilyEdit::LinkFamilyEvent { .. }
        | FamilyEdit::AttachMedia { .. }
        | FamilyEdit::AttachNote { .. }
        | FamilyEdit::AttachCitation { .. }
        | FamilyEdit::Tag { .. }
        | FamilyEdit::SetRestrictions { .. } => None,
    }
}

/// Dispatches an [`EventEdit`] to the record it names.
async fn dispatch_event_edit_to(
    workspace: &Workspace,
    session: &Session,
    edit: &EventEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match edit {
        EventEdit::SetHumanId { human_id, new_human_id } => {
            set_event_human_id(workspace, session, human_id, new_human_id.clone(), prov.provenance()).await
        }
        EventEdit::SetType { human_id, event_type } => {
            set_event_type(workspace, session, human_id, event_type.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        EventEdit::SetDate { human_id, date } => assert_event_date_value(
            workspace,
            session,
            human_id,
            build_genealogical_date(date.clone()),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        EventEdit::SetDescription { human_id, description } => {
            set_event_description(workspace, session, human_id, description.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        EventEdit::LinkPlace { human_id, place_id } => link_place(workspace, session, human_id, place_id, prov.meta())
            .await
            .map(|()| human_id.clone()),
        EventEdit::AddAddress { human_id, address } => {
            assert_event_address(workspace, session, human_id, address.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        EventEdit::AddParticipant {
            human_id,
            person_id,
            role,
            age,
            attributes,
            notes,
        } => assert_participation(
            workspace,
            session,
            person_id,
            human_id,
            NewParticipation {
                role: role.clone(),
                age: age.clone(),
                attributes: attributes.clone(),
                notes: notes.clone(),
            },
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        EventEdit::AttachCitation { human_id, citation_id } => {
            add_event_citation(workspace, session, human_id, citation_id, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        EventEdit::AttachMedia { human_id, media_id } => {
            let media = ImportedMediaRef {
                media_human_id: media_id.clone(),
                input: MediaRefInput::default(),
            };
            import_attach_event_media(workspace, session, human_id, media, Provenance::default())
                .await
                .map(|()| human_id.clone())
        }
        EventEdit::SetMediaRegion { .. } => event_set_region(workspace, session, edit, prov).await,
        EventEdit::AttachNote { human_id, note_id } => {
            import_attach_event_note(workspace, session, human_id, note_id, Provenance::default())
                .await
                .map(|()| human_id.clone())
        }
        EventEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_event(workspace, session, human_id, tag_id, *remove, prov.meta())
            .await
            .map(|()| human_id.clone()),
        EventEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_event_restrictions(workspace, session, human_id, restrictions, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        EventEdit::UndoAssertion { human_id, assertion_id } => {
            undo_event_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale)
                .await
                .map(|()| human_id.clone())
        }
    }
}

/// Dispatches a [`PlaceEdit`] to its `vitni-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected place ([`PlaceEdit::target`]) afterwards. Mirrors
/// [`dispatch_family_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_place_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &PlaceEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let owner = match corrected_assertion(edit_correction_place(edit), prov) {
        Some(assertion_id) => place_claim_owner(workspace, edit.target(), assertion_id).await?,
        None => edit.target().to_owned(),
    };
    if owner != edit.target() {
        dispatch_place_edit_to(workspace, session, &edit.retargeted(&owner), prov).await?;
        return Ok(edit.target().to_owned());
    }
    dispatch_place_edit_to(workspace, session, edit, prov).await
}

/// The row a place undo or media-region edit names.
fn edit_correction_place(edit: &PlaceEdit) -> Option<&str> {
    match edit {
        PlaceEdit::SetMediaRegion { assertion_id, .. } | PlaceEdit::UndoAssertion { assertion_id, .. } => {
            Some(assertion_id)
        }
        PlaceEdit::SetHumanId { .. }
        | PlaceEdit::SetType { .. }
        | PlaceEdit::SetCoordinates { .. }
        | PlaceEdit::AssertGeometry { .. }
        | PlaceEdit::SetCode { .. }
        | PlaceEdit::AddName { .. }
        | PlaceEdit::AddEnclosing { .. }
        | PlaceEdit::AssertSuccession { .. }
        | PlaceEdit::AttachCitation { .. }
        | PlaceEdit::AttachMedia { .. }
        | PlaceEdit::AttachNote { .. }
        | PlaceEdit::Tag { .. }
        | PlaceEdit::SetRestrictions { .. } => None,
    }
}

/// Dispatches a [`PlaceEdit`] to the record it names.
async fn dispatch_place_edit_to(
    workspace: &Workspace,
    session: &Session,
    edit: &PlaceEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match edit {
        PlaceEdit::SetHumanId { human_id, new_human_id } => {
            set_place_human_id(workspace, session, human_id, new_human_id.clone(), prov.provenance()).await
        }
        PlaceEdit::SetType { human_id, place_type } => {
            set_place_type(workspace, session, human_id, place_type.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        PlaceEdit::SetCoordinates { human_id, coordinates } => {
            assert_place_coordinates(workspace, session, human_id, *coordinates, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        PlaceEdit::AssertGeometry {
            human_id,
            geometry,
            year,
        } => {
            let date = year.map(year_only_date);
            assert_place_geometry(workspace, session, human_id, geometry.clone(), date, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        PlaceEdit::SetCode { human_id, code } => {
            set_place_code(workspace, session, human_id, code.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        PlaceEdit::AddName { human_id, text } => {
            add_place_name(workspace, session, human_id, text.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        PlaceEdit::AddEnclosing { human_id, enclosing_id } => {
            assert_place_enclosed_by(workspace, session, human_id, enclosing_id, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        PlaceEdit::AssertSuccession { .. } => place_assert_succession(workspace, session, edit, prov).await,
        PlaceEdit::AttachCitation { human_id, citation_id } => {
            add_place_citation(workspace, session, human_id, citation_id, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        PlaceEdit::AttachMedia { human_id, media_id } => {
            import_attach_place_media(workspace, session, human_id, media_id)
                .await
                .map(|()| human_id.clone())
        }
        PlaceEdit::SetMediaRegion {
            human_id,
            assertion_id,
            crop,
            caption,
        } => update_place_media_ref(
            workspace,
            session,
            human_id,
            assertion_id,
            MediaRefInput {
                crop: *crop,
                caption: caption.clone(),
            },
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        PlaceEdit::AttachNote { human_id, note_id } => import_attach_place_note(workspace, session, human_id, note_id)
            .await
            .map(|()| human_id.clone()),
        PlaceEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_place(workspace, session, human_id, tag_id, *remove, prov.meta())
            .await
            .map(|()| human_id.clone()),
        PlaceEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_place_restrictions(workspace, session, human_id, restrictions, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        PlaceEdit::UndoAssertion { human_id, assertion_id } => {
            undo_place_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale)
                .await
                .map(|()| human_id.clone())
        }
    }
}

/// Asserts a place succession ([`PlaceEdit::AssertSuccession`]), returning the anchor place's
/// `human_id` to reload. Takes the whole edit so the caller's match arm stays a one-line delegate (the
/// field-by-field destructure lives here); the `let else` is unreachable in practice.
///
/// The anchor **leads** the ceasing set: `assert_place_succession` rejects a `from` list that omits
/// `human_id` (`SuccessionAnchorMismatch`), so a caller never has to repeat it in `from_extra`.
async fn place_assert_succession(
    ws: &Workspace,
    session: &Session,
    edit: &PlaceEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let PlaceEdit::AssertSuccession {
        human_id,
        from_extra,
        to,
        kind,
        date,
    } = edit
    else {
        return Ok(edit.target().to_owned());
    };
    let mut from_human_ids = Vec::with_capacity(from_extra.len() + 1);
    from_human_ids.push(human_id.clone());
    from_human_ids.extend(from_extra.iter().cloned());
    assert_place_succession(
        ws,
        session,
        human_id,
        PlaceSuccessionInput {
            from_human_ids,
            to_human_ids: to.clone(),
            kind: *kind,
            date: date.clone().map(build_genealogical_date),
        },
        prov.meta(),
    )
    .await
    .map(|()| human_id.clone())
}

/// Supersedes a source media reference's crop/caption ([`SourceEdit::SetMediaRegion`]), returning the
/// source `human_id` to reload. Takes the whole edit so the caller's match arm stays a one-line
/// delegate (the field-by-field destructure lives here); the `let else` is unreachable in practice.
async fn source_set_region(
    ws: &Workspace,
    session: &Session,
    edit: &SourceEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let SourceEdit::SetMediaRegion {
        human_id,
        assertion_id,
        crop,
        caption,
    } = edit
    else {
        return Ok(String::new());
    };
    let input = MediaRefInput {
        crop: *crop,
        caption: caption.clone(),
    };
    update_source_media_ref(ws, session, human_id, assertion_id, input, prov.meta())
        .await
        .map(|()| human_id.clone())
}

/// Dispatches a [`SourceEdit`] to its `vitni-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected source ([`SourceEdit::target`]) afterwards. Mirrors
/// [`dispatch_event_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_source_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &SourceEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let owner = match corrected_assertion(edit_correction_source(edit), prov) {
        Some(assertion_id) => source_claim_owner(workspace, edit.target(), assertion_id).await?,
        None => edit.target().to_owned(),
    };
    if owner != edit.target() {
        dispatch_source_edit_to(workspace, session, &edit.retargeted(&owner), prov).await?;
        return Ok(edit.target().to_owned());
    }
    dispatch_source_edit_to(workspace, session, edit, prov).await
}

/// The row a source undo or media-region edit names.
fn edit_correction_source(edit: &SourceEdit) -> Option<&str> {
    match edit {
        SourceEdit::SetMediaRegion { assertion_id, .. } | SourceEdit::UndoAssertion { assertion_id, .. } => {
            Some(assertion_id)
        }
        SourceEdit::SetHumanId { .. }
        | SourceEdit::SetTitle { .. }
        | SourceEdit::SetAuthor { .. }
        | SourceEdit::SetPubInfo { .. }
        | SourceEdit::SetAbbrev { .. }
        | SourceEdit::LinkRepository { .. }
        | SourceEdit::AddAttribute { .. }
        | SourceEdit::AttachMedia { .. }
        | SourceEdit::AttachNote { .. }
        | SourceEdit::Tag { .. }
        | SourceEdit::SetRestrictions { .. } => None,
    }
}

/// Dispatches a [`SourceEdit`] to the record it names.
async fn dispatch_source_edit_to(
    workspace: &Workspace,
    session: &Session,
    edit: &SourceEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match edit {
        SourceEdit::SetHumanId { human_id, new_human_id } => {
            set_source_human_id(workspace, session, human_id, new_human_id.clone(), prov.provenance()).await
        }
        SourceEdit::SetTitle { human_id, title } => set_title(workspace, session, human_id, title.clone(), prov.meta())
            .await
            .map(|()| human_id.clone()),
        SourceEdit::SetAuthor { human_id, author } => {
            set_source_author(workspace, session, human_id, author.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        SourceEdit::SetPubInfo { human_id, pub_info } => {
            set_source_pub_info(workspace, session, human_id, pub_info.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        SourceEdit::SetAbbrev { human_id, abbrev } => {
            set_source_abbrev(workspace, session, human_id, abbrev.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        SourceEdit::LinkRepository {
            human_id,
            repository_id,
            call_number,
            media_type,
        } => link_source_repository(
            workspace,
            session,
            human_id,
            repository_id,
            call_number.clone(),
            media_type.clone(),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        SourceEdit::AddAttribute {
            human_id,
            attribute_type,
            value,
        } => add_source_attribute(
            workspace,
            session,
            human_id,
            attribute_type.clone(),
            value.clone(),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        SourceEdit::AttachMedia { human_id, media_id } => {
            import_attach_source_media(workspace, session, human_id, media_id)
                .await
                .map(|()| human_id.clone())
        }
        SourceEdit::SetMediaRegion { .. } => source_set_region(workspace, session, edit, prov).await,
        SourceEdit::AttachNote { human_id, note_id } => {
            import_attach_source_note(workspace, session, human_id, note_id)
                .await
                .map(|()| human_id.clone())
        }
        SourceEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_source(workspace, session, human_id, tag_id, *remove, prov.meta())
            .await
            .map(|()| human_id.clone()),
        SourceEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_source_restrictions(workspace, session, human_id, restrictions, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        SourceEdit::UndoAssertion { human_id, assertion_id } => {
            undo_source_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale)
                .await
                .map(|()| human_id.clone())
        }
    }
}

/// Dispatches a [`RepositoryEdit`] to its `vitni-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected repository ([`RepositoryEdit::target`]) afterwards. Note that
/// `LinkSource` emits a `LinkRepository` command against the *source*, with this repository as the
/// target. Mirrors [`dispatch_source_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_repository_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &RepositoryEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let owner = match corrected_assertion(edit_correction_repository(edit), prov) {
        Some(assertion_id) => repository_claim_owner(workspace, edit.target(), assertion_id).await?,
        None => edit.target().to_owned(),
    };
    if owner != edit.target() {
        dispatch_repository_edit_to(workspace, session, &edit.retargeted(&owner), prov).await?;
        return Ok(edit.target().to_owned());
    }
    dispatch_repository_edit_to(workspace, session, edit, prov).await
}

/// The row a repository undo or media-region edit names.
fn edit_correction_repository(edit: &RepositoryEdit) -> Option<&str> {
    match edit {
        RepositoryEdit::UndoAssertion { assertion_id, .. } => Some(assertion_id),
        RepositoryEdit::SetHumanId { .. }
        | RepositoryEdit::SetName { .. }
        | RepositoryEdit::SetType { .. }
        | RepositoryEdit::AddAddress { .. }
        | RepositoryEdit::AddUrl { .. }
        | RepositoryEdit::LinkSource { .. }
        | RepositoryEdit::AttachNote { .. }
        | RepositoryEdit::Tag { .. }
        | RepositoryEdit::SetRestrictions { .. } => None,
    }
}

/// Dispatches a [`RepositoryEdit`] to the record it names.
async fn dispatch_repository_edit_to(
    workspace: &Workspace,
    session: &Session,
    edit: &RepositoryEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match edit {
        RepositoryEdit::SetHumanId { human_id, new_human_id } => {
            set_repository_human_id(workspace, session, human_id, new_human_id.clone(), prov.provenance()).await
        }
        RepositoryEdit::SetName { human_id, name } => {
            set_repository_name(workspace, session, human_id, name.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        RepositoryEdit::SetType {
            human_id,
            repository_type,
        } => set_repository_type(workspace, session, human_id, repository_type.clone(), prov.meta())
            .await
            .map(|()| human_id.clone()),
        RepositoryEdit::AddAddress { human_id, address } => {
            add_repository_address(workspace, session, human_id, address.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        RepositoryEdit::AddUrl { human_id, url } => {
            add_repository_url(workspace, session, human_id, url.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        RepositoryEdit::LinkSource {
            human_id,
            source_id,
            call_number,
            media_type,
        } => link_source_repository(
            workspace,
            session,
            source_id,
            human_id,
            call_number.clone(),
            media_type.clone(),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        RepositoryEdit::AttachNote { human_id, note_id } => {
            import_attach_repository_note(workspace, session, human_id, note_id)
                .await
                .map(|()| human_id.clone())
        }
        RepositoryEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_repository(workspace, session, human_id, tag_id, *remove, prov.meta())
            .await
            .map(|()| human_id.clone()),
        RepositoryEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_repository_restrictions(workspace, session, human_id, restrictions, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        RepositoryEdit::UndoAssertion { human_id, assertion_id } => {
            undo_repository_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale)
                .await
                .map(|()| human_id.clone())
        }
    }
}

/// Dispatches a [`MediaEdit`] to its `vitni-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected media object ([`MediaEdit::target`]) afterwards. Mirrors
/// [`dispatch_source_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_media_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &MediaEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let owner = match corrected_assertion(edit_correction_media(edit), prov) {
        Some(assertion_id) => media_claim_owner(workspace, edit.target(), assertion_id).await?,
        None => edit.target().to_owned(),
    };
    if owner != edit.target() {
        dispatch_media_edit_to(workspace, session, &edit.retargeted(&owner), prov).await?;
        return Ok(edit.target().to_owned());
    }
    dispatch_media_edit_to(workspace, session, edit, prov).await
}

/// The row a media undo or media-region edit names.
fn edit_correction_media(edit: &MediaEdit) -> Option<&str> {
    match edit {
        MediaEdit::UndoAssertion { assertion_id, .. } => Some(assertion_id),
        MediaEdit::SetHumanId { .. }
        | MediaEdit::SetFilePath { .. }
        | MediaEdit::SetWebPath { .. }
        | MediaEdit::SetMime { .. }
        | MediaEdit::SetDate { .. }
        | MediaEdit::AddAttribute { .. }
        | MediaEdit::AttachCitation { .. }
        | MediaEdit::AttachNote { .. }
        | MediaEdit::Tag { .. }
        | MediaEdit::SetRestrictions { .. } => None,
    }
}

/// Dispatches a [`MediaEdit`] to the record it names.
async fn dispatch_media_edit_to(
    workspace: &Workspace,
    session: &Session,
    edit: &MediaEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match edit {
        MediaEdit::SetHumanId { human_id, new_human_id } => {
            set_media_human_id(workspace, session, human_id, new_human_id.clone(), prov.provenance()).await
        }
        MediaEdit::SetFilePath { human_id, path } => {
            set_media_file_path(workspace, session, human_id, path.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        MediaEdit::SetWebPath { human_id, href } => {
            set_media_web_path(workspace, session, human_id, href.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        MediaEdit::SetMime { human_id, mime } => {
            set_media_mime(workspace, session, human_id, mime.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        MediaEdit::SetDate { human_id, date } => assert_media_date_value(
            workspace,
            session,
            human_id,
            build_genealogical_date(date.clone()),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        MediaEdit::AddAttribute {
            human_id,
            attribute_type,
            value,
        } => add_media_attribute(
            workspace,
            session,
            human_id,
            attribute_type.clone(),
            value.clone(),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        MediaEdit::AttachCitation { human_id, citation_id } => {
            add_media_citation(workspace, session, human_id, citation_id, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        MediaEdit::AttachNote { human_id, note_id } => import_attach_media_note(workspace, session, human_id, note_id)
            .await
            .map(|()| human_id.clone()),
        MediaEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_media(workspace, session, human_id, tag_id, *remove, prov.meta())
            .await
            .map(|()| human_id.clone()),
        MediaEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_media_restrictions(workspace, session, human_id, restrictions, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        MediaEdit::UndoAssertion { human_id, assertion_id } => {
            undo_media_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale)
                .await
                .map(|()| human_id.clone())
        }
    }
}

/// Dispatches a [`NoteEdit`] to its `vitni-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected note ([`NoteEdit::target`]) afterwards. Mirrors
/// [`dispatch_source_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_note_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &NoteEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let owner = match corrected_assertion(edit_correction_note(edit), prov) {
        Some(assertion_id) => note_claim_owner(workspace, edit.target(), assertion_id).await?,
        None => edit.target().to_owned(),
    };
    if owner != edit.target() {
        dispatch_note_edit_to(workspace, session, &edit.retargeted(&owner), prov).await?;
        return Ok(edit.target().to_owned());
    }
    dispatch_note_edit_to(workspace, session, edit, prov).await
}

/// The row a note undo or media-region edit names.
fn edit_correction_note(edit: &NoteEdit) -> Option<&str> {
    match edit {
        NoteEdit::UndoAssertion { assertion_id, .. } => Some(assertion_id),
        NoteEdit::SetHumanId { .. }
        | NoteEdit::SetType { .. }
        | NoteEdit::SetText { .. }
        | NoteEdit::AddTranslation { .. }
        | NoteEdit::Tag { .. }
        | NoteEdit::SetRestrictions { .. } => None,
    }
}

/// Dispatches a [`NoteEdit`] to the record it names.
async fn dispatch_note_edit_to(
    workspace: &Workspace,
    session: &Session,
    edit: &NoteEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match edit {
        NoteEdit::SetHumanId { human_id, new_human_id } => {
            set_note_human_id(workspace, session, human_id, new_human_id.clone(), prov.provenance()).await
        }
        NoteEdit::SetType { human_id, note_type } => {
            set_note_type(workspace, session, human_id, note_type.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        NoteEdit::SetText {
            human_id,
            text,
            language,
        } => set_note_text(
            workspace,
            session,
            human_id,
            text.clone(),
            language.clone(),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        NoteEdit::AddTranslation {
            human_id,
            language,
            text,
            translator,
        } => add_note_translation(
            workspace,
            session,
            human_id,
            language.clone(),
            text.clone(),
            translator.clone(),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        NoteEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_note(workspace, session, human_id, tag_id, *remove, prov.meta())
            .await
            .map(|()| human_id.clone()),
        NoteEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_note_restrictions(workspace, session, human_id, restrictions, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        NoteEdit::UndoAssertion { human_id, assertion_id } => {
            undo_note_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale)
                .await
                .map(|()| human_id.clone())
        }
    }
}

/// Dispatches a [`ResearchNoteEdit`] to its `vitni-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected research note ([`ResearchNoteEdit::target`]) afterwards. Mirrors
/// [`dispatch_note_edit`]; a research note has no rename, so every arm returns the unchanged id.
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_research_note_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &ResearchNoteEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match edit {
        ResearchNoteEdit::SetBody {
            human_id,
            text,
            language,
        } => set_research_note_body(
            workspace,
            session,
            human_id,
            text.clone(),
            language.clone(),
            prov.meta(),
        )
        .await
        .map(|()| human_id.clone()),
        ResearchNoteEdit::AddSubject { human_id, subject } => {
            add_subject_to_research_note(workspace, session, human_id, subject_input(subject)?, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        ResearchNoteEdit::RemoveSubject { human_id, subject } => {
            remove_subject_from_research_note(workspace, session, human_id, subject_input(subject)?, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        ResearchNoteEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_research_note(workspace, session, human_id, tag_id, *remove, prov.meta())
            .await
            .map(|()| human_id.clone()),
        ResearchNoteEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_research_note_restrictions(workspace, session, human_id, restrictions, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        ResearchNoteEdit::UndoAssertion { human_id, assertion_id } => {
            undo_research_note_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale)
                .await
                .map(|()| human_id.clone())
        }
    }
}

/// Commits a [`ResearchNoteChangeSetRequest`] (the buffered research-note create form), returning the
/// new note's `human_id`.
///
/// The aggregate has no change-set use-case: the create carries the id, subjects, and title, and a
/// non-blank body is asserted afterwards (sequenced, non-atomic — the same shape as the media create's
/// follow-up date assert).
///
/// # Errors
///
/// Propagates the [`AppError`] from `create_research_note` (an unknown subject, an empty subject set,
/// a taken id, a database failure) or from the follow-up body assert.
pub async fn dispatch_research_note_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &ResearchNoteChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let mut subjects = Vec::with_capacity(request.subjects.len());
    for subject in &request.subjects {
        subjects.push(subject_input(subject)?);
    }
    let human_id = create_research_note(
        workspace,
        session,
        NewResearchNote {
            human_id: request.human_id.clone(),
            subjects,
            title: request.title.clone(),
        },
        prov.provenance(),
        &prov.citations,
    )
    .await?;
    if let Some(body) = request.body.clone() {
        set_research_note_body(
            workspace,
            session,
            &human_id,
            body,
            request.language.clone(),
            prov.meta(),
        )
        .await?;
    }
    Ok(human_id)
}

/// Commits a [`TagChangeSetRequest`] (the buffered tag record) through [`commit_tag_change_set`],
/// returning the tag's aggregate id (the minted one on create).
///
/// Maps the UI-side record to the app-layer [`TagChangeSet`]; the app validates every field is
/// present, mints an id on create, and emits only the changed fields on edit. Dispatched only when
/// the operator presses Save — Cancel never reaches here.
///
/// # Errors
///
/// Propagates the [`AppError`] from `commit_tag_change_set` (a domain rejection such as an empty
/// name/colour, an unknown tag on edit, or a database failure).
pub async fn dispatch_tag_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &TagChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let target = match &request.existing_id {
        Some(id) => TagTarget::Existing { id: id.clone() },
        None => TagTarget::New,
    };
    let restrictions: BTreeSet<Restriction> = request
        .restrictions
        .iter()
        .map(|&kind| Restriction::from(kind))
        .collect();
    commit_tag_change_set(
        workspace,
        session,
        TagChangeSet {
            target,
            name: request.name.clone(),
            priority: request.priority,
            color: request.color.clone(),
            restrictions,
            provenance: prov.provenance(),
            citations: prov.citations.clone(),
        },
    )
    .await
}

/// Commits a [`SourceChangeSetRequest`] (the buffered source create form) through
/// [`commit_source_change_set`], returning the new source's `human_id`. Dispatched only on Save;
/// Cancel never reaches here. The provenance block rides on the change-set (`record-editing.html`
/// §5b).
///
/// # Errors
///
/// Propagates the [`AppError`] from `commit_source_change_set` (a domain rejection, an unknown
/// backing citation, or a database failure).
pub async fn dispatch_source_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &SourceChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    commit_source_change_set(
        workspace,
        session,
        SourceChangeSet {
            human_id: request.human_id.clone(),
            title: request.title.clone(),
            author: request.author.clone(),
            publication: request.publication.clone(),
            abbreviation: request.abbreviation.clone(),
            provenance: prov.provenance(),
            citations: prov.citations.clone(),
        },
    )
    .await
}

/// Commits a [`RepositoryChangeSetRequest`] (the buffered repository create form) through
/// [`commit_repository_change_set`], returning the new repository's `human_id`.
///
/// # Errors
///
/// Propagates the [`AppError`] from `commit_repository_change_set`.
pub async fn dispatch_repository_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &RepositoryChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    commit_repository_change_set(
        workspace,
        session,
        RepositoryChangeSet {
            human_id: request.human_id.clone(),
            repository_type: request.repository_type.clone(),
            name: request.name.clone(),
            provenance: prov.provenance(),
            citations: prov.citations.clone(),
        },
    )
    .await
}

/// Commits a [`NoteChangeSetRequest`] (the buffered note create form) through
/// [`commit_note_change_set`], returning the new note's `human_id`.
///
/// # Errors
///
/// Propagates the [`AppError`] from `commit_note_change_set`.
pub async fn dispatch_note_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &NoteChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    commit_note_change_set(
        workspace,
        session,
        NoteChangeSet {
            human_id: request.human_id.clone(),
            note_type: request.note_type.clone(),
            text: request.text.clone(),
            language: request.language.clone(),
            provenance: prov.provenance(),
            citations: prov.citations.clone(),
        },
    )
    .await
}

/// Commits a [`MediaChangeSetRequest`] (the buffered media create form) through
/// [`commit_media_change_set`], returning the new media object's `human_id`. When the request carries
/// a date it is asserted after the create commits (sequenced, non-atomic).
///
/// # Errors
///
/// Propagates the [`AppError`] from `commit_media_change_set` or the follow-up date assert.
pub async fn dispatch_media_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &MediaChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let human_id = commit_media_change_set(
        workspace,
        session,
        MediaChangeSet {
            human_id: request.human_id.clone(),
            file_path: request.file_path.clone(),
            web_path: request.web_path.clone(),
            mime: request.mime.clone(),
            provenance: prov.provenance(),
            citations: prov.citations.clone(),
        },
    )
    .await?;
    // Sequenced, non-atomic (accepted): the create commits first, then the date is asserted. A failed
    // date-assert leaves the media without a date, settable afterwards in edit mode.
    if let Some(date) = &request.date {
        assert_media_date_value(
            workspace,
            session,
            &human_id,
            build_genealogical_date(date.clone()),
            prov.meta(),
        )
        .await?;
    }
    Ok(human_id)
}

/// Commits a [`DnaMatchChangeSetRequest`] (the buffered DNA-match create form) through
/// [`observe_dna_match`], returning the new match's `human_id`. The numeric fields arrive already
/// parsed — an unparseable value never reaches here (§7).
///
/// # Errors
///
/// Propagates the [`AppError`] from `observe_dna_match` (an unknown test, a domain rejection, or a
/// database failure).
pub async fn dispatch_dna_match_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &DnaMatchChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    observe_dna_match(
        workspace,
        session,
        NewDnaMatch {
            human_id: None,
            test_a: request.test_a.clone(),
            test_b: request.test_b.clone(),
            provider: request.provider.clone(),
            shared_cm: request.shared_cm,
            percent_shared: request.percent_shared,
            segment_count: request.segment_count,
            largest_segment_cm: request.largest_segment_cm,
            predicted_relationship: request.predicted_relationship.clone(),
        },
        prov.provenance(),
        &prov.citations,
    )
    .await
}

/// Commits a [`CitationChangeSetRequest`] (the buffered citation create form) through
/// [`commit_citation_change_set`], returning the new citation's `human_id`. A "new source" selection
/// becomes a pending source created inline (a §6b cascade). When the request carries a date it is
/// asserted after the create commits (sequenced, non-atomic).
///
/// # Errors
///
/// Propagates the [`AppError`] from `commit_citation_change_set` (an unknown source, a domain
/// rejection, or a database failure).
pub async fn dispatch_citation_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &CitationChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let (source, new_sources) = match &request.source {
        CitationSourceRequest::Existing(human_id) => (SourceRefInput::Existing(human_id.clone()), Vec::new()),
        CitationSourceRequest::New { title } => {
            let placeholder = PlaceholderRef("citation-source".to_owned());
            (
                SourceRefInput::Pending(placeholder.clone()),
                vec![NewSourceEntry {
                    placeholder,
                    title: title.clone(),
                }],
            )
        }
    };
    let human_id = commit_citation_change_set(
        workspace,
        session,
        CitationChangeSet {
            human_id: None,
            source,
            page: request.page.clone(),
            confidence: request.confidence.map(Into::into),
            evidence: request.evidence,
            new_sources,
            provenance: prov.provenance(),
            citations: prov.citations.clone(),
        },
    )
    .await?;
    // Sequenced, non-atomic (accepted): the create commits first, then the cited-record date is
    // asserted. A failed date-assert leaves the citation without a date, settable in edit mode.
    if let Some(date) = &request.date {
        assert_citation_date_value(
            workspace,
            session,
            &human_id,
            build_genealogical_date(date.clone()),
            prov.meta(),
        )
        .await?;
    }
    Ok(human_id)
}

/// Commits an [`EventChangeSetRequest`] (the buffered event create form) through
/// [`commit_event_change_set`], returning the new event's `human_id`. A "new place" selection becomes
/// a pending place created inline (a §6b cascade). When the request carries a date it is asserted
/// after the create commits (sequenced, non-atomic).
///
/// # Errors
///
/// Propagates the [`AppError`] from `commit_event_change_set` (an unknown place, a domain rejection,
/// or a database failure).
pub async fn dispatch_event_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &EventChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    let (place, new_places) = match &request.place {
        EventPlaceRequest::None => (None, Vec::new()),
        EventPlaceRequest::Existing(human_id) => (Some(PlaceRefInput::Existing(human_id.clone())), Vec::new()),
        EventPlaceRequest::New { place_type, name } => {
            let placeholder = PlaceholderRef("event-place".to_owned());
            (
                Some(PlaceRefInput::Pending(placeholder.clone())),
                vec![NewPlaceEntry {
                    placeholder,
                    place_type: place_type.clone(),
                    name: name.clone(),
                }],
            )
        }
    };
    let human_id = commit_event_change_set(
        workspace,
        session,
        EventChangeSet {
            human_id: None,
            event_type: request.event_type.clone(),
            description: request.description.clone(),
            place,
            new_places,
            provenance: prov.provenance(),
            citations: prov.citations.clone(),
        },
    )
    .await?;
    // Sequenced, non-atomic (accepted): the create commits first, then the date is asserted. A failed
    // date-assert leaves the event without a date, settable afterwards in edit mode.
    if let Some(date) = &request.date {
        assert_event_date_value(
            workspace,
            session,
            &human_id,
            build_genealogical_date(date.clone()),
            prov.meta(),
        )
        .await?;
    }
    Ok(human_id)
}

/// Commits a [`DnaTestChangeSetRequest`] (the buffered DNA-test create form) through
/// [`commit_dna_test_change_set`], returning the new test's `human_id`.
///
/// # Errors
///
/// Propagates the [`AppError`] from `commit_dna_test_change_set` (an unknown person, a domain
/// rejection, or a database failure).
pub async fn dispatch_dna_test_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &DnaTestChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    commit_dna_test_change_set(
        workspace,
        session,
        DnaTestChangeSet {
            human_id: None,
            person: request.person.clone(),
            provider: request.provider.clone(),
            test_type: request.test_type,
            genome_build: request.genome_build,
            kit_id: request.kit_id.clone(),
            provenance: prov.provenance(),
            citations: prov.citations.clone(),
        },
    )
    .await
}

/// Commits a [`FamilyChangeSetRequest`] (the buffered family create form) through
/// [`commit_family_change_set`], returning the new family's `human_id`.
///
/// # Errors
///
/// Propagates the [`AppError`] from `commit_family_change_set` (an unknown partner, a domain
/// rejection, or a database failure).
pub async fn dispatch_family_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &FamilyChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    commit_family_change_set(
        workspace,
        session,
        FamilyChangeSet {
            human_id: request.human_id.clone(),
            partners: request.partners.iter().map(partner_input).collect(),
            provenance: prov.provenance(),
            citations: prov.citations.clone(),
        },
    )
    .await
}

/// Maps a UI [`PartnerRequest`] to the app's [`PartnerInput`].
fn partner_input(request: &PartnerRequest) -> PartnerInput {
    match request {
        PartnerRequest::Existing(human_id) => PartnerInput::Existing(human_id.clone()),
        PartnerRequest::New { given, surname, birth } => PartnerInput::New {
            given: given.clone(),
            surname: surname.clone(),
            birth: *birth,
        },
    }
}

/// Commits a [`PlaceChangeSetRequest`] (the buffered place create form) through
/// [`commit_place_change_set`], returning the new place's `human_id`.
///
/// # Errors
///
/// Propagates the [`AppError`] from `commit_place_change_set`.
pub async fn dispatch_place_change_set(
    workspace: &Workspace,
    session: &Session,
    request: &PlaceChangeSetRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    commit_place_change_set(
        workspace,
        session,
        PlaceChangeSet {
            human_id: request.human_id.clone(),
            place_type: request.place_type.clone(),
            name: request.name.clone(),
            coordinates: request.coordinates,
            code: request.code.clone(),
            provenance: prov.provenance(),
            citations: prov.citations.clone(),
        },
    )
    .await
}

/// Dispatches a [`NewRecordRequest`] — the validated half of an attach picker's "+ New …" draft
/// (issue #314) — to whichever of the eight `dispatch_*_change_set` fns matches, returning the new
/// record's `human_id`. Deliberately not a new `vitni-app` use-case: the change-set placeholder route
/// (`new_sources`/`new_citations`, `commit_pending_sources_and_citations`) buys no atomicity of its
/// own — every `commit_*_change_set` that takes a follow-up date assert is already "sequenced,
/// non-atomic (accepted)" (see `dispatch_media_change_set` above) — so funnelling through the eight
/// existing dispatch fns costs nothing a dedicated app-layer entry point would have bought.
///
/// # Errors
///
/// Propagates the [`AppError`] from whichever `dispatch_*_change_set` fn the request maps to.
pub async fn dispatch_new_record(
    workspace: &Workspace,
    session: &Session,
    request: &NewRecordRequest,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match request {
        NewRecordRequest::Person(request) => dispatch_person_change_set(workspace, session, request, prov).await,
        NewRecordRequest::Place(request) => dispatch_place_change_set(workspace, session, request, prov).await,
        NewRecordRequest::Source(request) => dispatch_source_change_set(workspace, session, request, prov).await,
        NewRecordRequest::Citation(request) => dispatch_citation_change_set(workspace, session, request, prov).await,
        NewRecordRequest::Note(request) => dispatch_note_change_set(workspace, session, request, prov).await,
        NewRecordRequest::Media(request) => dispatch_media_change_set(workspace, session, request, prov).await,
        NewRecordRequest::Event(request) => dispatch_event_change_set(workspace, session, request, prov).await,
        NewRecordRequest::Repository(request) => {
            dispatch_repository_change_set(workspace, session, request, prov).await
        }
    }
}

/// Dispatches a [`DnaTestEdit`] to its `vitni-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected test ([`DnaTestEdit::target`]) afterwards.
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_dna_test_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &DnaTestEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match edit {
        DnaTestEdit::SetHumanId { human_id, new_human_id } => {
            set_dna_test_human_id(workspace, session, human_id, new_human_id.clone(), prov.provenance()).await
        }
        DnaTestEdit::SetProvider { human_id, provider } => {
            set_dna_test_provider(workspace, session, human_id, provider.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        DnaTestEdit::SetKitId { human_id, kit_id } => {
            set_dna_test_kit_id(workspace, session, human_id, kit_id.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        DnaTestEdit::SetType { human_id, test_type } => {
            set_dna_test_type(workspace, session, human_id, *test_type, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        DnaTestEdit::SetGenomeBuild { human_id, genome_build } => {
            set_dna_test_genome_build(workspace, session, human_id, *genome_build, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        DnaTestEdit::AddHaplogroup { human_id, haplogroup } => {
            assert_dna_test_haplogroup(workspace, session, human_id, haplogroup.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        DnaTestEdit::AttachNote { human_id, note_id } => {
            import_attach_dna_test_note(workspace, session, human_id, note_id)
                .await
                .map(|()| human_id.clone())
        }
        DnaTestEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_dna_test(workspace, session, human_id, tag_id, *remove, prov.meta())
            .await
            .map(|()| human_id.clone()),
        DnaTestEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_dna_test_restrictions(workspace, session, human_id, restrictions, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        DnaTestEdit::UndoAssertion { human_id, assertion_id } => {
            undo_dna_test_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale)
                .await
                .map(|()| human_id.clone())
        }
    }
}

/// Dispatches a [`DnaMatchEdit`] to its `vitni-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected match ([`DnaMatchEdit::target`]) afterwards.
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_dna_match_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &DnaMatchEdit,
    prov: &ProvenanceDraft,
) -> Result<String, AppError> {
    match edit {
        DnaMatchEdit::SetHumanId { human_id, new_human_id } => {
            set_dna_match_human_id(workspace, session, human_id, new_human_id.clone(), prov.provenance()).await
        }
        DnaMatchEdit::SetStatus { human_id, confirmed } => {
            set_dna_match_status(workspace, session, human_id, *confirmed, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        DnaMatchEdit::AddSegment { human_id, segment } => {
            add_dna_match_segment(workspace, session, human_id, segment.clone(), prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        DnaMatchEdit::AssertSharedAncestor {
            human_id,
            person_id,
            note,
        } => {
            let ancestor = build_shared_ancestor(person_id.as_deref(), note.clone())?;
            assert_dna_match_shared_ancestor(workspace, session, human_id, ancestor, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        DnaMatchEdit::AttachNote { human_id, note_id } => {
            import_attach_dna_match_note(workspace, session, human_id, note_id)
                .await
                .map(|()| human_id.clone())
        }
        DnaMatchEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_dna_match(workspace, session, human_id, tag_id, *remove, prov.meta())
            .await
            .map(|()| human_id.clone()),
        DnaMatchEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_dna_match_restrictions(workspace, session, human_id, restrictions, prov.meta())
                .await
                .map(|()| human_id.clone())
        }
        DnaMatchEdit::UndoAssertion { human_id, assertion_id } => {
            undo_dna_match_assertion(workspace, session, human_id, assertion_id, prov.provenance().rationale)
                .await
                .map(|()| human_id.clone())
        }
    }
}

/// Records the operator's approved-capability decision for plugin `id` into the workspace manifest
/// (ADR 0014 §5), the effective grant the host intersects with the plugin's declared capabilities.
/// `approved` is the full set to persist — an empty set records "deny everything declared".
///
/// # Errors
///
/// Propagates the [`AppError`] from [`vitni_app::save_plugin_grants`] (a missing or unwritable
/// manifest).
pub fn approve_plugin_grants(
    workspace_dir: &std::path::Path,
    id: &str,
    approved: &BTreeSet<String>,
) -> Result<(), AppError> {
    vitni_app::save_plugin_grants(workspace_dir, id, approved)
}

/// Pins `publisher`'s ed25519 public key (64 hex characters) into the client-scope trust store
/// (ADR 0014 §3), extending user-trust to that publisher's signed bundles.
///
/// # Errors
///
/// Propagates the [`AppError`] from [`vitni_app::add_trusted_publisher`] (a malformed key, or an
/// unreadable/unwritable config).
pub fn pin_publisher(config_path: &std::path::Path, publisher: &str, public_key_hex: &str) -> Result<(), AppError> {
    vitni_app::add_trusted_publisher(config_path, publisher, public_key_hex)
}

/// Unpins `publisher` from the client-scope trust store (ADR 0014 §3).
///
/// # Errors
///
/// Propagates the [`AppError`] from [`vitni_app::remove_trusted_publisher`] (the publisher is not
/// pinned, or the config is unreadable/unwritable).
pub fn unpin_publisher(config_path: &std::path::Path, publisher: &str) -> Result<(), AppError> {
    vitni_app::remove_trusted_publisher(config_path, publisher)
}
