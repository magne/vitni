use super::prelude::*;
use crate::screens::RecordDetail;

/// The selectable child-parent relationships offered when adding a child (the standard set; a custom
/// relationship is not entered from the UI).
fn relationship_choices() -> [ChildParentRelationship; 6] {
    [
        ChildParentRelationship::Birth,
        ChildParentRelationship::Adopted,
        ChildParentRelationship::Foster,
        ChildParentRelationship::Step,
        ChildParentRelationship::Sealed,
        ChildParentRelationship::Unknown,
    ]
}

/// The family master-detail: a searchable list on the left, the selected family's detail (overview,
/// children with per-partner relationships, events, media, notes, tags, history) on the right.
#[component]
pub fn FamilyScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Families.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().family_list_empty();
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
    // The top-bar `New` sets `pending_create`; open the draft here (nothing is created until Save).
    use_effect(move || {
        if *nav.pending_create.read() == Some(Category::Families) {
            creating.set(true);
            nav.pending_create.set(None);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowFamilyList).await }
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
                        category: Category::Families,
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
    let on_created = use_callback(move |id: String| {
        creating.set(false);
        nav.open_record(RecordRef {
            category: Category::Families,
            human_id: id.clone(),
            label: id,
        });
    });
    let detail = if creating() {
        rsx! {
            FamilyCreateRecord {
                oncreated: move |id| on_created.call(id),
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

/// The create-mode family record: an uncommitted [`FamilyDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). Partners are added by person id; Save commits the whole
/// family; Cancel discards.
#[component]
fn FamilyCreateRecord(
    oncreated: EventHandler<String>,
    oncancel: EventHandler<()>,
    onerror: EventHandler<String>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let draft = use_signal(genealogy_ui::FamilyDraft::new);
    let new_partner = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty();
    let on_save = use_callback(move |()| {
        let request = draft().to_request();
        let services = services.clone();
        let prov = prov();
        spawn(async move {
            match commit_family_change_set(services, request, prov).await {
                Ok(id) => oncreated.call(id),
                Err(message) => onerror.call(message),
            }
        });
    });
    rsx! {
        {create_record_header(&loc.family_new_title(), &loc.record_draft_badge())}
        {family_create_fields(loc, draft, new_partner)}
        {provenance_block(loc, prov)}
        RecordActions {
            save_label: loc.action_label("save"),
            cancel_label: loc.action_label("cancel"),
            can_save,
            onsave: move |()| on_save.call(()),
            oncancel: move |()| oncancel.call(()),
        }
    }
}

/// The family create form's field rows (`family.html`): the partner chips (removable) plus an
/// add-partner id input. A pure fn (no `AppCtx`) so SSR tests can render it directly; `new_partner`
/// holds the add input's text.
pub fn family_create_fields(
    loc: &Localizer,
    mut draft: Signal<genealogy_ui::FamilyDraft>,
    mut new_partner: Signal<String>,
) -> Element {
    let partners = draft().partners.clone();
    let at_capacity = partners.len() >= 2;
    rsx! {
        Card { title: loc.section_label("partners"),
            div { class: "wrap", style: "margin-bottom:8px",
                for partner in partners.iter() {
                    {
                        let id = partner.clone();
                        rsx! {
                            span { class: "chip",
                                "{partner}"
                                button {
                                    r#type: "button",
                                    class: "chip-x",
                                    aria_label: loc.action_label("dismiss"),
                                    onclick: move |_| draft.write().remove_partner(&id),
                                    "×"
                                }
                            }
                        }
                    }
                }
            }
            if !at_capacity {
                div { class: "fact-row",
                    Input {
                        label: loc.field_label("partner"),
                        name: "family-partner".to_owned(),
                        value: new_partner(),
                        oninput: move |event: FormEvent| new_partner.set(event.value()),
                    }
                    Button {
                        label: loc.action_label("add-partner"),
                        variant: ButtonVariant::Ghost,
                        onclick: move |_| {
                            let id = new_partner();
                            draft.write().add_partner(&id);
                            new_partner.set(String::new());
                        },
                    }
                }
            }
        }
    }
}

/// Which family edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamilyEditForm {
    /// Add a partner by `human_id`.
    Partner,
    /// Add a child with per-partner relationships.
    Child,
    /// Link an existing event by `human_id`.
    Event,
    /// Attach a media object by `human_id`.
    Media,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected family: header, related-item tabs, editing side panel, toast.
#[component]
pub(crate) fn FamilyDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let mut nav = use_context::<NavState>();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<FamilyEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowFamily { human_id }).await }
    });

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the family's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::FamilyDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        nav.set_record_label(
            Category::Families,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (FamilyEdit, ProvenanceDraft)| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_family_edit(services, edit, prov).await {
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
        Some(ScreenData::Loaded(IntentOutcome::FamilyDetail(detail))) => {
            family_detail(&state, detail, active, editing, on_submit, &human_id)
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::PlaceDetail(_)
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
        {body}
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: dismiss_label,
            onaction: move |_| toast.set(None),
        }
    }
}

/// Renders a loaded family's detail container: header (title, restriction toggles), the tab strip,
/// the active tab's content, and the editing side panel.
fn family_detail(
    state: &AppState,
    detail: &FamilyDetail,
    active: Signal<usize>,
    editing: Signal<Option<FamilyEditForm>>,
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let tabs = family_tabs(detail, loc);
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
        DetailContainer {
            title: detail.title.clone(),
            id_label: detail.human_id.clone(),
            avatar: "👪".to_owned(),
            extras: family_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {family_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {family_edit_panel(state, detail, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for a family (the mockup `resn-set`).
fn family_restriction_toggles(
    loc: &Localizer,
    detail: &FamilyDetail,
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
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
                on_submit.call((FamilyEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one family detail tab, with its contextual add/edit affordances.
fn family_tab_content(
    state: &AppState,
    detail: &FamilyDetail,
    tab_id: &str,
    mut editing: Signal<Option<FamilyEditForm>>,
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "children" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-child"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(FamilyEditForm::Child)) }
            }
            {family_children_table(loc, detail)}
        },
        "events" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("link-event"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(FamilyEditForm::Event)) }
            }
            {family_events_table(loc, &detail.events)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(FamilyEditForm::Media)) }
            }
            {family_media_gallery(loc, &detail.media)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(FamilyEditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => family_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => family_history_tab(loc, detail, on_submit, human_id),
        _ => family_overview(loc, detail, editing),
    }
}

/// The Overview tab: the neutral-roles note, the Partners card, the Marriage card, and a provenance
/// specimen for the marriage claim.
pub fn family_overview(loc: &Localizer, detail: &FamilyDetail, mut editing: Signal<Option<FamilyEditForm>>) -> Element {
    rsx! {
        div { class: "section-note", "{loc.family_overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.section_label("partners"),
                div { class: "tab-actions",
                    Button { label: loc.action_label("add-partner"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(FamilyEditForm::Partner)) }
                }
                if detail.partners.is_empty() {
                    EmptyState { message: loc.tab_empty() }
                } else {
                    div { class: "stack",
                        for partner in detail.partners.iter() {
                            div { class: "fact-row",
                                span { class: "grow", "{partner.name}" }
                                if let Some(vitals) = partner.vitals.clone() {
                                    span { class: "muted", "{vitals}" }
                                }
                                {provenance_cue(loc, loc.provenance_title_claim(&partner.name), &partner.citations)}
                            }
                        }
                    }
                }
            }
            Card { title: loc.section_label("marriage"),
                if let Some(marriage) = detail.marriage.as_ref() {
                    div { class: "stack",
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:64px;margin:0", "{loc.field_label(\"date\")}" }
                            span { class: "grow", {marriage.date.clone().unwrap_or_else(|| "—".to_owned())} }
                            ConfidenceBadge { level: marriage.confidence, label: marriage.confidence_label.clone() }
                            {provenance_cue(loc, loc.provenance_title_claim(&marriage.type_label), &marriage.citations)}
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:64px;margin:0", "{loc.field_label(\"place\")}" }
                            span { class: "grow", {marriage.place.clone().unwrap_or_else(|| "—".to_owned())} }
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:64px;margin:0", "{loc.field_label(\"attribute-type\")}" }
                            span { class: "grow", Chip { label: marriage.type_label.clone() } }
                        }
                    }
                } else {
                    EmptyState { message: loc.tab_empty() }
                }
            }
        }
    }
}

/// The Children tab: a row per child with a relationship column per family partner, plus surety and
/// source columns (the per-partner relationship model — GEDCOM `_FREL`/`_MREL`).
pub fn family_children_table(loc: &Localizer, detail: &FamilyDetail) -> Element {
    if detail.children.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    let mut headers = vec![loc.field_label("child"), loc.field_label("born")];
    for partner in &detail.partners {
        headers.push(partner.name.clone());
    }
    headers.push(loc.field_label("surety"));
    headers.push(loc.field_label("source"));
    let partner_ids: Vec<String> = detail.partners.iter().map(|partner| partner.human_id.clone()).collect();
    rsx! {
        Table { headers,
            for child in detail.children.iter() {
                tr {
                    td { "{child.name}" }
                    td { class: "muted", {child.born.clone().unwrap_or_else(|| "—".to_owned())} }
                    for partner_id in partner_ids.iter() {
                        td {
                            {
                                match child.relationships.iter().find(|(id, _)| id == partner_id) {
                                    Some((_, label)) => rsx! { Chip { label: label.clone() } },
                                    None => rsx! { span { class: "muted", "—" } },
                                }
                            }
                        }
                    }
                    td { ConfidenceBadge { level: child.confidence, label: child.confidence_label.clone() } }
                    td { {source_cue(loc, child.source_count)} }
                }
            }
        }
    }
}

/// The Events tab: a row per linked family event with its kind, date, place, surety, and source.
pub fn family_events_table(loc: &Localizer, events: &[FamilyEventVm]) -> Element {
    if events.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.tab_label("events"),
                loc.field_label("date"),
                loc.field_label("place"),
                loc.field_label("surety"),
                loc.field_label("source"),
            ],
            for event in events.iter() {
                tr {
                    td { "{event.type_label}" }
                    td { class: "muted", {event.date.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { {event.place.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { ConfidenceBadge { level: event.confidence, label: event.confidence_label.clone() } }
                    td { {source_cue(loc, event.source_count)} }
                }
            }
        }
    }
}

/// The Tags tab: each applied tag as a colour-dot chip (name + colour, never the id) with a remove
/// control, plus an "Add tag" affordance.
pub fn family_tags_panel(
    loc: &Localizer,
    detail: &FamilyDetail,
    mut editing: Signal<Option<FamilyEditForm>>,
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(FamilyEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call((FamilyEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
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
fn family_history_tab(
    loc: &Localizer,
    detail: &FamilyDetail,
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
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
                on_submit.call((FamilyEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The family editing side panel: renders the form for the open [`FamilyEditForm`], or nothing.
fn family_edit_panel(
    state: &AppState,
    detail: &FamilyDetail,
    mut editing: Signal<Option<FamilyEditForm>>,
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        FamilyEditForm::Partner => loc.action_label("add-partner"),
        FamilyEditForm::Child => loc.action_label("add-child"),
        FamilyEditForm::Event => loc.action_label("link-event"),
        FamilyEditForm::Media => loc.action_label("attach-media"),
        FamilyEditForm::Note => loc.action_label("attach-note"),
        FamilyEditForm::Tag => loc.action_label("add-tag"),
    };
    let human_id = human_id.to_owned();
    let partners: Vec<(String, String)> = detail
        .partners
        .iter()
        .map(|partner| (partner.human_id.clone(), partner.name.clone()))
        .collect();
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |_| editing.set(None),
            footer: rsx! {},
            {match form {
                FamilyEditForm::Partner => rsx! { FamilyAddPartnerForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                FamilyEditForm::Child => rsx! { FamilyAddChildForm { human_id, partners, onsubmit: move |edit| on_submit.call(edit) } },
                FamilyEditForm::Event => rsx! { FamilyLinkEventForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                FamilyEditForm::Media => rsx! { FamilyAttachForm { human_id, is_note: false, onsubmit: move |edit| on_submit.call(edit) } },
                FamilyEditForm::Note => rsx! { FamilyAttachForm { human_id, is_note: true, onsubmit: move |edit| on_submit.call(edit) } },
                FamilyEditForm::Tag => rsx! { FamilyTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Add partner" form: a person `human_id` → [`FamilyEdit::AddPartner`].
#[component]
fn FamilyAddPartnerForm(human_id: String, onsubmit: EventHandler<(FamilyEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut person = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("partner"), name: "partner".to_owned(), oninput: move |event: FormEvent| person.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let person = person();
                if person.trim().is_empty() {
                    return;
                }
                onsubmit.call((FamilyEdit::AddPartner { human_id: human_id.clone(), person_id: person }, prov()));
            },
        }
    }
}

/// The "Add child" form: a child `human_id` plus one relationship select per family partner →
/// [`FamilyEdit::AddChild`].
#[component]
fn FamilyAddChildForm(
    human_id: String,
    partners: Vec<(String, String)>,
    onsubmit: EventHandler<(FamilyEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let relationships = relationship_choices();
    let options: Vec<SelectChoice> = relationships
        .iter()
        .enumerate()
        .map(|(position, relationship)| SelectChoice {
            value: position.to_string(),
            label: loc.relationship_label(relationship),
        })
        .collect();
    let mut child = use_signal(String::new);
    let mut selections = use_signal(|| vec![0_usize; partners.len()]);
    let prov = use_signal(ProvenanceDraft::default);
    let partners_for_submit = partners.clone();
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("child"), name: "child".to_owned(), oninput: move |event: FormEvent| child.set(event.value()) }
        for (index , (_ , name)) in partners.iter().enumerate() {
            Select {
                label: name.clone(),
                name: "rel-{index}".to_owned(),
                value: Some(0.to_string()),
                options: options.clone(),
                onchange: move |event: FormEvent| {
                    let value = event.value().parse::<usize>().unwrap_or(0);
                    selections.with_mut(|slots| {
                        if let Some(slot) = slots.get_mut(index) {
                            *slot = value;
                        }
                    });
                },
            }
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let person = child();
                if person.trim().is_empty() {
                    return;
                }
                let chosen = selections();
                let relationships: Vec<(String, ChildParentRelationship)> = partners_for_submit
                    .iter()
                    .enumerate()
                    .map(|(index, (partner_id, _))| {
                        let relationship = relationship_choices()
                            .get(chosen.get(index).copied().unwrap_or(0))
                            .cloned()
                            .unwrap_or(ChildParentRelationship::Unknown);
                        (partner_id.clone(), relationship)
                    })
                    .collect();
                onsubmit.call((FamilyEdit::AddChild { human_id: human_id.clone(), person_id: person, relationships }, prov()));
            },
        }
    }
}

/// The "Link family event" form: an event `human_id` → [`FamilyEdit::LinkFamilyEvent`].
#[component]
fn FamilyLinkEventForm(human_id: String, onsubmit: EventHandler<(FamilyEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut event = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.tab_label("events"), name: "event".to_owned(), oninput: move |event_input: FormEvent| event.set(event_input.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let event_id = event();
                if event_id.trim().is_empty() {
                    return;
                }
                onsubmit.call((FamilyEdit::LinkFamilyEvent { human_id: human_id.clone(), event_id }, prov()));
            },
        }
    }
}

/// The "Attach media/note by id" form → [`FamilyEdit::AttachMedia`]/[`FamilyEdit::AttachNote`].
#[component]
fn FamilyAttachForm(human_id: String, is_note: bool, onsubmit: EventHandler<(FamilyEdit, ProvenanceDraft)>) -> Element {
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
                    FamilyEdit::AttachNote { human_id: human_id.clone(), note_id: id }
                } else {
                    FamilyEdit::AttachMedia { human_id: human_id.clone(), media_id: id }
                };
                onsubmit.call((edit, prov()));
            },
        }
    }
}

/// The "Add tag" form: a picker of existing tags by name (the tag id is the option value, never
/// shown) → [`FamilyEdit::Tag`].
#[component]
fn FamilyTagForm(human_id: String, onsubmit: EventHandler<(FamilyEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((FamilyEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}
