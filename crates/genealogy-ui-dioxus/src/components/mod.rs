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
mod record_picker;
mod select_input;
mod text_field;
mod text_input;
mod toggle;

pub use button::{Button, ButtonVariant, IconButton};
pub use color_picker::ColorPicker;
pub use data::{Badge, Chip, ListRow, Table};
pub use draft_field::{
    DraftDate, DraftSelect, DraftText, date_calendar_options, date_draft_field, date_field_error,
    date_modifier_options, date_quality_options,
};
pub use evidence::{
    ConfidenceBadge, EvidenceAxisChip, NoSourceFlag, ProvenancePopover, RestrictionChoice, RestrictionSet, SourceLink,
};
pub use feedback::Toast;
pub use forms::{Checkbox, DateInput, DatePicker, Input, LabeledValue, NumberInput, Select, Textarea};
pub use history::{HistoryEntry, HistoryTimeline};
pub use layout::{Card, EmptyState, Modal, SidePanel};
pub use nav::{Breadcrumb, StatusLine, TabItem, Tabs};
pub use provenance::{ProvenanceAxis, ProvenanceBlock, provenance_new_citation_card};
pub use record_picker::{
    DraftPickerView, PickerCallbacks, PickerConfig, PickerOptions, RecordPicker, draft_card, draft_picker_field,
    picker_options, record_picker,
};
pub use select_input::{SelectChoice, SelectInput};
pub use text_field::{FieldMessage, InputLabel, TextField};
pub use text_input::{TextInput, TextInputKind};
pub use toggle::{RadioChoice, RadioGroup, Switch};
