use super::prelude::*;
// The citation attribute row view-model (seeds the per-row attribute edit) and the record-link
// view-model enum (citation source); the latter shadows the prelude's `RecordLink` link component,
// which this screen does not use.
use genealogy_ui::{CitationAttributeVm, RecordLink};

/// The create-mode citation record: an uncommitted [`CitationDraft`] rendered as the create form in
/// the detail pane (`record-editing.html` §6). The source is required (§7); a "new source" selection
/// creates a source inline on Save (§6b). Save commits the whole citation; Cancel discards.
#[component]
pub fn CitationCreateRecord() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::CitationDraft>();
    let mut draft = record.draft;
    // The find-or-create source picker: options load once; pick/clear/"+ New" drive the draft's link.
    let source_state = use_signal(genealogy_ui::PickerState::default);
    let source_services = services.clone();
    let source_rows = use_resource(move || {
        let services = source_services.clone();
        async move { load_picker_rows(services, Category::Sources).await }
    });
    let source_onpick =
        use_callback(move |selection: PickerSelection| draft.write().source = RecordLink::Existing(selection));
    let source_onclear = use_callback(move |()| draft.write().source = RecordLink::Empty);
    let source_onnew =
        use_callback(move |_query: String| draft.write().source = RecordLink::New(NewSourceFields::default()));
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::CitationDraft, ProvenanceDraft)| {
        let Some(request) = draft.to_request() else {
            return;
        };
        let services = services.clone();
        spawn(async move {
            match commit_citation_change_set(services, request, prov).await {
                Ok(id) => nav.commit_draft(RecordRef {
                    category: Category::Citations,
                    human_id: id.clone(),
                    label: id,
                }),
                Err(message) => nav.notify(message),
            }
        });
    });
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| nav.cancel_draft(Category::Citations) }
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
    let source = RecordPicker {
        config: PickerConfig {
            label: loc.field_label("source"),
            name: "citation-source".to_owned(),
            entity_label: loc.picker_entity(Category::Sources),
            allow_new: true,
        },
        state: source_state,
        options: picker_options(source_rows.read_unchecked().as_ref()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: source_onpick,
            onclear: source_onclear,
            onnew: source_onnew,
        },
    };
    create_record_frame(
        &loc.citation_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {citation_create_fields(loc, draft, &source)}
            {record_edit_provenance(loc, record)}
        },
    )
}

/// The citation's evidence record fields (confidence + the three Evidence Explained axes), factored out
/// of [`citation_record_fields`] to stay under the length cap. Each is an optional-enum [`DraftSelect`]
/// over the [`record_enum_select`] parts; the three axes are the citation's own analysis.
fn citation_evidence_record_fields(loc: &Localizer, record: RecordEditState<genealogy_ui::CitationDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let (confidence_options, confidence_value, confidence_original) = record_enum_select(
        loc.record_unset(),
        &ConfidenceLevel::all(),
        draft().confidence.as_ref(),
        seed.read().confidence.as_ref(),
        |level| loc.confidence_label(*level),
    );
    let (source_options, source_value, source_original) = record_enum_select(
        loc.record_unset(),
        &genealogy_ui::SOURCE_QUALITIES,
        draft().source_quality.as_ref(),
        seed.read().source_quality.as_ref(),
        |value| loc.evidence_source_label(*value),
    );
    let (info_options, info_value, info_original) = record_enum_select(
        loc.record_unset(),
        &genealogy_ui::INFORMATION_KINDS,
        draft().information.as_ref(),
        seed.read().information.as_ref(),
        |value| loc.evidence_information_label(*value),
    );
    let (kind_options, kind_value, kind_original) = record_enum_select(
        loc.record_unset(),
        &genealogy_ui::EVIDENCE_KINDS,
        draft().evidence_kind.as_ref(),
        seed.read().evidence_kind.as_ref(),
        |value| loc.evidence_kind_label(*value),
    );
    rsx! {
        DraftSelect {
            label: loc.field_label("confidence"),
            name: "citation-confidence".to_owned(),
            editing,
            value: confidence_value,
            original: confidence_original,
            reset_label: loc.action_reset_field(&loc.field_label("confidence")),
            options: confidence_options,
            onchange: move |value: String| {
                let levels = ConfidenceLevel::all();
                draft.write().confidence = value.parse::<usize>().ok().and_then(|index| levels.get(index).copied());
            },
            onreset: move |()| {
                let value = seed.read().confidence;
                draft.write().confidence = value;
            },
        }
        DraftSelect {
            label: loc.evidence_axis_label(genealogy_ui::EvidenceAxis::Source),
            name: "citation-source-quality".to_owned(),
            editing,
            value: source_value,
            original: source_original,
            reset_label: loc.action_reset_field(&loc.evidence_axis_label(genealogy_ui::EvidenceAxis::Source)),
            options: source_options,
            onchange: move |value: String| {
                draft.write().source_quality = value.parse::<usize>().ok().and_then(|index| genealogy_ui::SOURCE_QUALITIES.get(index).copied());
            },
            onreset: move |()| {
                let value = seed.read().source_quality;
                draft.write().source_quality = value;
            },
        }
        DraftSelect {
            label: loc.evidence_axis_label(genealogy_ui::EvidenceAxis::Information),
            name: "citation-information".to_owned(),
            editing,
            value: info_value,
            original: info_original,
            reset_label: loc.action_reset_field(&loc.evidence_axis_label(genealogy_ui::EvidenceAxis::Information)),
            options: info_options,
            onchange: move |value: String| {
                draft.write().information = value.parse::<usize>().ok().and_then(|index| genealogy_ui::INFORMATION_KINDS.get(index).copied());
            },
            onreset: move |()| {
                let value = seed.read().information;
                draft.write().information = value;
            },
        }
        DraftSelect {
            label: loc.evidence_axis_label(genealogy_ui::EvidenceAxis::Evidence),
            name: "citation-evidence-kind".to_owned(),
            editing,
            value: kind_value,
            original: kind_original,
            reset_label: loc.action_reset_field(&loc.evidence_axis_label(genealogy_ui::EvidenceAxis::Evidence)),
            options: kind_options,
            onchange: move |value: String| {
                draft.write().evidence_kind = value.parse::<usize>().ok().and_then(|index| genealogy_ui::EVIDENCE_KINDS.get(index).copied());
            },
            onreset: move |()| {
                let value = seed.read().evidence_kind;
                draft.write().evidence_kind = value;
            },
        }
    }
}

/// The citation's scalar record fields (id · source · date · page · confidence · evidence axes),
/// read-first (`record-editing.html` §2/§3). The source pointer is locked (§3, disabled — set at
/// creation); the cited-record date is the structured `DraftDate` editor. A pure fn (the edit
/// state's signals passed in) so the SSR tests render it without `AppCtx`.
pub fn citation_record_fields(loc: &Localizer, record: RecordEditState<genealogy_ui::CitationDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let current = draft();
    let committed = seed.read().clone();
    let source_display = current.source.existing_id().unwrap_or_default().to_owned();
    let source_original = committed.source.existing_id().unwrap_or_default().to_owned();
    rsx! {
        Card { title: loc.tab_label("overview"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "citation-id".to_owned(),
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
                    label: loc.field_label("source"),
                    name: "citation-source".to_owned(),
                    editing,
                    value: source_display,
                    original: source_original,
                    reset_label: loc.action_reset_field(&loc.field_label("source")),
                    mono: true,
                    locked: true,
                    oninput: move |_: String| {},
                    onreset: move |()| {},
                }
                {date_draft_field(
                    loc,
                    "citation-date",
                    editing,
                    current.date.clone(),
                    committed.date.clone(),
                    Callback::new(move |value: genealogy_ui::DateDraft| draft.write().date = value),
                    Callback::new(move |()| {
                        let value = seed.read().date.clone();
                        draft.write().date = value;
                    }),
                )}
                DraftText {
                    label: loc.field_label("page"),
                    name: "citation-page".to_owned(),
                    editing,
                    value: current.page.clone(),
                    original: committed.page.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("page")),
                    oninput: move |value: String| draft.write().page = value,
                    onreset: move |()| {
                        let value = seed.read().page.clone();
                        draft.write().page = value;
                    },
                }
                {citation_evidence_record_fields(loc, record)}
            }
        }
    }
}

/// The citation create form's source field (§7 — a citation cites exactly one source): a find-or-create
/// Sources picker while unset or pointing at an existing source, or an inline new-source [`draft_card`]
/// (a title input) once "+ New" is chosen.
fn citation_source_field(
    loc: &Localizer,
    draft: Signal<genealogy_ui::CitationDraft>,
    source: &RecordPicker,
) -> Element {
    match &draft().source {
        RecordLink::New(_) => {
            let title = loc.source_new_title();
            let discard = source.callbacks.onclear;
            let body = citation_new_source_body(loc, draft);
            draft_card(
                &title,
                &loc.draft_card_badge(),
                loc.draft_card_discard(&title),
                discard,
                body,
            )
        }
        RecordLink::Empty | RecordLink::Existing(_) => record_picker(loc, source),
    }
}

/// The inline new-source fields inside the citation create form's draft card: a single title input,
/// bound to the draft's new-source link.
fn citation_new_source_body(loc: &Localizer, mut draft: Signal<genealogy_ui::CitationDraft>) -> Element {
    let current = match &draft().source {
        RecordLink::New(fields) => fields.clone(),
        _ => NewSourceFields::default(),
    };
    rsx! {
        Input {
            label: loc.field_label("title"),
            name: "citation-new-source-title".to_owned(),
            value: current.title.clone(),
            oninput: move |event: FormEvent| {
                if let RecordLink::New(fields) = &mut draft.write().source {
                    fields.title = event.value();
                }
            },
        }
    }
}

/// The citation create form's evidence rows: the record-level confidence + the three Evidence
/// Explained axis selects (distinct from the provenance block's).
fn citation_evidence_fields(loc: &Localizer, mut draft: Signal<genealogy_ui::CitationDraft>) -> Element {
    let confidence_levels = ConfidenceLevel::all();
    let (confidence_options, confidence_selected) = optional_enum_select(
        loc.record_unset(),
        &confidence_levels,
        draft().confidence.as_ref(),
        |level| loc.confidence_label(*level),
    );
    let (source_options, source_selected) = optional_enum_select(
        loc.record_unset(),
        &genealogy_ui::SOURCE_QUALITIES,
        draft().source_quality.as_ref(),
        |value| loc.evidence_source_label(*value),
    );
    let (info_options, info_selected) = optional_enum_select(
        loc.record_unset(),
        &genealogy_ui::INFORMATION_KINDS,
        draft().information.as_ref(),
        |value| loc.evidence_information_label(*value),
    );
    let (kind_options, kind_selected) = optional_enum_select(
        loc.record_unset(),
        &genealogy_ui::EVIDENCE_KINDS,
        draft().evidence_kind.as_ref(),
        |value| loc.evidence_kind_label(*value),
    );
    rsx! {
        Select {
            label: loc.field_label("confidence"),
            name: "citation-confidence".to_owned(),
            value: Some(confidence_selected),
            options: confidence_options,
            onchange: move |event: FormEvent| {
                let levels = ConfidenceLevel::all();
                draft.write().confidence = event.value().parse::<usize>().ok().and_then(|index| levels.get(index).copied());
            },
        }
        Select {
            label: loc.evidence_axis_label(genealogy_ui::EvidenceAxis::Source),
            name: "citation-source-quality".to_owned(),
            value: Some(source_selected),
            options: source_options,
            onchange: move |event: FormEvent| {
                draft.write().source_quality = event.value().parse::<usize>().ok().and_then(|index| genealogy_ui::SOURCE_QUALITIES.get(index).copied());
            },
        }
        Select {
            label: loc.evidence_axis_label(genealogy_ui::EvidenceAxis::Information),
            name: "citation-information".to_owned(),
            value: Some(info_selected),
            options: info_options,
            onchange: move |event: FormEvent| {
                draft.write().information = event.value().parse::<usize>().ok().and_then(|index| genealogy_ui::INFORMATION_KINDS.get(index).copied());
            },
        }
        Select {
            label: loc.evidence_axis_label(genealogy_ui::EvidenceAxis::Evidence),
            name: "citation-evidence-kind".to_owned(),
            value: Some(kind_selected),
            options: kind_options,
            onchange: move |event: FormEvent| {
                draft.write().evidence_kind = event.value().parse::<usize>().ok().and_then(|index| genealogy_ui::EVIDENCE_KINDS.get(index).copied());
            },
        }
    }
}

/// The citation create form's field rows (`citation.html` edit specimen): the source (existing or
/// inline new — §6b), the cited-record date, the page, and the record-level confidence + the three
/// evidence axes. A pure fn (no `AppCtx`) so SSR tests can render it directly.
pub fn citation_create_fields(
    loc: &Localizer,
    mut draft: Signal<genealogy_ui::CitationDraft>,
    source: &RecordPicker,
) -> Element {
    rsx! {
        Card { title: loc.tab_label("overview"),
            div { class: "stack",
                {citation_source_field(loc, draft, source)}
                {date_draft_field(
                    loc,
                    "citation-date",
                    true,
                    draft().date.clone(),
                    genealogy_ui::DateDraft::default(),
                    Callback::new(move |value: genealogy_ui::DateDraft| draft.write().date = value),
                    Callback::new(move |()| draft.write().date = genealogy_ui::DateDraft::default()),
                )}
                Input {
                    label: loc.field_label("page"),
                    name: "citation-page".to_owned(),
                    value: draft().page.clone(),
                    oninput: move |event: FormEvent| draft.write().page = event.value(),
                }
                {citation_evidence_fields(loc, draft)}
            }
        }
    }
}

/// Which citation collection-row edit form (if any) the side panel is showing. The citation's own
/// scalar record (id · page · confidence · evidence axes) is edited in place via the sticky header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationEditForm {
    /// Assert a typed attribute — `None` adds a new one, `Some(row)` edits (supersedes) an existing one.
    Attribute(Option<CitationAttributeVm>),
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
    let nav = use_context::<NavState>();
    let mut label_nav = nav;
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<CitationEditForm>);
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
        async move { load_screen(services, Intent::ShowCitation { human_id }).await }
    });

    // The shared whole-record edit state, seeded from the loaded citation (empty until it loads); it
    // reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::CitationDetail(detail))) => {
            genealogy_ui::CitationDraft::from_detail(detail)
        }
        _ => genealogy_ui::CitationDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::CitationDraft>(&seed);

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the cited
    // source (`tab_label` falls back to `human_id` when unsourced, mirroring the detail-head title).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::CitationDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::Citations,
            &label_human_id,
            genealogy_ui::tab_label(detail.source.as_deref(), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (CitationEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_citation_edit(services, edit, prov).await {
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
    let on_edit_open = use_callback(move |form: CitationEditForm| editing_for_open.set(Some(form)));
    let citation_tag_human = human_id.clone();
    let on_tag_remove = use_callback(move |tag_id: String| {
        on_submit.call((
            CitationEdit::Tag {
                human_id: citation_tag_human.clone(),
                tag_id,
                remove: true,
            },
            ProvenanceDraft::default(),
        ));
    });
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
            let edit = CitationEdit::UndoAssertion { human_id, assertion_id };
            match save_citation_edit(services, edit, prov).await {
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
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::CitationDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_citation_edit).await;
            finish_record_save(
                effective,
                Category::Citations,
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
        Some(ScreenData::Loaded(IntentOutcome::CitationDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read() || retract.read().is_some());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let undo_human = human_id.clone();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((
            CitationEdit::UndoAssertion {
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
        Some(ScreenData::Loaded(IntentOutcome::CitationDetail(detail))) => citation_detail(
            &state,
            detail,
            CitationPane {
                active,
                side_edit: editing,
                record,
                retract,
                retract_reason,
            },
            CitationCallbacks {
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
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::DataQuality(_)
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

/// The signals a citation's detail threads to its tabs: the active tab, the collection-row side panel,
/// and the whole-record edit state.
#[derive(Clone, Copy)]
struct CitationPane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<CitationEditForm>>,
    /// The whole-record edit state (id · page · confidence · evidence axes; source/date locked).
    record: RecordEditState<genealogy_ui::CitationDraft>,
    /// The row being retracted/detached, if the retract panel is open: `(assertion_id, label, detach)`.
    retract: Signal<Option<(String, String, bool)>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The commit callbacks a citation's detail wires in: one-command collection edits, the whole-record
/// save (the scalar edit via `edits_against`), and the per-row correction (edit-open + retract-confirm).
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "event-handler fields conventionally share the on_ prefix"
)]
struct CitationCallbacks {
    /// Commits one [`CitationEdit`] command (a collection row).
    on_submit: Callback<(CitationEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::CitationDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<CitationEditForm>,
    /// Retracts an assertion by id from the History tab (dispatches `UndoAssertion`).
    on_undo: Callback<String>,
    /// Untags a tag by id from the Tags tab (dispatches `Tag { remove: true }`).
    on_tag_remove: Callback<String>,
}

/// Renders a loaded citation's detail container: header (with the sticky-header record Edit/Cancel/
/// Save), the tab strip, the active tab's content, and the collection-row side panel.
fn citation_detail(
    state: &AppState,
    detail: &CitationDetail,
    pane: CitationPane,
    callbacks: CitationCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let CitationPane {
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
    let labels = RecordActionLabels::resolve(loc);
    rsx! {
        div { class: "record-pane", tabindex: "-1", onkeydown: move |event| record_keydown(&event, record, on_record_save),
            DetailContainer {
                title: detail.source.clone().unwrap_or_else(|| detail.human_id.clone()),
                subtitle,
                id_label: Some(detail.human_id.clone()),
                avatar: "❝".to_owned(),
                extras: citation_restriction_toggles(loc, detail, on_submit, human_id),
                actions: record_head_actions(&labels, record, rsx! {}, on_record_save),
                tabs: tab_items,
                active,
                {citation_tab_content(state, detail, active_id, editing, record, on_retract, on_edit_open, on_undo, on_tag_remove)}
            }
            {citation_edit_panel(state, editing, on_submit, human_id)}
            {citation_retract_panel(loc, retract, retract_reason, on_retract_confirm)}
        }
    }
}

/// Renders the shared Retract/Detach side panel when a citation collection row's action is armed.
/// Reads the armed `(assertion_id, label, detach)` and binds the rationale input; confirming dispatches
/// `UndoAssertion`. Closed (rendered empty) when nothing is armed. Never renders the target's
/// `AssertionId`.
fn citation_retract_panel(
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
            loc.action_title("detach-citation"),
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
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threads the pane's signals + callbacks"
)]
fn citation_tab_content(
    state: &AppState,
    detail: &CitationDetail,
    tab_id: &str,
    mut editing: Signal<Option<CitationEditForm>>,
    record: RecordEditState<genealogy_ui::CitationDraft>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<CitationEditForm>,
    on_undo: Callback<String>,
    on_tag_remove: Callback<String>,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "attributes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-attribute"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(CitationEditForm::Attribute(None))) }
            }
            {citation_attributes_table(loc, &detail.attributes, on_edit_open, on_retract)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(CitationEditForm::Media)) }
            }
            {media_gallery(loc, &detail.media, Some(on_retract))}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(CitationEditForm::Note)) }
            }
            {id_list(loc, &detail.notes, Some(on_retract))}
        },
        "tags" => tags_panel(loc, &detail.tags, editing, CitationEditForm::Tag, on_tag_remove),
        "history" => history_panel(loc, &detail.history, Some(on_undo)),
        _ => citation_overview(loc, detail, record),
    }
}

/// The Overview tab, read-first (`record-editing.html` §1/§2): the citation's scalar record (id ·
/// source · date · page · confidence · evidence axes) as read boxes, plus the Evidence Explained axis
/// chips. Entering edit mode (via the sticky-header Edit) swaps the record fields to inputs and, while
/// dirty, shows the provenance block; the axis-chip card is hidden in edit mode.
pub fn citation_overview(
    loc: &Localizer,
    detail: &CitationDetail,
    record: RecordEditState<genealogy_ui::CitationDraft>,
) -> Element {
    if record.editing.read().to_owned() {
        return rsx! {
            div { class: "section-note", "{loc.overview_note()}" }
            {citation_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        };
    }
    rsx! {
        div { class: "section-note", "{loc.overview_note()}" }
        div { class: "grid-2",
            {citation_record_fields(loc, record)}
            Card { title: loc.field_label("evidence"),
                div { class: "stack",
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
                    }
                    if detail.source.is_none() {
                        div { class: "fact-row",
                            NoSourceFlag { label: loc.no_source() }
                        }
                    }
                }
            }
        }
    }
}

/// The Attributes tab: a row per recorded `(type, value)` attribute, plus a per-row Edit (supersedes
/// via [`CitationEdit::AddAttribute`]) and Retract (retracts the attribute assertion — it stays in
/// History). Never renders the attribute's `AssertionId`.
pub fn citation_attributes_table(
    loc: &Localizer,
    attributes: &[CitationAttributeVm],
    onedit: Callback<CitationEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if attributes.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![loc.field_label("attribute-type"), loc.field_label("value"), String::new()],
            for attribute in attributes.iter() {
                tr {
                    td { "{attribute.attribute_type}" }
                    td { class: "muted", "{attribute.value}" }
                    {row_actions_cell(
                        loc,
                        &attribute.attribute_type,
                        Some((CitationEditForm::Attribute(Some(attribute.clone())), None)), None,
                        Some(RowRetract { assertion_id: attribute.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
                }
            }
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
    let title = match &form {
        CitationEditForm::Attribute(None) => loc.action_label("add-attribute"),
        CitationEditForm::Attribute(Some(_)) => loc.panel_title("edit-attribute"),
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
                CitationEditForm::Attribute(seed) => rsx! { CitationAttributeForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Media => rsx! { CitationAttachForm { human_id, is_note: false, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Note => rsx! { CitationAttachForm { human_id, is_note: true, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Tag => rsx! { CitationTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Add attribute" form → [`CitationEdit::AddAttribute`]. `seed: None` adds a new attribute;
/// `Some(row)` edits an existing one — the type + value are pre-filled and the draft's `supersedes`
/// is seeded with the row's assertion id so Save supersedes (replaces) rather than appends (ADR 0004 §2).
#[component]
fn CitationAttributeForm(
    human_id: String,
    seed: Option<CitationAttributeVm>,
    onsubmit: EventHandler<(CitationEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut attribute_type = use_signal(|| seed.as_ref().map(|row| row.attribute_type.clone()).unwrap_or_default());
    let mut value = use_signal(|| seed.as_ref().map(|row| row.value.clone()).unwrap_or_default());
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|row| row.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let save_label = loc.action_label("save");
    rsx! {
        Input {
            label: loc.field_label("attribute-type"),
            name: "attribute-type".to_owned(),
            value: attribute_type(),
            oninput: move |event: FormEvent| attribute_type.set(event.value()),
        }
        Input {
            label: loc.field_label("value"),
            name: "value".to_owned(),
            value: value(),
            oninput: move |event: FormEvent| value.set(event.value()),
        }
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
    let services = state.services().clone();
    let (field, category) = if is_note {
        ("note", Category::Notes)
    } else {
        ("media", Category::Media)
    };
    let picker = use_existing_picker(
        services,
        category,
        loc.field_label(field),
        field.to_owned(),
        loc.picker_entity(category),
        Vec::new(),
    );
    let prov = use_signal(ProvenanceDraft::default);
    let picker_for_save = picker.clone();
    let onsave = use_callback(move |()| {
        let Some(id) = picker_selection_id(&picker_for_save) else {
            return;
        };
        let edit = if is_note {
            CitationEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            }
        } else {
            CitationEdit::AttachMedia {
                human_id: human_id.clone(),
                media_id: id,
            }
        };
        onsubmit.call((edit, prov()));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
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
