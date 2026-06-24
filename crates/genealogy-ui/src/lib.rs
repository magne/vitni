//! `genealogy-ui` — the framework-agnostic presentation layer (ADR 0008).
//!
//! This crate sits between `genealogy-app` (use-cases + DTOs, ADR 0006) and a concrete framework
//! renderer (`genealogy-ui-dioxus` today). It holds **all presentation logic and no framework
//! types**: view-models derived from the app's DTOs ([`view_model`]), navigation/data intents and
//! their async dispatch to use-cases ([`navigation`], [`intent`]), the rail descriptor list
//! ([`rail`]) and keyboard shortcut map ([`shortcuts`]), Fluent string resolution ([`i18n`],
//! ADR 0003), shared render enums ([`presentation`]), and the plugin-UI [`vocabulary`] types
//! (ADR 0012).
//!
//! Dependency direction is one-way: `genealogy-app → genealogy-ui → genealogy-ui-<framework>`. No
//! `dioxus::` (or other framework) type appears here, and neither does the plugin host — a renderer
//! drives the host and hands this crate the plugin's JSON to [`vocabulary::parse`].

pub mod detail;
pub mod i18n;
pub mod intent;
pub mod list;
pub mod navigation;
pub mod presentation;
pub mod rail;
pub mod shortcuts;
pub mod view_model;
pub mod vocabulary;

pub use detail::DetailTab;
pub use i18n::{Localizer, resolve_form};
pub use intent::{IntentOutcome, dispatch, dispatch_citation_edit, dispatch_create, dispatch_edit};
pub use list::{ListQuery, RowSort, RowVm, visible_rows};
pub use navigation::{Category, CitationEdit, Destination, Intent, PersonEdit, RecordRef, Screen, Tool};
pub use presentation::{ConfidenceLevel, EvidenceAxis, RestrictionKind};
pub use rail::{RailGroup, RailItem, rail_items};
pub use shortcuts::{
    Chord, Key, Modifier, NavShortcut, Shortcut, ShortcutAction, ShortcutGroup, navigation_shortcuts, shortcuts,
};
pub use view_model::{
    ActivityVm, AssociationVm, CitationDetail, DashboardStats, DashboardVm, EventRefVm, EvidenceAxisVm, FactVm,
    FamilyVm, HistoryEntryVm, JumpVm, NameVm, PersonDetail, citation_row, citation_tabs, collapse_history,
    evidence_axes, person_row, person_tabs,
};
pub use vocabulary::{Field, Form, SelectOption, VocabularyError, parse};
