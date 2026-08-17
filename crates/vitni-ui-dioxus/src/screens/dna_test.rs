use super::prelude::*;
use vitni_app::{DnaGenomeBuild, DnaProvider, DnaTestType};
// The haplogroup row view-model seeds the per-row haplogroup edit (supersede by `AssertionId`).
use vitni_ui::HaplogroupRowVm;

/// The create-mode DNA-test record: an uncommitted [`DnaTestDraft`] rendered as the create form in
/// the detail pane (`record-editing.html` §6). The person is required (§7); Save commits the whole
/// test; Cancel discards.
#[component]
pub fn DnaTestCreateRecord(draft_id: DraftId) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<vitni_ui::DnaTestDraft>(Category::DnaTests, draft_id);
    let mut draft = record.draft;
    // The existing-person picker: its options refetch after any mutation (#266); pick/clear drive the
    // draft's (required) person id.
    let person_state = use_signal(vitni_ui::PickerState::default);
    let person_services = services.clone();
    let person_rows = use_resource(move || {
        let _ = data_version_ticket(Some(nav));
        let services = person_services.clone();
        async move { load_picker_rows(services, Category::People).await }
    });
    let person_onpick = use_callback(move |selection: PickerSelection| draft.write().person = selection.human_id);
    let person_onclear = use_callback(move |()| draft.write().person = String::new());
    let person_onnew = use_callback(move |_query: String| {});
    let created_label = loc.action_label(ActionLabel::Created);
    let on_save = use_callback(move |(draft, prov): (vitni_ui::DnaTestDraft, ProvenanceDraft)| {
        let Some(request) = draft.to_request() else {
            return;
        };
        let services = services.clone();
        let created = created_label.clone();
        spawn(async move {
            let committed = commit_dna_test_change_set(services, request, prov).await;
            finish_draft_commit(
                committed,
                DraftCommit::new(Category::DnaTests, draft_id, &draft, created),
                nav,
            );
        });
    });
    // The close/quit confirm's Save runs this same commit (issue #240), so a ⌘W/⌘Q over a half-filled
    // create form can keep the draft instead of losing it.
    let save_now = use_callback(move |()| {
        if record.can_save() {
            on_save.call((record.draft.read().clone(), record.prov.read().clone()));
        }
    });
    use_save_on_request(EditKey::draft(Category::DnaTests, draft_id), record, save_now);
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_button(ActionLabel::Cancel), variant: ButtonVariant::Ghost, small: true, onclick: move |_| nav.cancel_draft(draft_id) }
        Button {
            label: loc.action_button(ActionLabel::Save),
            variant: ButtonVariant::Primary,
            small: true,
            disabled: !can_save,
            onclick: move |_| save_now.call(()),
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
fn dna_test_select_fields(loc: &Localizer, record: RecordEditState<vitni_ui::DnaTestDraft>) -> Element {
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
pub fn dna_test_record_fields(loc: &Localizer, record: RecordEditState<vitni_ui::DnaTestDraft>) -> Element {
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
                {record_restrictions_field(loc, record)}
            }
        }
    }
}

/// The DNA-test create form's field rows (`dna-test.html` edit specimen): a required Person picker
/// (existing-only; a required-field error while unpicked, §7), then Provider · Test type · Genome build
/// · Kit id. A pure fn (the picker's state/options/callbacks passed in) so SSR tests render it.
pub fn dna_test_create_fields(
    loc: &Localizer,
    mut draft: Signal<vitni_ui::DnaTestDraft>,
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
    let active = use_detail_tab(Category::DnaTests, &human_id);
    let editing = use_signal(|| None::<DnaTestEditForm>);
    // The shared commit path (`screens/detail_commits.rs`): the reload counter, the retract panel's
    // state, and the five callbacks every detail pane dispatches through.
    let DetailCommits {
        reload,
        retract,
        retract_reason,
        on_submit,
        on_undo,
        on_tag_remove,
        on_retract,
        on_retract_confirm,
    } = use_detail_commits::<DnaTestCommits, DnaTestEditForm>(&state, &human_id, editing);
    let saved_label = state.data_loc().action_label(ActionLabel::Saved);

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
        Some(ScreenData::Loaded(IntentOutcome::DnaTestDetail(detail))) => vitni_ui::DnaTestDraft::from_detail(detail),
        _ => vitni_ui::DnaTestDraft::new(),
    };
    let record = use_record_edit::<vitni_ui::DnaTestDraft>(Category::DnaTests, &human_id, &seed);

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
            vitni_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let mut editing_for_open = editing;
    let on_edit_open = use_callback(move |form: DnaTestEditForm| editing_for_open.set(Some(form)));

    let record_services = services.clone();
    let record_nav = nav;
    let current_id = human_id.clone();
    let on_record_save = use_callback(move |(draft, prov): (vitni_ui::DnaTestDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_dna_test_edit).await;
            finish_record_save(effective, Category::DnaTests, &current, record_nav, reload, &saved);
        });
    });

    // ⌘Z retracts the newest undoable assertion of this record's loaded change log (WP5).
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::DnaTestDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read() || retract.read().is_some());
    let undo_notice = chrome.kbd_nothing_to_undo();
    use_record_undo(
        nav,
        Category::DnaTests,
        &human_id,
        undo_busy,
        undo_history,
        undo_notice,
        on_undo,
    );

    // The close/quit confirm's Save hands the record back to this pane (issue #240): it runs the same
    // whole-record commit the header's Save does, so ⌘W/⌘Q can keep the edit instead of discarding it.
    let save_now = use_callback(move |()| {
        if record.can_save() {
            on_record_save.call((record.draft.read().clone(), record.prov.read().clone()));
        }
    });
    use_save_on_request(EditKey::saved(Category::DnaTests, &human_id), record, save_now);

    match &*data.read_unchecked() {
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
                on_undo,
                on_tag_remove,
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
            | IntentOutcome::Geography(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::ResearchNoteDetail(_)
            | IntentOutcome::DataQuality(_),
        )) => rsx! {},
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
    record: RecordEditState<vitni_ui::DnaTestDraft>,
    /// The row being retracted/detached, if the retract panel is open.
    retract: Signal<Option<RetractTarget>>,
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
    on_record_save: Callback<(vitni_ui::DnaTestDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<DnaTestEditForm>,
    /// Retracts an assertion by id from the History tab (dispatches `UndoAssertion`).
    on_undo: Callback<String>,
    /// Arms the untag panel for a tag chip's ×: `(tag_id, tag name)`.
    on_tag_remove: Callback<(String, String)>,
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
    let on_undo = callbacks.on_undo;
    let on_tag_remove = callbacks.on_tag_remove;
    let tabs = dna_test_tabs(detail, loc);
    let tab_items: Vec<TabItem> = tabs
        .iter()
        .map(|tab| TabItem {
            id: tab.id.to_owned(),
            label: tab.label.clone(),
            count: tab.count,
        })
        .collect();
    let active_tab = tabs.get(active()).cloned().unwrap_or_else(|| fallback_tab("overview"));
    let labels = RecordActionLabels::resolve(loc);
    rsx! {
        div { class: "record-pane", tabindex: "-1", onkeydown: move |event| record_keydown(&event, record),
            DetailContainer {
                title: detail.title.clone(),
                id_label: Some(detail.human_id.clone()),
                avatar: "🧬".to_owned(),
                extras: restriction_display(loc, &detail.restrictions),
                actions: record_head_actions(&labels, record, rsx! {}, on_record_save),
                tabs: tab_items,
                active,
                {dna_test_tab_content(state, detail, &active_tab, editing, record, on_retract, on_edit_open, on_undo, on_tag_remove)}
            }
            {dna_test_edit_panel(state, editing, on_submit, human_id)}
            {retract_side_panel(loc, retract, retract_reason, on_retract_confirm, "detach-note")}
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
    tab: &DetailTab,
    editing: Signal<Option<DnaTestEditForm>>,
    record: RecordEditState<vitni_ui::DnaTestDraft>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<DnaTestEditForm>,
    on_undo: Callback<String>,
    on_tag_remove: Callback<(String, String)>,
) -> Element {
    let loc = state.data_loc();
    match tab.id {
        "haplogroups" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, DnaTestEditForm::Haplogroup(None)),
            None,
            rsx! {
                {dna_test_haplogroups_table(loc, &detail.haplogroups, on_edit_open, on_retract)}
            },
        ),
        "matches" => dna_test_matches_table(loc, &detail.matches),
        "notes" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, DnaTestEditForm::Note),
            None,
            rsx! {
                {id_list(loc, &detail.notes, Some(on_retract))}
            },
        ),
        "tags" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, DnaTestEditForm::Tag),
            Some(TabActionStyle {
                emphasis: Some(ButtonVariant::Ghost),
                ..Default::default()
            }),
            tags_panel(loc, &detail.tags, on_tag_remove),
        ),
        "history" => history_panel(loc, &detail.history, Some(on_undo)),
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
    record: RecordEditState<vitni_ui::DnaTestDraft>,
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
            caption: loc.tab_label("haplogroups"),
            headers: vec![loc.field_label("haplogroup"), String::new()],
            for haplogroup in haplogroups.iter() {
                tr {
                    td { b { "{haplogroup.value}" } }
                    {row_actions_cell(
                        loc,
                        &haplogroup.value,
                        Some((DnaTestEditForm::Haplogroup(Some(haplogroup.clone())), None)), None,
                        Some(RowRetract { assertion_id: haplogroup.assertion_id.clone(), button_label: RowVerb::Retract, title: "retract", detach: false }),
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
            caption: loc.tab_label("matches"),
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
        DnaTestEditForm::Haplogroup(None) => loc.action_label(ActionLabel::AddHaplogroup),
        DnaTestEditForm::Haplogroup(Some(_)) => loc.panel_title("edit-haplogroup"),
        DnaTestEditForm::Note => loc.action_label(ActionLabel::AttachNote),
        DnaTestEditForm::Tag => loc.action_label(ActionLabel::AddTag),
    };
    let human_id = human_id.to_owned();
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: loc.action_label(ActionLabel::Cancel),
            onclose: move |()| editing.set(None),
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
    let save_label = loc.action_button(ActionLabel::Save);
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
    let attach = use_attach_picker(
        services.clone(),
        Category::Notes,
        loc.field_label("note"),
        "note".to_owned(),
        loc.picker_entity(Category::Notes),
        Vec::new(),
    );
    let prov = use_signal(ProvenanceDraft::default);
    let onattach = use_callback(move |id: String| {
        onsubmit.call((
            DnaTestEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            },
            prov(),
        ));
    });
    let onsave = use_attach_save(services, &attach, prov, onattach);
    attach_link_form(loc, &attach, rsx! {}, prov, onsave)
}

/// The DNA-test "Add tag" form: a picker of existing tags by name → [`DnaTestEdit::Tag`].
#[component]
fn DnaTestTagForm(human_id: String, onsubmit: EventHandler<(DnaTestEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loc = state.data_loc();
    let save_label = loc.action_button(ActionLabel::Save);
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
