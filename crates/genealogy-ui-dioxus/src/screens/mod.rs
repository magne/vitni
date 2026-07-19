//! The application's own screens (ADR 0008 §5): per-framework RSX over the shared `genealogy-ui`
//! view-models, built on the design-system components and the generic master-detail framework
//! (`crate::master_detail`). Each aggregate contributes a `*DetailPane` (its editor-host content,
//! routed by [`RecordDetail`]) and a `*CreateRecord` (its draft create form); the entity list is the
//! shell-level [`Explorer`](crate::shell::explorer::Explorer), and the tabstrip/dock/keyboard come
//! from the shell. The plugin panel renders a plugin-supplied form through the vocabulary interpreter.

mod citation;
mod dashboard;
mod dna_match;
mod dna_test;
mod event;
mod family;
mod help;
mod import;
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
mod shared;
mod source;
mod tabs;
mod tag;

pub use citation::{CitationEditForm, citation_attributes_table, citation_create_fields, citation_overview};
pub use dashboard::{DashboardScreen, dashboard_view};
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
pub use family::{
    FamilyEditForm, family_children_table, family_create_fields, family_events_table, family_overview,
    family_record_fields,
};
pub use help::{HelpScreen, render_doc};
pub use import::{
    ConfirmChrome, ConfirmStage, ImportRowStatus, ImportScreen, RecordsLabels, RecordsStage, SaveStage, SourceLabels,
    SourceStage, SummaryLabels, SummaryStage, WizardLabels,
};
pub use media::{MediaEditForm, media_attributes_table, media_overview, media_record_fields};
pub use merge::{DuplicatesTable, MergeCompareGrid, MergeScreen, merge_blocked_card, merge_wizard_foot};
pub use note::{NoteEditForm, note_content_tab, note_language_tab, note_record_fields, note_references_table};
pub use pedigree::{AncestorTreeView, DescendantTreeView, PedigreeScreen, RelationshipView};
pub use person::{
    EditForm, associations_table, events_table, facts_table, families_panel, names_table, overview_tab,
    person_name_citation_field, person_record_fields, timeline_panel,
};
pub use place::{
    PlaceEditForm, place_hierarchy_table, place_map, place_names_table, place_overview, place_record_fields,
};
pub use plugin_panel::{PluginPanelScreen, plugin_table, submit_outcome_view};
pub use preferences::{LocaleFields, PreferencesScreen, RegisterFields, preferences_view};
pub use record_detail::{DockedRecordDetail, RecordDetail};
pub use record_form::{
    RecordActionLabels, RecordEditState, apply_record_edits, finish_record_save, record_edit_provenance,
    record_head_actions, record_keydown, use_record_create, use_record_edit,
};
pub use repository::{
    RepositoryEditForm, repository_overview, repository_record_fields, repository_sources_table, repository_urls_table,
};
pub use shared::{
    MediaTabState, RetractTarget, RowRetract, attach_picker_form, citation_table, create_record_frame,
    create_record_header, id_list, media_gallery, media_tab, media_viewer_labels, non_empty, picker_selection_id,
    provenance_block, provenance_block_dna, provenance_claim_row, provenance_cue, retract_panel, retract_side_panel,
    row_actions_cell, source_cue, source_media_type_choices, tag_chips, use_existing_picker, use_record_step,
};
pub use source::{
    SourceEditForm, source_attributes_table, source_citations_table, source_overview, source_record_fields,
    source_repositories_table,
};
pub use tabs::{
    AddressForm, ParticipationForm, ParticipationSeed, address_cards, address_form, citations_table, history_panel,
    participation_form, tab_with_add, tags_panel,
};
pub use tag::{tag_edit_colour_card, tag_edit_tag_card, tag_overview, tag_usage_tab};
