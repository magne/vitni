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
    let create_title = chrome.list_new();
    let cancel_label = state.data_loc().action_label("cancel");
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
    rsx! {
        MasterDetail { list: list_pane, detail: rsx! { RecordDetail {} } }
        if creating() {
            SidePanel {
                title: create_title,
                open: true,
                close_label: cancel_label,
                onclose: move |_| creating.set(false),
                footer: rsx! {},
                PersonDialog { seed: PersonDraft::new(), onsubmit: move |payload| on_create.call(payload), oncancel: move |()| creating.set(false) }
            }
        }
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: dismiss_label,
            onaction: move |_| toast.set(None),
        }
    }
}

/// The deferred person create/edit dialog. Buffers every field locally (name sub-form, gender,
/// optional `human_id`, tag multi-select, and an optional citation for the name) and emits a
/// [`PersonChangeSetRequest`] only when the operator presses Save. Cancel drops the buffer with no
/// side effects. `seed` is empty for New and pre-populated for Edit (its `existing_human_id` drives
/// which mode the change-set commits).
#[component]
fn PersonDialog(
    seed: PersonDraft,
    onsubmit: EventHandler<(PersonChangeSetRequest, ProvenanceDraft)>,
    oncancel: EventHandler<()>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let editing = seed.existing_human_id.is_some();
    let prov = use_signal(ProvenanceDraft::default);

    // Every buffered field is a local signal; nothing persists until Save.
    let name_types = loc.name_type_choices();
    let seed_name_type = seed.name_type.clone();
    let mut name_type_index = use_signal(|| {
        name_types
            .iter()
            .position(|(kind, _)| kind == &seed_name_type)
            .unwrap_or(0)
    });
    let mut prefix = use_signal(|| seed.prefix.clone());
    let mut given = use_signal(|| seed.given.clone());
    let mut nickname = use_signal(|| seed.nickname.clone());
    let mut surname_prefix = use_signal(|| seed.surname_prefix.clone());
    let mut surname = use_signal(|| seed.surname.clone());
    let mut suffix = use_signal(|| seed.suffix.clone());
    let mut human_id_override = use_signal(|| seed.human_id_override.clone());

    let sexes = [Sex::Female, Sex::Male, Sex::Unknown, Sex::Intersex];
    let seed_sex = seed.sex.clone();
    let mut sex_index = use_signal(|| sexes.iter().position(|s| s == &seed_sex).unwrap_or(2));

    let selected_tags = use_signal(|| seed.tags.clone());

    // Name-citation: "none" | "existing" | "new"; plus the pending citation's fields.
    let mut citation_mode = use_signal(|| "none".to_owned());
    let mut existing_citation = use_signal(String::new);
    let mut new_source_title = use_signal(String::new);
    let mut new_source_existing = use_signal(String::new);
    let mut citation_page = use_signal(String::new);

    let tags_resource = use_resource(move || {
        let services = services.clone();
        async move { load_tags(services).await }
    });

    let name_type_options: Vec<SelectChoice> = name_types
        .iter()
        .enumerate()
        .map(|(index, (_, label))| SelectChoice {
            value: index.to_string(),
            label: label.clone(),
        })
        .collect();
    let sex_options: Vec<SelectChoice> = sexes
        .iter()
        .enumerate()
        .map(|(index, sex)| SelectChoice {
            value: index.to_string(),
            label: loc.sex_label(Some(sex)),
        })
        .collect();
    let citation_options = vec![
        SelectChoice {
            value: "none".to_owned(),
            label: loc.dialog_no_citation(),
        },
        SelectChoice {
            value: "existing".to_owned(),
            label: loc.dialog_attach_existing_citation(),
        },
        SelectChoice {
            value: "new".to_owned(),
            label: loc.dialog_new_citation(),
        },
    ];

    let save_label = loc.action_label("save");
    let existing_seed = seed.existing_human_id.clone();
    let name_types_for_submit = name_types.clone();

    rsx! {
        h4 { class: "field-label", "{loc.section_preferred_name()}" }
        Select {
            label: loc.field_label("name-type"),
            name: "name-type".to_owned(),
            value: Some(name_type_index().to_string()),
            options: name_type_options,
            onchange: move |event: FormEvent| name_type_index.set(event.value().parse().unwrap_or(0)),
        }
        Input { label: loc.field_label("prefix"), name: "prefix".to_owned(), value: Some(prefix()), oninput: move |event: FormEvent| prefix.set(event.value()) }
        Input { label: loc.label_given(), name: "given".to_owned(), value: Some(given()), oninput: move |event: FormEvent| given.set(event.value()) }
        Input { label: loc.field_label("nickname"), name: "nickname".to_owned(), value: Some(nickname()), oninput: move |event: FormEvent| nickname.set(event.value()) }
        Input { label: loc.field_surname_prefix(), name: "surname-prefix".to_owned(), value: Some(surname_prefix()), oninput: move |event: FormEvent| surname_prefix.set(event.value()) }
        Input { label: loc.label_surname(), name: "surname".to_owned(), value: Some(surname()), oninput: move |event: FormEvent| surname.set(event.value()) }
        Input { label: loc.field_label("suffix"), name: "suffix".to_owned(), value: Some(suffix()), oninput: move |event: FormEvent| suffix.set(event.value()) }

        h4 { class: "field-label", "{loc.section_gender()}" }
        Select {
            label: loc.label_sex(),
            name: "sex".to_owned(),
            value: Some(sex_index().to_string()),
            options: sex_options,
            onchange: move |event: FormEvent| sex_index.set(event.value().parse().unwrap_or(2)),
        }

        if !editing {
            Input { label: loc.field_human_id(), name: "human-id".to_owned(), value: Some(human_id_override()), oninput: move |event: FormEvent| human_id_override.set(event.value()) }
        }

        h4 { class: "field-label", "{loc.section_name_source()}" }
        Select {
            label: loc.field_label("source"),
            name: "name-citation".to_owned(),
            value: Some(citation_mode()),
            options: citation_options,
            onchange: move |event: FormEvent| citation_mode.set(event.value()),
        }
        if citation_mode() == "existing" {
            Input { label: loc.field_label("citation"), name: "existing-citation".to_owned(), value: Some(existing_citation()), oninput: move |event: FormEvent| existing_citation.set(event.value()) }
        }
        if citation_mode() == "new" {
            Input { label: loc.field_label("source"), name: "new-source".to_owned(), value: Some(new_source_existing()), placeholder: Some(loc.action_new_source()), oninput: move |event: FormEvent| new_source_existing.set(event.value()) }
            Input { label: loc.field_label("title"), name: "new-source-title".to_owned(), value: Some(new_source_title()), oninput: move |event: FormEvent| new_source_title.set(event.value()) }
            Input { label: loc.field_label("page"), name: "citation-page".to_owned(), value: Some(citation_page()), oninput: move |event: FormEvent| citation_page.set(event.value()) }
        }

        h4 { class: "field-label", "{loc.section_tags()}" }
        {tag_multiselect(loc, tags_resource, selected_tags)}

        {provenance_block(loc, prov)}

        div { class: "sp-foot",
            Button { label: loc.action_label("cancel"), variant: ButtonVariant::Default, onclick: move |_| oncancel.call(()) }
            Button {
                label: save_label,
                variant: ButtonVariant::Primary,
                onclick: move |_| {
                    let name_type = name_types_for_submit
                        .get(name_type_index())
                        .map_or(NameType::BirthName, |(kind, _)| kind.clone());
                    let sex = sexes.get(sex_index()).cloned().unwrap_or(Sex::Unknown);
                    let name_citation = match citation_mode().as_str() {
                        "existing" => DraftNameCitation::Existing(existing_citation()),
                        "new" => DraftNameCitation::New,
                        _ => DraftNameCitation::None,
                    };
                    let pending_citation = if citation_mode() == "new" {
                        Some(DraftCitation {
                            placeholder: PersonDraft::PENDING_KEY.to_owned(),
                            source_human_id: new_source_existing(),
                            new_source_title: new_source_title(),
                            page: citation_page(),
                        })
                    } else {
                        None
                    };
                    let draft = PersonDraft {
                        existing_human_id: existing_seed.clone(),
                        human_id_override: human_id_override(),
                        name_type,
                        prefix: prefix(),
                        given: given(),
                        nickname: nickname(),
                        call_name: String::new(),
                        surname_prefix: surname_prefix(),
                        surname: surname(),
                        suffix: suffix(),
                        sex,
                        tags: selected_tags(),
                        name_citation,
                        pending_citation,
                    };
                    onsubmit.call((draft.to_request(), prov()));
                },
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

/// Which edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditForm {
    /// Edit the person's identity (primary name + sex) — the detail-head Edit action.
    Identity,
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
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let mut editing = use_signal(|| None::<EditForm>);
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

    let edit_services = services.clone();
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
    let saved_label_cs = state.data_loc().action_label("saved");
    let on_change_set = use_callback(move |(request, prov): (PersonChangeSetRequest, ProvenanceDraft)| {
        let services = edit_services.clone();
        let saved = saved_label_cs.clone();
        spawn(async move {
            match commit_person_change_set(services, request, prov).await {
                Ok(_) => {
                    editing.set(None);
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
            let callbacks = PersonCallbacks {
                on_submit,
                on_change_set,
            };
            person_detail(&state, &nav, detail, active, editing, callbacks, &human_id)
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

/// The two commit callbacks a person's detail wires into its edit affordances: single-command edits
/// (attach/tag/undo/restrictions) and the full change-set dialog (the identity edit).
#[derive(Clone, Copy)]
struct PersonCallbacks {
    /// Commits one [`PersonEdit`] command (the tab attach forms, restriction toggles, undo).
    on_submit: Callback<(PersonEdit, ProvenanceDraft)>,
    /// Commits the buffered person dialog as a change-set (the identity edit).
    on_change_set: Callback<(PersonChangeSetRequest, ProvenanceDraft)>,
}

/// Renders a loaded person's detail container: header (avatar, vital subtitle, restriction toggles,
/// Edit/Compare actions), the tab strip, the active tab's content, and the editing side panel.
fn person_detail(
    state: &AppState,
    nav: &NavState,
    detail: &PersonDetail,
    active: Signal<usize>,
    mut editing: Signal<Option<EditForm>>,
    callbacks: PersonCallbacks,
    human_id: &str,
) -> Element {
    let on_submit = callbacks.on_submit;
    let loc = state.data_loc();
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
    let edit_label = loc.action_label("edit");
    let compare_label = loc.action_label("compare");
    let mut compare_nav = *nav;
    let actions = rsx! {
        Button { label: compare_label, variant: ButtonVariant::Default, onclick: move |_| compare_nav.go_to(Destination::Tool(Tool::Merge)) }
        Button { label: edit_label, variant: ButtonVariant::Primary, onclick: move |_| editing.set(Some(EditForm::Identity)) }
    };
    rsx! {
        DetailContainer {
            title: detail.name.clone(),
            subtitle,
            id_label: detail.human_id.clone(),
            badges: vec![detail.evidence_level_label.clone()],
            avatar: person_initials(detail),
            extras: restriction_toggles(loc, detail, on_submit, human_id),
            actions,
            tabs: tab_items,
            active,
            {person_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {edit_panel(state, detail, editing, callbacks, human_id)}
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
fn person_tab_content(
    state: &AppState,
    detail: &PersonDetail,
    tab_id: &str,
    mut editing: Signal<Option<EditForm>>,
    on_submit: Callback<(PersonEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "names" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-name"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Name)) }
            }
            {names_table(loc, &detail.names)}
        },
        "facts" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-fact"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Fact)) }
            }
            {facts_table(loc, &detail.facts)}
        },
        "events" => events_table(loc, &detail.events),
        "associations" => associations_table(loc, &detail.associations),
        "families" => families_panel(loc, &detail.families),
        "citations" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-citation"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Citation)) }
            }
            {person_citations_table(loc, &detail.citations)}
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
        "tags" => tags_panel(loc, &detail.tags),
        "history" => history_tab(loc, detail, on_submit, human_id),
        _ => overview_tab(loc, detail),
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
pub fn names_table(loc: &Localizer, names: &[NameVm]) -> Element {
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
pub fn facts_table(loc: &Localizer, facts: &[FactVm]) -> Element {
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
                }
            }
        }
    }
}

/// The Events tab: each participation's role and the joined event id + date.
pub fn events_table(loc: &Localizer, events: &[EventRefVm]) -> Element {
    if events.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![loc.tab_label("events"), loc.field_label("role"), loc.field_label("date")],
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
                }
            }
        }
    }
}

/// The Associations tab: each linked person, the role, and the evidence cues (surety + source).
pub fn associations_table(loc: &Localizer, associations: &[AssociationVm]) -> Element {
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
                }
            }
        }
    }
}

/// The Citations tab: each backing citation's id, cited source, surety, and Evidence Explained axes
/// — the research-grade-citation differentiator surfaced on the person.
pub fn person_citations_table(loc: &Localizer, citations: &[CitationRefVm]) -> Element {
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

/// The editing side panel: renders the form for the currently-open [`EditForm`], or nothing. The
/// `Identity` form is the deferred create/edit dialog (buffered, change-set commit); the rest are the
/// still-immediate attach affordances (out of scope for the change-set slice).
fn edit_panel(
    state: &AppState,
    detail: &PersonDetail,
    mut editing: Signal<Option<EditForm>>,
    callbacks: PersonCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let on_submit = callbacks.on_submit;
    let on_change_set = callbacks.on_change_set;
    let title = match form {
        EditForm::Identity => loc.dialog_person_title(true),
        EditForm::Name => loc.action_label("add-name"),
        EditForm::Fact => loc.action_label("add-fact"),
        EditForm::Citation => loc.action_label("attach-citation"),
        EditForm::Media => loc.action_label("attach-media"),
        EditForm::Note => loc.action_label("attach-note"),
    };
    let human_id = human_id.to_owned();
    let seed = detail.edit_seed.clone();
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |_| editing.set(None),
            footer: rsx! {},
            {match form {
                EditForm::Identity => rsx! { PersonDialog { seed: seed.clone(), onsubmit: move |payload| on_change_set.call(payload), oncancel: move |()| editing.set(None) } },
                EditForm::Name => rsx! { AddNameForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Fact => rsx! { AddFactForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Citation => rsx! { AttachForm { human_id, kind: EditForm::Citation, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Media => rsx! { AttachForm { human_id, kind: EditForm::Media, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Note => rsx! { AttachForm { human_id, kind: EditForm::Note, onsubmit: move |edit| on_submit.call(edit) } },
            }}
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
    let field = match kind {
        EditForm::Media => "media",
        EditForm::Note => "note",
        _ => "citation",
    };
    let mut id = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label(field), name: field.to_owned(), oninput: move |event: FormEvent| id.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let id = id();
                if id.is_empty() {
                    return;
                }
                let edit = match kind {
                    EditForm::Media => PersonEdit::AttachMedia { human_id: human_id.clone(), media_id: id },
                    EditForm::Note => PersonEdit::AttachNote { human_id: human_id.clone(), note_id: id },
                    _ => PersonEdit::AttachCitation { human_id: human_id.clone(), citation_id: id },
                };
                onsubmit.call((edit, prov()));
            },
        }
    }
}
