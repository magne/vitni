//! The `ResearchNote` aggregate (ADR 0028, data-model §17): a written proof argument about one
//! conclusion-bearing subject (GEDCOM X's `Document(Analysis)`).
//!
//! Follows the Note/Event template: a pure [`decide`](decide::decide)/[`evolve`](decide::evolve)
//! core, the [`ResearchNoteCommand`]/[`ResearchNoteEvent`]/[`ResearchNoteError`] vocabulary, the
//! folded [`ResearchNoteState`], a conclusion-layer [`ResearchNoteView`], and a thin `cqrs-es`
//! adapter. Like Event/Citation, it carries one cross-aggregate reference (its `subject`), resolved
//! through [`ref_resolver::ResearchNoteRefResolver`] before `decide` runs (the §9 aggregate tax).
//!
//! A `ResearchNote` points *at* its one subject rather than being attached *by* it — unlike
//! `Media`/`Note`, which are two-sided reusable attachments, no subject aggregate gains a
//! `ResearchNoteAttached` event or an id-list field. "Which arguments exist about this Person" is
//! answered by a reverse query over this aggregate's projection, not a field on Person/Family/
//! Event/Place (ADR 0028 §5).

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod ref_resolver;
pub mod state;
pub mod subject;
pub mod view;

pub use command::{ResearchNoteCommand, ResearchNoteCommandEnvelope};
pub use decide::{decide, evolve};
pub use error::ResearchNoteError;
pub use event::{ResearchNoteEvent, ResearchNoteEventBody};
pub use ref_resolver::{ResearchNoteRefResolver, ResearchNoteRefs};
pub use state::ResearchNoteState;
pub use subject::SubjectRef;
pub use view::ResearchNoteView;
