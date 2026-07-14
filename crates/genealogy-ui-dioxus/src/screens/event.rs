use super::prelude::*;
use genealogy_app::EventType;
// The record-link view-model enum (event place); shadows the prelude's `RecordLink` link component
// (the participant edit form reaches it as `super::shared::RecordLink`). `ParticipantVm` seeds the
// per-row participant edit.
use genealogy_ui::{ParticipantVm, RecordLink};

/// A row's armed retract, for the shared retract panel. Carries the assertion to retract plus the row
/// label + detach flag (the panel wording). `person_human_id` is set only for a canonical person-origin
/// participant on the Participants tab: the retract then targets the Person aggregate (via `save_edit`)
/// rather than this event, so the correct aggregate's assertion is undone (ADR 0004 §2).
#[derive(Clone, PartialEq)]
struct RetractTarget {
    assertion_id: String,
    label: String,
    detach: bool,
    person_human_id: Option<String>,
}

/// The create-mode event record: an uncommitted [`EventDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). The type is required; a "new place" selection creates a
/// place inline on Save (§6b cascade). Save commits the whole event; Cancel discards.
#[component]
pub fn EventCreateRecord() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::EventDraft>();
    let mut draft = record.draft;
    // The find-or-create place picker: options load once; pick/clear/"+ New" drive the draft's link.
    let place_state = use_signal(genealogy_ui::PickerState::default);
    let place_services = services.clone();
    let place_rows = use_resource(move || {
        let services = place_services.clone();
        async move { load_picker_rows(services, Category::Places).await }
    });
    let place_onpick =
        use_callback(move |selection: PickerSelection| draft.write().place = RecordLink::Existing(selection));
    let place_onclear = use_callback(move |()| draft.write().place = RecordLink::Empty);
    let place_onnew =
        use_callback(move |_query: String| draft.write().place = RecordLink::New(NewPlaceFields::default()));
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::EventDraft, ProvenanceDraft)| {
        let request = draft.to_request();
        let services = services.clone();
        spawn(async move {
            match commit_event_change_set(services, request, prov).await {
                Ok(id) => nav.commit_draft(RecordRef {
                    category: Category::Events,
                    human_id: id.clone(),
                    label: id,
                }),
                Err(message) => nav.notify(message),
            }
        });
    });
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| nav.cancel_draft(Category::Events) }
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
    let place = RecordPicker {
        config: PickerConfig {
            label: loc.field_label("place"),
            name: "event-place".to_owned(),
            entity_label: loc.picker_entity(Category::Places),
            allow_new: true,
        },
        state: place_state,
        options: picker_options(place_rows.read_unchecked().as_ref()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: place_onpick,
            onclear: place_onclear,
            onnew: place_onnew,
        },
    };
    create_record_frame(
        &loc.event_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {event_create_fields(loc, draft, &place)}
            {record_edit_provenance(loc, record)}
        },
    )
}

/// The whole-record edit context an event's record fields need: the buffered edit state plus the
/// existing-place picker (its live state, loaded options, and pick/clear callbacks) and the per-field
/// reset that restores the place link to its committed value.
#[derive(Clone)]
pub struct EventEditCtx {
    /// The buffered whole-record edit state.
    pub record: RecordEditState<genealogy_ui::EventDraft>,
    /// The existing-place picker (configured existing-only — inline place creation is create-only).
    pub place: RecordPicker,
    /// Restores the draft's place link to the committed seed (the place field's reset control).
    pub place_reset: Callback<()>,
}

/// The event's scalar record fields (id · type · date · place · description), read-first: read boxes in
/// view mode, inputs with per-field reset in edit mode (`record-editing.html` §2/§3). Date is the
/// structured `DraftDate` editor (modifier · date · quality · calendar · original text); the place is
/// an existing-place picker ([`draft_picker_field`], [`EventEdit::LinkPlace`]) — the inline new-place
/// cascade stays create-only. A pure fn (the edit state's signals passed in) so the SSR tests render it
/// without `AppCtx`.
pub fn event_record_fields(loc: &Localizer, ctx: &EventEditCtx) -> Element {
    let record = ctx.record;
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let types = event_type_choices();
    let options: Vec<SelectChoice> = types
        .iter()
        .enumerate()
        .map(|(index, event_type)| SelectChoice {
            value: index.to_string(),
            label: loc.event_type_label(event_type),
        })
        .collect();
    let index_of = |event_type: &EventType| {
        event_type_choices()
            .iter()
            .position(|t| t == event_type)
            .unwrap_or(0)
            .to_string()
    };
    let current = draft();
    let committed = seed.read().clone();
    rsx! {
        Card { title: loc.tab_label("events"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "event-id".to_owned(),
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
                DraftSelect {
                    label: loc.field_label("type"),
                    name: "event-type".to_owned(),
                    editing,
                    value: index_of(&current.event_type),
                    original: index_of(&committed.event_type),
                    reset_label: loc.action_reset_field(&loc.field_label("type")),
                    options,
                    onchange: move |value: String| {
                        let types = event_type_choices();
                        if let Some(event_type) = value.parse::<usize>().ok().and_then(|index| types.get(index).cloned()) {
                            draft.write().event_type = event_type;
                        }
                    },
                    onreset: move |()| {
                        let value = seed.read().event_type.clone();
                        draft.write().event_type = value;
                    },
                }
                {date_draft_field(
                    loc,
                    "event-date",
                    editing,
                    current.date.clone(),
                    committed.date.clone(),
                    Callback::new(move |value: genealogy_ui::DateDraft| draft.write().date = value),
                    Callback::new(move |()| {
                        let value = seed.read().date.clone();
                        draft.write().date = value;
                    }),
                )}
                {event_place_edit_field(loc, ctx)}
                DraftText {
                    label: loc.field_label("description"),
                    name: "event-description".to_owned(),
                    editing,
                    value: current.description.clone(),
                    original: committed.description.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("description")),
                    oninput: move |value: String| draft.write().description = value,
                    onreset: move |()| {
                        let value = seed.read().description.clone();
                        draft.write().description = value;
                    },
                }
            }
        }
    }
}

/// The place types offered for an inline "new place" (a common subset; the model has more).
fn event_place_type_choices() -> [genealogy_app::PlaceType; 5] {
    use genealogy_app::PlaceType;
    [
        PlaceType::City,
        PlaceType::Town,
        PlaceType::Parish,
        PlaceType::Building,
        PlaceType::Country,
    ]
}

/// The event's place field in the whole-record editor: a read-first existing-place picker
/// ([`draft_picker_field`]). The collapsed selection is derived from the draft's place link (so
/// `use_record_edit` reseeds cleanly), and it is modified when the link differs from the committed one.
fn event_place_edit_field(loc: &Localizer, ctx: &EventEditCtx) -> Element {
    let record = ctx.record;
    let editing = record.editing.read().to_owned();
    let current = record.draft.read().place.clone();
    let committed = record.seed.read().place.clone();
    let selection = match &current {
        RecordLink::Existing(selection) => Some(selection.clone()),
        RecordLink::Empty | RecordLink::New(_) => None,
    };
    let modified = current.existing_id() != committed.existing_id();
    let view = DraftPickerView {
        editing,
        selection,
        modified,
    };
    draft_picker_field(loc, &ctx.place, &view, ctx.place_reset)
}

/// The event create form's field rows (`event.html`): a required Type select, a structured date, a
/// find-or-create Place picker (existing → a collapsed chip; "+ New" → an inline place [`draft_card`]),
/// and a Description. A pure fn (the picker's state/options/callbacks passed in) so SSR tests render it.
pub fn event_create_fields(
    loc: &Localizer,
    mut draft: Signal<genealogy_ui::EventDraft>,
    place: &RecordPicker,
) -> Element {
    let event_types = event_type_choices();
    let type_options: Vec<SelectChoice> = event_types
        .iter()
        .enumerate()
        .map(|(index, event_type)| SelectChoice {
            value: index.to_string(),
            label: loc.event_type_label(event_type),
        })
        .collect();
    let type_selected = event_types
        .iter()
        .position(|t| *t == draft().event_type)
        .unwrap_or(0)
        .to_string();
    rsx! {
        Card { title: loc.tab_label("overview"),
            div { class: "stack",
                Select {
                    label: loc.field_label("type"),
                    name: "event-type".to_owned(),
                    value: Some(type_selected),
                    options: type_options,
                    onchange: move |event: FormEvent| {
                        let types = event_type_choices();
                        if let Some(event_type) = event.value().parse::<usize>().ok().and_then(|index| types.get(index).cloned()) {
                            draft.write().event_type = event_type;
                        }
                    },
                }
                {date_draft_field(
                    loc,
                    "event-date",
                    true,
                    draft().date.clone(),
                    genealogy_ui::DateDraft::default(),
                    Callback::new(move |value: genealogy_ui::DateDraft| draft.write().date = value),
                    Callback::new(move |()| draft.write().date = genealogy_ui::DateDraft::default()),
                )}
                {event_place_create_field(loc, draft, place)}
                Input {
                    label: loc.field_label("description"),
                    name: "event-description".to_owned(),
                    value: draft().description.clone(),
                    oninput: move |event: FormEvent| draft.write().description = event.value(),
                }
            }
        }
    }
}

/// The event create form's place field: the record picker while unset or pointing at an existing
/// place, or an inline new-place [`draft_card`] (a type select + name input) once "+ New" is chosen.
fn event_place_create_field(loc: &Localizer, draft: Signal<genealogy_ui::EventDraft>, place: &RecordPicker) -> Element {
    match &draft().place {
        RecordLink::New(_) => {
            let title = loc.place_new_title();
            let discard = place.callbacks.onclear;
            let body = event_new_place_body(loc, draft);
            draft_card(
                &title,
                &loc.draft_card_badge(),
                loc.draft_card_discard(&title),
                discard,
                body,
            )
        }
        RecordLink::Empty | RecordLink::Existing(_) => record_picker(loc, place),
    }
}

/// The inline new-place fields inside the event create form's draft card: a place-type select and a
/// name input, both bound to the draft's new-place link.
fn event_new_place_body(loc: &Localizer, mut draft: Signal<genealogy_ui::EventDraft>) -> Element {
    let place_types = event_place_type_choices();
    let current = match &draft().place {
        RecordLink::New(fields) => fields.clone(),
        _ => NewPlaceFields::default(),
    };
    let (type_options, type_selected) = optional_enum_select(
        loc.record_unset(),
        &place_types,
        Some(&current.place_type),
        |place_type| loc.place_type_label(place_type),
    );
    rsx! {
        Select {
            label: loc.field_label("type"),
            name: "event-new-place-type".to_owned(),
            value: Some(type_selected),
            options: type_options,
            onchange: move |event: FormEvent| {
                let place_types = event_place_type_choices();
                if let Some(place_type) = event.value().parse::<usize>().ok().and_then(|index| place_types.get(index).cloned())
                    && let RecordLink::New(fields) = &mut draft.write().place
                {
                    fields.place_type = place_type;
                }
            },
        }
        Input {
            label: loc.field_label("name"),
            name: "event-new-place-name".to_owned(),
            value: current.name.clone(),
            oninput: move |event: FormEvent| {
                if let RecordLink::New(fields) = &mut draft.write().place {
                    fields.name = event.value();
                }
            },
        }
    }
}

/// Which event collection-row edit form (if any) the side panel is showing. The event's own scalar
/// record (id · type · date · place · description) is edited in place via the sticky header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventEditForm {
    /// Assert a participant — `None` adds a new one (person picker + role), `Some(row)` edits
    /// (supersedes) an existing participant's role (the person is fixed).
    Participant(Option<ParticipantVm>),
    /// Attach a citation by `human_id`.
    Citation,
    /// Attach a media object by `human_id`.
    Media,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected event: header, related-item tabs, editing side panel, toast.
#[component]
pub(crate) fn EventDetailPane(human_id: String) -> Element {
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
    let editing = use_signal(|| None::<EventEditForm>);
    let mut retract = use_signal(|| None::<RetractTarget>);
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
        async move { load_screen(services, Intent::ShowEvent { human_id }).await }
    });

    // The shared whole-record edit state, seeded from the loaded event (empty until it loads); it
    // reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::EventDetail(detail))) => genealogy_ui::EventDraft::from_detail(detail),
        _ => genealogy_ui::EventDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::EventDraft>(&seed);

    // The existing-place picker: its options load once, and pick/clear/reset drive the draft's place
    // link (inline place creation is create-only, so this picker never offers "+ New").
    let place_state = use_signal(genealogy_ui::PickerState::default);
    let place_services = services.clone();
    let place_rows = use_resource(move || {
        let services = place_services.clone();
        async move { load_picker_rows(services, Category::Places).await }
    });
    let mut place_draft = record.draft;
    let place_seed = record.seed;
    let place_onpick =
        use_callback(move |selection: PickerSelection| place_draft.write().place = RecordLink::Existing(selection));
    let place_onclear = use_callback(move |()| place_draft.write().place = RecordLink::Empty);
    let place_onnew = use_callback(move |_query: String| {});
    let place_reset = use_callback(move |()| {
        let place = place_seed.read().place.clone();
        place_draft.write().place = place;
    });

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the event's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::EventDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::Events,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (EventEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_event_edit(services, edit, prov).await {
                Ok(_) => {
                    editing_for_submit.set(None);
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
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::EventDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_event_edit).await;
            finish_record_save(effective, Category::Events, &current, record_nav, reload, toast, &saved);
        });
    });

    // A per-row Edit/Remove/Detach opens either a seeded form or the shared retract panel; confirming a
    // retract dispatches an `UndoAssertion` carrying the typed rationale (the note stays in History —
    // ADR 0004 §2).
    let on_retract = use_callback(move |(assertion_id, label, detach): (String, String, bool)| {
        retract_reason.set(String::new());
        retract.set(Some(RetractTarget {
            assertion_id,
            label,
            detach,
            person_human_id: None,
        }));
    });
    // A canonical person-origin participant on the Participants tab retracts against the Person aggregate.
    let on_person_retract = use_callback(
        move |(assertion_id, label, detach, person_human_id): (String, String, bool, String)| {
            retract_reason.set(String::new());
            retract.set(Some(RetractTarget {
                assertion_id,
                label,
                detach,
                person_human_id: Some(person_human_id),
            }));
        },
    );
    let mut editing_for_open = editing;
    let on_edit_open = use_callback(move |form: EventEditForm| editing_for_open.set(Some(form)));
    let retract_services = state.services().clone();
    let retract_human = human_id.clone();
    let retract_saved = state.data_loc().action_label("saved");
    let on_retract_confirm = use_callback(move |()| {
        let Some(target) = retract() else {
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
            let outcome = if let Some(person_human_id) = target.person_human_id {
                let edit = PersonEdit::UndoAssertion {
                    human_id: person_human_id,
                    assertion_id: target.assertion_id,
                };
                save_edit(services, edit, prov).await
            } else {
                let edit = EventEdit::UndoAssertion {
                    human_id,
                    assertion_id: target.assertion_id,
                };
                save_event_edit(services, edit, prov).await.map(|_| ())
            };
            match outcome {
                Ok(()) => {
                    retract.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    // ⌘Z retracts the newest undoable assertion of this record's loaded change log (WP5).
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::EventDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read() || retract.read().is_some());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let undo_human = human_id.clone();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((
            EventEdit::UndoAssertion {
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
        Some(ScreenData::Loaded(IntentOutcome::EventDetail(detail))) => {
            let loc = state.data_loc();
            let place = RecordPicker {
                config: PickerConfig {
                    label: loc.field_label("place"),
                    name: "event-place".to_owned(),
                    entity_label: loc.picker_entity(Category::Places),
                    allow_new: false,
                },
                state: place_state,
                options: picker_options(place_rows.read_unchecked().as_ref()),
                exclude: Vec::new(),
                callbacks: PickerCallbacks {
                    onpick: place_onpick,
                    onclear: place_onclear,
                    onnew: place_onnew,
                },
            };
            let ctx = EventEditCtx {
                record,
                place,
                place_reset,
            };
            event_detail(
                &state,
                detail,
                EventPane {
                    active,
                    side_edit: editing,
                    ctx,
                    retract,
                    retract_reason,
                },
                EventCallbacks {
                    on_submit,
                    on_record_save,
                    on_retract,
                    on_person_retract,
                    on_retract_confirm,
                    on_edit_open,
                },
                &human_id,
            )
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::PlaceDetail(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::DataQuality(_)
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

/// The signals an event's detail threads to its tabs: the active tab, the collection-row side panel,
/// and the whole-record edit context (edit state + place picker).
#[derive(Clone)]
struct EventPane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<EventEditForm>>,
    /// The whole-record (id · type · date · place · description) edit context.
    ctx: EventEditCtx,
    /// The row being retracted/detached, if the retract panel is open.
    retract: Signal<Option<RetractTarget>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The commit callbacks an event's detail wires in: one-command collection edits, the whole-record
/// save (the scalar edit via `edits_against`), and the per-row correction (edit-open + retract-confirm).
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "event-handler fields conventionally share the on_ prefix"
)]
struct EventCallbacks {
    /// Commits one [`EventEdit`] command (a collection row).
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::EventDraft, ProvenanceDraft)>,
    /// Opens the retract panel for an event-side row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Opens the retract panel for a canonical person-origin participant, routing the retract to the
    /// Person aggregate: `(assertion_id, label, detach, person_human_id)`.
    on_person_retract: Callback<(String, String, bool, String)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<EventEditForm>,
}

/// Renders a loaded event's detail container: header (with the sticky-header record Edit/Cancel/Save),
/// the tab strip, the active tab's content, and the collection-row side panel.
fn event_detail(
    state: &AppState,
    detail: &EventDetail,
    pane: EventPane,
    callbacks: EventCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let EventPane {
        active,
        side_edit: editing,
        ctx,
        retract,
        retract_reason,
    } = pane;
    let record = ctx.record;
    let on_submit = callbacks.on_submit;
    let on_record_save = callbacks.on_record_save;
    let on_retract = callbacks.on_retract;
    let on_person_retract = callbacks.on_person_retract;
    let on_retract_confirm = callbacks.on_retract_confirm;
    let on_edit_open = callbacks.on_edit_open;
    let tabs = event_tabs(detail, loc);
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
                avatar: "📅".to_owned(),
                extras: event_restriction_toggles(loc, detail, on_submit, human_id),
                actions: record_head_actions(&labels, record, rsx! {}, on_record_save),
                tabs: tab_items,
                active,
                {event_tab_content(state, detail, active_id, editing, &ctx, on_submit, on_retract, on_person_retract, on_edit_open, human_id)}
            }
            {event_edit_panel(state, editing, on_submit, human_id)}
            {event_retract_panel(loc, retract, retract_reason, on_retract_confirm)}
        }
    }
}

/// Renders the shared Retract/Detach side panel when an event collection row's action is armed. Reads
/// the armed `(assertion_id, label, detach)` and binds the rationale input; confirming dispatches
/// `UndoAssertion`. Closed (rendered empty) when nothing is armed. Never renders the target's
/// `AssertionId`.
fn event_retract_panel(
    loc: &Localizer,
    mut retract: Signal<Option<RetractTarget>>,
    reason: Signal<String>,
    on_confirm: Callback<()>,
) -> Element {
    let Some(RetractTarget { label, detach, .. }) = retract() else {
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

/// The interactive privacy-restriction toggles for an event (the mockup `resn-set`).
fn event_restriction_toggles(
    loc: &Localizer,
    detail: &EventDetail,
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
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
                on_submit.call((EventEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one event detail tab, with its contextual add/edit affordances.
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threads the pane's signals + callbacks"
)]
fn event_tab_content(
    state: &AppState,
    detail: &EventDetail,
    tab_id: &str,
    mut editing: Signal<Option<EventEditForm>>,
    ctx: &EventEditCtx,
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
    on_retract: Callback<(String, String, bool)>,
    on_person_retract: Callback<(String, String, bool, String)>,
    on_edit_open: Callback<EventEditForm>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "participants" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-participant"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EventEditForm::Participant(None))) }
            }
            {event_participants_table(loc, detail, on_edit_open, on_person_retract)}
        },
        "citations" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-citation"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EventEditForm::Citation)) }
            }
            {event_citations_table(loc, &detail.citations, on_retract)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EventEditForm::Media)) }
            }
            {family_media_gallery(loc, &detail.media, Some(on_retract))}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EventEditForm::Note)) }
            }
            {id_list(loc, &detail.notes, Some(on_retract))}
        },
        "tags" => event_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => event_history_tab(loc, detail, on_submit, human_id),
        _ => event_overview(loc, detail, ctx),
    }
}

/// The Overview tab, read-first (`record-editing.html` §1/§2): the event's scalar record (id · type ·
/// date · place · description) as read boxes plus provenance for the date claim. Entering edit mode
/// (via the sticky-header Edit) swaps the record fields to inputs and, while dirty, shows the
/// provenance block; the read-mode provenance cues are hidden in edit mode.
pub fn event_overview(loc: &Localizer, detail: &EventDetail, ctx: &EventEditCtx) -> Element {
    if ctx.record.editing.read().to_owned() {
        return rsx! {
            div { class: "section-note", "{loc.event_overview_note()}" }
            {event_record_fields(loc, ctx)}
            {record_edit_provenance(loc, ctx.record)}
        };
    }
    rsx! {
        div { class: "section-note", "{loc.event_overview_note()}" }
        div { class: "grid-2",
            {event_record_fields(loc, ctx)}
            Card { title: loc.field_label("value"),
                if let Some(description) = detail.description.clone() {
                    p { "{description}" }
                } else {
                    EmptyState { message: loc.tab_empty() }
                }
                if !detail.date_citations.is_empty() {
                    div { class: "fact-row", style: "margin-top:8px",
                        span { class: "field-label", style: "width:80px;margin:0", "{loc.field_label(\"date\")}" }
                        {provenance_cue(loc, loc.provenance_title_claim(&loc.field_label("date")), &detail.date_citations)}
                    }
                }
            }
        }
    }
}

/// The Participants tab: a row per participant with role, surety, and source columns.
///
/// Participation is owned solely by the Person aggregate (ADR 0019): every row edits the role via
/// [`EventEditForm::Participant`] (a supersede that lands on the Person aggregate) and Remove
/// retracts the person-side assertion (`on_person_retract`).
pub fn event_participants_table(
    loc: &Localizer,
    detail: &EventDetail,
    onedit: Callback<EventEditForm>,
    on_person_retract: Callback<(String, String, bool, String)>,
) -> Element {
    if detail.participants.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("name"),
                loc.field_label("role"),
                loc.field_label("confidence"),
                loc.field_label("source"),
                String::new(),
            ],
            for participant in detail.participants.iter() {
                {event_participant_row(loc, participant, onedit, on_person_retract)}
            }
        }
    }
}

/// One Participants-tab row. Participation is person-owned (ADR 0019), so Edit supersedes and Remove
/// retracts the person-side assertion.
fn event_participant_row(
    loc: &Localizer,
    participant: &ParticipantVm,
    onedit: Callback<EventEditForm>,
    on_person_retract: Callback<(String, String, bool, String)>,
) -> Element {
    let person_human_id = participant.human_id.clone();
    let retract_cb = Callback::new(move |(assertion_id, label, detach): (String, String, bool)| {
        on_person_retract.call((assertion_id, label, detach, person_human_id.clone()));
    });
    let edit = Some((
        EventEditForm::Participant(Some(participant.clone())),
        Some("edit-participation"),
    ));
    rsx! {
        tr {
            td { "{participant.name}" }
            td { Chip { label: participant.role_label.clone() } }
            td { ConfidenceBadge { level: participant.confidence, label: participant.confidence_label.clone() } }
            td { {source_cue(loc, participant.source_count)} }
            {row_actions_cell(
                loc,
                &participant.name,
                edit, None,
                Some(RowRetract { assertion_id: participant.assertion_id.clone(), button_label: "remove", title: "remove-participant", detach: false }),
                Some(onedit),
                retract_cb)}
        }
    }
}

/// The event Citations tab: each backing citation's source, page, surety, and Evidence Explained
/// axes, plus a per-row Detach (retracts the attach assertion — it stays in History). A citation with
/// no attach `AssertionId` (shown as evidence, not an attachment) renders no Detach.
pub fn event_citations_table(
    loc: &Localizer,
    citations: &[CitationRefVm],
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if citations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("source"),
                loc.field_label("page"),
                loc.field_label("confidence"),
                loc.field_label("evidence"),
                String::new(),
            ],
            for citation in citations.iter() {
                tr {
                    td {
                        if let Some(source_id) = &citation.source_id {
                            super::shared::RecordLink {
                                category: Category::Sources,
                                human_id: source_id.clone(),
                                label: citation.source.clone().unwrap_or_else(|| source_id.clone()),
                            }
                        } else {
                            {citation.source.clone().unwrap_or_else(|| citation.human_id.clone())}
                        }
                    }
                    td { class: "muted", {citation.page.clone().unwrap_or_else(|| "—".to_owned())} }
                    td {
                        if let (Some(level), Some(label)) = (citation.confidence, citation.confidence_label.clone()) {
                            ConfidenceBadge { level, label }
                        } else {
                            span { class: "muted", "—" }
                        }
                    }
                    td { class: "wrap",
                        for chip in citation.evidence_axes.iter() {
                            EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() }
                        }
                    }
                    {row_actions_cell::<EventEditForm>(
                        loc,
                        &citation.human_id,
                        None, None,
                        citation.assertion_id.clone().map(|id| RowRetract { assertion_id: id, button_label: "detach", title: "detach-citation", detach: true }),
                        None,
                        onretract)}
                }
            }
        }
    }
}

/// The event Tags tab: each applied tag as a colour-dot chip (name + colour, never id) with remove.
pub fn event_tags_panel(
    loc: &Localizer,
    detail: &EventDetail,
    mut editing: Signal<Option<EventEditForm>>,
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EventEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call((EventEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The event History tab: the per-record audit timeline, each undoable entry carrying an undo control.
fn event_history_tab(
    loc: &Localizer,
    detail: &EventDetail,
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
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
                on_submit.call((EventEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The event editing side panel: renders the form for the open [`EventEditForm`], or nothing.
fn event_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<EventEditForm>>,
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match &form {
        EventEditForm::Participant(None) => loc.action_label("add-participant"),
        EventEditForm::Participant(Some(_)) => loc.panel_title("edit-participation"),
        EventEditForm::Citation => loc.action_label("attach-citation"),
        EventEditForm::Media => loc.action_label("attach-media"),
        EventEditForm::Note => loc.action_label("attach-note"),
        EventEditForm::Tag => loc.action_label("add-tag"),
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
                EventEditForm::Participant(seed) => rsx! { EventAddParticipantForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Citation => rsx! { EventAttachForm { human_id, field: "citation".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Media => rsx! { EventAttachForm { human_id, field: "media".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Note => rsx! { EventAttachForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Tag => rsx! { EventTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The participant form → [`EventEdit::AddParticipant`]. `seed: None` adds a new participant (a People
/// picker + a role select); `Some(row)` edits an existing participant's role — the person is fixed
/// (shown as a link), the role select is pre-filled, and the provenance draft's `supersedes` is seeded
/// with the row's assertion id so Save supersedes (replaces) rather than appends (ADR 0004 §2).
#[component]
fn EventAddParticipantForm(
    human_id: String,
    seed: Option<ParticipantVm>,
    onsubmit: EventHandler<(EventEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let roles = participant_role_choices();
    let seed_index = seed
        .as_ref()
        .and_then(|row| roles.iter().position(|role| *role == row.role))
        .unwrap_or(0);
    let options: Vec<SelectChoice> = roles
        .iter()
        .enumerate()
        .map(|(position, role)| SelectChoice {
            value: position.to_string(),
            label: loc.participant_role_label(role),
        })
        .collect();
    // Edit mode fixes the person (only the role changes); add mode offers an existing-person picker.
    let fixed_person = seed.as_ref().map(|row| row.human_id.clone());
    let picker = use_existing_picker(
        services,
        Category::People,
        loc.field_label("name"),
        "participant".to_owned(),
        loc.picker_entity(Category::People),
        Vec::new(),
    );
    let mut role = use_signal(|| seed_index);
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|row| row.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let extra = rsx! {
        Select {
            label: loc.field_label("role"),
            name: "role".to_owned(),
            value: Some(seed_index.to_string()),
            options,
            onchange: move |event: FormEvent| role.set(event.value().parse::<usize>().unwrap_or(0)),
        }
    };
    let picker_for_save = picker.clone();
    let fixed_for_save = fixed_person.clone();
    let onsave = use_callback(move |()| {
        let Some(person_id) = fixed_for_save.clone().or_else(|| picker_selection_id(&picker_for_save)) else {
            return;
        };
        let role = participant_role_choices()
            .get(role())
            .cloned()
            .unwrap_or(ParticipantRole::Primary);
        onsubmit.call((
            EventEdit::AddParticipant {
                human_id: human_id.clone(),
                person_id,
                role,
            },
            prov(),
        ));
    });
    if let Some(person) = &fixed_person {
        rsx! {
            div { class: "field",
                label { "{loc.field_label(\"name\")}" }
                super::shared::RecordLink { category: Category::People, human_id: person.clone(), label: person.clone() }
            }
            {extra}
            {provenance_block(loc, prov)}
            Button {
                label: loc.action_label("save"),
                variant: ButtonVariant::Primary,
                onclick: move |_| onsave.call(()),
            }
        }
    } else {
        attach_picker_form(loc, &picker, extra, prov, onsave)
    }
}

/// The "Attach citation/media/note by id" form → the matching [`EventEdit`] attach variant.
#[component]
fn EventAttachForm(human_id: String, field: String, onsubmit: EventHandler<(EventEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let category = match field.as_str() {
        "citation" => Category::Citations,
        "note" => Category::Notes,
        _ => Category::Media,
    };
    let picker = use_existing_picker(
        services,
        category,
        loc.field_label(&field),
        field.clone(),
        loc.picker_entity(category),
        Vec::new(),
    );
    let prov = use_signal(ProvenanceDraft::default);
    let picker_for_save = picker.clone();
    let onsave = use_callback(move |()| {
        let Some(id) = picker_selection_id(&picker_for_save) else {
            return;
        };
        let edit = match field.as_str() {
            "citation" => EventEdit::AttachCitation {
                human_id: human_id.clone(),
                citation_id: id,
            },
            "note" => EventEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            },
            _ => EventEdit::AttachMedia {
                human_id: human_id.clone(),
                media_id: id,
            },
        };
        onsubmit.call((edit, prov()));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
}

/// The event "Add tag" form: a picker of existing tags by name → [`EventEdit::Tag`].
#[component]
fn EventTagForm(human_id: String, onsubmit: EventHandler<(EventEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((EventEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

/// The participant roles offered by the "Add participant" form (a common subset; the model has more).
fn participant_role_choices() -> [ParticipantRole; 6] {
    [
        ParticipantRole::Primary,
        ParticipantRole::Witness,
        ParticipantRole::Officiator,
        ParticipantRole::Spouse,
        ParticipantRole::Godparent,
        ParticipantRole::Multiple,
    ]
}

/// The event types offered by the type picker (a common subset; the model has more).
fn event_type_choices() -> [EventType; 8] {
    [
        EventType::Birth,
        EventType::Death,
        EventType::Marriage,
        EventType::Baptism,
        EventType::Burial,
        EventType::Census,
        EventType::Residence,
        EventType::Immigration,
    ]
}
