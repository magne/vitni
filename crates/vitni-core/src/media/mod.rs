//! The Media aggregate (data-model §6, §10).
//!
//! A `Media` object is a digital artifact (a file or web reference) other aggregates reference via
//! [`MediaRef`](crate::text::MediaRef). Follows the Person template: a pure
//! [`decide`](decide::decide)/[`evolve`](decide::evolve) core, the
//! [`MediaCommand`]/[`MediaEvent`]/[`MediaError`] vocabulary, the folded [`MediaState`], a
//! conclusion-layer [`MediaView`], and a thin `cqrs-es` adapter.

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod state;
pub mod view;

pub use command::{MediaCommand, MediaCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::MediaError;
pub use event::{MediaEvent, MediaEventBody};
pub use state::MediaState;
pub use view::MediaView;
