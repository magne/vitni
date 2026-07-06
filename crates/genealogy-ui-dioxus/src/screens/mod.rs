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
mod repository;
mod shared;
mod source;
mod tag;

pub use citation::{
    CitationEditForm, CitationScreen, citation_attributes_table, citation_overview, citation_tags_panel,
};
pub use dashboard::{DashboardScreen, dashboard_view};
pub use dna_match::{
    DnaMatchEditForm, DnaMatchScreen, dna_match_ancestors_table, dna_match_overview, dna_match_segments_table,
    dna_match_tags_panel,
};
pub use dna_test::{
    DnaTestEditForm, DnaTestScreen, dna_test_haplogroups_table, dna_test_matches_table, dna_test_overview,
    dna_test_tags_panel,
};
pub use event::{EventEditForm, EventScreen, event_overview, event_participants_table, event_tags_panel};
pub use family::{
    FamilyEditForm, FamilyScreen, family_children_table, family_events_table, family_overview, family_tags_panel,
};
pub use help::{HelpScreen, render_doc};
pub use media::{MediaEditForm, MediaScreen, media_citations_table, media_overview, media_tags_panel};
pub use merge::{DuplicatesTable, MergeCompareGrid, MergeScreen};
pub use note::{NoteEditForm, NoteScreen, note_content_tab, note_language_tab, note_references_table, note_tags_panel};
pub use pedigree::{AncestorTreeView, DescendantTreeView, PedigreeScreen, RelationshipView};
pub use person::{
    PersonScreen, associations_table, events_table, facts_table, families_panel, names_table, overview_tab,
    person_citations_table,
};
pub use place::{
    PlaceEditForm, PlaceScreen, place_hierarchy_table, place_names_table, place_overview, place_tags_panel,
};
pub use plugin_panel::{PluginPanelScreen, plugin_table};
pub use preferences::{LocaleFields, PreferencesScreen, preferences_view};
pub use record_detail::RecordDetail;
pub use repository::{
    RepositoryEditForm, RepositoryScreen, repository_addresses_cards, repository_overview, repository_sources_table,
    repository_tags_panel, repository_urls_table,
};
pub use shared::{
    RecordActions, citation_table, create_record_header, family_media_gallery, id_list, media_gallery, non_empty,
    provenance_block, provenance_claim_row, provenance_cue, source_cue, source_media_type_choices, tags_panel,
};
pub use source::{
    SourceEditForm, SourceScreen, source_attributes_table, source_citations_table, source_create_fields,
    source_overview, source_repositories_table, source_tags_panel,
};
pub use tag::{TagScreen, tag_record_header, tag_usage_tab};
