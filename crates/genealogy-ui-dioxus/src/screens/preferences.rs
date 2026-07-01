//! The Preferences tool (Phase 5 PR 20; `docs/phase5/preferences.html`): operator identity,
//! appearance, language & locale, date & number format, and the workspace-defaults override chain
//! together with the workspace switcher. Unlike the aggregate slices, Preferences talks straight to
//! `genealogy-app` config read/write use-cases (`crate::services::{load_preferences, save_*,
//! switch_workspace}`) — there is no `genealogy_ui::dispatch`/`Intent` involved, since preferences
//! are not an aggregate.
//!
//! Fields are edited inline inside cards (per `docs/phase5/edit-patterns.html`: simple fields use
//! the inline convention, not a side panel/modal), and a single "Save preferences" commits every
//! section's pending edits in one pass. The theme control is the exception: it saves immediately
//! (through the same `save_theme_mode` the top-bar toggle uses), so the two stay in sync.

use std::cell::RefCell;
use std::rc::Rc;

use genealogy_app::{DateFormat, IdFormats, LayerKind, LocaleDefaults, NumberFormat, ResolvedLocale, ThemeMode};
use genealogy_i18n::fallback_chain;
use i18n_embed::DesktopLanguageRequester;
use unic_langid::LanguageIdentifier;

use super::prelude::*;
use crate::app::request_restart;
use crate::components::{Badge, LabeledValue};
use crate::i18n::Chrome;
use crate::services::{
    PreferencesData, load_preferences, save_id_format_defaults, save_locale_defaults, save_operator_identity,
    switch_workspace,
};

/// A fixed example date, rendered in each [`DateFormat`] style (matching the mockup's `12 April 1850`).
const EXAMPLE_DATE_LONG: &str = "12 April 1850";
const EXAMPLE_DATE_MEDIUM: &str = "12 Apr 1850";
const EXAMPLE_DATE_NUMERIC: &str = "1850-04-12";

/// A fixed example number, rendered in each [`NumberFormat`] style (matching the mockup's `1 234,56`).
const EXAMPLE_NUMBER_SPACE_COMMA: &str = "1 234,56";
const EXAMPLE_NUMBER_COMMA_POINT: &str = "1,234.56";

/// The Preferences screen: loads its data from `genealogy-app` config/manifest reads (never opens
/// the store) and renders the settings sub-nav + cards.
#[component]
pub fn PreferencesScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome_rc();
    let saved_label = state.chrome().prefs_saved();
    let mut nav = use_context::<NavState>();
    let mut data = use_signal(|| load_preferences(&services));
    let mut status = use_signal(|| None::<String>);

    let mut display = use_signal(|| data().config.operator.display.clone().unwrap_or_default());
    let mut email = use_signal(|| data().config.operator.email.clone().unwrap_or_default());
    let mut person_id_format = use_signal(|| data().layers.person_id_format.shared_default.clone());
    let mut ui_language = use_signal(|| optional_tag(data().locale.ui_language.as_ref()));
    let mut data_locale = use_signal(|| optional_tag(data().locale.data_locale.as_ref()));
    let mut date_format = use_signal(|| date_format_value(data().locale.date_format).to_owned());
    let mut number_format = use_signal(|| number_format_value(data().locale.number_format).to_owned());

    let save_services = services.clone();
    let onsave = move |_| {
        let outcome = save_operator_identity(&save_services, non_empty(display()), non_empty(email()))
            .and_then(|()| {
                let formats = IdFormats {
                    person: person_id_format(),
                    ..IdFormats::default()
                };
                save_id_format_defaults(&save_services, formats)
            })
            .and_then(|()| {
                let locale = LocaleDefaults {
                    ui_language: parse_tag(&ui_language()),
                    data_locale: parse_tag(&data_locale()),
                    date_format: date_format_from_value(&date_format()),
                    number_format: number_format_from_value(&number_format()),
                };
                save_locale_defaults(&save_services, locale)
            });
        match outcome {
            Ok(()) => {
                data.set(load_preferences(&save_services));
                status.set(Some(saved_label.clone()));
            }
            Err(message) => status.set(Some(message)),
        }
    };

    let theme_services = services.clone();
    let onthemechange = move |mode: ThemeMode| {
        nav.theme_mode.set(mode);
        nav.theme.set(crate::shell::nav_state::resolve_theme(mode));
        if let Err(error) = genealogy_app::save_theme_mode(&theme_services.dir, mode) {
            tracing::warn!(%error, "could not persist the theme mode");
        }
        data.set(load_preferences(&theme_services));
    };

    let switch_services = services.clone();
    let onswitch = move |name: String| {
        let outcome = switch_workspace(&switch_services, &name);
        match outcome {
            Ok(()) => request_restart(),
            Err(message) => status.set(Some(message)),
        }
    };

    // Discards unsaved edits by re-seeding the fields from the last-loaded (on-disk) data, without
    // writing anything — the counterpart to `onsave`.
    let onreset = move |_| {
        let loaded = data();
        display.set(loaded.config.operator.display.clone().unwrap_or_default());
        email.set(loaded.config.operator.email.clone().unwrap_or_default());
        person_id_format.set(loaded.layers.person_id_format.shared_default.clone());
        ui_language.set(optional_tag(loaded.locale.ui_language.as_ref()));
        data_locale.set(optional_tag(loaded.locale.data_locale.as_ref()));
        date_format.set(date_format_value(loaded.locale.date_format).to_owned());
        number_format.set(number_format_value(loaded.locale.number_format).to_owned());
        status.set(None);
    };

    preferences_view(
        &chrome,
        &data(),
        *nav.theme_mode.read(),
        display,
        email,
        person_id_format,
        LocaleFields {
            ui_language,
            data_locale,
            date_format,
            number_format,
        },
        status(),
        onsave,
        onreset,
        onthemechange,
        onswitch,
    )
}

/// The editable Language/locale/date/number fields as raw `<select>` value tokens (`ui_language`/
/// `data_locale` are BCP-47 tag strings or empty for "follow the system"; `date_format`/
/// `number_format` are the stable tokens from [`date_format_value`]/[`number_format_value`]).
/// Grouped into one struct so [`preferences_view`]'s signature stays readable now that the
/// Language & locale / Date & number cards are editable too.
#[derive(Debug, Clone, Copy)]
pub struct LocaleFields {
    /// The UI-language field.
    pub ui_language: Signal<String>,
    /// The data-locale field.
    pub data_locale: Signal<String>,
    /// The date-format field.
    pub date_format: Signal<String>,
    /// The number-format field.
    pub number_format: Signal<String>,
}

/// Renders the settings sub-nav + every card. A pure function of its inputs (data, the current
/// theme mode, the editable-field signals, and plain callbacks) so the SSR test can exercise it with
/// hand-built fixtures — no `AppCtx`/plugin host required (mirrors `dashboard_view`).
#[expect(
    clippy::too_many_arguments,
    reason = "one screen, one render entry point; splitting the sub-nav + five cards' shared inputs into a struct would just move the same fields around"
)]
pub fn preferences_view(
    chrome: &Chrome,
    data: &PreferencesData,
    theme_mode: ThemeMode,
    display: Signal<String>,
    email: Signal<String>,
    person_id_format: Signal<String>,
    locale_fields: LocaleFields,
    status: Option<String>,
    onsave: impl FnMut(MouseEvent) + 'static,
    onreset: impl FnMut(MouseEvent) + 'static,
    onthemechange: impl FnMut(ThemeMode) + 'static,
    onswitch: impl FnMut(String) + 'static,
) -> Element {
    rsx! {
        div { style: "display:grid;grid-template-columns:200px 1fr;height:100%;min-height:0",
            nav { class: "list", "aria-label": "{chrome.prefs_nav_label()}", style: "border-right:1px solid var(--line)",
                div { class: "list-rows", style: "padding:var(--sp-2)",
                    for id in ["identity", "appearance", "locale", "formats", "defaults"] {
                        a { class: "nav-item", href: "#{id}", "{chrome.prefs_section_label(id)}" }
                    }
                }
            }
            div { style: "padding:var(--sp-6);overflow:auto;height:100%",
                {identity_card(chrome, &data.config.operator.id.to_string(), display, email)}
                {appearance_card(chrome, theme_mode, onthemechange)}
                {locale_card(chrome, &data.locale, locale_fields.ui_language, locale_fields.data_locale)}
                {formats_card(chrome, locale_fields.date_format, locale_fields.number_format)}
                {defaults_card(chrome, data, person_id_format, onswitch)}
                div { class: "row-actions", style: "justify-content:flex-end;margin-top:8px",
                    Button { label: chrome.prefs_reset(), variant: ButtonVariant::Default, onclick: onreset }
                    Button { label: chrome.prefs_save(), variant: ButtonVariant::Primary, onclick: onsave }
                }
                div { role: "status", aria_live: "polite",
                    if let Some(status) = status {
                        "{status}"
                    }
                }
            }
        }
    }
}

/// The "Operator identity" card: display name / email are editable inline; agent kind and operator
/// id are read-only (mockup: only "Person" is selectable, the id is stamped, never chosen).
fn identity_card(
    chrome: &Chrome,
    operator_id: &str,
    mut display: Signal<String>,
    mut email: Signal<String>,
) -> Element {
    rsx! {
        h2 { id: "identity", style: "border:0;margin:0 0 12px", "{chrome.prefs_section_label(\"identity\")}" }
        Card { title: chrome.prefs_identity_title(),
            div { class: "grid-2",
                Input {
                    label: chrome.prefs_display_name_label(),
                    name: "operator-display".to_owned(),
                    value: Some(display()),
                    oninput: move |event: FormEvent| display.set(event.value()),
                }
                Input {
                    label: chrome.prefs_email_label(),
                    name: "operator-email".to_owned(),
                    value: Some(email()),
                    oninput: move |event: FormEvent| email.set(event.value()),
                }
                Select {
                    label: chrome.prefs_agent_kind_label(),
                    name: "operator-agent-kind".to_owned(),
                    value: Some("person".to_owned()),
                    options: vec![
                        SelectChoice { value: "person".to_owned(), label: chrome.prefs_agent_kind_person() },
                    ],
                }
                LabeledValue { label: chrome.prefs_operator_id_label(), value: operator_id.to_owned() }
            }
            div { class: "muted", style: "font-size:var(--fs-sm)", "{chrome.prefs_software_agent_note()}" }
        }
    }
}

/// The "Appearance" card: the theme radiogroup (`role="radio"`/`aria-checked`, matching the mockup —
/// distinct from the multi-select `RestrictionSet`'s `aria-pressed` toggle group).
fn appearance_card(chrome: &Chrome, mode: ThemeMode, onchange: impl FnMut(ThemeMode) + 'static) -> Element {
    let onchange = Rc::new(RefCell::new(onchange));
    rsx! {
        h2 { id: "appearance", style: "border:0;margin:24px 0 12px", "{chrome.prefs_section_label(\"appearance\")}" }
        Card { title: chrome.prefs_theme_title(),
            div {
                class: "resn-set",
                role: "radiogroup",
                "aria-label": "{chrome.prefs_theme_radiogroup_label()}",
                style: "gap:8px",
                for choice in [ThemeMode::Light, ThemeMode::Dark, ThemeMode::System] {
                    {
                        let checked = choice == mode;
                        let onchange = Rc::clone(&onchange);
                        rsx! {
                            button {
                                class: "resn",
                                role: "radio",
                                aria_checked: if checked { "true" } else { "false" },
                                onclick: move |_| (onchange.borrow_mut())(choice),
                                "{chrome.theme_mode_label(choice)}"
                            }
                        }
                    }
                }
            }
            div { class: "muted", style: "font-size:var(--fs-sm);margin-top:8px", "{chrome.prefs_theme_system_note()}" }
        }
    }
}

/// The "Language & locale" card: UI language / data locale selects (seeded from the *resolved*
/// value — a workspace override, if pinned, is what the user sees and edits) plus the fallback
/// chain resolved live from the current UI-language field, so it updates as the user picks.
fn locale_card(
    chrome: &Chrome,
    resolved: &ResolvedLocale,
    mut ui_language: Signal<String>,
    mut data_locale: Signal<String>,
) -> Element {
    let requested = requested_or(parse_tag(&ui_language()));
    let fallback: LanguageIdentifier = "en".parse().unwrap_or_default();
    let chain = fallback_chain(&requested, &fallback);
    rsx! {
        h2 { id: "locale", style: "border:0;margin:24px 0 12px", "{chrome.prefs_section_label(\"locale\")}" }
        Card { title: chrome.prefs_locale_title(),
            div { class: "grid-2",
                Select {
                    label: chrome.prefs_ui_language_label(),
                    name: "ui-language".to_owned(),
                    value: Some(ui_language()),
                    options: language_options(chrome, resolved.ui_language.as_ref()),
                    onchange: move |event: FormEvent| ui_language.set(event.value()),
                }
                Select {
                    label: format!("{} — {}", chrome.prefs_data_locale_label(), chrome.prefs_data_locale_hint()),
                    name: "data-locale".to_owned(),
                    value: Some(data_locale()),
                    options: language_options(chrome, resolved.data_locale.as_ref()),
                    onchange: move |event: FormEvent| data_locale.set(event.value()),
                }
            }
            div { class: "fact-row", style: "margin-top:4px",
                span { class: "field-label", style: "margin:0", "{chrome.prefs_fallback_chain_label()}" }
                span { class: "wrap",
                    for (index , tag) in chain.iter().enumerate() {
                        if index > 0 {
                            span { class: "faint", "→" }
                        }
                        Chip { label: tag.to_string() }
                    }
                }
                span { class: "muted", style: "font-size:var(--fs-sm)", "{chrome.prefs_fallback_chain_note()}" }
            }
            div { class: "muted", style: "font-size:var(--fs-sm);margin-top:8px", "{chrome.prefs_locale_note()}" }
        }
    }
}

/// The language-select options: the "follow system" default plus every UI language the app ships.
/// `resolved` seeds the "extra" option appended when the resolved tag isn't one of the fixed ones
/// (e.g. a workspace override in some other language).
fn language_options(chrome: &Chrome, resolved: Option<&LanguageIdentifier>) -> Vec<SelectChoice> {
    let system = requested_or(None).first().map(ToString::to_string).unwrap_or_default();
    let mut options = vec![SelectChoice {
        value: String::new(),
        label: chrome.prefs_follow_system(&system),
    }];
    for tag in ["en", "no", "nb-NO", "nn-NO"] {
        options.push(SelectChoice {
            value: tag.to_owned(),
            label: tag.to_owned(),
        });
    }
    if let Some(resolved) = resolved
        && !options.iter().any(|option| option.value == resolved.to_string())
    {
        options.push(SelectChoice {
            value: resolved.to_string(),
            label: resolved.to_string(),
        });
    }
    options
}

/// Resolves the requested-languages list: `override_tag` alone when the user pinned one, else the
/// live system request (matching how `Localizer`/`Chrome` already resolve with no override).
fn requested_or(override_tag: Option<LanguageIdentifier>) -> Vec<LanguageIdentifier> {
    match override_tag {
        Some(tag) => vec![tag],
        None => DesktopLanguageRequester::requested_languages(),
    }
}

/// The empty-string `<select>` sentinel for "follow the system" (no override), or the tag's own
/// string form.
fn optional_tag(tag: Option<&LanguageIdentifier>) -> String {
    tag.map(ToString::to_string).unwrap_or_default()
}

/// The inverse of [`optional_tag`]: an empty field means "follow the system" (`None`); anything
/// else is parsed as a BCP-47 tag, falling back to `None` if it fails to parse (never blocks save).
fn parse_tag(value: &str) -> Option<LanguageIdentifier> {
    if value.is_empty() {
        return None;
    }
    value.parse().ok()
}

/// The "Date & number format" card: format selects plus a live example rendered from the current
/// (unsaved) selection, so it updates as the user picks.
fn formats_card(chrome: &Chrome, mut date_format: Signal<String>, mut number_format: Signal<String>) -> Element {
    let date_example = date_example(date_format_from_value(&date_format()));
    let number_example = number_example(number_format_from_value(&number_format()));
    rsx! {
        h2 { id: "formats", style: "border:0;margin:24px 0 12px", "{chrome.prefs_section_label(\"formats\")}" }
        Card { title: chrome.prefs_formats_title(),
            div { class: "grid-2",
                Select {
                    label: chrome.prefs_date_format_label(),
                    name: "date-format".to_owned(),
                    value: Some(date_format()),
                    options: date_format_options(chrome),
                    onchange: move |event: FormEvent| date_format.set(event.value()),
                }
                Select {
                    label: chrome.prefs_number_format_label(),
                    name: "number-format".to_owned(),
                    value: Some(number_format()),
                    options: number_format_options(chrome),
                    onchange: move |event: FormEvent| number_format.set(event.value()),
                }
            }
            div { class: "fact-row", style: "margin-top:4px",
                span { class: "field-label", style: "margin:0", "{chrome.prefs_live_example_label()}" }
                span { class: "grow mono", "{date_example}" }
                span { class: "muted", "·" }
                span { class: "mono", "{number_example}" }
            }
            div { class: "muted", style: "font-size:var(--fs-sm);margin-top:6px", "{chrome.prefs_formats_note()}" }
        }
    }
}

/// The stable `<select>` value token for a [`DateFormat`] variant.
fn date_format_value(format: DateFormat) -> &'static str {
    match format {
        DateFormat::Long => "long",
        DateFormat::Medium => "medium",
        DateFormat::Numeric => "numeric",
        DateFormat::LocaleDefault => "locale-default",
    }
}

/// The inverse of [`date_format_value`]: an unrecognized token (should not happen — the field is a
/// `<select>` over exactly these tokens) falls back to [`DateFormat::LocaleDefault`] rather than
/// panicking, so a stray value never blocks Save.
fn date_format_from_value(value: &str) -> DateFormat {
    match value {
        "long" => DateFormat::Long,
        "medium" => DateFormat::Medium,
        "numeric" => DateFormat::Numeric,
        _ => DateFormat::LocaleDefault,
    }
}

/// The stable `<select>` value token for a [`NumberFormat`] variant.
fn number_format_value(format: NumberFormat) -> &'static str {
    match format {
        NumberFormat::SpaceComma => "space-comma",
        NumberFormat::CommaPoint => "comma-point",
        NumberFormat::LocaleDefault => "locale-default",
    }
}

/// The inverse of [`number_format_value`]; an unrecognized token falls back to
/// [`NumberFormat::LocaleDefault`] (see [`date_format_from_value`]).
fn number_format_from_value(value: &str) -> NumberFormat {
    match value {
        "space-comma" => NumberFormat::SpaceComma,
        "comma-point" => NumberFormat::CommaPoint,
        _ => NumberFormat::LocaleDefault,
    }
}

/// Every [`DateFormat`] option, localized with its worked example.
fn date_format_options(chrome: &Chrome) -> Vec<SelectChoice> {
    [
        DateFormat::Long,
        DateFormat::Medium,
        DateFormat::Numeric,
        DateFormat::LocaleDefault,
    ]
    .into_iter()
    .map(|format| SelectChoice {
        value: date_format_value(format).to_owned(),
        label: chrome.prefs_date_format_option(format, date_example(format)),
    })
    .collect()
}

/// Every [`NumberFormat`] option, localized with its worked example.
fn number_format_options(chrome: &Chrome) -> Vec<SelectChoice> {
    [
        NumberFormat::SpaceComma,
        NumberFormat::CommaPoint,
        NumberFormat::LocaleDefault,
    ]
    .into_iter()
    .map(|format| SelectChoice {
        value: number_format_value(format).to_owned(),
        label: chrome.prefs_number_format_option(format, number_example(format)),
    })
    .collect()
}

/// The fixed example date rendered in `format`'s style.
fn date_example(format: DateFormat) -> &'static str {
    match format {
        DateFormat::Long | DateFormat::LocaleDefault => EXAMPLE_DATE_LONG,
        DateFormat::Medium => EXAMPLE_DATE_MEDIUM,
        DateFormat::Numeric => EXAMPLE_DATE_NUMERIC,
    }
}

/// The fixed example number rendered in `format`'s style.
fn number_example(format: NumberFormat) -> &'static str {
    match format {
        NumberFormat::SpaceComma | NumberFormat::LocaleDefault => EXAMPLE_NUMBER_SPACE_COMMA,
        NumberFormat::CommaPoint => EXAMPLE_NUMBER_COMMA_POINT,
    }
}

/// The "Workspace defaults" card: the editable Person id format (the worked example the layer chain
/// below explains), the three-layer override chain for both theme and that format, and the
/// registered-workspace switcher.
fn defaults_card(
    chrome: &Chrome,
    data: &PreferencesData,
    mut person_id_format: Signal<String>,
    onswitch: impl FnMut(String) + 'static,
) -> Element {
    let onswitch = Rc::new(RefCell::new(onswitch));
    let active = data.config.default.clone();
    rsx! {
        h2 { id: "defaults", style: "border:0;margin:24px 0 12px", "{chrome.prefs_section_label(\"defaults\")}" }
        Card { title: chrome.prefs_defaults_title(),
            div { class: "muted", style: "font-size:var(--fs-sm);margin-bottom:12px",
                "{chrome.prefs_defaults_intro()} {chrome.prefs_defaults_worked_example()}"
            }
            Input {
                label: chrome.prefs_person_id_format_label(),
                name: "person-id-format".to_owned(),
                value: Some(person_id_format()),
                oninput: move |event: FormEvent| person_id_format.set(event.value()),
            }
            div { class: "stack", style: "margin-top:8px",
                {layer_row(chrome, &chrome.prefs_layer_workspace("workspace.toml"), data.layers.theme.winner == LayerKind::Workspace)}
                {layer_row(chrome, &chrome.prefs_layer_shared("~/.config/genealogy/config.toml"), data.layers.theme.winner == LayerKind::SharedDefault)}
                {layer_row(chrome, &chrome.prefs_layer_embedded(), data.layers.theme.winner == LayerKind::Embedded)}
            }
            div { class: "muted", style: "font-size:var(--fs-sm);margin-top:10px", "{chrome.prefs_defaults_footnote()}" }
        }
        Card { title: chrome.prefs_workspaces_title(),
            div { class: "stack",
                for (name , _entry) in &data.config.workspaces {
                    {workspace_row(chrome, name, active.as_deref() == Some(name.as_str()), Rc::clone(&onswitch))}
                }
            }
        }
    }
}

/// One override-chain row: the `wins`/`fallback` badge plus the layer's own label.
fn layer_row(chrome: &Chrome, label: &str, wins: bool) -> Element {
    let badge = if wins {
        chrome.prefs_layer_wins()
    } else {
        chrome.prefs_layer_fallback()
    };
    rsx! {
        div { class: "fact-row card", style: "margin:0",
            Badge { label: badge }
            span { class: "grow", "{label}" }
        }
    }
}

/// One registered-workspace row: its name, an "Active" badge when it is the current default, else a
/// "switch to" button.
fn workspace_row(
    chrome: &Chrome,
    name: &str,
    active: bool,
    onswitch: Rc<RefCell<impl FnMut(String) + 'static>>,
) -> Element {
    let name = name.to_owned();
    rsx! {
        div { class: "fact-row card", style: "margin:0",
            span { class: "grow", "{name}" }
            if active {
                Badge { label: chrome.prefs_workspace_active() }
            } else {
                Button {
                    label: chrome.prefs_switch_to(&name),
                    variant: ButtonVariant::Default,
                    onclick: move |_| (onswitch.borrow_mut())(name.clone()),
                }
            }
        }
    }
}
