//! The Event aggregate (data-model §6, §9, §10).
//!
//! An `Event` is a shared, dated, placed occurrence (a birth, a marriage, a census) that Persons
//! participate in via `Person::AssertParticipation` (data-model §10). Like Citation, it carries a
//! **cross-aggregate reference** — `LinkPlace` names a `place_id` that must exist — so it pays the
//! aggregate tax (data-model §9) through the `cqrs-es` `Services` slot (ADR 0004 §3): `decide`
//! stays pure and receives the resolved [`EventRefs`](ref_resolver::EventRefs).

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod events;
pub mod ref_resolver;
pub mod state;
pub mod upcasters;
pub mod view;

pub use command::{EventCommand, EventCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::EventError;
pub use events::{EventEvent, EventEventBody};
pub use ref_resolver::{EventRefResolver, EventRefs};
pub use state::EventState;
pub use upcasters::upcasters;
pub use view::EventView;
