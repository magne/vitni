use super::prelude::*;
use crate::screens::RecordDetail;
use genealogy_app::MatchStatus;

/// The DNA providers offered in the create form's provider select, in display order.
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

/// The DNA-match master-detail screen: a list of matches on the left, the selected match's detail
/// (compared tests + shared DNA + inferred relationship + segments/ancestors/notes/tags + history) on
/// the right. `New` opens a form collecting both tests, the provider, and the shared cM.
#[component]
pub fn DnaMatchScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::DnaMatches.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().dna_match_list_empty();
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
    use_record_step(nav, Category::DnaMatches, list, query, selected);
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
                        category: Category::DnaMatches,
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
            category: Category::DnaMatches,
            human_id: id.clone(),
            label: id,
        });
    });
    let detail = if creating() {
        rsx! {
            DnaMatchCreateRecord {
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

/// The create-mode DNA-match record: an uncommitted [`DnaMatchDraft`] rendered as the create form in
/// the detail pane (`record-editing.html` §6). The two tests, provider, and shared-cM are required; an
/// unparseable numeric is rejected (never zero-filled — §7). Save commits the match; Cancel discards.
#[component]
fn DnaMatchCreateRecord(
    oncreated: EventHandler<String>,
    oncancel: EventHandler<()>,
    onerror: EventHandler<String>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::DnaMatchDraft>();
    let mut draft = record.draft;
    // The two existing-test pickers: options load once; each excludes the other's pick so a match is
    // never asserted between a test and itself. Pick/clear drive the draft's (required) test ids.
    let test_a_state = use_signal(genealogy_ui::PickerState::default);
    let test_b_state = use_signal(genealogy_ui::PickerState::default);
    let test_a_services = services.clone();
    let test_a_rows = use_resource(move || {
        let services = test_a_services.clone();
        async move { load_picker_rows(services, Category::DnaTests).await }
    });
    let test_b_services = services.clone();
    let test_b_rows = use_resource(move || {
        let services = test_b_services.clone();
        async move { load_picker_rows(services, Category::DnaTests).await }
    });
    let test_a_onpick = use_callback(move |selection: PickerSelection| draft.write().test_a = selection.human_id);
    let test_a_onclear = use_callback(move |()| draft.write().test_a = String::new());
    let test_b_onpick = use_callback(move |selection: PickerSelection| draft.write().test_b = selection.human_id);
    let test_b_onclear = use_callback(move |()| draft.write().test_b = String::new());
    let noop_new = use_callback(move |_query: String| {});
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::DnaMatchDraft, ProvenanceDraft)| {
        let Some(request) = draft.to_request() else {
            return;
        };
        let services = services.clone();
        spawn(async move {
            match commit_dna_match_change_set(services, request, prov).await {
                Ok(id) => oncreated.call(id),
                Err(message) => onerror.call(message),
            }
        });
    });
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| oncancel.call(()) }
        Button {
            label: loc.action_label("save"),
            variant: ButtonVariant::Primary,
            small: true,
            disabled: !can_save,
            onclick: move |_| {
                if record.can_save() {
                    on_save.call((record.draft.read().clone(), record.prov.read().clone()));
                }
            },
        }
    };
    let exclude_of = |id: String| if id.trim().is_empty() { Vec::new() } else { vec![id] };
    let test_a = RecordPicker {
        config: PickerConfig {
            label: loc.field_label("test-a"),
            name: "dna-match-test-a".to_owned(),
            entity_label: loc.picker_entity(Category::DnaTests),
            allow_new: false,
        },
        state: test_a_state,
        options: picker_options(test_a_rows.read_unchecked().as_ref()),
        exclude: exclude_of(draft().test_b.clone()),
        callbacks: PickerCallbacks {
            onpick: test_a_onpick,
            onclear: test_a_onclear,
            onnew: noop_new,
        },
    };
    let test_b = RecordPicker {
        config: PickerConfig {
            label: loc.field_label("test-b"),
            name: "dna-match-test-b".to_owned(),
            entity_label: loc.picker_entity(Category::DnaTests),
            allow_new: false,
        },
        state: test_b_state,
        options: picker_options(test_b_rows.read_unchecked().as_ref()),
        exclude: exclude_of(draft().test_a.clone()),
        callbacks: PickerCallbacks {
            onpick: test_b_onpick,
            onclear: test_b_onclear,
            onnew: noop_new,
        },
    };
    create_record_frame(
        &loc.dna_match_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {dna_match_create_fields(loc, draft, &test_a, &test_b)}
            {record_edit_provenance(loc, record)}
        },
    )
}

/// A DNA match's locked observation fields (§3, disabled inputs): the two compared tests, provider,
/// and the observed shared-DNA totals are the provider's observation, never edited here — shown
/// read-only from the record. Factored out of [`dna_match_record_fields`] to stay under the length cap.
fn dna_match_locked_fields(loc: &Localizer, editing: bool, detail: &DnaMatchDetail) -> Element {
    let dash = "—".to_owned();
    let locked = |name: &'static str, label: String, value: String| {
        rsx! {
            DraftText {
                label,
                name: name.to_owned(),
                editing,
                value: value.clone(),
                original: value,
                reset_label: String::new(),
                locked: true,
                oninput: move |_: String| {},
                onreset: move |()| {},
            }
        }
    };
    let test_a = detail.test_a.as_ref().map_or_else(|| dash.clone(), |t| t.label.clone());
    let test_b = detail.test_b.as_ref().map_or_else(|| dash.clone(), |t| t.label.clone());
    rsx! {
        {locked("dna-match-test-a", loc.field_label("test-a"), test_a)}
        {locked("dna-match-test-b", loc.field_label("test-b"), test_b)}
        {locked("dna-match-provider", loc.field_label("provider"), detail.provider.clone().unwrap_or_else(|| dash.clone()))}
        {locked("dna-match-shared-cm", loc.field_label("shared-cm"), detail.shared_cm.clone().unwrap_or_else(|| dash.clone()))}
        {locked("dna-match-percent", loc.field_label("percent-shared"), detail.percent_shared.clone().unwrap_or_else(|| dash.clone()))}
        {locked("dna-match-largest", loc.field_label("largest-segment"), detail.largest_segment_cm.clone().unwrap_or_else(|| dash.clone()))}
        {locked("dna-match-segments", loc.field_label("segment-count"), detail.segments.len().to_string())}
        {locked("dna-match-predicted", loc.field_label("predicted"), detail.predicted_relationship.clone().unwrap_or_else(|| dash.clone()))}
    }
}

/// The DNA match's scalar record fields, read-first (`record-editing.html` §2/§3): the editable id and
/// confirmation status, plus the locked observation totals ([`dna_match_locked_fields`]). Takes the
/// detail for the locked display values; a pure fn so the SSR tests render it without `AppCtx`.
pub fn dna_match_record_fields(
    loc: &Localizer,
    detail: &DnaMatchDetail,
    record: RecordEditState<genealogy_ui::DnaMatchDraft>,
) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let statuses = [MatchStatus::Confirmed, MatchStatus::Rejected];
    let (status_options, status_value, status_original) = record_enum_select(
        loc.match_status_label(None),
        &statuses,
        draft().status.as_ref(),
        seed.read().status.as_ref(),
        |status| loc.match_status_label(Some(*status)),
    );
    let id_value = draft().human_id.clone();
    let id_original = seed.read().human_id.clone();
    rsx! {
        Card { title: loc.section_label("compared-tests"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "dna-match-id".to_owned(),
                    editing,
                    value: id_value,
                    original: id_original,
                    reset_label: loc.action_reset_field(&loc.field_label("id")),
                    mono: true,
                    hint: Some(loc.field_human_id_hint()),
                    oninput: move |value: String| draft.write().human_id = value,
                    onreset: move |()| {
                        let value = seed.read().human_id.clone();
                        draft.write().human_id = value;
                    },
                }
                {dna_match_locked_fields(loc, editing, detail)}
                DraftSelect {
                    label: loc.field_label("status"),
                    name: "dna-match-status".to_owned(),
                    editing,
                    value: status_value,
                    original: status_original,
                    reset_label: loc.action_reset_field(&loc.field_label("status")),
                    options: status_options,
                    onchange: move |value: String| {
                        let statuses = [MatchStatus::Confirmed, MatchStatus::Rejected];
                        draft.write().status = value.parse::<usize>().ok().and_then(|index| statuses.get(index).copied());
                    },
                    onreset: move |()| {
                        let value = seed.read().status;
                        draft.write().status = value;
                    },
                }
            }
        }
    }
}

/// The DNA-match create form's field rows (`dna-match.html` edit specimen, segments/ancestors are
/// PR30): the two tests + provider (required), the shared-cM (required, flagged when unparseable —
/// §7), and the optional %-shared, largest cM, and segment count. A pure fn (no `AppCtx`) so SSR
/// tests render it directly.
pub fn dna_match_create_fields(
    loc: &Localizer,
    mut draft: Signal<genealogy_ui::DnaMatchDraft>,
    test_a: &RecordPicker,
    test_b: &RecordPicker,
) -> Element {
    let providers = dna_provider_choices();
    let (provider_options, provider_selected) =
        optional_enum_select(loc.record_unset(), &providers, draft().provider.as_ref(), |provider| {
            loc.dna_provider_label(provider)
        });
    let shared_cm_invalid = draft().shared_cm_invalid();
    let shared_error = loc.dna_match_shared_cm_invalid();
    rsx! {
        Card { title: loc.tab_label("overview"),
            div { class: "stack",
                {record_picker(loc, test_a)}
                {record_picker(loc, test_b)}
                Select {
                    label: loc.field_label("provider"),
                    name: "dna-match-provider".to_owned(),
                    value: Some(provider_selected),
                    options: provider_options,
                    onchange: move |event: FormEvent| {
                        let providers = dna_provider_choices();
                        draft.write().provider = event.value().parse::<usize>().ok().and_then(|index| providers.get(index).cloned());
                    },
                }
                div { class: "field",
                    label { r#for: "dna-match-shared-cm", "{loc.field_label(\"shared-cm\")}" }
                    input {
                        class: if shared_cm_invalid { "in invalid" } else { "in" },
                        r#type: "text",
                        inputmode: "decimal",
                        id: "dna-match-shared-cm",
                        name: "dna-match-shared-cm",
                        value: "{draft().shared_cm}",
                        aria_invalid: if shared_cm_invalid { "true" } else { "false" },
                        oninput: move |event| draft.write().shared_cm = event.value(),
                    }
                    if shared_cm_invalid {
                        div { class: "field-error", "{shared_error}" }
                    }
                }
                Input {
                    label: loc.field_label("percent-shared"),
                    name: "dna-match-percent".to_owned(),
                    value: draft().percent_shared.clone(),
                    oninput: move |event: FormEvent| draft.write().percent_shared = event.value(),
                }
                Input {
                    label: loc.field_label("largest-segment"),
                    name: "dna-match-largest".to_owned(),
                    value: draft().largest_segment_cm.clone(),
                    oninput: move |event: FormEvent| draft.write().largest_segment_cm = event.value(),
                }
                Input {
                    label: loc.field_label("segment-count"),
                    name: "dna-match-segments".to_owned(),
                    value: draft().segment_count.clone(),
                    oninput: move |event: FormEvent| draft.write().segment_count = event.value(),
                }
            }
        }
    }
}

/// Which DNA-match edit form (if any) the side panel is showing. A `Some(row)` seeds the form from an
/// existing row so Save supersedes it (a per-row correction); `None` would add a fresh one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnaMatchEditForm {
    /// Add/supersede a shared segment — `Some(row)` edits (supersedes) an existing one.
    Segment(Option<DnaSegmentVm>),
    /// Assert/supersede an inferred shared ancestor — `Some(row)` edits (supersedes) an existing one.
    Ancestor(Option<SharedAncestorVm>),
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
    let nav = use_context::<NavState>();
    let mut label_nav = nav;
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<DnaMatchEditForm>);
    let mut retract = use_signal(|| None::<(String, String, bool)>);
    let mut retract_reason = use_signal(String::new);
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

    // The shared whole-record edit state, seeded from the loaded match (empty until it loads); it
    // reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::DnaMatchDetail(detail))) => {
            genealogy_ui::DnaMatchDraft::from_detail(detail)
        }
        _ => genealogy_ui::DnaMatchDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::DnaMatchDraft>(&seed);

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the match's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::DnaMatchDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::DnaMatches,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (DnaMatchEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_dna_match_edit(services, edit, prov).await {
                Ok(_) => {
                    editing_for_submit.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    // A per-row Retract/Detach opens the shared retract panel; confirming dispatches an
    // `UndoAssertion` carrying the typed rationale (the retract note stays in History — ADR 0004 §2).
    let on_retract = use_callback(move |target: (String, String, bool)| {
        retract_reason.set(String::new());
        retract.set(Some(target));
    });
    let mut editing_for_open = editing;
    let on_edit_open = use_callback(move |form: DnaMatchEditForm| editing_for_open.set(Some(form)));
    let retract_services = state.services().clone();
    let retract_human = human_id.clone();
    let retract_saved = saved_label.clone();
    let on_retract_confirm = use_callback(move |()| {
        let Some((assertion_id, _, _)) = retract() else {
            return;
        };
        let services = retract_services.clone();
        let human_id = retract_human.clone();
        let saved = retract_saved.clone();
        let prov = ProvenanceDraft {
            rationale: retract_reason(),
            ..ProvenanceDraft::default()
        };
        spawn(async move {
            let edit = DnaMatchEdit::UndoAssertion { human_id, assertion_id };
            match save_dna_match_edit(services, edit, prov).await {
                Ok(_) => {
                    retract.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    let record_services = services.clone();
    let record_nav = nav;
    let current_id = human_id.clone();
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::DnaMatchDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_dna_match_edit).await;
            finish_record_save(
                effective,
                Category::DnaMatches,
                &current,
                record_nav,
                reload,
                toast,
                &saved,
            );
        });
    });

    let body = match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::DnaMatchDetail(detail))) => dna_match_detail(
            &state,
            detail,
            DnaMatchPane {
                active,
                side_edit: editing,
                record,
                retract,
                retract_reason,
            },
            DnaMatchCallbacks {
                on_submit,
                on_record_save,
                on_retract,
                on_retract_confirm,
                on_edit_open,
            },
            &human_id,
        ),
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

/// The signals a DNA match's detail threads to its tabs: the active tab, the collection-row side
/// panel, and the whole-record edit state.
#[derive(Clone, Copy)]
struct DnaMatchPane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<DnaMatchEditForm>>,
    /// The whole-record edit state (id · status editable; observations locked).
    record: RecordEditState<genealogy_ui::DnaMatchDraft>,
    /// The row being retracted/detached, if the retract panel is open: `(assertion_id, label, detach)`.
    retract: Signal<Option<(String, String, bool)>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The commit callbacks a DNA match's detail wires in: one-command collection edits, the whole-record
/// save (the scalar edit via `edits_against`), and the per-row correction (edit-open + retract-confirm).
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "event-handler fields conventionally share the on_ prefix"
)]
struct DnaMatchCallbacks {
    /// Commits one [`DnaMatchEdit`] command (a collection row).
    on_submit: Callback<(DnaMatchEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::DnaMatchDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<DnaMatchEditForm>,
}

/// Renders a loaded DNA match's detail container: header (with the sticky-header record Edit/Cancel/
/// Save), the tab strip, the active tab, and the collection-row side panel.
fn dna_match_detail(
    state: &AppState,
    detail: &DnaMatchDetail,
    pane: DnaMatchPane,
    callbacks: DnaMatchCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let DnaMatchPane {
        active,
        side_edit: editing,
        record,
        retract,
        retract_reason,
    } = pane;
    let on_submit = callbacks.on_submit;
    let on_record_save = callbacks.on_record_save;
    let on_retract = callbacks.on_retract;
    let on_retract_confirm = callbacks.on_retract_confirm;
    let on_edit_open = callbacks.on_edit_open;
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
    let labels = RecordActionLabels::resolve(loc);
    let status_actions = dna_match_status_actions(loc, on_submit, human_id);
    rsx! {
        div { class: "record-pane", tabindex: "-1", onkeydown: move |event| record_keydown(&event, record, on_record_save),
            DetailContainer {
                title: detail.title.clone(),
                id_label: Some(detail.human_id.clone()),
                avatar: "🔗".to_owned(),
                extras: dna_match_restriction_toggles(loc, detail, on_submit, human_id),
                actions: record_head_actions(&labels, record, status_actions, on_record_save),
                tabs: tab_items,
                active,
                {dna_match_tab_content(state, detail, active_id, editing, record, on_submit, on_retract, on_edit_open, human_id)}
            }
            {dna_match_edit_panel(state, editing, on_submit, human_id)}
            {dna_match_retract_panel(loc, retract, retract_reason, on_retract_confirm)}
        }
    }
}

/// Renders the shared Retract/Detach side panel when a DNA-match collection row's action is armed.
/// Reads the armed `(assertion_id, label, detach)` and binds the rationale input; confirming dispatches
/// `UndoAssertion`. Closed (rendered empty) when nothing is armed. Never renders the target's
/// `AssertionId`.
fn dna_match_retract_panel(
    loc: &Localizer,
    mut retract: Signal<Option<(String, String, bool)>>,
    reason: Signal<String>,
    on_confirm: Callback<()>,
) -> Element {
    let Some((_, label, detach)) = retract() else {
        return rsx! {};
    };
    let (title_id, button_id, note, accessible) = if detach {
        (
            "detach",
            "detach",
            loc.action_title("detach-note"),
            loc.action_detach_row(&label),
        )
    } else {
        ("retract", "retract", loc.retract_note(), loc.action_retract_row(&label))
    };
    rsx! {
        SidePanel {
            title: loc.panel_title(title_id),
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |_| retract.set(None),
            footer: rsx! {},
            {retract_panel(loc, &loc.panel_title(title_id), &label, accessible, &note, loc.action_label(button_id), reason, on_confirm)}
        }
    }
}

/// The header's Confirm / Reject quick actions (the mockup's audited `MatchConfirmed` /
/// `MatchRejected` shortcuts): each commits one `SetStatus` without entering the edit session.
fn dna_match_status_actions(
    loc: &Localizer,
    on_submit: Callback<(DnaMatchEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id_confirm = human_id.to_owned();
    let human_id_reject = human_id.to_owned();
    rsx! {
        Button {
            label: loc.action_label("confirm"),
            small: true,
            onclick: move |_| {
                on_submit
                    .call((
                        DnaMatchEdit::SetStatus {
                            human_id: human_id_confirm.clone(),
                            confirmed: true,
                        },
                        ProvenanceDraft::default(),
                    ));
            },
        }
        Button {
            label: loc.action_label("reject"),
            small: true,
            onclick: move |_| {
                on_submit
                    .call((
                        DnaMatchEdit::SetStatus {
                            human_id: human_id_reject.clone(),
                            confirmed: false,
                        },
                        ProvenanceDraft::default(),
                    ));
            },
        }
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

/// The content of one DNA-match detail tab, with its contextual add/edit affordances.
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threads the pane's signals + callbacks"
)]
fn dna_match_tab_content(
    state: &AppState,
    detail: &DnaMatchDetail,
    tab_id: &str,
    mut editing: Signal<Option<DnaMatchEditForm>>,
    record: RecordEditState<genealogy_ui::DnaMatchDraft>,
    on_submit: Callback<(DnaMatchEdit, ProvenanceDraft)>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<DnaMatchEditForm>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "segments" => dna_match_segments_table(loc, &detail.segments, on_edit_open, on_retract),
        "ancestors" => dna_match_ancestors_table(loc, &detail.shared_ancestors, on_edit_open, on_retract),
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(DnaMatchEditForm::Note)) }
            }
            {id_list(loc, &detail.notes, Some(on_retract))}
        },
        "tags" => dna_match_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => dna_match_history_tab(loc, detail, on_submit, human_id),
        _ => dna_match_overview(loc, detail, record),
    }
}

/// The DNA-match Overview, read-first (`record-editing.html` §1/§2): the match's scalar record (id,
/// the locked observed totals, and the editable confirmation status) plus the inferred-relationship
/// conclusion. Entering edit mode (via the sticky-header Edit) swaps the record fields to inputs (the
/// status becomes a select, the observations stay locked) and, while dirty, shows the provenance block.
pub fn dna_match_overview(
    loc: &Localizer,
    detail: &DnaMatchDetail,
    record: RecordEditState<genealogy_ui::DnaMatchDraft>,
) -> Element {
    if record.editing.read().to_owned() {
        return rsx! {
            div { class: "section-note", "{loc.dna_match_overview_note()}" }
            {dna_match_record_fields(loc, detail, record)}
            {record_edit_provenance(loc, record)}
        };
    }
    let dash = "—".to_owned();
    rsx! {
        div { class: "section-note", "{loc.dna_match_overview_note()}" }
        div { class: "grid-2",
            {dna_match_record_fields(loc, detail, record)}
            Card { title: loc.section_label("inferred-relationship"),
                div { class: "section-note", style: "margin:0 0 8px", "{loc.dna_match_overview_note()}" }
                div { class: "fact-row",
                    span { class: "grow", {detail.predicted_relationship.clone().unwrap_or_else(|| dash.clone())} }
                    Chip { label: detail.status.clone() }
                }
            }
        }
    }
}

/// The DNA-match Segments tab: one row per matching segment (chr/start/end/cM/SNPs/side), plus a
/// per-row Edit (supersedes via [`DnaMatchEdit::AddSegment`]) and Retract (retracts the segment
/// assertion — it stays in History). Never renders the segment's `AssertionId`.
pub fn dna_match_segments_table(
    loc: &Localizer,
    segments: &[DnaSegmentVm],
    onedit: Callback<DnaMatchEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    let add = rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-segment"), variant: ButtonVariant::Default, onclick: move |_| onedit.call(DnaMatchEditForm::Segment(None)) }
        }
    };
    if segments.is_empty() {
        return rsx! {
            {add}
            div { class: "section-note", "{loc.dna_match_segments_note()}" }
            EmptyState { message: loc.tab_empty() }
        };
    }
    let dash = "—".to_owned();
    rsx! {
        {add}
        div { class: "section-note", "{loc.dna_match_segments_note()}" }
        Table {
            headers: vec![
                loc.field_label("chromosome"),
                loc.field_label("start"),
                loc.field_label("end"),
                loc.field_label("centimorgans"),
                loc.field_label("snps"),
                loc.field_label("side"),
                String::new(),
            ],
            for segment in segments.iter() {
                tr {
                    td { "{segment.chromosome}" }
                    td { class: "mono", "{segment.start}" }
                    td { class: "mono", "{segment.end}" }
                    td { b { "{segment.centimorgans}" } }
                    td { {segment.snps.clone().unwrap_or_else(|| dash.clone())} }
                    td { Chip { label: segment.side.clone() } }
                    {row_actions_cell(
                        loc,
                        &segment.chromosome,
                        Some((DnaMatchEditForm::Segment(Some(segment.clone())), None)), None,
                        Some(RowRetract { assertion_id: segment.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
                }
            }
        }
    }
}

/// The DNA-match Shared ancestors tab: one row per inferred common ancestor (name + note), plus a
/// per-row Edit (supersedes via [`DnaMatchEdit::AssertSharedAncestor`]) and Retract (retracts the
/// assertion — it stays in History). Never renders the ancestor's `AssertionId`.
pub fn dna_match_ancestors_table(
    loc: &Localizer,
    ancestors: &[SharedAncestorVm],
    onedit: Callback<DnaMatchEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    let add = rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-shared-ancestor"), variant: ButtonVariant::Default, onclick: move |_| onedit.call(DnaMatchEditForm::Ancestor(None)) }
        }
    };
    if ancestors.is_empty() {
        return rsx! {
            {add}
            div { class: "section-note", "{loc.dna_match_ancestors_note()}" }
            EmptyState { message: loc.tab_empty() }
        };
    }
    let dash = "—".to_owned();
    rsx! {
        {add}
        div { class: "section-note", "{loc.dna_match_ancestors_note()}" }
        Table {
            headers: vec![loc.field_label("ancestor"), loc.field_label("note"), String::new()],
            for ancestor in ancestors.iter() {
                {
                    let label = ancestor
                        .person
                        .as_ref()
                        .map(|p| p.label.clone())
                        .or_else(|| ancestor.note.clone())
                        .unwrap_or_else(|| dash.clone());
                    rsx! {
                        tr {
                            td { {ancestor.person.as_ref().map_or_else(|| dash.clone(), |p| p.label.clone())} }
                            td { class: "muted", {ancestor.note.clone().unwrap_or_else(|| dash.clone())} }
                            {row_actions_cell(
                                loc,
                                &label,
                                Some((DnaMatchEditForm::Ancestor(Some(ancestor.clone())), None)), None,
                                Some(RowRetract { assertion_id: ancestor.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                                Some(onedit),
                                onretract)}
                        }
                    }
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
    let title = match &form {
        DnaMatchEditForm::Segment(Some(_)) => loc.panel_title("edit-segment"),
        DnaMatchEditForm::Segment(None) => loc.panel_title("add-segment"),
        DnaMatchEditForm::Ancestor(Some(_)) => loc.panel_title("edit-ancestor"),
        DnaMatchEditForm::Ancestor(None) => loc.panel_title("add-ancestor"),
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
                DnaMatchEditForm::Segment(seed) => rsx! { DnaMatchSegmentForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                DnaMatchEditForm::Ancestor(seed) => rsx! { DnaMatchAncestorForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                DnaMatchEditForm::Note => rsx! { DnaMatchNoteForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                DnaMatchEditForm::Tag => rsx! { DnaMatchTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "edit segment" form → [`DnaMatchEdit::AddSegment`]. `seed: Some(row)` pre-fills the chromosome,
/// positions, length, SNPs, and side from the row and seeds the draft's `supersedes` with the row's
/// assertion id so Save supersedes (replaces) rather than appends (ADR 0004 §2). A blank required
/// field or an unparseable numeric makes Save a no-op (never zero-filled).
#[component]
fn DnaMatchSegmentForm(
    human_id: String,
    seed: Option<DnaSegmentVm>,
    onsubmit: EventHandler<(DnaMatchEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut chromosome = use_signal(|| seed.as_ref().map(|s| s.chromosome.clone()).unwrap_or_default());
    let mut start = use_signal(|| seed.as_ref().map(|s| s.start.clone()).unwrap_or_default());
    let mut end = use_signal(|| seed.as_ref().map(|s| s.end.clone()).unwrap_or_default());
    let mut centimorgans = use_signal(|| seed.as_ref().map(|s| s.centimorgans.clone()).unwrap_or_default());
    let mut snps = use_signal(|| seed.as_ref().and_then(|s| s.snps.clone()).unwrap_or_default());
    let mut side = use_signal(|| seed.as_ref().map_or(ChromosomeSide::Unknown, |s| s.side_kind));
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|s| s.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let sides = [
        ChromosomeSide::Paternal,
        ChromosomeSide::Maternal,
        ChromosomeSide::Unknown,
    ];
    let side_options: Vec<SelectChoice> = sides
        .iter()
        .enumerate()
        .map(|(index, kind)| SelectChoice {
            value: index.to_string(),
            label: loc.chromosome_side_label(*kind),
        })
        .collect();
    let side_selected = sides.iter().position(|kind| *kind == side()).unwrap_or(0).to_string();
    let save_label = loc.action_label("save");
    rsx! {
        Input {
            label: loc.field_label("chromosome"),
            name: "dna-segment-chromosome".to_owned(),
            value: chromosome(),
            oninput: move |event: FormEvent| chromosome.set(event.value()),
        }
        Input {
            label: loc.field_label("start"),
            name: "dna-segment-start".to_owned(),
            value: start(),
            oninput: move |event: FormEvent| start.set(event.value()),
        }
        Input {
            label: loc.field_label("end"),
            name: "dna-segment-end".to_owned(),
            value: end(),
            oninput: move |event: FormEvent| end.set(event.value()),
        }
        Input {
            label: loc.field_label("centimorgans"),
            name: "dna-segment-cm".to_owned(),
            value: centimorgans(),
            oninput: move |event: FormEvent| centimorgans.set(event.value()),
        }
        Input {
            label: loc.field_label("snps"),
            name: "dna-segment-snps".to_owned(),
            value: snps(),
            oninput: move |event: FormEvent| snps.set(event.value()),
        }
        Select {
            label: loc.field_label("side"),
            name: "dna-segment-side".to_owned(),
            value: Some(side_selected),
            options: side_options,
            onchange: move |event: FormEvent| {
                let sides = [ChromosomeSide::Paternal, ChromosomeSide::Maternal, ChromosomeSide::Unknown];
                if let Some(kind) = event.value().parse::<usize>().ok().and_then(|index| sides.get(index)) {
                    side.set(*kind);
                }
            },
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let Some(segment) = build_segment(&chromosome(), &start(), &end(), &centimorgans(), &snps(), side()) else {
                    return;
                };
                onsubmit.call((DnaMatchEdit::AddSegment { human_id: human_id.clone(), segment }, prov()));
            },
        }
    }
}

/// Builds a [`DnaSegment`] from the segment form's raw fields, or `None` when the chromosome is blank
/// or a numeric does not parse (Save is then a no-op — never zero-filled).
fn build_segment(
    chromosome: &str,
    start: &str,
    end: &str,
    centimorgans: &str,
    snps: &str,
    side: ChromosomeSide,
) -> Option<DnaSegment> {
    let chromosome = chromosome.trim();
    if chromosome.is_empty() {
        return None;
    }
    let start = start.trim().parse::<u64>().ok()?;
    let end = end.trim().parse::<u64>().ok()?;
    let centimorgans = centimorgans.trim().parse::<Centimorgans>().ok()?;
    let snps = match snps.trim() {
        "" => None,
        text => Some(text.parse::<u32>().ok()?),
    };
    Some(DnaSegment {
        chromosome: chromosome.to_owned(),
        start,
        end,
        centimorgans,
        snps,
        side,
    })
}

/// The "edit shared ancestor" form → [`DnaMatchEdit::AssertSharedAncestor`]. `seed: Some(row)` pre-fills
/// the note and seeds the draft's `supersedes` with the row's assertion id so Save supersedes rather
/// than appends (ADR 0004 §2). The linked person is preserved (shown read-only, carried by its id); the
/// note is the editable correction.
#[component]
fn DnaMatchAncestorForm(
    human_id: String,
    seed: Option<SharedAncestorVm>,
    onsubmit: EventHandler<(DnaMatchEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let person_id = seed.as_ref().and_then(|s| s.person.as_ref().map(|p| p.id.clone()));
    let person_label = seed.as_ref().and_then(|s| s.person.as_ref().map(|p| p.label.clone()));
    let mut note = use_signal(|| seed.as_ref().and_then(|s| s.note.clone()).unwrap_or_default());
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|s| s.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let save_label = loc.action_label("save");
    rsx! {
        if let Some(name) = person_label {
            div { class: "field",
                label { "{loc.field_label(\"ancestor\")}" }
                div { class: "in", "{name}" }
            }
        }
        Input {
            label: loc.field_label("note"),
            name: "dna-ancestor-note".to_owned(),
            value: note(),
            oninput: move |event: FormEvent| note.set(event.value()),
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                onsubmit.call((
                    DnaMatchEdit::AssertSharedAncestor {
                        human_id: human_id.clone(),
                        person_id: person_id.clone(),
                        note: non_empty(note()),
                    },
                    prov(),
                ));
            },
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
    let services = state.services().clone();
    let picker = use_existing_picker(
        services,
        Category::Notes,
        loc.field_label("note"),
        "note".to_owned(),
        loc.picker_entity(Category::Notes),
        Vec::new(),
    );
    let prov = use_signal(ProvenanceDraft::default);
    let picker_for_save = picker.clone();
    let onsave = use_callback(move |()| {
        let Some(id) = picker_selection_id(&picker_for_save) else {
            return;
        };
        onsubmit.call((
            DnaMatchEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            },
            prov(),
        ));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
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
