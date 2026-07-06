use super::prelude::*;
use crate::screens::RecordDetail;
use genealogy_app::{DnaGenomeBuild, DnaProvider, DnaTestType};

/// The DNA-test master-detail screen: a list of tests on the left, the selected test's detail
/// (kit metadata + haplogroups + matches + notes/tags + history) on the right. `New` opens a form
/// collecting the anchoring person's `human_id`.
#[component]
pub fn DnaTestScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::DnaTests.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().dna_test_list_empty();
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
        if *nav.pending_create.read() == Some(Category::DnaTests) {
            creating.set(true);
            nav.pending_create.set(None);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowDnaTestList).await }
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
                        category: Category::DnaTests,
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
    let on_created = use_callback(move |id: String| {
        creating.set(false);
        nav.open_record(RecordRef {
            category: Category::DnaTests,
            human_id: id.clone(),
            label: id,
        });
    });
    let detail = if creating() {
        rsx! {
            DnaTestCreateRecord {
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

/// The create-mode DNA-test record: an uncommitted [`DnaTestDraft`] rendered as the create form in
/// the detail pane (`record-editing.html` §6). The person is required (§7); Save commits the whole
/// test; Cancel discards.
#[component]
fn DnaTestCreateRecord(
    oncreated: EventHandler<String>,
    oncancel: EventHandler<()>,
    onerror: EventHandler<String>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let draft = use_signal(genealogy_ui::DnaTestDraft::new);
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty() && draft().is_valid();
    let on_save = use_callback(move |()| {
        let Some(request) = draft().to_request() else {
            return;
        };
        let services = services.clone();
        let prov = prov();
        spawn(async move {
            match commit_dna_test_change_set(services, request, prov).await {
                Ok(id) => oncreated.call(id),
                Err(message) => onerror.call(message),
            }
        });
    });
    rsx! {
        {create_record_header(&loc.dna_test_new_title(), &loc.record_draft_badge())}
        {dna_test_create_fields(loc, draft)}
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

/// The DNA-test create form's field rows (`dna-test.html` edit specimen): a required Person, then
/// Provider · Test type · Genome build · Kit id. A pure fn (no `AppCtx`) so SSR tests render it.
pub fn dna_test_create_fields(loc: &Localizer, mut draft: Signal<genealogy_ui::DnaTestDraft>) -> Element {
    let person_invalid = draft().person_invalid();
    let providers = dna_provider_choices();
    let (provider_options, provider_selected) =
        optional_enum_select(loc.record_unset(), &providers, draft().provider.as_ref(), |provider| {
            loc.dna_provider_label(provider)
        });
    let test_types = [
        DnaTestType::Autosomal,
        DnaTestType::YDna,
        DnaTestType::MtDna,
        DnaTestType::XDna,
    ];
    let (type_options, type_selected) = optional_enum_select(
        loc.record_unset(),
        &test_types,
        draft().test_type.as_ref(),
        |test_type| loc.dna_test_type_label(*test_type),
    );
    let builds = [DnaGenomeBuild::GRCh37, DnaGenomeBuild::GRCh38];
    let (build_options, build_selected) =
        optional_enum_select(loc.record_unset(), &builds, draft().genome_build.as_ref(), |build| {
            loc.dna_genome_build_label(*build)
        });
    let person_error = loc.dna_test_person_required();
    rsx! {
        Card { title: loc.section_label("kit"),
            div { class: "stack",
                div { class: "field",
                    label { r#for: "dna-test-person", "{loc.field_label(\"person\")}" }
                    input {
                        class: if person_invalid { "in invalid" } else { "in" },
                        r#type: "text",
                        id: "dna-test-person",
                        name: "dna-test-person",
                        value: "{draft().person}",
                        aria_invalid: if person_invalid { "true" } else { "false" },
                        oninput: move |event| draft.write().person = event.value(),
                    }
                    if person_invalid {
                        div { class: "field-error", "{person_error}" }
                    }
                }
                Select {
                    label: loc.field_label("provider"),
                    name: "dna-test-provider".to_owned(),
                    value: Some(provider_selected),
                    options: provider_options,
                    onchange: move |event: FormEvent| {
                        let providers = dna_provider_choices();
                        draft.write().provider = event.value().parse::<usize>().ok().and_then(|index| providers.get(index).cloned());
                    },
                }
                Select {
                    label: loc.field_label("test-type"),
                    name: "dna-test-type".to_owned(),
                    value: Some(type_selected),
                    options: type_options,
                    onchange: move |event: FormEvent| {
                        let test_types = [DnaTestType::Autosomal, DnaTestType::YDna, DnaTestType::MtDna, DnaTestType::XDna];
                        draft.write().test_type = event.value().parse::<usize>().ok().and_then(|index| test_types.get(index).copied());
                    },
                }
                Select {
                    label: loc.field_label("genome-build"),
                    name: "dna-test-genome-build".to_owned(),
                    value: Some(build_selected),
                    options: build_options,
                    onchange: move |event: FormEvent| {
                        let builds = [DnaGenomeBuild::GRCh37, DnaGenomeBuild::GRCh38];
                        draft.write().genome_build = event.value().parse::<usize>().ok().and_then(|index| builds.get(index).copied());
                    },
                }
                Input {
                    label: loc.field_label("kit-id"),
                    name: "dna-test-kit-id".to_owned(),
                    value: draft().kit_id.clone(),
                    oninput: move |event: FormEvent| draft.write().kit_id = event.value(),
                }
            }
        }
    }
}

/// Which DNA-test edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnaTestEditForm {
    /// Set the testing provider.
    Provider,
    /// Set the kit id.
    KitId,
    /// Set the test type.
    Type,
    /// Set the genome build.
    GenomeBuild,
    /// Assert a haplogroup.
    Haplogroup,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected DNA test: header, related-item tabs, editing side panel.
#[component]
pub(crate) fn DnaTestDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let mut nav = use_context::<NavState>();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<DnaTestEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowDnaTest { human_id }).await }
    });

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the test's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::DnaTestDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        nav.set_record_label(
            Category::DnaTests,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (DnaTestEdit, ProvenanceDraft)| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_dna_test_edit(services, edit, prov).await {
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
        Some(ScreenData::Loaded(IntentOutcome::DnaTestDetail(detail))) => {
            dna_test_detail(&state, detail, active, editing, on_submit, &human_id)
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
            | IntentOutcome::TagDetail(_)
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

/// Renders a loaded DNA test's detail container: header, the tab strip, the active tab, the panel.
fn dna_test_detail(
    state: &AppState,
    detail: &DnaTestDetail,
    active: Signal<usize>,
    editing: Signal<Option<DnaTestEditForm>>,
    on_submit: Callback<(DnaTestEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let tabs = dna_test_tabs(detail, loc);
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
            avatar: "🧬".to_owned(),
            extras: dna_test_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {dna_test_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {dna_test_edit_panel(state, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for a DNA test (the mockup `resn-set`).
fn dna_test_restriction_toggles(
    loc: &Localizer,
    detail: &DnaTestDetail,
    on_submit: Callback<(DnaTestEdit, ProvenanceDraft)>,
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
                on_submit.call((DnaTestEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one DNA-test detail tab, with its contextual add affordances.
fn dna_test_tab_content(
    state: &AppState,
    detail: &DnaTestDetail,
    tab_id: &str,
    mut editing: Signal<Option<DnaTestEditForm>>,
    on_submit: Callback<(DnaTestEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "haplogroups" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-haplogroup"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(DnaTestEditForm::Haplogroup)) }
            }
            {dna_test_haplogroups_table(loc, &detail.haplogroups)}
        },
        "matches" => dna_test_matches_table(loc, &detail.matches),
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(DnaTestEditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => dna_test_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => dna_test_history_tab(loc, detail, on_submit, human_id),
        _ => dna_test_overview(loc, detail, editing),
    }
}

/// The DNA-test Overview: the Kit details card, the Tested-person card, and the ethnicity note.
pub fn dna_test_overview(
    loc: &Localizer,
    detail: &DnaTestDetail,
    mut editing: Signal<Option<DnaTestEditForm>>,
) -> Element {
    let dash = "—".to_owned();
    rsx! {
        div { class: "section-note", "{loc.dna_test_overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.section_label("kit"),
                div { class: "tab-actions",
                    Button { label: loc.field_label("provider"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(DnaTestEditForm::Provider)) }
                    Button { label: loc.field_label("kit-id"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(DnaTestEditForm::KitId)) }
                    Button { label: loc.field_label("type"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(DnaTestEditForm::Type)) }
                    Button { label: loc.field_label("genome-build"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(DnaTestEditForm::GenomeBuild)) }
                }
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"provider\")}" }
                        span { class: "grow", {detail.provider.clone().unwrap_or_else(|| dash.clone())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"test-type\")}" }
                        span { class: "grow", {detail.test_type.clone().unwrap_or_else(|| dash.clone())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"kit-id\")}" }
                        span { class: "grow mono", {detail.kit_id.clone().unwrap_or_else(|| dash.clone())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"genome-build\")}" }
                        span { class: "grow", {detail.genome_build.clone().unwrap_or_else(|| dash.clone())} }
                    }
                }
            }
            Card { title: loc.section_label("tested-person"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"person\")}" }
                        span { class: "grow", {detail.person_name.clone().unwrap_or_else(|| dash.clone())} }
                        if let Some(person) = &detail.person {
                            span { class: "muted mono", "{person.human_id}" }
                        }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.tab_label(\"matches\")}" }
                        span { class: "grow", "{detail.matches.len()}" }
                    }
                }
            }
        }
        Card { title: loc.section_label("ethnicity"),
            div { class: "section-note", style: "margin:0", "{loc.dna_test_ethnicity_note()}" }
        }
    }
}

/// The DNA-test Haplogroups tab: one row per recorded haplogroup.
pub fn dna_test_haplogroups_table(loc: &Localizer, haplogroups: &[String]) -> Element {
    if haplogroups.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![loc.field_label("haplogroup")],
            for haplogroup in haplogroups.iter() {
                tr { td { b { "{haplogroup}" } } }
            }
        }
    }
}

/// The DNA-test Matches tab: one row per match this kit produced (match, compared test, cM, %, predicted).
pub fn dna_test_matches_table(loc: &Localizer, matches: &[DnaTestMatchVm]) -> Element {
    if matches.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    let dash = "—".to_owned();
    rsx! {
        Table {
            headers: vec![
                loc.tab_label("matches"),
                loc.field_label("compared-test"),
                loc.field_label("shared-cm"),
                loc.field_label("percent-shared"),
                loc.field_label("predicted"),
            ],
            for row in matches.iter() {
                tr {
                    td { "{row.match_ref.human_id}" }
                    td { class: "muted mono", {row.compared_test.as_ref().map_or_else(|| dash.clone(), |t| t.human_id.clone())} }
                    td { b { {row.shared_cm.clone().unwrap_or_else(|| dash.clone())} } }
                    td { {row.percent_shared.clone().unwrap_or_else(|| dash.clone())} }
                    td { if let Some(predicted) = row.predicted.clone() { Chip { label: predicted } } }
                }
            }
        }
    }
}

/// The DNA-test Tags tab: each applied tag as a colour-dot chip (name + colour, never id) with remove.
pub fn dna_test_tags_panel(
    loc: &Localizer,
    detail: &DnaTestDetail,
    mut editing: Signal<Option<DnaTestEditForm>>,
    on_submit: Callback<(DnaTestEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(DnaTestEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call((DnaTestEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The DNA-test History tab: the audit timeline, each undoable entry carrying an undo control.
fn dna_test_history_tab(
    loc: &Localizer,
    detail: &DnaTestDetail,
    on_submit: Callback<(DnaTestEdit, ProvenanceDraft)>,
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
                on_submit.call((DnaTestEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The DNA-test editing side panel: renders the form for the open [`DnaTestEditForm`], or nothing.
fn dna_test_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<DnaTestEditForm>>,
    on_submit: Callback<(DnaTestEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        DnaTestEditForm::Provider => loc.field_label("provider"),
        DnaTestEditForm::KitId => loc.field_label("kit-id"),
        DnaTestEditForm::Type => loc.field_label("type"),
        DnaTestEditForm::GenomeBuild => loc.field_label("genome-build"),
        DnaTestEditForm::Haplogroup => loc.action_label("add-haplogroup"),
        DnaTestEditForm::Note => loc.action_label("attach-note"),
        DnaTestEditForm::Tag => loc.action_label("add-tag"),
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
                DnaTestEditForm::Provider => rsx! { DnaTestProviderForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                DnaTestEditForm::KitId => rsx! { DnaTestFieldForm { human_id, field: "kit-id".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                DnaTestEditForm::Type => rsx! { DnaTestTypeForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                DnaTestEditForm::GenomeBuild => rsx! { DnaTestGenomeBuildForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                DnaTestEditForm::Haplogroup => rsx! { DnaTestFieldForm { human_id, field: "haplogroup".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                DnaTestEditForm::Note => rsx! { DnaTestFieldForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                DnaTestEditForm::Tag => rsx! { DnaTestTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "add haplogroup / attach note by id" form → the matching [`DnaTestEdit`] variant.
#[component]
fn DnaTestFieldForm(
    human_id: String,
    field: String,
    onsubmit: EventHandler<(DnaTestEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut value = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    let field_label = loc.field_label(&field);
    rsx! {
        Input { label: field_label, name: field.clone(), oninput: move |event: FormEvent| value.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let value = value();
                if value.trim().is_empty() {
                    return;
                }
                let edit = match field.as_str() {
                    "haplogroup" => DnaTestEdit::AddHaplogroup { human_id: human_id.clone(), haplogroup: value },
                    "kit-id" => DnaTestEdit::SetKitId { human_id: human_id.clone(), kit_id: value },
                    _ => DnaTestEdit::AttachNote { human_id: human_id.clone(), note_id: value },
                };
                onsubmit.call((edit, prov()));
            },
        }
    }
}

/// The DNA-test "Add tag" form: a picker of existing tags by name → [`DnaTestEdit::Tag`].
#[component]
fn DnaTestTagForm(human_id: String, onsubmit: EventHandler<(DnaTestEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((DnaTestEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

/// The "Set provider" form: a provider picker → [`DnaTestEdit::SetProvider`].
#[component]
fn DnaTestProviderForm(human_id: String, onsubmit: EventHandler<(DnaTestEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let choices = dna_provider_choices();
    let options: Vec<SelectChoice> = choices
        .iter()
        .enumerate()
        .map(|(position, provider)| SelectChoice {
            value: position.to_string(),
            label: loc.dna_provider_label(provider),
        })
        .collect();
    let mut chosen = use_signal(|| 0_usize);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Select {
            label: loc.field_label("provider"),
            name: "provider".to_owned(),
            value: Some(0.to_string()),
            options,
            onchange: move |event: FormEvent| chosen.set(event.value().parse::<usize>().unwrap_or(0)),
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let provider = dna_provider_choices().get(chosen()).cloned().unwrap_or(DnaProvider::AncestryDna);
                onsubmit.call((DnaTestEdit::SetProvider { human_id: human_id.clone(), provider }, prov()));
            },
        }
    }
}

/// The "Set type" form: a test-type picker → [`DnaTestEdit::SetType`].
#[component]
fn DnaTestTypeForm(human_id: String, onsubmit: EventHandler<(DnaTestEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let choices = [
        DnaTestType::Autosomal,
        DnaTestType::YDna,
        DnaTestType::MtDna,
        DnaTestType::XDna,
    ];
    let options: Vec<SelectChoice> = choices
        .iter()
        .enumerate()
        .map(|(position, test_type)| SelectChoice {
            value: position.to_string(),
            label: loc.dna_test_type_label(*test_type),
        })
        .collect();
    let mut chosen = use_signal(|| 0_usize);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Select {
            label: loc.field_label("type"),
            name: "type".to_owned(),
            value: Some(0.to_string()),
            options,
            onchange: move |event: FormEvent| chosen.set(event.value().parse::<usize>().unwrap_or(0)),
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let test_type = [DnaTestType::Autosomal, DnaTestType::YDna, DnaTestType::MtDna, DnaTestType::XDna]
                    .get(chosen())
                    .copied()
                    .unwrap_or(DnaTestType::Autosomal);
                onsubmit.call((DnaTestEdit::SetType { human_id: human_id.clone(), test_type }, prov()));
            },
        }
    }
}

/// The "Set genome build" form: a build picker → [`DnaTestEdit::SetGenomeBuild`].
#[component]
fn DnaTestGenomeBuildForm(human_id: String, onsubmit: EventHandler<(DnaTestEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let choices = [DnaGenomeBuild::GRCh37, DnaGenomeBuild::GRCh38];
    let options: Vec<SelectChoice> = choices
        .iter()
        .enumerate()
        .map(|(position, build)| SelectChoice {
            value: position.to_string(),
            label: loc.dna_genome_build_label(*build),
        })
        .collect();
    let mut chosen = use_signal(|| 0_usize);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Select {
            label: loc.field_label("genome-build"),
            name: "genome-build".to_owned(),
            value: Some(0.to_string()),
            options,
            onchange: move |event: FormEvent| chosen.set(event.value().parse::<usize>().unwrap_or(0)),
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let genome_build = [DnaGenomeBuild::GRCh37, DnaGenomeBuild::GRCh38]
                    .get(chosen())
                    .copied()
                    .unwrap_or(DnaGenomeBuild::GRCh38);
                onsubmit.call((DnaTestEdit::SetGenomeBuild { human_id: human_id.clone(), genome_build }, prov()));
            },
        }
    }
}

/// The DNA providers offered by the provider picker (the named providers; not the free-text custom).
fn dna_provider_choices() -> Vec<DnaProvider> {
    vec![
        DnaProvider::AncestryDna,
        DnaProvider::TwentyThreeAndMe,
        DnaProvider::MyHeritage,
        DnaProvider::FamilyTreeDna,
        DnaProvider::GedMatch,
        DnaProvider::LivingDna,
    ]
}

// ---------------------------------------------------------------------------------------------------
// DnaMatch slice
// ---------------------------------------------------------------------------------------------------
