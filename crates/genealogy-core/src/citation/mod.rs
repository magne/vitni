//! The Citation aggregate (data-model §6, §9, §10).
//!
//! A `Citation` points into a `Source` (by `source_id`) and is the reusable evidence link an
//! assertion's [`EventContext`](crate::provenance::EventContext) references via
//! [`CitationRef`](crate::provenance::CitationRef). It is the first aggregate with a
//! **cross-aggregate reference**, so it is also the first to pay the "aggregate tax" (data-model
//! §9): the `source_id` it cites must exist, checked against the (possibly-lagging) Source
//! projection via the `cqrs-es` `Services` slot (ADR 0004 §3) — see [`ref_resolver`]. `decide`
//! stays pure, receiving the resolved [`CitationRefs`](ref_resolver::CitationRefs).

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod ref_resolver;
pub mod state;
pub mod view;

pub use command::{CitationCommand, CitationCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::CitationError;
pub use event::{CitationEvent, CitationEventBody};
pub use ref_resolver::{CitationRefResolver, CitationRefs};
pub use state::{CitationState, CreationStamp};
pub use view::CitationView;
