//! The Tag aggregate (data-model §6, §9, §10).
//!
//! A `Tag` is the lightweight *definition* (name/colour/priority) other aggregates reference by id;
//! applying a tag is an event on the tagged aggregate, not here. Tags carry no `HumanId` and no
//! retract/supersede pair — the setters are last-writer-wins. Otherwise it follows the Person
//! template: a pure [`decide`](decide::decide)/[`evolve`](decide::evolve) core, the
//! [`TagCommand`]/[`TagEvent`]/[`TagError`] vocabulary, the folded [`TagState`], a conclusion-layer
//! [`TagView`], and a thin `cqrs-es` adapter.

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod state;
pub mod view;

pub use command::{TagCommand, TagCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::TagError;
pub use event::{TagEvent, TagEventBody};
pub use state::TagState;
pub use view::TagView;
