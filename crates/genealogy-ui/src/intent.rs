//! Intent dispatch: turns a data-loading [`Intent`] into a `genealogy-app` use-case call and returns
//! render-ready view-models.
//!
//! This is the only place the presentation layer touches the application's use-cases. It is async
//! because the use-cases are; a renderer awaits it on its own runtime.

use std::collections::{BTreeSet, HashMap};

use genealogy_app::{
    AppError, EvidenceLevel, NewFact, NewPerson, PersonNameParts, Provenance, Restriction, Session, Sex, Workspace,
    add_name, add_person_citation, assert_association, assert_fact, assert_sex, attach_person_media,
    attach_person_note, create_person, families_for_person, list_events, list_persons, set_restrictions, show_person,
};

use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::navigation::{Intent, PersonEdit};
use crate::view_model::{EventRefVm, FamilyVm, PersonDetail, person_row};

/// The data a dispatched [`Intent`] produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentOutcome {
    /// The list, as generic rows.
    List(Vec<RowVm>),
    /// One person's detail.
    Detail(Box<PersonDetail>),
    /// The requested person id was not found.
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
                Ok(IntentOutcome::Detail(Box::new(detail)))
            }
            None => Ok(IntentOutcome::NotFound {
                human_id: human_id.clone(),
            }),
        },
    }
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
    }
}
