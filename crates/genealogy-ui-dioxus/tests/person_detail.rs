//! SSR assertions for the Person detail tabs (Phase 5 PR4): render the rich Facts and Names tables
//! and assert the evidence-first cues (confidence badge colour token + redundant text, the
//! no-source flag, the source-count link) and the table roles. Pure render-and-inspect — no window,
//! no workspace — the same pattern as `components.rs`. The editing side panel's `role=dialog`
//! semantics are covered by the `SidePanel` gallery assertion in `components.rs`.

use dioxus::prelude::*;
use genealogy_app::TagRef;
use genealogy_app::{AssociationRole, Attribute, FactType, NameType, ParticipantRole, Sex};
use genealogy_ui::{
    AssociationVm, AttachedRefVm, CitationRefVm, ConfidenceLevel, EventRefVm, EvidenceAxis, EvidenceAxisVm, FactVm,
    FamilyVm, Localizer, NameVm, PersonDraft, ProvenanceDraft, TimelineKind, TimelineRowVm,
};
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::screens::{
    EditForm, RecordEditState, associations_table, citations_table, events_table, facts_table, families_panel, id_list,
    names_table, person_record_fields, tags_panel, timeline_panel,
};
use genealogy_ui_dioxus::shell::ChromeCtx;
use genealogy_ui_dioxus::shell::nav_state::NavState;
use std::rc::Rc;

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
        confidence: Some(ConfidenceLevel::High),
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
            confidence: Some(ConfidenceLevel::High),
            confidence_label: "High".to_owned(),
            source_count: 2,
            citations: Vec::new(),
            assertion_id: "0190a2b3-0000-7000-8000-000000000002".to_owned(),
        },
        FactVm {
            type_label: "Residence".to_owned(),
            fact_type: FactType::Residence,
            value: Some("London".to_owned()),
            date: Some("1815".to_owned()),
            confidence: Some(ConfidenceLevel::Low),
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

/// A single fact asserted with no surety judgment (ADR 0021 §5) — its confidence badge renders the
/// unset label with no `data-level` colour token.
fn unjudged_fact_table() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let facts = vec![FactVm {
        type_label: "Occupation".to_owned(),
        fact_type: FactType::Occupation,
        value: Some("Mathematician".to_owned()),
        date: None,
        confidence: None,
        confidence_label: "No judgment".to_owned(),
        source_count: 1,
        citations: Vec::new(),
        assertion_id: "0190a2b3-0000-7000-8000-00000000000f".to_owned(),
    }];
    let onretract = use_callback(|_| {});
    let onedit = use_callback(|_| {});
    rsx! {
        {facts_table(&loc, &facts, onedit, onretract)}
    }
}

#[test]
fn a_row_without_confidence_renders_the_unset_badge() {
    let mut vdom = VirtualDom::new(unjudged_fact_table);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"class="conf conf-unset""#),
        "an unjudged row renders the faint unset badge:\n{html}"
    );
    assert!(html.contains("No judgment"), "the unset label renders:\n{html}");
    assert!(
        !html.contains("data-level"),
        "an unjudged row carries no confidence colour token:\n{html}"
    );
}

/// Renders the Names table with a chrome context in scope so the row-actions column header resolves
/// its visually-hidden "Actions" accessible name (U43).
fn names_table_with_chrome() -> Element {
    use_context_provider(|| {
        ChromeCtx(Rc::new(Chrome::with_languages(
            None,
            &["en".parse().unwrap_or_default()],
        )))
    });
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
        confidence: Some(ConfidenceLevel::High),
        confidence_label: "High".to_owned(),
        source_count: 1,
        assertion_id: "0190a2b3-0000-7000-8000-000000000001".to_owned(),
    }];
    let onretract = use_callback(|_| {});
    let onedit = use_callback(|_| {});
    rsx! {
        {names_table(&loc, &names, onedit, onretract)}
    }
}

#[test]
fn a_data_table_carries_a_caption_and_named_actions_column() {
    let mut vdom = VirtualDom::new(names_table_with_chrome);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"<caption class="sr-only">Names</caption>"#),
        "the table announces its accessible name via an sr-only caption:\n{html}"
    );
    assert!(
        html.contains(r#"<span class="sr-only">Actions</span>"#),
        "the row-actions column header carries a visually-hidden accessible name:\n{html}"
    );
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
fn name_rows_carry_a_cite_action_between_edit_and_retract() {
    let mut vdom = VirtualDom::new(person_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    // Every name row offers a Cite action with the mockup tooltip and a row-scoped accessible name.
    assert!(html.contains("❝ Cite"), "the name row offers a Cite verb:\n{html}");
    assert!(
        html.contains(r#"title="Attach a citation to this name assertion""#),
        "the Cite tooltip is the mockup sentence:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Cite Ada Lovelace""#),
        "the Cite button names the row:\n{html}"
    );
    // Facts do not gain a Cite action — it is a name-only affordance.
    assert!(
        !html.contains(r#"aria-label="Cite Occupation""#),
        "facts have no Cite action:\n{html}"
    );
}

#[test]
fn fact_rows_are_focusable_and_expose_a_source_link_cite_affordance() {
    let mut vdom = VirtualDom::new(person_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    // Fact rows are focusable so the `s` shortcut can cite the focused fact (only the Facts table
    // adds a row tabindex; the Names table does not).
    assert!(
        html.contains(r#"tabindex="0""#),
        "fact rows are focusable for the cite shortcut:\n{html}"
    );
    // The sourced fact exposes the source-count link as a real control (the click-to-cite affordance),
    // not inert text.
    assert!(
        html.contains(r#"class="src-link""#),
        "the fact source-link cite control renders:\n{html}"
    );
    assert!(
        html.contains("❝ 2 sources"),
        "the fact's source-count cite trigger:\n{html}"
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
        html.contains(r#"aria-label="Remove Groom""#),
        "the participation row also offers Remove (PR39 verb flip):\n{html}"
    );
}

#[test]
fn the_events_tab_carries_the_canonical_columns() {
    let mut vdom = VirtualDom::new(person_relation_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    // Canonical Events shape (ui-review Appendix A): Event · Role · Age · Date · Place · Confidence · Source.
    for needle in [">Role<", ">Age<", ">Date<", ">Place<", ">Confidence<", ">Source<"] {
        assert!(html.contains(needle), "expected the {needle:?} column header:\n{html}");
    }
    // Age is its own column value, the place joins in, and the confidence badge renders.
    assert!(html.contains("over 42y"), "the age renders in its own column:\n{html}");
    assert!(
        html.contains("Trinity Church, New York"),
        "the event place renders in the Place column:\n{html}"
    );
}

#[test]
fn a_participation_row_shows_age_attributes_and_notes() {
    let mut vdom = VirtualDom::new(person_relation_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    // The person-origin row surfaces its participant-scoped detail (ADR 0019).
    assert!(html.contains("over 42y"), "the localized age label renders:\n{html}");
    assert!(
        html.contains("occupation: farmer"),
        "the participant attribute renders as type: value:\n{html}"
    );
    assert!(
        html.contains("N0001"),
        "the participation note renders as a chip:\n{html}"
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
        confidence: Some(ConfidenceLevel::Low),
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
            backs_count: 0,
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
            backs_count: 0,
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
        {citations_table::<EditForm>(&loc, &citations, true, onretract)}
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
        place: Some("Trinity Church, New York".to_owned()),
        age_label: Some("over 42y".to_owned()),
        age: None,
        attributes: vec![Attribute {
            attribute_type: "occupation".to_owned(),
            value: "farmer".to_owned(),
        }],
        notes: vec!["N0001".to_owned()],
        confidence: Some(ConfidenceLevel::High),
        confidence_label: "High".to_owned(),
        source_count: 1,
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
    let editing = use_signal(|| None::<EditForm>);
    let on_remove = use_callback(|_: String| {});
    let tags = vec![TagRef {
        id: "0190a2b3-0000-7000-8000-0000000000ff".to_owned(),
        name: "Direct ancestor".to_owned(),
        color: Some("#e5534b".to_owned()),
        priority: Some(1),
    }];
    tags_panel(&loc, &tags, editing, EditForm::Tag, on_remove)
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

/// Renders the person Timeline tab over a VM-ordered fixture: a dated event, a dated unsourced fact,
/// then an undated event participation (undated rows last, per `timeline_rows`).
fn person_timeline() -> Element {
    // RecordLink resolves NavState from context, so the harness must provide it.
    use_context_provider(NavState::new);
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let rows = vec![
        TimelineRowVm {
            kind: TimelineKind::Event,
            kind_label: "Event".to_owned(),
            date: Some("12 Apr 1850".to_owned()),
            type_label: "Primary".to_owned(),
            detail: Some("New York".to_owned()),
            event_id: Some("E0001".to_owned()),
            confidence: Some(ConfidenceLevel::High),
            confidence_label: "High".to_owned(),
            source_count: 2,
        },
        TimelineRowVm {
            kind: TimelineKind::Fact,
            kind_label: "Fact".to_owned(),
            date: Some("1880".to_owned()),
            type_label: "Occupation".to_owned(),
            detail: Some("Carpenter".to_owned()),
            event_id: None,
            confidence: Some(ConfidenceLevel::Low),
            confidence_label: "Low".to_owned(),
            source_count: 0,
        },
        TimelineRowVm {
            kind: TimelineKind::Event,
            kind_label: "Event".to_owned(),
            date: None,
            type_label: "Witness".to_owned(),
            detail: None,
            event_id: Some("E0007".to_owned()),
            confidence: None,
            confidence_label: "No judgment".to_owned(),
            source_count: 1,
        },
    ];
    timeline_panel(&loc, &rows)
}

#[test]
fn the_timeline_tab_merges_facts_and_events_with_evidence_cues_distinct_from_history() {
    let mut vdom = VirtualDom::new(person_timeline);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    // The distinguishing section-note: the life story derived from facts + events, NOT the audit log.
    assert!(html.contains("section-note"), "the tab carries a section-note:\n{html}");
    assert!(
        html.contains("life story"),
        "the note frames the Timeline as the genealogical life story:\n{html}"
    );
    assert!(
        html.contains("audit trail"),
        "the note contrasts the Timeline with the History audit trail:\n{html}"
    );

    // Rows render oldest-first with the undated participation last (the VM already ordered them).
    let pos_event_1850 = html.find("E0001").expect("the 1850 event row renders");
    let pos_fact_1880 = html.find("Occupation").expect("the 1880 fact row renders");
    let pos_event_undated = html.find("E0007").expect("the undated event row renders");
    assert!(
        pos_event_1850 < pos_fact_1880 && pos_fact_1880 < pos_event_undated,
        "rows render in chronological order, undated last:\n{html}"
    );
    // The undated row renders an em dash in its date cell.
    assert!(html.contains("—"), "the undated row shows an em dash date:\n{html}");

    // The event rows link to their event record (the shared RecordLink renders as an inline src-link).
    assert!(
        html.contains(r#"class="src-link""#),
        "event rows link to the event record:\n{html}"
    );

    // Per-claim confidence cue: colour token AND redundant text (colour is never the only signal).
    assert!(
        html.contains(r#"data-level="high""#),
        "the confidence colour token renders:\n{html}"
    );
    assert!(html.contains(">High"), "the confidence label text renders:\n{html}");
    // Per-claim source cue: a source-count link for the sourced row, the no-source flag for the bare one.
    assert!(
        html.contains("2 sources"),
        "the sourced row shows its source count:\n{html}"
    );
    assert!(
        html.contains(r#"class="no-source""#),
        "the unsourced row shows the no-source flag:\n{html}"
    );
    assert!(
        html.contains("No source"),
        "the no-source flag carries text, not colour alone:\n{html}"
    );

    // A tab panel introduces no page heading — the screen keeps exactly one h1 (a11y).
    assert!(!html.contains("<h1"), "the timeline panel adds no second h1:\n{html}");
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

/// A person draft whose sex is a free-text `Other` value — the case the old `SEXES`-index logic
/// mislabelled as "Unknown".
fn other_sex_person() -> PersonDraft {
    let mut draft = PersonDraft::new();
    draft.sex = Sex::Other("two-spirit".to_owned());
    draft
}

/// Renders the person record fields in edit mode over a draft whose sex is `Other("two-spirit")`.
fn person_other_sex_fields() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let state = RecordEditState {
        editing: use_signal(|| true),
        seed: use_signal(other_sex_person),
        draft: use_signal(other_sex_person),
        prov: use_signal(ProvenanceDraft::default),
    };
    person_record_fields(&loc, state)
}

#[test]
fn a_stored_other_sex_selects_other_and_prefills_the_free_text() {
    let mut vdom = VirtualDom::new(person_other_sex_fields);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains("Other…"),
        "the Sex select offers an Other choice:\n{html}"
    );
    assert!(
        html.contains(r#"name="sex-other""#),
        "a stored Other sex reveals the free-text input:\n{html}"
    );
    assert!(
        html.contains(r#"value="two-spirit""#),
        "the free-text is pre-filled with the stored value, not mislabelled as Unknown:\n{html}"
    );
}
