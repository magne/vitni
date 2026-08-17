//! The application's own screens (ADR 0008 §5): per-framework RSX over the shared `vitni-ui`
//! view-models, built on the design-system components and the generic master-detail framework
//! (`crate::master_detail`). Each aggregate contributes a `*DetailPane` (its editor-host content,
//! routed by [`RecordDetail`]) and a `*CreateRecord` (its draft create form); the entity list is the
//! shell-level [`Explorer`](crate::shell::explorer::Explorer), and the tabstrip/dock/keyboard come
//! from the shell. The plugin panel renders a plugin-supplied form through the vocabulary interpreter.

mod bulk_import;
mod citation;
mod dashboard;
mod detail_commits;
mod dna_match;
mod dna_test;
mod event;
mod export;
mod family;
mod geography;
mod help;
mod import;
mod map_shared;
mod media;
mod merge;
mod note;
mod pedigree;
mod person;
mod place;
mod plugin_panel;
mod preferences;
mod prelude;
mod record_detail;
mod record_form;
mod repository;
mod research_note;
mod shared;
mod source;
mod tabs;
mod tag;

pub use bulk_import::{
    BulkConfirmLabels, BulkImportBody, BulkImportWizardLabels, BulkRunningLabels, BulkRunningStage, BulkSourceLabels,
    BulkSourceStage, BulkSummaryLabels, BulkSummaryStage,
};
pub use citation::{CitationEditForm, citation_attributes_table, citation_create_fields, citation_overview};
pub use dashboard::{DashboardScreen, dashboard_view};
pub use detail_commits::{
    CitationCommits, DetailAggregate, DetailCommits, DnaMatchCommits, DnaTestCommits, EventCommits, FamilyCommits,
    MediaCommits, NoteCommits, PersonCommits, PlaceCommits, RepositoryCommits, ResearchNoteCommits, SourceCommits,
    use_detail_commits,
};
pub use dna_match::{
    DnaMatchEditForm, dna_match_ancestors_table, dna_match_create_fields, dna_match_overview, dna_match_record_fields,
    dna_match_segments_table,
};
pub use dna_test::{
    DnaTestEditForm, dna_test_create_fields, dna_test_haplogroups_table, dna_test_matches_table, dna_test_overview,
    dna_test_record_fields,
};
pub use event::{
    EventEditCtx, EventEditForm, event_create_fields, event_overview, event_participants_table, event_record_fields,
};
pub use export::{
    DestinationStage, ExportDestinationLabels, ExportRunningLabels, ExportScreen, ExportSummaryLabels,
    ExportSummaryStage, ExportWizardLabels, NoticeStage, RunningStage, WizardNoticeTone,
};
pub use family::{
    ChildRemoval, FamilyEditForm, child_removal_side_panel, family_children_table, family_create_fields,
    family_events_table, family_overview, family_record_fields,
};
pub use geography::{
    GeographyScreen, MapPane, geography_draw_target, geography_empty_state, geography_map_surface,
    geography_provider_choices, geography_rail, geography_time_slider, geography_unplotted_note,
};
pub use help::{HelpScreen, render_doc};
pub use import::{
    ConfirmChrome, ConfirmStage, ImportModeLabels, ImportModeSwitch, ImportRowStatus, ImportScreen, RecordsLabels,
    RecordsStage, SaveStage, SourceLabels, SourceStage, SummaryLabels, SummaryStage, WizardLabels,
};
pub use map_shared::{DrawTool, MapControlLabels, MapDraft, MapZoomReadout, MovedCamera, effective_date_choice};
pub use media::{MediaEditForm, media_attributes_table, media_overview, media_record_fields};
pub use merge::{DuplicatesTable, MergeCompareGrid, MergeScreen, merge_blocked_card, merge_wizard_foot};
pub use note::{NoteEditForm, note_content_tab, note_language_tab, note_record_fields, note_references_table};
pub use pedigree::{AncestorTreeView, DescendantTreeView, PedigreeScreen, RelationshipView};
pub use person::{
    EditForm, associations_table, events_table, facts_table, families_panel, names_table, overview_tab,
    person_name_citation_field, person_record_fields, timeline_panel,
};
pub use place::{
    PlaceEditForm, SuccessionFormState, place_geometry_table, place_hierarchy_table, place_map, place_map_as_of_note,
    place_names_table, place_overview, place_record_fields, place_succession_card, place_succession_form_fields,
};
pub use plugin_panel::{PluginPanelScreen, plugin_table, submit_outcome_view};
pub use preferences::{
    LocaleFields, MaintenanceFields, PreferencesScreen, ShortcutFields, SuretyFieldValues, SuretyFields, SuretySave,
    SuretyScope, preferences_view, surety_save, surety_scope_values,
};
pub use record_detail::{DockedRecordDetail, RecordDetail};
pub use record_form::{
    DraftCommit, RecordActionLabels, RecordEditState, apply_record_edits, finish_draft_commit, finish_record_save,
    record_edit_provenance, record_head_actions, record_keydown, record_restrictions_field, use_record_create,
    use_record_edit, use_save_on_request,
};
pub use repository::{
    RepositoryEditForm, repository_overview, repository_record_fields, repository_sources_table, repository_urls_table,
};
pub use research_note::{
    ResearchNoteCreateRecord, ResearchNoteEditForm, ResearchNotesTab, research_note_content_tab,
    research_note_draft_subjects, research_note_record_fields, research_note_subjects_table, research_notes_table,
};
pub use shared::{
    AttachLink, AttachPicker, CreateFormFocus, MediaTabState, RegisterFields, RetractTarget, RowRetract, RowVerb,
    attach_link_field, attach_link_form, citation_table, create_record_frame, create_record_header,
    finish_attach_create, id_list, media_gallery, media_tab, media_viewer_labels, non_empty, picker_selection_id,
    provenance_block, provenance_block_dna, provenance_claim_row, provenance_cue, register_fields_form,
    restriction_display, retract_panel, retract_side_panel, row_actions_cell, source_cue, source_media_type_choices,
    tag_chips, use_attach_picker, use_attach_save, use_detail_tab, use_existing_picker, use_record_step,
    use_record_undo,
};
pub use source::{
    SourceEditForm, source_attributes_table, source_citations_table, source_overview, source_record_fields,
    source_repositories_table,
};
pub use tabs::{
    AddressForm, ParticipationForm, ParticipationSeed, TabActionStyle, TabActionTarget, address_cards, address_form,
    citations_table, history_panel, participation_form, tab_frame, tags_panel,
};
pub use tag::{tag_edit_colour_card, tag_edit_tag_card, tag_overview, tag_usage_tab};
