//! Shared imports for the per-aggregate screen modules. Submodules glob this
//! (`use super::prelude::*;`); the `::prelude` suffix is exempt from `clippy::wildcard_imports`.
pub use dioxus::prelude::*;
pub use genealogy_app::{
    Address, ChildParentRelationship, DateParts, DnaProvider, EvidenceAnalysis, EvidenceKind, InformationKind,
    NameType, NoteType, ParticipantRole, PersonNameParts, RecentItem, Sex, SourceMediaType, SourceQuality, TagSummary,
    Url,
};
pub use genealogy_ui::{
    ActivityVm, AssociationVm, Category, CitationDetail, CitationEdit, CitationRefVm, CitingRecordVm, ConfidenceLevel,
    DashboardVm, Destination, DnaMatchDetail, DnaMatchEdit, DnaSegmentVm, DnaTestDetail, DnaTestEdit, DnaTestMatchVm,
    DraftCitation, DraftNameCitation, DuplicateCandidateVm, EventDetail, EventEdit, EventRefVm, FactVm, FamilyDetail,
    FamilyEdit, FamilyEventVm, FamilyMediaVm, FamilyVm, Intent, IntentOutcome, JumpVm, Localizer, MediaDetail,
    MediaEdit, MergeCompareVm, MergeFieldRowVm, MergePersons, NameVm, NoteDetail, NoteEdit, PersonChangeSetRequest,
    PersonDetail, PersonDraft, PersonEdit, PlaceDetail, PlaceEdit, RecordRef, RepositoryDetail, RepositoryEdit,
    RestrictionKind, RowVm, SharedAncestorVm, SourceCitationVm, SourceDetail, SourceEdit, TagDetail, TagEdit,
    TagUsageGroupVm, Tool, UsingRecordVm, citation_tabs, dna_match_tabs, dna_test_tabs, event_tabs, family_tabs,
    media_tabs, note_tabs, person_tabs, place_tabs, repository_tabs, source_tabs, tag_tabs,
};

pub use crate::app::{AppCtx, AppState};
pub use crate::components::{
    Button, ButtonVariant, Card, Chip, ConfidenceBadge, EmptyState, EvidenceAxisChip, HistoryEntry, HistoryTimeline,
    Input, NoSourceFlag, ProvenancePopover, RestrictionChoice, RestrictionSet, Select, SelectChoice, SidePanel,
    SourceLink, TabItem, Table, Toast,
};
pub use crate::master_detail::{DetailContainer, ListChrome, ListPane, MasterDetail};
pub use crate::services::{
    ScreenData, commit_person_change_set, create_citation_record, create_dna_match_record, create_dna_test_record,
    create_event_record, create_family_record, create_media_record, create_note_record, create_place_record,
    create_repository_record, create_source_record, create_tag_record, load_plugin_form, load_screen, load_tags,
    merge_persons, save_citation_edit, save_dna_match_edit, save_dna_test_edit, save_edit, save_event_edit,
    save_family_edit, save_media_edit, save_note_edit, save_place_edit, save_repository_edit, save_source_edit,
    save_tag_edit,
};
pub use crate::shell::ChromeCtx;
pub use crate::shell::nav_state::NavState;
pub use crate::vocabulary_render::FormView;

pub use super::shared::{
    JumpButton, RecordLink, citation_table, family_media_gallery, id_list, media_gallery, non_empty, provenance_cue,
    source_cue, source_media_type_choices, tags_panel,
};
