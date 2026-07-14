use super::prelude::*;
use crate::screens::RecordDetail;
use genealogy_app::{DnaGenomeBuild, DnaProvider, DnaTestType};
// The haplogroup row view-model seeds the per-row haplogroup edit (supersede by `AssertionId`).
use genealogy_ui::HaplogroupRowVm;

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
    use_record_step(nav, Category::DnaTests, list, query, selected);
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
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::DataQuality(_),
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
    let record = use_record_create::<genealogy_ui::DnaTestDraft>();
    let mut draft = record.draft;
    // The existing-person picker: options load once; pick/clear drive the draft's (required) person id.
    let person_state = use_signal(genealogy_ui::PickerState::default);
    let person_services = services.clone();
    let person_rows = use_resource(move || {
        let services = person_services.clone();
        async move { load_picker_rows(services, Category::People).await }
    });
    let person_onpick = use_callback(move |selection: PickerSelection| draft.write().person = selection.human_id);
    let person_onclear = use_callback(move |()| draft.write().person = String::new());
    let person_onnew = use_callback(move |_query: String| {});
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::DnaTestDraft, ProvenanceDraft)| {
        let Some(request) = draft.to_request() else {
            return;
        };
        let services = services.clone();
        spawn(async move {
            match commit_dna_test_change_set(services, request, prov).await {
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
    let person = RecordPicker {
        config: PickerConfig {
            label: loc.field_label("person"),
            name: "dna-test-person".to_owned(),
            entity_label: loc.picker_entity(Category::People),
            allow_new: false,
        },
        state: person_state,
        options: picker_options(person_rows.read_unchecked().as_ref()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: person_onpick,
            onclear: person_onclear,
            onnew: person_onnew,
        },
    };
    create_record_frame(
        &loc.dna_test_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {dna_test_create_fields(loc, draft, &person)}
            {record_edit_provenance(loc, record)}
        },
    )
}

/// The DNA test's provider / test-type / genome-build selects, factored out of
/// [`dna_test_record_fields`] to keep it under the length cap. Each is an optional-enum
/// [`DraftSelect`] over the [`record_enum_select`] parts.
fn dna_test_select_fields(loc: &Localizer, record: RecordEditState<genealogy_ui::DnaTestDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let providers = dna_provider_choices();
    let test_types = [
        DnaTestType::Autosomal,
        DnaTestType::YDna,
        DnaTestType::MtDna,
        DnaTestType::XDna,
    ];
    let builds = [DnaGenomeBuild::GRCh37, DnaGenomeBuild::GRCh38];
    let (provider_options, provider_value, provider_original) = record_enum_select(
        loc.record_unset(),
        &providers,
        draft().provider.as_ref(),
        seed.read().provider.as_ref(),
        |provider| loc.dna_provider_label(provider),
    );
    let (type_options, type_value, type_original) = record_enum_select(
        loc.record_unset(),
        &test_types,
        draft().test_type.as_ref(),
        seed.read().test_type.as_ref(),
        |test_type| loc.dna_test_type_label(*test_type),
    );
    let (build_options, build_value, build_original) = record_enum_select(
        loc.record_unset(),
        &builds,
        draft().genome_build.as_ref(),
        seed.read().genome_build.as_ref(),
        |build| loc.dna_genome_build_label(*build),
    );
    rsx! {
        DraftSelect {
            label: loc.field_label("provider"),
            name: "dna-test-provider".to_owned(),
            editing,
            value: provider_value,
            original: provider_original,
            reset_label: loc.action_reset_field(&loc.field_label("provider")),
            options: provider_options,
            onchange: move |value: String| {
                let providers = dna_provider_choices();
                draft.write().provider = value.parse::<usize>().ok().and_then(|index| providers.get(index).cloned());
            },
            onreset: move |()| {
                let value = seed.read().provider.clone();
                draft.write().provider = value;
            },
        }
        DraftSelect {
            label: loc.field_label("test-type"),
            name: "dna-test-type".to_owned(),
            editing,
            value: type_value,
            original: type_original,
            reset_label: loc.action_reset_field(&loc.field_label("test-type")),
            options: type_options,
            onchange: move |value: String| {
                let test_types = [DnaTestType::Autosomal, DnaTestType::YDna, DnaTestType::MtDna, DnaTestType::XDna];
                draft.write().test_type = value.parse::<usize>().ok().and_then(|index| test_types.get(index).copied());
            },
            onreset: move |()| {
                let value = seed.read().test_type;
                draft.write().test_type = value;
            },
        }
        DraftSelect {
            label: loc.field_label("genome-build"),
            name: "dna-test-genome-build".to_owned(),
            editing,
            value: build_value,
            original: build_original,
            reset_label: loc.action_reset_field(&loc.field_label("genome-build")),
            options: build_options,
            onchange: move |value: String| {
                let builds = [DnaGenomeBuild::GRCh37, DnaGenomeBuild::GRCh38];
                draft.write().genome_build = value.parse::<usize>().ok().and_then(|index| builds.get(index).copied());
            },
            onreset: move |()| {
                let value = seed.read().genome_build;
                draft.write().genome_build = value;
            },
        }
    }
}

/// The DNA test's scalar record fields (id · person · provider · type · genome build · kit id),
/// read-first (`record-editing.html` §2/§3). The anchoring person is locked (§3, disabled — it is set
/// at creation). A pure fn (the edit state's signals passed in) so the SSR tests render it without
/// `AppCtx`.
pub fn dna_test_record_fields(loc: &Localizer, record: RecordEditState<genealogy_ui::DnaTestDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let current = draft();
    let committed = seed.read().clone();
    rsx! {
        Card { title: loc.section_label("kit"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "dna-test-id".to_owned(),
                    editing,
                    value: current.human_id.clone(),
                    original: committed.human_id.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("id")),
                    mono: true,
                    hint: Some(loc.field_human_id_hint()),
                    oninput: move |value: String| draft.write().human_id = value,
                    onreset: move |()| {
                        let value = seed.read().human_id.clone();
                        draft.write().human_id = value;
                    },
                }
                DraftText {
                    label: loc.field_label("person"),
                    name: "dna-test-person".to_owned(),
                    editing,
                    value: current.person.clone(),
                    original: committed.person.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("person")),
                    mono: true,
                    locked: true,
                    oninput: move |_: String| {},
                    onreset: move |()| {},
                }
                {dna_test_select_fields(loc, record)}
                DraftText {
                    label: loc.field_label("kit-id"),
                    name: "dna-test-kit-id".to_owned(),
                    editing,
                    value: current.kit_id.clone(),
                    original: committed.kit_id.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("kit-id")),
                    oninput: move |value: String| draft.write().kit_id = value,
                    onreset: move |()| {
                        let value = seed.read().kit_id.clone();
                        draft.write().kit_id = value;
                    },
                }
            }
        }
    }
}

/// The DNA-test create form's field rows (`dna-test.html` edit specimen): a required Person picker
/// (existing-only; a required-field error while unpicked, §7), then Provider · Test type · Genome build
/// · Kit id. A pure fn (the picker's state/options/callbacks passed in) so SSR tests render it.
pub fn dna_test_create_fields(
    loc: &Localizer,
    mut draft: Signal<genealogy_ui::DnaTestDraft>,
    person: &RecordPicker,
) -> Element {
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
                {record_picker(loc, person)}
                if person_invalid {
                    div { class: "field-error", "{person_error}" }
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

/// Which DNA-test collection-row edit form (if any) the side panel is showing. The test's own scalar
/// record (id · person · provider · type · genome build · kit id) is edited in place via the sticky
/// header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnaTestEditForm {
    /// Assert a haplogroup — `None` adds a new one, `Some(row)` edits (supersedes) an existing one.
    Haplogroup(Option<HaplogroupRowVm>),
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
    let nav = use_context::<NavState>();
    let mut label_nav = nav;
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<DnaTestEditForm>);
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
        async move { load_screen(services, Intent::ShowDnaTest { human_id }).await }
    });

    // The shared whole-record edit state, seeded from the loaded test (empty until it loads); it
    // reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::DnaTestDetail(detail))) => {
            genealogy_ui::DnaTestDraft::from_detail(detail)
        }
        _ => genealogy_ui::DnaTestDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::DnaTestDraft>(&seed);

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the test's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::DnaTestDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::DnaTests,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (DnaTestEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_dna_test_edit(services, edit, prov).await {
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
    let on_edit_open = use_callback(move |form: DnaTestEditForm| editing_for_open.set(Some(form)));
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
            let edit = DnaTestEdit::UndoAssertion { human_id, assertion_id };
            match save_dna_test_edit(services, edit, prov).await {
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
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::DnaTestDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_dna_test_edit).await;
            finish_record_save(
                effective,
                Category::DnaTests,
                &current,
                record_nav,
                reload,
                toast,
                &saved,
            );
        });
    });

    // ⌘Z retracts the newest undoable assertion of this record's loaded change log (WP5).
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::DnaTestDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read() || retract.read().is_some());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let undo_human = human_id.clone();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((
            DnaTestEdit::UndoAssertion {
                human_id: undo_human.clone(),
                assertion_id,
            },
            ProvenanceDraft::default(),
        ));
    });
    use_record_undo(nav, undo_busy, undo_history, undo_notice, on_undo);

    let body = match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::DnaTestDetail(detail))) => dna_test_detail(
            &state,
            detail,
            DnaTestPane {
                active,
                side_edit: editing,
                record,
                retract,
                retract_reason,
            },
            DnaTestCallbacks {
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
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::Pedigree(_)
            | IntentOutcome::Relationship(_)
            | IntentOutcome::DuplicateCandidates(_)
            | IntentOutcome::MergeCompare(_)
            | IntentOutcome::Dashboard(_)
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

/// The signals a DNA test's detail threads to its tabs: the active tab, the collection-row side panel,
/// and the whole-record edit state.
#[derive(Clone, Copy)]
struct DnaTestPane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<DnaTestEditForm>>,
    /// The whole-record edit state (id · person · provider · type · genome build · kit id).
    record: RecordEditState<genealogy_ui::DnaTestDraft>,
    /// The row being retracted/detached, if the retract panel is open: `(assertion_id, label, detach)`.
    retract: Signal<Option<(String, String, bool)>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The commit callbacks a DNA test's detail wires in: one-command collection edits, the whole-record
/// save (the scalar edit via `edits_against`), and the per-row correction (edit-open + retract-confirm).
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "event-handler fields conventionally share the on_ prefix"
)]
struct DnaTestCallbacks {
    /// Commits one [`DnaTestEdit`] command (a collection row).
    on_submit: Callback<(DnaTestEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::DnaTestDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<DnaTestEditForm>,
}

/// Renders a loaded DNA test's detail container: header (with the sticky-header record Edit/Cancel/
/// Save), the tab strip, the active tab, and the collection-row side panel.
fn dna_test_detail(
    state: &AppState,
    detail: &DnaTestDetail,
    pane: DnaTestPane,
    callbacks: DnaTestCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let DnaTestPane {
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
    let labels = RecordActionLabels::resolve(loc);
    rsx! {
        div { class: "record-pane", tabindex: "-1", onkeydown: move |event| record_keydown(&event, record, on_record_save),
            DetailContainer {
                title: detail.title.clone(),
                id_label: Some(detail.human_id.clone()),
                avatar: "🧬".to_owned(),
                extras: dna_test_restriction_toggles(loc, detail, on_submit, human_id),
                actions: record_head_actions(&labels, record, rsx! {}, on_record_save),
                tabs: tab_items,
                active,
                {dna_test_tab_content(state, detail, active_id, editing, record, on_submit, on_retract, on_edit_open, human_id)}
            }
            {dna_test_edit_panel(state, editing, on_submit, human_id)}
            {dna_test_retract_panel(loc, retract, retract_reason, on_retract_confirm)}
        }
    }
}

/// Renders the shared Retract/Detach side panel when a DNA-test collection row's action is armed.
/// Reads the armed `(assertion_id, label, detach)` and binds the rationale input; confirming dispatches
/// `UndoAssertion`. Closed (rendered empty) when nothing is armed. Never renders the target's
/// `AssertionId`.
fn dna_test_retract_panel(
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

/// The content of one DNA-test detail tab, with its contextual add/edit affordances.
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threads the pane's signals + callbacks"
)]
fn dna_test_tab_content(
    state: &AppState,
    detail: &DnaTestDetail,
    tab_id: &str,
    mut editing: Signal<Option<DnaTestEditForm>>,
    record: RecordEditState<genealogy_ui::DnaTestDraft>,
    on_submit: Callback<(DnaTestEdit, ProvenanceDraft)>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<DnaTestEditForm>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "haplogroups" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-haplogroup"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(DnaTestEditForm::Haplogroup(None))) }
            }
            {dna_test_haplogroups_table(loc, &detail.haplogroups, on_edit_open, on_retract)}
        },
        "matches" => dna_test_matches_table(loc, &detail.matches),
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(DnaTestEditForm::Note)) }
            }
            {id_list(loc, &detail.notes, Some(on_retract))}
        },
        "tags" => dna_test_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => dna_test_history_tab(loc, detail, on_submit, human_id),
        _ => dna_test_overview(loc, detail, record),
    }
}

/// The DNA-test Overview, read-first (`record-editing.html` §1/§2): the test's scalar record (id ·
/// person · provider · type · genome build · kit id) as read boxes plus the Tested-person and
/// ethnicity cards. Entering edit mode (via the sticky-header Edit) swaps the record fields to inputs
/// and, while dirty, shows the provenance block; the ancillary cards are hidden in edit mode.
pub fn dna_test_overview(
    loc: &Localizer,
    detail: &DnaTestDetail,
    record: RecordEditState<genealogy_ui::DnaTestDraft>,
) -> Element {
    if record.editing.read().to_owned() {
        return rsx! {
            div { class: "section-note", "{loc.dna_test_overview_note()}" }
            {dna_test_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        };
    }
    let dash = "—".to_owned();
    rsx! {
        div { class: "section-note", "{loc.dna_test_overview_note()}" }
        div { class: "grid-2",
            {dna_test_record_fields(loc, record)}
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

/// The DNA-test Haplogroups tab: one row per recorded haplogroup, plus a per-row Edit (supersedes via
/// [`DnaTestEdit::AddHaplogroup`]) and Retract (retracts the haplogroup assertion — it stays in
/// History). Never renders the haplogroup's `AssertionId`.
pub fn dna_test_haplogroups_table(
    loc: &Localizer,
    haplogroups: &[HaplogroupRowVm],
    onedit: Callback<DnaTestEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if haplogroups.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![loc.field_label("haplogroup"), String::new()],
            for haplogroup in haplogroups.iter() {
                tr {
                    td { b { "{haplogroup.value}" } }
                    {row_actions_cell(
                        loc,
                        &haplogroup.value,
                        Some((DnaTestEditForm::Haplogroup(Some(haplogroup.clone())), None)), None,
                        Some(RowRetract { assertion_id: haplogroup.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
                }
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
    let title = match &form {
        DnaTestEditForm::Haplogroup(None) => loc.action_label("add-haplogroup"),
        DnaTestEditForm::Haplogroup(Some(_)) => loc.panel_title("edit-haplogroup"),
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
                DnaTestEditForm::Haplogroup(seed) => rsx! { DnaTestHaplogroupForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                DnaTestEditForm::Note => rsx! { DnaTestNoteForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                DnaTestEditForm::Tag => rsx! { DnaTestTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "add haplogroup" form → [`DnaTestEdit::AddHaplogroup`]. `seed: None` adds a new haplogroup;
/// `Some(row)` edits an existing one — the value is pre-filled and the draft's `supersedes` is seeded
/// with the row's assertion id so Save supersedes (replaces) rather than appends (ADR 0004 §2).
#[component]
fn DnaTestHaplogroupForm(
    human_id: String,
    seed: Option<HaplogroupRowVm>,
    onsubmit: EventHandler<(DnaTestEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut value = use_signal(|| seed.as_ref().map(|row| row.value.clone()).unwrap_or_default());
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|row| row.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let save_label = loc.action_label("save");
    rsx! {
        Input {
            label: loc.field_label("haplogroup"),
            name: "haplogroup".to_owned(),
            value: value(),
            oninput: move |event: FormEvent| value.set(event.value()),
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let value = value();
                if value.trim().is_empty() {
                    return;
                }
                onsubmit.call((DnaTestEdit::AddHaplogroup { human_id: human_id.clone(), haplogroup: value }, prov()));
            },
        }
    }
}

/// The "attach note" form: an existing-note picker → [`DnaTestEdit::AttachNote`].
#[component]
fn DnaTestNoteForm(human_id: String, onsubmit: EventHandler<(DnaTestEdit, ProvenanceDraft)>) -> Element {
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
            DnaTestEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            },
            prov(),
        ));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
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
