//! The canonical registry of the 12 aggregates for the persistence layer (issue #38).
//!
//! Both the engine-neutral facade ([`store`](crate::store)) and the SQLite backend
//! ([`sqlite`](crate::sqlite)) are per-aggregate-repetitive in lockstep: every aggregate needs the
//! same delegation methods, the same `cqrs-es` wiring, the same projection table, and the same
//! rebuild entry. This module is the single place that list lives; each consumer defines a callback
//! macro and hands it to [`for_each_db_aggregate!`] (all 12) or [`for_each_db_human_id_aggregate!`]
//! (the 11 that allocate a `HumanId` — every aggregate but Tag).
//!
//! Types are fully-qualified paths so a consumer needs no imports. The `wiring` column drives how
//! the SQLite backend assembles each aggregate's `CqrsFramework` in `open()`:
//! - `(plain)` — `sqlite_cqrs` with the unit `Services`;
//! - `(resolver <Path>)` — `sqlite_cqrs` with a projection-reading resolver (the §9 aggregate tax);
//! - `(event <Path>)` — hand-assembled so upcasters attach to the event store (ADR 0010).
//!
//! The `upcasters` column is the expression fed to a projection rebuild: `Vec::new()` for a stable
//! schema, `genealogy_core::event::upcasters()` for the one aggregate whose events have evolved.

/// Invokes `$callback!` with one parenthesized row per aggregate. Columns, in order:
/// `(snake, State, View, Cmd, Err, table_const, table_str, execute_fn, find_fn, find_param,
/// list_fn, wiring, upcasters)`.
macro_rules! for_each_db_aggregate {
    ($callback:ident) => {
        $callback! {
            (person, genealogy_core::person::PersonState, genealogy_core::person::PersonView, genealogy_core::person::PersonCommandEnvelope, genealogy_core::person::PersonError, PERSON_VIEW_TABLE, "person_view", execute_person, find_person, human_id, list_persons, (plain), Vec::new(),),
            (family, genealogy_core::family::FamilyState, genealogy_core::family::FamilyView, genealogy_core::family::FamilyCommandEnvelope, genealogy_core::family::FamilyError, FAMILY_VIEW_TABLE, "family_view", execute_family, find_family, human_id, list_families, (plain), Vec::new(),),
            (place, genealogy_core::place::PlaceState, genealogy_core::place::PlaceView, genealogy_core::place::PlaceCommandEnvelope, genealogy_core::place::PlaceError, PLACE_VIEW_TABLE, "place_view", execute_place, find_place, human_id, list_places, (resolver crate::resolver::PlaceRefService), Vec::new(),),
            (source, genealogy_core::source::SourceState, genealogy_core::source::SourceView, genealogy_core::source::SourceCommandEnvelope, genealogy_core::source::SourceError, SOURCE_VIEW_TABLE, "source_view", execute_source, find_source, human_id, list_sources, (resolver crate::resolver::SourceRefService), Vec::new(),),
            (citation, genealogy_core::citation::CitationState, genealogy_core::citation::CitationView, genealogy_core::citation::CitationCommandEnvelope, genealogy_core::citation::CitationError, CITATION_VIEW_TABLE, "citation_view", execute_citation, find_citation, human_id, list_citations, (resolver crate::resolver::CitationRefService), Vec::new(),),
            (event, genealogy_core::event::EventState, genealogy_core::event::EventView, genealogy_core::event::EventCommandEnvelope, genealogy_core::event::EventError, EVENT_VIEW_TABLE, "event_view", execute_event, find_event, human_id, list_events, (event crate::resolver::EventRefService), genealogy_core::event::upcasters(),),
            (dna_test, genealogy_core::dna_test::DnaTestState, genealogy_core::dna_test::DnaTestView, genealogy_core::dna_test::DnaTestCommandEnvelope, genealogy_core::dna_test::DnaTestError, DNA_TEST_VIEW_TABLE, "dna_test_view", execute_dna_test, find_dna_test, human_id, list_dna_tests, (resolver crate::resolver::DnaTestRefService), Vec::new(),),
            (dna_match, genealogy_core::dna_match::DnaMatchState, genealogy_core::dna_match::DnaMatchView, genealogy_core::dna_match::DnaMatchCommandEnvelope, genealogy_core::dna_match::DnaMatchError, DNA_MATCH_VIEW_TABLE, "dna_match_view", execute_dna_match, find_dna_match, human_id, list_dna_matches, (resolver crate::resolver::DnaMatchRefService), Vec::new(),),
            (repository, genealogy_core::repository::RepositoryState, genealogy_core::repository::RepositoryView, genealogy_core::repository::RepositoryCommandEnvelope, genealogy_core::repository::RepositoryError, REPOSITORY_VIEW_TABLE, "repository_view", execute_repository, find_repository, human_id, list_repositories, (plain), Vec::new(),),
            (note, genealogy_core::note::NoteState, genealogy_core::note::NoteView, genealogy_core::note::NoteCommandEnvelope, genealogy_core::note::NoteError, NOTE_VIEW_TABLE, "note_view", execute_note, find_note, human_id, list_notes, (plain), Vec::new(),),
            (media, genealogy_core::media::MediaState, genealogy_core::media::MediaView, genealogy_core::media::MediaCommandEnvelope, genealogy_core::media::MediaError, MEDIA_VIEW_TABLE, "media_view", execute_media, find_media, human_id, list_media, (plain), Vec::new(),),
            (tag, genealogy_core::tag::TagState, genealogy_core::tag::TagView, genealogy_core::tag::TagCommandEnvelope, genealogy_core::tag::TagError, TAG_VIEW_TABLE, "tag_view", execute_tag, find_tag, tag_id, list_tags, (plain), Vec::new(),),
        }
    };
}

pub(crate) use for_each_db_aggregate;

/// Invokes `$callback!` with one row per aggregate that allocates a `HumanId` (all but Tag).
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
            (person, find_person_by_external_id, PERSON_VIEW_TABLE, genealogy_core::person::PersonView),
            (family, find_family_by_external_id, FAMILY_VIEW_TABLE, genealogy_core::family::FamilyView),
        }
    };
}

pub(crate) use for_each_db_external_id_aggregate;
