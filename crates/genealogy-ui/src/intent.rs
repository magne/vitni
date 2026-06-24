//! Intent dispatch: turns a data-loading [`Intent`] into a `genealogy-app` use-case call and returns
//! render-ready view-models.
//!
//! This is the only place the presentation layer touches the application's use-cases. It is async
//! because the use-cases are; a renderer awaits it on its own runtime.

use std::collections::{BTreeSet, HashMap};

use genealogy_app::{
    AppError, ChildParentRelationship, EvidenceLevel, NewFact, NewPerson, PersonNameParts, Provenance, Restriction,
    Session, Sex, Workspace, add_child, add_citation_attribute, add_name, add_partner, add_person_citation,
    assert_association, assert_citation_date, assert_fact, assert_sex, attach_citation_media, attach_citation_note,
    attach_family_media, attach_family_note, attach_person_media, attach_person_note, change_log_for_citation,
    change_log_for_family, change_log_for_person, create_person, families_for_person, link_family_event,
    list_citations, list_events, list_families, list_persons, recent_activity, set_citation_confidence,
    set_citation_evidence_analysis, set_citation_restrictions, set_family_restrictions, set_page, set_restrictions,
    show_citation, show_family, show_person, tag_citation, tag_family, undo_assertion, undo_citation_assertion,
    undo_family_assertion, workspace_counts,
};

use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::navigation::{CitationEdit, FamilyEdit, Intent, PersonEdit};
use crate::view_model::{
    CitationDetail, CitationRefVm, DashboardVm, EventRefVm, FamilyDetail, FamilyVm, PersonDetail, citation_ref_vm,
    citation_row, collapse_history, family_row, person_row,
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
