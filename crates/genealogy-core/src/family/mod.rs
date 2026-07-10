//! The Family aggregate (data-model §6, §10).
//!
//! A `Family` is a union and its children: it links persons as partners (neutral roles) and as
//! children (each with a [`ChildParentRelationship`](crate::enums::ChildParentRelationship)). It is
//! the relationship backbone over the Person aggregate, and follows the Person reference shape: a
//! pure decision core ([`decide`](decide::decide)) and fold ([`evolve`](decide::evolve)), the
//! [`FamilyCommand`]/[`FamilyEvent`]/[`FamilyError`] vocabulary, the folded [`FamilyState`], a
//! conclusion-layer [`FamilyView`], and a thin `cqrs-es` adapter (in `aggregate`).

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod state;
pub mod view;

pub use command::{FamilyCommand, FamilyCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::FamilyError;
pub use event::{FamilyEvent, FamilyEventBody};
pub use state::{AssertedChild, ChildEntry, ChildRelationship, FamilyState};
pub use view::FamilyView;
