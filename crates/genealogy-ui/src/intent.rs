//! Intent dispatch: turns a data-loading [`Intent`] into a `genealogy-app` use-case call and returns
//! render-ready view-models.
//!
//! This is the only place the presentation layer touches the application's use-cases. It is async
//! because the use-cases are; a renderer awaits it on its own runtime.

use std::collections::BTreeSet;

use genealogy_app::{
    AppError, ChildParentRelationship, EvidenceLevel, NewFact, NewPerson, PersonNameParts, Provenance, Restriction,
    Session, Sex, Workspace, add_child, add_citation_attribute, add_event_citation, add_media_citation, add_name,
    add_note_translation, add_partner, add_person_citation, add_place_citation, add_place_name, add_repository_address,
    add_repository_url, add_source_attribute, assert_association, assert_citation_date, assert_fact,
    assert_place_enclosed_by, assert_sex, attach_citation_media, attach_citation_note, attach_family_media,
    attach_family_note, attach_person_media, attach_person_note, change_log_for_citation, change_log_for_event,
    change_log_for_family, change_log_for_media, change_log_for_note, change_log_for_person, change_log_for_place,
    change_log_for_repository, change_log_for_source, create_person, families_for_person, import_attach_event_media,
    import_attach_event_note, import_attach_media_note, import_attach_place_media, import_attach_place_note,
    import_attach_repository_note, import_attach_source_media, import_attach_source_note, link_family_event,
    link_source_repository, list_citations, list_events, list_families, list_media, list_notes, list_persons,
    list_places, list_repositories, list_sources, recent_activity, set_citation_confidence,
    set_citation_evidence_analysis, set_citation_restrictions, set_event_restrictions, set_family_restrictions,
    set_media_restrictions, set_note_restrictions, set_note_text, set_note_type, set_page, set_participant_role,
    set_place_restrictions, set_repository_restrictions, set_restrictions, set_source_restrictions, show_citation,
    show_event, show_family, show_media, show_note, show_person, show_place, show_repository, show_source,
    tag_citation, tag_event, tag_family, tag_media, tag_note, tag_place, tag_repository, tag_source, undo_assertion,
    undo_citation_assertion, undo_event_assertion, undo_family_assertion, undo_media_assertion, undo_note_assertion,
    undo_place_assertion, undo_repository_assertion, undo_source_assertion, workspace_counts,
};
use genealogy_app::{ancestors, descendants, find_duplicate_candidates, merge_persons, relationship};
use genealogy_app::{
    assert_dna_test_haplogroup, change_log_for_dna_match, change_log_for_dna_test, change_log_for_tag,
    import_attach_dna_match_note, import_attach_dna_test_note, list_dna_matches, list_dna_tests, list_tags, rename_tag,
    set_dna_match_restrictions, set_dna_match_status, set_dna_test_restrictions, set_tag_color, set_tag_priority,
    show_dna_match, show_dna_test, show_tag, tag_dna_match, tag_dna_test, undo_dna_match_assertion,
    undo_dna_test_assertion,
};

use genealogy_app::{
    assert_event_date, assert_media_date, assert_place_coordinates, set_dna_test_genome_build, set_dna_test_kit_id,
    set_dna_test_provider, set_dna_test_type, set_event_description, set_event_type, set_media_checksum,
    set_media_file_path, set_media_web_path, set_place_code, set_place_type, set_repository_name, set_repository_type,
    set_source_abbrev, set_source_author, set_source_pub_info,
};

use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::navigation::{
    Category, CitationEdit, DnaMatchEdit, DnaTestEdit, EventEdit, FamilyEdit, Intent, MediaEdit, MergePersons,
    NoteEdit, PersonEdit, PlaceEdit, RepositoryEdit, SourceEdit, TagEdit,
};
use crate::view_model::{
    CitationDetail, DashboardVm, DnaMatchDetail, DnaTestDetail, DuplicateCandidateVm, EventDetail, FamilyDetail,
    FamilyVm, MediaDetail, MergeCompareVm, MergeResultVm, NoteDetail, PedigreeVm, PersonDetail, PlaceDetail,
    RelationshipVm, RepositoryDetail, SourceDetail, TagDetail, citation_row, collapse_history, dna_match_row,
    dna_test_row, event_row, family_row, media_row, note_row, person_row, place_row, repository_row, source_row,
    tag_row,
};

/// How many recent changes the dashboard activity feed shows.
const ACTIVITY_LIMIT: u32 = 12;

/// How many quick entry points "Jump back in" shows.
const JUMP_BACK_LIMIT: usize = 4;

/// The data a dispatched [`Intent`] produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentOutcome {
    /// The workspace dashboard.
    Dashboard(Box<DashboardVm>),
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
    /// The Merge tool's possible-duplicates table.
    DuplicateCandidates(Vec<DuplicateCandidateVm>),
    /// The Merge tool's compare/merge wizard, loaded for a chosen pair.
    MergeCompare(Box<MergeCompareVm>),
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
        Intent::ShowDashboard => {
            let counts = workspace_counts(workspace).await?;
            let persons = list_persons(workspace).await?;
            let activity = recent_activity(workspace, ACTIVITY_LIMIT).await?;
            let dashboard = DashboardVm::build(counts, &persons, &activity, loc, JUMP_BACK_LIMIT);
            Ok(IntentOutcome::Dashboard(Box::new(dashboard)))
        }
        Intent::ShowList => {
            let summaries = list_persons(workspace).await?;
            let mut rows = Vec::with_capacity(summaries.len());
            for summary in &summaries {
                rows.push(person_row(summary, loc));
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
            let summaries = list_families(workspace).await?;
            let mut rows = Vec::with_capacity(summaries.len());
            for summary in &summaries {
                rows.push(family_row(summary, loc));
            }
            Ok(IntentOutcome::List(rows))
        }
        Intent::ShowFamily { human_id } => match show_family(workspace, human_id).await? {
            Some(summary) => {
                let mut detail = FamilyDetail::from_summary(&summary, loc);
                let change_log = change_log_for_family(workspace, human_id).await?;
                detail.history = collapse_history(&change_log, loc);
                Ok(IntentOutcome::FamilyDetail(Box::new(detail)))
            }
            None => Ok(IntentOutcome::NotFound {
                human_id: human_id.clone(),
            }),
        },
        Intent::ShowEventList => {
            let summaries = list_events(workspace).await?;
            let mut rows = Vec::with_capacity(summaries.len());
            for summary in &summaries {
                rows.push(event_row(summary, loc));
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
        Intent::ListDuplicateCandidates => list_duplicate_candidates(workspace, loc).await,
        Intent::MergeCompare {
            surviving_human_id,
            merged_human_id,
        } => merge_compare(workspace, loc, surviving_human_id, merged_human_id).await,
    }
}

/// Scans the workspace for possible-duplicate person pairs (the Merge tool's landing table).
async fn list_duplicate_candidates(workspace: &Workspace, loc: &Localizer) -> Result<IntentOutcome, AppError> {
    let candidates = find_duplicate_candidates(workspace).await?;
    let vms = candidates
        .iter()
        .map(|candidate| DuplicateCandidateVm::build(candidate, loc))
        .collect();
    Ok(IntentOutcome::DuplicateCandidates(vms))
}

/// Loads both people's summaries for the Merge tool's compare/merge wizard. Like [`show_pedigree`],
/// an unknown `human_id` propagates as an [`AppError`] rather than [`IntentOutcome::NotFound`] — the
/// Merge tool has no per-record detail pane to degrade gracefully into.
async fn merge_compare(
    workspace: &Workspace,
    loc: &Localizer,
    surviving_human_id: &str,
    merged_human_id: &str,
) -> Result<IntentOutcome, AppError> {
    let survivor = show_person(workspace, surviving_human_id)
        .await?
        .ok_or_else(|| AppError::PersonNotFound(surviving_human_id.to_owned()))?;
    let merged = show_person(workspace, merged_human_id)
        .await?
        .ok_or_else(|| AppError::PersonNotFound(merged_human_id.to_owned()))?;
    let vm = MergeCompareVm::build(&survivor, &merged, loc);
    Ok(IntentOutcome::MergeCompare(Box::new(vm)))
}

/// Dispatches a [`MergePersons`] request to `genealogy_app::merge_persons`, mutating the workspace.
///
/// Unlike [`dispatch`] (a read), this emits an event; the renderer bumps its data version to refresh
/// the duplicates list afterwards. Returns the localized [`MergeResultVm`] the screen shows as
/// confirmation.
///
/// # Errors
///
/// Propagates the [`AppError`] from `merge_persons` (either `human_id` not found, a self-merge
/// domain rejection, or a database failure).
pub async fn dispatch_merge(
    workspace: &Workspace,
    session: &Session,
    loc: &Localizer,
    request: &MergePersons,
) -> Result<MergeResultVm, AppError> {
    let result = merge_persons(
        workspace,
        session,
        &request.surviving_human_id,
        &request.merged_human_id,
    )
    .await?;
    Ok(MergeResultVm::build(&result, loc))
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
            let change_log = change_log_for_place(workspace, human_id).await?;
            detail.history = collapse_history(&change_log, loc);
            Ok(IntentOutcome::PlaceDetail(Box::new(detail)))
        }
        None => Ok(IntentOutcome::NotFound {
            human_id: human_id.to_owned(),
        }),
    }
}

/// Creates a person from an optional initial name and sex, returning the assigned `human_id`.
///
/// Emits `CreatePerson` (auto-allocating the id) as a conclusion persona, then `AssertSex` when a
/// sex is given. The renderer opens the new record afterwards.
///
/// # Errors
///
/// Propagates the [`AppError`] from `create_person`/`assert_sex` (e.g. a database failure).
pub async fn dispatch_create(
    workspace: &Workspace,
    session: &Session,
    name: Option<PersonNameParts>,
    sex: Option<Sex>,
) -> Result<String, AppError> {
    let human_id = create_person(
        workspace,
        session,
        NewPerson {
            human_id: None,
            name,
            evidence_level: EvidenceLevel::Conclusion,
        },
    )
    .await?;
    if let Some(sex) = sex {
        assert_sex(workspace, session, &human_id, sex).await?;
    }
    Ok(human_id)
}

/// Dispatches a [`PersonEdit`] to its `genealogy-app` command use-case.
///
/// Unlike [`dispatch`] (a read), this mutates the workspace and is stamped with the session's
/// operator/clock/id. The renderer reloads the affected person ([`PersonEdit::target`]) afterwards.
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_edit(workspace: &Workspace, session: &Session, edit: &PersonEdit) -> Result<(), AppError> {
    match edit {
        PersonEdit::AssertName { human_id, name } => {
            add_name(workspace, session, human_id, name.clone(), Provenance::default(), &[]).await
        }
        PersonEdit::AssertSex { human_id, sex } => assert_sex(workspace, session, human_id, sex.clone()).await,
        PersonEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_restrictions(workspace, session, human_id, restrictions).await
        }
        PersonEdit::AssertFact {
            human_id,
            fact_type,
            value,
            confidence,
            citation,
        } => {
            let new = NewFact {
                fact_type: fact_type.clone(),
                value: value.clone(),
                date: None,
            };
            let provenance = Provenance {
                confidence: (*confidence).into(),
                rationale: None,
            };
            let citations: Vec<String> = citation.iter().cloned().collect();
            assert_fact(workspace, session, human_id, new, provenance, &citations).await
        }
        PersonEdit::AttachCitation { human_id, citation_id } => {
            add_person_citation(workspace, session, human_id, citation_id).await
        }
        PersonEdit::AttachMedia { human_id, media_id } => {
            attach_person_media(workspace, session, human_id, media_id).await
        }
        PersonEdit::AttachNote { human_id, note_id } => attach_person_note(workspace, session, human_id, note_id).await,
        PersonEdit::AssertAssociation {
            human_id,
            other_id,
            role,
        } => assert_association(workspace, session, human_id, other_id, role.clone()).await,
        PersonEdit::UndoAssertion { human_id, assertion_id } => {
            undo_assertion(workspace, session, human_id, assertion_id).await
        }
    }
}

/// Dispatches a [`CitationEdit`] to its `genealogy-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected citation ([`CitationEdit::target`]) afterwards. Mirrors
/// [`dispatch_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_citation_edit(
    workspace: &Workspace,
    session: &Session,
    edit: &CitationEdit,
) -> Result<(), AppError> {
    match edit {
        CitationEdit::SetPage { human_id, page } => set_page(workspace, session, human_id, page.clone()).await,
        CitationEdit::SetDate { human_id, parts } => assert_citation_date(workspace, session, human_id, *parts).await,
        CitationEdit::SetConfidence { human_id, confidence } => {
            set_citation_confidence(workspace, session, human_id, (*confidence).into()).await
        }
        CitationEdit::SetEvidenceAnalysis { human_id, analysis } => {
            set_citation_evidence_analysis(workspace, session, human_id, *analysis).await
        }
        CitationEdit::AddAttribute {
            human_id,
            attribute_type,
            value,
        } => add_citation_attribute(workspace, session, human_id, attribute_type.clone(), value.clone()).await,
        CitationEdit::AttachMedia { human_id, media_id } => {
            attach_citation_media(workspace, session, human_id, media_id, None).await
        }
        CitationEdit::AttachNote { human_id, note_id } => {
            attach_citation_note(workspace, session, human_id, note_id).await
        }
        CitationEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_citation(workspace, session, human_id, tag_id, *remove).await,
        CitationEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_citation_restrictions(workspace, session, human_id, restrictions).await
        }
        CitationEdit::UndoAssertion { human_id, assertion_id } => {
            undo_citation_assertion(workspace, session, human_id, assertion_id).await
        }
    }
}

/// Dispatches a [`FamilyEdit`] to its `genealogy-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected family ([`FamilyEdit::target`]) afterwards. Mirrors
/// [`dispatch_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_family_edit(workspace: &Workspace, session: &Session, edit: &FamilyEdit) -> Result<(), AppError> {
    match edit {
        FamilyEdit::AddPartner { human_id, person_id } => add_partner(workspace, session, human_id, person_id).await,
        FamilyEdit::AddChild {
            human_id,
            person_id,
            relationships,
        } => {
            let relationships: Vec<(String, ChildParentRelationship)> = relationships.clone();
            add_child(workspace, session, human_id, person_id, relationships).await
        }
        FamilyEdit::LinkFamilyEvent { human_id, event_id } => {
            link_family_event(workspace, session, human_id, event_id).await
        }
        FamilyEdit::AttachMedia { human_id, media_id } => {
            attach_family_media(workspace, session, human_id, media_id).await
        }
        FamilyEdit::AttachNote { human_id, note_id } => attach_family_note(workspace, session, human_id, note_id).await,
        FamilyEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_family(workspace, session, human_id, tag_id, *remove).await,
        FamilyEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_family_restrictions(workspace, session, human_id, restrictions).await
        }
        FamilyEdit::UndoAssertion { human_id, assertion_id } => {
            undo_family_assertion(workspace, session, human_id, assertion_id).await
        }
    }
}

/// Dispatches an [`EventEdit`] to its `genealogy-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected event ([`EventEdit::target`]) afterwards. Mirrors
/// [`dispatch_family_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_event_edit(workspace: &Workspace, session: &Session, edit: &EventEdit) -> Result<(), AppError> {
    match edit {
        EventEdit::SetType { human_id, event_type } => {
            set_event_type(workspace, session, human_id, event_type.clone()).await
        }
        EventEdit::SetDate { human_id, date } => assert_event_date(workspace, session, human_id, *date).await,
        EventEdit::SetDescription { human_id, description } => {
            set_event_description(workspace, session, human_id, description.clone()).await
        }
        EventEdit::AddParticipant {
            human_id,
            person_id,
            role,
        } => set_participant_role(workspace, session, human_id, person_id, role.clone(), false).await,
        EventEdit::AttachCitation { human_id, citation_id } => {
            add_event_citation(workspace, session, human_id, citation_id).await
        }
        EventEdit::AttachMedia { human_id, media_id } => {
            import_attach_event_media(workspace, session, human_id, media_id).await
        }
        EventEdit::AttachNote { human_id, note_id } => {
            import_attach_event_note(workspace, session, human_id, note_id).await
        }
        EventEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_event(workspace, session, human_id, tag_id, *remove).await,
        EventEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_event_restrictions(workspace, session, human_id, restrictions).await
        }
        EventEdit::UndoAssertion { human_id, assertion_id } => {
            undo_event_assertion(workspace, session, human_id, assertion_id).await
        }
    }
}

/// Dispatches a [`PlaceEdit`] to its `genealogy-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected place ([`PlaceEdit::target`]) afterwards. Mirrors
/// [`dispatch_family_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_place_edit(workspace: &Workspace, session: &Session, edit: &PlaceEdit) -> Result<(), AppError> {
    match edit {
        PlaceEdit::SetType { human_id, place_type } => {
            set_place_type(workspace, session, human_id, place_type.clone()).await
        }
        PlaceEdit::SetCoordinates { human_id, coordinates } => {
            assert_place_coordinates(workspace, session, human_id, *coordinates).await
        }
        PlaceEdit::SetCode { human_id, code } => set_place_code(workspace, session, human_id, code.clone()).await,
        PlaceEdit::AddName { human_id, text } => add_place_name(workspace, session, human_id, text.clone()).await,
        PlaceEdit::AddEnclosing { human_id, enclosing_id } => {
            assert_place_enclosed_by(workspace, session, human_id, enclosing_id).await
        }
        PlaceEdit::AttachCitation { human_id, citation_id } => {
            add_place_citation(workspace, session, human_id, citation_id).await
        }
        PlaceEdit::AttachMedia { human_id, media_id } => {
            import_attach_place_media(workspace, session, human_id, media_id).await
        }
        PlaceEdit::AttachNote { human_id, note_id } => {
            import_attach_place_note(workspace, session, human_id, note_id).await
        }
        PlaceEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_place(workspace, session, human_id, tag_id, *remove).await,
        PlaceEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_place_restrictions(workspace, session, human_id, restrictions).await
        }
        PlaceEdit::UndoAssertion { human_id, assertion_id } => {
            undo_place_assertion(workspace, session, human_id, assertion_id).await
        }
    }
}

/// Dispatches a [`SourceEdit`] to its `genealogy-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected source ([`SourceEdit::target`]) afterwards. Mirrors
/// [`dispatch_event_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_source_edit(workspace: &Workspace, session: &Session, edit: &SourceEdit) -> Result<(), AppError> {
    match edit {
        SourceEdit::SetAuthor { human_id, author } => {
            set_source_author(workspace, session, human_id, author.clone()).await
        }
        SourceEdit::SetPubInfo { human_id, pub_info } => {
            set_source_pub_info(workspace, session, human_id, pub_info.clone()).await
        }
        SourceEdit::SetAbbrev { human_id, abbrev } => {
            set_source_abbrev(workspace, session, human_id, abbrev.clone()).await
        }
        SourceEdit::LinkRepository {
            human_id,
            repository_id,
            call_number,
            media_type,
        } => {
            link_source_repository(
                workspace,
                session,
                human_id,
                repository_id,
                call_number.clone(),
                media_type.clone(),
            )
            .await
        }
        SourceEdit::AddAttribute {
            human_id,
            attribute_type,
            value,
        } => add_source_attribute(workspace, session, human_id, attribute_type.clone(), value.clone()).await,
        SourceEdit::AttachMedia { human_id, media_id } => {
            import_attach_source_media(workspace, session, human_id, media_id).await
        }
        SourceEdit::AttachNote { human_id, note_id } => {
            import_attach_source_note(workspace, session, human_id, note_id).await
        }
        SourceEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_source(workspace, session, human_id, tag_id, *remove).await,
        SourceEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_source_restrictions(workspace, session, human_id, restrictions).await
        }
        SourceEdit::UndoAssertion { human_id, assertion_id } => {
            undo_source_assertion(workspace, session, human_id, assertion_id).await
        }
    }
}

/// Dispatches a [`RepositoryEdit`] to its `genealogy-app` command use-case, mutating the workspace.
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
) -> Result<(), AppError> {
    match edit {
        RepositoryEdit::SetName { human_id, name } => {
            set_repository_name(workspace, session, human_id, name.clone()).await
        }
        RepositoryEdit::SetType {
            human_id,
            repository_type,
        } => set_repository_type(workspace, session, human_id, repository_type.clone()).await,
        RepositoryEdit::AddAddress { human_id, address } => {
            add_repository_address(workspace, session, human_id, address.clone()).await
        }
        RepositoryEdit::AddUrl { human_id, url } => add_repository_url(workspace, session, human_id, url.clone()).await,
        RepositoryEdit::LinkSource {
            human_id,
            source_id,
            call_number,
            media_type,
        } => {
            link_source_repository(
                workspace,
                session,
                source_id,
                human_id,
                call_number.clone(),
                media_type.clone(),
            )
            .await
        }
        RepositoryEdit::AttachNote { human_id, note_id } => {
            import_attach_repository_note(workspace, session, human_id, note_id).await
        }
        RepositoryEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_repository(workspace, session, human_id, tag_id, *remove).await,
        RepositoryEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_repository_restrictions(workspace, session, human_id, restrictions).await
        }
        RepositoryEdit::UndoAssertion { human_id, assertion_id } => {
            undo_repository_assertion(workspace, session, human_id, assertion_id).await
        }
    }
}

/// Dispatches a [`MediaEdit`] to its `genealogy-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected media object ([`MediaEdit::target`]) afterwards. Mirrors
/// [`dispatch_source_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_media_edit(workspace: &Workspace, session: &Session, edit: &MediaEdit) -> Result<(), AppError> {
    match edit {
        MediaEdit::SetFilePath { human_id, path } => {
            set_media_file_path(workspace, session, human_id, path.clone()).await
        }
        MediaEdit::SetWebPath { human_id, href } => {
            set_media_web_path(workspace, session, human_id, href.clone()).await
        }
        MediaEdit::SetChecksum { human_id, checksum } => {
            set_media_checksum(workspace, session, human_id, checksum.clone()).await
        }
        MediaEdit::SetDate { human_id, date } => assert_media_date(workspace, session, human_id, *date).await,
        MediaEdit::AttachCitation { human_id, citation_id } => {
            add_media_citation(workspace, session, human_id, citation_id).await
        }
        MediaEdit::AttachNote { human_id, note_id } => {
            import_attach_media_note(workspace, session, human_id, note_id).await
        }
        MediaEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_media(workspace, session, human_id, tag_id, *remove).await,
        MediaEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_media_restrictions(workspace, session, human_id, restrictions).await
        }
        MediaEdit::UndoAssertion { human_id, assertion_id } => {
            undo_media_assertion(workspace, session, human_id, assertion_id).await
        }
    }
}

/// Dispatches a [`NoteEdit`] to its `genealogy-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected note ([`NoteEdit::target`]) afterwards. Mirrors
/// [`dispatch_source_edit`].
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_note_edit(workspace: &Workspace, session: &Session, edit: &NoteEdit) -> Result<(), AppError> {
    match edit {
        NoteEdit::SetType { human_id, note_type } => {
            set_note_type(workspace, session, human_id, note_type.clone()).await
        }
        NoteEdit::SetText { human_id, text } => set_note_text(workspace, session, human_id, text.clone()).await,
        NoteEdit::AddTranslation {
            human_id,
            language,
            text,
            translator,
        } => {
            add_note_translation(
                workspace,
                session,
                human_id,
                language.clone(),
                text.clone(),
                translator.clone(),
            )
            .await
        }
        NoteEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_note(workspace, session, human_id, tag_id, *remove).await,
        NoteEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_note_restrictions(workspace, session, human_id, restrictions).await
        }
        NoteEdit::UndoAssertion { human_id, assertion_id } => {
            undo_note_assertion(workspace, session, human_id, assertion_id).await
        }
    }
}

/// Dispatches a [`TagEdit`] to its `genealogy-app` command use-case, mutating the workspace.
///
/// The renderer reloads the affected tag ([`TagEdit::target`]) afterwards.
///
/// # Errors
///
/// Propagates the [`AppError`] from the underlying use-case (not-found, domain rejection, or a
/// database failure).
pub async fn dispatch_tag_edit(workspace: &Workspace, session: &Session, edit: &TagEdit) -> Result<(), AppError> {
    match edit {
        TagEdit::SetName { id, name } => rename_tag(workspace, session, id, name.clone()).await,
        TagEdit::SetPriority { id, priority } => set_tag_priority(workspace, session, id, *priority).await,
        TagEdit::SetColor { id, color } => set_tag_color(workspace, session, id, color.clone()).await,
    }
}

/// Dispatches a [`DnaTestEdit`] to its `genealogy-app` command use-case, mutating the workspace.
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
) -> Result<(), AppError> {
    match edit {
        DnaTestEdit::SetProvider { human_id, provider } => {
            set_dna_test_provider(workspace, session, human_id, provider.clone()).await
        }
        DnaTestEdit::SetKitId { human_id, kit_id } => {
            set_dna_test_kit_id(workspace, session, human_id, kit_id.clone()).await
        }
        DnaTestEdit::SetType { human_id, test_type } => {
            set_dna_test_type(workspace, session, human_id, *test_type).await
        }
        DnaTestEdit::SetGenomeBuild { human_id, genome_build } => {
            set_dna_test_genome_build(workspace, session, human_id, *genome_build).await
        }
        DnaTestEdit::AddHaplogroup { human_id, haplogroup } => {
            assert_dna_test_haplogroup(workspace, session, human_id, haplogroup.clone()).await
        }
        DnaTestEdit::AttachNote { human_id, note_id } => {
            import_attach_dna_test_note(workspace, session, human_id, note_id).await
        }
        DnaTestEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_dna_test(workspace, session, human_id, tag_id, *remove).await,
        DnaTestEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_dna_test_restrictions(workspace, session, human_id, restrictions).await
        }
        DnaTestEdit::UndoAssertion { human_id, assertion_id } => {
            undo_dna_test_assertion(workspace, session, human_id, assertion_id).await
        }
    }
}

/// Dispatches a [`DnaMatchEdit`] to its `genealogy-app` command use-case, mutating the workspace.
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
) -> Result<(), AppError> {
    match edit {
        DnaMatchEdit::SetStatus { human_id, confirmed } => {
            set_dna_match_status(workspace, session, human_id, *confirmed).await
        }
        DnaMatchEdit::AttachNote { human_id, note_id } => {
            import_attach_dna_match_note(workspace, session, human_id, note_id).await
        }
        DnaMatchEdit::Tag {
            human_id,
            tag_id,
            remove,
        } => tag_dna_match(workspace, session, human_id, tag_id, *remove).await,
        DnaMatchEdit::SetRestrictions { human_id, restrictions } => {
            let restrictions: BTreeSet<Restriction> =
                restrictions.iter().map(|&kind| Restriction::from(kind)).collect();
            set_dna_match_restrictions(workspace, session, human_id, restrictions).await
        }
        DnaMatchEdit::UndoAssertion { human_id, assertion_id } => {
            undo_dna_match_assertion(workspace, session, human_id, assertion_id).await
        }
    }
}
