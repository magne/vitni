//! SSR assertions for the Preferences tool (Phase 5 PR 20): every card renders its labelled
//! fields, the theme control is a true `role="radio"`/`aria-checked` group (not the multi-select
//! `aria-pressed` toggle set), the workspace-defaults card shows the three-layer override chain with
//! the correct layer marked `wins`, the registered-workspace list marks the active one and offers a
//! named "switch to" action for the rest, and the `aria-live` status region carries save feedback.
//! Pure render-and-inspect over hand-built fixtures — no window, no workspace, no plugin host — the
//! same pattern as `pedigree.rs`/`history_dashboard.rs`.

use std::collections::BTreeMap;

use dioxus::prelude::*;
use genealogy_app::{
    Config, DateFormat, IdFormatLayers, LayerKind, NumberFormat, OperatorConfig, PreferenceLayers, ResolvedLocale,
    ThemeLayers, ThemeMode, WorkspaceDefaults, WorkspaceEntry,
};
use genealogy_core::ids::AgentId;
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::screens::{LocaleFields, preferences_view};
use genealogy_ui_dioxus::services::PreferencesData;
use unic_langid::LanguageIdentifier;
use uuid::Uuid;

/// A chrome localizer for a single explicit language (deterministic for tests).
fn chrome(tag: &str) -> Chrome {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Chrome::with_languages(None, &[language])
}

/// A config with one registered workspace ("gen", the active default) and a stable operator.
fn config_with_one_workspace() -> Config {
    let mut workspaces = BTreeMap::new();
    workspaces.insert(
        "gen".to_owned(),
        WorkspaceEntry {
            path: "/data/gen".into(),
        },
    );
    Config {
        default: Some("gen".to_owned()),
        workspaces,
        operator: OperatorConfig {
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Magne Rasmussen".to_owned()),
            email: Some("magne@example.com".to_owned()),
        },
        defaults: genealogy_app::AppDefaults::default(),
        workspace_defaults: WorkspaceDefaults::default(),
    }
}

/// A config with two registered workspaces ("gen" active, "tree2" not).
fn config_with_two_workspaces() -> Config {
    let mut config = config_with_one_workspace();
    config.workspaces.insert(
        "tree2".to_owned(),
        WorkspaceEntry {
            path: "/data/tree2".into(),
        },
    );
    config
}

/// The Person id format pinned by a workspace manifest override (the "wins" case).
fn layers_with_pinned_theme_and_id_format() -> PreferenceLayers {
    PreferenceLayers {
        theme: ThemeLayers {
            workspace: Some(ThemeMode::Dark),
            shared_default: ThemeMode::Light,
            embedded: ThemeMode::System,
            winner: LayerKind::Workspace,
        },
        person_id_format: IdFormatLayers {
            workspace: Some("Z%02d".to_owned()),
            shared_default: "A%04d".to_owned(),
            embedded: "I%04d".to_owned(),
            winner: LayerKind::Workspace,
        },
    }
}

/// The default layers: neither the theme nor the Person id format has a workspace override, so the
/// shared default wins both.
fn layers_falling_back_to_shared_default() -> PreferenceLayers {
    PreferenceLayers {
        theme: ThemeLayers {
            workspace: None,
            shared_default: ThemeMode::Light,
            embedded: ThemeMode::System,
            winner: LayerKind::SharedDefault,
        },
        person_id_format: IdFormatLayers {
            workspace: None,
            shared_default: "I%04d".to_owned(),
            embedded: "I%04d".to_owned(),
            winner: LayerKind::SharedDefault,
        },
    }
}

fn resolved_locale(date_format: DateFormat, number_format: NumberFormat) -> ResolvedLocale {
    resolved_locale_with_languages(None, None, date_format, number_format)
}

fn resolved_locale_with_languages(
    ui_language: Option<&str>,
    data_locale: Option<&str>,
    date_format: DateFormat,
    number_format: NumberFormat,
) -> ResolvedLocale {
    ResolvedLocale {
        ui_language: ui_language.map(|tag| tag.parse().unwrap_or_default()),
        data_locale: data_locale.map(|tag| tag.parse().unwrap_or_default()),
        date_format,
        number_format,
    }
}

/// Renders [`preferences_view`] over `config`/`layers`/`locale` in English, theme `Dark`, with no
/// status message and inert callbacks. Signals need a Dioxus scope, so this whole function is the
/// SSR root component (mirrors how `dashboard_view` needs `NavState` provided by its caller).
/// The `LocaleFields` signals are seeded from `locale` (the *resolved* value), exactly as
/// `PreferencesScreen` seeds them.
fn view(config: Config, layers: PreferenceLayers, locale: ResolvedLocale) -> Element {
    view_with_status_and_locale(config, layers, locale, None)
}

/// The same render, with a save-status message set.
fn view_with_status(status: &'static str) -> Element {
    view_with_status_and_locale(
        config_with_one_workspace(),
        layers_falling_back_to_shared_default(),
        resolved_locale(DateFormat::Long, NumberFormat::SpaceComma),
        Some(status.to_owned()),
    )
}

fn view_with_status_and_locale(
    config: Config,
    layers: PreferenceLayers,
    locale: ResolvedLocale,
    status: Option<String>,
) -> Element {
    let data = PreferencesData { config, layers, locale };
    let display = use_signal(|| data.config.operator.display.clone().unwrap_or_default());
    let email = use_signal(|| data.config.operator.email.clone().unwrap_or_default());
    let person_id_format = use_signal(|| data.layers.person_id_format.shared_default.clone());
    let locale_fields = LocaleFields {
        ui_language: use_signal(|| {
            data.locale
                .ui_language
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default()
        }),
        data_locale: use_signal(|| {
            data.locale
                .data_locale
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default()
        }),
        date_format: use_signal(|| date_format_value(data.locale.date_format).to_owned()),
        number_format: use_signal(|| number_format_value(data.locale.number_format).to_owned()),
    };
    preferences_view(
        &chrome("en"),
        &data,
        ThemeMode::Dark,
        display,
        email,
        person_id_format,
        locale_fields,
        status,
        |_| {},
        |_| {},
        |_| {},
        |_| {},
    )
}

/// The `<select>` value token for a [`DateFormat`] variant (mirrors the private helper in
/// `preferences.rs` — duplicated here since the test harness seeds its own signals rather than
/// going through `PreferencesScreen`).
fn date_format_value(format: DateFormat) -> &'static str {
    match format {
        DateFormat::Long => "long",
        DateFormat::Medium => "medium",
        DateFormat::Numeric => "numeric",
        DateFormat::LocaleDefault => "locale-default",
    }
}

/// The `<select>` value token for a [`NumberFormat`] variant (see [`date_format_value`]).
fn number_format_value(format: NumberFormat) -> &'static str {
    match format {
        NumberFormat::SpaceComma => "space-comma",
        NumberFormat::CommaPoint => "comma-point",
        NumberFormat::LocaleDefault => "locale-default",
    }
}

fn one_workspace_pinned() -> Element {
    view(
        config_with_one_workspace(),
        layers_with_pinned_theme_and_id_format(),
        resolved_locale(DateFormat::Long, NumberFormat::SpaceComma),
    )
}

fn one_workspace_fallback() -> Element {
    view(
        config_with_one_workspace(),
        layers_falling_back_to_shared_default(),
        resolved_locale(DateFormat::Numeric, NumberFormat::CommaPoint),
    )
}

fn two_workspaces() -> Element {
    view(
        config_with_two_workspaces(),
        layers_falling_back_to_shared_default(),
        resolved_locale(DateFormat::Long, NumberFormat::SpaceComma),
    )
}

/// A resolved locale carrying a workspace-manifest UI-language override ("nn-NO") that differs from
/// the shared default (unset — `None`/"follow the system").
fn workspace_language_override() -> Element {
    view(
        config_with_one_workspace(),
        layers_falling_back_to_shared_default(),
        resolved_locale_with_languages(Some("nn-NO"), None, DateFormat::Long, NumberFormat::SpaceComma),
    )
}

fn saved_status() -> Element {
    view_with_status("Preferences saved.")
}

/// Renders a component to an HTML string.
fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn identity_card_renders_the_operator_fields() {
    let html = render(one_workspace_fallback);
    for needle in [
        "Operator identity",
        "Who is making changes",
        "Display name",
        r#"value="Magne Rasmussen""#,
        "Email",
        r#"value="magne@example.com""#,
        "Agent kind",
        "Person",
        "Operator id",
        // The operator id (a UUID v7) renders verbatim as the read-only field's value.
        &AgentId::from_uuid(Uuid::from_u128(1)).to_string(),
        "Software agents (import/export plugins) are stamped automatically",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn theme_control_is_a_true_radiogroup_not_a_toggle_set() {
    let html = render(one_workspace_fallback);
    assert!(
        html.contains(r#"role="radiogroup""#),
        "the theme control is a radiogroup:\n{html}"
    );
    assert_eq!(
        html.matches(r#"role="radio""#).count(),
        3,
        "Light/Dark/System are each a radio:\n{html}"
    );
    assert!(
        html.contains(r#"aria-checked="true""#) && html.contains(r#"aria-checked="false""#),
        "exactly the active mode is checked:\n{html}"
    );
    // Distinct from the RestrictionSet's aria-pressed multi-select toggles — this screen must not
    // reuse that semantic for a single-choice control.
    assert!(
        !html.contains("aria-pressed"),
        "the theme radiogroup must not carry aria-pressed:\n{html}"
    );
    // The RadioGroup primitive adds the roving-tabindex contract the old inline picker lacked: the
    // selected radio (Dark, this fixture's theme mode) is the single tab stop, the rest are removed
    // from the tab order.
    assert_eq!(
        html.matches(r#"tabindex="0""#).count(),
        1,
        "exactly the selected radio is the tab stop:\n{html}"
    );
    assert_eq!(
        html.matches(r#"tabindex="-1""#).count(),
        2,
        "the two non-selected radios are removed from the tab order:\n{html}"
    );
}

#[test]
fn locale_card_renders_the_resolved_fallback_chain() {
    let html = render(one_workspace_fallback);
    for needle in [
        "Language &#38; locale", // dioxus_ssr HTML-escapes `&` in text nodes
        "Interface &#38; data",
        "UI language",
        "Data locale",
        "Resolved fallback chain",
        r#"class="chip""#,
        "en", // the fallback_chain over an empty override always ends at the "en" baseline
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn locale_card_displays_a_pinned_workspace_override_even_though_the_shared_default_differs() {
    // The shared default (config_with_one_workspace's WorkspaceDefaults::default()) leaves
    // ui_language unset; the resolved value carries a workspace-manifest override ("nn-NO") that
    // must win for *display* — this is the fix for the bug where the card read the raw shared
    // default instead of the resolved value, silently ignoring any workspace override.
    let html = render(workspace_language_override);
    assert!(
        html.contains(r#"value="nn-NO""#),
        "the UI-language select shows the resolved override, not the (unset) shared default:\n{html}"
    );
    // The fallback chain is likewise computed from the override, not the system default.
    assert!(
        html.contains(">nn-NO<") && html.contains(">no<"),
        "the fallback chain starts from the pinned nn-NO override:\n{html}"
    );
}

#[test]
fn locale_and_format_selects_offer_onchange_wired_fields() {
    // Every editable select in the Language/locale and Date/number cards carries a name — this is a
    // smoke check that the fields render as real controlled inputs (the onchange wiring itself is a
    // Dioxus event handler, not observable from static SSR output, but a missing `name`/id would
    // indicate the field lost its label association during refactoring).
    let html = render(one_workspace_fallback);
    for name in ["ui-language", "data-locale", "date-format", "number-format"] {
        assert!(
            html.contains(&format!(r#"name="{name}""#)),
            "expected the {name:?} field to render:\n{html}"
        );
    }
}

#[test]
fn formats_card_renders_the_live_example_for_the_resolved_format() {
    let numeric_html = render(one_workspace_fallback);
    assert!(
        numeric_html.contains("1850-04-12"),
        "the Numeric date format's example renders:\n{numeric_html}"
    );
    assert!(
        numeric_html.contains("1,234.56"),
        "the comma-point number format's example renders:\n{numeric_html}"
    );

    let long_html = render(one_workspace_pinned);
    assert!(
        long_html.contains("12 April 1850"),
        "the Long date format's example renders:\n{long_html}"
    );
    assert!(
        long_html.contains("1 234,56"),
        "the space-comma number format's example renders:\n{long_html}"
    );
}

#[test]
fn defaults_card_marks_the_workspace_layer_as_the_winner_when_pinned() {
    let html = render(one_workspace_pinned);
    assert!(html.contains("Workspace defaults"), "the section heading:\n{html}");
    assert!(
        html.contains("Where a setting&#39;s value comes from"), // dioxus_ssr HTML-escapes `'`
        "the card title:\n{html}"
    );
    assert!(html.contains(">wins<"), "the winning layer's badge:\n{html}");
    assert_eq!(
        html.matches(">fallback<").count(),
        2,
        "the two non-winning layers:\n{html}"
    );
    assert!(
        html.contains("workspace.toml"),
        "the workspace layer names its file:\n{html}"
    );
    assert!(
        html.contains("~/.config/genealogy/config.toml"),
        "the shared-app layer names its file:\n{html}"
    );
    assert!(html.contains("built-in baseline"), "the embedded layer:\n{html}");
    // The editable Person id format field is pre-filled from the live shared default (not the
    // workspace override), matching how `person_id_format` is seeded in `PreferencesScreen`.
    assert!(
        html.contains(r#"value="A%04d""#),
        "the id-format field's seeded value:\n{html}"
    );
}

#[test]
fn defaults_card_marks_the_shared_default_as_the_winner_when_unpinned() {
    let html = render(one_workspace_fallback);
    assert!(html.contains(">wins<"), "a winning badge still renders:\n{html}");
    // With no workspace override, the shared-app row is the one marked "wins".
    let shared_row_start = html.find("~/.config/genealogy/config.toml").expect("shared row");
    let wins_before_shared = html[..shared_row_start].rfind(">wins<").is_some();
    let wins_is_closest = html[..shared_row_start]
        .rfind(">wins<")
        .zip(html[..shared_row_start].rfind(">fallback<"))
        .is_none_or(|(wins_at, fallback_at)| wins_at > fallback_at);
    assert!(
        wins_before_shared && wins_is_closest,
        "the shared-app row is the winner:\n{html}"
    );
}

#[test]
fn workspace_list_marks_the_active_workspace_and_offers_switch_actions_for_the_rest() {
    let html = render(two_workspaces);
    assert!(html.contains("Registered workspaces"), "the card title:\n{html}");
    assert!(html.contains("gen"), "the active workspace's name:\n{html}");
    assert!(html.contains("tree2"), "the other workspace's name:\n{html}");
    assert!(html.contains(">Active<"), "the active workspace's badge:\n{html}");
    // The button's visible text is its accessible name (a real <button>, no icon-only control).
    assert!(
        html.contains(">Switch to tree2<"),
        "a named switch action for the non-active workspace:\n{html}"
    );
    assert!(
        !html.contains("Switch to gen"),
        "the active workspace has no switch-to-itself action:\n{html}"
    );
}

#[test]
fn a_single_registered_workspace_has_no_switch_action() {
    let html = render(one_workspace_fallback);
    assert!(
        html.contains(">Active<"),
        "the sole workspace is marked active:\n{html}"
    );
    assert!(
        !html.contains("Switch to"),
        "there is nothing else to switch to:\n{html}"
    );
}

#[test]
fn the_action_row_offers_reset_and_save() {
    let html = render(one_workspace_fallback);
    assert!(html.contains(">Reset to defaults<"), "the reset action:\n{html}");
    assert!(html.contains(">Save preferences<"), "the save action:\n{html}");
}

#[test]
fn save_status_renders_in_an_aria_live_region() {
    let html = render(saved_status);
    assert!(html.contains(r#"role="status""#), "the status role:\n{html}");
    assert!(html.contains(r#"aria-live="polite""#), "the live region:\n{html}");
    assert!(html.contains("Preferences saved."), "the status message text:\n{html}");
}

#[test]
fn settings_sub_nav_lists_every_section() {
    let html = render(one_workspace_fallback);
    assert!(
        html.contains(r#"aria-label="Preference sections""#),
        "the sub-nav landmark:\n{html}"
    );
    for section in [
        "Operator identity",
        "Appearance",
        "Language &#38; locale", // dioxus_ssr HTML-escapes `&` in text nodes
        "Date &#38; number",
        "Workspace defaults",
    ] {
        assert!(
            html.contains(&format!(">{section}<")),
            "expected sub-nav entry {section:?}:\n{html}"
        );
    }
}
