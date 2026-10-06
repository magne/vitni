//! The canonical registry of the 14 aggregates for the persistence layer (issue #38).
//!
//! Both the engine-neutral facade ([`store`](crate::store)) and the SQLite backend
//! ([`sqlite`](crate::sqlite)) are per-aggregate-repetitive in lockstep: every aggregate needs the
//! same delegation methods, the same `cqrs-es` wiring, the same projection table, and the same
//! rebuild entry. This module is the single place that list lives; each consumer defines a callback
//! macro and hands it to [`for_each_db_aggregate!`] (all 14) or [`for_each_db_human_id_aggregate!`]
//! (the 12 that allocate a `HumanId` — every aggregate but Tag and `ImportRun`).
//!
//! Types are fully-qualified paths so a consumer needs no imports. The `wiring` column drives how
//! the SQLite backend assembles each aggregate's `CqrsFramework` in `open()`:
//! - `(plain)` — `sqlite_cqrs` with the unit `Services`;
//! - `(resolver <Path>)` — `sqlite_cqrs` with a projection-reading resolver (the §9 aggregate tax);
//! - `(event <Path>)` — hand-assembled so upcasters attach to the event store (ADR 0010).
//!
//! The `upcasters` column is the expression fed to a projection rebuild: `Vec::new()` for a stable
//! schema, `vitni_core::event::upcasters()` for the one aggregate whose events have evolved.

/// Invokes `$callback!` with one parenthesized row per aggregate. Columns, in order:
/// `(snake, State, View, Cmd, Err, table_const, table_str, execute_fn, find_fn, find_param,
/// list_fn, preview_fn, wiring, upcasters)`.
macro_rules! for_each_db_aggregate {
    ($callback:ident) => {
        $callback! {
            (person, vitni_core::person::PersonState, vitni_core::person::PersonView, vitni_core::person::PersonCommandEnvelope, vitni_core::person::PersonError, PERSON_VIEW_TABLE, "person_view", execute_person, find_person, human_id, list_persons, preview_person, (plain), Vec::new(),),
            (family, vitni_core::family::FamilyState, vitni_core::family::FamilyView, vitni_core::family::FamilyCommandEnvelope, vitni_core::family::FamilyError, FAMILY_VIEW_TABLE, "family_view", execute_family, find_family, human_id, list_families, preview_family, (plain), Vec::new(),),
            (place, vitni_core::place::PlaceState, vitni_core::place::PlaceView, vitni_core::place::PlaceCommandEnvelope, vitni_core::place::PlaceError, PLACE_VIEW_TABLE, "place_view", execute_place, find_place, human_id, list_places, preview_place, (resolver crate::resolver::PlaceRefService), Vec::new(),),
            (source, vitni_core::source::SourceState, vitni_core::source::SourceView, vitni_core::source::SourceCommandEnvelope, vitni_core::source::SourceError, SOURCE_VIEW_TABLE, "source_view", execute_source, find_source, human_id, list_sources, preview_source, (resolver crate::resolver::SourceRefService), Vec::new(),),
            (citation, vitni_core::citation::CitationState, vitni_core::citation::CitationView, vitni_core::citation::CitationCommandEnvelope, vitni_core::citation::CitationError, CITATION_VIEW_TABLE, "citation_view", execute_citation, find_citation, human_id, list_citations, preview_citation, (resolver crate::resolver::CitationRefService), Vec::new(),),
            (event, vitni_core::event::EventState, vitni_core::event::EventView, vitni_core::event::EventCommandEnvelope, vitni_core::event::EventError, EVENT_VIEW_TABLE, "event_view", execute_event, find_event, human_id, list_events, preview_event, (event crate::resolver::EventRefService), vitni_core::event::upcasters(),),
            (dna_test, vitni_core::dna_test::DnaTestState, vitni_core::dna_test::DnaTestView, vitni_core::dna_test::DnaTestCommandEnvelope, vitni_core::dna_test::DnaTestError, DNA_TEST_VIEW_TABLE, "dna_test_view", execute_dna_test, find_dna_test, human_id, list_dna_tests, preview_dna_test, (resolver crate::resolver::DnaTestRefService), Vec::new(),),
            (dna_match, vitni_core::dna_match::DnaMatchState, vitni_core::dna_match::DnaMatchView, vitni_core::dna_match::DnaMatchCommandEnvelope, vitni_core::dna_match::DnaMatchError, DNA_MATCH_VIEW_TABLE, "dna_match_view", execute_dna_match, find_dna_match, human_id, list_dna_matches, preview_dna_match, (resolver crate::resolver::DnaMatchRefService), Vec::new(),),
            (repository, vitni_core::repository::RepositoryState, vitni_core::repository::RepositoryView, vitni_core::repository::RepositoryCommandEnvelope, vitni_core::repository::RepositoryError, REPOSITORY_VIEW_TABLE, "repository_view", execute_repository, find_repository, human_id, list_repositories, preview_repository, (plain), Vec::new(),),
            (note, vitni_core::note::NoteState, vitni_core::note::NoteView, vitni_core::note::NoteCommandEnvelope, vitni_core::note::NoteError, NOTE_VIEW_TABLE, "note_view", execute_note, find_note, human_id, list_notes, preview_note, (plain), Vec::new(),),
            (media, vitni_core::media::MediaState, vitni_core::media::MediaView, vitni_core::media::MediaCommandEnvelope, vitni_core::media::MediaError, MEDIA_VIEW_TABLE, "media_view", execute_media, find_media, human_id, list_media, preview_media, (plain), Vec::new(),),
            (tag, vitni_core::tag::TagState, vitni_core::tag::TagView, vitni_core::tag::TagCommandEnvelope, vitni_core::tag::TagError, TAG_VIEW_TABLE, "tag_view", execute_tag, find_tag, tag_id, list_tags, preview_tag, (plain), Vec::new(),),
            (research_note, vitni_core::research_note::ResearchNoteState, vitni_core::research_note::ResearchNoteView, vitni_core::research_note::ResearchNoteCommandEnvelope, vitni_core::research_note::ResearchNoteError, RESEARCH_NOTE_VIEW_TABLE, "research_note_view", execute_research_note, find_research_note, human_id, list_research_notes, preview_research_note, (resolver crate::resolver::ResearchNoteRefService), Vec::new(),),
            (import_run, vitni_core::import_run::ImportRunState, vitni_core::import_run::ImportRunView, vitni_core::import_run::ImportRunCommandEnvelope, vitni_core::import_run::ImportRunError, IMPORT_RUN_VIEW_TABLE, "import_run_view", execute_import_run, find_import_run, run_id, list_import_runs, preview_import_run, (plain), Vec::new(),),
        }
    };
}

pub(crate) use for_each_db_aggregate;

/// Invokes `$callback!` with one row per aggregate that allocates a `HumanId` (all but Tag and `ImportRun`).
/// Columns, in order: `(snake, next_fn, table_const)`.
macro_rules! for_each_db_human_id_aggregate {
    ($callback:ident) => {
        $callback! {
            (person, next_person_human_id, PERSON_VIEW_TABLE),
            (family, next_family_human_id, FAMILY_VIEW_TABLE),
            (place, next_place_human_id, PLACE_VIEW_TABLE),
            (source, next_source_human_id, SOURCE_VIEW_TABLE),
            (citation, next_citation_human_id, CITATION_VIEW_TABLE),
            (event, next_event_human_id, EVENT_VIEW_TABLE),
            (dna_test, next_dna_test_human_id, DNA_TEST_VIEW_TABLE),
            (dna_match, next_dna_match_human_id, DNA_MATCH_VIEW_TABLE),
            (repository, next_repository_human_id, REPOSITORY_VIEW_TABLE),
            (note, next_note_human_id, NOTE_VIEW_TABLE),
            (media, next_media_human_id, MEDIA_VIEW_TABLE),
            (research_note, next_research_note_human_id, RESEARCH_NOTE_VIEW_TABLE),
        }
    };
}

pub(crate) use for_each_db_human_id_aggregate;

/// Invokes `$callback!` with one row per aggregate that carries `ExternalId`s — the re-import
/// resolution key (data-model §11). Columns, in order: `(snake, find_fn, table_const, View)`.
/// Grows as more aggregates are wired for import (Source/Citation/Media in later PR 2 commits).
macro_rules! for_each_db_external_id_aggregate {
    ($callback:ident) => {
        $callback! {
            (person, find_person_by_external_id, PERSON_VIEW_TABLE, vitni_core::person::PersonView),
            (family, find_family_by_external_id, FAMILY_VIEW_TABLE, vitni_core::family::FamilyView),
        }
    };
}

pub(crate) use for_each_db_external_id_aggregate;

/// Invokes `$callback!` with one row per aggregate the matching profiles read by id (ADR 0038 §2) —
/// the matchable kinds. Columns, in order: `(snake, by_ids_fn, table_const, View)`.
macro_rules! for_each_db_by_ids_aggregate {
    ($callback:ident) => {
        $callback! {
            (person, persons_by_ids, PERSON_VIEW_TABLE, vitni_core::person::PersonView),
            (family, families_by_ids, FAMILY_VIEW_TABLE, vitni_core::family::FamilyView),
            (event, events_by_ids, EVENT_VIEW_TABLE, vitni_core::event::EventView),
            (place, places_by_ids, PLACE_VIEW_TABLE, vitni_core::place::PlaceView),
            (source, sources_by_ids, SOURCE_VIEW_TABLE, vitni_core::source::SourceView),
            (repository, repositories_by_ids, REPOSITORY_VIEW_TABLE, vitni_core::repository::RepositoryView),
            (citation, citations_by_ids, CITATION_VIEW_TABLE, vitni_core::citation::CitationView),
            (media, media_by_ids, MEDIA_VIEW_TABLE, vitni_core::media::MediaView),
            (note, notes_by_ids, NOTE_VIEW_TABLE, vitni_core::note::NoteView),
            (tag, tags_by_ids, TAG_VIEW_TABLE, vitni_core::tag::TagView),
        }
    };
}

pub(crate) use for_each_db_by_ids_aggregate;
