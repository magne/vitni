//! Framework-free guard (issue #314 slice 3): a collection tab's [`ActionLabel`] is a property of the
//! `DetailTab` itself, declared once per aggregate in its `*_tabs()` builder — not chosen again at
//! each `tab_frame` call site, which is what once let six labels drift into rendering "Save". These
//! tests exercise the real `*_tabs()` fn for all 13 aggregates, so a new collection tab that forgets
//! its action fails to compile (the struct literal gains a required field) and, if it slips past that
//! via a fresh entry in one of these tables, fails here.

use vitni_ui::{
    ActionLabel, CitationDetail, DetailTab, DnaMatchDetail, DnaTestDetail, EventDetail, FamilyDetail, Localizer,
    MediaDetail, NoteDetail, PersonDetail, PersonDraft, PlaceDetail, RepositoryDetail, ResearchNoteDetail,
    SourceDetail, SourceReliabilityVm, TagDetail, citation_tabs, dna_match_tabs, dna_test_tabs, event_tabs,
    family_tabs, media_tabs, note_tabs, person_tabs, place_tabs, repository_tabs, research_note_tabs, source_tabs,
    tag_tabs,
};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

/// Looks up a tab's action by id — `None` both when the tab is missing and when it declares no
/// action, which is fine here: either way the caller's assertion should fail with a clear message.
fn action_of(tabs: &[DetailTab], id: &str) -> Option<ActionLabel> {
    tabs.iter().find(|tab| tab.id == id).and_then(|tab| tab.action)
}

fn assert_declares_action(tabs: &[DetailTab], ids: &[&str]) {
    for id in ids {
        assert!(action_of(tabs, id).is_some(), "{id:?} tab should declare an action");
    }
}

fn assert_no_action(tabs: &[DetailTab], ids: &[&str]) {
    for id in ids {
        assert!(action_of(tabs, id).is_none(), "{id:?} tab should be read-only");
    }
}

fn person_detail() -> PersonDetail {
    PersonDetail {
        human_id: "I0001".to_owned(),
        is_persona: false,
        evidence_level_label: String::new(),
        name: String::new(),
        given: None,
        surname: None,
        sex: String::new(),
        vitals: None,
        restrictions: Vec::new(),
        names: Vec::new(),
        facts: Vec::new(),
        events: Vec::new(),
        timeline: Vec::new(),
        associations: Vec::new(),
        families: Vec::new(),
        citations: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        research_notes: Vec::new(),
        history: Vec::new(),
        edit_seed: PersonDraft::new(),
    }
}

fn family_detail() -> FamilyDetail {
    FamilyDetail {
        human_id: "F0001".to_owned(),
        id: "family-id".to_owned(),
        title: String::new(),
        partners: Vec::new(),
        marriage: None,
        children: Vec::new(),
        events: Vec::new(),
        citations: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        restrictions: Vec::new(),
        research_notes: Vec::new(),
        history: Vec::new(),
    }
}

fn event_detail() -> EventDetail {
    EventDetail {
        human_id: "E0001".to_owned(),
        id: "event-id".to_owned(),
        title: String::new(),
        event_type: None,
        type_label: String::new(),
        date: None,
        date_value: None,
        date_confidence: None,
        date_confidence_label: None,
        date_source_count: 0,
        date_citations: Vec::new(),
        place: None,
        place_confidence: None,
        place_confidence_label: None,
        description: None,
        addresses: Vec::new(),
        participants: Vec::new(),
        citations: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        restrictions: Vec::new(),
        research_notes: Vec::new(),
        history: Vec::new(),
    }
}

fn place_detail() -> PlaceDetail {
    PlaceDetail {
        human_id: "P0001".to_owned(),
        id: "place-id".to_owned(),
        title: String::new(),
        place_type: None,
        type_label: None,
        coordinates: None,
        map_point: None,
        resolved_geometry: None,
        geometries: Vec::new(),
        coordinates_confidence: None,
        coordinates_confidence_label: None,
        coordinate_citations: Vec::new(),
        code: None,
        code_confidence: None,
        code_confidence_label: None,
        code_citations: Vec::new(),
        names: Vec::new(),
        hierarchy: Vec::new(),
        predecessors: Vec::new(),
        successors: Vec::new(),
        events: Vec::new(),
        citations: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        restrictions: Vec::new(),
        research_notes: Vec::new(),
        history: Vec::new(),
    }
}

fn source_detail() -> SourceDetail {
    SourceDetail {
        human_id: "S0001".to_owned(),
        id: "source-id".to_owned(),
        title: String::new(),
        author: None,
        pub_info: None,
        abbrev: None,
        repositories: Vec::new(),
        citations: Vec::new(),
        attributes: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        reliability: SourceReliabilityVm {
            confidence: None,
            confidence_label: None,
            evidence_axes: Vec::new(),
            citation_count: 0,
            record_count: 0,
        },
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

fn citation_detail() -> CitationDetail {
    CitationDetail {
        human_id: "C0001".to_owned(),
        source: None,
        page: None,
        date: None,
        date_value: None,
        confidence: None,
        confidence_label: None,
        source_quality: None,
        information: None,
        evidence_kind: None,
        evidence_axes: Vec::new(),
        restrictions: Vec::new(),
        attributes: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        history: Vec::new(),
    }
}

fn repository_detail() -> RepositoryDetail {
    RepositoryDetail {
        human_id: "R0001".to_owned(),
        id: "repository-id".to_owned(),
        title: String::new(),
        name: None,
        repository_type: None,
        type_label: None,
        addresses: Vec::new(),
        urls: Vec::new(),
        sources: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

fn media_detail() -> MediaDetail {
    MediaDetail {
        human_id: "O0001".to_owned(),
        id: "media-id".to_owned(),
        title: String::new(),
        path: None,
        file_path: None,
        web_path: None,
        mime: None,
        checksum: None,
        date: None,
        date_value: None,
        attributes: Vec::new(),
        citations: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        used_by: Vec::new(),
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

fn note_detail() -> NoteDetail {
    NoteDetail {
        human_id: "N0001".to_owned(),
        id: "note-id".to_owned(),
        title: String::new(),
        note_type: None,
        note_type_label: None,
        text: None,
        language: None,
        translations: Vec::new(),
        references: Vec::new(),
        tags: Vec::new(),
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

fn tag_detail() -> TagDetail {
    TagDetail {
        id: "tag-id".to_owned(),
        title: String::new(),
        name: None,
        color: None,
        priority: None,
        total: 0,
        usage: Vec::new(),
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

fn dna_test_detail() -> DnaTestDetail {
    DnaTestDetail {
        human_id: "D0001".to_owned(),
        id: "dna-test-id".to_owned(),
        title: String::new(),
        provider: None,
        provider_kind: None,
        test_type: None,
        test_type_kind: None,
        kit_id: None,
        genome_build: None,
        genome_build_kind: None,
        person: None,
        person_name: None,
        haplogroups: Vec::new(),
        matches: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

fn dna_match_detail() -> DnaMatchDetail {
    DnaMatchDetail {
        human_id: "X0001".to_owned(),
        id: "dna-match-id".to_owned(),
        title: String::new(),
        test_a: None,
        test_b: None,
        provider: None,
        shared_cm: None,
        percent_shared: None,
        largest_segment_cm: None,
        predicted_relationship: None,
        status: String::new(),
        status_kind: None,
        segments: Vec::new(),
        shared_ancestors: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        restrictions: Vec::new(),
        cited_by: Vec::new(),
        history: Vec::new(),
    }
}

fn research_note_detail() -> ResearchNoteDetail {
    ResearchNoteDetail {
        human_id: "A0001".to_owned(),
        id: "research-note-id".to_owned(),
        title: String::new(),
        body: None,
        language: None,
        subjects: Vec::new(),
        tags: Vec::new(),
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

/// Every collection tab across all 13 aggregates declares its own [`ActionLabel`] — the guard that a
/// new collection tab cannot ship without deciding its action. Read-only reverse-index tabs (overview,
/// map, timeline, history, families, usage, matches, content, references) and the ambiguous `events`
/// id (a real action for Family, a documented gap for Person) are covered by
/// [`a_read_only_tab_declares_no_action`] instead.
#[test]
fn every_collection_tab_declares_an_action() {
    let loc = loc();
    assert_declares_action(
        &person_tabs(&person_detail(), &loc),
        &[
            "names",
            "facts",
            "associations",
            "citations",
            "media",
            "notes",
            "research-notes",
            "tags",
        ],
    );
    assert_declares_action(
        &family_tabs(&family_detail(), &loc),
        &[
            "children",
            "events",
            "citations",
            "media",
            "notes",
            "research-notes",
            "tags",
        ],
    );
    assert_declares_action(
        &event_tabs(&event_detail(), &loc),
        &[
            "addresses",
            "participants",
            "citations",
            "media",
            "notes",
            "research-notes",
            "tags",
        ],
    );
    assert_declares_action(
        &place_tabs(&place_detail(), &loc),
        &[
            "names",
            "hierarchy",
            "citations",
            "media",
            "notes",
            "research-notes",
            "tags",
        ],
    );
    assert_declares_action(
        &source_tabs(&source_detail(), &loc),
        &["repositories", "attributes", "media", "notes", "tags"],
    );
    assert_declares_action(
        &citation_tabs(&citation_detail(), &loc),
        &["attributes", "media", "notes", "tags"],
    );
    assert_declares_action(
        &repository_tabs(&repository_detail(), &loc),
        &["addresses", "urls", "sources", "notes", "tags"],
    );
    assert_declares_action(
        &media_tabs(&media_detail(), &loc),
        &["attributes", "citations", "notes", "tags"],
    );
    assert_declares_action(&note_tabs(&note_detail(), &loc), &["language", "tags"]);
    assert_declares_action(
        &dna_test_tabs(&dna_test_detail(), &loc),
        &["haplogroups", "notes", "tags"],
    );
    assert_declares_action(
        &dna_match_tabs(&dna_match_detail(), &loc),
        &["segments", "ancestors", "notes", "tags"],
    );
    assert_declares_action(
        &research_note_tabs(&research_note_detail(), &loc),
        &["subjects", "tags"],
    );
}

/// The read-only tabs — no `.tab-actions` bar — across all 13 aggregates, plus Person's Events tab: a
/// known gap (`screens/person.rs` renders a bare table with no add action, while
/// `docs/mockups/person.html:275` shows "+ Participate in event" and no Fluent key exists yet).
#[test]
fn a_read_only_tab_declares_no_action() {
    let loc = loc();
    let person = person_tabs(&person_detail(), &loc);
    assert_no_action(&person, &["overview", "events", "families", "timeline", "history"]);
    assert_no_action(&family_tabs(&family_detail(), &loc), &["overview", "history"]);
    assert_no_action(&event_tabs(&event_detail(), &loc), &["overview", "history"]);
    assert_no_action(&place_tabs(&place_detail(), &loc), &["overview", "map", "history"]);
    assert_no_action(
        &source_tabs(&source_detail(), &loc),
        &["overview", "citations", "history"],
    );
    assert_no_action(&citation_tabs(&citation_detail(), &loc), &["overview", "history"]);
    assert_no_action(&repository_tabs(&repository_detail(), &loc), &["overview", "history"]);
    assert_no_action(&media_tabs(&media_detail(), &loc), &["overview", "history"]);
    assert_no_action(&note_tabs(&note_detail(), &loc), &["content", "references", "history"]);
    assert_no_action(&tag_tabs(&tag_detail(), &loc), &["overview", "usage", "history"]);
    assert_no_action(
        &dna_test_tabs(&dna_test_detail(), &loc),
        &["overview", "matches", "history"],
    );
    assert_no_action(&dna_match_tabs(&dna_match_detail(), &loc), &["overview", "history"]);
    assert_no_action(
        &research_note_tabs(&research_note_detail(), &loc),
        &["content", "history"],
    );
}
