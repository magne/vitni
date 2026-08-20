use super::prelude::*;
// The citation attribute row view-model (seeds the per-row attribute edit) and the record-link
// view-model enum (citation source); the latter shadows the prelude's `RecordLink` link component,
// which this screen does not use.
use vitni_ui::{CitationAttributeVm, EvidenceAxis, RecordLink};

/// The create-mode citation record: an uncommitted [`CitationDraft`] rendered as the create form in
/// the detail pane (`record-editing.html` §6). The source is required (§7); a "new source" selection
/// creates a source inline on Save (§6b). Save commits the whole citation; Cancel discards.
#[component]
pub fn CitationCreateRecord(draft_id: DraftId) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<vitni_ui::CitationDraft>(Category::Citations, draft_id);
    let mut draft = record.draft;
    // The find-or-create source picker: its options refetch after any mutation (#266); pick/clear/
    // "+ New" drive the draft's link.
    let source_state = use_signal(vitni_ui::PickerState::default);
    let source_services = services.clone();
    let source_rows = use_resource(move || {
        let _ = data_version_ticket(Some(nav));
        let services = source_services.clone();
        async move { load_picker_rows(services, Category::Sources).await }
    });
    let source_onpick =
        use_callback(move |selection: PickerSelection| draft.write().source = RecordLink::Existing(selection));
    let source_onclear = use_callback(move |()| draft.write().source = RecordLink::Empty);
    let source_onnew =
        use_callback(move |_query: String| draft.write().source = RecordLink::New(NewSourceFields::default()));
    let created_label = loc.action_label(ActionLabel::Created);
    let on_save = use_callback(move |(draft, prov): (vitni_ui::CitationDraft, ProvenanceDraft)| {
        let Some(request) = draft.to_request() else {
            return;
        };
        let services = services.clone();
        let created = created_label.clone();
        spawn(async move {
            let committed = commit_citation_change_set(services, request, prov).await;
            finish_draft_commit(
                committed,
                DraftCommit::new(Category::Citations, draft_id, &draft, created),
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
    use_save_on_request(EditKey::draft(Category::Citations, draft_id), record, save_now);
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
fn citation_evidence_record_fields(loc: &Localizer, record: RecordEditState<vitni_ui::CitationDraft>) -> Element {
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
        &vitni_ui::SOURCE_QUALITIES,
        draft().source_quality.as_ref(),
        seed.read().source_quality.as_ref(),
        |value| loc.evidence_source_label(*value),
    );
    let (info_options, info_value, info_original) = record_enum_select(
        loc.record_unset(),
        &vitni_ui::INFORMATION_KINDS,
        draft().information.as_ref(),
        seed.read().information.as_ref(),
        |value| loc.evidence_information_label(*value),
    );
    let (kind_options, kind_value, kind_original) = record_enum_select(
        loc.record_unset(),
        &vitni_ui::EVIDENCE_KINDS,
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
            label: loc.evidence_axis_label(vitni_ui::EvidenceAxis::Source),
            name: "citation-source-quality".to_owned(),
            editing,
            value: source_value,
            original: source_original,
            reset_label: loc.action_reset_field(&loc.evidence_axis_label(vitni_ui::EvidenceAxis::Source)),
            options: source_options,
            onchange: move |value: String| {
                draft.write().source_quality = value.parse::<usize>().ok().and_then(|index| vitni_ui::SOURCE_QUALITIES.get(index).copied());
            },
            onreset: move |()| {
                let value = seed.read().source_quality;
                draft.write().source_quality = value;
            },
        }
        DraftSelect {
            label: loc.evidence_axis_label(vitni_ui::EvidenceAxis::Information),
            name: "citation-information".to_owned(),
            editing,
            value: info_value,
            original: info_original,
            reset_label: loc.action_reset_field(&loc.evidence_axis_label(vitni_ui::EvidenceAxis::Information)),
            options: info_options,
            onchange: move |value: String| {
                draft.write().information = value.parse::<usize>().ok().and_then(|index| vitni_ui::INFORMATION_KINDS.get(index).copied());
            },
            onreset: move |()| {
                let value = seed.read().information;
                draft.write().information = value;
            },
        }
        DraftSelect {
            label: loc.evidence_axis_label(vitni_ui::EvidenceAxis::Evidence),
            name: "citation-evidence-kind".to_owned(),
            editing,
            value: kind_value,
            original: kind_original,
            reset_label: loc.action_reset_field(&loc.evidence_axis_label(vitni_ui::EvidenceAxis::Evidence)),
            options: kind_options,
            onchange: move |value: String| {
                draft.write().evidence_kind = value.parse::<usize>().ok().and_then(|index| vitni_ui::EVIDENCE_KINDS.get(index).copied());
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
pub fn citation_record_fields(loc: &Localizer, record: RecordEditState<vitni_ui::CitationDraft>) -> Element {
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
                    Callback::new(move |value: vitni_ui::DateDraft| draft.write().date = value),
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
                {record_restrictions_field(loc, record)}
            }
        }
    }
}

/// The citation create form's source field (§7 — a citation cites exactly one source): a find-or-create
/// Sources picker while unset or pointing at an existing source, or an inline new-source [`draft_card`]
/// (a title input) once "+ New" is chosen.
fn citation_source_field(loc: &Localizer, draft: Signal<vitni_ui::CitationDraft>, source: &RecordPicker) -> Element {
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
fn citation_new_source_body(loc: &Localizer, mut draft: Signal<vitni_ui::CitationDraft>) -> Element {
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
fn citation_evidence_fields(loc: &Localizer, mut draft: Signal<vitni_ui::CitationDraft>) -> Element {
    let confidence_levels = ConfidenceLevel::all();
    let (confidence_options, confidence_selected) = optional_enum_select(
        loc.record_unset(),
        &confidence_levels,
        draft().confidence.as_ref(),
        |level| loc.confidence_label(*level),
    );
    let (source_options, source_selected) = optional_enum_select(
        loc.record_unset(),
        &vitni_ui::SOURCE_QUALITIES,
        draft().source_quality.as_ref(),
        |value| loc.evidence_source_label(*value),
    );
    let (info_options, info_selected) = optional_enum_select(
        loc.record_unset(),
        &vitni_ui::INFORMATION_KINDS,
        draft().information.as_ref(),
        |value| loc.evidence_information_label(*value),
    );
    let (kind_options, kind_selected) = optional_enum_select(
        loc.record_unset(),
        &vitni_ui::EVIDENCE_KINDS,
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
            label: loc.evidence_axis_label(vitni_ui::EvidenceAxis::Source),
            name: "citation-source-quality".to_owned(),
            value: Some(source_selected),
            options: source_options,
            onchange: move |event: FormEvent| {
                draft.write().source_quality = event.value().parse::<usize>().ok().and_then(|index| vitni_ui::SOURCE_QUALITIES.get(index).copied());
            },
        }
        Select {
            label: loc.evidence_axis_label(vitni_ui::EvidenceAxis::Information),
            name: "citation-information".to_owned(),
            value: Some(info_selected),
            options: info_options,
            onchange: move |event: FormEvent| {
                draft.write().information = event.value().parse::<usize>().ok().and_then(|index| vitni_ui::INFORMATION_KINDS.get(index).copied());
            },
        }
        Select {
            label: loc.evidence_axis_label(vitni_ui::EvidenceAxis::Evidence),
            name: "citation-evidence-kind".to_owned(),
            value: Some(kind_selected),
            options: kind_options,
            onchange: move |event: FormEvent| {
                draft.write().evidence_kind = event.value().parse::<usize>().ok().and_then(|index| vitni_ui::EVIDENCE_KINDS.get(index).copied());
            },
        }
    }
}

/// The citation create form's field rows (`citation.html` edit specimen): the source (existing or
/// inline new — §6b), the cited-record date, the page, and the record-level confidence + the three
/// evidence axes. A pure fn (no `AppCtx`) so SSR tests can render it directly.
pub fn citation_create_fields(
    loc: &Localizer,
    mut draft: Signal<vitni_ui::CitationDraft>,
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
                    vitni_ui::DateDraft::default(),
                    Callback::new(move |value: vitni_ui::DateDraft| draft.write().date = value),
                    Callback::new(move |()| draft.write().date = vitni_ui::DateDraft::default()),
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
    let active = use_detail_tab(Category::Citations, &human_id);
    let editing = use_signal(|| None::<CitationEditForm>);
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
    } = use_detail_commits::<CitationCommits, CitationEditForm>(&state, &human_id, editing);
    let saved_label = state.data_loc().action_label(ActionLabel::Saved);

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
        Some(ScreenData::Loaded(IntentOutcome::CitationDetail(detail))) => vitni_ui::CitationDraft::from_detail(detail),
        _ => vitni_ui::CitationDraft::new(),
    };
    let record = use_record_edit::<vitni_ui::CitationDraft>(Category::Citations, &human_id, &seed);

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
            vitni_ui::tab_label(detail.source.as_deref(), &label_human_id),
        );
    });

    let mut editing_for_open = editing;
    let on_edit_open = use_callback(move |form: CitationEditForm| editing_for_open.set(Some(form)));

    let record_services = services.clone();
    let record_nav = nav;
    let current_id = human_id.clone();
    let on_record_save = use_callback(move |(draft, prov): (vitni_ui::CitationDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_citation_edit).await;
            finish_record_save(effective, Category::Citations, &current, record_nav, reload, &saved);
        });
    });

    // ⌘Z retracts the newest undoable assertion of this record's loaded change log (WP5).
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::CitationDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read() || retract.read().is_some());
    let undo_notice = chrome.kbd_nothing_to_undo();
    use_record_undo(
        nav,
        Category::Citations,
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
    use_save_on_request(EditKey::saved(Category::Citations, &human_id), record, save_now);

    // The Media tab's crop viewer: opening a card, and superseding its crop via `SetMediaRegion`.
    let media_viewing = use_signal(|| None::<MediaRefVm>);
    let on_view = use_callback(move |item: MediaRefVm| media_viewing.clone().set(Some(item)));
    let region_human = human_id.clone();
    let on_region = use_callback(
        move |(assertion_id, crop, caption): (String, Option<Rect>, Option<String>)| {
            on_submit.call((
                CitationEdit::SetMediaRegion {
                    human_id: region_human.clone(),
                    assertion_id,
                    crop,
                    caption,
                },
                ProvenanceDraft::default(),
            ));
        },
    );
    let media_state = MediaTabState {
        viewing: media_viewing,
        on_view,
        on_region,
    };

    match &*data.read_unchecked() {
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
            &CitationCallbacks {
                on_submit,
                on_record_save,
                on_retract,
                on_retract_confirm,
                on_edit_open,
                on_undo,
                on_tag_remove,
                media_state,
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
            | IntentOutcome::MergeCompare(_)
            | IntentOutcome::ResearchNoteDetail(_)
            | IntentOutcome::Geography(_),
        )) => rsx! {},
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
    record: RecordEditState<vitni_ui::CitationDraft>,
    /// The row being retracted/detached, if the retract panel is open.
    retract: Signal<Option<RetractTarget>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The commit callbacks a citation's detail wires in: one-command collection edits, the whole-record
/// save (the scalar edit via `edits_against`), and the per-row correction (edit-open + retract-confirm).
#[derive(Clone, Copy)]
struct CitationCallbacks {
    /// Commits one [`CitationEdit`] command (a collection row).
    on_submit: Callback<(CitationEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(vitni_ui::CitationDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<CitationEditForm>,
    /// Retracts an assertion by id from the History tab (dispatches `UndoAssertion`).
    on_undo: Callback<String>,
    /// Arms the untag panel for a tag chip's ×: `(tag_id, tag name)`.
    on_tag_remove: Callback<(String, String)>,
    /// The Media tab's viewer state + crop-supersede wiring.
    media_state: MediaTabState,
}

/// Renders a loaded citation's detail container: header (with the sticky-header record Edit/Cancel/
/// Save), the tab strip, the active tab's content, and the collection-row side panel.
fn citation_detail(
    state: &AppState,
    detail: &CitationDetail,
    pane: CitationPane,
    callbacks: &CitationCallbacks,
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
    let media_state = callbacks.media_state;
    let tabs = citation_tabs(detail, loc);
    let tab_items: Vec<TabItem> = tabs
        .iter()
        .map(|tab| TabItem {
            id: tab.id.to_owned(),
            label: tab.label.clone(),
            count: tab.count,
        })
        .collect();
    let active_tab = tabs.get(active()).cloned().unwrap_or_else(|| fallback_tab("overview"));
    let subtitle = detail.page.clone();
    let labels = RecordActionLabels::resolve(loc);
    rsx! {
        div { class: "record-pane", tabindex: "-1", onkeydown: move |event| record_keydown(&event, record),
            DetailContainer {
                title: detail.source.clone().unwrap_or_else(|| detail.human_id.clone()),
                subtitle,
                id_label: Some(detail.human_id.clone()),
                avatar: "❝".to_owned(),
                extras: restriction_display(loc, &detail.restrictions),
                actions: record_head_actions(&labels, record, rsx! {}, on_record_save),
                tabs: tab_items,
                active,
                {citation_tab_content(state, detail, &active_tab, editing, record, on_retract, on_edit_open, on_undo, on_tag_remove, media_state)}
            }
            {citation_edit_panel(state, editing, on_submit, human_id)}
            {retract_side_panel(loc, retract, retract_reason, on_retract_confirm, "detach-citation")}
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
    tab: &DetailTab,
    editing: Signal<Option<CitationEditForm>>,
    record: RecordEditState<vitni_ui::CitationDraft>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<CitationEditForm>,
    on_undo: Callback<String>,
    on_tag_remove: Callback<(String, String)>,
    media_state: MediaTabState,
) -> Element {
    let loc = state.data_loc();
    match tab.id {
        "attributes" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, CitationEditForm::Attribute(None)),
            None,
            rsx! {
                {citation_attributes_table(loc, &detail.attributes, on_edit_open, on_retract)}
            },
        ),
        "media" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, CitationEditForm::Media),
            None,
            rsx! {
                {media_tab(loc, &detail.media, Some(on_retract), media_state)}
            },
        ),
        "notes" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, CitationEditForm::Note),
            None,
            rsx! {
                {notes_table(loc, &detail.notes, Some(on_retract))}
            },
        ),
        "tags" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, CitationEditForm::Tag),
            Some(TabActionStyle {
                emphasis: Some(ButtonVariant::Ghost),
                ..Default::default()
            }),
            tags_panel(loc, &detail.tags, on_tag_remove),
        ),
        "history" => tab_frame::<()>(
            loc,
            tab,
            TabActionTarget::None,
            None,
            history_panel(loc, &detail.history, Some(on_undo)),
        ),
        _ => citation_overview(loc, detail, record),
    }
}

/// The Overview tab, read-first (`record-editing.html` §1/§2): the citation's scalar record (id ·
/// source · date · page · confidence · evidence axes) as read boxes, plus the Evidence Explained axis
/// chips. Entering edit mode (via the sticky-header Edit) swaps the record fields to inputs and, while
/// dirty, shows the provenance block; the axis-chip card is hidden in edit mode.
///
/// The chip card is one row per axis, each named for the axis it holds (`citation.html`) — a single row
/// labelled "Evidence" named the third axis rather than the set of three (issue #316).
pub fn citation_overview(
    loc: &Localizer,
    detail: &CitationDetail,
    record: RecordEditState<vitni_ui::CitationDraft>,
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
            Card { title: loc.field_label("analysis"),
                div { class: "stack",
                    for axis in [EvidenceAxis::Source, EvidenceAxis::Information, EvidenceAxis::Evidence] {
                        {axis_chip_row(loc, detail, axis)}
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

/// One row of the Overview's Analysis card: the axis's own label and the chip recorded on it, or `—`
/// when that axis was never graded.
fn axis_chip_row(loc: &Localizer, detail: &CitationDetail, axis: EvidenceAxis) -> Element {
    let chip = detail.evidence_axes.iter().find(|chip| chip.axis == axis);
    rsx! {
        FactRow { label: loc.evidence_axis_label(axis),
            span { class: "grow wrap",
                match chip {
                    Some(chip) => rsx! { EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() } },
                    None => rsx! { "—" },
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
            caption: loc.tab_label("attributes"),
            headers: vec![loc.field_label("attribute-type"), loc.field_label("value"), String::new()],
            for attribute in attributes.iter() {
                tr {
                    td { "{attribute.attribute_type}" }
                    td { class: "muted", "{attribute.value}" }
                    {row_actions_cell(
                        loc,
                        &attribute.attribute_type,
                        Some((CitationEditForm::Attribute(Some(attribute.clone())), None)), None,
                        Some(RowRetract { assertion_id: attribute.assertion_id.clone(), button_label: RowVerb::Retract, title: "retract", detach: false }),
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
        CitationEditForm::Attribute(None) => loc.action_label(ActionLabel::AddAttribute),
        CitationEditForm::Attribute(Some(_)) => loc.panel_title("edit-attribute"),
        CitationEditForm::Media => loc.action_label(ActionLabel::AttachMedia),
        CitationEditForm::Note => loc.action_label(ActionLabel::AttachNote),
        CitationEditForm::Tag => loc.action_label(ActionLabel::AddTag),
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
    let save_label = loc.action_button(ActionLabel::Save);
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
    let attach = use_attach_picker(
        services.clone(),
        category,
        loc.field_label(field),
        field.to_owned(),
        loc.picker_entity(category),
        Vec::new(),
    );
    let prov = use_signal(ProvenanceDraft::default);
    let onattach = use_callback(move |id: String| {
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
    let onsave = use_attach_save(services, &attach, prov, onattach);
    attach_link_form(loc, &attach, rsx! {}, prov, onsave)
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
                        onsubmit.call((CitationEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

// ─── Family slice (PR7) ──────────────────────────────────────────────────────────────────────────
