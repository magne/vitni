//! SSR assertions for the Person detail tabs (Phase 5 PR4): render the rich Facts and Names tables
//! and assert the evidence-first cues (confidence badge colour token + redundant text, the
//! no-source flag, the source-count link) and the table roles. Pure render-and-inspect — no window,
//! no workspace — the same pattern as `components.rs`. The editing side panel's `role=dialog`
//! semantics are covered by the `SidePanel` gallery assertion in `components.rs`.

use dioxus::prelude::*;
use genealogy_app::TagRef;
use genealogy_app::{AssociationRole, FactType, NameType, ParticipantRole};
use genealogy_ui::{
    AssociationVm, AttachedRefVm, CitationRefVm, ConfidenceLevel, EventRefVm, EvidenceAxis, EvidenceAxisVm, FactVm,
    FamilyVm, Localizer, NameVm, PersonDraft, ProvenanceDraft,
};
use genealogy_ui_dioxus::screens::{
    RecordEditState, associations_table, events_table, facts_table, families_panel, id_list, names_table,
    person_citations_table, person_record_fields, person_tags_panel,
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
    let onretract = use_callback(|_| {});
    let onedit = use_callback(|_| {});
    rsx! {
        {names_table(&loc, &names, onedit, onretract)}
        {facts_table(&loc, &facts, onedit, onretract)}
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
fn collection_rows_carry_edit_and_retract_actions_with_row_scoped_names() {
    let mut vdom = VirtualDom::new(person_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    // Each name/fact row carries a Retract button with the mockup tooltip and a row-scoped
    // accessible name (a bare "Retract" is not descriptive enough).
    assert!(
        html.contains(r#"title="Retract this assertion — it stays in History""#),
        "the retract tooltip renders:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract Ada Lovelace""#),
        "the name row's retract names the row:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract Occupation""#),
        "the fact row's retract names the row:\n{html}"
    );
    // Each row also carries an Edit (=supersede) button, row-scoped.
    assert!(
        html.contains(r#"aria-label="Edit Ada Lovelace""#),
        "the name row's edit names the row:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Edit Occupation""#),
        "the fact row's edit names the row:\n{html}"
    );
    // No assertion UUID leaks into the rendered rows.
    assert!(
        !html.contains("0190a2b3-0000-7000-8000-000000000001"),
        "assertion UUIDs must not render:\n{html}"
    );
}

#[test]
fn a_participation_row_edit_changes_the_role() {
    let mut vdom = VirtualDom::new(person_relation_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    // The events-row Edit is a participation-role change with the mockup tooltip + row-scoped name.
    // (The apostrophe is HTML-escaped in the attribute, so match the stable prefix.)
    assert!(
        html.contains(r#"title="Change this participation"#),
        "the participation edit tooltip renders:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Edit Groom""#),
        "the participation edit names the row by role:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract Groom""#),
        "the participation row also offers Retract:\n{html}"
    );
}

#[test]
fn person_citations_offer_detach_and_none_offers_no_detach() {
    let mut vdom = VirtualDom::new(person_evidence_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    // The attached citation (assertion_id Some) offers Detach with the mockup tooltip; it is Detach,
    // never Retract (a citation is detached, not retracted).
    assert!(
        html.contains(r#"aria-label="Detach C0001""#),
        "the attached citation offers a row-scoped Detach:\n{html}"
    );
    assert!(
        html.contains(r#"title="Detach this citation — the detachment is recorded in History""#),
        "the detach tooltip renders:\n{html}"
    );
    // The evidence-only citation (assertion_id None) offers no Detach.
    assert!(
        !html.contains(r#"aria-label="Detach C0002""#),
        "an assertion_id: None citation offers no Detach:\n{html}"
    );
    // No attach-assertion UUID leaks.
    assert!(
        !html.contains("0190a2b3-0000-7000-8000-0000000000c1"),
        "attach-assertion UUIDs must not render:\n{html}"
    );
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
    let citations = vec![
        CitationRefVm {
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
            // An owner's own attachment: carries the attach assertion id → offers Detach.
            assertion_id: Some("0190a2b3-0000-7000-8000-0000000000c1".to_owned()),
        },
        CitationRefVm {
            human_id: "C0002".to_owned(),
            source: Some("S0002".to_owned()),
            source_id: Some("S0002".to_owned()),
            page: None,
            confidence: None,
            confidence_label: None,
            evidence_axes: Vec::new(),
            asserted_by: None,
            // Shown as evidence, not a detachable attachment: no Detach.
            assertion_id: None,
        },
    ];
    let onretract = use_callback(|_| {});
    let onedit = use_callback(|_| {});
    rsx! {
        {associations_table(&loc, &associations, onedit, onretract)}
        {person_citations_table(&loc, &citations, onretract)}
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
    let onretract = use_callback(|_| {});
    let onedit = use_callback(|_| {});
    rsx! {
        {events_table(&loc, &events, onedit, onretract)}
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

/// Renders the person Tags tab (the dispatching panel) over one applied tag.
fn person_tags() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None);
    let on_submit = use_callback(|_| {});
    let tags = vec![TagRef {
        id: "0190a2b3-0000-7000-8000-0000000000ff".to_owned(),
        name: "Direct ancestor".to_owned(),
        color: Some("#e5534b".to_owned()),
        priority: Some(1),
    }];
    person_tags_panel(&loc, &tags, editing, on_submit, "I0001")
}

#[test]
fn person_tags_panel_offers_add_and_a_named_untag_chip_without_the_tag_uuid() {
    let mut vdom = VirtualDom::new(person_tags);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    // "+ Add tag" affordance and the tag chip by name.
    assert!(html.contains("Add tag"), "the add-tag affordance renders:\n{html}");
    assert!(html.contains("Direct ancestor"), "the tag is shown by name:\n{html}");
    // The chip × is an icon button whose accessible name references the tag by name, with the
    // Untag tooltip; it dispatches Untag (never a Retract on a tag).
    assert!(
        html.contains(r#"aria-label="Remove tag Direct ancestor""#),
        "the remove button names the tag:\n{html}"
    );
    assert!(
        html.contains(r#"title="Untag — recorded in History""#),
        "the remove button carries the Untag tooltip:\n{html}"
    );
    // The tag's UUID is never rendered (data-model §9).
    assert!(
        !html.contains("0190a2b3-0000-7000-8000-0000000000ff"),
        "the tag UUID must not appear:\n{html}"
    );
}

/// Renders the notes list with a detach callback (the person Notes tab), then read-only (no callback).
fn person_notes_detachable() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let notes = vec![AttachedRefVm {
        human_id: "N0001".to_owned(),
        assertion_id: "0190a2b3-0000-7000-8000-0000000000n1".to_owned(),
    }];
    let ondetach = use_callback(|_| {});
    id_list(&loc, &notes, Some(ondetach))
}

#[test]
fn attached_notes_offer_a_row_scoped_detach() {
    let mut vdom = VirtualDom::new(person_notes_detachable);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(html.contains("N0001"), "the note is listed by human id:\n{html}");
    assert!(
        html.contains(r#"aria-label="Detach N0001""#),
        "the note row offers a row-scoped Detach:\n{html}"
    );
    assert!(
        html.contains(r#"title="Detach this note — the detachment is recorded in History""#),
        "the note detach tooltip renders:\n{html}"
    );
    assert!(
        !html.contains("0190a2b3-0000-7000-8000-0000000000n1"),
        "the attach-assertion UUID must not render:\n{html}"
    );
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
