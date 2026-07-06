use super::prelude::*;
use crate::screens::RecordDetail;

/// The DNA providers offered in the "New match" picker, with a stable value key (matched back in
/// [`provider_from_key`]).
fn provider_choices(loc: &Localizer) -> Vec<SelectChoice> {
    [
        ("ancestry", DnaProvider::AncestryDna),
        ("23andme", DnaProvider::TwentyThreeAndMe),
        ("myheritage", DnaProvider::MyHeritage),
        ("ftdna", DnaProvider::FamilyTreeDna),
        ("gedmatch", DnaProvider::GedMatch),
        ("livingdna", DnaProvider::LivingDna),
    ]
    .into_iter()
    .map(|(value, provider)| SelectChoice {
        value: value.to_owned(),
        label: loc.dna_provider_label(&provider),
    })
    .collect()
}

/// Maps a [`provider_choices`] value key back to its [`DnaProvider`] (defaults to `AncestryDna`).
fn provider_from_key(key: &str) -> DnaProvider {
    match key {
        "23andme" => DnaProvider::TwentyThreeAndMe,
        "myheritage" => DnaProvider::MyHeritage,
        "ftdna" => DnaProvider::FamilyTreeDna,
        "gedmatch" => DnaProvider::GedMatch,
        "livingdna" => DnaProvider::LivingDna,
        _ => DnaProvider::AncestryDna,
    }
}

/// The DNA-match master-detail screen: a list of matches on the left, the selected match's detail
/// (compared tests + shared DNA + inferred relationship + segments/ancestors/notes/tags + history) on
/// the right. `New` opens a form collecting both tests, the provider, and the shared cM.
#[component]
pub fn DnaMatchScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let create_services = services.clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::DnaMatches.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().dna_match_list_empty();
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
        if *nav.pending_create.read() == Some(Category::DnaMatches) {
            creating.set(true);
            nav.pending_create.set(None);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowDnaMatchList).await }
    });
    let on_create = use_callback(
        move |(test_a, test_b, provider, shared_cm): (String, String, DnaProvider, String)| {
            let services = create_services.clone();
            spawn(async move {
                match create_dna_match_record(services, test_a, test_b, provider, shared_cm).await {
                    Ok(human_id) => {
                        creating.set(false);
                        nav.open_record(RecordRef {
                            category: Category::DnaMatches,
                            label: human_id.clone(),
                            human_id,
                        });
                    }
                    Err(message) => toast.set(Some(message)),
                }
            });
        },
    );
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
                    category: Category::DnaMatches,
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
    rsx! {
        MasterDetail { list: list_pane, detail: rsx! { RecordDetail {} } }
        if creating() {
            SidePanel {
                title: create_title,
                open: true,
                close_label: cancel_label,
                onclose: move |_| creating.set(false),
                footer: rsx! {},
                CreateDnaMatchForm { onsubmit: move |payload| on_create.call(payload) }
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

/// The "New DNA match" form: the two tests' `human_id`s (required), the provider, and the shared cM.
#[component]
fn CreateDnaMatchForm(onsubmit: EventHandler<(String, String, DnaProvider, String)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut test_a = use_signal(String::new);
    let mut test_b = use_signal(String::new);
    let mut provider = use_signal(|| "ancestry".to_owned());
    let mut shared_cm = use_signal(String::new);
    let save_label = loc.action_label("save");
    let options = provider_choices(loc);
    rsx! {
        Input { label: loc.field_label("test-a"), name: "test-a".to_owned(), oninput: move |event: FormEvent| test_a.set(event.value()) }
        Input { label: loc.field_label("test-b"), name: "test-b".to_owned(), oninput: move |event: FormEvent| test_b.set(event.value()) }
        Select {
            label: loc.field_label("provider"),
            name: "provider".to_owned(),
            value: Some("ancestry".to_owned()),
            options,
            onchange: move |event: FormEvent| provider.set(event.value()),
        }
        Input { label: loc.field_label("shared-cm"), name: "shared-cm".to_owned(), oninput: move |event: FormEvent| shared_cm.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let test_a = test_a();
                let test_b = test_b();
                if test_a.trim().is_empty() || test_b.trim().is_empty() {
                    return;
                }
                onsubmit.call((test_a, test_b, provider_from_key(&provider()), shared_cm()));
            },
        }
    }
}

/// Which DNA-match edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnaMatchEditForm {
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected DNA match: header, related-item tabs, editing side panel.
#[component]
pub(crate) fn DnaMatchDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let mut nav = use_context::<NavState>();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<DnaMatchEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowDnaMatch { human_id }).await }
    });

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the match's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::DnaMatchDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        nav.set_record_label(
            Category::DnaMatches,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (DnaMatchEdit, ProvenanceDraft)| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_dna_match_edit(services, edit, prov).await {
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
        Some(ScreenData::Loaded(IntentOutcome::DnaMatchDetail(detail))) => {
            dna_match_detail(&state, detail, active, editing, on_submit, &human_id)
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
            | IntentOutcome::DnaTestDetail(_)
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

/// Renders a loaded DNA match's detail container: header, the tab strip, the active tab, the panel.
fn dna_match_detail(
    state: &AppState,
    detail: &DnaMatchDetail,
    active: Signal<usize>,
    editing: Signal<Option<DnaMatchEditForm>>,
    on_submit: Callback<(DnaMatchEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let tabs = dna_match_tabs(detail, loc);
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
            avatar: "🔗".to_owned(),
            extras: dna_match_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {dna_match_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {dna_match_edit_panel(state, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for a DNA match (the mockup `resn-set`).
fn dna_match_restriction_toggles(
    loc: &Localizer,
    detail: &DnaMatchDetail,
    on_submit: Callback<(DnaMatchEdit, ProvenanceDraft)>,
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
                on_submit.call((DnaMatchEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one DNA-match detail tab, with its contextual add affordances.
fn dna_match_tab_content(
    state: &AppState,
    detail: &DnaMatchDetail,
    tab_id: &str,
    mut editing: Signal<Option<DnaMatchEditForm>>,
    on_submit: Callback<(DnaMatchEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "segments" => dna_match_segments_table(loc, &detail.segments),
        "ancestors" => dna_match_ancestors_table(loc, &detail.shared_ancestors),
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(DnaMatchEditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => dna_match_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => dna_match_history_tab(loc, detail, on_submit, human_id),
        _ => dna_match_overview(loc, detail, on_submit, human_id),
    }
}

/// The DNA-match Overview: compared-tests card, the observed shared-DNA card, and the inferred
/// relationship (conclusion) card with confirm/reject controls.
pub fn dna_match_overview(
    loc: &Localizer,
    detail: &DnaMatchDetail,
    on_submit: Callback<(DnaMatchEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let dash = "—".to_owned();
    let human_id_confirm = human_id.to_owned();
    let human_id_reject = human_id.to_owned();
    rsx! {
        div { class: "section-note", "{loc.dna_match_overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.section_label("compared-tests"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"test-a\")}" }
                        span { class: "grow", {detail.test_a.as_ref().map_or_else(|| dash.clone(), |t| t.label.clone())} }
                        if let Some(test) = &detail.test_a { span { class: "muted mono", "{test.human_id}" } }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"test-b\")}" }
                        span { class: "grow", {detail.test_b.as_ref().map_or_else(|| dash.clone(), |t| t.label.clone())} }
                        if let Some(test) = &detail.test_b { span { class: "muted mono", "{test.human_id}" } }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"provider\")}" }
                        span { class: "grow", {detail.provider.clone().unwrap_or_else(|| dash.clone())} }
                    }
                }
            }
            Card { title: loc.section_label("shared-dna"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:130px;margin:0", "{loc.field_label(\"shared-cm\")}" }
                        span { class: "grow", b { {detail.shared_cm.clone().unwrap_or_else(|| dash.clone())} } }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:130px;margin:0", "{loc.field_label(\"percent-shared\")}" }
                        span { class: "grow", {detail.percent_shared.clone().unwrap_or_else(|| dash.clone())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:130px;margin:0", "{loc.field_label(\"segment-count\")}" }
                        span { class: "grow", "{detail.segments.len()}" }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:130px;margin:0", "{loc.field_label(\"largest-segment\")}" }
                        span { class: "grow", {detail.largest_segment_cm.clone().unwrap_or_else(|| dash.clone())} }
                    }
                }
            }
        }
        Card { title: loc.section_label("inferred-relationship"),
            div { class: "section-note", style: "margin:0 0 8px", "{loc.dna_match_overview_note()}" }
            div { class: "fact-row",
                span { class: "grow", {detail.predicted_relationship.clone().unwrap_or_else(|| dash.clone())} }
                Chip { label: detail.status.clone() }
            }
            div { class: "row-actions", style: "margin-top:8px",
                Button { label: loc.action_label("confirm"), variant: ButtonVariant::Default, small: true, onclick: move |_| on_submit.call((DnaMatchEdit::SetStatus { human_id: human_id_confirm.clone(), confirmed: true }, ProvenanceDraft::default())) }
                Button { label: loc.action_label("reject"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| on_submit.call((DnaMatchEdit::SetStatus { human_id: human_id_reject.clone(), confirmed: false }, ProvenanceDraft::default())) }
            }
        }
    }
}

/// The DNA-match Segments tab: one row per matching segment (chr/start/end/cM/SNPs/side).
pub fn dna_match_segments_table(loc: &Localizer, segments: &[DnaSegmentVm]) -> Element {
    if segments.is_empty() {
        return rsx! {
            div { class: "section-note", "{loc.dna_match_segments_note()}" }
            EmptyState { message: loc.tab_empty() }
        };
    }
    let dash = "—".to_owned();
    rsx! {
        div { class: "section-note", "{loc.dna_match_segments_note()}" }
        Table {
            headers: vec![
                loc.field_label("chromosome"),
                loc.field_label("start"),
                loc.field_label("end"),
                loc.field_label("centimorgans"),
                loc.field_label("snps"),
                loc.field_label("side"),
            ],
            for segment in segments.iter() {
                tr {
                    td { "{segment.chromosome}" }
                    td { class: "mono", "{segment.start}" }
                    td { class: "mono", "{segment.end}" }
                    td { b { "{segment.centimorgans}" } }
                    td { {segment.snps.clone().unwrap_or_else(|| dash.clone())} }
                    td { Chip { label: segment.side.clone() } }
                }
            }
        }
    }
}

/// The DNA-match Shared ancestors tab: one row per inferred common ancestor (name + note).
pub fn dna_match_ancestors_table(loc: &Localizer, ancestors: &[SharedAncestorVm]) -> Element {
    if ancestors.is_empty() {
        return rsx! {
            div { class: "section-note", "{loc.dna_match_ancestors_note()}" }
            EmptyState { message: loc.tab_empty() }
        };
    }
    let dash = "—".to_owned();
    rsx! {
        div { class: "section-note", "{loc.dna_match_ancestors_note()}" }
        Table {
            headers: vec![loc.field_label("ancestor"), loc.field_label("note")],
            for ancestor in ancestors.iter() {
                tr {
                    td { {ancestor.person.as_ref().map_or_else(|| dash.clone(), |p| p.label.clone())} }
                    td { class: "muted", {ancestor.note.clone().unwrap_or_else(|| dash.clone())} }
                }
            }
        }
    }
}

/// The DNA-match Tags tab: each applied tag as a colour-dot chip (name + colour, never id) with remove.
pub fn dna_match_tags_panel(
    loc: &Localizer,
    detail: &DnaMatchDetail,
    mut editing: Signal<Option<DnaMatchEditForm>>,
    on_submit: Callback<(DnaMatchEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(DnaMatchEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call((DnaMatchEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The DNA-match History tab: the audit timeline, each undoable entry carrying an undo control.
fn dna_match_history_tab(
    loc: &Localizer,
    detail: &DnaMatchDetail,
    on_submit: Callback<(DnaMatchEdit, ProvenanceDraft)>,
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
                on_submit.call((DnaMatchEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The DNA-match editing side panel: renders the form for the open [`DnaMatchEditForm`], or nothing.
fn dna_match_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<DnaMatchEditForm>>,
    on_submit: Callback<(DnaMatchEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        DnaMatchEditForm::Note => loc.action_label("attach-note"),
        DnaMatchEditForm::Tag => loc.action_label("add-tag"),
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
                DnaMatchEditForm::Note => rsx! { DnaMatchNoteForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                DnaMatchEditForm::Tag => rsx! { DnaMatchTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The DNA-match "attach note by id" form → [`DnaMatchEdit::AttachNote`].
#[component]
fn DnaMatchNoteForm(human_id: String, onsubmit: EventHandler<(DnaMatchEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut value = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("note"), name: "note".to_owned(), oninput: move |event: FormEvent| value.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let value = value();
                if value.trim().is_empty() {
                    return;
                }
                onsubmit.call((DnaMatchEdit::AttachNote { human_id: human_id.clone(), note_id: value }, prov()));
            },
        }
    }
}

/// The DNA-match "Add tag" form: a picker of existing tags by name → [`DnaMatchEdit::Tag`].
#[component]
fn DnaMatchTagForm(human_id: String, onsubmit: EventHandler<(DnaMatchEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((DnaMatchEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}
