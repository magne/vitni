use super::prelude::*;
use crate::screens::RecordDetail;

/// The person master-detail: a searchable list on the left, the selected person's detail
/// (an overview tab plus related-item tabs) on the right.
#[component]
pub fn PersonScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let create_services = services.clone();
    let chrome = state.chrome();
    let entity = chrome.nav_people();
    let loading = chrome.loading();
    let empty = state.data_loc().list_empty();
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
    // Keep the list-row highlight in sync with the active record tab (clicking a tab re-highlights).
    use_effect(move || selected.set(nav.active_record_ref().map(|record| record.human_id)));
    // The top-bar `New`/`⌘N`/new-record menu set `pending_create`; opening the create form here makes
    // them work too.
    use_effect(move || {
        if *nav.pending_create.read() == Some(Category::People) {
            creating.set(true);
            nav.pending_create.set(None);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowList).await }
    });
    let on_create = use_callback(move |(request, prov): (PersonChangeSetRequest, ProvenanceDraft)| {
        let services = create_services.clone();
        let label = request
            .name
            .as_ref()
            .map(|parts| {
                [parts.given.as_deref(), parts.surname.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|joined| !joined.is_empty());
        spawn(async move {
            match commit_person_change_set(services, request, prov).await {
                Ok(human_id) => {
                    creating.set(false);
                    nav.open_record(RecordRef {
                        category: Category::People,
                        label: label.unwrap_or_else(|| human_id.clone()),
                        human_id,
                    });
                }
                Err(message) => toast.set(Some(message)),
            }
        });
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
                onselect: move |row: RowVm| nav.open_record(RecordRef {
                    category: Category::People,
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
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_)
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
    let create_detail = rsx! {
        PersonCreateRecord {
            onsubmit: move |payload| on_create.call(payload),
            oncancel: move |()| creating.set(false),
        }
    };
    let detail = if creating() {
        create_detail
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

/// The create-mode person record (`record-editing.html` §6): an empty draft rendered in edit mode in
/// the detail pane, with Cancel/Save in the sticky header. The scalar identity fields (editable human
/// id, name type, name parts, sex) come from the shared [`person_record_fields`] Card; the name-citation
/// cascade (a Citations picker → an inline new-citation → a nested new-source) and the tag multi-select
/// are buffered here. Save turns the draft into a [`PersonChangeSetRequest`]; Cancel drops it.
#[component]
fn PersonCreateRecord(
    onsubmit: EventHandler<(PersonChangeSetRequest, ProvenanceDraft)>,
    oncancel: EventHandler<()>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<PersonDraft>();
    let mut draft = record.draft;
    let selected_tags = use_signal(Vec::<String>::new);

    // The name-citation cascade: a Citations picker; "+ New" opens a citation draft card (a page input
    // + a nested Sources picker), whose "+ New" opens a source draft card (a title input). Every value
    // lives in the draft's `name_citation` link, so dirtiness / validity flow through unchanged.
    let citation_state = use_signal(genealogy_ui::PickerState::default);
    let source_state = use_signal(genealogy_ui::PickerState::default);
    let citation_services = services.clone();
    let citation_rows = use_resource(move || {
        let services = citation_services.clone();
        async move { load_picker_rows(services, Category::Citations).await }
    });
    let source_services = services.clone();
    let source_rows = use_resource(move || {
        let services = source_services.clone();
        async move { load_picker_rows(services, Category::Sources).await }
    });
    let citation_onpick = use_callback(move |selection: PickerSelection| {
        draft.write().name_citation = genealogy_ui::RecordLink::Existing(selection);
    });
    let citation_onclear = use_callback(move |()| draft.write().name_citation = genealogy_ui::RecordLink::Empty);
    let citation_onnew = use_callback(move |_query: String| {
        draft.write().name_citation = genealogy_ui::RecordLink::New(NewCitationFields::default());
    });
    let source_onpick = use_callback(move |selection: PickerSelection| {
        if let genealogy_ui::RecordLink::New(citation) = &mut draft.write().name_citation {
            citation.source = genealogy_ui::RecordLink::Existing(selection);
        }
    });
    let source_onclear = use_callback(move |()| {
        if let genealogy_ui::RecordLink::New(citation) = &mut draft.write().name_citation {
            citation.source = genealogy_ui::RecordLink::Empty;
        }
    });
    let source_onnew = use_callback(move |_query: String| {
        if let genealogy_ui::RecordLink::New(citation) = &mut draft.write().name_citation {
            citation.source = genealogy_ui::RecordLink::New(NewSourceFields::default());
        }
    });

    let tags_resource = use_resource(move || {
        let services = services.clone();
        async move { load_tags(services).await }
    });

    // Fold the tag selection into the draft so the dirty gate sees tag-only changes too.
    use_effect(move || draft.write().tags = selected_tags());

    let title = loc.person_new_title();
    let draft_badge = loc.record_draft_badge();
    let save_label = loc.action_label("save");
    let cancel_label = loc.action_label("cancel");
    let can_save = record.can_save();
    let actions = rsx! {
        Button {
            label: cancel_label,
            variant: ButtonVariant::Ghost,
            small: true,
            onclick: move |_| oncancel.call(()),
        }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            small: true,
            disabled: !can_save,
            onclick: move |_| {
                if !record.can_save() {
                    return;
                }
                onsubmit.call((record.draft.read().to_request(), record.prov.read().clone()));
            },
        }
    };

    let citation_picker = RecordPicker {
        config: PickerConfig {
            label: loc.section_name_citation(),
            name: "name-citation".to_owned(),
            entity_label: loc.picker_entity(Category::Citations),
            allow_new: true,
        },
        state: citation_state,
        options: picker_options(citation_rows.read_unchecked().as_ref()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: citation_onpick,
            onclear: citation_onclear,
            onnew: citation_onnew,
        },
    };
    let source_picker = RecordPicker {
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
        &title,
        &draft_badge,
        actions,
        rsx! {
            {person_record_fields(loc, record)}
            {person_name_citation_field(loc, draft, &citation_picker, &source_picker)}
            h4 { class: "field-label", "{loc.section_tags()}" }
            {tag_multiselect(loc, tags_resource, selected_tags)}
            {record_edit_provenance(loc, record)}
        },
    )
}

/// The person create form's name-citation cascade (data-model §7): a Citations picker; "+ New" opens a
/// citation [`draft_card`] (a page input + a nested Sources picker), whose "+ New" opens a source draft
/// card (a title input). A pure fn over the draft signal + the two configured pickers.
pub fn person_name_citation_field(
    loc: &Localizer,
    draft: Signal<PersonDraft>,
    citation: &RecordPicker,
    source: &RecordPicker,
) -> Element {
    match &draft().name_citation {
        genealogy_ui::RecordLink::New(_) => {
            let title = loc.citation_new_title();
            let discard = citation.callbacks.onclear;
            let body = person_new_citation_body(loc, draft, source);
            draft_card(
                &title,
                &loc.draft_card_badge(),
                loc.draft_card_discard(&title),
                discard,
                body,
            )
        }
        genealogy_ui::RecordLink::Empty | genealogy_ui::RecordLink::Existing(_) => record_picker(loc, citation),
    }
}

/// The inline new-citation fields: a page input plus the citation's own source cascade (a nested
/// Sources picker that can itself open a new-source draft card).
fn person_new_citation_body(loc: &Localizer, mut draft: Signal<PersonDraft>, source: &RecordPicker) -> Element {
    let (page, source_is_new) = match &draft().name_citation {
        genealogy_ui::RecordLink::New(citation) => {
            let source_is_new = match &citation.source {
                genealogy_ui::RecordLink::New(_) => true,
                genealogy_ui::RecordLink::Empty | genealogy_ui::RecordLink::Existing(_) => false,
            };
            (citation.page.clone(), source_is_new)
        }
        genealogy_ui::RecordLink::Empty | genealogy_ui::RecordLink::Existing(_) => (String::new(), false),
    };
    let source_field = if source_is_new {
        let title = loc.source_new_title();
        let discard = source.callbacks.onclear;
        let body = person_new_source_body(loc, draft);
        draft_card(
            &title,
            &loc.draft_card_badge(),
            loc.draft_card_discard(&title),
            discard,
            body,
        )
    } else {
        record_picker(loc, source)
    };
    rsx! {
        Input {
            label: loc.field_label("page"),
            name: "citation-page".to_owned(),
            value: page,
            oninput: move |event: FormEvent| {
                if let genealogy_ui::RecordLink::New(citation) = &mut draft.write().name_citation {
                    citation.page = event.value();
                }
            },
        }
        {source_field}
    }
}

/// The inline new-source field inside a new citation: a single title input, bound to the deeply-nested
/// new-source link (`name_citation → New citation → New source`).
fn person_new_source_body(loc: &Localizer, mut draft: Signal<PersonDraft>) -> Element {
    let title = match &draft().name_citation {
        genealogy_ui::RecordLink::New(citation) => match &citation.source {
            genealogy_ui::RecordLink::New(fields) => fields.title.clone(),
            genealogy_ui::RecordLink::Empty | genealogy_ui::RecordLink::Existing(_) => String::new(),
        },
        genealogy_ui::RecordLink::Empty | genealogy_ui::RecordLink::Existing(_) => String::new(),
    };
    rsx! {
        Input {
            label: loc.field_label("title"),
            name: "citation-new-source-title".to_owned(),
            value: title,
            oninput: move |event: FormEvent| {
                if let genealogy_ui::RecordLink::New(citation) = &mut draft.write().name_citation
                    && let genealogy_ui::RecordLink::New(fields) = &mut citation.source
                {
                    fields.title = event.value();
                }
            },
        }
    }
}

/// The four sexes offered by the record's Gender select (the model also has `Sex::Other`, kept when
/// present but not offered as a new choice).
const SEXES: [Sex; 4] = [Sex::Female, Sex::Male, Sex::Unknown, Sex::Intersex];

/// The person record's scalar identity fields (name type · the six name parts · sex), rendered
/// read-first: read boxes in view mode, inputs with per-field reset in edit mode
/// (`record-editing.html` §2/§3). A pure fn (the edit state's signals passed in) so the create pane
/// and the SSR tests render it without `AppCtx`. Shared by view, edit, and create.
pub fn person_record_fields(loc: &Localizer, record: RecordEditState<PersonDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    rsx! {
        Card { title: loc.tab_label("overview"),
            div { class: "stack",
                {person_human_id_field(loc, editing, record)}
                {person_name_type_field(loc, editing, record)}
                {person_name_text_fields(loc, editing, record)}
                {person_sex_field(loc, editing, record)}
            }
        }
    }
}

/// The editable user-facing id of the person record (monospace; clearing it regenerates on save).
fn person_human_id_field(loc: &Localizer, editing: bool, record: RecordEditState<PersonDraft>) -> Element {
    let mut draft = record.draft;
    let seed = record.seed;
    let value = draft().human_id_override.clone();
    let original = seed.read().human_id_override.clone();
    rsx! {
        DraftText {
            label: loc.field_label("id"),
            name: "human-id".to_owned(),
            editing,
            value,
            original,
            reset_label: loc.action_reset_field(&loc.field_label("id")),
            mono: true,
            hint: Some(loc.field_human_id_hint()),
            oninput: move |value: String| draft.write().human_id_override = value,
            onreset: move |()| {
                let value = seed.read().human_id_override.clone();
                draft.write().human_id_override = value;
            },
        }
    }
}

/// The name-type select of the person record (index-valued into [`Localizer::name_type_choices`]).
fn person_name_type_field(loc: &Localizer, editing: bool, record: RecordEditState<PersonDraft>) -> Element {
    let mut draft = record.draft;
    let choices = loc.name_type_choices();
    let index_of = |kind: &NameType| choices.iter().position(|(candidate, _)| candidate == kind).unwrap_or(0);
    let options: Vec<SelectChoice> = choices
        .iter()
        .enumerate()
        .map(|(index, (_, label))| SelectChoice {
            value: index.to_string(),
            label: label.clone(),
        })
        .collect();
    let value = index_of(&draft().name_type).to_string();
    let original = index_of(&record.seed.read().name_type).to_string();
    let choices_for_change = choices.clone();
    rsx! {
        DraftSelect {
            label: loc.field_label("name-type"),
            name: "name-type".to_owned(),
            editing,
            value,
            original,
            reset_label: loc.action_reset_field(&loc.field_label("name-type")),
            options,
            onchange: move |value: String| {
                if let Some((kind, _)) = value.parse::<usize>().ok().and_then(|index| choices_for_change.get(index)) {
                    draft.write().name_type = kind.clone();
                }
            },
            onreset: move |()| {
                let kind = record.seed.read().name_type.clone();
                draft.write().name_type = kind;
            },
        }
    }
}

/// The six preferred-name text fields (prefix · given · nickname · surname prefix · surname · suffix).
fn person_name_text_fields(loc: &Localizer, editing: bool, record: RecordEditState<PersonDraft>) -> Element {
    let mut draft = record.draft;
    let seed = record.seed;
    let current = draft();
    let original = seed.read().clone();
    rsx! {
        DraftText { label: loc.field_label("prefix"), name: "prefix".to_owned(), editing,
            value: current.prefix.clone(), original: original.prefix.clone(),
            reset_label: loc.action_reset_field(&loc.field_label("prefix")),
            oninput: move |value: String| draft.write().prefix = value,
            onreset: move |()| { let value = seed.read().prefix.clone(); draft.write().prefix = value; } }
        DraftText { label: loc.label_given(), name: "given".to_owned(), editing,
            value: current.given.clone(), original: original.given.clone(),
            reset_label: loc.action_reset_field(&loc.label_given()),
            oninput: move |value: String| draft.write().given = value,
            onreset: move |()| { let value = seed.read().given.clone(); draft.write().given = value; } }
        DraftText { label: loc.field_label("nickname"), name: "nickname".to_owned(), editing,
            value: current.nickname.clone(), original: original.nickname.clone(),
            reset_label: loc.action_reset_field(&loc.field_label("nickname")),
            oninput: move |value: String| draft.write().nickname = value,
            onreset: move |()| { let value = seed.read().nickname.clone(); draft.write().nickname = value; } }
        DraftText { label: loc.field_surname_prefix(), name: "surname-prefix".to_owned(), editing,
            value: current.surname_prefix.clone(), original: original.surname_prefix.clone(),
            reset_label: loc.action_reset_field(&loc.field_surname_prefix()),
            oninput: move |value: String| draft.write().surname_prefix = value,
            onreset: move |()| { let value = seed.read().surname_prefix.clone(); draft.write().surname_prefix = value; } }
        DraftText { label: loc.label_surname(), name: "surname".to_owned(), editing,
            value: current.surname.clone(), original: original.surname.clone(),
            reset_label: loc.action_reset_field(&loc.label_surname()),
            oninput: move |value: String| draft.write().surname = value,
            onreset: move |()| { let value = seed.read().surname.clone(); draft.write().surname = value; } }
        DraftText { label: loc.field_label("suffix"), name: "suffix".to_owned(), editing,
            value: current.suffix.clone(), original: original.suffix.clone(),
            reset_label: loc.action_reset_field(&loc.field_label("suffix")),
            oninput: move |value: String| draft.write().suffix = value,
            onreset: move |()| { let value = seed.read().suffix.clone(); draft.write().suffix = value; } }
    }
}

/// The sex select of the person record (index-valued into [`SEXES`], defaulting to Unknown).
fn person_sex_field(loc: &Localizer, editing: bool, record: RecordEditState<PersonDraft>) -> Element {
    let mut draft = record.draft;
    let index_of = |sex: &Sex| SEXES.iter().position(|candidate| candidate == sex).unwrap_or(2);
    let options: Vec<SelectChoice> = SEXES
        .iter()
        .enumerate()
        .map(|(index, sex)| SelectChoice {
            value: index.to_string(),
            label: loc.sex_label(Some(sex)),
        })
        .collect();
    let value = index_of(&draft().sex).to_string();
    let original = index_of(&record.seed.read().sex).to_string();
    rsx! {
        DraftSelect {
            label: loc.label_sex(),
            name: "sex".to_owned(),
            editing,
            value,
            original,
            reset_label: loc.action_reset_field(&loc.label_sex()),
            options,
            onchange: move |value: String| {
                if let Some(sex) = value.parse::<usize>().ok().and_then(|index| SEXES.get(index)) {
                    draft.write().sex = sex.clone();
                }
            },
            onreset: move |()| {
                let sex = record.seed.read().sex.clone();
                draft.write().sex = sex;
            },
        }
    }
}

/// The dialog's tag multi-select: a name picker that adds a tag, plus removable chips (name + colour,
/// never the id — data-model §9). Selection is buffered in `selected` until the dialog commits.
fn tag_multiselect(
    loc: &Localizer,
    tags: Resource<Result<Vec<TagSummary>, String>>,
    mut selected: Signal<Vec<String>>,
) -> Element {
    let available = match &*tags.read_unchecked() {
        None => return rsx! { p { class: "loading", "{loc.dialog_add_tag_hint()}" } },
        Some(Err(message)) => return rsx! { p { class: "empty", "{message}" } },
        Some(Ok(list)) => list.clone(),
    };
    let mut options = vec![SelectChoice {
        value: String::new(),
        label: loc.dialog_add_tag_hint(),
    }];
    for tag in &available {
        if let Some(name) = tag.name.clone() {
            options.push(SelectChoice {
                value: tag.id.clone(),
                label: name,
            });
        }
    }
    let chips: Vec<(String, String, Option<String>)> = selected()
        .iter()
        .filter_map(|id| {
            available
                .iter()
                .find(|tag| &tag.id == id)
                .map(|tag| (tag.id.clone(), tag.name.clone().unwrap_or_default(), tag.color.clone()))
        })
        .collect();
    let remove_label = loc.action_label("remove-tag");
    rsx! {
        Select {
            label: loc.section_tags(),
            name: "tag-picker".to_owned(),
            value: Some(String::new()),
            options,
            onchange: move |event: FormEvent| {
                let id = event.value();
                if !id.is_empty() && !selected().contains(&id) {
                    selected.write().push(id);
                }
            },
        }
        if chips.is_empty() {
            span { class: "muted", "{loc.dialog_no_tags()}" }
        } else {
            div { class: "wrap",
                for (id , name , color) in chips {
                    span { class: "fact-row",
                        Chip { label: name, dot_color: color }
                        Button {
                            label: remove_label.clone(),
                            variant: ButtonVariant::Ghost,
                            small: true,
                            onclick: move |_| {
                                let target = id.clone();
                                selected.write().retain(|existing| existing != &target);
                            },
                        }
                    }
                }
            }
        }
    }
}

/// Which collection-row edit form (if any) the side panel is showing. The person's own scalar record
/// (name + sex) is edited in place via the sticky-header Edit, not here (`record-editing.html` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditForm {
    /// Assert an additional name.
    Name,
    /// Assert a fact, with confidence and an optional source.
    Fact,
    /// Attach an existing citation by id.
    Citation,
    /// Attach an existing media object by id.
    Media,
    /// Attach an existing note by id.
    Note,
    /// Apply a tag the operator picks by name (the "+ Add tag" side panel; chip × dispatches Untag
    /// directly without a panel).
    Tag,
}

/// The detail pane for the selected person: a header, the related-item tab strip, the editing side
/// panel, and a save toast. Owns the reload/editing/toast state; reads are reloaded after each save.
#[component]
pub(crate) fn PersonDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let nav = use_context::<NavState>();
    let mut label_nav = nav;
    let mut record_nav = nav;
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let mut editing = use_signal(|| None::<EditForm>);
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
        // Subscribe to `reload`: bumping it after a save refetches the detail.
        let _ = reload();
        async move { load_screen(services, Intent::ShowPerson { human_id }).await }
    });

    // The whole-record edit state, seeded from the person's edit draft (empty until it loads); reseeds
    // on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::Detail(detail))) => detail.edit_seed.clone(),
        _ => PersonDraft::new(),
    };
    let record = use_record_edit::<PersonDraft>(&seed);

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the person's
    // name (`tab_label` falls back to `human_id` when the name is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::Detail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::People,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.name), &label_human_id),
        );
    });

    let record_services = services.clone();
    let on_submit = use_callback(move |(edit, prov): (PersonEdit, ProvenanceDraft)| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_edit(services, edit, prov).await {
                Ok(()) => {
                    editing.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });
    let saved_label_rec = state.data_loc().action_label("saved");
    // The whole-record Save: the buffered draft becomes a change-set commit (the identity edit).
    let on_record_save = use_callback(move |(draft, prov): (PersonDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let saved = saved_label_rec.clone();
        let request = draft.to_request();
        spawn(async move {
            match commit_person_change_set(services, request, prov).await {
                Ok(_) => {
                    reload += 1;
                    record_nav.mark_changed();
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
    let retract_services = state.services().clone();
    let retract_human = human_id.clone();
    let saved_label_retract = state.data_loc().action_label("saved");
    let on_retract_confirm = use_callback(move |()| {
        let Some((assertion_id, _, _)) = retract() else {
            return;
        };
        let services = retract_services.clone();
        let human_id = retract_human.clone();
        let saved = saved_label_retract.clone();
        let prov = ProvenanceDraft {
            rationale: retract_reason(),
            ..ProvenanceDraft::default()
        };
        spawn(async move {
            let edit = PersonEdit::UndoAssertion { human_id, assertion_id };
            match save_edit(services, edit, prov).await {
                Ok(()) => {
                    retract.set(None);
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
        Some(ScreenData::Loaded(IntentOutcome::Detail(detail))) => {
            let pane = PersonPane {
                active,
                side_edit: editing,
                record,
                retract,
                retract_reason,
            };
            let callbacks = PersonCallbacks {
                on_submit,
                on_record_save,
                on_retract,
                on_retract_confirm,
            };
            person_detail(&state, &nav, detail, pane, callbacks, &human_id)
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Dashboard(_)
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

/// The signals a person's detail threads to its tabs: the active tab, the collection-row side panel,
/// and the whole-record edit state.
#[derive(Clone, Copy)]
struct PersonPane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<EditForm>>,
    /// The whole-record (scalar identity) edit state.
    record: RecordEditState<PersonDraft>,
    /// The row being retracted/detached, if the retract panel is open: `(assertion_id, label, detach)`.
    retract: Signal<Option<(String, String, bool)>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The two commit callbacks a person's detail wires in: one-command collection edits (attach / assert
/// / undo / restrictions) and the whole-record change-set save (the identity edit).
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "event-handler fields conventionally share the on_ prefix"
)]
struct PersonCallbacks {
    /// Commits one [`PersonEdit`] command.
    on_submit: Callback<(PersonEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a change-set.
    on_record_save: Callback<(PersonDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
}

/// Renders a loaded person's detail container: header (avatar, vital subtitle, restriction toggles,
/// Compare + the sticky-header record Edit/Cancel/Save), the tab strip, the active tab's content, and
/// the collection-row side panel.
fn person_detail(
    state: &AppState,
    nav: &NavState,
    detail: &PersonDetail,
    pane: PersonPane,
    callbacks: PersonCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let PersonPane {
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
    let tabs = person_tabs(detail, loc);
    let tab_items: Vec<TabItem> = tabs
        .iter()
        .map(|tab| TabItem {
            id: tab.id.to_owned(),
            label: tab.label.clone(),
            count: tab.count,
        })
        .collect();
    let active_id = tabs.get(active()).map_or("overview", |tab| tab.id);
    let subtitle = match &detail.vitals {
        Some(vitals) => format!("{vitals} · {}", detail.sex),
        None => detail.sex.clone(),
    };
    let compare_label = loc.action_label("compare");
    let mut compare_nav = *nav;
    let labels = RecordActionLabels::resolve(loc);
    // Compare is the view-mode extra action, alongside the record Edit; Save/Cancel replace both in
    // edit mode (`record_head_actions`).
    let extra_actions = rsx! {
        Button { label: compare_label, variant: ButtonVariant::Default, small: true, onclick: move |_| compare_nav.go_to(Destination::Tool(Tool::Merge)) }
    };
    rsx! {
        DetailContainer {
            title: detail.name.clone(),
            subtitle,
            id_label: Some(detail.human_id.clone()),
            badges: vec![detail.evidence_level_label.clone()],
            avatar: person_initials(detail),
            extras: restriction_toggles(loc, detail, on_submit, human_id),
            actions: record_head_actions(&labels, record, extra_actions, on_record_save),
            tabs: tab_items,
            active,
            {person_tab_content(state, detail, active_id, editing, record, on_submit, on_retract, human_id)}
        }
        {edit_panel(state, detail, editing, on_submit, human_id)}
        {person_retract_panel(loc, retract, retract_reason, on_retract_confirm)}
    }
}

/// Renders the shared Retract/Detach side panel when a collection row's action is armed. Reads the
/// armed `(assertion_id, label, detach)` and binds the rationale input to `reason`; confirming calls
/// `on_confirm` (which dispatches `UndoAssertion`). Closed (rendered empty) when nothing is armed.
fn person_retract_panel(
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

/// The person's initials (first letters of given + surname, uppercased), or `?` when unknown.
fn person_initials(detail: &PersonDetail) -> String {
    let mut initials = String::new();
    for part in [detail.given.as_deref(), detail.surname.as_deref()] {
        if let Some(first) = part.and_then(|name| name.chars().next()) {
            initials.extend(first.to_uppercase());
        }
    }
    if initials.is_empty() {
        initials.push('?');
    }
    initials
}

/// The interactive privacy-restriction toggles shown in the detail header (the mockup `resn-set`).
fn restriction_toggles(
    loc: &Localizer,
    detail: &PersonDetail,
    on_submit: Callback<(PersonEdit, ProvenanceDraft)>,
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
                on_submit
                    .call((
                        PersonEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next },
                        ProvenanceDraft::default(),
                    ));
            },
        }
    }
}

/// The content of one person detail tab, with its contextual add/edit affordances.
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threads the pane's signals + callbacks"
)]
fn person_tab_content(
    state: &AppState,
    detail: &PersonDetail,
    tab_id: &str,
    mut editing: Signal<Option<EditForm>>,
    record: RecordEditState<PersonDraft>,
    on_submit: Callback<(PersonEdit, ProvenanceDraft)>,
    on_retract: Callback<(String, String, bool)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "names" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-name"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Name)) }
            }
            {names_table(loc, &detail.names, on_retract)}
        },
        "facts" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-fact"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Fact)) }
            }
            {facts_table(loc, &detail.facts, on_retract)}
        },
        "events" => events_table(loc, &detail.events, on_retract),
        "associations" => associations_table(loc, &detail.associations, on_retract),
        "families" => families_panel(loc, &detail.families),
        "citations" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-citation"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Citation)) }
            }
            {person_citations_table(loc, &detail.citations, on_retract)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Media)) }
            }
            {media_gallery(loc, &detail.media)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => person_tags_panel(loc, &detail.tags, editing, on_submit, human_id),
        "history" => history_tab(loc, detail, on_submit, human_id),
        _ => person_overview(loc, detail, record),
    }
}

/// The person Tags tab: a dispatching panel (mirrors the other ten aggregates). "+ Add tag" opens the
/// picker side panel; each applied tag is a name + colour-dot chip with a × that dispatches
/// [`PersonEdit::Tag`] with `remove: true` (Untag — recorded in History). The tag is referenced by
/// name; its UUID is never rendered (data-model §9). Tags never retract — Untag is the only removal.
pub fn person_tags_panel(
    loc: &Localizer,
    tags: &[TagRef],
    mut editing: Signal<Option<EditForm>>,
    on_submit: Callback<(PersonEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    let untag_title = loc.action_title("untag");
    rsx! {
        div { class: "tab-actions",
            Button {
                label: loc.action_label("add-tag"),
                variant: ButtonVariant::Default,
                onclick: move |_| editing.set(Some(EditForm::Tag)),
            }
        }
        if tags.is_empty() {
            EmptyState { message: loc.tab_empty() }
        } else {
            div { class: "wrap",
                for tag in tags.iter() {
                    {
                        let tag_id = tag.id.clone();
                        let human_id = human_id.clone();
                        let remove_name = loc.action_remove_tag_named(&tag.name);
                        let untag_title = untag_title.clone();
                        rsx! {
                            span { class: "fact-row",
                                Chip { label: tag.name.clone(), dot_color: tag.color.clone() }
                                IconButton {
                                    icon: "×".to_owned(),
                                    label: remove_name,
                                    title: untag_title,
                                    onclick: move |_| on_submit.call((
                                        PersonEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true },
                                        ProvenanceDraft::default(),
                                    )),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The Overview tab, read-first (`record-editing.html` §1/§2): the person's scalar identity record
/// (name + sex) as read boxes plus the vital-facts and family summary cards. Entering edit mode (via
/// the sticky-header Edit) swaps the identity fields to inputs and, while dirty, shows the provenance
/// block; the summary cards are hidden in edit mode to keep the focus on the record being changed.
fn person_overview(loc: &Localizer, detail: &PersonDetail, record: RecordEditState<PersonDraft>) -> Element {
    if record.editing.read().to_owned() {
        return rsx! {
            div { class: "section-note", "{loc.overview_note()}" }
            {person_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        };
    }
    rsx! {
        {person_record_fields(loc, record)}
        {overview_tab(loc, detail)}
    }
}

/// The History tab: the per-record audit timeline (who/when/why), each undoable entry carrying an
/// undo control. The event-sourced differentiator — free from the event log.
fn history_tab(
    loc: &Localizer,
    detail: &PersonDetail,
    on_submit: Callback<(PersonEdit, ProvenanceDraft)>,
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
                on_submit
                    .call((
                        PersonEdit::UndoAssertion { human_id: human_id.clone(), assertion_id },
                        ProvenanceDraft::default(),
                    ));
            },
        }
    }
}

/// The overview tab: the evidence-first note plus two cards — vital facts (each with its surety and
/// source cue) and the immediate family.
pub fn overview_tab(loc: &Localizer, detail: &PersonDetail) -> Element {
    rsx! {
        div { class: "section-note", "{loc.overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.section_label("vitals"),
                if detail.facts.is_empty() {
                    span { class: "muted", "{loc.tab_empty()}" }
                } else {
                    div { class: "stack",
                        for fact in detail.facts.iter() {
                            div { class: "fact-row",
                                span { class: "field-label", style: "width:96px;margin:0", "{fact.type_label}" }
                                span { class: "grow", {fact_value_date(fact)} }
                                ConfidenceBadge { level: fact.confidence, label: fact.confidence_label.clone() }
                                {provenance_cue(loc, loc.provenance_title_claim(&fact.type_label), &fact.citations)}
                            }
                        }
                    }
                }
            }
            Card { title: loc.section_label("family"),
                if detail.families.is_empty() {
                    span { class: "muted", "{loc.tab_empty()}" }
                } else {
                    div { class: "stack",
                        for family in detail.families.iter() {
                            div { class: "fact-row",
                                span { class: "muted", "{family.role_label}" }
                                span { class: "grow", {family.partners.join(" · ")} }
                            }
                            if !family.children.is_empty() {
                                div { class: "fact-row",
                                    span { class: "muted", "{loc.family_children()}" }
                                    span { class: "grow", {family.children.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>().join(" · ")} }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Renders a fact's value and/or date as a single display string (`date · value`, or whichever is
/// present), or an em dash when neither is known.
fn fact_value_date(fact: &FactVm) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(date) = fact.date.as_deref() {
        parts.push(date.to_owned());
    }
    if let Some(value) = fact.value.as_deref() {
        parts.push(value.to_owned());
    }
    if parts.is_empty() {
        "—".to_owned()
    } else {
        parts.join(" · ")
    }
}

/// The Names tab: every asserted name variant with its type chip, date / language, and its
/// evidence cues (surety badge + source-count / no-source flag — colour is never the only signal).
pub fn names_table(loc: &Localizer, names: &[NameVm], onretract: Callback<(String, String, bool)>) -> Element {
    if names.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("name-type"),
                loc.label_name(),
                format!("{} / {}", loc.field_label("date"), loc.field_label("language")),
                loc.field_label("surety"),
                loc.field_label("source"),
                String::new(),
            ],
            for name in names.iter() {
                tr {
                    td {
                        Chip { label: name.type_label.clone() }
                    }
                    td { "{name.display}" }
                    td { class: "muted", {name_date_language(name)} }
                    td {
                        ConfidenceBadge { level: name.confidence, label: name.confidence_label.clone() }
                    }
                    td {
                        if name.has_source() {
                            SourceLink { label: loc.source_count(name.source_count), onclick: move |_| {} }
                        } else {
                            NoSourceFlag { label: loc.no_source() }
                        }
                    }
                    {row_retract_cell(loc, &name.assertion_id, &name.display, onretract)}
                }
            }
        }
    }
}

/// A collection row's actions cell: a ghost **Retract** button that hands the row's assertion id +
/// label up to `onretract` (which opens the shared retract panel). `detach` swaps the wording to
/// Detach for an attachment row. Shared by the person collection tables (`record-editing.html` §8).
fn row_retract_cell(
    loc: &Localizer,
    assertion_id: &str,
    label: &str,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    row_action_cell(loc, assertion_id, label, false, onretract)
}

/// A collection row's actions cell, parameterized by whether it is a Detach (an attachment) or a
/// Retract (a sub-record). Renders one ghost button with the mockup tooltip + a row-scoped accessible
/// name; clicking hands `(assertion_id, label, detach)` to `onretract`.
fn row_action_cell(
    loc: &Localizer,
    assertion_id: &str,
    label: &str,
    detach: bool,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    let assertion_id = assertion_id.to_owned();
    let label_owned = label.to_owned();
    let (button_label, title, accessible) = if detach {
        (
            loc.action_label("detach"),
            loc.action_title("detach-citation"),
            loc.action_detach_row(label),
        )
    } else {
        (
            loc.action_label("retract"),
            loc.action_title("retract"),
            loc.action_retract_row(label),
        )
    };
    rsx! {
        td { class: "row-actions",
            Button {
                label: button_label,
                variant: ButtonVariant::Ghost,
                small: true,
                title,
                aria_label: accessible,
                onclick: move |_| onretract.call((assertion_id.clone(), label_owned.clone(), detach)),
            }
        }
    }
}

/// Renders a name's `date / language` cell from whichever parts are present, or an em dash.
fn name_date_language(name: &NameVm) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(date) = name.date.as_deref() {
        parts.push(date.to_owned());
    }
    if let Some(language) = name.language.as_deref() {
        parts.push(language.to_owned());
    }
    if parts.is_empty() {
        "—".to_owned()
    } else {
        parts.join(" · ")
    }
}

/// The Facts tab: each fact with its confidence badge and source count / no-source flag — the
/// evidence-first row (colour is never the only signal).
pub fn facts_table(loc: &Localizer, facts: &[FactVm], onretract: Callback<(String, String, bool)>) -> Element {
    if facts.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("fact-type"),
                loc.field_label("date"),
                loc.field_label("value"),
                loc.field_label("surety"),
                loc.field_label("source"),
                String::new(),
            ],
            for fact in facts.iter() {
                tr {
                    td { "{fact.type_label}" }
                    td { class: "muted", {fact.date.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { {fact.value.clone().unwrap_or_else(|| "—".to_owned())} }
                    td {
                        ConfidenceBadge { level: fact.confidence, label: fact.confidence_label.clone() }
                    }
                    td {
                        if fact.has_source() {
                            SourceLink { label: loc.source_count(fact.source_count), onclick: move |_| {} }
                        } else {
                            NoSourceFlag { label: loc.no_source() }
                        }
                    }
                    {row_retract_cell(loc, &fact.assertion_id, &fact.type_label, onretract)}
                }
            }
        }
    }
}

/// The Events tab: each participation's role and the joined event id + date.
pub fn events_table(loc: &Localizer, events: &[EventRefVm], onretract: Callback<(String, String, bool)>) -> Element {
    if events.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.tab_label("events"),
                loc.field_label("role"),
                loc.field_label("date"),
                String::new(),
            ],
            for event in events.iter() {
                tr {
                    td {
                        RecordLink {
                            category: Category::Events,
                            human_id: event.event_id.clone(),
                            label: event.event_id.clone(),
                        }
                    }
                    td {
                        Chip { label: event.role_label.clone() }
                    }
                    td { class: "muted", {event.date.clone().unwrap_or_else(|| "—".to_owned())} }
                    {row_retract_cell(loc, &event.assertion_id, &event.role_label, onretract)}
                }
            }
        }
    }
}

/// The Associations tab: each linked person, the role, and the evidence cues (surety + source).
pub fn associations_table(
    loc: &Localizer,
    associations: &[AssociationVm],
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if associations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("association"),
                loc.field_label("relationship"),
                loc.field_label("surety"),
                loc.field_label("source"),
                String::new(),
            ],
            for association in associations.iter() {
                tr {
                    td {
                        RecordLink {
                            category: Category::People,
                            human_id: association.other_id.clone(),
                            label: association.other_id.clone(),
                        }
                    }
                    td {
                        Chip { label: association.role_label.clone() }
                    }
                    td {
                        ConfidenceBadge { level: association.confidence, label: association.confidence_label.clone() }
                    }
                    td {
                        if association.has_source() {
                            SourceLink { label: loc.source_count(association.source_count), onclick: move |_| {} }
                        } else {
                            NoSourceFlag { label: loc.no_source() }
                        }
                    }
                    {row_retract_cell(loc, &association.assertion_id, &association.role_label, onretract)}
                }
            }
        }
    }
}

/// The Citations tab: each backing citation's id, cited source, surety, and Evidence Explained axes
/// — the research-grade-citation differentiator surfaced on the person.
pub fn person_citations_table(
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
                loc.label_id(),
                loc.field_label("source"),
                loc.field_label("surety"),
                loc.field_label("evidence"),
                String::new(),
            ],
            for citation in citations.iter() {
                tr {
                    td {
                        RecordLink {
                            category: Category::Citations,
                            human_id: citation.human_id.clone(),
                            label: citation.human_id.clone(),
                        }
                    }
                    td { class: "muted",
                        if let Some(source_id) = &citation.source_id {
                            RecordLink {
                                category: Category::Sources,
                                human_id: source_id.clone(),
                                label: citation.source.clone().unwrap_or_else(|| source_id.clone()),
                            }
                        } else {
                            {citation.source.clone().unwrap_or_else(|| "—".to_owned())}
                        }
                    }
                    td {
                        if let (Some(level), Some(label)) = (citation.confidence, citation.confidence_label.clone()) {
                            ConfidenceBadge { level, label }
                        } else {
                            "—"
                        }
                    }
                    td { class: "wrap",
                        if citation.evidence_axes.is_empty() {
                            "—"
                        } else {
                            for chip in citation.evidence_axes.iter() {
                                EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() }
                            }
                        }
                    }
                    if let Some(assertion_id) = &citation.assertion_id {
                        {row_action_cell(loc, assertion_id, &citation.human_id, true, onretract)}
                    } else {
                        td {}
                    }
                }
            }
        }
    }
}

/// The Families tab: each family the person belongs to, their role, and the members.
pub fn families_panel(loc: &Localizer, families: &[FamilyVm]) -> Element {
    if families.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "grid-2",
            for family in families.iter() {
                Card { title: format!("{} · {}", family.role_label, family.family_id),
                    div { class: "stack",
                        for partner in family.partners.iter() {
                            div { class: "fact-row",
                                span { class: "muted", "{loc.partner_role_label()}" }
                                span { class: "grow",
                                    RecordLink {
                                        category: Category::People,
                                        human_id: partner.clone(),
                                        label: partner.clone(),
                                    }
                                }
                            }
                        }
                        for (child , relationship) in family.children.iter() {
                            div { class: "fact-row",
                                span { class: "muted", "{relationship}" }
                                span { class: "grow",
                                    RecordLink {
                                        category: Category::People,
                                        human_id: child.clone(),
                                        label: child.clone(),
                                    }
                                }
                            }
                        }
                        RecordLink {
                            category: Category::Families,
                            human_id: family.family_id.clone(),
                            label: family.family_id.clone(),
                            button: true,
                        }
                    }
                }
            }
        }
    }
}

/// The collection-row editing side panel: renders the attach/assert form for the currently-open
/// [`EditForm`], or nothing. The person's scalar record is edited in place (the sticky-header Edit),
/// not here; these are the collection add affordances (a new name, a fact, an attached citation etc.).
fn edit_panel(
    state: &AppState,
    _detail: &PersonDetail,
    mut editing: Signal<Option<EditForm>>,
    on_submit: Callback<(PersonEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        EditForm::Name => loc.action_label("add-name"),
        EditForm::Fact => loc.action_label("add-fact"),
        EditForm::Citation => loc.action_label("attach-citation"),
        EditForm::Media => loc.action_label("attach-media"),
        EditForm::Note => loc.action_label("attach-note"),
        EditForm::Tag => loc.action_label("add-tag"),
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
                EditForm::Name => rsx! { AddNameForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Fact => rsx! { AddFactForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Citation => rsx! { AttachForm { human_id, kind: EditForm::Citation, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Media => rsx! { AttachForm { human_id, kind: EditForm::Media, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Note => rsx! { AttachForm { human_id, kind: EditForm::Note, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Tag => rsx! { PersonTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The person "Add tag" side-panel form: a picker of existing tags by name → [`PersonEdit::Tag`]
/// (mirrors the other ten aggregates' add-tag form). The tag is chosen by name; its id rides the
/// command but is never shown (data-model §9).
#[component]
fn PersonTagForm(human_id: String, onsubmit: EventHandler<(PersonEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((PersonEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

/// The "Add name" side-panel form: name parts → [`PersonEdit::AssertName`].
#[component]
fn AddNameForm(human_id: String, onsubmit: EventHandler<(PersonEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut given = use_signal(String::new);
    let mut surname = use_signal(String::new);
    let mut nickname = use_signal(String::new);
    let mut prefix = use_signal(String::new);
    let mut suffix = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.label_given(), name: "given".to_owned(), oninput: move |event: FormEvent| given.set(event.value()) }
        Input { label: loc.label_surname(), name: "surname".to_owned(), oninput: move |event: FormEvent| surname.set(event.value()) }
        Input { label: loc.field_label("nickname"), name: "nickname".to_owned(), oninput: move |event: FormEvent| nickname.set(event.value()) }
        Input { label: loc.field_label("prefix"), name: "prefix".to_owned(), oninput: move |event: FormEvent| prefix.set(event.value()) }
        Input { label: loc.field_label("suffix"), name: "suffix".to_owned(), oninput: move |event: FormEvent| suffix.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let name = PersonNameParts {
                    name_type: genealogy_app::NameType::BirthName,
                    given: non_empty(given()),
                    surname_prefix: None,
                    surname: non_empty(surname()),
                    nickname: non_empty(nickname()),
                    prefix: non_empty(prefix()),
                    suffix: non_empty(suffix()),
                };
                onsubmit.call((PersonEdit::AssertName { human_id: human_id.clone(), name }, prov()));
            },
        }
    }
}

/// The "Add fact" side-panel form: type + value → [`PersonEdit::AssertFact`]. Confidence and the
/// backing citation are captured by the shared provenance block (PR25), not here.
#[component]
fn AddFactForm(human_id: String, onsubmit: EventHandler<(PersonEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let fact_choices = loc.fact_type_choices();
    let mut fact_index = use_signal(|| 0_usize);
    let mut value = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let fact_options: Vec<SelectChoice> = fact_choices
        .iter()
        .enumerate()
        .map(|(index, (_, label))| SelectChoice {
            value: index.to_string(),
            label: label.clone(),
        })
        .collect();
    let save_label = loc.action_label("save");
    rsx! {
        Select {
            label: loc.field_label("fact-type"),
            name: "fact-type".to_owned(),
            options: fact_options,
            onchange: move |event: FormEvent| fact_index.set(event.value().parse().unwrap_or(0)),
        }
        Input { label: loc.field_label("value"), name: "value".to_owned(), oninput: move |event: FormEvent| value.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let fact_type = fact_choices
                    .get(fact_index())
                    .map_or(genealogy_app::FactType::Occupation, |(kind, _)| kind.clone());
                onsubmit
                    .call((
                        PersonEdit::AssertFact {
                            human_id: human_id.clone(),
                            fact_type,
                            value: non_empty(value()),
                        },
                        prov(),
                    ));
            },
        }
    }
}

/// The "Attach by id" side-panel form for a citation/media/note → the matching attach edit.
#[component]
fn AttachForm(human_id: String, kind: EditForm, onsubmit: EventHandler<(PersonEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let (field, category) = match kind {
        EditForm::Media => ("media", Category::Media),
        EditForm::Note => ("note", Category::Notes),
        _ => ("citation", Category::Citations),
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
        let edit = match kind {
            EditForm::Media => PersonEdit::AttachMedia {
                human_id: human_id.clone(),
                media_id: id,
            },
            EditForm::Note => PersonEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            },
            _ => PersonEdit::AttachCitation {
                human_id: human_id.clone(),
                citation_id: id,
            },
        };
        onsubmit.call((edit, prov()));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
}
