use super::prelude::*;

/// The create-mode person record (`record-editing.html` §6): an empty draft rendered in edit mode in
/// the detail pane, with Cancel/Save in the sticky header. The scalar identity fields (editable human
/// id, name type, name parts, sex) come from the shared [`person_record_fields`] Card; the name-citation
/// cascade (a Citations picker → an inline new-citation → a nested new-source) and the tag multi-select
/// are buffered here. Save turns the draft into a [`PersonChangeSetRequest`]; Cancel drops it.
#[component]
pub fn PersonCreateRecord(draft_id: DraftId) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<PersonDraft>(Category::People, draft_id);
    let mut draft = record.draft;
    let selected_tags = use_signal(Vec::<String>::new);
    let save_services = services.clone();
    let created_label = loc.action_label(ActionLabel::Created);
    // Takes the draft, like the other twelve create screens, so the commit can read the label the draft
    // names itself with rather than re-deriving it from the request.
    let on_save = use_callback(move |(draft, prov): (PersonDraft, ProvenanceDraft)| {
        let request = draft.to_request();
        let services = save_services.clone();
        let created = created_label.clone();
        spawn(async move {
            let committed = commit_person_change_set(services, request, prov).await;
            finish_draft_commit(
                committed,
                DraftCommit::new(Category::People, draft_id, &draft, created),
                nav,
            );
        });
    });

    // The name-citation cascade: a Citations picker; "+ New" opens a citation draft card (a page input
    // + a nested Sources picker), whose "+ New" opens a source draft card (a title input). Every value
    // lives in the draft's `name_citation` link, so dirtiness / validity flow through unchanged.
    let citation_state = use_signal(vitni_ui::PickerState::default);
    let source_state = use_signal(vitni_ui::PickerState::default);
    let citation_services = services.clone();
    let citation_rows = use_resource(move || {
        let _ = data_version_ticket(Some(nav));
        let services = citation_services.clone();
        async move { load_picker_rows(services, Category::Citations).await }
    });
    let source_services = services.clone();
    let source_rows = use_resource(move || {
        let _ = data_version_ticket(Some(nav));
        let services = source_services.clone();
        async move { load_picker_rows(services, Category::Sources).await }
    });
    let citation_onpick = use_callback(move |selection: PickerSelection| {
        draft.write().name_citation = vitni_ui::RecordLink::Existing(selection);
    });
    let citation_onclear = use_callback(move |()| draft.write().name_citation = vitni_ui::RecordLink::Empty);
    let citation_onnew = use_callback(move |_query: String| {
        draft.write().name_citation = vitni_ui::RecordLink::New(NewCitationFields::default());
    });
    let source_onpick = use_callback(move |selection: PickerSelection| {
        if let vitni_ui::RecordLink::New(citation) = &mut draft.write().name_citation {
            citation.source = vitni_ui::RecordLink::Existing(selection);
        }
    });
    let source_onclear = use_callback(move |()| {
        if let vitni_ui::RecordLink::New(citation) = &mut draft.write().name_citation {
            citation.source = vitni_ui::RecordLink::Empty;
        }
    });
    let source_onnew = use_callback(move |_query: String| {
        if let vitni_ui::RecordLink::New(citation) = &mut draft.write().name_citation {
            citation.source = vitni_ui::RecordLink::New(NewSourceFields::default());
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
    let save_label = loc.action_button(ActionLabel::Save);
    let cancel_label = loc.action_button(ActionLabel::Cancel);
    // The close/quit confirm's Save runs this same commit (issue #240), so a ⌘W/⌘Q over a half-filled
    // create form can keep the draft instead of losing it.
    let save_now = use_callback(move |()| {
        if record.can_save() {
            on_save.call((record.draft.read().clone(), record.prov.read().clone()));
        }
    });
    use_save_on_request(EditKey::draft(Category::People, draft_id), record, save_now);
    let can_save = record.can_save();
    let actions = rsx! {
        Button {
            label: cancel_label,
            variant: ButtonVariant::Ghost,
            small: true,
            onclick: move |_| nav.cancel_draft(draft_id),
        }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            small: true,
            disabled: !can_save,
            onclick: move |_| save_now.call(()),
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
        vitni_ui::RecordLink::New(_) => {
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
        vitni_ui::RecordLink::Empty | vitni_ui::RecordLink::Existing(_) => record_picker(loc, citation),
    }
}

/// The inline new-citation fields: a page input plus the citation's own source cascade (a nested
/// Sources picker that can itself open a new-source draft card).
fn person_new_citation_body(loc: &Localizer, mut draft: Signal<PersonDraft>, source: &RecordPicker) -> Element {
    let (page, source_is_new) = match &draft().name_citation {
        vitni_ui::RecordLink::New(citation) => {
            let source_is_new = match &citation.source {
                vitni_ui::RecordLink::New(_) => true,
                vitni_ui::RecordLink::Empty | vitni_ui::RecordLink::Existing(_) => false,
            };
            (citation.page.clone(), source_is_new)
        }
        vitni_ui::RecordLink::Empty | vitni_ui::RecordLink::Existing(_) => (String::new(), false),
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
                if let vitni_ui::RecordLink::New(citation) = &mut draft.write().name_citation {
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
        vitni_ui::RecordLink::New(citation) => match &citation.source {
            vitni_ui::RecordLink::New(fields) => fields.title.clone(),
            vitni_ui::RecordLink::Empty | vitni_ui::RecordLink::Existing(_) => String::new(),
        },
        vitni_ui::RecordLink::Empty | vitni_ui::RecordLink::Existing(_) => String::new(),
    };
    rsx! {
        Input {
            label: loc.field_label("title"),
            name: "citation-new-source-title".to_owned(),
            value: title,
            oninput: move |event: FormEvent| {
                if let vitni_ui::RecordLink::New(citation) = &mut draft.write().name_citation
                    && let vitni_ui::RecordLink::New(fields) = &mut citation.source
                {
                    fields.title = event.value();
                }
            },
        }
    }
}

/// The four coded sexes offered by the record's Gender select; `person_sex_field` appends an "Other…"
/// choice (index [`SEX_OTHER_INDEX`]) for a free-text [`Sex::Other`] value.
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

/// The index of the appended "Other…" choice — one past the coded [`SEXES`].
const SEX_OTHER_INDEX: usize = SEXES.len();

/// The sex select of the person record: the four coded [`SEXES`] plus an "Other…" choice that reveals a
/// free-text entry for a [`Sex::Other`] value. A stored `Sex::Other(v)` selects "Other…" and pre-fills
/// the free text with `v` (the old `SEXES`-index logic mislabelled it as "Unknown").
fn person_sex_field(loc: &Localizer, editing: bool, record: RecordEditState<PersonDraft>) -> Element {
    let mut draft = record.draft;
    let index_of = |sex: &Sex| match sex {
        Sex::Other(_) => SEX_OTHER_INDEX,
        _ => SEXES.iter().position(|candidate| candidate == sex).unwrap_or(2),
    };
    let current_sex = draft().sex.clone();
    let is_other = matches!(current_sex, Sex::Other(_));
    let other_text = match &current_sex {
        Sex::Other(value) => value.clone(),
        _ => String::new(),
    };
    // In view mode the selected "Other…" option renders the stored value verbatim; in edit mode it stays
    // the "Other…" prompt and the value lives in the revealed free-text input below.
    let other_label = if is_other && !editing {
        loc.sex_label(Some(&current_sex))
    } else {
        loc.sex_other()
    };
    let mut options: Vec<SelectChoice> = SEXES
        .iter()
        .enumerate()
        .map(|(index, sex)| SelectChoice {
            value: index.to_string(),
            label: loc.sex_label(Some(sex)),
        })
        .collect();
    options.push(SelectChoice {
        value: SEX_OTHER_INDEX.to_string(),
        label: other_label,
    });
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
                let index = value.parse::<usize>().unwrap_or(2);
                if index == SEX_OTHER_INDEX {
                    let text = match &record.draft.read().sex {
                        Sex::Other(existing) => existing.clone(),
                        _ => String::new(),
                    };
                    draft.write().sex = Sex::Other(text);
                } else if let Some(sex) = SEXES.get(index) {
                    draft.write().sex = sex.clone();
                }
            },
            onreset: move |()| {
                let sex = record.seed.read().sex.clone();
                draft.write().sex = sex;
            },
        }
        if editing && is_other {
            Input {
                label: loc.sex_other(),
                name: "sex-other".to_owned(),
                value: other_text,
                oninput: move |event: FormEvent| draft.write().sex = Sex::Other(event.value()),
            }
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
    let remove_label = loc.action_label(ActionLabel::RemoveTag);
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditForm {
    /// Assert a name — `None` adds a new one, `Some(row)` edits (supersedes) an existing one.
    Name(Option<NameVm>),
    /// Attach a citation to an existing name: re-assert the same name parts (supersede) with fresh
    /// citations, via a provenance-only panel that never exposes the name fields for editing.
    CiteName(NameVm),
    /// Assert a fact — `None` adds, `Some(row)` edits (supersedes).
    Fact(Option<FactVm>),
    /// Attach a citation to an existing fact: re-assert the same type + value (supersede) with fresh
    /// citations, via a provenance-only panel that never exposes the fact fields for editing (the `s`
    /// shortcut / the fact's source-count link). Mirrors [`Self::CiteName`].
    CiteFact(FactVm),
    /// Assert an association — `None` adds, `Some(row)` edits (supersedes) the role.
    Association(Option<AssociationVm>),
    /// Change (supersede) a participation's role. Always edit — the row is opened pre-filled.
    Participation(EventRefVm),
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
    let active = use_detail_tab(Category::People, &human_id);
    let mut reload = use_signal(|| 0_u32);
    let mut editing = use_signal(|| None::<EditForm>);
    let mut retract = use_signal(|| None::<RetractTarget>);
    let mut retract_reason = use_signal(String::new);
    let saved_label = state.data_loc().action_label(ActionLabel::Saved);

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
    let record = use_record_edit::<PersonDraft>(Category::People, &human_id, &seed);

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
            vitni_ui::tab_label(Some(&detail.name), &label_human_id),
        );
    });

    let record_services = services.clone();
    let mut submit_nav = nav;
    let on_submit = use_callback(move |(edit, prov): (PersonEdit, ProvenanceDraft)| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_edit(services, edit, prov).await {
                Ok(()) => {
                    editing.set(None);
                    reload += 1;
                    submit_nav.notify(saved);
                }
                Err(message) => submit_nav.notify_error(message),
            }
        });
    });
    let saved_label_rec = state.data_loc().action_label(ActionLabel::Saved);
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
                    record_nav.notify(saved);
                }
                Err(message) => record_nav.notify_error(message),
            }
        });
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
    let on_edit_open = use_callback(move |form: EditForm| editing.set(Some(form)));
    let tag_human = human_id.clone();
    let on_tag_remove = use_callback(move |tag_id: String| {
        on_submit.call((
            PersonEdit::Tag {
                human_id: tag_human.clone(),
                tag_id,
                remove: true,
            },
            ProvenanceDraft::default(),
        ));
    });
    let retract_services = state.services().clone();
    let retract_human = human_id.clone();
    let saved_label_retract = state.data_loc().action_label(ActionLabel::Saved);
    let mut retract_nav = nav;
    let on_retract_confirm = use_callback(move |()| {
        let Some(target) = retract() else {
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
            let edit = PersonEdit::UndoAssertion {
                human_id,
                assertion_id: target.assertion_id,
            };
            let outcome = save_edit(services, edit, prov).await;
            match outcome {
                Ok(()) => {
                    retract.set(None);
                    reload += 1;
                    retract_nav.notify(saved);
                }
                Err(message) => retract_nav.notify_error(message),
            }
        });
    });

    // ⌘Z retracts the newest undoable assertion of this person's loaded change log (WP5).
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::Detail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read() || retract.read().is_some());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let undo_human = human_id.clone();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((
            PersonEdit::UndoAssertion {
                human_id: undo_human.clone(),
                assertion_id,
            },
            ProvenanceDraft::default(),
        ));
    });
    use_record_undo(
        nav,
        Category::People,
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
    use_save_on_request(EditKey::saved(Category::People, &human_id), record, save_now);

    // The Media tab's crop viewer: opening a card, and superseding its crop via `SetMediaRegion`.
    let media_viewing = use_signal(|| None::<MediaRefVm>);
    let on_view = use_callback(move |item: MediaRefVm| media_viewing.clone().set(Some(item)));
    let region_human = human_id.clone();
    let on_region = use_callback(
        move |(assertion_id, crop, caption): (String, Option<Rect>, Option<String>)| {
            on_submit.call((
                PersonEdit::SetMediaRegion {
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
                on_edit_open,
                on_undo,
                on_tag_remove,
                media_state,
            };
            person_detail(&state, &nav, detail, pane, &callbacks, &human_id)
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::DataQuality(_)
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
            | IntentOutcome::ResearchNoteDetail(_)
            | IntentOutcome::Geography(_),
        )) => rsx! {},
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
    /// The row being retracted/detached, if the retract panel is open.
    retract: Signal<Option<RetractTarget>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The two commit callbacks a person's detail wires in: one-command collection edits (attach / assert
/// / undo / restrictions) and the whole-record change-set save (the identity edit).
#[derive(Clone, Copy)]
struct PersonCallbacks {
    /// Commits one [`PersonEdit`] command.
    on_submit: Callback<(PersonEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a change-set.
    on_record_save: Callback<(PersonDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a person-side row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<EditForm>,
    /// Retracts an assertion by id from the History tab (dispatches `UndoAssertion`).
    on_undo: Callback<String>,
    /// Untags a tag by id from the Tags tab (dispatches `Tag { remove: true }`).
    on_tag_remove: Callback<String>,
    /// The Media tab's viewer state + crop-supersede wiring.
    media_state: MediaTabState,
}

/// Renders a loaded person's detail container: header (avatar, vital subtitle, restriction toggles,
/// Compare + the sticky-header record Edit/Cancel/Save), the tab strip, the active tab's content, and
/// the collection-row side panel.
fn person_detail(
    state: &AppState,
    nav: &NavState,
    detail: &PersonDetail,
    pane: PersonPane,
    callbacks: &PersonCallbacks,
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
    let on_edit_open = callbacks.on_edit_open;
    let on_undo = callbacks.on_undo;
    let on_tag_remove = callbacks.on_tag_remove;
    let media_state = callbacks.media_state;
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
    let compare_label = loc.action_button(ActionLabel::Compare);
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
            {person_tab_content(state, detail, active_id, editing, record, on_retract, on_edit_open, on_undo, on_tag_remove, media_state)}
        }
        {edit_panel(state, detail, editing, on_submit, human_id)}
        {retract_side_panel(loc, retract, retract_reason, on_retract_confirm, "detach-citation")}
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
    editing: Signal<Option<EditForm>>,
    record: RecordEditState<PersonDraft>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<EditForm>,
    on_undo: Callback<String>,
    on_tag_remove: Callback<String>,
    media_state: MediaTabState,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "names" => tab_frame(
            loc,
            Some(ActionLabel::AddName),
            TabActionTarget::Form(editing, EditForm::Name(None)),
            None,
            rsx! {
                {names_table(loc, &detail.names, on_edit_open, on_retract)}
            },
        ),
        "facts" => tab_frame(
            loc,
            Some(ActionLabel::AddFact),
            TabActionTarget::Form(editing, EditForm::Fact(None)),
            None,
            rsx! {
                {facts_table(loc, &detail.facts, on_edit_open, on_retract)}
            },
        ),
        "events" => events_table(loc, &detail.events, on_edit_open, on_retract),
        "associations" => tab_frame(
            loc,
            Some(ActionLabel::AddAssociation),
            TabActionTarget::Form(editing, EditForm::Association(None)),
            None,
            rsx! {
                {associations_table(loc, &detail.associations, on_edit_open, on_retract)}
            },
        ),
        "families" => families_panel(loc, &detail.families),
        "citations" => tab_frame(
            loc,
            Some(ActionLabel::AttachCitation),
            TabActionTarget::Form(editing, EditForm::Citation),
            None,
            rsx! {
                {citations_table::<EditForm>(loc, &detail.citations, true, on_retract)}
            },
        ),
        "media" => tab_frame(
            loc,
            Some(ActionLabel::AttachMedia),
            TabActionTarget::Form(editing, EditForm::Media),
            None,
            rsx! {
                {media_tab(loc, &detail.media, Some(on_retract), media_state)}
            },
        ),
        "notes" => tab_frame(
            loc,
            Some(ActionLabel::AttachNote),
            TabActionTarget::Form(editing, EditForm::Note),
            None,
            rsx! {
                {id_list(loc, &detail.notes, Some(on_retract))}
            },
        ),
        "tags" => tab_frame(
            loc,
            Some(ActionLabel::AddTag),
            TabActionTarget::Form(editing, EditForm::Tag),
            Some(TabActionStyle {
                emphasis: Some(ButtonVariant::Ghost),
                ..Default::default()
            }),
            tags_panel(loc, &detail.tags, on_tag_remove),
        ),
        "research-notes" => rsx! {
            ResearchNotesTab {
                category: Category::People,
                human_id: detail.human_id.clone(),
                rows: detail.research_notes.clone(),
            }
        },
        "timeline" => timeline_panel(loc, &detail.timeline),
        "history" => history_panel(loc, &detail.history, Some(on_undo)),
        _ => person_overview(loc, detail, record),
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
pub fn names_table(
    loc: &Localizer,
    names: &[NameVm],
    onedit: Callback<EditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if names.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            caption: loc.tab_label("names"),
            headers: vec![
                loc.field_label("name-type"),
                loc.label_name(),
                format!("{} / {}", loc.field_label("date"), loc.field_label("language")),
                loc.field_label("confidence"),
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
                            {
                                let cite_name = name.clone();
                                rsx! {
                                    SourceLink { label: loc.source_count(name.source_count), onclick: move |_| onedit.call(EditForm::CiteName(cite_name.clone())) }
                                }
                            }
                        } else {
                            NoSourceFlag { label: loc.no_source() }
                        }
                    }
                    {row_actions_cell(
                        loc,
                        &name.display,
                        Some((EditForm::Name(Some(name.clone())), None)),
                        Some((EditForm::CiteName(name.clone()), "cite-name")),
                        Some(RowRetract { assertion_id: name.assertion_id.clone(), button_label: RowVerb::Retract, title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
                }
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
pub fn facts_table(
    loc: &Localizer,
    facts: &[FactVm],
    onedit: Callback<EditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if facts.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            caption: loc.tab_label("facts"),
            headers: vec![
                loc.field_label("fact-type"),
                loc.field_label("date"),
                loc.field_label("value"),
                loc.field_label("confidence"),
                loc.field_label("source"),
                String::new(),
            ],
            for fact in facts.iter() {
                {
                    // `s` on the focused fact row, and the fact's source-count link, both open the
                    // provenance-only Cite panel (re-assert the same fact with fresh citations).
                    let cite_on_key = fact.clone();
                    let cite_on_click = fact.clone();
                    rsx! {
                        tr {
                            tabindex: "0",
                            onkeydown: move |event: KeyboardEvent| {
                                let plain = !(event.modifiers().meta() || event.modifiers().ctrl() || event.modifiers().alt());
                                if let Key::Character(character) = event.key()
                                    && plain
                                    && character == "s"
                                {
                                    event.stop_propagation();
                                    onedit.call(EditForm::CiteFact(cite_on_key.clone()));
                                }
                            },
                            td { "{fact.type_label}" }
                            td { class: "muted", {fact.date.clone().unwrap_or_else(|| "—".to_owned())} }
                            td { {fact.value.clone().unwrap_or_else(|| "—".to_owned())} }
                            td {
                                ConfidenceBadge { level: fact.confidence, label: fact.confidence_label.clone() }
                            }
                            td {
                                if fact.has_source() {
                                    SourceLink { label: loc.source_count(fact.source_count), onclick: move |_| onedit.call(EditForm::CiteFact(cite_on_click.clone())) }
                                } else {
                                    NoSourceFlag { label: loc.no_source() }
                                }
                            }
                            {row_actions_cell(
                                loc,
                                &fact.type_label,
                                Some((EditForm::Fact(Some(fact.clone())), None)), None,
                                Some(RowRetract { assertion_id: fact.assertion_id.clone(), button_label: RowVerb::Retract, title: "retract", detach: false }),
                                Some(onedit),
                                onretract)}
                        }
                    }
                }
            }
        }
    }
}

/// The Events tab: each participation joined to its event — Event · Role · Age · Date · Place ·
/// Confidence · Source (the canonical Events shape, ui-review Appendix A), sorted chronologically by
/// the VM (`sorted_participations`).
///
/// Participation is owned solely by the Person aggregate (ADR 0019): every row edits the role via
/// [`EditForm::Participation`] and removes (retracts) this person's assertion (`onretract`).
pub fn events_table(
    loc: &Localizer,
    events: &[EventRefVm],
    onedit: Callback<EditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if events.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            caption: loc.tab_label("events"),
            headers: vec![
                loc.tab_label("events"),
                loc.field_label("role"),
                loc.field_label("age"),
                loc.field_label("date"),
                loc.field_label("place"),
                loc.field_label("confidence"),
                loc.field_label("source"),
                String::new(),
            ],
            for event in events.iter() {
                {events_row(loc, event, onedit, onretract)}
            }
        }
    }
}

/// One Events-tab row. Participation is person-owned (ADR 0019), so Edit supersedes and Remove
/// retracts this person's assertion.
fn events_row(
    loc: &Localizer,
    event: &EventRefVm,
    onedit: Callback<EditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    let edit = Some((EditForm::Participation(event.clone()), Some("edit-participation")));
    let retract_cb = onretract;
    rsx! {
        tr {
            td {
                RecordLink {
                    category: Category::Events,
                    human_id: event.event_id.clone(),
                    label: event.event_id.clone(),
                }
            }
            td { Chip { label: event.role_label.clone() } }
            td { class: "muted", {event.age_label.clone().unwrap_or_else(|| "—".to_owned())} }
            td { class: "muted",
                {event.date.clone().unwrap_or_else(|| "—".to_owned())}
                for attribute in event.attributes.iter() {
                    div { class: "muted", "{attribute.attribute_type}: {attribute.value}" }
                }
                for note in event.notes.iter() {
                    Chip { label: note.clone() }
                }
            }
            td { class: "muted", {event.place.clone().unwrap_or_else(|| "—".to_owned())} }
            td {
                if let Some(level) = event.confidence {
                    ConfidenceBadge { level, label: event.confidence_label.clone() }
                } else {
                    span { class: "muted", "—" }
                }
            }
            td { {source_cue(loc, event.source_count)} }
            {row_actions_cell(
                loc,
                &event.role_label,
                edit, None,
                Some(RowRetract { assertion_id: event.assertion_id.clone(), button_label: RowVerb::Remove, title: "remove-participant", detach: false }),
                Some(onedit),
                retract_cb)}
        }
    }
}

/// The Associations tab: each linked person, the role, and the evidence cues (surety + source).
pub fn associations_table(
    loc: &Localizer,
    associations: &[AssociationVm],
    onedit: Callback<EditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if associations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            caption: loc.tab_label("associations"),
            headers: vec![
                loc.field_label("association"),
                loc.field_label("relationship"),
                loc.field_label("confidence"),
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
                    {row_actions_cell(
                        loc,
                        &association.role_label,
                        Some((EditForm::Association(Some(association.clone())), None)), None,
                        Some(RowRetract { assertion_id: association.assertion_id.clone(), button_label: RowVerb::Retract, title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
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

/// The Timeline tab (PR44): the person's facts and event participations merged into one
/// chronological list — [`timeline_rows`](vitni_ui::PersonDetail) orders it oldest-first with
/// undated rows last — each row read-only with the same confidence + source cues the Facts and
/// Events tabs carry.
///
/// Distinct from the History tab: the leading section-note states this is the genealogical life
/// story derived from facts + events, not the who/when/why audit trail of every change.
pub fn timeline_panel(loc: &Localizer, rows: &[TimelineRowVm]) -> Element {
    if rows.is_empty() {
        return rsx! {
            div { class: "section-note", "{loc.timeline_note()}" }
            EmptyState { message: loc.tab_empty() }
        };
    }
    rsx! {
        div { class: "section-note", "{loc.timeline_note()}" }
        Table {
            caption: loc.tab_label("timeline"),
            headers: vec![
                loc.field_label("date"),
                loc.field_label("type"),
                loc.field_label("description"),
                loc.field_label("confidence"),
                loc.field_label("source"),
            ],
            for row in rows.iter() {
                {timeline_row(loc, row)}
            }
        }
    }
}

/// One Timeline row: the date, a Fact/Event kind chip, the entry (an event link + role, or the fact
/// type) with its value/place, then the confidence badge and source cue.
fn timeline_row(loc: &Localizer, row: &TimelineRowVm) -> Element {
    rsx! {
        tr {
            td { class: "muted", {row.date.clone().unwrap_or_else(|| "—".to_owned())} }
            td {
                Chip { label: row.kind_label.clone() }
            }
            td {
                if let Some(event_id) = row.event_id.clone() {
                    RecordLink {
                        category: Category::Events,
                        human_id: event_id.clone(),
                        label: event_id,
                    }
                    span { class: "muted", " · {row.type_label}" }
                } else {
                    span { "{row.type_label}" }
                }
                if let Some(detail) = row.detail.clone() {
                    span { class: "muted", " · {detail}" }
                }
            }
            td {
                ConfidenceBadge { level: row.confidence, label: row.confidence_label.clone() }
            }
            td { {source_cue(loc, row.source_count)} }
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
    let title = match &form {
        EditForm::Name(None) => loc.action_label(ActionLabel::AddName),
        EditForm::Name(Some(_)) => loc.panel_title("edit-name"),
        EditForm::CiteName(_) => loc.panel_title("cite-name"),
        EditForm::Fact(None) => loc.action_label(ActionLabel::AddFact),
        EditForm::Fact(Some(_)) => loc.panel_title("edit-fact"),
        EditForm::CiteFact(_) => loc.panel_title("cite-fact"),
        EditForm::Association(None) => loc.action_label(ActionLabel::AddAssociation),
        EditForm::Association(Some(_)) => loc.panel_title("edit-association"),
        EditForm::Participation(_) => loc.panel_title("edit-participation"),
        EditForm::Citation => loc.action_label(ActionLabel::AttachCitation),
        EditForm::Media => loc.action_label(ActionLabel::AttachMedia),
        EditForm::Note => loc.action_label(ActionLabel::AttachNote),
        EditForm::Tag => loc.action_label(ActionLabel::AddTag),
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
                EditForm::Name(seed) => rsx! { AddNameForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::CiteName(name) => rsx! { CiteNameForm { human_id, name, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Fact(seed) => rsx! { AddFactForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::CiteFact(fact) => rsx! { CiteFactForm { human_id, fact, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Association(seed) => rsx! { AssociationForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Participation(seed) => {
                    let event_id = seed.event_id.clone();
                    let participation_seed = ParticipationSeed {
                        role: seed.role.clone(),
                        age: seed.age.clone(),
                        attributes: seed.attributes.clone(),
                        notes: seed.notes.clone(),
                        supersedes: Some(seed.assertion_id.clone()),
                    };
                    rsx! {
                        ParticipationForm {
                            seed: participation_seed,
                            onsubmit: move |(fields, prov): (NewParticipation, ProvenanceDraft)| {
                                on_submit.call((
                                    PersonEdit::AssertParticipation {
                                        human_id: human_id.clone(),
                                        event_id: event_id.clone(),
                                        role: fields.role,
                                        age: fields.age,
                                        attributes: fields.attributes,
                                        notes: fields.notes,
                                    },
                                    prov,
                                ));
                            },
                        }
                    }
                }
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
                        onsubmit.call((PersonEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

/// The name side-panel form → [`PersonEdit::AssertName`]. `seed: None` adds a new name; `Some(row)`
/// pre-fills every field and seeds the provenance draft's `supersedes` with the row's assertion id, so
/// Save supersedes (replaces) rather than appends (ADR 0004 §2).
#[component]
fn AddNameForm(
    human_id: String,
    seed: Option<NameVm>,
    onsubmit: EventHandler<(PersonEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let name_type = seed
        .as_ref()
        .map_or(vitni_app::NameType::BirthName, |s| s.name_type.clone());
    let surname_prefix = seed.as_ref().and_then(|s| s.surname_prefix.clone());
    let mut given = use_signal(|| seed.as_ref().and_then(|s| s.given.clone()).unwrap_or_default());
    let mut surname = use_signal(|| seed.as_ref().and_then(|s| s.surname.clone()).unwrap_or_default());
    let mut nickname = use_signal(|| seed.as_ref().and_then(|s| s.nickname.clone()).unwrap_or_default());
    let mut prefix = use_signal(|| seed.as_ref().and_then(|s| s.name_prefix.clone()).unwrap_or_default());
    let mut suffix = use_signal(|| seed.as_ref().and_then(|s| s.suffix.clone()).unwrap_or_default());
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|s| s.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let save_label = loc.action_button(ActionLabel::Save);
    rsx! {
        Input { label: loc.label_given(), name: "given".to_owned(), value: Some(given()), oninput: move |event: FormEvent| given.set(event.value()) }
        Input { label: loc.label_surname(), name: "surname".to_owned(), value: Some(surname()), oninput: move |event: FormEvent| surname.set(event.value()) }
        Input { label: loc.field_label("nickname"), name: "nickname".to_owned(), value: Some(nickname()), oninput: move |event: FormEvent| nickname.set(event.value()) }
        Input { label: loc.field_label("prefix"), name: "prefix".to_owned(), value: Some(prefix()), oninput: move |event: FormEvent| prefix.set(event.value()) }
        Input { label: loc.field_label("suffix"), name: "suffix".to_owned(), value: Some(suffix()), oninput: move |event: FormEvent| suffix.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let name = PersonNameParts {
                    name_type: name_type.clone(),
                    given: non_empty(given()),
                    surname_prefix: surname_prefix.clone(),
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

/// Reconstructs the [`PersonNameParts`] of an already-asserted name from its view-model, unchanged —
/// the Cite path re-asserts the identical name with fresh citations, so it must not alter any part.
fn name_parts_of(name: &NameVm) -> PersonNameParts {
    PersonNameParts {
        name_type: name.name_type.clone(),
        given: name.given.clone(),
        surname_prefix: name.surname_prefix.clone(),
        surname: name.surname.clone(),
        nickname: name.nickname.clone(),
        prefix: name.name_prefix.clone(),
        suffix: name.suffix.clone(),
    }
}

/// The per-name "Cite" side-panel form (`person.html` Names): a provenance-only panel that attaches a
/// citation to an existing name assertion. It shows the name read-only (never an editable field, so the
/// user cannot accidentally alter it) and re-asserts the same [`PersonNameParts`] via
/// [`PersonEdit::AssertName`], seeding the provenance draft's `supersedes` with the row's assertion id
/// so Save supersedes the name in place with the added citation (ADR 0004 §2).
#[component]
fn CiteNameForm(human_id: String, name: NameVm, onsubmit: EventHandler<(PersonEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let save_label = loc.action_button(ActionLabel::Save);
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: Some(name.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let parts = name_parts_of(&name);
    rsx! {
        div { class: "fact-row",
            Chip { label: name.type_label.clone() }
            span { class: "grow", "{name.display}" }
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                onsubmit.call((PersonEdit::AssertName { human_id: human_id.clone(), name: parts.clone() }, prov()));
            },
        }
    }
}

/// The per-fact "Cite" side-panel form (the `s` shortcut on a focused fact, or the fact's source-count
/// link): a provenance-only panel that attaches a citation to an existing fact assertion. It shows the
/// fact read-only (never an editable field) and re-asserts the same type + value via
/// [`PersonEdit::AssertFact`], seeding the provenance draft's `supersedes` with the row's assertion id
/// so Save supersedes the fact in place with the added citation (ADR 0004 §2). Mirrors [`CiteNameForm`].
///
/// Note: `PersonEdit::AssertFact` carries no date (a pre-existing PR29 gap), so this re-assertion does
/// not re-send the fact's date; adding structured dates to `AssertFact` is a separate follow-up.
#[component]
fn CiteFactForm(human_id: String, fact: FactVm, onsubmit: EventHandler<(PersonEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let save_label = loc.action_button(ActionLabel::Save);
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: Some(fact.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let fact_type = fact.fact_type.clone();
    let value = fact.value.clone();
    let display = fact.value.clone().unwrap_or_else(|| "—".to_owned());
    rsx! {
        div { class: "fact-row",
            span { class: "field-label", style: "width:96px;margin:0", "{fact.type_label}" }
            span { class: "grow", "{display}" }
        }
        {provenance_block_dna(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                onsubmit.call((
                    PersonEdit::AssertFact { human_id: human_id.clone(), fact_type: fact_type.clone(), value: value.clone() },
                    prov(),
                ));
            },
        }
    }
}

/// The "Add fact" side-panel form: type + value → [`PersonEdit::AssertFact`]. Confidence and the
/// backing citation are captured by the shared provenance block (PR25), not here.
#[component]
fn AddFactForm(
    human_id: String,
    seed: Option<FactVm>,
    onsubmit: EventHandler<(PersonEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let fact_choices = loc.fact_type_choices();
    let seed_index = seed
        .as_ref()
        .and_then(|s| fact_choices.iter().position(|(kind, _)| *kind == s.fact_type))
        .unwrap_or(0);
    let mut fact_index = use_signal(|| seed_index);
    let mut value = use_signal(|| seed.as_ref().and_then(|s| s.value.clone()).unwrap_or_default());
    let selected_fact = seed_index.to_string();
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|s| s.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let fact_options: Vec<SelectChoice> = fact_choices
        .iter()
        .enumerate()
        .map(|(index, (_, label))| SelectChoice {
            value: index.to_string(),
            label: label.clone(),
        })
        .collect();
    let save_label = loc.action_button(ActionLabel::Save);
    rsx! {
        Select {
            label: loc.field_label("fact-type"),
            name: "fact-type".to_owned(),
            value: Some(selected_fact),
            options: fact_options,
            onchange: move |event: FormEvent| fact_index.set(event.value().parse().unwrap_or(0)),
        }
        Input { label: loc.field_label("value"), name: "value".to_owned(), value: Some(value()), oninput: move |event: FormEvent| value.set(event.value()) }
        {provenance_block_dna(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let fact_type = fact_choices
                    .get(fact_index())
                    .map_or(vitni_app::FactType::Occupation, |(kind, _)| kind.clone());
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

/// The association form → [`PersonEdit::AssertAssociation`]. `seed: None` adds (a People picker + role);
/// `Some(row)` edits the role of an existing association (the other person is fixed) and seeds the
/// supersede target so Save replaces rather than appends.
#[component]
fn AssociationForm(
    human_id: String,
    seed: Option<AssociationVm>,
    onsubmit: EventHandler<(PersonEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let choices = loc.association_role_choices();
    let seed_index = seed
        .as_ref()
        .and_then(|s| choices.iter().position(|(role, _)| *role == s.role))
        .unwrap_or(0);
    let mut role_index = use_signal(|| seed_index);
    let options: Vec<SelectChoice> = choices
        .iter()
        .enumerate()
        .map(|(index, (_, label))| SelectChoice {
            value: index.to_string(),
            label: label.clone(),
        })
        .collect();
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|s| s.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    // Edit mode fixes the other person; add mode offers a find-or-create picker.
    let fixed_other = seed.as_ref().map(|s| s.other_id.clone());
    let attach = use_attach_picker(
        services.clone(),
        Category::People,
        loc.field_label("association"),
        "other".to_owned(),
        loc.picker_entity(Category::People),
        Vec::new(),
    );
    let save_label = loc.action_button(ActionLabel::Save);
    let onattach = use_callback(move |other_id: String| {
        let role = choices
            .get(role_index())
            .map_or(AssociationRole::Witness, |(role, _)| role.clone());
        onsubmit.call((
            PersonEdit::AssertAssociation {
                human_id: human_id.clone(),
                other_id,
                role,
            },
            prov(),
        ));
    });
    let attach_onsave = use_attach_save(services, &attach, prov, onattach);
    let fixed_for_save = fixed_other.clone();
    let onsave = use_callback(move |()| match &fixed_for_save {
        Some(id) => onattach.call(id.clone()),
        None => attach_onsave.call(()),
    });
    let disabled = fixed_other.is_none() && (*attach.link.saving.read() || !link_is_savable(&attach.link.link.read()));
    rsx! {
        if let Some(other) = &fixed_other {
            div { class: "field",
                label { "{loc.field_label(\"association\")}" }
                RecordLink { category: Category::People, human_id: other.clone(), label: other.clone() }
            }
        } else {
            {attach_link_field(loc, &attach)}
        }
        Select {
            label: loc.field_label("relationship"),
            name: "role".to_owned(),
            value: Some(seed_index.to_string()),
            options,
            onchange: move |event: FormEvent| role_index.set(event.value().parse().unwrap_or(0)),
        }
        {provenance_block_dna(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            disabled,
            onclick: move |_| onsave.call(()),
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
    let onsave = use_attach_save(services, &attach, prov, onattach);
    attach_link_form(loc, &attach, rsx! {}, prov, onsave)
}

#[cfg(test)]
mod tests {
    use super::{NameVm, name_parts_of};
    use vitni_app::NameType;
    use vitni_ui::ConfidenceLevel;

    #[test]
    fn cite_reconstructs_every_name_part_unchanged() {
        let name = NameVm {
            type_label: "Also known as".to_owned(),
            display: "Johnny Smith".to_owned(),
            given: Some("Johnny".to_owned()),
            surname: Some("Smith".to_owned()),
            surname_prefix: Some("van".to_owned()),
            name_prefix: Some("Dr".to_owned()),
            suffix: Some("Jr".to_owned()),
            name_type: NameType::AlsoKnownAs,
            nickname: Some("JJ".to_owned()),
            date: None,
            language: None,
            confidence: Some(ConfidenceLevel::Normal),
            confidence_label: "Normal".to_owned(),
            source_count: 0,
            assertion_id: "0190a2b3-0000-7000-8000-000000000009".to_owned(),
        };
        let parts = name_parts_of(&name);
        assert_eq!(parts.name_type, NameType::AlsoKnownAs);
        assert_eq!(parts.given.as_deref(), Some("Johnny"));
        assert_eq!(parts.surname.as_deref(), Some("Smith"));
        assert_eq!(parts.surname_prefix.as_deref(), Some("van"));
        assert_eq!(parts.prefix.as_deref(), Some("Dr"));
        assert_eq!(parts.suffix.as_deref(), Some("Jr"));
        assert_eq!(parts.nickname.as_deref(), Some("JJ"));
    }
}
