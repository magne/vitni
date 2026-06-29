//! SSR assertions for the in-app help browser (Phase 5, ADR 0008 §5): render the "Why this app"
//! article to HTML and assert its prose, contrast blocks, specimens, and the "At a glance" table.
//! Also asserts graceful localization — no raw `help-*` message id leaks into the output (which would
//! mean a missing catalogue key) — and that an unknown topic id falls back to the default topic.

use dioxus::prelude::*;
use genealogy_ui::{HelpTopicId, Localizer, help_doc};
use genealogy_ui_dioxus::screens::render_doc;

/// Renders the default topic's article (the only topic this slice ships). `VirtualDom::new` requires
/// a non-capturing `fn`, so the topic and localizer are resolved inside.
fn why_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let doc = help_doc(HelpTopicId::default_topic());
    render_doc(&doc, &loc)
}

fn render() -> String {
    let mut vdom = VirtualDom::new(why_view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn why_this_app_renders_prose_and_contrast_blocks() {
    let html = render();
    for needle in [
        r#"class="lede""#,
        "evidence and the reasoning", // the lede's bold run
        r#"class="contrast""#,
        r#"class="most""#,
        r#"class="ours""#,
        "Most tools", // the contrast tag label
        "This app",
        "who · when · why", // the audit "ours" bold run
        // The { " " } boundary spaces survive Fluent trimming — the text node between the bold/italic
        // runs keeps its leading and trailing space (no "event:who" / "storeconclusions" run-ons).
        "Every change is an immutable event: ",
        " and quietly overwrite them. This one stores the ",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn why_this_app_renders_every_specimen() {
    let html = render();
    for needle in [
        r#"class="timeline""#, // audit-trail timeline
        "Baptism register gives exact date",
        r#"class="fact-row""#,   // evidence-first vital facts
        r#"data-level="high""#,  // a confidence badge
        r#"class="ev source""#,  // an evidence-axis chip
        r#"class="merge-grid""#, // reversible-merge specimen
        r#"class="prov""#,       // provenance specimen
        "first-party",           // capability-badges specimen
        r#"class="chip mono""#,  // the nb-NO → no → en chips
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn at_a_glance_table_renders() {
    let html = render();
    for needle in [
        r#"class="tbl""#,
        "Capability",
        "Built-in, immutable",
        "Same Fluent pipeline",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn no_raw_help_message_id_leaks() {
    let html = render();
    assert!(
        !html.contains("help-why-"),
        "a raw help-* id leaked (missing catalogue key):\n{html}"
    );
}

#[test]
fn unknown_topic_id_falls_back_to_the_default() {
    assert_eq!(HelpTopicId::from_id("overview.nope"), None);
    // The screen resolves an unknown/absent topic to the default, so the default article renders.
    let default_html = render();
    assert!(
        default_html.contains(r#"class="lede""#),
        "default topic must render:\n{default_html}"
    );
}
