use super::prelude::*;
use vitni_app::EventType;
// The record-link view-model enum (event place); shadows the prelude's `RecordLink` link component
// (the participant edit form reaches it as `super::shared::RecordLink`). `ParticipantVm` seeds the
// per-row participant edit.
use vitni_ui::{ParticipantVm, RecordLink};

/// The create-mode event record: an uncommitted [`EventDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). The type is required; a "new place" selection creates a
/// place inline on Save (§6b cascade). Save commits the whole event; Cancel discards.
#[component]
pub fn EventCreateRecord(draft_id: DraftId) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<vitni_ui::EventDraft>(Category::Events, draft_id);
    let mut draft = record.draft;
    // The find-or-create place picker: its options refetch after any mutation (#266); pick/clear/
    // "+ New" drive the draft's link.
    let place_state = use_signal(vitni_ui::PickerState::default);
    let place_services = services.clone();
    let place_rows = use_resource(move || {
        let _ = data_version_ticket(Some(nav));
        let services = place_services.clone();
        async move { load_picker_rows(services, Category::Places).await }
    });
    let place_onpick =
        use_callback(move |selection: PickerSelection| draft.write().place = RecordLink::Existing(selection));
    let place_onclear = use_callback(move |()| draft.write().place = RecordLink::Empty);
    let place_onnew =
        use_callback(move |_query: String| draft.write().place = RecordLink::New(NewPlaceFields::default()));
    let created_label = loc.action_label(ActionLabel::Created);
    let on_save = use_callback(move |(draft, prov): (vitni_ui::EventDraft, ProvenanceDraft)| {
        let request = draft.to_request();
        let services = services.clone();
        let created = created_label.clone();
        spawn(async move {
            let committed = commit_event_change_set(services, request, prov).await;
            finish_draft_commit(
                committed,
                DraftCommit::new(Category::Events, draft_id, &draft, created),
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
    use_save_on_request(EditKey::draft(Category::Events, draft_id), record, save_now);
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
    pub record: RecordEditState<vitni_ui::EventDraft>,
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
                    label_width: RECORD_LABEL_WIDTH,
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
                    label_width: RECORD_LABEL_WIDTH,
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
                    RECORD_LABEL_WIDTH,
                    DateFieldBinding {
                        value: current.date.clone(),
                        original: committed.date.clone(),
                        onchange: Callback::new(move |value: vitni_ui::DateDraft| draft.write().date = value),
                        onreset: Callback::new(move |()| {
                            let value = seed.read().date.clone();
                            draft.write().date = value;
                        }),
                    },
                )}
                {event_place_edit_field(loc, ctx)}
                DraftText {
                    label: loc.field_label("description"),
                    name: "event-description".to_owned(),
                    label_width: RECORD_LABEL_WIDTH,
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
                {record_restrictions_field(loc, record, RECORD_LABEL_WIDTH)}
            }
        }
    }
}

/// The place types offered for an inline "new place" — [`vitni_ui::NEW_PLACE_TYPES`], the same list
/// the Places category's own create form and the find-or-create attach card's Place body offer, so the
/// choices never disagree by entry point.
fn event_place_type_choices() -> [vitni_app::PlaceType; 9] {
    vitni_ui::NEW_PLACE_TYPES
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
pub fn event_create_fields(loc: &Localizer, mut draft: Signal<vitni_ui::EventDraft>, place: &RecordPicker) -> Element {
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
                    DEFAULT_LABEL_WIDTH,
                    DateFieldBinding {
                        value: draft().date.clone(),
                        original: vitni_ui::DateDraft::default(),
                        onchange: Callback::new(move |value: vitni_ui::DateDraft| draft.write().date = value),
                        onreset: Callback::new(move |()| draft.write().date = vitni_ui::DateDraft::default()),
                    },
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
fn event_place_create_field(loc: &Localizer, draft: Signal<vitni_ui::EventDraft>, place: &RecordPicker) -> Element {
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
fn event_new_place_body(loc: &Localizer, mut draft: Signal<vitni_ui::EventDraft>) -> Element {
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
    /// Postal address — `None` adds a new one, `Some(card)` edits (supersedes) an existing one. Boxed
    /// to keep the enum's variants close in size (`AddressVm` carries the full postal address).
    Address(Option<Box<AddressVm>>),
    /// Assert a participant — `None` adds a new one (person picker + role), `Some(row)` edits
    /// (supersedes) an existing participant's role (the person is fixed). Boxed to keep the enum's
    /// variants close in size (`ParticipantVm` carries the participant-scoped payload).
    Participant(Option<Box<ParticipantVm>>),
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
    let active = use_detail_tab(Category::Events, &human_id);
    let editing = use_signal(|| None::<EventEditForm>);
    // The shared commit path (`screens/detail_commits.rs`) — all of it but the retract confirm, which
    // this pane owns (see `on_retract_confirm` below): a participation is person-canonical, so
    // retracting one dispatches a `PersonEdit` against that person, not an `EventEdit`.
    let DetailCommits {
        mut reload,
        mut retract,
        mut retract_reason,
        on_submit,
        on_undo,
        on_tag_remove,
        on_retract: on_row_retract,
        ..
    } = use_detail_commits::<EventCommits, EventEditForm>(&state, &human_id, editing);
    // A canonical person-origin participant's retract targets the Person aggregate instead of this
    // event; set alongside `retract` only for that case (`on_person_retract`), cleared with it.
    let mut retract_person = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label(ActionLabel::Saved);

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
        Some(ScreenData::Loaded(IntentOutcome::EventDetail(detail))) => vitni_ui::EventDraft::from_detail(detail),
        _ => vitni_ui::EventDraft::new(),
    };
    let record = use_record_edit::<vitni_ui::EventDraft>(Category::Events, &human_id, &seed);

    // The existing-place picker: its options refetch after any mutation (#266), and pick/clear/reset
    // drive the draft's place link (inline place creation is create-only, so this picker never offers
    // "+ New").
    let place_state = use_signal(vitni_ui::PickerState::default);
    let place_services = services.clone();
    let place_rows = use_resource(move || {
        let _ = data_version_ticket(Some(nav));
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
            vitni_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let record_services = services.clone();
    let record_nav = nav;
    let current_id = human_id.clone();
    let on_record_save = use_callback(move |(draft, prov): (vitni_ui::EventDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            // The future captures a `Localizer` (grown by the ADR 0027 surety-override field), so it
            // is boxed to keep it off the async-fn/generator's inline stack frame (clippy large_futures).
            let effective = Box::pin(apply_record_edits(
                services,
                edits,
                prov,
                current.clone(),
                save_event_edit,
            ))
            .await;
            finish_record_save(effective, Category::Events, &current, record_nav, reload, &saved);
        });
    });

    // The shared arming (a per-row Edit/Remove/Detach opens either a seeded form or the retract panel),
    // plus this pane's own step: clearing any person-canonical target a previous `on_person_retract`
    // left armed, so the confirm below dispatches against the event.
    let on_retract = use_callback(move |row: (String, String, bool)| {
        retract_person.set(None);
        on_row_retract.call(row);
    });
    // A canonical person-origin participant on the Participants tab retracts against the Person aggregate.
    let on_person_retract = use_callback(
        move |(assertion_id, label, detach, person_human_id): (String, String, bool, String)| {
            retract_reason.set(String::new());
            retract_person.set(Some(person_human_id));
            retract.set(Some(RetractTarget {
                subject: RetractSubject::Assertion { assertion_id, detach },
                label,
            }));
        },
    );
    let mut editing_for_open = editing;
    let on_edit_open = use_callback(move |form: EventEditForm| editing_for_open.set(Some(form)));

    // This pane's own retract confirm, not the shared one: a participation is person-canonical
    // (data-model §5), so retracting a canonical participant's row dispatches a `PersonEdit` against
    // that person while every other row retracts against the event. An untag is never person-canonical,
    // so it takes the same shape here as on the eleven panes the shared confirm serves (issue #315).
    let retract_services = state.services().clone();
    let retract_human = human_id.clone();
    let retract_saved = state.data_loc().action_label(ActionLabel::Saved);
    let mut retract_nav = nav;
    let on_retract_confirm = use_callback(move |()| {
        let Some(RetractTarget { subject, .. }) = retract() else {
            return;
        };
        let services = retract_services.clone();
        let human_id = retract_human.clone();
        let saved = retract_saved.clone();
        let prov = ProvenanceDraft {
            rationale: retract_reason(),
            ..ProvenanceDraft::default()
        };
        let person_human_id = retract_person();
        spawn(async move {
            let outcome = match subject {
                RetractSubject::Assertion { assertion_id, .. } => {
                    if let Some(person_human_id) = person_human_id {
                        let edit = PersonEdit::UndoAssertion {
                            human_id: person_human_id,
                            assertion_id,
                        };
                        save_person_edit(services, edit, prov).await
                    } else {
                        let edit = EventEdit::UndoAssertion { human_id, assertion_id };
                        save_event_edit(services, edit, prov).await
                    }
                }
                RetractSubject::Tag { tag_id } => {
                    let edit = EventEdit::Tag {
                        human_id,
                        tag_id,
                        remove: true,
                    };
                    save_event_edit(services, edit, prov).await
                }
            };
            match outcome {
                Ok(_) => {
                    retract.set(None);
                    retract_person.set(None);
                    reload += 1;
                    retract_nav.notify(saved);
                }
                Err(message) => retract_nav.notify_error(message),
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
    use_record_undo(
        nav,
        Category::Events,
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
    use_save_on_request(EditKey::saved(Category::Events, &human_id), record, save_now);

    // The Media tab's crop viewer: opening a card, and superseding its crop via `SetMediaRegion`.
    let media_viewing = use_signal(|| None::<MediaRefVm>);
    let on_view = use_callback(move |item: MediaRefVm| media_viewing.clone().set(Some(item)));
    let region_human = human_id.clone();
    let on_region = use_callback(
        move |(assertion_id, crop, caption): (String, Option<Rect>, Option<String>)| {
            on_submit.call((
                EventEdit::SetMediaRegion {
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
                &EventCallbacks {
                    on_submit,
                    on_record_save,
                    on_retract,
                    on_person_retract,
                    on_retract_confirm,
                    on_edit_open,
                    on_undo,
                    on_tag_remove,
                    media_state,
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
            | IntentOutcome::MergeCompare(_)
            | IntentOutcome::ResearchNoteDetail(_)
            | IntentOutcome::Geography(_),
        )) => rsx! {},
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
struct EventCallbacks {
    /// Commits one [`EventEdit`] command (a collection row).
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(vitni_ui::EventDraft, ProvenanceDraft)>,
    /// Opens the retract panel for an event-side row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Opens the retract panel for a canonical person-origin participant, routing the retract to the
    /// Person aggregate: `(assertion_id, label, detach, person_human_id)`.
    on_person_retract: Callback<(String, String, bool, String)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<EventEditForm>,
    /// Retracts an assertion by id from the History tab (dispatches `UndoAssertion`).
    on_undo: Callback<String>,
    /// Arms the untag panel for a tag chip's ×: `(tag_id, tag name)`.
    on_tag_remove: Callback<(String, String)>,
    /// The Media tab's viewer state + crop-supersede wiring.
    media_state: MediaTabState,
}

/// Renders a loaded event's detail container: header (with the sticky-header record Edit/Cancel/Save),
/// the tab strip, the active tab's content, and the collection-row side panel.
fn event_detail(
    state: &AppState,
    detail: &EventDetail,
    pane: EventPane,
    callbacks: &EventCallbacks,
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
    let on_undo = callbacks.on_undo;
    let on_tag_remove = callbacks.on_tag_remove;
    let media_state = callbacks.media_state;
    let tabs = event_tabs(detail, loc);
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
                avatar: "📅".to_owned(),
                extras: restriction_display(loc, &detail.restrictions),
                actions: record_head_actions(&labels, record, rsx! {}, on_record_save),
                tabs: tab_items,
                active,
                {event_tab_content(state, detail, &active_tab, editing, &ctx, on_retract, on_person_retract, on_edit_open, on_undo, on_tag_remove, media_state)}
            }
            {event_edit_panel(state, editing, on_submit, human_id)}
            {retract_side_panel(loc, retract, retract_reason, on_retract_confirm, "detach-citation")}
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
    tab: &DetailTab,
    editing: Signal<Option<EventEditForm>>,
    ctx: &EventEditCtx,
    on_retract: Callback<(String, String, bool)>,
    on_person_retract: Callback<(String, String, bool, String)>,
    on_edit_open: Callback<EventEditForm>,
    on_undo: Callback<String>,
    on_tag_remove: Callback<(String, String)>,
    media_state: MediaTabState,
) -> Element {
    let loc = state.data_loc();
    match tab.id {
        "addresses" => {
            let onedit =
                Callback::new(move |seed: AddressVm| on_edit_open.call(EventEditForm::Address(Some(Box::new(seed)))));
            tab_frame(
                loc,
                tab,
                TabActionTarget::Form(editing, EventEditForm::Address(None)),
                None,
                rsx! {
                    {address_cards(loc, &detail.addresses, onedit, on_retract)}
                },
            )
        }
        "participants" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, EventEditForm::Participant(None)),
            None,
            rsx! {
                {event_participants_table(loc, detail, on_edit_open, on_person_retract)}
            },
        ),
        "citations" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, EventEditForm::Citation),
            None,
            rsx! {
                {citations_table::<EventEditForm>(loc, &detail.citations, false, on_retract)}
            },
        ),
        "media" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, EventEditForm::Media),
            None,
            rsx! {
                {media_tab(loc, &detail.media, Some(on_retract), media_state)}
            },
        ),
        "notes" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, EventEditForm::Note),
            None,
            rsx! {
                {notes_table(loc, &detail.notes, Some(on_retract))}
            },
        ),
        "tags" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, EventEditForm::Tag),
            Some(TabActionStyle {
                emphasis: Some(ButtonVariant::Ghost),
                ..Default::default()
            }),
            tags_panel(loc, &detail.tags, on_tag_remove),
        ),
        "research-notes" => rsx! {
            ResearchNotesTab {
                tab: tab.clone(),
                category: Category::Events,
                human_id: detail.human_id.clone(),
                rows: detail.research_notes.clone(),
            }
        },
        "history" => tab_frame::<()>(
            loc,
            tab,
            TabActionTarget::None,
            None,
            history_panel(loc, &detail.history, Some(on_undo)),
        ),
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
            caption: loc.tab_label("participants"),
            headers: vec![
                loc.field_label("name"),
                loc.field_label("role"),
                loc.field_label("age"),
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
        EventEditForm::Participant(Some(Box::new(participant.clone()))),
        Some("edit-participation"),
    ));
    rsx! {
        tr {
            td { "{participant.name}" }
            td { Chip { label: participant.role_label.clone() } }
            td { class: "muted", {or_dash(participant.age_label.clone())} }
            td { ConfidenceBadge { level: participant.confidence, label: participant.confidence_label.clone() } }
            td { {source_cue(loc, participant.source_count)} }
            {row_actions_cell(
                loc,
                &participant.name,
                edit, None,
                Some(RowRetract { assertion_id: participant.assertion_id.clone(), button_label: RowVerb::Remove, title: "remove-participant", detach: false }),
                Some(onedit),
                retract_cb)}
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
        EventEditForm::Address(None) => loc.action_label(ActionLabel::AddAddress),
        EventEditForm::Address(Some(_)) => loc.panel_title("edit-address"),
        EventEditForm::Participant(None) => loc.action_label(ActionLabel::AddParticipant),
        EventEditForm::Participant(Some(_)) => loc.panel_title("edit-participation"),
        EventEditForm::Citation => loc.action_label(ActionLabel::AttachCitation),
        EventEditForm::Media => loc.action_label(ActionLabel::AttachMedia),
        EventEditForm::Note => loc.action_label(ActionLabel::AttachNote),
        EventEditForm::Tag => loc.action_label(ActionLabel::AddTag),
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
                EventEditForm::Address(seed) => rsx! {
                    AddressForm {
                        seed: seed.map(|card| *card),
                        onsubmit: move |(address, prov): (Address, ProvenanceDraft)| {
                            on_submit.call((EventEdit::AddAddress { human_id: human_id.clone(), address }, prov));
                        },
                    }
                },
                EventEditForm::Participant(seed) => rsx! { EventAddParticipantForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Citation => rsx! { EventAttachForm { human_id, field: "citation".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Media => rsx! { EventAttachForm { human_id, field: "media".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Note => rsx! { EventAttachForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Tag => rsx! { EventTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The participant form → [`EventEdit::AddParticipant`], at full person-screen parity (role · age ·
/// attributes · notes · provenance — ADR 0019) via the shared [`participation_form`] body. `seed: None`
/// adds a new participant (an existing-person picker above the shared form); `Some(row)` edits an
/// existing participant — the person is fixed (shown as a link), the shared form is pre-filled from the
/// row, and the draft's `supersedes` is the row's assertion id so Save supersedes rather than appends
/// (ADR 0004 §2). Either way the write lands on the Person aggregate (the canonical participation owner).
#[component]
fn EventAddParticipantForm(
    human_id: String,
    seed: Option<Box<ParticipantVm>>,
    onsubmit: EventHandler<(EventEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    // Edit mode fixes the person (only the participation changes); add mode offers a find-or-create picker.
    let fixed_person = seed.as_ref().map(|row| row.human_id.clone());
    let attach = use_attach_picker(
        services.clone(),
        Category::People,
        loc.field_label("name"),
        "participant".to_owned(),
        loc.picker_entity(Category::People),
        Vec::new(),
    );
    let participation_seed = seed
        .as_ref()
        .map_or_else(ParticipationSeed::empty, |row| ParticipationSeed {
            role: row.role.clone(),
            age: row.age.clone(),
            attributes: row.attributes.clone(),
            notes: row.notes.clone(),
            supersedes: Some(row.assertion_id.clone()),
        });
    // The participation's own provenance draft lives inside `ParticipationForm`; `prov` here only
    // relays it to `use_attach_save` at submit time (so a "+ New person" create shares the same
    // operator "why" as the participation it is being attached to — `record-editing.html` §5b), and
    // `pending` carries the submitted fields across to `onattach`, which fires once the person id is
    // resolved (synchronously for an existing pick, after the create commits for a "+ New …" draft).
    let mut prov = use_signal(ProvenanceDraft::default);
    let mut pending = use_signal(|| None::<NewParticipation>);
    let human_id_for_attach = human_id.clone();
    let onattach = use_callback(move |person_id: String| {
        let Some(fields) = pending.write().take() else {
            return;
        };
        onsubmit.call((
            EventEdit::AddParticipant {
                human_id: human_id_for_attach.clone(),
                person_id,
                role: fields.role,
                age: fields.age,
                attributes: fields.attributes,
                notes: fields.notes,
            },
            prov(),
        ));
    });
    let onsave = use_attach_save(services, &attach, prov, onattach);
    rsx! {
        if let Some(person) = &fixed_person {
            div { class: "field",
                label { "{loc.field_label(\"name\")}" }
                super::shared::RecordLink { category: Category::People, human_id: person.clone(), label: person.clone() }
            }
        } else {
            {attach_link_field(loc, &attach)}
        }
        ParticipationForm {
            seed: participation_seed,
            onsubmit: move |(fields, incoming_prov): (NewParticipation, ProvenanceDraft)| {
                if let Some(person_id) = fixed_person.clone() {
                    onsubmit.call((
                        EventEdit::AddParticipant {
                            human_id: human_id.clone(),
                            person_id,
                            role: fields.role,
                            age: fields.age,
                            attributes: fields.attributes,
                            notes: fields.notes,
                        },
                        incoming_prov,
                    ));
                    return;
                }
                prov.set(incoming_prov);
                pending.set(Some(fields));
                onsave.call(());
            },
        }
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
    let attach = use_attach_picker(
        services.clone(),
        category,
        loc.field_label(&field),
        field.clone(),
        loc.picker_entity(category),
        Vec::new(),
    );
    let prov = use_signal(ProvenanceDraft::default);
    let onattach = use_callback(move |id: String| {
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
    let onsave = use_attach_save(services, &attach, prov, onattach);
    attach_link_form(loc, &attach, rsx! {}, prov, onsave)
}

/// The event "Add tag" form: a picker of existing tags by name → [`EventEdit::Tag`].
#[component]
fn EventTagForm(human_id: String, onsubmit: EventHandler<(EventEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((EventEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

/// The event types offered by the type picker — [`vitni_ui::NEW_EVENT_TYPES`], the same list the
/// find-or-create attach card's Event body offers.
fn event_type_choices() -> [EventType; 8] {
    vitni_ui::NEW_EVENT_TYPES
}
