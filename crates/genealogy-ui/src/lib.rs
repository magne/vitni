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
pub use intent::{
    IntentOutcome, dispatch, dispatch_citation_edit, dispatch_create, dispatch_dna_match_edit, dispatch_dna_test_edit,
    dispatch_edit, dispatch_event_edit, dispatch_family_edit, dispatch_media_edit, dispatch_note_edit,
    dispatch_place_edit, dispatch_repository_edit, dispatch_source_edit, dispatch_tag_edit,
};
pub use list::{ListQuery, RowSort, RowVm, visible_rows};
pub use navigation::{
    Category, CitationEdit, Destination, DnaMatchEdit, DnaTestEdit, EventEdit, FamilyEdit, Intent, MediaEdit, NoteEdit,
    PersonEdit, PlaceEdit, RecordRef, RepositoryEdit, Screen, SourceEdit, TagEdit, Tool,
};
pub use presentation::{ConfidenceLevel, EvidenceAxis, RestrictionKind};
pub use rail::{RailGroup, RailItem, rail_items};
pub use shortcuts::{
    Chord, Key, Modifier, NavShortcut, Shortcut, ShortcutAction, ShortcutGroup, navigation_shortcuts, shortcuts,
};
pub use view_model::{
    ActivityVm, AssociationVm, CitationDetail, CitationRefVm, CitingRecordVm, DashboardStats, DashboardVm,
    DnaMatchDetail, DnaSegmentVm, DnaTestDetail, DnaTestMatchVm, EventDetail, EventRefVm, EvidenceAxisVm, FactVm,
    FamilyChildVm, FamilyDetail, FamilyEventVm, FamilyMediaVm, FamilyVm, HistoryEntryVm, JumpVm, MediaAttributeVm,
    MediaDetail, NameVm, NoteDetail, ParticipantVm, PartnerVm, PersonDetail, PlaceDetail, PlaceHierarchyVm,
    PlaceLinkVm, PlaceNameVm, RepositoryDetail, RepositoryLinkVm, SharedAncestorVm, SourceAttributeVm,
    SourceCitationVm, SourceDetail, SourceHeldVm, SourceReliabilityVm, TagDetail, TagUsageGroupVm, TranslationVm,
    UsingRecordVm, citation_row, citation_tabs, collapse_history, dna_match_row, dna_match_tabs, dna_test_row,
    dna_test_tabs, event_row, event_tabs, evidence_axes, family_row, family_tabs, media_row, media_tabs, note_row,
    note_tabs, person_row, person_tabs, place_row, place_tabs, repository_row, repository_tabs, source_row,
    source_tabs, tag_row, tag_tabs,
};
pub use vocabulary::{Field, Form, SelectOption, VocabularyError, parse};
