use genealogy_ui::{DEFAULT_TAG_COLOR, DEFAULT_TAG_PRIORITY};

use super::prelude::*;
use crate::components::{ColorPicker, IconButton, Tabs};
use crate::screens::RecordDetail;
use crate::shell::focus_trap::keep_typing_local;

/// The tag master-detail screen: a list of tags (colour dot + name) on the left, the selected tag's
/// detail (an editable Overview record + usage + history) on the right. Tags carry no `human_id`; the
/// nav key is the stable tag id (a UUID, never rendered — data-model §9). `+ New` opens an
/// uncommitted draft record in the detail pane (nothing is created until Save).
#[component]
pub fn TagScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Tags.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().tag_list_empty();
    let dismiss_label = state.data_loc().action_label("dismiss");
    let list_chrome = ListChrome {
        list_label: entity.clone(),
        filter_placeholder: chrome.list_filter(&entity),
        empty,
    };
    let mut nav = use_context::<NavState>();
    let mut selected = use_signal(|| None::<String>);
    let mut creating = use_signal(|| false);
    let mut toast = use_signal(|| None::<String>);
    use_effect(move || selected.set(nav.active_record_ref().map(|record| record.human_id)));
    // The top-bar `New` / new-record menu set `pending_create`; opening the draft here honours them.
    use_effect(move || {
        if *nav.pending_create.read() == Some(Category::Tags) {
            creating.set(true);
            nav.pending_create.set(None);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowTagList).await }
    });
    let list_pane = match &*list.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::List(rows))) => rsx! {
            ListPane {
                rows: rows.clone(),
                query,
                selected,
                chrome: list_chrome.clone(),
                onselect: move |row: RowVm| {
                    creating.set(false);
                    nav.open_record(RecordRef {
                        category: Category::Tags,
                        human_id: row.id,
                        label: row.title,
                    });
                },
            }
        },
        Some(ScreenData::Loaded(
            IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::PlaceDetail(_)
            | IntentOutcome::SourceDetail(_)
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::Pedigree(_)
            | IntentOutcome::Relationship(_)
            | IntentOutcome::DuplicateCandidates(_)
            | IntentOutcome::MergeCompare(_)
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_),
        )) => rsx! {},
    };
    let on_created = use_callback(move |(id, name): (String, String)| {
        creating.set(false);
        nav.open_record(RecordRef {
            category: Category::Tags,
            human_id: id.clone(),
            label: name,
        });
    });
    let detail = if creating() {
        rsx! {
            TagCreateRecord {
                oncreated: move |created| on_created.call(created),
                oncancel: move |()| creating.set(false),
                onerror: move |message| toast.set(Some(message)),
            }
        }
    } else {
        rsx! { RecordDetail {} }
    };
    rsx! {
        MasterDetail { list: list_pane, detail }
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: dismiss_label,
            onaction: move |_| toast.set(None),
        }
    }
}

/// The create-mode tag record: an uncommitted [`TagDraft`] rendered as the editable record in the
/// detail pane (Name focused). Save commits the whole tag; Cancel drops the draft.
#[component]
fn TagCreateRecord(
    oncreated: EventHandler<(String, String)>,
    oncancel: EventHandler<()>,
    onerror: EventHandler<String>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let title = loc.tag_new_title();
    let on_save = use_callback(move |(draft, prov): (TagDraft, ProvenanceDraft)| {
        let Some(request) = draft.to_request() else {
            return;
        };
        let services = services.clone();
        let name = request.name.clone();
        spawn(async move {
            match commit_tag_change_set(services, request, prov).await {
                Ok(id) => oncreated.call((id, name)),
                Err(message) => onerror.call(message),
            }
        });
    });
    rsx! {
        div { class: "detail-head",
            div { class: "avatar-lg", style: "background:transparent",
                span { class: "dot", style: "width:28px;height:28px;border-radius:var(--r-pill);background:{DEFAULT_TAG_COLOR}" }
            }
            div { class: "detail-id",
                div { class: "detail-title", "{title}" }
            }
        }
        TagRecordEditor {
            seed: TagDraft::new(),
            autofocus_name: true,
            onsave: move |pair| on_save.call(pair),
            oncancel: move |()| oncancel.call(()),
        }
    }
}

/// The detail pane for the selected tag: an editable-record header, the overview/usage/history tabs.
#[component]
pub(crate) fn TagDetailPane(id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let mut nav = use_context::<NavState>();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowTag { id }).await }
    });

    // Once the detail loads, upgrade the tab label from the tag id placeholder to its name
    // (`tab_label` falls back to the id when the name is blank).
    let label_id = id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::TagDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        nav.set_record_label(
            Category::Tags,
            &label_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_id),
        );
    });

    let on_save = use_callback(move |(draft, prov): (TagDraft, ProvenanceDraft)| {
        let Some(request) = draft.to_request() else {
            return;
        };
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match commit_tag_change_set(services, request, prov).await {
                Ok(_) => {
                    reload += 1;
                    // A rename/recolour changes what every applied-tag chip and reference elsewhere
                    // shows; bump the shared data version so those re-resolve rather than cache stale.
                    nav.mark_changed();
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    let body = match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::TagDetail(detail))) => tag_detail(&state, detail, active, on_save),
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::PlaceDetail(_)
            | IntentOutcome::SourceDetail(_)
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::Pedigree(_)
            | IntentOutcome::Relationship(_)
            | IntentOutcome::DuplicateCandidates(_)
            | IntentOutcome::MergeCompare(_)
            | IntentOutcome::Dashboard(_),
        )) => rsx! {},
    };

    rsx! {
        {body}
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: dismiss_label,
            onaction: move |_| toast.set(None),
        }
    }
}

/// Renders a loaded tag's detail: the record header (colour dot + name + priority/count badges), the
/// tabs, and the active tab (an editable Overview record, the Usage links, or the History timeline).
fn tag_detail(
    state: &AppState,
    detail: &TagDetail,
    mut active: Signal<usize>,
    on_save: Callback<(TagDraft, ProvenanceDraft)>,
) -> Element {
    let loc = state.data_loc();
    let tabs = tag_tabs(detail, loc);
    let tab_items: Vec<TabItem> = tabs
        .iter()
        .map(|tab| TabItem {
            id: tab.id.to_owned(),
            label: tab.label.clone(),
            count: tab.count,
        })
        .collect();
    let active_id = tabs.get(active()).map_or("overview", |tab| tab.id);
    rsx! {
        {tag_record_header(loc, detail)}
        Tabs {
            tabs: tab_items,
            active: active(),
            onselect: move |index| active.set(index),
            {tag_tab_content(loc, detail, active_id, on_save)}
        }
    }
}

/// The tag detail-record header (mockup `tag.html:58-72`): a large colour dot, the name title, the
/// `priority N · applied to X objects` subtitle, and a colour + priority badge. No id badge, no emoji
/// — a tag has no `human_id` and its UUID is never shown (data-model §9).
pub fn tag_record_header(loc: &Localizer, detail: &TagDetail) -> Element {
    let priority = detail.priority.unwrap_or(genealogy_ui::DEFAULT_TAG_PRIORITY);
    let color = detail
        .color
        .clone()
        .unwrap_or_else(|| genealogy_ui::DEFAULT_TAG_COLOR.to_owned());
    let priority_badge = loc.tag_priority_badge(priority);
    rsx! {
        div { class: "detail-head",
            div { class: "avatar-lg", style: "background:transparent",
                span { class: "dot", style: "width:28px;height:28px;border-radius:var(--r-pill);background:{color}" }
            }
            div { class: "detail-id",
                div { class: "detail-title", "{detail.title}" }
                div { class: "detail-sub", "{loc.tag_header_subtitle(priority, detail.total)}" }
                div { class: "wrap", style: "margin-top:8px",
                    span { class: "badge",
                        span { class: "dot", style: "width:8px;height:8px;border-radius:var(--r-pill);background:{color}" }
                        " {color}"
                    }
                    span { class: "badge", "{priority_badge}" }
                }
            }
        }
    }
}

/// The content of one tag detail tab.
fn tag_tab_content(
    loc: &Localizer,
    detail: &TagDetail,
    tab_id: &str,
    on_save: Callback<(TagDraft, ProvenanceDraft)>,
) -> Element {
    match tab_id {
        "usage" => tag_usage_tab(loc, detail),
        "history" => tag_history_tab(loc, detail),
        _ => rsx! {
            div { class: "section-note", "{loc.tag_overview_note()}" }
            TagRecordEditor {
                seed: TagDraft::from_detail(detail),
                autofocus_name: false,
                onsave: move |pair| on_save.call(pair),
                oncancel: move |()| {},
            }
        },
    }
}

/// The directly-editable tag record (create + edit, one mechanism), matching `docs/phase5/tag.html`
/// (Tag card: Name, Priority; Colour card: swatch + hex + a live preview chip). Buffers a [`TagDraft`]
/// locally against the committed `seed`; per-field revert restores that field, Save commits the whole
/// record, and nothing persists until then. Save is disabled while any field is empty/invalid.
#[component]
fn TagRecordEditor(
    seed: TagDraft,
    autofocus_name: bool,
    onsave: EventHandler<(TagDraft, ProvenanceDraft)>,
    oncancel: EventHandler<()>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let committed = seed.clone();
    let mut draft = use_signal(|| seed.clone());
    let prov = use_signal(ProvenanceDraft::default);
    let mut name_touched = use_signal(|| false);
    let mut picker_open = use_signal(|| false);
    // Re-seed when the committed record changes underneath (e.g. after a save reload).
    use_effect(use_reactive!(|seed| {
        draft.set(seed.clone());
        name_touched.set(false);
    }));

    let current = draft();
    let dirty = current != committed;
    let name_empty = current.name.trim().is_empty();
    let priority_invalid = current.parsed_priority().is_none();
    let color_empty = current.color.trim().is_empty();
    let can_save = current.is_valid();

    let committed_for_name = committed.clone();
    let committed_for_priority = committed.clone();
    let committed_for_color = committed.clone();
    let color = current.color.clone();
    let preview_name = if name_empty {
        loc.display_name(None)
    } else {
        current.name.clone()
    };
    rsx! {
        div { class: "grid-2",
            Card { title: loc.section_label("tag"),
                div { class: "stack",
                    div { class: "field",
                        label { r#for: "tag-name", "{loc.field_label(\"name\")}" }
                        div { class: "field-with-revert",
                            input {
                                class: if name_touched() && name_empty { "in invalid" } else { "in" },
                                r#type: "text",
                                id: "tag-name",
                                name: "tag-name",
                                autofocus: autofocus_name,
                                value: "{current.name}",
                                aria_invalid: if name_touched() && name_empty { "true" } else { "false" },
                                oninput: move |event| draft.write().name = event.value(),
                                onblur: move |_| name_touched.set(true),
                            }
                            if current.name != committed_for_name.name {
                                IconButton {
                                    icon: "↺".to_owned(),
                                    label: loc.action_revert(),
                                    title: loc.action_revert(),
                                    onclick: move |_| draft.write().name.clone_from(&committed_for_name.name),
                                }
                            }
                        }
                        if name_touched() && name_empty {
                            div { class: "field-error", "{loc.tag_name_required()}" }
                        }
                    }
                    div { class: "field",
                        label { r#for: "tag-priority", "{loc.field_label(\"priority\")}" }
                        div { class: if priority_invalid { "number-stepper invalid" } else { "number-stepper" },
                            input {
                                class: "stepper-value",
                                r#type: "text",
                                inputmode: "numeric",
                                id: "tag-priority",
                                name: "tag-priority",
                                value: "{current.priority}",
                                aria_invalid: if priority_invalid { "true" } else { "false" },
                                oninput: move |event| draft.write().priority = event.value(),
                                onkeydown: move |event| keep_typing_local(&event),
                            }
                            if current.priority != committed_for_priority.priority {
                                IconButton {
                                    icon: "↺".to_owned(),
                                    label: loc.action_revert(),
                                    title: loc.action_revert(),
                                    onclick: move |_| draft.write().priority.clone_from(&committed_for_priority.priority),
                                }
                            }
                            div { class: "stepper-arrows",
                                button {
                                    r#type: "button",
                                    class: "stepper-arrow",
                                    aria_label: loc.action_step_up(),
                                    title: loc.action_step_up(),
                                    onclick: move |_| {
                                        let mut draft = draft.write();
                                        let current = draft.priority.trim().parse::<i32>().unwrap_or(DEFAULT_TAG_PRIORITY);
                                        draft.priority = current.saturating_add(1).to_string();
                                    },
                                    "▲"
                                }
                                button {
                                    r#type: "button",
                                    class: "stepper-arrow",
                                    aria_label: loc.action_step_down(),
                                    title: loc.action_step_down(),
                                    onclick: move |_| {
                                        let mut draft = draft.write();
                                        let current = draft.priority.trim().parse::<i32>().unwrap_or(DEFAULT_TAG_PRIORITY);
                                        draft.priority = current.saturating_sub(1).max(1).to_string();
                                    },
                                    "▼"
                                }
                            }
                        }
                    }
                }
            }
            Card { title: loc.section_label("color"),
                div { class: "field",
                    label { "{loc.field_label(\"swatch\")}" }
                    div { class: "fact-row",
                        button {
                            r#type: "button",
                            class: "swatch-btn",
                            aria_label: loc.color_picker_title(),
                            title: loc.color_picker_title(),
                            onclick: move |_| picker_open.set(true),
                            span {
                                class: "dot swatch-dot",
                                style: "width:36px;height:36px;border-radius:var(--r-md);background:{color};flex:none",
                            }
                        }
                        div { class: "field-with-revert", style: "max-width:160px",
                            input {
                                class: if color_empty { "in invalid" } else { "in" },
                                r#type: "text",
                                id: "tag-color",
                                name: "tag-color",
                                style: "font-family:var(--font-mono)",
                                value: "{current.color}",
                                aria_invalid: if color_empty { "true" } else { "false" },
                                oninput: move |event| draft.write().color = event.value(),
                            }
                            if current.color != committed_for_color.color {
                                IconButton {
                                    icon: "↺".to_owned(),
                                    label: loc.action_revert(),
                                    title: loc.action_revert(),
                                    onclick: move |_| draft.write().color.clone_from(&committed_for_color.color),
                                }
                            }
                        }
                    }
                }
                p { class: "muted", style: "margin-bottom:0",
                    "{loc.tag_preview_label()}: "
                    Chip { label: preview_name, dot_color: Some(current.color.clone()) }
                }
            }
        }
        if dirty {
            {provenance_block(loc, prov)}
            div { class: "record-actions",
                Button {
                    label: loc.action_label("cancel"),
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| {
                        draft.set(committed.clone());
                        name_touched.set(false);
                        oncancel.call(());
                    },
                }
                Button {
                    label: loc.action_label("save"),
                    variant: ButtonVariant::Primary,
                    disabled: !can_save,
                    onclick: move |_| {
                        if can_save {
                            onsave.call((draft(), prov()));
                        }
                    },
                }
            }
        }
        ColorPicker {
            open: picker_open(),
            value: current.color.clone(),
            title: loc.color_picker_title(),
            presets_label: loc.color_picker_presets(),
            hex_label: loc.color_picker_hex(),
            confirm_label: loc.action_label("save"),
            cancel_label: loc.action_label("cancel"),
            onselect: move |hex: String| {
                draft.write().color = hex;
                picker_open.set(false);
            },
            oncancel: move |()| picker_open.set(false),
        }
    }
}

/// The tag Usage tab: a row per object type with its count and up to three example records rendered
/// as clickable links, an ellipsis when more exist.
pub fn tag_usage_tab(loc: &Localizer, detail: &TagDetail) -> Element {
    if detail.usage.is_empty() {
        return rsx! {
            div { class: "section-note", "{loc.tag_usage_note()}" }
            EmptyState { message: loc.tab_empty() }
        };
    }
    rsx! {
        div { class: "section-note", "{loc.tag_usage_note()}" }
        Table {
            headers: vec![
                loc.field_label("object-type"),
                loc.field_label("count"),
                loc.field_label("examples"),
            ],
            for group in detail.usage.iter() {
                {tag_usage_row(group)}
            }
        }
    }
}

/// One Usage-tab row: the object-type chip, the count, and up to three example records as links (an
/// ellipsis follows when the group holds more than the shown examples).
fn tag_usage_row(group: &TagUsageGroupVm) -> Element {
    let more = group.count > group.examples.len();
    rsx! {
        tr {
            td { Chip { label: group.kind_label.clone() } }
            td { b { "{group.count}" } }
            td { class: "muted",
                for (index, record) in group.examples.iter().enumerate() {
                    {
                        let category = Category::from_using_kind(record.kind);
                        let sep = if index > 0 { ", " } else { "" };
                        rsx! {
                            "{sep}"
                            RecordLink { category, human_id: record.human_id.clone(), label: record.label.clone() }
                        }
                    }
                }
                if more {
                    "…"
                }
            }
        }
    }
}

/// The tag History tab: the audit timeline. Tags have no retraction command, so entries are not
/// undoable (no undo control).
fn tag_history_tab(loc: &Localizer, detail: &TagDetail) -> Element {
    if detail.history.is_empty() {
        return rsx! { EmptyState { symbol: "🕓".to_owned(), message: loc.history_empty() } };
    }
    let undo_text = loc.history_undo_short();
    let entries: Vec<HistoryEntry> = detail
        .history
        .iter()
        .map(|entry| HistoryEntry {
            when: entry.when.clone(),
            what: entry.what.clone(),
            who: entry.who.clone(),
            why: entry.why.clone(),
            assertion_id: entry.assertion_id.clone(),
            can_undo: entry.can_undo,
            undo_text: undo_text.clone(),
            undo_label: loc.history_undo_label(&entry.what),
        })
        .collect();
    rsx! {
        div { class: "section-note", "{loc.history_note()}" }
        HistoryTimeline { entries, onundo: move |_assertion_id: String| {} }
    }
}
