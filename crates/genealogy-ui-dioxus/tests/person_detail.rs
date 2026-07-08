//! SSR assertions for the Person detail tabs (Phase 5 PR4): render the rich Facts and Names tables
//! and assert the evidence-first cues (confidence badge colour token + redundant text, the
//! no-source flag, the source-count link) and the table roles. Pure render-and-inspect — no window,
//! no workspace — the same pattern as `components.rs`. The editing side panel's `role=dialog`
//! semantics are covered by the `SidePanel` gallery assertion in `components.rs`.

use dioxus::prelude::*;
use genealogy_app::{AssociationRole, FactType, NameType, ParticipantRole};
use genealogy_ui::{
    AssociationVm, CitationRefVm, ConfidenceLevel, EventRefVm, EvidenceAxis, EvidenceAxisVm, FactVm, FamilyVm,
    Localizer, NameVm, PersonDraft, ProvenanceDraft,
};
use genealogy_ui_dioxus::screens::{
    RecordEditState, associations_table, events_table, facts_table, families_panel, names_table,
    person_citations_table, person_record_fields,
};
use genealogy_ui_dioxus::shell::nav_state::NavState;

/// Renders the Names + Facts tables over representative view-models, in English.
fn person_tables() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let names = vec![NameVm {
        type_label: "Birth name".to_owned(),
        display: "Ada Lovelace".to_owned(),
        given: Some("Ada".to_owned()),
        surname: Some("Lovelace".to_owned()),
        surname_prefix: None,
        name_prefix: None,
        suffix: None,
        name_type: NameType::BirthName,
        nickname: None,
        date: None,
        language: Some("en".to_owned()),
        confidence: ConfidenceLevel::High,
        confidence_label: "High".to_owned(),
        source_count: 1,
        assertion_id: "0190a2b3-0000-7000-8000-000000000001".to_owned(),
    }];
    let facts = vec![
        FactVm {
            type_label: "Occupation".to_owned(),
            fact_type: FactType::Occupation,
            value: Some("Mathematician".to_owned()),
            date: None,
            confidence: ConfidenceLevel::High,
            confidence_label: "High".to_owned(),
            source_count: 2,
            citations: Vec::new(),
            assertion_id: "0190a2b3-0000-7000-8000-000000000002".to_owned(),
        },
        FactVm {
            type_label: "Birth".to_owned(),
            fact_type: FactType::Birth,
            value: None,
            date: Some("1815".to_owned()),
            confidence: ConfidenceLevel::Low,
            confidence_label: "Low".to_owned(),
            source_count: 0,
            citations: Vec::new(),
            assertion_id: "0190a2b3-0000-7000-8000-000000000003".to_owned(),
        },
    ];
    rsx! {
        {names_table(&loc, &names)}
        {facts_table(&loc, &facts)}
    }
}

#[test]
fn facts_and_names_render_tables_with_evidence_cues() {
    let mut vdom = VirtualDom::new(person_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        r#"class="tbl""#,       // both render as the design-system Table
        "Ada Lovelace",         // the name display
        "Mathematician",        // the fact value
        r#"data-level="high""#, // confidence colour token (the high-confidence fact)
        ">High",                // confidence label text — colour is never the only signal
        "2 sources",            // the sourced fact's source-count link
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn an_unsourced_fact_shows_the_no_source_flag() {
    let mut vdom = VirtualDom::new(person_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    // The birth fact has no citations: the flag carries an icon AND text (not colour alone).
    assert!(html.contains(r#"class="no-source""#), "no-source flag class:\n{html}");
    assert!(html.contains("No source"), "no-source flag text:\n{html}");
}

/// Renders the Associations table and the Citations tab over representative view-models.
fn person_evidence_tables() -> Element {
    // RecordLink resolves NavState from context, so the harness must provide it.
    use_context_provider(NavState::new);
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let associations = vec![AssociationVm {
        other_id: "I0002".to_owned(),
        role: AssociationRole::Godparent,
        role_label: "Godparent".to_owned(),
        confidence: ConfidenceLevel::Low,
        confidence_label: "Low".to_owned(),
        source_count: 0,
        assertion_id: "0190a2b3-0000-7000-8000-000000000004".to_owned(),
    }];
    let citations = vec![CitationRefVm {
        human_id: "C0001".to_owned(),
        source: Some("S0001".to_owned()),
        source_id: Some("S0001".to_owned()),
        page: Some("p. 42".to_owned()),
        confidence: Some(ConfidenceLevel::High),
        confidence_label: Some("High".to_owned()),
        evidence_axes: vec![EvidenceAxisVm {
            axis: EvidenceAxis::Source,
            label: "Original".to_owned(),
        }],
        asserted_by: Some("asserted by magne · 2026-06-22 14:35".to_owned()),
        assertion_id: None,
    }];
    rsx! {
        {associations_table(&loc, &associations)}
        {person_citations_table(&loc, &citations)}
    }
}

/// Renders the Events + Families tabs, whose related records are clickable links.
fn person_relation_tables() -> Element {
    // RecordLink resolves NavState from context, so the harness must provide it.
    use_context_provider(NavState::new);
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let events = vec![EventRefVm {
        event_id: "E0007".to_owned(),
        role: ParticipantRole::Groom,
        role_label: "Groom".to_owned(),
        date: Some("1876".to_owned()),
        assertion_id: "0190a2b3-0000-7000-8000-000000000005".to_owned(),
    }];
    let families = vec![FamilyVm {
        family_id: "F0017".to_owned(),
        role_label: "Partner".to_owned(),
        partners: vec!["I0002".to_owned()],
        children: vec![("I0061".to_owned(), "Birth".to_owned())],
    }];
    rsx! {
        {events_table(&loc, &events)}
        {families_panel(&loc, &families)}
    }
}

#[test]
fn events_and_families_link_their_records() {
    let mut vdom = VirtualDom::new(person_relation_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    // Each related record is a clickable link (the shared RecordLink), not plain text: the event,
    // partner and child render as inline `src-link`s; the family as a `btn` chip.
    let links = html.matches(r#"class="src-link""#).count();
    assert!(links >= 3, "event, partner and child are inline links:\n{html}");
    for needle in ["E0007", "F0017", "I0002", "I0061"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn associations_and_citations_carry_evidence_cues() {
    let mut vdom = VirtualDom::new(person_evidence_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "Godparent",            // the association role
        r#"data-level="low""#,  // the association's surety token
        r#"class="no-source""#, // the unsourced association's flag
        "S0001",                // the backing citation's source
        "ev source",            // the citation's evidence-axis chip
        "Original",             // the axis value
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

/// A person draft in edit mode, seeded with the current human id `I0001`.
fn seeded_person() -> PersonDraft {
    let mut draft = PersonDraft::new();
    draft.existing_human_id = Some("I0001".to_owned());
    "I0001".clone_into(&mut draft.human_id_override);
    draft
}

/// Renders the person record fields in edit mode over a draft seeded with the current human id.
fn person_edit_fields() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let state = RecordEditState {
        editing: use_signal(|| true),
        seed: use_signal(seeded_person),
        draft: use_signal(seeded_person),
        prov: use_signal(ProvenanceDraft::default),
    };
    person_record_fields(&loc, state)
}

#[test]
fn person_edit_mode_offers_an_editable_human_id_with_a_regenerate_hint() {
    let mut vdom = VirtualDom::new(person_edit_fields);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"id="human-id""#),
        "an editable human-id input is present:\n{html}"
    );
    assert!(
        html.contains(r#"value="I0001""#),
        "the input is seeded with the current id:\n{html}"
    );
    assert!(
        html.contains("field-hint"),
        "the regenerate hint element is present:\n{html}"
    );
    assert!(
        html.contains("Leave empty to generate"),
        "the hint explains that clearing the id regenerates it:\n{html}"
    );
}
