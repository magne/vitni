//! SSR assertions for the in-app help browser (Phase 5, ADR 0008 §5): render the "Why this app"
//! article to HTML and assert its prose, contrast blocks, specimens, and the "At a glance" table.
//! Also asserts graceful localization — no raw `help-*` message id leaks into the output (which would
//! mean a missing catalogue key) — and that an unknown topic id falls back to the default topic.

use dioxus::prelude::*;
use genealogy_ui::{HelpTopicId, Localizer, help_doc};
use genealogy_ui_dioxus::screens::render_doc;
use genealogy_ui_dioxus::shell::nav_state::NavState;

/// Renders an article under a `NavState` provider, so topics that contain `TopicLink` runs (which
/// navigate via `use_context::<NavState>()`) render the same way they do inside the shell.
#[component]
fn TopicView(topic: HelpTopicId) -> Element {
    use_context_provider(NavState::new);
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    render_doc(&help_doc(topic), &loc)
}

fn render_topic(topic: HelpTopicId) -> String {
    let mut vdom = VirtualDom::new_with_props(TopicView, TopicViewProps { topic });
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn render() -> String {
    render_topic(HelpTopicId::default_topic())
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

#[test]
fn recording_contents_links_to_the_guides_and_glossary() {
    let html = render_topic(HelpTopicId::RecordingOverview);
    assert!(
        html.contains(r#"class="help-link""#),
        "the contents page renders in-prose topic links:\n{html}"
    );
    for needle in ["Recording a person", "Recording a census", "Glossary"] {
        assert!(html.contains(needle), "expected link {needle:?} in:\n{html}");
    }
}

#[test]
fn glossary_defines_the_core_terms() {
    let html = render_topic(HelpTopicId::Glossary);
    for needle in [
        r#"class="tbl""#,
        "Assertion",
        "Fact",
        "Event",
        "Citation",
        "Association",
        "Family",
        "the evidence layer", // the assertion definition
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn every_topic_renders_without_leaking_a_raw_id() {
    for topic in HelpTopicId::all() {
        let html = render_topic(topic);
        assert!(!html.is_empty(), "empty render for {topic:?}");
        for prefix in [
            "help-rec-",
            "help-person-",
            "help-family-",
            "help-census-",
            "help-burial-",
            "help-gloss-",
            "help-topic-",
        ] {
            assert!(
                !html.contains(prefix),
                "a raw {prefix}* id leaked for {topic:?} (missing catalogue key):\n{html}"
            );
        }
    }
}
