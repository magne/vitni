//! The Preferences tool (Phase 5 PR 20; `docs/mockups/preferences.html`): operator identity,
//! appearance, language & locale, date & number format, the workspace-defaults override chain, and
//! the registered-workspaces table (open / make default / register). Unlike the aggregate slices,
//! Preferences talks straight to `genealogy-app` config read/write use-cases
//! (`crate::services::{load_preferences, save_*, make_default_workspace, register_workspace}` and
//! `crate::app::open_workspace`) — there is no `genealogy_ui::dispatch`/`Intent` involved, since
//! preferences are not an aggregate.
//!
//! Fields are edited inline inside cards (per `docs/mockups/edit-patterns.html`: simple fields use
//! the inline convention, not a side panel/modal), and a single "Save preferences" commits every
//! section's pending edits in one pass. Two controls are exceptions that act immediately: the theme
//! control (through the same `save_theme_mode` the top-bar toggle uses, so the two stay in sync),
//! and the Workspaces card's Open / Make default / Register actions (each a distinct config
//! operation, not part of the batched Save).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use genealogy_app::{
    DateFormat, Engine, IdFormats, LayerKind, LocaleDefaults, NumberFormat, ResolvedLocale, ShortcutConfig,
    SuretyLabelOverride, SuretyLabelOverrides, ThemeMode, WorkspaceSummary, requested_languages_for,
};
use genealogy_i18n::fallback_chain;
use genealogy_ui::{ShortcutBindingVm, ShortcutGroup, ShortcutsVm, resolved_shortcuts, shortcuts, shortcuts_vm};
use i18n_embed::DesktopLanguageRequester;
use unic_langid::LanguageIdentifier;

use super::prelude::*;
use crate::app::{open_workspace, request_restart};
use crate::components::{Badge, LabeledValue, TextField};
use crate::i18n::Chrome;
use crate::services::{
    PreferencesData, load_preferences, make_default_workspace, register_workspace, save_id_format_defaults,
    save_locale_defaults, save_operator_identity, save_shortcuts, save_surety_defaults,
};
use crate::shell::ShortcutsCtx;

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
    // The live client-scope shortcut overrides (ADR 0030 §3): absent under a bare SSR render of this
    // screen (no `Shell`), present in the real app so a save takes effect without a restart.
    let shortcuts_ctx = try_consume_context::<ShortcutsCtx>();

    let mut display = use_signal(|| data().config.operator.display.clone().unwrap_or_default());
    let mut email = use_signal(|| data().config.operator.email.clone().unwrap_or_default());
    let mut person_id_format = use_signal(|| data().layers.person_id_format.shared_default.clone());
    let mut ui_language = use_signal(|| optional_tag(data().locale.ui_language.as_ref()));
    let mut data_locale = use_signal(|| optional_tag(data().locale.data_locale.as_ref()));
    let mut date_format = use_signal(|| date_format_value(data().locale.date_format).to_owned());
    let mut number_format = use_signal(|| number_format_value(data().locale.number_format).to_owned());
    let mut surety_very_low = use_signal(|| surety_field_from_override(data().surety.very_low.as_ref()));
    let mut surety_low = use_signal(|| surety_field_from_override(data().surety.low.as_ref()));
    let mut surety_normal = use_signal(|| surety_field_from_override(data().surety.normal.as_ref()));
    let mut surety_high = use_signal(|| surety_field_from_override(data().surety.high.as_ref()));
    let mut surety_very_high = use_signal(|| surety_field_from_override(data().surety.very_high.as_ref()));
    let mut shortcut_bindings = use_signal(|| shortcut_field_seed(&data().shortcuts));

    let save_services = services.clone();
    let onsave = move |_| {
        let shortcuts_config = shortcuts_config_from_fields(&shortcut_bindings());
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
            })
            .and_then(|()| {
                let surety = SuretyLabelOverrides {
                    very_low: surety_override_from_field(&surety_very_low()),
                    low: surety_override_from_field(&surety_low()),
                    normal: surety_override_from_field(&surety_normal()),
                    high: surety_override_from_field(&surety_high()),
                    very_high: surety_override_from_field(&surety_very_high()),
                };
                save_surety_defaults(&save_services, surety)
            })
            .and_then(|()| save_shortcuts(&save_services, &shortcuts_config));
        match outcome {
            Ok(()) => {
                data.set(load_preferences(&save_services));
                if let Some(mut ctx) = shortcuts_ctx {
                    ctx.0.set(shortcuts_config);
                }
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

    // Open switches the session (in-memory override + restart); Make default persists the default
    // and refreshes the card in place (no restart).
    let onopen = move |name: String| open_workspace(name);
    let makedefault_services = services.clone();
    let onmakedefault = move |name: String| match make_default_workspace(&makedefault_services, &name) {
        Ok(()) => data.set(load_preferences(&makedefault_services)),
        Err(message) => status.set(Some(message)),
    };

    let register = RegisterFields {
        open: use_signal(|| false),
        name: use_signal(String::new),
        directory: use_signal(String::new),
        database_url: use_signal(String::new),
    };
    let register_services = services.clone();
    let register_chrome = chrome.clone();
    let onregister = move |_: MouseEvent| {
        let name = register.name.peek().trim().to_owned();
        if name.is_empty() {
            status.set(Some(register_chrome.prefs_register_name_required()));
            return;
        }
        let directory = non_empty(register.directory.peek().clone()).map(PathBuf::from);
        let database_url = non_empty(register.database_url.peek().clone());
        let register_services = register_services.clone();
        spawn(async move {
            match register_workspace(&register_services, &name, directory, database_url.as_deref()).await {
                Ok(()) => request_restart(),
                Err(message) => status.set(Some(message)),
            }
        });
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
        surety_very_low.set(surety_field_from_override(loaded.surety.very_low.as_ref()));
        surety_low.set(surety_field_from_override(loaded.surety.low.as_ref()));
        surety_normal.set(surety_field_from_override(loaded.surety.normal.as_ref()));
        surety_high.set(surety_field_from_override(loaded.surety.high.as_ref()));
        surety_very_high.set(surety_field_from_override(loaded.surety.very_high.as_ref()));
        shortcut_bindings.set(shortcut_field_seed(&loaded.shortcuts));
        status.set(None);
    };

    let shortcuts_vm_value = shortcuts_vm(&data().shortcuts, state.data_loc());

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
        SuretyFields {
            very_low: surety_very_low,
            low: surety_low,
            normal: surety_normal,
            high: surety_high,
            very_high: surety_very_high,
        },
        &shortcuts_vm_value,
        ShortcutFields {
            bindings: shortcut_bindings,
        },
        status(),
        onsave,
        onreset,
        onthemechange,
        register,
        onopen,
        onmakedefault,
        onregister,
    )
}

/// The chord-string value each rebindable ([`ShortcutGroup::Global`]) action's field seeds to: the
/// currently-effective chord (an accepted override, else the default) — no [`genealogy_ui::Localizer`]
/// needed, since only the *display* labels/errors are localized, not the plain chord strings.
fn shortcut_field_seed(config: &ShortcutConfig) -> BTreeMap<String, String> {
    let (resolved, _errors) = resolved_shortcuts(&config.bindings);
    resolved
        .into_iter()
        .filter(|entry| entry.group == ShortcutGroup::Global)
        .map(|entry| (entry.action.config_id().to_owned(), entry.chord.to_string()))
        .collect()
}

/// The inverse of [`shortcut_field_seed`]: builds the `[shortcuts]` config to persist from the
/// editable fields. A field left blank, or matching its action's default chord, is omitted — the
/// action then falls back to the default, exactly like [`surety_override_from_field`]'s "blank keeps
/// the default" convention.
fn shortcuts_config_from_fields(bindings: &BTreeMap<String, String>) -> ShortcutConfig {
    let mut result = BTreeMap::new();
    for entry in shortcuts()
        .into_iter()
        .filter(|entry| entry.group == ShortcutGroup::Global)
    {
        let config_id = entry.action.config_id();
        let Some(value) = bindings.get(config_id) else {
            continue;
        };
        let trimmed = value.trim();
        if !trimmed.is_empty() && trimmed != entry.chord.to_string() {
            result.insert(config_id.to_owned(), trimmed.to_owned());
        }
    }
    ShortcutConfig { bindings: result }
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

/// The editable surety-scheme label fields (ADR 0027), one per fixed `Confidence` ordinal. Each is
/// the raw text field value: empty means "no override" (the Fluent-resolved default wins). Grouped
/// into one struct so [`preferences_view`]'s signature stays readable (mirrors [`LocaleFields`]).
#[derive(Debug, Clone, Copy)]
pub struct SuretyFields {
    /// The `Confidence::VeryLow` label override field.
    pub very_low: Signal<String>,
    /// The `Confidence::Low` label override field.
    pub low: Signal<String>,
    /// The `Confidence::Normal` label override field.
    pub normal: Signal<String>,
    /// The `Confidence::High` label override field.
    pub high: Signal<String>,
    /// The `Confidence::VeryHigh` label override field.
    pub very_high: Signal<String>,
}

/// The editable per-action shortcut-override fields (ADR 0030 §4): current chord-string values,
/// keyed by the action's config id (the `[shortcuts]` key). Unlike [`SuretyFields`]'s five fixed
/// named fields, the rebindable row set is `ShortcutAction::all()`'s `Global` subset, so this holds
/// one map rather than one signal per action.
#[derive(Debug, Clone, Copy)]
pub struct ShortcutFields {
    /// The chord-string value for each rebindable action's config id.
    pub bindings: Signal<BTreeMap<String, String>>,
}

/// The "Register workspace…" inline disclosure form's state: whether it is open, and the (trimmed
/// on submit) name and optional directory. Grouped into one struct so [`preferences_view`]'s
/// signature stays readable (mirrors [`LocaleFields`]).
#[derive(Debug, Clone, Copy)]
pub struct RegisterFields {
    /// Whether the disclosure form is open.
    pub open: Signal<bool>,
    /// The workspace name (required; trimmed on submit).
    pub name: Signal<String>,
    /// The optional workspace directory (empty ⇒ the default data directory).
    pub directory: Signal<String>,
    /// The optional Postgres connection URL (empty ⇒ the default SQLite engine). Kept on the struct
    /// unconditionally so the submit-handler plumbing needs no `cfg`; only its field in
    /// [`register_form`] is gated behind the `postgres` feature, since a default build never lets a
    /// GUI user reach it (`genealogy-app`'s postgres backend isn't compiled in either).
    pub database_url: Signal<String>,
}

/// Renders the settings sub-nav + every card. A pure function of its inputs (data, the current
/// theme mode, the editable-field signals, and plain callbacks) so the SSR test can exercise it with
/// hand-built fixtures — no `AppCtx`/plugin host required (mirrors `dashboard_view`).
#[expect(
    clippy::too_many_arguments,
    reason = "one screen, one render entry point; splitting the sub-nav + six cards' shared inputs into a struct would just move the same fields around"
)]
pub fn preferences_view(
    chrome: &Chrome,
    data: &PreferencesData,
    theme_mode: ThemeMode,
    display: Signal<String>,
    email: Signal<String>,
    person_id_format: Signal<String>,
    locale_fields: LocaleFields,
    surety_fields: SuretyFields,
    shortcuts_vm: &ShortcutsVm,
    shortcut_fields: ShortcutFields,
    status: Option<String>,
    onsave: impl FnMut(MouseEvent) + 'static,
    onreset: impl FnMut(MouseEvent) + 'static,
    onthemechange: impl FnMut(ThemeMode) + 'static,
    register: RegisterFields,
    onopen: impl FnMut(String) + 'static,
    onmakedefault: impl FnMut(String) + 'static,
    onregister: impl FnMut(MouseEvent) + 'static,
) -> Element {
    rsx! {
        div { style: "display:grid;grid-template-columns:200px 1fr;height:100%;min-height:0",
            nav { class: "list", "aria-label": "{chrome.prefs_nav_label()}", style: "border-right:1px solid var(--line)",
                div { class: "list-rows", style: "padding:var(--sp-2)",
                    for id in ["identity", "appearance", "locale", "formats", "surety", "shortcuts", "defaults"] {
                        a { class: "nav-item", href: "#{id}", "{chrome.prefs_section_label(id)}" }
                    }
                }
            }
            div { style: "padding:var(--sp-6);overflow:auto;height:100%",
                h1 { class: "sr-only", "{chrome.rail_label(\"nav-preferences\")}" }
                {identity_card(chrome, &data.config.operator.id.to_string(), display, email)}
                {appearance_card(chrome, theme_mode, onthemechange)}
                {locale_card(chrome, &data.locale, locale_fields.ui_language, locale_fields.data_locale)}
                {formats_card(chrome, locale_fields.date_format, locale_fields.number_format)}
                {surety_card(chrome, surety_fields)}
                {shortcuts_card(chrome, shortcuts_vm, shortcut_fields)}
                {defaults_card(chrome, data, person_id_format)}
                {workspaces_card(chrome, data, register, onopen, onmakedefault, onregister)}
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

/// The "Appearance" card: the theme radiogroup — the reusable single-choice [`RadioGroup`]
/// (`role="radio"`/`aria-checked` with a roving tab stop), distinct from the multi-select
/// `RestrictionSet`'s `aria-pressed` toggle group. The call site maps [`ThemeMode`] to and from the
/// component's string ids.
fn appearance_card(chrome: &Chrome, mode: ThemeMode, mut onchange: impl FnMut(ThemeMode) + 'static) -> Element {
    let choices = [ThemeMode::Light, ThemeMode::Dark, ThemeMode::System]
        .into_iter()
        .map(|choice| RadioChoice {
            id: theme_mode_id(choice).to_owned(),
            label: chrome.theme_mode_label(choice),
        })
        .collect();
    rsx! {
        h2 { id: "appearance", style: "border:0;margin:24px 0 12px", "{chrome.prefs_section_label(\"appearance\")}" }
        Card { title: chrome.prefs_theme_title(),
            RadioGroup {
                group_label: chrome.prefs_theme_radiogroup_label(),
                choices,
                selected: theme_mode_id(mode).to_owned(),
                onselect: move |id: String| onchange(theme_mode_from_id(&id)),
            }
            div { class: "muted", style: "font-size:var(--fs-sm);margin-top:8px", "{chrome.prefs_theme_system_note()}" }
        }
    }
}

/// The stable [`RadioGroup`] choice id for a [`ThemeMode`] variant.
fn theme_mode_id(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
        ThemeMode::System => "system",
    }
}

/// The inverse of [`theme_mode_id`]: an unrecognized id (impossible — the group only emits the ids
/// above) falls back to [`ThemeMode::System`] rather than panicking.
fn theme_mode_from_id(id: &str) -> ThemeMode {
    match id {
        "light" => ThemeMode::Light,
        "dark" => ThemeMode::Dark,
        _ => ThemeMode::System,
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
    let requested = requested_languages_for(
        parse_tag(&ui_language()).as_ref(),
        &DesktopLanguageRequester::requested_languages(),
    );
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
    let system = requested_languages_for(None, &DesktopLanguageRequester::requested_languages())
        .first()
        .map(ToString::to_string)
        .unwrap_or_default();
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

/// The surety-field text value for one ordinal's current override, or empty when unset (ADR 0027).
fn surety_field_from_override(override_: Option<&SuretyLabelOverride>) -> String {
    override_.map(|o| o.label.clone()).unwrap_or_default()
}

/// The inverse of [`surety_field_from_override`]: an empty (or whitespace-only) field means "no
/// override" (`None`, the Fluent default wins); anything else becomes the workspace's own label,
/// with no description set from this form (ADR 0027).
fn surety_override_from_field(value: &str) -> Option<SuretyLabelOverride> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(SuretyLabelOverride {
        label: trimmed.to_owned(),
        description: None,
    })
}

/// The "Rebind global shortcuts" card (ADR 0030): one text field per rebindable ([`ShortcutGroup::Global`])
/// action, taking the canonical chord string (`mod+shift+alt+key`). Any override this workspace could
/// not apply is shown inline (per row when it names one, else in a general list) rather than dropped.
fn shortcuts_card(chrome: &Chrome, vm: &ShortcutsVm, fields: ShortcutFields) -> Element {
    rsx! {
        h2 { id: "shortcuts", style: "border:0;margin:24px 0 12px", "{chrome.prefs_section_label(\"shortcuts\")}" }
        Card { title: chrome.prefs_shortcuts_title(),
            div { class: "muted", style: "font-size:var(--fs-sm);margin-bottom:12px", "{chrome.prefs_shortcuts_intro()}" }
            div { class: "grid-2",
                for row in &vm.rows {
                    {shortcut_field_row(chrome, row, fields.bindings)}
                }
            }
            if !vm.general_errors.is_empty() {
                div { class: "field-error", style: "margin-top:8px",
                    "{chrome.prefs_shortcuts_general_errors()}"
                    ul {
                        for message in &vm.general_errors {
                            li { "{message}" }
                        }
                    }
                }
            }
        }
    }
}

/// One rebindable action's chord field: labelled with its `sc-*` description, the default chord
/// shown as the hint (or as the placeholder-style error when the override was rejected).
fn shortcut_field_row(
    chrome: &Chrome,
    row: &ShortcutBindingVm,
    mut bindings: Signal<BTreeMap<String, String>>,
) -> Element {
    let label = chrome.shortcut_label(row.label_id);
    let config_id = row.config_id.clone();
    let value = bindings.read().get(&config_id).cloned().unwrap_or_default();
    let hint = chrome.prefs_shortcuts_default_hint(&row.default_chord);
    rsx! {
        TextField {
            label,
            name: format!("shortcut-{config_id}"),
            value,
            invalid: row.error.is_some(),
            error: row.error.clone(),
            hint,
            oninput: move |event: FormEvent| {
                bindings.write().insert(config_id.clone(), event.value());
            },
        }
    }
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

/// The "Surety scheme" card (ADR 0027): one text field per fixed `Confidence` ordinal. An empty
/// field keeps the Fluent-resolved default wording; a filled-in field is shown verbatim (not
/// translated) in every locale, since it is the workspace's own chosen word.
fn surety_card(chrome: &Chrome, surety_fields: SuretyFields) -> Element {
    rsx! {
        h2 { id: "surety", style: "border:0;margin:24px 0 12px", "{chrome.prefs_section_label(\"surety\")}" }
        Card { title: chrome.prefs_surety_title(),
            div { class: "muted", style: "font-size:var(--fs-sm);margin-bottom:12px", "{chrome.prefs_surety_intro()}" }
            div { class: "grid-2",
                {surety_field(chrome, "very-low", surety_fields.very_low)}
                {surety_field(chrome, "low", surety_fields.low)}
                {surety_field(chrome, "normal", surety_fields.normal)}
                {surety_field(chrome, "high", surety_fields.high)}
                {surety_field(chrome, "very-high", surety_fields.very_high)}
            }
            div { class: "muted", style: "font-size:var(--fs-sm);margin-top:8px", "{chrome.prefs_surety_hint()}" }
        }
    }
}

/// One surety-ordinal text field, labelled with its fixed Fluent-resolved default wording as both
/// the accessible label and the placeholder shown when the field is empty (no override).
fn surety_field(chrome: &Chrome, ordinal: &str, mut field: Signal<String>) -> Element {
    let label = chrome.prefs_surety_field_label(ordinal);
    rsx! {
        Input {
            label: label.clone(),
            name: format!("surety-{ordinal}"),
            value: Some(field()),
            placeholder: Some(label),
            oninput: move |event: FormEvent| field.set(event.value()),
        }
    }
}

/// The "Workspace defaults" card: the editable Person id format (the worked example the layer chain
/// below explains), the three-layer override chain for both theme and that format, and the
/// registered-workspace switcher.
fn defaults_card(chrome: &Chrome, data: &PreferencesData, mut person_id_format: Signal<String>) -> Element {
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

/// The "Registered workspaces" card: a table (name, Active/Default badges, path, engine, actions)
/// over the summaries, plus the "+ Register workspace…" inline disclosure form. Open switches the
/// session (restart); Make default persists the default (no restart); Register creates a new one.
fn workspaces_card(
    chrome: &Chrome,
    data: &PreferencesData,
    register: RegisterFields,
    onopen: impl FnMut(String) + 'static,
    onmakedefault: impl FnMut(String) + 'static,
    onregister: impl FnMut(MouseEvent) + 'static,
) -> Element {
    let onopen = Rc::new(RefCell::new(onopen));
    let onmakedefault = Rc::new(RefCell::new(onmakedefault));
    rsx! {
        Card { title: chrome.prefs_workspaces_title(),
            table { class: "tbl", style: "margin-top:4px",
                caption { class: "sr-only", "{chrome.prefs_workspaces_title()}" }
                thead {
                    tr {
                        th { "{chrome.prefs_workspace_col_name()}" }
                        th {}
                        th { "{chrome.prefs_workspace_col_path()}" }
                        th { "{chrome.prefs_workspace_col_engine()}" }
                        th {
                            span { class: "sr-only", "{chrome.table_actions()}" }
                        }
                    }
                }
                tbody {
                    for summary in &data.workspaces {
                        {workspace_row(chrome, summary, &data.open_workspace, Rc::clone(&onopen), Rc::clone(&onmakedefault))}
                    }
                }
            }
            {register_form(chrome, register, onregister)}
            div { class: "muted", style: "font-size:var(--fs-sm);margin-top:6px", "{chrome.prefs_workspaces_note()}" }
        }
    }
}

/// One workspace table row: name, the Active (open) / Default (config default) badges, the path
/// (`.mono`), the engine chip, and the Open / Make default actions applicable to that row.
fn workspace_row(
    chrome: &Chrome,
    summary: &WorkspaceSummary,
    open_workspace: &str,
    onopen: Rc<RefCell<impl FnMut(String) + 'static>>,
    onmakedefault: Rc<RefCell<impl FnMut(String) + 'static>>,
) -> Element {
    let name = summary.name.clone();
    let is_open = name == open_workspace;
    let open_name = name.clone();
    let default_name = name.clone();
    rsx! {
        tr {
            td {
                b { "{name}" }
            }
            td {
                if is_open {
                    Badge { label: chrome.prefs_workspace_active() }
                }
                if summary.is_default {
                    Badge { label: chrome.prefs_workspace_default() }
                }
            }
            td { class: "mono", "{summary.path.display()}" }
            td {
                span { class: "chip", "{engine_label(summary.engine)}" }
            }
            td { class: "row-actions",
                if !is_open {
                    Button {
                        label: chrome.prefs_open_workspace(),
                        aria_label: chrome.prefs_open_workspace_label(&name),
                        variant: ButtonVariant::Ghost,
                        small: true,
                        onclick: move |_| (onopen.borrow_mut())(open_name.clone()),
                    }
                }
                if !summary.is_default {
                    Button {
                        label: chrome.prefs_make_default(),
                        aria_label: chrome.prefs_make_default_label(&name),
                        variant: ButtonVariant::Ghost,
                        small: true,
                        onclick: move |_| (onmakedefault.borrow_mut())(default_name.clone()),
                    }
                }
            }
        }
    }
}

/// The product name shown in a workspace's engine chip (a product name, not localized), or `—` when
/// the engine could not be determined.
fn engine_label(engine: Option<Engine>) -> &'static str {
    match engine {
        Some(Engine::Sqlite) => "SQLite",
        Some(Engine::Postgres) => "PostgreSQL",
        None => "—",
    }
}

/// The "+ Register workspace…" button and its inline disclosure form (Name required, Directory
/// optional with a default-data-dir hint, an opt-in Database URL field behind the `postgres`
/// feature, Register/Cancel).
fn register_form(chrome: &Chrome, register: RegisterFields, onregister: impl FnMut(MouseEvent) + 'static) -> Element {
    let RegisterFields {
        mut open,
        mut name,
        mut directory,
        database_url,
    } = register;
    rsx! {
        div { class: "row-actions", style: "margin-top:8px",
            Button {
                label: chrome.prefs_register_workspace(),
                variant: ButtonVariant::Primary,
                small: true,
                onclick: move |_| open.set(!open()),
            }
        }
        if open() {
            div { class: "stack", style: "margin-top:8px",
                Input {
                    label: chrome.prefs_register_name_label(),
                    name: "register-name".to_owned(),
                    value: Some(name()),
                    oninput: move |event: FormEvent| name.set(event.value()),
                }
                Input {
                    label: chrome.prefs_register_path_label(),
                    name: "register-directory".to_owned(),
                    value: Some(directory()),
                    oninput: move |event: FormEvent| directory.set(event.value()),
                }
                div { class: "muted", style: "font-size:var(--fs-sm)", "{chrome.prefs_register_path_hint()}" }
                {database_url_field(chrome, database_url)}
                div { class: "row-actions",
                    Button {
                        label: chrome.prefs_register_submit(),
                        variant: ButtonVariant::Primary,
                        small: true,
                        onclick: onregister,
                    }
                    Button {
                        label: chrome.prefs_register_cancel(),
                        variant: ButtonVariant::Default,
                        small: true,
                        onclick: move |_| open.set(false),
                    }
                }
            }
        }
    }
}

/// The optional "Database URL" field: freezes a Postgres connection string into the manifest at
/// registration (mirrors `genealogy init --database-url`); empty keeps the default SQLite engine.
/// Gated behind the `postgres` feature — off by default, so the field never appears unless the
/// binary was built to support it.
#[cfg(feature = "postgres")]
fn database_url_field(chrome: &Chrome, mut database_url: Signal<String>) -> Element {
    rsx! {
        Input {
            label: chrome.prefs_register_database_url_label(),
            name: "register-database-url".to_owned(),
            value: Some(database_url()),
            oninput: move |event: FormEvent| database_url.set(event.value()),
        }
        div { class: "muted", style: "font-size:var(--fs-sm)", "{chrome.prefs_register_database_url_hint()}" }
    }
}

/// The `postgres`-off counterpart of [`database_url_field`]: renders nothing, so [`register_form`]
/// stays unconditional while the field itself disappears from a default build.
#[cfg(not(feature = "postgres"))]
fn database_url_field(_chrome: &Chrome, _database_url: Signal<String>) -> Element {
    rsx! {}
}
