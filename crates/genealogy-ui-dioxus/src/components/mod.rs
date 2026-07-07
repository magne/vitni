//! The design-system component library (Phase 5, ADR 0008 §5).
//!
//! Reusable, accessible RSX components built on the `tokens.css` + `components.css` design system.
//! Each ships its ARIA roles/labels, works with the keyboard, keeps colour from being the only
//! signal, and is *controlled* — stateful components take their value as a prop and emit an
//! `EventHandler`, so the call site owns the state. The dynamic keyboard layer (roving `tabindex`,
//! focus trap, command palette) and the app shell are built on top of these in PR2.

mod button;
mod color_picker;
mod data;
mod draft_field;
mod evidence;
mod feedback;
mod forms;
mod history;
mod layout;
mod nav;
mod provenance;

pub use button::{Button, ButtonVariant, IconButton};
pub use color_picker::ColorPicker;
pub use data::{Badge, Chip, ListRow, Table};
pub use draft_field::{DraftSelect, DraftText};
pub use evidence::{
    ConfidenceBadge, EvidenceAxisChip, NoSourceFlag, ProvenancePopover, RestrictionChoice, RestrictionSet, SourceLink,
};
pub use feedback::Toast;
pub use forms::{Checkbox, DatePicker, Input, LabeledValue, NumberInput, Select, SelectChoice};
pub use history::{HistoryEntry, HistoryTimeline};
pub use layout::{Card, EmptyState, Modal, SidePanel};
pub use nav::{Breadcrumb, StatusLine, TabItem, Tabs};
pub use provenance::{ProvenanceAxis, ProvenanceBlock};
