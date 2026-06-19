//! Intent dispatch: turns a data-loading [`Intent`] into a `genealogy-app` use-case call and returns
//! render-ready view-models.
//!
//! This is the only place the presentation layer touches the application's use-cases. It is async
//! because the use-cases are; a renderer awaits it on its own runtime.

use genealogy_app::{AppError, Workspace, list_persons, show_person};

use crate::i18n::Localizer;
use crate::navigation::Intent;
use crate::view_model::{PersonDetail, PersonRow};

/// The data a dispatched [`Intent`] produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentOutcome {
    /// The person list, as rows.
    List(Vec<PersonRow>),
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
                rows.push(PersonRow::from_summary(summary, loc));
            }
            Ok(IntentOutcome::List(rows))
        }
        Intent::ShowPerson { human_id } => match show_person(workspace, human_id).await? {
            Some(summary) => Ok(IntentOutcome::Detail(Box::new(PersonDetail::from_summary(
                &summary, loc,
            )))),
            None => Ok(IntentOutcome::NotFound {
                human_id: human_id.clone(),
            }),
        },
    }
}
