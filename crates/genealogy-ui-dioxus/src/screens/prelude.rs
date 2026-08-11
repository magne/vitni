//! Shared imports for the per-aggregate screen modules. Submodules glob this
//! (`use super::prelude::*;`); the `::prelude` suffix is exempt from `clippy::wildcard_imports`.
pub use dioxus::prelude::*;
pub use genealogy_app::{
    Address, Age, AgeBound, AssociationRole, Attribute, Centimorgans, ChildParentRelationship, ChromosomeSide,
    DnaProvider, DnaSegment, NameType, NewParticipation, NoteType, ParticipantRole, PersonNameParts, RecentItem, Rect,
    Sex, SourceMediaType, TagRef, TagSummary, Url,
};
pub use genealogy_ui::{
    ActivityVm, AddressVm, AssociationVm, AttachedRefVm, Category, CitationDetail, CitationEdit, CitationRefVm,
    CitingRecordVm, ConfidenceLevel, DashboardVm, DataQualityVm, Destination, DnaInferenceVm, DnaMatchDetail,
    DnaMatchEdit, DnaSegmentVm, DnaTestDetail, DnaTestEdit, DnaTestMatchVm, DuplicateCandidateVm, EventDetail,
    EventEdit, EventRefVm, FactVm, FamilyDetail, FamilyEdit, FamilyEventVm, FamilyVm, Intent, IntentOutcome, JumpVm,
    Localizer, MediaDetail, MediaEdit, MediaRefVm, MergeBlockedVm, MergeCompareVm, MergeFailure, MergeFieldRowVm,
    MergePersons, NameVm, NewCitationFields, NewPlaceFields, NewSourceFields, NoteDetail, NoteEdit, Panel,
    PersonChangeSetRequest, PersonDetail, PersonDraft, PersonEdit, PickerSelection, PlaceDetail, PlaceEdit,
    ProvenanceDraft, RecordDraft, RecordRef, RepositoryDetail, RepositoryEdit, RestrictionKind, RowVm,
    SharedAncestorVm, SourceCitationVm, SourceDetail, SourceEdit, SubmitResult, TagDetail, TagDraft, TagUsageGroupVm,
    TimelineRowVm, Tool, TranslationVm, UsingRecordVm, citation_tabs, dna_match_tabs, dna_test_tabs, event_tabs,
    family_tabs, media_tabs, note_tabs, person_tabs, place_tabs, repository_tabs, source_tabs, tag_tabs,
};

pub use crate::app::{AppCtx, AppState};
pub use crate::components::{
    Button, ButtonVariant, Card, Chip, ConfidenceBadge, DraftPickerView, DraftSelect, DraftText, EmptyState,
    EvidenceAxisChip, HistoryEntry, HistoryTimeline, Input, NoSourceFlag, PickerCallbacks, PickerConfig,
    ProvenancePopover, RadioChoice, RadioGroup, RecordPicker, RestrictionChoice, RestrictionSet, Select, SelectChoice,
    SidePanel, SourceLink, Switch, TabItem, Table, TextField, TextInput, TextInputKind, date_draft_field, draft_card,
    draft_picker_field, picker_options, record_picker,
};
pub use crate::master_detail::{DetailContainer, ListChrome, ListPane, SortChrome};
pub use crate::services::{
    ScreenData, commit_citation_change_set, commit_dna_match_change_set, commit_dna_test_change_set,
    commit_event_change_set, commit_family_change_set, commit_media_change_set, commit_note_change_set,
    commit_person_change_set, commit_place_change_set, commit_repository_change_set, commit_source_change_set,
    commit_tag_change_set, load_data_quality, load_picker_rows, load_plugin_panel, load_screen, load_tags,
    merge_persons, save_citation_edit, save_dna_match_edit, save_dna_test_edit, save_edit, save_event_edit,
    save_family_edit, save_media_edit, save_note_edit, save_place_edit, save_repository_edit, save_source_edit,
    submit_plugin_panel,
};
pub use crate::shell::ChromeCtx;
pub use crate::shell::nav_state::{DraftId, NavState, data_version_ticket};
pub use crate::vocabulary_render::{PanelAction, PanelView};

pub use super::record_form::{
    RecordActionLabels, RecordEditState, apply_record_edits, finish_draft_commit, finish_record_save,
    record_edit_provenance, record_head_actions, record_keydown, use_record_create, use_record_edit,
    use_save_on_request,
};
pub use super::research_note::ResearchNotesTab;
pub use super::shared::{
    JumpButton, MediaTabState, RecordLink, RegisterFields, RetractTarget, RowRetract, attach_picker_form,
    create_record_frame, id_list, media_tab, non_empty, optional_enum_select, picker_selection_id, provenance_block,
    provenance_block_dna, provenance_cue, record_enum_select, register_fields_form, retract_side_panel,
    row_actions_cell, source_cue, source_media_type_choices, use_detail_tab, use_existing_picker, use_record_undo,
};
pub use super::tabs::{
    AddressForm, ParticipationForm, ParticipationSeed, address_cards, citations_table, history_panel, tab_with_add,
    tags_panel,
};
