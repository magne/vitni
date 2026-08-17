//! The renderer's registry of the 12 detail aggregates — the single place the per-aggregate wiring of
//! a detail pane's commit path is enumerated.
//!
//! The same "x-macro" pattern `vitni_app`'s aggregate registry uses: the data lives once, here, and
//! each consumer defines a small *callback* macro that pattern-matches the columns it needs and hands
//! it to [`for_each_detail_aggregate!`]. Today that is the `save_*_edit` wrappers in
//! [`crate::services`] and the [`DetailAggregate`](crate::screens::DetailAggregate) impls behind the
//! shared commit hook. Adding an aggregate is a one-line edit here instead of a new copy of both.
//!
//! Edit types are written as fully-qualified paths so a consumer needs no imports of its own.

/// Invokes `$callback!` with one parenthesized row per detail aggregate. Columns, in order:
/// `(noun, Commits, Edit, save_fn, dispatch_fn)`.
///
/// - `noun` — the lower-case display noun used in generated doc comments.
/// - `Commits` — the marker type the generated
///   [`DetailAggregate`](crate::screens::DetailAggregate) impl hangs on.
/// - `Edit` — the aggregate's one-command edit enum (its `Tag` and `UndoAssertion` variants are what
///   the generated impl constructs).
/// - `save_fn` — the [`crate::services`] wrapper's name, generated there and called from the impl.
/// - `dispatch_fn` — the `vitni-ui` intent dispatcher that wrapper drives.
///
/// **Tag is deliberately absent.** It has no `*Edit` enum and no `save_tag_edit` — a tag's whole
/// record commits through `commit_tag_change_set`, its only removal is Untag from the *tagged*
/// record, and its `⌘Z` is already a no-op (`screens/tag.rs`). Twelve rows, not thirteen.
macro_rules! for_each_detail_aggregate {
    ($callback:ident) => {
        $callback! {
            ("person", PersonCommits, vitni_ui::PersonEdit, save_person_edit, vitni_ui::dispatch_person_edit),
            ("family", FamilyCommits, vitni_ui::FamilyEdit, save_family_edit, vitni_ui::dispatch_family_edit),
            ("event", EventCommits, vitni_ui::EventEdit, save_event_edit, vitni_ui::dispatch_event_edit),
            ("place", PlaceCommits, vitni_ui::PlaceEdit, save_place_edit, vitni_ui::dispatch_place_edit),
            ("source", SourceCommits, vitni_ui::SourceEdit, save_source_edit, vitni_ui::dispatch_source_edit),
            ("citation", CitationCommits, vitni_ui::CitationEdit, save_citation_edit, vitni_ui::dispatch_citation_edit),
            ("repository", RepositoryCommits, vitni_ui::RepositoryEdit, save_repository_edit, vitni_ui::dispatch_repository_edit),
            ("media object", MediaCommits, vitni_ui::MediaEdit, save_media_edit, vitni_ui::dispatch_media_edit),
            ("note", NoteCommits, vitni_ui::NoteEdit, save_note_edit, vitni_ui::dispatch_note_edit),
            ("research note", ResearchNoteCommits, vitni_ui::ResearchNoteEdit, save_research_note_edit, vitni_ui::dispatch_research_note_edit),
            ("DNA test", DnaTestCommits, vitni_ui::DnaTestEdit, save_dna_test_edit, vitni_ui::dispatch_dna_test_edit),
            ("DNA match", DnaMatchCommits, vitni_ui::DnaMatchEdit, save_dna_match_edit, vitni_ui::dispatch_dna_match_edit),
        }
    };
}

pub(crate) use for_each_detail_aggregate;
