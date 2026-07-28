use genealogy_ui::{DEFAULT_TAG_COLOR, DEFAULT_TAG_PRIORITY};

use super::prelude::*;
use crate::components::{ColorPicker, IconButton};

/// The create-mode tag record: an uncommitted [`TagDraft`] rendered as the editable record in the
/// detail pane (Name focused). Save commits the whole tag; Cancel drops the draft.
#[component]
pub fn TagCreateRecord() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let title = loc.tag_new_title();
    let draft_badge = loc.record_draft_badge();
    let save_label = loc.action_label("save");
    let cancel_label = loc.action_label("cancel");
    let edit = use_record_create::<TagDraft>();
    let name_touched = use_signal(|| false);
    let picker_open = use_signal(|| false);
    let on_save = use_callback(move |(draft, prov): (TagDraft, ProvenanceDraft)| {
        let Some(request) = draft.to_request() else {
            return;
        };
        let services = services.clone();
        let name = request.name.clone();
        spawn(async move {
            match commit_tag_change_set(services, request, prov).await {
                Ok(id) => nav.commit_draft(RecordRef {
                    category: Category::Tags,
                    human_id: id.clone(),
                    label: if name.is_empty() { id } else { name },
                }),
                Err(message) => nav.notify(message),
            }
        });
    });
    let can_save = edit.can_save();
    let actions = rsx! {
        Button {
            label: cancel_label,
            variant: ButtonVariant::Ghost,
            small: true,
            onclick: move |_| nav.cancel_draft(Category::Tags),
        }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            small: true,
            disabled: !can_save,
            onclick: move |_| {
                if edit.can_save() {
                    on_save.call((edit.draft.read().clone(), edit.prov.read().clone()));
                }
            },
        }
    };
    create_record_frame(
        &title,
        &draft_badge,
        actions,
        rsx! {
            {tag_record_fields(loc, edit, name_touched, picker_open, true)}
        },
    )
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
    let name_touched = use_signal(|| false);
    let picker_open = use_signal(|| false);
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

    // The shared whole-record edit state, seeded from the loaded tag (empty until it loads); it
    // reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::TagDetail(detail))) => TagDraft::from_detail(detail),
        _ => TagDraft::new(),
    };
    let edit = use_record_edit::<TagDraft>(&seed);

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

    // Tags have no retract path (Untag is the only removal — data-model §9), so ⌘Z always reports
    // nothing to undo rather than acting (WP5).
    let undo_history: Memo<Vec<genealogy_ui::HistoryEntryVm>> = use_memo(Vec::new);
    let undo_busy = use_memo(move || *edit.editing.read());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let on_undo = use_callback(|_assertion_id: String| {});
    use_record_undo(nav, undo_busy, undo_history, undo_notice, on_undo);

    let body = match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::TagDetail(detail))) => {
            tag_detail(&state, detail, active, edit, name_touched, picker_open, on_save)
        }
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
            | IntentOutcome::Geography(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::ResearchNoteDetail(_)
            | IntentOutcome::DataQuality(_),
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

/// Renders a loaded tag's detail: the record header (a colour-dot avatar + name + colour/priority
/// badges, never the UUID — data-model §9), with Edit / Cancel / Save in the sticky head-actions, the
/// tabs, and the active tab (the read-first Overview record, the Usage links, or the History timeline).
fn tag_detail(
    state: &AppState,
    detail: &TagDetail,
    active: Signal<usize>,
    edit: RecordEditState<TagDraft>,
    name_touched: Signal<bool>,
    picker_open: Signal<bool>,
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
    let priority = detail.priority.unwrap_or(DEFAULT_TAG_PRIORITY);
    let color = detail.color.clone().unwrap_or_else(|| DEFAULT_TAG_COLOR.to_owned());
    let labels = RecordActionLabels::resolve(loc);
    rsx! {
        DetailContainer {
            title: detail.title.clone(),
            subtitle: loc.tag_header_subtitle(priority, detail.total),
            avatar_color: color.clone(),
            badges: vec![color, loc.tag_priority_badge(priority)],
            extras: rsx! {},
            actions: record_head_actions(&labels, edit, rsx! {}, on_save),
            tabs: tab_items,
            active,
            {tag_tab_content(loc, detail, active_id, edit, name_touched, picker_open)}
        }
    }
}

/// The content of one tag detail tab.
fn tag_tab_content(
    loc: &Localizer,
    detail: &TagDetail,
    tab_id: &str,
    edit: RecordEditState<TagDraft>,
    name_touched: Signal<bool>,
    picker_open: Signal<bool>,
) -> Element {
    match tab_id {
        "usage" => tag_usage_tab(loc, detail),
        "history" => history_panel(loc, &detail.history, None),
        _ => tag_overview(loc, detail, edit, name_touched, picker_open),
    }
}

/// The tag Overview tab, read-first (`record-editing.html` §1/§2): read-only rows by default (Edit is
/// in the sticky header); edit mode swaps in the editable record cards and, while dirty, the
/// provenance block. `edit` is owned by the detail pane so the mode survives tab switches.
pub fn tag_overview(
    loc: &Localizer,
    detail: &TagDetail,
    edit: RecordEditState<TagDraft>,
    name_touched: Signal<bool>,
    picker_open: Signal<bool>,
) -> Element {
    rsx! {
        div { class: "section-note", "{loc.tag_overview_note()}" }
        if edit.editing.read().to_owned() {
            {tag_record_fields(loc, edit, name_touched, picker_open, false)}
        } else {
            {tag_read_rows(loc, detail)}
        }
    }
}

/// The editable tag record body (create + edit): the Tag card (name + priority) and Colour card
/// (swatch + hex) bound to the draft, the colour picker, and — while dirty — the provenance block.
/// The whole-record Save/Cancel live in the sticky header, not here.
fn tag_record_fields(
    loc: &Localizer,
    edit: RecordEditState<TagDraft>,
    name_touched: Signal<bool>,
    mut picker_open: Signal<bool>,
    autofocus_name: bool,
) -> Element {
    let mut draft = edit.draft;
    let committed = edit.seed.read().clone();
    let current = draft();
    rsx! {
        div { class: "grid-2",
            {tag_edit_tag_card(loc, draft, &committed, name_touched, autofocus_name)}
            {tag_edit_colour_card(loc, draft, &committed, picker_open)}
        }
        {record_edit_provenance(loc, edit)}
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

/// The tag Overview read rows (view mode): Name · Priority · Colour as read text, matching the edit
/// record's layout so toggling to edit moves no text (`record-editing.html` §3).
fn tag_read_rows(loc: &Localizer, detail: &TagDetail) -> Element {
    let priority = detail
        .priority
        .map_or_else(String::new, |priority| priority.to_string());
    let color = detail.color.clone().unwrap_or_default();
    rsx! {
        div { class: "grid-2",
            Card { title: loc.section_label("tag"),
                div { class: "stack",
                    div { class: "field",
                        label { "{loc.field_label(\"name\")}" }
                        div { class: "val", {detail.name.clone().unwrap_or_default()} }
                    }
                    div { class: "field",
                        label { "{loc.field_label(\"priority\")}" }
                        div { class: "val", "{priority}" }
                    }
                }
            }
            Card { title: loc.section_label("color"),
                div { class: "field",
                    label { "{loc.field_label(\"swatch\")}" }
                    div { class: "val", style: "font-family:var(--font-mono)", "{color}" }
                }
            }
        }
    }
}

/// The tag record's Tag card (Name + Priority inputs, with per-field revert). A pure fn (signals
/// passed in) so both [`tag_record_fields`] and the SSR test render it without `AppCtx`.
pub fn tag_edit_tag_card(
    loc: &Localizer,
    mut draft: Signal<TagDraft>,
    committed: &TagDraft,
    mut name_touched: Signal<bool>,
    autofocus_name: bool,
) -> Element {
    let current = draft();
    let show_name_error = name_touched() && current.name_missing();
    let name_modified = current.name != committed.name;
    let priority_modified = current.priority != committed.priority;
    let priority_invalid = current.priority_invalid();
    let revert_name = committed.name.clone();
    let revert_priority = committed.priority.clone();
    rsx! {
        Card { title: loc.section_label("tag"),
            div { class: "stack",
                TextField {
                    label: loc.field_label("name"),
                    name: "tag-name".to_owned(),
                    value: current.name.clone(),
                    autofocus: autofocus_name,
                    invalid: show_name_error,
                    error: if show_name_error { Some(loc.tag_name_required()) } else { None },
                    modified: name_modified,
                    reset_label: loc.action_revert(),
                    oninput: move |event: FormEvent| draft.write().name = event.value(),
                    onblur: move |_| name_touched.set(true),
                    onreset: move |()| draft.write().name.clone_from(&revert_name),
                }
                TextField {
                    label: loc.field_label("priority"),
                    name: "tag-priority".to_owned(),
                    value: current.priority.clone(),
                    invalid: priority_invalid,
                    inputmode: "numeric",
                    container_class: "number-stepper",
                    input_class: "stepper-value",
                    modified: priority_modified,
                    reset_label: loc.action_revert(),
                    oninput: move |event: FormEvent| draft.write().priority = event.value(),
                    onreset: move |()| draft.write().priority.clone_from(&revert_priority),
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
}

/// The tag record's Colour card (swatch button, hex input with revert, live preview chip). A pure fn
/// so both [`TagRecordEditor`] and the SSR test render it without `AppCtx`.
pub fn tag_edit_colour_card(
    loc: &Localizer,
    mut draft: Signal<TagDraft>,
    committed: &TagDraft,
    mut picker_open: Signal<bool>,
) -> Element {
    let current = draft();
    let color_empty = current.color_missing();
    let name_empty = current.name_missing();
    let committed_color = committed.color.clone();
    let revert_color = committed.color.clone();
    let color = current.color.clone();
    let preview_name = if name_empty {
        loc.display_name(None)
    } else {
        current.name.clone()
    };
    rsx! {
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
                        TextInput {
                            id: "tag-color",
                            name: "tag-color",
                            style: "font-family:var(--font-mono)",
                            value: "{current.color}",
                            invalid: color_empty,
                            oninput: move |event: FormEvent| draft.write().color = event.value(),
                        }
                        if current.color != committed_color {
                            IconButton {
                                icon: "↺".to_owned(),
                                label: loc.action_revert(),
                                title: loc.action_revert(),
                                onclick: move |_| draft.write().color.clone_from(&revert_color),
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
            caption: loc.tab_label("usage"),
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
