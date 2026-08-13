//! The Repository aggregate (data-model §6, §9, §10).
//!
//! A `Repository` is a place that holds sources (a library, an archive, a church). It follows the
//! Person template: a pure [`decide`](decide::decide)/[`evolve`](decide::evolve) core, the
//! [`RepositoryCommand`]/[`RepositoryEvent`]/[`RepositoryError`] vocabulary, the folded
//! [`RepositoryState`], a conclusion-layer [`RepositoryView`], and a thin `cqrs-es` adapter.

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod state;
pub mod view;

pub use command::{RepositoryCommand, RepositoryCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::RepositoryError;
pub use event::{RepositoryEvent, RepositoryEventBody};
pub use state::RepositoryState;
pub use view::RepositoryView;
