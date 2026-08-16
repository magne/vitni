//! The nested "+ New …" draft card an attach picker opens (`record-editing.html` §6b, issue #314):
//! the create half of the find-or-create attach mechanism. A record-detail side panel's attach picker
//! (`screens::shared::use_attach_picker`) offers "+ New …" alongside its existing-record search; picking
//! it seeds a [`NewRecordDraft`] from the typed query and mounts [`NewRecordCard`] over it. Nothing is
//! written here — the panel's own Save resolves the link (`screens::shared::use_attach_save`), commits
//! the draft, and only then attaches it to the panel's root record.
//!
//! A `#[component]`, not a plain fn, for the same reason [`ProvenanceNewCitation`](super::provenance) is
//! one: the Citation body owns a nested Sources picker (its own resource load, its own "+ New source"
//! cascade), so a conditionally-mounted component scope means every other category's body pays nothing
//! for it, and hook order cannot drift across the New/Empty/Existing branching above it. `AppCtx` is
//! resolved with `try_consume_context` and falls back to a baseline [`Localizer`] exactly as
//! [`ProvenanceNewCitation`](super::provenance) does, so the SSR tests render every category's card with
//! no app.

use dioxus::prelude::*;
use vitni_app::EventType;
use vitni_ui::{
    Category, Localizer, NEW_EVENT_TYPES, NEW_PLACE_TYPES, NewCitationFields, NewEventFields, NewMediaFields,
    NewNoteFields, NewPersonFields, NewPlaceFields, NewRecordDraft, NewRepositoryFields, NewSourceFields,
    PickerSelection, PickerState, RecordLink,
};

use crate::app::AppCtx;
use crate::components::record_picker::{
    PickerCallbacks, PickerConfig, RecordPicker, draft_card, picker_options, record_picker,
};
use crate::components::{Input, Select, SelectChoice, Textarea};
use crate::services::{Services, load_picker_rows};
use crate::shell::nav_state::{NavState, data_version_ticket};

/// The nested "+ New …" draft card: mounted only while `link` holds [`RecordLink::New`] (renders
/// nothing otherwise, so an `Empty`/`Existing` link never pays for this component's hooks). Its body is
/// one of the eight per-category field sets; a failed Save's error (set by
/// `screens::shared::use_attach_save`) renders inside the card, below the fields, exactly as
/// [`ProvenanceNewCitation`](super::provenance)'s commit error does.
#[component]
pub fn NewRecordCard(
    /// The link this card edits.
    mut link: Signal<RecordLink<NewRecordDraft>>,
    /// The last Save attempt's create failure, already localized.
    error: Signal<Option<String>>,
    /// Fired when the operator discards the draft (the card's ✕ control).
    onclose: Callback<()>,
) -> Element {
    let ctx = try_consume_context::<AppCtx>();
    let services = ctx_services(ctx.as_ref());
    let nav = try_consume_context::<NavState>();
    let fallback;
    let loc: &Localizer = match &ctx {
        Some(AppCtx::Ready(state)) => state.data_loc(),
        Some(AppCtx::Failed(_)) | None => {
            fallback = Localizer::with_languages(None, &[]);
            &fallback
        }
    };
    let draft = match &*link.read() {
        RecordLink::New(draft) => draft.clone(),
        RecordLink::Empty | RecordLink::Existing(_) => return rsx! {},
    };
    let title = new_record_title(loc, &draft);
    let fields = new_record_fields(loc, services, nav.as_ref(), link, &draft);
    let body = rsx! {
        {fields}
        if let Some(message) = error() {
            span { class: "field-error", role: "alert", "{message}" }
        }
    };
    draft_card(
        &title,
        &loc.draft_card_badge(),
        loc.draft_card_discard(&title),
        onclose,
        body,
    )
}

/// The services handle from an [`AppCtx`], or `None` when startup failed or no context is present (an
/// SSR render — the Citation body's nested source picker then loads no options).
fn ctx_services(ctx: Option<&AppCtx>) -> Option<Services> {
    match ctx {
        Some(AppCtx::Ready(state)) => Some(state.services().clone()),
        Some(AppCtx::Failed(_)) | None => None,
    }
}

/// The card header title for `draft`'s category — one of the eight already-localized `*_new_title`s.
fn new_record_title(loc: &Localizer, draft: &NewRecordDraft) -> String {
    match draft {
        NewRecordDraft::Person(_) => loc.person_new_title(),
        NewRecordDraft::Place(_) => loc.place_new_title(),
        NewRecordDraft::Source(_) => loc.source_new_title(),
        NewRecordDraft::Citation(_) => loc.citation_new_title(),
        NewRecordDraft::Note(_) => loc.note_new_title(),
        NewRecordDraft::Media(_) => loc.media_new_title(),
        NewRecordDraft::Event(_) => loc.event_new_title(),
        NewRecordDraft::Repository(_) => loc.repository_new_title(),
    }
}

/// Applies `apply` to the draft `link` holds, a no-op when it is not (or no longer) [`RecordLink::New`]
/// — every field handler below goes through this rather than repeating the match.
fn with_new<F: FnOnce(&mut NewRecordDraft)>(mut link: Signal<RecordLink<NewRecordDraft>>, apply: F) {
    if let RecordLink::New(draft) = &mut *link.write() {
        apply(draft);
    }
}

/// Dispatches to the per-category body — the only place that matches on every [`NewRecordDraft`]
/// variant, so a ninth category is a compile error here rather than a silently blank card.
fn new_record_fields(
    loc: &Localizer,
    services: Option<Services>,
    nav: Option<&NavState>,
    link: Signal<RecordLink<NewRecordDraft>>,
    draft: &NewRecordDraft,
) -> Element {
    match draft {
        NewRecordDraft::Person(fields) => person_body(loc, link, fields),
        NewRecordDraft::Place(fields) => place_body(loc, link, fields),
        NewRecordDraft::Source(fields) => source_body(loc, link, fields),
        NewRecordDraft::Citation(fields) => citation_body(loc, services, nav, link, fields),
        NewRecordDraft::Note(fields) => note_body(loc, link, fields),
        NewRecordDraft::Media(fields) => media_body(loc, link, fields),
        NewRecordDraft::Event(fields) => event_body(loc, link, fields),
        NewRecordDraft::Repository(fields) => repository_body(loc, link, fields),
    }
}

/// The new-person fields: given name and surname, both optional individually (Save requires at least
/// one non-blank — [`NewRecordDraft::is_valid`]).
fn person_body(loc: &Localizer, link: Signal<RecordLink<NewRecordDraft>>, fields: &NewPersonFields) -> Element {
    rsx! {
        Input {
            label: loc.label_given(),
            name: "new-record-person-given".to_owned(),
            value: fields.given.clone(),
            oninput: move |event: FormEvent| {
                with_new(link, |draft| if let NewRecordDraft::Person(fields) = draft { fields.given = event.value(); });
            },
        }
        Input {
            label: loc.label_surname(),
            name: "new-record-person-surname".to_owned(),
            value: fields.surname.clone(),
            oninput: move |event: FormEvent| {
                with_new(link, |draft| if let NewRecordDraft::Person(fields) = draft { fields.surname = event.value(); });
            },
        }
    }
}

/// The new-place fields: a required type select ([`NEW_PLACE_TYPES`], defaulting to City — a place
/// always has some type) and a name input.
fn place_body(loc: &Localizer, link: Signal<RecordLink<NewRecordDraft>>, fields: &NewPlaceFields) -> Element {
    let options: Vec<SelectChoice> = NEW_PLACE_TYPES
        .iter()
        .enumerate()
        .map(|(index, place_type)| SelectChoice {
            value: index.to_string(),
            label: loc.place_type_label(place_type),
        })
        .collect();
    let selected = NEW_PLACE_TYPES
        .iter()
        .position(|place_type| place_type == &fields.place_type)
        .unwrap_or(0)
        .to_string();
    rsx! {
        Select {
            label: loc.field_label("type"),
            name: "new-record-place-type".to_owned(),
            value: Some(selected),
            options,
            onchange: move |event: FormEvent| {
                if let Some(place_type) = event.value().parse::<usize>().ok().and_then(|index| NEW_PLACE_TYPES.get(index).cloned()) {
                    with_new(link, move |draft| if let NewRecordDraft::Place(fields) = draft { fields.place_type = place_type; });
                }
            },
        }
        Input {
            label: loc.field_label("name"),
            name: "new-record-place-name".to_owned(),
            value: fields.name.clone(),
            oninput: move |event: FormEvent| {
                with_new(link, |draft| if let NewRecordDraft::Place(fields) = draft { fields.name = event.value(); });
            },
        }
    }
}

/// The new-source fields: a title input (Save requires it non-blank).
fn source_body(loc: &Localizer, link: Signal<RecordLink<NewRecordDraft>>, fields: &NewSourceFields) -> Element {
    rsx! {
        Input {
            label: loc.field_label("title"),
            name: "new-record-source-title".to_owned(),
            value: fields.title.clone(),
            oninput: move |event: FormEvent| {
                with_new(link, |draft| if let NewRecordDraft::Source(fields) = draft { fields.title = event.value(); });
            },
        }
    }
}

/// The new-citation fields: the citation's own required source cascade (a nested Sources picker that
/// can itself open a "+ New source" draft card — a citation → source cascade nested one level deeper
/// than the outer attach picker → citation cascade) plus a page input.
fn citation_body(
    loc: &Localizer,
    services: Option<Services>,
    nav: Option<&NavState>,
    link: Signal<RecordLink<NewRecordDraft>>,
    fields: &NewCitationFields,
) -> Element {
    let nav = nav.copied();
    let source_state = use_signal(PickerState::default);
    let source_rows = use_resource(move || {
        let _ = data_version_ticket(nav);
        let services = services.clone();
        async move {
            match services {
                Some(services) => load_picker_rows(services, Category::Sources).await,
                None => Ok(Vec::new()),
            }
        }
    });
    let source_picker = RecordPicker {
        config: PickerConfig {
            label: loc.field_label("source"),
            name: "new-record-citation-source".to_owned(),
            entity_label: loc.picker_entity(Category::Sources),
            allow_new: true,
        },
        state: source_state,
        options: picker_options(source_rows.read_unchecked().as_ref()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: use_callback(move |selection: PickerSelection| {
                with_new(link, |draft| {
                    if let NewRecordDraft::Citation(fields) = draft {
                        fields.source = RecordLink::Existing(selection);
                    }
                });
            }),
            onclear: use_callback(move |()| {
                with_new(link, |draft| {
                    if let NewRecordDraft::Citation(fields) = draft {
                        fields.source = RecordLink::Empty;
                    }
                });
            }),
            onnew: use_callback(move |query: String| {
                with_new(link, |draft| {
                    if let NewRecordDraft::Citation(fields) = draft {
                        fields.source = RecordLink::New(NewSourceFields { title: query });
                    }
                });
            }),
        },
    };
    let source_field = match &fields.source {
        RecordLink::New(_) => {
            let title = loc.source_new_title();
            let discard = source_picker.callbacks.onclear;
            let body = citation_new_source_body(loc, link);
            draft_card(
                &title,
                &loc.draft_card_badge(),
                loc.draft_card_discard(&title),
                discard,
                body,
            )
        }
        RecordLink::Empty | RecordLink::Existing(_) => record_picker(loc, &source_picker),
    };
    rsx! {
        {source_field}
        Input {
            label: loc.field_label("page"),
            name: "new-record-citation-page".to_owned(),
            value: fields.page.clone(),
            oninput: move |event: FormEvent| {
                with_new(link, |draft| if let NewRecordDraft::Citation(fields) = draft { fields.page = event.value(); });
            },
        }
    }
}

/// The nested new-source fields inside a new citation's source cascade: a single title input, bound to
/// `Citation(fields).source`'s own new-source link.
fn citation_new_source_body(loc: &Localizer, link: Signal<RecordLink<NewRecordDraft>>) -> Element {
    let title = match &*link.read() {
        RecordLink::New(NewRecordDraft::Citation(fields)) => match &fields.source {
            RecordLink::New(source_fields) => source_fields.title.clone(),
            RecordLink::Empty | RecordLink::Existing(_) => String::new(),
        },
        RecordLink::Empty
        | RecordLink::Existing(_)
        | RecordLink::New(
            NewRecordDraft::Person(_)
            | NewRecordDraft::Place(_)
            | NewRecordDraft::Source(_)
            | NewRecordDraft::Note(_)
            | NewRecordDraft::Media(_)
            | NewRecordDraft::Event(_)
            | NewRecordDraft::Repository(_),
        ) => String::new(),
    };
    rsx! {
        Input {
            label: loc.field_label("title"),
            name: "new-record-citation-source-title".to_owned(),
            value: title,
            oninput: move |event: FormEvent| {
                with_new(link, |draft| {
                    if let NewRecordDraft::Citation(fields) = draft
                        && let RecordLink::New(source_fields) = &mut fields.source
                    {
                        source_fields.title = event.value();
                    }
                });
            },
        }
    }
}

/// The new-note fields: a Markdown content textarea (Save requires it non-blank).
fn note_body(loc: &Localizer, link: Signal<RecordLink<NewRecordDraft>>, fields: &NewNoteFields) -> Element {
    rsx! {
        Textarea {
            label: loc.field_label("content"),
            name: "new-record-note-text".to_owned(),
            value: fields.text.clone(),
            oninput: move |event: FormEvent| {
                with_new(link, |draft| if let NewRecordDraft::Note(fields) = draft { fields.text = event.value(); });
            },
        }
    }
}

/// The new-media fields: a local file path input (Save requires it non-blank).
fn media_body(loc: &Localizer, link: Signal<RecordLink<NewRecordDraft>>, fields: &NewMediaFields) -> Element {
    rsx! {
        Input {
            label: loc.field_label("file-path"),
            name: "new-record-media-file-path".to_owned(),
            value: fields.file_path.clone(),
            oninput: move |event: FormEvent| {
                with_new(link, |draft| if let NewRecordDraft::Media(fields) = draft { fields.file_path = event.value(); });
            },
        }
    }
}

/// The new-event fields: a required type select ([`NEW_EVENT_TYPES`], with **no** default — the draft
/// stays invalid until one is chosen) and a description input.
fn event_body(loc: &Localizer, link: Signal<RecordLink<NewRecordDraft>>, fields: &NewEventFields) -> Element {
    let mut options = vec![SelectChoice {
        value: String::new(),
        label: loc.record_unset(),
    }];
    let mut selected = String::new();
    for (index, event_type) in NEW_EVENT_TYPES.iter().enumerate() {
        options.push(SelectChoice {
            value: index.to_string(),
            label: loc.event_type_label(event_type),
        });
        if fields.event_type.as_ref() == Some(event_type) {
            selected = index.to_string();
        }
    }
    rsx! {
        Select {
            label: loc.field_label("type"),
            name: "new-record-event-type".to_owned(),
            value: Some(selected),
            options,
            onchange: move |event: FormEvent| {
                let event_type: Option<EventType> = event
                    .value()
                    .parse::<usize>()
                    .ok()
                    .and_then(|index| NEW_EVENT_TYPES.get(index).cloned());
                with_new(link, move |draft| if let NewRecordDraft::Event(fields) = draft { fields.event_type = event_type; });
            },
        }
        Input {
            label: loc.field_label("description"),
            name: "new-record-event-description".to_owned(),
            value: fields.description.clone(),
            oninput: move |event: FormEvent| {
                with_new(link, |draft| if let NewRecordDraft::Event(fields) = draft { fields.description = event.value(); });
            },
        }
    }
}

/// The new-repository fields: a name input (Save requires it non-blank).
fn repository_body(loc: &Localizer, link: Signal<RecordLink<NewRecordDraft>>, fields: &NewRepositoryFields) -> Element {
    rsx! {
        Input {
            label: loc.field_label("name"),
            name: "new-record-repository-name".to_owned(),
            value: fields.name.clone(),
            oninput: move |event: FormEvent| {
                with_new(link, |draft| if let NewRecordDraft::Repository(fields) = draft { fields.name = event.value(); });
            },
        }
    }
}
