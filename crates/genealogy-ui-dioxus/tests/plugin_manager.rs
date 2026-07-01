//! SSR a11y assertions for the plugin manager table (Phase 5 PR21): each row's enabled/disabled
//! control is a real `role="switch"` with an accessible name and a text state (never colour alone),
//! and each declared capability renders as a text-labelled badge. Pure render-and-inspect over
//! hand-built rows — no window, no workspace, no plugin host — the same pattern as `pedigree.rs`.

use dioxus::prelude::*;
use genealogy_plugin_host::{Capability, PluginRole};
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::screens::plugin_table;
use genealogy_ui_dioxus::services::PluginRow;
use unic_langid::LanguageIdentifier;

fn chrome(tag: &str) -> Chrome {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Chrome::with_languages(None, &[language])
}

fn rows() -> Vec<PluginRow> {
    vec![
        PluginRow {
            id: "gedcom-import".to_owned(),
            role: PluginRole::BulkImport,
            host_api_version: "0.12.0".to_owned(),
            capabilities: vec![Capability::Log, Capability::Commands, Capability::Progress],
            enabled: true,
        },
        PluginRow {
            id: "ui-panel".to_owned(),
            role: PluginRole::UiPanel,
            host_api_version: "0.12.0".to_owned(),
            capabilities: vec![Capability::Log],
            enabled: false,
        },
    ]
}

thread_local! {
    /// The language the table harness renders in — `VirtualDom::new` requires a bare no-argument
    /// root, so the language is smuggled in via a thread-local (the same trick `pedigree.rs` uses).
    static TABLE_LANG: std::cell::Cell<&'static str> = const { std::cell::Cell::new("en") };
}

fn table() -> Element {
    let chrome = chrome(TABLE_LANG.with(std::cell::Cell::get));
    let on_toggle = Callback::new(|_: (String, bool)| {});
    plugin_table(&chrome, &rows(), on_toggle)
}

#[test]
fn every_row_has_a_labelled_switch_with_a_text_state() {
    TABLE_LANG.with(|lang| lang.set("en"));
    let mut vdom = VirtualDom::new(table);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert_eq!(
        html.matches(r#"role="switch""#).count(),
        2,
        "one switch per plugin row:\n{html}"
    );
    assert!(
        html.contains(r#"aria-checked="true""#) && html.contains(r#"aria-checked="false""#),
        "both switch states are represented:\n{html}"
    );
    assert!(
        html.contains("aria-label=\"gedcom-import enabled\""),
        "the switch has an accessible name naming its plugin:\n{html}"
    );
    assert!(
        html.contains(">On<") && html.contains(">Off<"),
        "the switch state is a visible text label, not colour alone:\n{html}"
    );
}

#[test]
fn declared_capabilities_render_as_text_labelled_badges() {
    TABLE_LANG.with(|lang| lang.set("en"));
    let mut vdom = VirtualDom::new(table);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in ["log", "commands", "progress"] {
        assert!(html.contains(needle), "expected capability {needle:?} in:\n{html}");
    }
    assert!(
        html.matches(r#"class="ev info""#).count() >= 3,
        "log/progress + ui-panel's log are the read/observability hue:\n{html}"
    );
    assert!(
        html.contains(r#"class="ev evidence""#),
        "commands (the only write capability) gets its own hue:\n{html}"
    );
}

#[test]
fn every_row_shows_its_role_host_api_version_and_trust_tier() {
    TABLE_LANG.with(|lang| lang.set("en"));
    let mut vdom = VirtualDom::new(table);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Bulk import"), "the role label renders:\n{html}");
    assert!(html.contains("UI panel"), "the other role label renders:\n{html}");
    assert!(
        html.contains("host-api 0.12.0"),
        "the host-api version caption renders:\n{html}"
    );
    assert!(
        html.matches("unsigned").count() == 2,
        "every plugin is shown as the honest read-only trust tier:\n{html}"
    );
}

#[test]
fn the_table_has_localized_column_headers_in_norwegian() {
    TABLE_LANG.with(|lang| lang.set("no"));
    let mut vdom = VirtualDom::new(table);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in ["Tillegg", "Aktivert", "Deklarerte kapabiliteter", "Tillit"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}
