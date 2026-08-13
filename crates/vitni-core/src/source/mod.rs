//! The Source aggregate (data-model §6, §9, §10).
//!
//! A `Source` is a body of information a `Citation` points into (a parish register, a census, a
//! book). For this spike it is minimal — an identity and a title — enough to be the target of a
//! Citation's `source_id` and the `UnknownSource` check. It follows the Person template: a pure
//! [`decide`](decide::decide)/[`evolve`](decide::evolve) core, the
//! [`SourceCommand`]/[`SourceEvent`]/[`SourceError`] vocabulary, the folded [`SourceState`], a
//! conclusion-layer [`SourceView`], and a thin `cqrs-es` adapter.

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod ref_resolver;
pub mod state;
pub mod view;

pub use command::{SourceCommand, SourceCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::SourceError;
pub use event::{SourceEvent, SourceEventBody};
pub use ref_resolver::{SourceRefResolver, SourceRefs};
pub use state::SourceState;
pub use view::SourceView;
