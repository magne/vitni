//! The canonical registry of the 12 aggregates — the single place the per-aggregate wiring of the
//! application layer is enumerated (issue #38).
//!
//! Adding an aggregate is a one-line edit here instead of parallel edits scattered across
//! [`error`](crate::error), [`session`](crate::session), [`config`](crate::config), and
//! [`workspace`](crate::workspace). Each of those modules defines a small *callback* macro that
//! pattern-matches the columns it needs and hands it to [`for_each_aggregate!`] (all 12) or
//! [`for_each_human_id_aggregate!`] (the 11 that allocate a `HumanId` — every aggregate but Tag).
//!
//! This is the classic "x-macro" pattern: the data lives once, here; each consumer decides what to
//! generate from it. Identifier and error types are written as fully-qualified paths so a consumer
//! needs no imports of its own.

/// Invokes `$callback!` with one parenthesized row per aggregate. Columns, in order:
/// `(snake, noun, IdType, minter_fn, ErrorType, domain_variant, not_found_variant, not_found_msg)`.
///
/// - `noun` — the lower-case display noun used in generated doc comments.
/// - `minter_fn` — the [`Session`](crate::session::Session) id-minting method name.
/// - `domain_variant` / `not_found_variant` — the [`AppError`](crate::error::AppError) variant names
///   (Person's domain wrapper is historically `Domain`, the rest are `<Name>Domain`).
/// - `not_found_msg` — the `thiserror` `#[error(...)]` string (Tag is keyed by `id`, not `human_id`).
macro_rules! for_each_aggregate {
    ($callback:ident) => {
        $callback! {
            (person, "person", genealogy_core::ids::PersonId, new_person_id, genealogy_core::person::PersonError, Domain, PersonNotFound, "no person with human_id {0:?}"),
            (family, "family", genealogy_core::ids::FamilyId, new_family_id, genealogy_core::family::FamilyError, FamilyDomain, FamilyNotFound, "no family with human_id {0:?}"),
            (place, "place", genealogy_core::ids::PlaceId, new_place_id, genealogy_core::place::PlaceError, PlaceDomain, PlaceNotFound, "no place with human_id {0:?}"),
            (source, "source", genealogy_core::ids::SourceId, new_source_id, genealogy_core::source::SourceError, SourceDomain, SourceNotFound, "no source with human_id {0:?}"),
            (citation, "citation", genealogy_core::ids::CitationId, new_citation_id, genealogy_core::citation::CitationError, CitationDomain, CitationNotFound, "no citation with human_id {0:?}"),
            (event, "event", genealogy_core::ids::EventId, new_event_id, genealogy_core::event::EventError, EventDomain, EventNotFound, "no event with human_id {0:?}"),
            (dna_test, "DNA test", genealogy_core::ids::DnaTestId, new_dna_test_id, genealogy_core::dna_test::DnaTestError, DnaTestDomain, DnaTestNotFound, "no dna test with human_id {0:?}"),
            (dna_match, "DNA match", genealogy_core::ids::DnaMatchId, new_dna_match_id, genealogy_core::dna_match::DnaMatchError, DnaMatchDomain, DnaMatchNotFound, "no dna match with human_id {0:?}"),
            (repository, "repository", genealogy_core::ids::RepositoryId, new_repository_id, genealogy_core::repository::RepositoryError, RepositoryDomain, RepositoryNotFound, "no repository with human_id {0:?}"),
            (note, "note", genealogy_core::ids::NoteId, new_note_id, genealogy_core::note::NoteError, NoteDomain, NoteNotFound, "no note with human_id {0:?}"),
            (media, "media", genealogy_core::ids::MediaId, new_media_id, genealogy_core::media::MediaError, MediaDomain, MediaNotFound, "no media with human_id {0:?}"),
            (tag, "tag", genealogy_core::ids::TagId, new_tag_id, genealogy_core::tag::TagError, TagDomain, TagNotFound, "no tag with id {0:?}"),
        }
    };
}

pub(crate) use for_each_aggregate;

/// Invokes `$callback!` with one parenthesized row per aggregate that allocates a `HumanId` — every
/// aggregate except Tag (tags are keyed by their own id). Columns, in order:
/// `(snake, noun, default_format, id_format_accessor)`.
///
/// - `noun` — the display noun used in generated doc comments (back-ticked where it is a type name).
/// - `default_format` — the Gramps-style printf default for that aggregate's `HumanId`.
/// - `id_format_accessor` — the [`Workspace`](crate::workspace::Workspace) effective-format method.
macro_rules! for_each_human_id_aggregate {
    ($callback:ident) => {
        $callback! {
            (person, "Person", "I%04d", person_id_format),
            (family, "Family", "F%04d", family_id_format),
            (place, "Place", "P%04d", place_id_format),
            (source, "Source", "S%04d", source_id_format),
            (citation, "Citation", "C%04d", citation_id_format),
            (event, "Event", "E%04d", event_id_format),
            (dna_test, "`DnaTest`", "D%04d", dna_test_id_format),
            (dna_match, "`DnaMatch`", "X%04d", dna_match_id_format),
            (repository, "Repository", "R%04d", repository_id_format),
            (note, "Note", "N%04d", note_id_format),
            (media, "Media", "O%04d", media_id_format),
        }
    };
}

pub(crate) use for_each_human_id_aggregate;
