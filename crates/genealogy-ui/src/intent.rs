//! Intent dispatch: turns a data-loading [`Intent`] into a `genealogy-app` use-case call and returns
//! render-ready view-models.
//!
//! This is the only place the presentation layer touches the application's use-cases. It is async
//! because the use-cases are; a renderer awaits it on its own runtime.

use std::collections::{BTreeSet, HashMap};

use genealogy_app::{
    AppError, ChildParentRelationship, EvidenceLevel, NewFact, NewPerson, PersonNameParts, Provenance, Restriction,
    Session, Sex, Workspace, add_child, add_citation_attribute, add_event_citation, add_name, add_partner,
    add_person_citation, add_place_citation, add_place_name, add_repository_address, add_repository_url,
    add_source_attribute, assert_association, assert_citation_date, assert_fact, assert_place_enclosed_by, assert_sex,
    attach_citation_media, attach_citation_note, attach_family_media, attach_family_note, attach_person_media,
    attach_person_note, change_log_for_citation, change_log_for_event, change_log_for_family, change_log_for_person,
    change_log_for_place, change_log_for_repository, change_log_for_source, create_person, families_for_person,
    import_attach_event_media, import_attach_event_note, import_attach_place_media, import_attach_place_note,
    import_attach_repository_note, import_attach_source_media, import_attach_source_note, link_family_event,
    link_source_repository, list_citations, list_events, list_families, list_persons, list_places, list_repositories,
    list_sources, recent_activity, set_citation_confidence, set_citation_evidence_analysis, set_citation_restrictions,
    set_event_restrictions, set_family_restrictions, set_page, set_participant_role, set_place_restrictions,
    set_repository_restrictions, set_restrictions, set_source_restrictions, show_citation, show_event, show_family,
    show_person, show_place, show_repository, show_source, tag_citation, tag_event, tag_family, tag_place,
    tag_repository, tag_source, undo_assertion, undo_citation_assertion, undo_event_assertion, undo_family_assertion,
    undo_place_assertion, undo_repository_assertion, undo_source_assertion, workspace_counts,
};

use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::navigation::{
    CitationEdit, EventEdit, FamilyEdit, Intent, PersonEdit, PlaceEdit, RepositoryEdit, SourceEdit,
};
use crate::view_model::{
    CitationDetail, CitationRefVm, DashboardVm, EventDetail, EventRefVm, FamilyDetail, FamilyVm, PersonDetail,
    PlaceDetail, RepositoryDetail, SourceDetail, citation_ref_vm, citation_row, collapse_history, event_row,
    family_row, person_row, place_row, repository_row, source_row,
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
        Intent::ShowPerson { human_id } => match show_person(workspace, human_id).await? {
            Some(summary) => {
                let mut detail = PersonDetail::from_summary(&summary, loc);
                detail.events = build_events(workspace, loc, &summary.participations).await?;
                detail.families = families_for_person(workspace, human_id)
                    .await?
                    .iter()
                    .map(|family| FamilyVm::from_app(family, loc))
                    .collect();
                detail.citations = build_citations(workspace, loc, &summary.citations).await?;
                let change_log = change_log_for_person(workspace, human_id).await?;
                detail.history = collapse_history(&change_log, loc);
                Ok(IntentOutcome::Detail(Box::new(detail)))
            }
            None => Ok(IntentOutcome::NotFound {
                human_id: human_id.clone(),
            }),
        },
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

/// Builds the Citations-tab view-models by joining a person's backing-citation `human_id`s to the
/// citation projection, surfacing each citation's source, surety, and Evidence Explained axes. A
/// citation whose record cannot be loaded degrades to an id-only row rather than failing the load.
async fn build_citations(
    workspace: &Workspace,
    loc: &Localizer,
    citation_ids: &[String],
) -> Result<Vec<CitationRefVm>, AppError> {
    let summaries: HashMap<String, genealogy_app::CitationSummary> = list_citations(workspace)
        .await?
        .into_iter()
        .map(|summary| (summary.human_id.clone(), summary))
        .collect();
    Ok(citation_ids
        .iter()
        .map(|id| match summaries.get(id) {
            Some(summary) => citation_ref_vm(summary, loc),
            None => CitationRefVm {
                human_id: id.clone(),
                source: None,
                page: None,
                confidence: None,
                confidence_label: None,
                evidence_axes: Vec::new(),
            },
        })
        .collect())
}

/// Builds the Events-tab view-models by joining a person's participations to the event projection
/// for each event's rendered date (the role is on the participation).
async fn build_events(
    workspace: &Workspace,
    loc: &Localizer,
    participations: &[(String, genealogy_app::ParticipantRole)],
) -> Result<Vec<EventRefVm>, AppError> {
    let dates: HashMap<String, String> = list_events(workspace)
        .await?
        .into_iter()
        .filter_map(|event| event.date.as_ref().map(|date| (event.human_id.clone(), loc.date(date))))
        .collect();
    Ok(participations
        .iter()
        .map(|(event_id, role)| EventRefVm {
            event_id: event_id.clone(),
            role_label: loc.participant_role_label(role),
            date: dates.get(event_id).cloned(),
        })
        .collect())
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
