//! The Person aggregate (data-model §6, §10).
//!
//! A `Person` is an individual — either a *persona* (extracted from a single source) or a
//! *conclusion* (a researcher's synthesis). This module is the reference shape for every other
//! aggregate: a pure decision core ([`decide`](decide::decide)) and fold ([`evolve`](decide::evolve)),
//! the [`PersonCommand`]/[`PersonEvent`]/[`PersonError`] vocabulary, the folded [`PersonState`], a
//! conclusion-layer [`PersonView`], and a thin `cqrs-es` adapter (in `aggregate`).

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod state;
pub mod view;

pub use command::{PersonCommand, PersonCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::PersonError;
pub use event::{PersonEvent, PersonEventBody};
pub use state::{Association, Participation, PersonState};
pub use view::PersonView;
