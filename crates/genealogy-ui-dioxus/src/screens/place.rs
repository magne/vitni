use genealogy_app::{PlaceGeometry, PlaceType};
// The place row view-models the prelude doesn't re-export; they seed the per-row Name / enclosing edits.
use genealogy_ui::{
    DateDraft, MarkerShapeVm, PickerState, PlaceGeometryVm, PlaceHierarchyVm, PlaceNameVm, place_map_display_shape,
};

use super::geography::geography_time_slider;
use super::map_shared::{
    DEFAULT_CENTER, DrawTool, GeometrySaveForm, MapDraft, events_geojson, fit_bounds, geo_point, map_surface,
    markers_geojson, push_map_data, push_map_draft, shape_to_draft,
};
use super::prelude::*;

/// The create-mode place record: an uncommitted [`PlaceDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). Save commits the whole place; Cancel discards. Save is
/// blocked while the coordinate pair is half-filled or unparseable (§7).
#[component]
pub fn PlaceCreateRecord() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::PlaceDraft>(Category::Places);
    let mut draft = record.draft;
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::PlaceDraft, ProvenanceDraft)| {
        let Some(request) = draft.to_request() else {
            return;
        };
        let label = request.name.clone().unwrap_or_default();
        let services = services.clone();
        spawn(async move {
            let committed = commit_place_change_set(services, request, prov).await;
            finish_draft_commit(committed, Category::Places, Some(label), nav);
        });
    });
    // The close/quit confirm's Save runs this same commit (issue #240), so a ⌘W/⌘Q over a half-filled
    // create form can keep the draft instead of losing it.
    let save_now = use_callback(move |()| {
        if record.can_save() {
            on_save.call((record.draft.read().clone(), record.prov.read().clone()));
        }
    });
    use_save_on_request(Category::Places, None, record, save_now);
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| nav.cancel_draft(Category::Places) }
        Button {
            label: loc.action_label("save"),
            variant: ButtonVariant::Primary,
            small: true,
            disabled: !can_save,
            onclick: move |_| save_now.call(()),
        }
    };
    create_record_frame(
        &loc.place_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {place_record_fields(loc, record, None)}
            Input {
                label: loc.field_label("name"),
                name: "place-name".to_owned(),
                value: draft().name.clone(),
                oninput: move |event: FormEvent| draft.write().name = event.value(),
            }
            {record_edit_provenance(loc, record)}
        },
    )
}

/// The place's scalar record fields (id · type · latitude · longitude · code), read-first: read boxes
/// in view mode, inputs with per-field reset in edit mode (`record-editing.html` §2/§3). The primary
/// name is not a scalar here — on an existing place it is the Names collection, and the create pane
/// adds its own Name field. Latitude/longitude flag an invalid pair inline (§7). A pure fn (the edit
/// state's signals passed in) so the create pane and SSR tests render it without `AppCtx`. In view mode
/// a `Some(detail)` surfaces the coordinate and code provenance cues; the create pane passes `None`.
pub fn place_record_fields(
    loc: &Localizer,
    record: RecordEditState<genealogy_ui::PlaceDraft>,
    detail: Option<&PlaceDetail>,
) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let types = place_type_choices();
    let options: Vec<SelectChoice> = types
        .iter()
        .enumerate()
        .map(|(index, place_type)| SelectChoice {
            value: index.to_string(),
            label: loc.place_type_label(place_type),
        })
        .collect();
    let index_of = |place_type: &PlaceType| {
        place_type_choices()
            .iter()
            .position(|t| t == place_type)
            .unwrap_or(0)
            .to_string()
    };
    let current = draft();
    let committed = seed.read().clone();
    rsx! {
        Card { title: loc.field_label("place"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "place-id".to_owned(),
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
                    name: "place-type".to_owned(),
                    editing,
                    value: index_of(&current.place_type),
                    original: index_of(&committed.place_type),
                    reset_label: loc.action_reset_field(&loc.field_label("type")),
                    options,
                    onchange: move |value: String| {
                        let types = place_type_choices();
                        if let Some(place_type) = value.parse::<usize>().ok().and_then(|index| types.get(index).cloned()) {
                            draft.write().place_type = place_type;
                        }
                    },
                    onreset: move |()| {
                        let value = seed.read().place_type.clone();
                        draft.write().place_type = value;
                    },
                }
                {place_coordinate_fields(loc, editing, draft, seed, detail)}
                DraftText {
                    label: loc.field_label("code"),
                    name: "place-code".to_owned(),
                    editing,
                    value: current.code.clone(),
                    original: committed.code.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("code")),
                    mono: true,
                    oninput: move |value: String| draft.write().code = value,
                    onreset: move |()| {
                        let value = seed.read().code.clone();
                        draft.write().code = value;
                    },
                }
                if !editing {
                    if let Some(detail) = detail {
                        if detail.code.is_some() {
                            {scalar_provenance_row(loc, &loc.field_label("code"), detail.code_confidence, detail.code_confidence_label.clone(), &detail.code_citations)}
                        }
                    }
                }
            }
        }
    }
}

/// The place's latitude/longitude record fields, each flagging an invalid or half-filled pair inline
/// (`record-editing.html` §7). Split out of [`place_record_fields`] to keep that fn within its line
/// budget. In view mode a `Some(detail)` overrides the read-box values with `display_coordinates`'s
/// resolved-geometry-first point (falling back to the scalar draft when the place has no geometry
/// assertion) and renders the coordinate provenance cue (confidence badge + the "Why we believe"
/// popover) beneath the pair, mirroring the Person overview's sourced facts. Edit mode always shows and
/// commits the scalar draft — `display_coordinates` only changes what view mode *displays*, never what
/// Save writes.
fn place_coordinate_fields(
    loc: &Localizer,
    editing: bool,
    mut draft: Signal<genealogy_ui::PlaceDraft>,
    seed: Signal<genealogy_ui::PlaceDraft>,
    detail: Option<&PlaceDetail>,
) -> Element {
    let current = draft();
    let committed = seed.read().clone();
    let coordinate_error = loc.place_coordinate_invalid();
    let displayed = (!editing)
        .then(|| detail.and_then(genealogy_ui::display_coordinates))
        .flatten();
    let latitude_value = displayed.map_or_else(|| current.latitude.clone(), |(lat, _)| format!("{lat:.6}"));
    let longitude_value = displayed.map_or_else(|| current.longitude.clone(), |(_, lon)| format!("{lon:.6}"));
    rsx! {
        DraftText {
            label: loc.field_label("latitude"),
            name: "place-latitude".to_owned(),
            editing,
            value: latitude_value,
            original: committed.latitude.clone(),
            reset_label: loc.action_reset_field(&loc.field_label("latitude")),
            error: current.latitude_invalid().then(|| coordinate_error.clone()),
            oninput: move |value: String| draft.write().latitude = value,
            onreset: move |()| {
                let value = seed.read().latitude.clone();
                draft.write().latitude = value;
            },
        }
        DraftText {
            label: loc.field_label("longitude"),
            name: "place-longitude".to_owned(),
            editing,
            value: longitude_value,
            original: committed.longitude.clone(),
            reset_label: loc.action_reset_field(&loc.field_label("longitude")),
            error: current.longitude_invalid().then_some(coordinate_error),
            oninput: move |value: String| draft.write().longitude = value,
            onreset: move |()| {
                let value = seed.read().longitude.clone();
                draft.write().longitude = value;
            },
        }
        if !editing {
            if let Some(detail) = detail {
                if detail.coordinates.is_some() {
                    {scalar_provenance_row(loc, &loc.field_label("coordinates"), detail.coordinates_confidence, detail.coordinates_confidence_label.clone(), &detail.coordinate_citations)}
                }
            }
        }
    }
}

/// A scalar claim's provenance cue for the Place overview (view mode): the claim's confidence badge (if
/// asserted) and its "Why we believe" source-link popover — `⚠ No source` when unsourced. Mirrors the
/// Person overview's sourced facts ([`overview_tab`](super::person::overview_tab)) for the place's
/// coordinate and code fields, which render as read boxes above.
fn scalar_provenance_row(
    loc: &Localizer,
    label: &str,
    confidence: Option<ConfidenceLevel>,
    confidence_label: Option<String>,
    citations: &[CitationRefVm],
) -> Element {
    rsx! {
        div { class: "fact-row",
            span { class: "field-label", style: "width:96px;margin:0", "{label}" }
            span { class: "grow" }
            if let (Some(level), Some(confidence_label)) = (confidence, confidence_label) {
                ConfidenceBadge { level, label: confidence_label }
            }
            {provenance_cue(loc, loc.provenance_title_claim(label), citations)}
        }
    }
}

/// Which place collection-row edit form (if any) the side panel is showing. The place's own scalar
/// record (id · type · coordinates · code) is edited in place via the sticky header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceEditForm {
    /// Assert a name — `None` adds a new one, `Some(row)` edits (supersedes) an existing one.
    Name(Option<PlaceNameVm>),
    /// Assert an enclosing place — `None` adds a new link, `Some(row)` edits (supersedes) an existing
    /// one (the enclosing place is fixed; the correction updates its provenance).
    Enclosing(Option<PlaceHierarchyVm>),
    /// Assert an identity-changing succession (ADR 0026 §3). Add-only: an existing row's sole action
    /// stays Retract, so there is no seeded edit variant.
    Succession,
    /// Attach a citation by `human_id`.
    Citation,
    /// Attach a media object by `human_id`.
    Media,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected place: header, related-item tabs, editing side panel, toast.
#[component]
pub(crate) fn PlaceDetailPane(human_id: String) -> Element {
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
    let editing = use_signal(|| None::<PlaceEditForm>);
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
        async move { load_screen(services, Intent::ShowPlace { human_id }).await }
    });

    // The shared whole-record edit state, seeded from the loaded place (empty until it loads); it
    // reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::PlaceDetail(detail))) => genealogy_ui::PlaceDraft::from_detail(detail),
        _ => genealogy_ui::PlaceDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::PlaceDraft>(Category::Places, &human_id, &seed);

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the place's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::PlaceDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::Places,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (PlaceEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_place_edit(services, edit, prov).await {
                Ok(_) => {
                    editing_for_submit.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    // The Map tab's `GeometrySaveForm` dispatches its own `save_place_edit` (shared with the Geography
    // tool); this only reloads the detail + surfaces the toast once it has (mirrors `on_submit`'s Ok
    // arm without repeating the dispatch).
    let mut map_saved_reload = reload;
    let mut map_saved_toast = toast;
    let map_saved_label = saved_label.clone();
    let on_map_saved = use_callback(move |()| {
        map_saved_reload += 1;
        map_saved_toast.set(Some(map_saved_label.clone()));
    });

    // A per-row Retract/Detach opens the shared retract panel; confirming dispatches an
    // `UndoAssertion` carrying the typed rationale (the retract note stays in History — ADR 0004 §2).
    let on_retract = use_callback(move |(assertion_id, label, detach): (String, String, bool)| {
        retract_reason.set(String::new());
        retract.set(Some(RetractTarget {
            assertion_id,
            label,
            detach,
        }));
    });
    let mut editing_for_open = editing;
    let on_edit_open = use_callback(move |form: PlaceEditForm| editing_for_open.set(Some(form)));
    let place_tag_human = human_id.clone();
    let on_tag_remove = use_callback(move |tag_id: String| {
        on_submit.call((
            PlaceEdit::Tag {
                human_id: place_tag_human.clone(),
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
        let Some(RetractTarget { assertion_id, .. }) = retract() else {
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
            let edit = PlaceEdit::UndoAssertion { human_id, assertion_id };
            match save_place_edit(services, edit, prov).await {
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
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::PlaceDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_place_edit).await;
            finish_record_save(effective, Category::Places, &current, record_nav, reload, toast, &saved);
        });
    });

    // ⌘Z retracts the newest undoable assertion of this record's loaded change log (WP5).
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::PlaceDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read() || retract.read().is_some());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let undo_human = human_id.clone();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((
            PlaceEdit::UndoAssertion {
                human_id: undo_human.clone(),
                assertion_id,
            },
            ProvenanceDraft::default(),
        ));
    });
    use_record_undo(nav, undo_busy, undo_history, undo_notice, on_undo);

    // The close/quit confirm's Save hands the record back to this pane (issue #240): it runs the same
    // whole-record commit the header's Save does, so ⌘W/⌘Q can keep the edit instead of discarding it.
    let save_now = use_callback(move |()| {
        if record.can_save() {
            on_record_save.call((record.draft.read().clone(), record.prov.read().clone()));
        }
    });
    use_save_on_request(Category::Places, Some(&human_id), record, save_now);

    // The Media tab's crop viewer: opening a card, and superseding its crop via `SetMediaRegion`.
    let media_viewing = use_signal(|| None::<MediaRefVm>);
    let on_view = use_callback(move |item: MediaRefVm| media_viewing.clone().set(Some(item)));
    let region_human = human_id.clone();
    let on_region = use_callback(
        move |(assertion_id, crop, caption): (String, Option<Rect>, Option<String>)| {
            on_submit.call((
                PlaceEdit::SetMediaRegion {
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

    let body = match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::PlaceDetail(detail))) => place_detail(
            &state,
            detail,
            PlacePane {
                active,
                side_edit: editing,
                record,
                retract,
                retract_reason,
            },
            &PlaceCallbacks {
                on_submit,
                on_record_save,
                on_retract,
                on_retract_confirm,
                on_edit_open,
                on_undo,
                on_tag_remove,
                on_map_saved,
                media_state,
            },
            &human_id,
        ),
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::EventDetail(_)
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

/// The signals a place's detail threads to its tabs: the active tab, the collection-row side panel,
/// and the whole-record edit state.
#[derive(Clone, Copy)]
struct PlacePane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<PlaceEditForm>>,
    /// The whole-record (id · type · coordinates · code) edit state.
    record: RecordEditState<genealogy_ui::PlaceDraft>,
    /// The row being retracted/detached, if the retract panel is open.
    retract: Signal<Option<RetractTarget>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The commit callbacks a place's detail wires in: one-command collection edits, the whole-record
/// save (the scalar edit via `edits_against`), and the per-row correction (edit-open + retract-confirm).
#[derive(Clone, Copy)]
struct PlaceCallbacks {
    /// Commits one [`PlaceEdit`] command (a collection row).
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::PlaceDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<PlaceEditForm>,
    /// Retracts an assertion by id from the History tab (dispatches `UndoAssertion`).
    on_undo: Callback<String>,
    /// Untags a tag by id from the Tags tab (dispatches `Tag { remove: true }`).
    on_tag_remove: Callback<String>,
    /// Reloads the detail + surfaces the saved toast once the Map tab's own `GeometrySaveForm` has
    /// dispatched its `AssertGeometry` (Phase 9).
    on_map_saved: Callback<()>,
    /// The Media tab's viewer state + crop-supersede wiring.
    media_state: MediaTabState,
}

/// Renders a loaded place's detail container: header (with the sticky-header record Edit/Cancel/Save),
/// the tab strip, the active tab, and the collection-row side panel.
fn place_detail(
    state: &AppState,
    detail: &PlaceDetail,
    pane: PlacePane,
    callbacks: &PlaceCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let PlacePane {
        active,
        side_edit: editing,
        record,
        retract,
        retract_reason,
    } = pane;
    let on_submit = callbacks.on_submit;
    let on_retract = callbacks.on_retract;
    let on_retract_confirm = callbacks.on_retract_confirm;
    let on_edit_open = callbacks.on_edit_open;
    let on_undo = callbacks.on_undo;
    let on_tag_remove = callbacks.on_tag_remove;
    let on_map_saved = callbacks.on_map_saved;
    let media_state = callbacks.media_state;
    let tabs = place_tabs(detail, loc);
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
        DetailContainer {
            title: detail.title.clone(),
            id_label: Some(detail.human_id.clone()),
            avatar: "📍".to_owned(),
            extras: place_restriction_toggles(loc, detail, on_submit, human_id),
            actions: record_head_actions(&labels, record, rsx! {}, callbacks.on_record_save),
            tabs: tab_items,
            active,
            {place_tab_content(state, detail, active_id, editing, record, on_retract, on_edit_open, on_undo, on_tag_remove, on_map_saved, media_state)}
        }
        {place_edit_panel(state, editing, on_submit, human_id)}
        {retract_side_panel(loc, retract, retract_reason, on_retract_confirm, "detach-citation")}
    }
}

/// The interactive privacy-restriction toggles for a place (the mockup `resn-set`).
fn place_restriction_toggles(
    loc: &Localizer,
    detail: &PlaceDetail,
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
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
                on_submit.call((PlaceEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one place detail tab, with its contextual add/edit affordances.
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threads the pane's signals + callbacks"
)]
fn place_tab_content(
    state: &AppState,
    detail: &PlaceDetail,
    tab_id: &str,
    mut editing: Signal<Option<PlaceEditForm>>,
    record: RecordEditState<genealogy_ui::PlaceDraft>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<PlaceEditForm>,
    on_undo: Callback<String>,
    on_tag_remove: Callback<String>,
    on_map_saved: Callback<()>,
    media_state: MediaTabState,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "map" => place_map(detail, on_map_saved, on_retract),
        "names" => rsx! {
            div { class: "section-note", "{loc.place_names_note()}" }
            div { class: "tab-actions",
                Button { label: loc.action_label("add-name"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Name(None))) }
            }
            {place_names_table(loc, detail, on_edit_open, on_retract)}
        },
        "hierarchy" => rsx! {
            div { class: "section-note", "{loc.place_hierarchy_note()}" }
            div { class: "tab-actions",
                Button { label: loc.action_label("add-enclosing"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Enclosing(None))) }
            }
            {place_hierarchy_table(loc, detail, on_edit_open, on_retract)}
            {place_succession_card(loc, detail, on_edit_open, on_retract)}
        },
        "citations" => tab_with_add(
            loc,
            "attach-citation",
            editing,
            PlaceEditForm::Citation,
            rsx! {
                {citations_table::<PlaceEditForm>(loc, &detail.citations, false, on_retract)}
            },
        ),
        "media" => tab_with_add(
            loc,
            "attach-media",
            editing,
            PlaceEditForm::Media,
            rsx! {
                {media_tab(loc, &detail.media, Some(on_retract), media_state)}
            },
        ),
        "notes" => tab_with_add(
            loc,
            "attach-note",
            editing,
            PlaceEditForm::Note,
            rsx! {
                {id_list(loc, &detail.notes, Some(on_retract))}
            },
        ),
        "tags" => tags_panel(loc, &detail.tags, editing, PlaceEditForm::Tag, on_tag_remove),
        "research-notes" => rsx! {
            ResearchNotesTab {
                category: Category::Places,
                human_id: detail.human_id.clone(),
                rows: detail.research_notes.clone(),
            }
        },
        "history" => history_panel(loc, &detail.history, Some(on_undo)),
        _ => place_overview(loc, detail, record),
    }
}

/// The Overview tab, read-first (`record-editing.html` §1/§2): the place's scalar record (id · type ·
/// coordinates · code) as read boxes plus an "Enclosed by" card. Entering edit mode (via the
/// sticky-header Edit) swaps the record fields to inputs and, while dirty, shows the provenance block;
/// the enclosing card is hidden in edit mode. In view mode the coordinate and code claims render their
/// confidence badge and "Why we believe" provenance popover (or `⚠ No source`).
pub fn place_overview(
    loc: &Localizer,
    detail: &PlaceDetail,
    record: RecordEditState<genealogy_ui::PlaceDraft>,
) -> Element {
    if record.editing.read().to_owned() {
        return rsx! {
            div { class: "section-note", "{loc.place_overview_note()}" }
            {place_record_fields(loc, record, None)}
            {record_edit_provenance(loc, record)}
        };
    }
    rsx! {
        div { class: "section-note", "{loc.place_overview_note()}" }
        div { class: "grid-2",
            {place_record_fields(loc, record, Some(detail))}
            Card { title: loc.tab_label("hierarchy"),
                if let Some(enclosing) = detail.hierarchy.first() {
                    div { class: "stack",
                        div { class: "fact-row",
                            span { class: "grow", "{enclosing.name}" }
                            if let Some(date) = enclosing.date.clone() {
                                span { class: "muted", "{date}" }
                            }
                        }
                    }
                } else {
                    EmptyState { message: loc.tab_empty() }
                }
            }
        }
    }
}

/// The Place screen's Map tab (Phase 9 map editor, ADR 0024/0025/0026): the same `MapLibre` draw
/// surface as the Geography tool (`screens::map_shared`), scoped to this one place — no rail, no
/// place search (that's the Geography atlas' job). Interactive (draw tools, the live map canvas), so
/// — unlike the Phase-6 read-only MVP it replaces — this needs `AppCtx` for its services;
/// [`PlaceMapEditor`] is the actual component, this is the thin dispatcher-facing wrapper mirroring
/// [`place_overview`]'s shape. `on_saved` reloads the parent detail once a geometry save completes;
/// `on_retract` retracts a geometry-over-time row the same way every other tab's rows do.
pub fn place_map(
    detail: &PlaceDetail,
    on_saved: Callback<()>,
    on_retract: Callback<(String, String, bool)>,
) -> Element {
    rsx! {
        PlaceMapEditor { detail: detail.clone(), on_saved, on_retract }
    }
}

/// The Place Map tab's `MapLibre` container id (distinct from the Geography tool's own mount, so both
/// could coexist).
const PLACE_MAP_CONTAINER_ID: &str = "place-map";

/// The Place Map tab's default time-slider year (matches the Geography tool's own default).
const PLACE_MAP_DEFAULT_YEAR: i32 = 1900;

/// The interactive Map tab body: draw tools, the map surface (this place's resolved-as-of-year
/// geometry only — no other places, no event pins; Geography shows those), the time slider, the
/// "Geometry over time" table, and the save-geometry card once a shape is confirmed. Every save
/// dispatches [`PlaceEdit::AssertGeometry`] via [`GeometrySaveForm`] — the same audited path a typed
/// field edit uses.
#[component]
fn PlaceMapEditor(
    detail: PlaceDetail,
    on_saved: Callback<()>,
    on_retract: Callback<(String, String, bool)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let chrome = state.chrome();
    let mut nav = use_context::<NavState>();

    let year = use_signal(|| PLACE_MAP_DEFAULT_YEAR);
    let mut tool = use_signal(|| DrawTool::Pan);
    let mut draft = use_signal(|| MapDraft::Empty);
    let mut pending = use_signal(|| None::<PlaceGeometry>);
    let mut toast = use_signal(|| None::<String>);

    // A "Geometry over time" row's Edit loads that assertion's own shape back into the draw state
    // (rather than opening a side panel) so it can be adjusted and re-saved through the same
    // `AssertGeometry` path — also switches the active draw tool to match, so the Confirm/Finish
    // affordance for that shape kind shows immediately.
    let on_edit_geometry = use_callback(move |geometry: PlaceGeometryVm| {
        tool.set(match &geometry.shape {
            MarkerShapeVm::Point { .. } => DrawTool::Point,
            MarkerShapeVm::Polygon { .. } => DrawTool::Polygon,
        });
        draft.set(shape_to_draft(&geometry.shape));
    });

    let on_click_tool = tool;
    let mut on_click_draft = draft;
    let on_map_click = move |lat: f64, lon: f64| match on_click_tool() {
        DrawTool::Pan => {}
        DrawTool::Point => on_click_draft.set(MapDraft::Point((lat, lon))),
        DrawTool::Polygon => {
            let mut vertices = match on_click_draft() {
                MapDraft::Polygon(vertices) => vertices,
                _ => Vec::new(),
            };
            vertices.push((lat, lon));
            on_click_draft.set(MapDraft::Polygon(vertices));
        }
    };

    // Re-push this place's as-of-year shape whenever the loaded geometries or the slider year change
    // (resolved client-side over the already-loaded list — see `resolve_geometry_as_of`, no extra
    // query). Falls back to the scalar coordinate (`place_map_display_shape`) when the place has no
    // dedicated ADR 0024 geometry assertion of its own yet, mirroring the Geography atlas' identical
    // fallback — otherwise a GEDCOM-imported or manually geocoded place shows no location at all.
    let push_detail = detail.clone();
    use_effect(move || {
        let shape = place_map_display_shape(&push_detail, year());
        push_this_place(&push_detail, shape.as_ref());
    });
    // Re-push the in-progress draft overlay whenever it changes.
    use_effect(move || push_map_draft(PLACE_MAP_CONTAINER_ID, &draft()));

    let resolved_shape = place_map_display_shape(&detail, year());
    let center = map_center(resolved_shape.as_ref());

    let coordinate_invalid = loc.place_coordinate_invalid();
    let on_finish_polygon = move |_| {
        let MapDraft::Polygon(vertices) = draft() else { return };
        if vertices.len() < 3 {
            toast.set(Some(coordinate_invalid.clone()));
            return;
        }
        pending.set(Some(PlaceGeometry::Polygon {
            exterior: vertices.iter().map(|&(lat, lon)| geo_point(lat, lon)).collect(),
            holes: Vec::new(),
        }));
        draft.set(MapDraft::Empty);
    };
    let on_confirm_point = move |_| {
        let MapDraft::Point((lat, lon)) = draft() else { return };
        pending.set(Some(PlaceGeometry::Point(geo_point(lat, lon))));
        draft.set(MapDraft::Empty);
    };
    let on_clear_draft = move |_| draft.set(MapDraft::Empty);

    let aria = loc.place_map_aria(&detail.title);
    let human_id = detail.human_id.clone();
    let fit_shape = resolved_shape.clone();
    let open_geography_human_id = detail.human_id.clone();
    let open_geography_title = detail.title.clone();
    rsx! {
        div { class: "map-pane",
            div { class: "section-note", "{loc.place_map_scope_note()}" }
            div { class: "geo-toolbar", style: "margin-bottom:10px",
                {draw_tool_button(tool, DrawTool::Pan, chrome.geography_tool_pan())}
                {draw_tool_button(tool, DrawTool::Point, chrome.geography_tool_point())}
                {draw_tool_button(tool, DrawTool::Polygon, chrome.geography_tool_polygon())}
                Button {
                    label: chrome.geography_tool_fit(),
                    small: true,
                    variant: ButtonVariant::Ghost,
                    title: chrome.place_map_fit_title(),
                    onclick: move |_| {
                        if let Some(shape) = &fit_shape {
                            fit_bounds(PLACE_MAP_CONTAINER_ID, std::slice::from_ref(shape));
                        }
                    },
                }
                span { class: "spacer" }
                {geography_provider_select_placeholder(loc)}
                Button {
                    label: chrome.place_map_open_in_geography(),
                    small: true,
                    variant: ButtonVariant::Ghost,
                    title: chrome.place_map_open_in_geography_title(),
                    onclick: move |_| nav.open_geography_focused(open_geography_human_id.clone(), open_geography_title.clone()),
                }
            }
            div { class: "card map-card",
                {map_surface(PLACE_MAP_CONTAINER_ID, aria, tool, on_map_click, center, 13.0)}
            }
            if matches!(tool(), DrawTool::Point) && matches!(draft(), MapDraft::Point(_)) {
                div { class: "wrap", style: "gap:8px",
                    Button { label: chrome.place_map_confirm_point(), small: true, variant: ButtonVariant::Primary, onclick: on_confirm_point }
                    Button { label: chrome.geography_clear_draft(), small: true, variant: ButtonVariant::Ghost, onclick: on_clear_draft }
                }
            }
            if matches!(tool(), DrawTool::Polygon) {
                div { class: "wrap", style: "gap:8px",
                    Button { label: chrome.geography_finish_polygon(), small: true, variant: ButtonVariant::Primary, onclick: on_finish_polygon }
                    Button { label: chrome.geography_clear_draft(), small: true, variant: ButtonVariant::Ghost, onclick: on_clear_draft }
                }
            }
            {geography_time_slider(chrome, year)}
            {place_geometry_table(loc, &detail.geometries, on_retract, on_edit_geometry)}
            if let Some(geometry) = pending() {
                div { class: "card", style: "margin-top:10px",
                    GeometrySaveForm {
                        human_id: human_id.clone(),
                        geometry,
                        year: Some(year()),
                        onsaved: move |()| { pending.set(None); on_saved.call(()); },
                    }
                    Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| pending.set(None) }
                }
            }
        }
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: loc.action_label("dismiss"),
            onaction: move |_| toast.set(None),
        }
    }
}

/// One draw-tool toggle button (Pan/Point/Polygon), highlighted while active.
fn draw_tool_button(mut tool: Signal<DrawTool>, this: DrawTool, label: String) -> Element {
    let active = tool() == this;
    rsx! {
        Button {
            label,
            small: true,
            variant: if active { ButtonVariant::Primary } else { ButtonVariant::Default },
            onclick: move |_| tool.set(this),
        }
    }
}

/// A read-only placeholder for the map provider label (the Place Map tab reuses whatever provider is
/// configured; changing it is the Geography tool's own provider select — one config, one place to set
/// it, ADR 0025 §3).
fn geography_provider_select_placeholder(loc: &Localizer) -> Element {
    rsx! {
        span { class: "muted", style: "font-size:var(--fs-xs)", "{loc.field_label(\"provider\")}" }
    }
}

/// The map's center: the resolved shape's point (or its first vertex, for a polygon), falling back to
/// the shared default when the place has no geometry yet.
fn map_center(shape: Option<&MarkerShapeVm>) -> (f64, f64) {
    match shape {
        Some(MarkerShapeVm::Point { lat, lon }) => (*lat, *lon),
        Some(MarkerShapeVm::Polygon { exterior, .. }) => exterior.first().copied().unwrap_or(DEFAULT_CENTER),
        None => DEFAULT_CENTER,
    }
}

/// Pushes this place's own resolved-as-of-year shape as the map's single "marker" (no other places —
/// Geography shows those), plus the events that occurred here (ADR 0025 §1's event-at-place pins,
/// scoped to just this place); `shape: None` pushes an empty marker collection.
fn push_this_place(detail: &PlaceDetail, shape: Option<&MarkerShapeVm>) {
    let markers = shape.map_or_else(Vec::new, |shape| {
        vec![genealogy_ui::PlaceMarkerVm {
            human_id: detail.human_id.clone(),
            id: detail.id.clone(),
            name: detail.title.clone(),
            type_label: None,
            shape: shape.clone(),
        }]
    });
    push_map_data(
        PLACE_MAP_CONTAINER_ID,
        &markers_geojson(&markers),
        &events_geojson(&detail.events),
    );
}

/// The "Geometry over time" table (ADR 0024/0026): one row per dated (or undated/primary) geometry
/// assertion, each Retract-able through the same audited [`PlaceEdit::UndoAssertion`] path as any
/// other row (`row_actions_cell`'s Retract, mirroring `place_names_table`). Drawing a new geometry is
/// the map's own draw tools, not a form here — this list is read + retract only.
pub fn place_geometry_table(
    loc: &Localizer,
    geometries: &[PlaceGeometryVm],
    on_retract: Callback<(String, String, bool)>,
    on_edit: Callback<PlaceGeometryVm>,
) -> Element {
    if geometries.is_empty() {
        return rsx! {
            Card { title: loc.place_geometry_table_title(),
                EmptyState { message: loc.place_map_empty_heading() }
                div { class: "faint", style: "font-size:var(--fs-xs)", "{loc.place_map_empty_help()}" }
            }
        };
    }
    rsx! {
        Card { title: loc.place_geometry_table_title(),
            Table {
                caption: loc.place_geometry_table_title(),
                headers: vec![
                    loc.field_label("type"),
                    loc.field_label("date"),
                    loc.field_label("coordinates"),
                    loc.field_label("confidence"),
                    loc.field_label("source"),
                    String::new(),
                ],
                for geometry in geometries.iter() {
                    tr {
                        td { Chip { label: geometry.kind_label.clone() } }
                        td { class: "muted", {geometry.date.clone().unwrap_or_else(|| "—".to_owned())} }
                        td { class: "mono", "{geometry_detail_text(loc, &geometry.shape)}" }
                        td { ConfidenceBadge { level: geometry.confidence, label: geometry.confidence_label.clone() } }
                        td { {source_cue(loc, geometry.source_count)} }
                        {row_actions_cell(
                            loc,
                            &geometry.kind_label,
                            Some((geometry.clone(), Some("edit-geometry"))), None,
                            Some(RowRetract { assertion_id: geometry.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                            Some(on_edit),
                            on_retract)}
                    }
                }
            }
        }
    }
}

/// The "Geometry over time" table's Detail column: decimal-degree coordinates for a point, or the
/// localized, pluralized vertex count for a polygon.
fn geometry_detail_text(loc: &Localizer, shape: &MarkerShapeVm) -> String {
    match shape {
        MarkerShapeVm::Point { lat, lon } => format!("{lat:.4}, {lon:.4}"),
        MarkerShapeVm::Polygon { exterior, .. } => loc.place_geometry_vertex_count(exterior.len()),
    }
}

/// The Names tab: a row per asserted name with language, date, surety, and source columns, plus a
/// per-row Edit (supersedes via [`PlaceEdit::AddName`]) and Retract (retracts the name assertion — it
/// stays in History).
pub fn place_names_table(
    loc: &Localizer,
    detail: &PlaceDetail,
    onedit: Callback<PlaceEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if detail.names.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            caption: loc.tab_label("names"),
            headers: vec![
                loc.field_label("name"),
                loc.field_label("language"),
                loc.field_label("period"),
                loc.field_label("confidence"),
                loc.field_label("source"),
                String::new(),
            ],
            for name in detail.names.iter() {
                tr {
                    td { b { "{name.text}" } }
                    td { class: "muted", {name.language.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { {name.date.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { ConfidenceBadge { level: name.confidence, label: name.confidence_label.clone() } }
                    td { {source_cue(loc, name.source_count)} }
                    {row_actions_cell(
                        loc,
                        &name.text,
                        Some((PlaceEditForm::Name(Some(name.clone())), None)), None,
                        Some(RowRetract { assertion_id: name.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
                }
            }
        }
    }
}

/// The Hierarchy tab: a breadcrumb of the jurisdiction chain plus a level-by-level table, each row
/// carrying a per-row Edit (supersedes via [`PlaceEdit::AddEnclosing`]) and Retract (retracts the
/// enclosing-by assertion — it stays in History).
pub fn place_hierarchy_table(
    loc: &Localizer,
    detail: &PlaceDetail,
    onedit: Callback<PlaceEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if detail.hierarchy.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "breadcrumb", style: "margin-bottom:16px",
            b { "{detail.title}" }
            for enclosing in detail.hierarchy.iter() {
                span { class: "sep", "›" }
                span { "{enclosing.name}" }
            }
        }
        Table {
            caption: loc.tab_label("attributes"),
            headers: vec![
                loc.field_label("name"),
                loc.field_label("attribute-type"),
                loc.field_label("date"),
                loc.field_label("confidence"),
                String::new(),
            ],
            for enclosing in detail.hierarchy.iter() {
                tr {
                    td { "{enclosing.name}" }
                    td {
                        if let Some(type_label) = enclosing.type_label.clone() {
                            Chip { label: type_label }
                        } else {
                            span { class: "muted", "—" }
                        }
                    }
                    td { class: "muted", {enclosing.date.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { ConfidenceBadge { level: enclosing.confidence, label: enclosing.confidence_label.clone() } }
                    {row_actions_cell(
                        loc,
                        &enclosing.name,
                        Some((PlaceEditForm::Enclosing(Some(enclosing.clone())), None)), None,
                        Some(RowRetract { assertion_id: enclosing.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
                }
            }
        }
    }
}

/// The Hierarchy tab's Succession card (ADR 0026 §3–§4): every identity change this place was party
/// to, either as a predecessor ("X merged/split/absorbed/elevated/renamed → this place") or a
/// successor ("this place → Y"), each dated and Retract-able. Distinct from the enclosing-place chain
/// above — a succession reaches a *different* place aggregate, not a dated link to the same one.
pub fn place_succession_card(
    loc: &Localizer,
    detail: &PlaceDetail,
    onedit: Callback<PlaceEditForm>,
    on_retract: Callback<(String, String, bool)>,
) -> Element {
    let empty = detail.predecessors.is_empty() && detail.successors.is_empty();
    rsx! {
        Card { title: loc.place_succession_title(),
            div { class: "tab-actions",
                Button {
                    label: loc.place_succession_add(),
                    variant: ButtonVariant::Primary,
                    small: true,
                    title: loc.place_succession_add_title(),
                    onclick: move |_| onedit.call(PlaceEditForm::Succession),
                }
            }
            div { class: "faint", style: "font-size:var(--fs-xs);margin-bottom:6px", "{loc.place_succession_note()}" }
            if empty {
                EmptyState { message: loc.tab_empty() }
            } else {
                div { class: "stack",
                    for rel in detail.predecessors.iter() {
                        {succession_row(loc, &detail.title, rel, true, on_retract)}
                    }
                    for rel in detail.successors.iter() {
                        {succession_row(loc, &detail.title, rel, false, on_retract)}
                    }
                }
            }
        }
    }
}

/// One Succession card row: the kind chip, the dated from→to link (direction set by `is_predecessor`),
/// and a Retract targeting the succession assertion.
fn succession_row(
    loc: &Localizer,
    this_title: &str,
    rel: &genealogy_ui::PlaceSuccessionVm,
    is_predecessor: bool,
    on_retract: Callback<(String, String, bool)>,
) -> Element {
    let assertion_id = rel.assertion_id.clone();
    let label = rel.name.clone();
    let counterpart = rsx! {
        RecordLink { category: Category::Places, human_id: rel.human_id.clone(), label: rel.name.clone() }
    };
    rsx! {
        div { class: "fact-row",
            Chip { label: rel.kind_label.clone() }
            span { class: "grow",
                if is_predecessor {
                    {counterpart}
                    " → "
                    b { "{this_title}" }
                } else {
                    b { "{this_title}" }
                    " → "
                    {counterpart}
                }
            }
            if let Some(date) = rel.date.clone() {
                span { class: "muted", "{date}" }
            }
            Button {
                label: loc.action_label("retract"),
                variant: ButtonVariant::Ghost,
                small: true,
                title: loc.action_title("retract"),
                aria_label: loc.action_retract_row(&label),
                onclick: move |_| on_retract.call((assertion_id.clone(), label.clone(), false)),
            }
        }
    }
}

/// The place editing side panel: renders the form for the open [`PlaceEditForm`], or nothing.
fn place_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<PlaceEditForm>>,
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match &form {
        PlaceEditForm::Name(None) => loc.action_label("add-name"),
        PlaceEditForm::Name(Some(_)) => loc.panel_title("edit-name"),
        PlaceEditForm::Enclosing(None) => loc.action_label("add-enclosing"),
        PlaceEditForm::Enclosing(Some(_)) => loc.panel_title("edit-enclosing"),
        PlaceEditForm::Succession => loc.place_succession_add(),
        PlaceEditForm::Citation => loc.action_label("attach-citation"),
        PlaceEditForm::Media => loc.action_label("attach-media"),
        PlaceEditForm::Note => loc.action_label("attach-note"),
        PlaceEditForm::Tag => loc.action_label("add-tag"),
    };
    let human_id = human_id.to_owned();
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |()| editing.set(None),
            footer: rsx! {},
            {match form {
                PlaceEditForm::Name(seed) => rsx! { PlaceNameForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Enclosing(seed) => rsx! { PlaceEnclosingForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Succession => rsx! { PlaceSuccessionForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Citation => rsx! { PlaceLinkForm { human_id, field: "citation".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Media => rsx! { PlaceLinkForm { human_id, field: "media".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Note => rsx! { PlaceLinkForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Tag => rsx! { PlaceTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The place name form → [`PlaceEdit::AddName`]. `seed: None` adds a new name (a free-text place-name
/// string, not a record link); `Some(row)` edits an existing name — the text input is pre-filled and
/// the provenance draft's `supersedes` is seeded with the row's assertion id so Save supersedes
/// (replaces) rather than appends (ADR 0004 §2). The scalar code is edited in the record, not here.
#[component]
fn PlaceNameForm(
    human_id: String,
    seed: Option<PlaceNameVm>,
    onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut value = use_signal(|| seed.as_ref().map(|row| row.text.clone()).unwrap_or_default());
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|row| row.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let save_label = loc.action_label("save");
    rsx! {
        Input {
            label: loc.field_label("name"),
            name: "name".to_owned(),
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
                onsubmit.call((PlaceEdit::AddName { human_id: human_id.clone(), text: value }, prov()));
            },
        }
    }
}

/// The place enclosing-place form → [`PlaceEdit::AddEnclosing`]. `seed: None` adds a new enclosing-by
/// link over an existing-place picker; `Some(row)` edits an existing one — the enclosing place is fixed
/// (shown as a link), the correction updates its provenance, and the draft's `supersedes` is seeded with
/// the row's assertion id so Save supersedes rather than appends (ADR 0004 §2).
#[component]
fn PlaceEnclosingForm(
    human_id: String,
    seed: Option<PlaceHierarchyVm>,
    onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    // Edit mode fixes the enclosing place (only the provenance changes); add mode offers a picker.
    let fixed = seed.as_ref().map(|row| (row.human_id.clone(), row.name.clone()));
    let picker = use_existing_picker(
        services,
        Category::Places,
        loc.field_label("place"),
        "enclosing".to_owned(),
        loc.picker_entity(Category::Places),
        Vec::new(),
    );
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|row| row.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let picker_for_save = picker.clone();
    let fixed_for_save = fixed.as_ref().map(|(id, _)| id.clone());
    let onsave = use_callback(move |()| {
        let Some(enclosing_id) = fixed_for_save.clone().or_else(|| picker_selection_id(&picker_for_save)) else {
            return;
        };
        onsubmit.call((
            PlaceEdit::AddEnclosing {
                human_id: human_id.clone(),
                enclosing_id,
            },
            prov(),
        ));
    });
    if let Some((id, name)) = &fixed {
        rsx! {
            div { class: "field",
                label { "{loc.field_label(\"place\")}" }
                RecordLink { category: Category::Places, human_id: id.clone(), label: name.clone() }
            }
            {provenance_block(loc, prov)}
            Button {
                label: loc.action_label("save"),
                variant: ButtonVariant::Primary,
                onclick: move |_| onsave.call(()),
            }
        }
    } else {
        attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
    }
}

/// The Succession panel's live state: the picked kind (an index into [`genealogy_ui::SUCCESSION_KINDS`]),
/// the two accumulating place lists, and the effective-date draft. Bundled so the field-rendering fn
/// stays within the argument budget; every field is a `Signal`, so the struct is `Copy` too.
#[derive(Clone, Copy)]
pub struct SuccessionFormState {
    /// The picked kind's index into [`genealogy_ui::SUCCESSION_KINDS`].
    pub kind: Signal<usize>,
    /// The resulting place(s) picked so far — the app's `to` endpoints (many for a split).
    pub to: Signal<Vec<PickerSelection>>,
    /// The *other* ceasing place(s) picked so far — a merge's many side; the anchor is implicit.
    pub from: Signal<Vec<PickerSelection>>,
    /// When the succession took effect; a blank draft leaves it undated.
    pub date: Signal<DateDraft>,
}

/// The Succession panel's field set (`place.html`'s Succession card): the kind select, the repeatable
/// resulting-place picker, the repeatable also-ceased picker (a merge's many side), and the
/// effective-date cluster. A pure fn (the pickers + signals passed in) so the SSR tests render it
/// without `AppCtx`.
pub fn place_succession_form_fields(
    loc: &Localizer,
    to_picker: &RecordPicker,
    from_picker: &RecordPicker,
    state: SuccessionFormState,
) -> Element {
    let mut kind = state.kind;
    let mut date = state.date;
    let options: Vec<SelectChoice> = genealogy_ui::SUCCESSION_KINDS
        .iter()
        .enumerate()
        .map(|(index, succession_kind)| SelectChoice {
            value: index.to_string(),
            label: loc.succession_kind_label(*succession_kind),
        })
        .collect();
    rsx! {
        Select {
            label: loc.place_succession_kind_field(),
            name: "succession-kind".to_owned(),
            value: Some(kind().to_string()),
            options,
            onchange: move |event: FormEvent| {
                if let Ok(index) = event.value().parse::<usize>() {
                    kind.set(index);
                }
            },
        }
        {succession_place_field(loc, to_picker, state.to)}
        {succession_place_field(loc, from_picker, state.from)}
        {date_draft_field(
            loc,
            "succession-date",
            true,
            date(),
            DateDraft::default(),
            Callback::new(move |value: DateDraft| date.set(value)),
            Callback::new(move |()| date.set(DateDraft::default())),
        )}
    }
}

/// One repeatable place field of the Succession panel: an existing-place picker, an "add the picked
/// place" control that moves the pick into `picked` and clears the picker, and the accumulated picks
/// as deletable chips. Local to this screen — the succession panel is its only caller.
fn succession_place_field(loc: &Localizer, picker: &RecordPicker, mut picked: Signal<Vec<PickerSelection>>) -> Element {
    let mut picker_state = picker.state;
    let nothing_picked = picker.state.read().selection.is_none();
    let dismiss = loc.action_label("dismiss");
    rsx! {
        {record_picker(loc, picker)}
        div { class: "tab-actions",
            Button {
                label: loc.place_succession_add_picked(),
                variant: ButtonVariant::Default,
                small: true,
                disabled: nothing_picked,
                onclick: move |_| {
                    let Some(selection) = picker_state.read().selection.clone() else {
                        return;
                    };
                    picked.write().push(selection);
                    picker_state.set(PickerState::default());
                },
            }
        }
        if !picked.read().is_empty() {
            div { class: "wrap", style: "margin-bottom:8px",
                for (index , selection) in picked.read().iter().enumerate() {
                    Chip {
                        key: "{selection.human_id}",
                        label: selection.title.clone(),
                        id_label: selection.human_id.clone(),
                        delete_label: dismiss.clone(),
                        ondelete: move |()| {
                            picked.write().remove(index);
                        },
                    }
                }
            }
        }
    }
}

/// The place succession form → [`PlaceEdit::AssertSuccession`] (ADR 0026 §3). Add-only: an existing
/// succession row's sole action is Retract, so there is no seeded edit mode.
///
/// `human_id` is the **anchor** — the place the assertion is recorded against, and always one of the
/// ceasing places; the dispatcher prepends it to the app's `from` list, so the "Also ceased" picker
/// names only the *other* places that ceased. Save is a no-op until at least one resulting place is
/// picked (the app rejects an empty endpoint list).
#[component]
fn PlaceSuccessionForm(human_id: String, onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let form = SuccessionFormState {
        kind: use_signal(|| 0_usize),
        to: use_signal(Vec::<PickerSelection>::new),
        from: use_signal(Vec::<PickerSelection>::new),
        date: use_signal(DateDraft::default),
    };
    // Neither picker may offer this place or an already-picked one: a succession's endpoints are
    // distinct places, and the anchor rides in `human_id`.
    let mut taken: Vec<String> = vec![human_id.clone()];
    taken.extend(form.to.read().iter().map(|pick| pick.human_id.clone()));
    taken.extend(form.from.read().iter().map(|pick| pick.human_id.clone()));
    let to_picker = use_existing_picker(
        services.clone(),
        Category::Places,
        loc.place_succession_to_field(),
        "succession-to".to_owned(),
        loc.picker_entity(Category::Places),
        taken.clone(),
    );
    let from_picker = use_existing_picker(
        services,
        Category::Places,
        loc.place_succession_from_field(),
        "succession-from".to_owned(),
        loc.picker_entity(Category::Places),
        taken,
    );
    let prov = use_signal(ProvenanceDraft::default);
    let onsave = use_callback(move |()| {
        let edit = succession_edit(
            &human_id,
            form.kind.peek().to_owned(),
            &form.to.read(),
            &form.from.read(),
            &form.date.read(),
        );
        let Some(edit) = edit else {
            return;
        };
        onsubmit.call((edit, prov()));
    });
    rsx! {
        {place_succession_form_fields(loc, &to_picker, &from_picker, form)}
        {provenance_block(loc, prov)}
        Button {
            label: loc.action_label("save"),
            variant: ButtonVariant::Primary,
            onclick: move |_| onsave.call(()),
        }
    }
}

/// The edit the Succession panel's Save dispatches, or `None` when the form is not assertable yet — no
/// resulting place, an unknown kind index, or an unparseable date (the app would reject each). Pure, so
/// the guard is unit-tested without a render scope.
fn succession_edit(
    human_id: &str,
    kind_index: usize,
    to: &[PickerSelection],
    from: &[PickerSelection],
    date: &DateDraft,
) -> Option<PlaceEdit> {
    if to.is_empty() {
        return None;
    }
    Some(PlaceEdit::AssertSuccession {
        human_id: human_id.to_owned(),
        from_extra: from.iter().map(|pick| pick.human_id.clone()).collect(),
        to: to.iter().map(|pick| pick.human_id.clone()).collect(),
        kind: *genealogy_ui::SUCCESSION_KINDS.get(kind_index)?,
        date: date.to_input().ok()?,
    })
}

/// A place collection link form over an existing-only picker (an attached citation/media/note) → the
/// matching [`PlaceEdit`] attach variant.
#[component]
fn PlaceLinkForm(human_id: String, field: String, onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let (label, category) = match field.as_str() {
        "citation" => (loc.field_label("citation"), Category::Citations),
        "note" => (loc.field_label("note"), Category::Notes),
        _ => (loc.field_label("media"), Category::Media),
    };
    let picker = use_existing_picker(
        services,
        category,
        label,
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
            "citation" => PlaceEdit::AttachCitation {
                human_id: human_id.clone(),
                citation_id: id,
            },
            "note" => PlaceEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            },
            _ => PlaceEdit::AttachMedia {
                human_id: human_id.clone(),
                media_id: id,
            },
        };
        onsubmit.call((edit, prov()));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
}

/// The place "Add tag" form: a picker of existing tags by name → [`PlaceEdit::Tag`].
#[component]
fn PlaceTagForm(human_id: String, onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((PlaceEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

/// The place types offered by the type picker.
fn place_type_choices() -> [PlaceType; 9] {
    [
        PlaceType::Country,
        PlaceType::County,
        PlaceType::Municipality,
        PlaceType::Parish,
        PlaceType::City,
        PlaceType::Town,
        PlaceType::Village,
        PlaceType::Farm,
        PlaceType::Building,
    ]
}

#[cfg(test)]
mod succession_form_tests {
    use genealogy_app::SuccessionKind;
    use genealogy_ui::{DateDraft, DateModifierKind, PickerSelection, PlaceEdit};

    use super::succession_edit;

    fn pick(human_id: &str) -> PickerSelection {
        PickerSelection {
            human_id: human_id.to_owned(),
            title: format!("{human_id} place"),
        }
    }

    #[test]
    fn a_merge_puts_the_other_ceasing_places_in_from_extra_and_leaves_the_anchor_implicit() {
        let edit =
            succession_edit("P0001", 0, &[pick("P0003")], &[pick("P0002")], &DateDraft::default()).expect("assertable");
        assert_eq!(
            edit,
            PlaceEdit::AssertSuccession {
                human_id: "P0001".to_owned(),
                from_extra: vec!["P0002".to_owned()],
                to: vec!["P0003".to_owned()],
                kind: SuccessionKind::Merged,
                date: None,
            },
            "the anchor is never repeated in `from_extra`; the dispatcher prepends it"
        );
    }

    #[test]
    fn a_split_preserves_the_picked_order_of_the_resulting_places() {
        let edit = succession_edit("P0001", 1, &[pick("P0002"), pick("P0003")], &[], &DateDraft::default())
            .expect("assertable");
        let PlaceEdit::AssertSuccession { to, kind, .. } = edit else {
            panic!("expected an AssertSuccession");
        };
        assert_eq!(to, vec!["P0002".to_owned(), "P0003".to_owned()]);
        assert_eq!(kind, SuccessionKind::Split);
    }

    #[test]
    fn no_resulting_place_is_not_assertable() {
        assert_eq!(
            succession_edit("P0001", 0, &[], &[pick("P0002")], &DateDraft::default()),
            None,
            "the app rejects an empty endpoint list, so Save stays a no-op"
        );
    }

    #[test]
    fn a_dated_succession_carries_its_parsed_date() {
        let date = DateDraft {
            start: "1948".to_owned(),
            ..DateDraft::default()
        };
        let edit = succession_edit("P0001", 0, &[pick("P0003")], &[], &date).expect("assertable");
        let PlaceEdit::AssertSuccession { date, .. } = edit else {
            panic!("expected an AssertSuccession");
        };
        assert!(date.is_some(), "a typed year reaches the edit");
    }

    #[test]
    fn an_unparseable_date_is_not_assertable() {
        let date = DateDraft {
            kind: DateModifierKind::TextOnly,
            ..DateDraft::default()
        };
        assert_eq!(
            succession_edit("P0001", 0, &[pick("P0003")], &[], &date),
            None,
            "a text-only date with no text would be rejected downstream"
        );
    }
}

#[cfg(test)]
mod map_editor_tests {
    use super::{DEFAULT_CENTER, geometry_detail_text, map_center};
    use genealogy_ui::{Localizer, MarkerShapeVm};

    fn loc() -> Localizer {
        Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
    }

    #[test]
    fn a_points_shape_centers_on_itself() {
        let shape = MarkerShapeVm::Point {
            lat: 40.7128,
            lon: -74.006,
        };
        assert_eq!(map_center(Some(&shape)), (40.7128, -74.006));
    }

    #[test]
    fn a_polygons_shape_centers_on_its_first_vertex() {
        let shape = MarkerShapeVm::Polygon {
            exterior: vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)],
            holes: Vec::new(),
        };
        assert_eq!(map_center(Some(&shape)), (60.0, 5.0));
    }

    #[test]
    fn no_shape_falls_back_to_the_shared_default_center() {
        assert_eq!(map_center(None), DEFAULT_CENTER);
    }

    #[test]
    fn an_empty_polygon_falls_back_to_the_shared_default_center() {
        let shape = MarkerShapeVm::Polygon {
            exterior: Vec::new(),
            holes: Vec::new(),
        };
        assert_eq!(map_center(Some(&shape)), DEFAULT_CENTER);
    }

    #[test]
    fn a_point_shape_renders_its_decimal_degrees() {
        let shape = MarkerShapeVm::Point {
            lat: 40.7128,
            lon: -74.006,
        };
        assert_eq!(geometry_detail_text(&loc(), &shape), "40.7128, -74.0060");
    }

    #[test]
    fn a_polygon_shape_renders_its_pluralized_vertex_count() {
        let one_vertex = MarkerShapeVm::Polygon {
            exterior: vec![(60.0, 5.0)],
            holes: Vec::new(),
        };
        assert_eq!(geometry_detail_text(&loc(), &one_vertex), "1 vertex");
        let three_vertices = MarkerShapeVm::Polygon {
            exterior: vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)],
            holes: Vec::new(),
        };
        assert_eq!(geometry_detail_text(&loc(), &three_vertices), "3 vertices");
    }
}
