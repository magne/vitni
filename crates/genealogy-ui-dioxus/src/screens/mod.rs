//! The application's own screens (ADR 0008 §5): per-framework RSX over the shared `genealogy-ui`
//! view-models, built on the design-system components and the generic master-detail framework
//! (`crate::master_detail`). [`PersonScreen`] is the reference slice — adding an aggregate copies it:
//! supply a row builder (`genealogy_ui::person_row`-style), a tab builder
//! (`genealogy_ui::person_tabs`-style), and the tab-id→content match below; the list/detail layout,
//! search, sort, and keyboard come from the framework. The plugin panel renders a plugin-supplied
//! form through the vocabulary interpreter.

mod citation;
mod dashboard;
mod dna_match;
mod dna_test;
mod event;
mod family;
mod help;
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
mod tag;

pub use citation::{
    CitationEditForm, CitationScreen, citation_attributes_table, citation_create_fields, citation_overview,
    citation_tags_panel,
};
pub use dashboard::{DashboardScreen, dashboard_view};
pub use dna_match::{
    DnaMatchEditForm, DnaMatchScreen, dna_match_ancestors_table, dna_match_create_fields, dna_match_overview,
    dna_match_record_fields, dna_match_segments_table, dna_match_tags_panel,
};
pub use dna_test::{
    DnaTestEditForm, DnaTestScreen, dna_test_create_fields, dna_test_haplogroups_table, dna_test_matches_table,
    dna_test_overview, dna_test_record_fields, dna_test_tags_panel,
};
pub use event::{
    EventEditCtx, EventEditForm, EventScreen, event_create_fields, event_overview, event_participants_table,
    event_record_fields, event_tags_panel,
};
pub use family::{
    FamilyEditForm, FamilyScreen, family_children_table, family_create_fields, family_events_table, family_overview,
    family_record_fields, family_tags_panel,
};
pub use help::{HelpScreen, render_doc};
pub use media::{
    MediaEditForm, MediaScreen, media_citations_table, media_overview, media_record_fields, media_tags_panel,
};
pub use merge::{DuplicatesTable, MergeCompareGrid, MergeScreen};
pub use note::{
    NoteEditForm, NoteScreen, note_content_tab, note_language_tab, note_record_fields, note_references_table,
    note_tags_panel,
};
pub use pedigree::{AncestorTreeView, DescendantTreeView, PedigreeScreen, RelationshipView};
pub use person::{
    PersonScreen, associations_table, events_table, facts_table, families_panel, names_table, overview_tab,
    person_citations_table, person_name_citation_field, person_record_fields,
};
pub use place::{
    PlaceEditForm, PlaceScreen, place_hierarchy_table, place_names_table, place_overview, place_record_fields,
    place_tags_panel,
};
pub use plugin_panel::{PluginPanelScreen, plugin_table};
pub use preferences::{LocaleFields, PreferencesScreen, preferences_view};
pub use record_detail::RecordDetail;
pub use record_form::{
    RecordActionLabels, RecordEditState, apply_record_edits, finish_record_save, record_edit_provenance,
    record_head_actions, record_keydown, use_record_create, use_record_edit,
};
pub use repository::{
    RepositoryEditForm, RepositoryScreen, repository_addresses_cards, repository_overview, repository_record_fields,
    repository_sources_table, repository_tags_panel, repository_urls_table,
};
pub use shared::{
    attach_picker_form, citation_table, create_record_header, family_media_gallery, id_list, media_gallery, non_empty,
    picker_selection_id, provenance_block, provenance_claim_row, provenance_cue, source_cue, source_media_type_choices,
    tags_panel, use_existing_picker,
};
pub use source::{
    SourceEditForm, SourceScreen, source_attributes_table, source_citations_table, source_overview,
    source_record_fields, source_repositories_table, source_tags_panel,
};
pub use tag::{TagScreen, tag_edit_colour_card, tag_edit_tag_card, tag_overview, tag_usage_tab};
