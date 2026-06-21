//! The Note aggregate (data-model §6, §10).
//!
//! A `Note` is free or rich text other aggregates attach by id. Follows the Person template: a pure
//! [`decide`](decide::decide)/[`evolve`](decide::evolve) core, the
//! [`NoteCommand`]/[`NoteEvent`]/[`NoteError`] vocabulary, the folded [`NoteState`], a
//! conclusion-layer [`NoteView`], and a thin `cqrs-es` adapter.

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod state;
pub mod view;

pub use command::{NoteCommand, NoteCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::NoteError;
pub use event::{NoteEvent, NoteEventBody};
pub use state::NoteState;
pub use view::NoteView;
