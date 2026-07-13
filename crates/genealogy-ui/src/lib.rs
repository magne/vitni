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
pub mod help;
pub mod i18n;
pub mod intent;
pub mod list;
pub mod navigation;
pub mod palette;
pub mod picker;
pub mod presentation;
pub mod rail;
pub mod shortcuts;
pub mod view_model;
pub mod vocabulary;

pub use detail::DetailTab;
pub use help::{
    Cell, HelpBlock, HelpDoc, HelpSection, HelpTopicId, HelpTopicMeta, Run, SpecimenKind, help_doc, help_topics,
};
pub use i18n::{Localizer, resolve_form};
pub use intent::{
    IntentOutcome, dispatch, dispatch_citation_change_set, dispatch_citation_edit, dispatch_dna_match_change_set,
    dispatch_dna_match_edit, dispatch_dna_test_change_set, dispatch_dna_test_edit, dispatch_edit,
    dispatch_event_change_set, dispatch_event_edit, dispatch_family_change_set, dispatch_family_edit,
    dispatch_media_change_set, dispatch_media_edit, dispatch_merge, dispatch_note_change_set, dispatch_note_edit,
    dispatch_person_change_set, dispatch_place_change_set, dispatch_place_edit, dispatch_repository_change_set,
    dispatch_repository_edit, dispatch_source_change_set, dispatch_source_edit, dispatch_tag_change_set,
    resolve_record_name,
};
pub use list::{ListQuery, RowSort, RowVm, step_row, visible_rows};
pub use navigation::{
    Category, CitationChangeSetRequest, CitationEdit, CitationSourceRequest, Destination, DnaMatchChangeSetRequest,
    DnaMatchEdit, DnaTestChangeSetRequest, DnaTestEdit, DraftCitationRef, DraftNewCitation, DraftNewSource,
    DraftSourceRef, EventChangeSetRequest, EventEdit, EventPlaceRequest, FamilyChangeSetRequest, FamilyEdit, Intent,
    MediaChangeSetRequest, MediaEdit, MergePersons, NavHistory, NavLocation, NoteChangeSetRequest, NoteEdit,
    PartnerRequest, PersonChangeSetRequest, PersonEdit, PlaceChangeSetRequest, PlaceEdit, RecordRef,
    RepositoryChangeSetRequest, RepositoryEdit, Screen, SourceChangeSetRequest, SourceEdit, TagChangeSetRequest, Tool,
    tab_label,
};
pub use palette::{
    PALETTE_GROUP_MAX, PaletteAction, PaletteCommand, PaletteCommandVm, PaletteEntry, PaletteGroup, PaletteGroupKind,
    activate, move_active, palette_commands, palette_groups,
};
pub use picker::{PICKER_MAX_ROWS, PickerSelection, PickerState, list_intent, picker_rows};
pub use presentation::{
    ConfidenceLevel, EVIDENCE_KINDS, EvidenceAxis, EvidenceKind, INFORMATION_KINDS, InformationKind, RestrictionKind,
    SOURCE_QUALITIES, SourceQuality,
};
pub use rail::{RailGroup, RailItem, rail_items};
pub use shortcuts::{
    Chord, Key, Modifier, NavShortcut, Shortcut, ShortcutAction, ShortcutGroup, navigation_shortcuts, shortcuts,
};
pub use view_model::{
    ActivityVm, AssociationVm, AttachedRefVm, ChildRelationshipVm, CitationAttributeVm, CitationDetail, CitationDraft,
    CitationRefVm, CitingRecordVm, DATE_CALENDARS, DATE_QUALITIES, DEFAULT_TAG_COLOR, DEFAULT_TAG_PRIORITY,
    DashboardStats, DashboardVm, DateDraft, DateEntryError, DateModifierKind, DnaMatchDetail, DnaMatchDraft,
    DnaSegmentVm, DnaTestDetail, DnaTestDraft, DnaTestMatchVm, DuplicateCandidateVm, EventDetail, EventDraft,
    EventRefVm, EvidenceAxisVm, FactVm, FamilyChildVm, FamilyDetail, FamilyDraft, FamilyEventVm, FamilyMediaVm,
    FamilyVm, HaplogroupRowVm, HistoryEntryVm, JumpVm, MediaAttributeVm, MediaDetail, MediaDraft, MergeBlockedVm,
    MergeCompareVm, MergeFailure, MergeFieldRowVm, MergeResultVm, NameVm, NewCitationFields, NewPersonFields,
    NewPlaceFields, NewSourceFields, NoteDetail, NoteDraft, ParticipantVm, PartnerInput, PartnerVm, PedigreeNodeVm,
    PedigreeSlotVm, PedigreeVm, PersonDetail, PersonDraft, PlaceDetail, PlaceDraft, PlaceHierarchyVm, PlaceLinkVm,
    PlaceNameVm, ProvenanceDraft, RecordDraft, RecordLink, RelationshipVm, RepositoryDetail, RepositoryDraft,
    RepositoryLinkVm, RepositoryUrlVm, SharedAncestorVm, SourceAttributeVm, SourceCitationVm, SourceDetail,
    SourceDraft, SourceHeldVm, SourceReliabilityVm, TagDetail, TagDraft, TagUsageGroupVm, TranslationVm, UsingRecordVm,
    citation_row, citation_tabs, collapse_history, dna_match_row, dna_match_tabs, dna_test_row, dna_test_tabs,
    event_row, event_tabs, evidence_axes, family_row, family_tabs, first_undoable, format_date_point, media_row,
    media_tabs, note_row, note_tabs, parse_date_point, person_row, person_tabs, place_row, place_tabs, repository_row,
    repository_tabs, source_row, source_tabs, tag_row, tag_tabs,
};
pub use vocabulary::{Field, Form, SelectOption, VocabularyError, parse};
