//! `vitni-ui` — the framework-agnostic presentation layer (ADR 0008).
//!
//! This crate sits between `vitni-app` (use-cases + DTOs, ADR 0006) and a concrete framework
//! renderer (`vitni-ui-dioxus` today). It holds **all presentation logic and no framework
//! types**: view-models derived from the app's DTOs ([`view_model`]), navigation/data intents and
//! their async dispatch to use-cases ([`navigation`], [`intent`]), the rail descriptor list
//! ([`rail`]) and keyboard shortcut map ([`shortcuts`]), Fluent string resolution ([`i18n`],
//! ADR 0003), shared render enums ([`presentation`]), and the plugin-UI [`vocabulary`] types
//! (ADR 0012).
//!
//! Dependency direction is one-way: `vitni-app → vitni-ui → vitni-ui-<framework>`. No
//! `dioxus::` (or other framework) type appears here, and neither does the plugin host — a renderer
//! drives the host and hands this crate the plugin's JSON to [`vocabulary::parse`].
//!
//! # Licence
//!
//! `AGPL-3.0-or-later` (ADR 0034). Additional permission under GNU AGPL version 3 section 7: if you
//! modify this Program, or any covered work, by combining it with a WebAssembly component that
//! interacts with the Program solely through the versioned `vitni:host-api` WIT world (or any later
//! version of that world), the licensor grants you additional permission to convey the resulting
//! work. Such a component is not required to be licensed under the GNU AGPL.

pub mod action;
pub mod detail;
pub mod help;
pub mod i18n;
pub mod import_payload;
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

pub use action::{ActionLabel, Affordance};
pub use detail::DetailTab;
pub use help::{
    Cell, HelpBlock, HelpDoc, HelpSection, HelpTopicId, HelpTopicMeta, Run, SpecimenKind, help_doc, help_topics,
};
pub use i18n::{Localizer, resolve_confirm_record, resolve_panel, resolve_submit_result};
pub use import_payload::{
    ConfirmRecord, ConfirmRecordPayload, CropRegion, FieldValue, ImportPayload, ImportPayloadError, ImportResponse,
    ImportedRecord, PayloadAction, PayloadConfidence, PayloadField, ProvenancePreview, RecordRow, RecordsPayload,
    ResponseValues, SaveScanPayload, SaveSuggestion, ScanRef, SourceRef, SummaryPayload, parse_payload, parse_response,
};
pub use intent::{
    IntentOutcome, approve_plugin_grants, dispatch, dispatch_citation_change_set, dispatch_citation_edit,
    dispatch_dna_match_change_set, dispatch_dna_match_edit, dispatch_dna_test_change_set, dispatch_dna_test_edit,
    dispatch_event_change_set, dispatch_event_edit, dispatch_family_change_set, dispatch_family_edit,
    dispatch_media_change_set, dispatch_media_edit, dispatch_merge, dispatch_new_record, dispatch_note_change_set,
    dispatch_note_edit, dispatch_person_change_set, dispatch_person_edit, dispatch_place_change_set,
    dispatch_place_edit, dispatch_repository_change_set, dispatch_repository_edit, dispatch_research_note_change_set,
    dispatch_research_note_edit, dispatch_source_change_set, dispatch_source_edit, dispatch_tag_change_set,
    pin_publisher, resolve_record_name, unpin_publisher,
};
pub use list::{ListQuery, RowSort, RowVm, step_row, visible_rows};
pub use navigation::{
    Category, CitationChangeSetRequest, CitationEdit, CitationSourceRequest, Destination, DnaMatchChangeSetRequest,
    DnaMatchEdit, DnaTestChangeSetRequest, DnaTestEdit, DraftCitationRef, DraftNewCitation, DraftNewSource,
    DraftSourceRef, EventChangeSetRequest, EventEdit, EventPlaceRequest, FamilyChangeSetRequest, FamilyEdit, Intent,
    MediaChangeSetRequest, MediaEdit, MergePersons, NavHistory, NavLocation, NewRecordRequest, NoteChangeSetRequest,
    NoteEdit, PartnerRequest, PersonChangeSetRequest, PersonEdit, PlaceChangeSetRequest, PlaceEdit, RecordRef,
    RepositoryChangeSetRequest, RepositoryEdit, ResearchNoteChangeSetRequest, ResearchNoteEdit, Screen,
    SourceChangeSetRequest, SourceEdit, SubjectRequest, TagChangeSetRequest, Tool, tab_label,
};
pub use palette::{
    PALETTE_GROUP_MAX, PaletteAction, PaletteCommand, PaletteCommandVm, PaletteEntry, PaletteGroup, PaletteGroupKind,
    activate, move_active, palette_commands, palette_groups,
};
pub use picker::{ActiveMove, PICKER_MAX_ROWS, PickerSelection, PickerState, list_intent, next_active, picker_rows};
pub use presentation::{
    ConfidenceLevel, EVIDENCE_KINDS, EvidenceAxis, EvidenceKind, INFORMATION_KINDS, InformationKind, RestrictionKind,
    SOURCE_QUALITIES, SourceQuality,
};
pub use rail::{RailGroup, RailItem, rail_items};
pub use shortcuts::{
    BindingError, Chord, ChordParseError, Key, Modifier, NavShortcut, Shortcut, ShortcutAction, ShortcutGroup,
    navigation_shortcuts, resolved_shortcuts, shortcuts,
};
pub use view_model::{
    ActivityVm, AddressVm, AssociationVm, AttachSaveAction, AttachedRefVm, BulkImportProgress, BulkImportSession,
    BulkImportStage, BulkImportSummary, CATEGORY_CONVENTION, CapabilityGrantVm, ChildRelationshipVm,
    CitationAttributeVm, CitationDetail, CitationDraft, CitationRefVm, CitingRecordVm, DATE_CALENDARS, DATE_QUALITIES,
    DEFAULT_TAG_COLOR, DEFAULT_TAG_PRIORITY, DashboardStats, DashboardVm, DataQualityVm, DateDraft, DateEntryError,
    DateModifierKind, DnaInferenceVm, DnaMatchDetail, DnaMatchDraft, DnaSegmentVm, DnaTestDetail, DnaTestDraft,
    DnaTestMatchVm, DuplicateCandidateVm, EventDetail, EventDraft, EventPinVm, EventRefVm, EvidenceAxisVm,
    ExportDestination, ExportProgress, ExportSession, ExportStage, ExportSummary, FactVm, FamilyChildVm, FamilyDetail,
    FamilyDraft, FamilyEventVm, FamilyVm, FilenameHints, GeographyVm, HaplogroupRowVm, HistoryEntryVm, ImportSession,
    ImportSourcePath, ImportStage, ImportTargetChoice, ImportTargetError, JumpVm, MapPointVm, MarkerShapeVm,
    MediaAttributeVm, MediaDetail, MediaDraft, MediaRefVm, MediaSaveDraft, MergeBlockedVm, MergeCompareVm,
    MergeFailure, MergeFieldRowVm, MergeResultVm, NEW_EVENT_TYPES, NEW_PLACE_TYPES, NameVm, NewCitationFields,
    NewEventFields, NewMediaFields, NewNoteFields, NewPersonFields, NewPlaceFields, NewRecordDraft,
    NewRepositoryFields, NewSourceFields, NoteDetail, NoteDraft, ParticipantVm, PartnerInput, PartnerVm,
    PedigreeNodeVm, PedigreeSlotVm, PedigreeVm, PersonDetail, PersonDraft, PinnedPublisherVm, PlaceDetail, PlaceDraft,
    PlaceGeometryVm, PlaceHierarchyVm, PlaceLinkVm, PlaceMarkerVm, PlaceNameVm, PlaceRowStatus, PlaceRowVm,
    PlaceSuccessionVm, PluginGrantVm, ProvenanceDraft, RecordDraft, RecordLink, RelationshipVm, RepositoryDetail,
    RepositoryDraft, RepositoryLinkVm, RepositoryUrlVm, ResearchNoteDetail, ResearchNoteDraft, SUCCESSION_KINDS,
    SharedAncestorVm, ShortcutBindingVm, ShortcutsVm, SourceAttributeVm, SourceCitationVm, SourceDetail, SourceDraft,
    SourceHeldVm, SourceReliabilityVm, SubjectVm, TIME_SLIDER_RANGE, TagDetail, TagDraft, TagUsageGroupVm,
    TimelineKind, TimelineRowVm, TranslationVm, TrustStoreVm, UsingRecordVm, ZOOM_RANGE, citation_row, citation_tabs,
    clamp_slider_year, clamp_zoom, collapse_history, display_coordinates, dna_match_row, dna_match_tabs, dna_test_row,
    dna_test_tabs, event_list_row, event_row, event_tabs, evidence_axes, family_list_row, family_row, family_tabs,
    first_undoable, format_date_point, link_is_savable, media_row, media_tabs, name_matches, note_row, note_tabs,
    parse_date_point, person_list_row, person_row, person_tabs, place_map_display_shape, place_row, place_tabs,
    plugin_grant_vm, rect_css, rect_from_drag, repository_row, repository_tabs, research_note_row, research_note_tabs,
    resolve_attach_save, resolve_geometry_as_of, shortcuts_vm, slugify, source_row, source_tabs, suggest_filename,
    tag_row, tag_tabs, toggled_restrictions, trust_store_vm,
};
pub use vocabulary::{
    Action, Field, Form, Panel, SelectOption, SubmitResult, Table, VocabularyError, parse, parse_submit_result,
};
