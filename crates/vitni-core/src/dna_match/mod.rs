//! The `DnaMatch` aggregate (data-model §6, §9, §12).
//!
//! A `DnaMatch` is a pairwise observation between two `DnaTest`s, owned by neither person. The
//! observation is high-surety data; the relationship it *implies* is a separate citing assertion on
//! Person/Family (a `FactAsserted`/`AssociationAsserted` citing this match — data-model §12), not a
//! field here. Both tests must exist (the §9 aggregate tax), resolved before `decide` via the
//! `Services` slot — see [`ref_resolver`].

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod ref_resolver;
pub mod state;
pub mod view;

pub use command::{DnaMatchCommand, DnaMatchCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::DnaMatchError;
pub use event::{DnaMatchEvent, DnaMatchEventBody};
pub use ref_resolver::{DnaMatchRefResolver, DnaMatchRefs};
pub use state::{DnaMatchState, MatchStatus};
pub use view::DnaMatchView;
