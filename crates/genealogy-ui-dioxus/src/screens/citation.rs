use super::prelude::*;
use crate::screens::RecordDetail;

/// The citation master-detail screen (ADR 0008 §5): a searchable list of citations on the left and
/// the selected citation's detail (overview + related-item tabs) on the right. Parallel to
/// [`PersonScreen`]; the research-grade Evidence Explained axes live on the overview.
#[component]
pub fn CitationScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let create_services = services.clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Citations.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().citation_list_empty();
    let create_title = chrome.list_new();
    let cancel_label = state.data_loc().action_label("cancel");
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
    use_effect(move || {
        if *nav.pending_create.read() == Some(Category::Citations) {
            creating.set(true);
            nav.pending_create.set(None);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowCitationList).await }
    });
    let on_create = use_callback(move |(source, page): (String, Option<String>)| {
        let services = create_services.clone();
        spawn(async move {
            match create_citation_record(services, source, page).await {
                Ok(human_id) => {
                    creating.set(false);
                    nav.open_record(RecordRef {
                        category: Category::Citations,
                        label: human_id.clone(),
                        human_id,
                    });
                }
                Err(message) => toast.set(Some(message)),
            }
        });
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
                onselect: move |row: RowVm| nav.open_record(RecordRef {
                    category: Category::Citations,
                    human_id: row.id,
                    label: row.title,
                }),
            }
        },
        Some(ScreenData::Loaded(
            IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::PlaceDetail(_)
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_)
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
            | IntentOutcome::MergeCompare(_),
        )) => rsx! {},
    };
    rsx! {
        MasterDetail { list: list_pane, detail: rsx! { RecordDetail {} } }
        if creating() {
            SidePanel {
                title: create_title,
                open: true,
                close_label: cancel_label,
                onclose: move |_| creating.set(false),
                footer: rsx! {},
                CreateCitationForm { onsubmit: move |payload| on_create.call(payload) }
            }
        }
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: dismiss_label,
            onaction: move |_| toast.set(None),
        }
    }
}

/// The "New citation" form: a cited source `human_id` (required) plus an optional page.
#[component]
fn CreateCitationForm(onsubmit: EventHandler<(String, Option<String>)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut source = use_signal(String::new);
    let mut page = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("source"), name: "source".to_owned(), oninput: move |event: FormEvent| source.set(event.value()) }
        Input { label: loc.field_label("page"), name: "page".to_owned(), oninput: move |event: FormEvent| page.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let source = source();
                if source.trim().is_empty() {
                    return;
                }
                onsubmit.call((source, non_empty(page())));
            },
        }
    }
}

/// Which citation edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitationEditForm {
    /// Set the page / locator.
    Page,
    /// Assert the cited record's date.
    Date,
    /// Set the operator's confidence.
    Confidence,
    /// Set the Evidence Explained analysis.
    Evidence,
    /// Add a typed attribute.
    Attribute,
    /// Attach a media object by `human_id`.
    Media,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected citation: header, related-item tabs, editing side panel, toast.
#[component]
pub(crate) fn CitationDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let mut nav = use_context::<NavState>();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<CitationEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowCitation { human_id }).await }
    });

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the cited
    // source (`tab_label` falls back to `human_id` when unsourced, mirroring the detail-head title).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::CitationDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        nav.set_record_label(
            Category::Citations,
            &label_human_id,
            genealogy_ui::tab_label(detail.source.as_deref(), &label_human_id),
        );
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (CitationEdit, ProvenanceDraft)| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_citation_edit(services, edit, prov).await {
                Ok(()) => {
                    editing_for_submit.set(None);
                    reload += 1;
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
        Some(ScreenData::Loaded(IntentOutcome::CitationDetail(detail))) => {
            citation_detail(&state, detail, active, editing, on_submit, &human_id)
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Detail(_)
            | IntentOutcome::Dashboard(_)
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
            | IntentOutcome::MergeCompare(_),
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

/// Renders a loaded citation's detail container: header (source, page, restriction toggles), the tab
/// strip, the active tab's content, and the editing side panel.
fn citation_detail(
    state: &AppState,
    detail: &CitationDetail,
    active: Signal<usize>,
    editing: Signal<Option<CitationEditForm>>,
    on_submit: Callback<(CitationEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let tabs = citation_tabs(detail, loc);
    let tab_items: Vec<TabItem> = tabs
        .iter()
        .map(|tab| TabItem {
            id: tab.id.to_owned(),
            label: tab.label.clone(),
            count: tab.count,
        })
        .collect();
    let active_id = tabs.get(active()).map_or("overview", |tab| tab.id);
    let subtitle = detail.page.clone();
    rsx! {
        DetailContainer {
            title: detail.source.clone().unwrap_or_else(|| detail.human_id.clone()),
            subtitle,
            id_label: detail.human_id.clone(),
            avatar: "❝".to_owned(),
            extras: citation_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {citation_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {citation_edit_panel(state, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for a citation (the mockup `resn-set`).
fn citation_restriction_toggles(
    loc: &Localizer,
    detail: &CitationDetail,
    on_submit: Callback<(CitationEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let selected: Vec<RestrictionKind> = detail.restrictions.clone();
    let choices: Vec<RestrictionChoice> = RestrictionKind::all()
        .into_iter()
        .map(|kind| RestrictionChoice {
            kind,
            label: loc.restriction_label(kind),
        })
        .collect();
    let human_id = human_id.to_owned();
    rsx! {
        RestrictionSet {
            choices,
            selected: selected.clone(),
            ontoggle: move |kind: RestrictionKind| {
                let mut next = selected.clone();
                if let Some(position) = next.iter().position(|&k| k == kind) {
                    next.remove(position);
                } else {
                    next.push(kind);
                }
                on_submit.call((CitationEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one citation detail tab, with its contextual add/edit affordances.
fn citation_tab_content(
    state: &AppState,
    detail: &CitationDetail,
    tab_id: &str,
    mut editing: Signal<Option<CitationEditForm>>,
    on_submit: Callback<(CitationEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "attributes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-attribute"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(CitationEditForm::Attribute)) }
            }
            {citation_attributes_table(loc, &detail.attributes)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(CitationEditForm::Media)) }
            }
            {media_gallery(loc, &detail.media)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(CitationEditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => citation_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => citation_history_tab(loc, detail, on_submit, human_id),
        _ => citation_overview(loc, detail, editing),
    }
}

/// The Overview tab: the evidence-first note, the source/page/date/confidence card with its edit
/// affordances, and the Evidence Explained axis chips (or a no-source flag when unsourced).
pub fn citation_overview(
    loc: &Localizer,
    detail: &CitationDetail,
    mut editing: Signal<Option<CitationEditForm>>,
) -> Element {
    rsx! {
        div { class: "section-note", "{loc.overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.field_label("source"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"source\")}" }
                        span { class: "grow",
                            if let Some(source) = detail.source.as_deref() {
                                "{source}"
                            } else {
                                NoSourceFlag { label: loc.no_source() }
                            }
                        }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"page\")}" }
                        span { class: "grow", {detail.page.clone().unwrap_or_else(|| "—".to_owned())} }
                        Button { label: loc.action_label("set-page"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(CitationEditForm::Page)) }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"date\")}" }
                        span { class: "grow", {detail.date.clone().unwrap_or_else(|| "—".to_owned())} }
                        Button { label: loc.action_label("set-date"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(CitationEditForm::Date)) }
                    }
                }
            }
            Card { title: loc.field_label("evidence"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"surety\")}" }
                        span { class: "grow",
                            if let (Some(level), Some(label)) = (detail.confidence, detail.confidence_label.clone()) {
                                ConfidenceBadge { level, label }
                            } else {
                                "—"
                            }
                        }
                        Button { label: loc.action_label("set-confidence"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(CitationEditForm::Confidence)) }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"evidence\")}" }
                        span { class: "grow wrap",
                            if detail.evidence_axes.is_empty() {
                                "—"
                            } else {
                                for chip in detail.evidence_axes.iter() {
                                    EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() }
                                }
                            }
                        }
                        Button { label: loc.action_label("set-evidence"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(CitationEditForm::Evidence)) }
                    }
                }
            }
        }
    }
}

/// The Attributes tab: each recorded `(type, value)` attribute as a table row.
pub fn citation_attributes_table(loc: &Localizer, attributes: &[(String, String)]) -> Element {
    if attributes.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table { headers: vec![loc.field_label("attribute-type"), loc.field_label("value")],
            for (attribute_type , value) in attributes.iter() {
                tr {
                    td { "{attribute_type}" }
                    td { class: "muted", "{value}" }
                }
            }
        }
    }
}

/// The Tags tab: each applied tag as a colour-dot chip (name + colour, never the id) with a remove
/// control, plus an "Add tag" affordance.
pub fn citation_tags_panel(
    loc: &Localizer,
    detail: &CitationDetail,
    mut editing: Signal<Option<CitationEditForm>>,
    on_submit: Callback<(CitationEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(CitationEditForm::Tag)) }
        }
        if detail.tags.is_empty() {
            EmptyState { message: loc.tab_empty() }
        } else {
            div { class: "wrap",
                for tag in detail.tags.iter() {
                    {
                        let tag_id = tag.id.clone();
                        let human_id = human_id.clone();
                        let remove_label = loc.action_label("remove-tag");
                        rsx! {
                            span { class: "fact-row",
                                Chip { label: tag.name.clone(), dot_color: tag.color.clone() }
                                Button {
                                    label: remove_label,
                                    variant: ButtonVariant::Ghost,
                                    small: true,
                                    onclick: move |_| on_submit.call((CitationEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The History tab: the per-record audit timeline, each undoable entry carrying an undo control.
fn citation_history_tab(
    loc: &Localizer,
    detail: &CitationDetail,
    on_submit: Callback<(CitationEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
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
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "section-note", "{loc.history_note()}" }
        HistoryTimeline {
            entries,
            onundo: move |assertion_id: String| {
                on_submit.call((CitationEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The citation editing side panel: renders the form for the open [`CitationEditForm`], or nothing.
fn citation_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<CitationEditForm>>,
    on_submit: Callback<(CitationEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        CitationEditForm::Page => loc.action_label("set-page"),
        CitationEditForm::Date => loc.action_label("set-date"),
        CitationEditForm::Confidence => loc.action_label("set-confidence"),
        CitationEditForm::Evidence => loc.action_label("set-evidence"),
        CitationEditForm::Attribute => loc.action_label("add-attribute"),
        CitationEditForm::Media => loc.action_label("attach-media"),
        CitationEditForm::Note => loc.action_label("attach-note"),
        CitationEditForm::Tag => loc.action_label("add-tag"),
    };
    let human_id = human_id.to_owned();
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |_| editing.set(None),
            footer: rsx! {},
            {match form {
                CitationEditForm::Page => rsx! { CitationPageForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Date => rsx! { CitationDateForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Confidence => rsx! { CitationConfidenceForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Evidence => rsx! { CitationEvidenceForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Attribute => rsx! { CitationAttributeForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Media => rsx! { CitationAttachForm { human_id, is_note: false, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Note => rsx! { CitationAttachForm { human_id, is_note: true, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Tag => rsx! { CitationTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Set page" form → [`CitationEdit::SetPage`].
#[component]
fn CitationPageForm(human_id: String, onsubmit: EventHandler<(CitationEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut page = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("page"), name: "page".to_owned(), oninput: move |event: FormEvent| page.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| onsubmit.call((CitationEdit::SetPage { human_id: human_id.clone(), page: page() }, prov())),
        }
    }
}

/// The "Set date" form (year required; month/day optional) → [`CitationEdit::SetDate`].
#[component]
fn CitationDateForm(human_id: String, onsubmit: EventHandler<(CitationEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut year = use_signal(String::new);
    let mut month = use_signal(String::new);
    let mut day = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("date"), name: "year".to_owned(), oninput: move |event: FormEvent| year.set(event.value()) }
        Input { label: loc.field_label("attribute-type"), name: "month".to_owned(), oninput: move |event: FormEvent| month.set(event.value()) }
        Input { label: loc.field_label("value"), name: "day".to_owned(), oninput: move |event: FormEvent| day.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let Ok(year) = year().trim().parse::<i32>() else {
                    return;
                };
                let parts = DateParts {
                    year,
                    month: month().trim().parse::<u8>().ok(),
                    day: day().trim().parse::<u8>().ok(),
                };
                onsubmit.call((CitationEdit::SetDate { human_id: human_id.clone(), parts }, prov()));
            },
        }
    }
}

/// The "Set confidence" form → [`CitationEdit::SetConfidence`].
#[component]
fn CitationConfidenceForm(human_id: String, onsubmit: EventHandler<(CitationEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let levels = ConfidenceLevel::all();
    let mut index = use_signal(|| 2_usize);
    let options: Vec<SelectChoice> = levels
        .iter()
        .enumerate()
        .map(|(position, level)| SelectChoice {
            value: position.to_string(),
            label: loc.confidence_label(*level),
        })
        .collect();
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Select {
            label: loc.field_label("confidence"),
            name: "confidence".to_owned(),
            value: Some(2.to_string()),
            options,
            onchange: move |event: FormEvent| index.set(event.value().parse().unwrap_or(2)),
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let confidence = *levels.get(index()).unwrap_or(&ConfidenceLevel::Normal);
                onsubmit.call((CitationEdit::SetConfidence { human_id: human_id.clone(), confidence }, prov()));
            },
        }
    }
}

/// The "Set evidence analysis" form: the three Evidence Explained axes → [`CitationEdit::SetEvidenceAnalysis`].
#[component]
fn CitationEvidenceForm(human_id: String, onsubmit: EventHandler<(CitationEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let sources = [SourceQuality::Original, SourceQuality::Derivative];
    let informations = [InformationKind::Primary, InformationKind::Secondary];
    let evidences = [EvidenceKind::Direct, EvidenceKind::Indirect, EvidenceKind::Negative];
    let mut source_index = use_signal(|| 0_usize);
    let mut information_index = use_signal(|| 0_usize);
    let mut evidence_index = use_signal(|| 0_usize);
    let source_options: Vec<SelectChoice> = sources
        .iter()
        .enumerate()
        .map(|(position, quality)| SelectChoice {
            value: position.to_string(),
            label: loc.evidence_source_label(*quality),
        })
        .collect();
    let information_options: Vec<SelectChoice> = informations
        .iter()
        .enumerate()
        .map(|(position, kind)| SelectChoice {
            value: position.to_string(),
            label: loc.evidence_information_label(*kind),
        })
        .collect();
    let evidence_options: Vec<SelectChoice> = evidences
        .iter()
        .enumerate()
        .map(|(position, kind)| SelectChoice {
            value: position.to_string(),
            label: loc.evidence_kind_label(*kind),
        })
        .collect();
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Select { label: loc.field_label("source"), name: "source".to_owned(), value: Some(0.to_string()), options: source_options, onchange: move |event: FormEvent| source_index.set(event.value().parse().unwrap_or(0)) }
        Select { label: loc.field_label("evidence"), name: "information".to_owned(), value: Some(0.to_string()), options: information_options, onchange: move |event: FormEvent| information_index.set(event.value().parse().unwrap_or(0)) }
        Select { label: loc.field_label("evidence"), name: "evidence".to_owned(), value: Some(0.to_string()), options: evidence_options, onchange: move |event: FormEvent| evidence_index.set(event.value().parse().unwrap_or(0)) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let analysis = EvidenceAnalysis {
                    source: *sources.get(source_index()).unwrap_or(&SourceQuality::Original),
                    information: *informations.get(information_index()).unwrap_or(&InformationKind::Primary),
                    evidence: *evidences.get(evidence_index()).unwrap_or(&EvidenceKind::Direct),
                };
                onsubmit.call((CitationEdit::SetEvidenceAnalysis { human_id: human_id.clone(), analysis }, prov()));
            },
        }
    }
}

/// The "Add attribute" form → [`CitationEdit::AddAttribute`].
#[component]
fn CitationAttributeForm(human_id: String, onsubmit: EventHandler<(CitationEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut attribute_type = use_signal(String::new);
    let mut value = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("attribute-type"), name: "attribute-type".to_owned(), oninput: move |event: FormEvent| attribute_type.set(event.value()) }
        Input { label: loc.field_label("value"), name: "value".to_owned(), oninput: move |event: FormEvent| value.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let attribute_type = attribute_type();
                if attribute_type.trim().is_empty() {
                    return;
                }
                onsubmit.call((CitationEdit::AddAttribute { human_id: human_id.clone(), attribute_type, value: value() }, prov()));
            },
        }
    }
}

/// The "Attach media/note by id" form → [`CitationEdit::AttachMedia`]/[`CitationEdit::AttachNote`].
#[component]
fn CitationAttachForm(
    human_id: String,
    is_note: bool,
    onsubmit: EventHandler<(CitationEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let field = if is_note { "note" } else { "media" };
    let mut id = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label(field), name: field.to_owned(), oninput: move |event: FormEvent| id.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let id = id();
                if id.trim().is_empty() {
                    return;
                }
                let edit = if is_note {
                    CitationEdit::AttachNote { human_id: human_id.clone(), note_id: id }
                } else {
                    CitationEdit::AttachMedia { human_id: human_id.clone(), media_id: id }
                };
                onsubmit.call((edit, prov()));
            },
        }
    }
}

/// The "Add tag" form: a picker of existing tags by name (the tag id is the option value, never
/// shown) → [`CitationEdit::Tag`].
#[component]
fn CitationTagForm(human_id: String, onsubmit: EventHandler<(CitationEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loc = state.data_loc();
    let save_label = loc.action_label("save");
    let field_label = loc.field_label("tag");
    let tags = use_resource(move || {
        let services = services.clone();
        async move { load_tags(services).await }
    });
    let mut chosen = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    match &*tags.read_unchecked() {
        None => rsx! { p { class: "loading", "{loc.tab_empty()}" } },
        Some(Err(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(Ok(list)) => {
            let options: Vec<SelectChoice> = list
                .iter()
                .filter_map(|tag| {
                    tag.name.clone().map(|name| SelectChoice {
                        value: tag.id.clone(),
                        label: name,
                    })
                })
                .collect();
            let first = options.first().map(|choice| choice.value.clone()).unwrap_or_default();
            if chosen().is_empty() {
                chosen.set(first.clone());
            }
            rsx! {
                Select {
                    label: field_label,
                    name: "tag".to_owned(),
                    value: Some(first),
                    options,
                    onchange: move |event: FormEvent| chosen.set(event.value()),
                }
                {provenance_block(loc, prov)}
                Button {
                    label: save_label,
                    variant: ButtonVariant::Primary,
                    onclick: move |_| {
                        let tag_id = chosen();
                        if tag_id.is_empty() {
                            return;
                        }
                        onsubmit.call((CitationEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

// ─── Family slice (PR7) ──────────────────────────────────────────────────────────────────────────
