//! The Place aggregate (data-model §6, §9, §10).
//!
//! A `Place` is a named location with a type (country, parish, farm, …) that other aggregates
//! reference by id — an Event's `LinkPlace`, a Fact's `place_id`. It follows the Person template:
//! a pure [`decide`](decide::decide)/[`evolve`](decide::evolve) core, the
//! [`PlaceCommand`]/[`PlaceEvent`]/[`PlaceError`] vocabulary, the folded [`PlaceState`], a
//! conclusion-layer [`PlaceView`], and a thin `cqrs-es` adapter.

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod ref_resolver;
pub mod state;
pub mod view;

pub use command::{PlaceCommand, PlaceCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::PlaceError;
pub use event::{PlaceEvent, PlaceEventBody};
pub use ref_resolver::{PlaceRefResolver, PlaceRefs};
pub use state::PlaceState;
pub use view::PlaceView;
