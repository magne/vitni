//! The `DnaTest` aggregate (data-model §6, §9, §12).
//!
//! A `DnaTest` is anchored to one Person and carries the raw result metadata (provider, kit,
//! type, build, haplogroups). It is the cross-aggregate-checked analogue of Citation/Event: the
//! anchoring person must exist (the §9 aggregate tax), resolved before `decide` via the `Services`
//! slot — see [`ref_resolver`]. Otherwise it follows the Person template.

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod ref_resolver;
pub mod state;
pub mod view;

pub use command::{DnaTestCommand, DnaTestCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::DnaTestError;
pub use event::{DnaTestEvent, DnaTestEventBody};
pub use ref_resolver::{DnaTestRefResolver, DnaTestRefs};
pub use state::DnaTestState;
pub use view::DnaTestView;
