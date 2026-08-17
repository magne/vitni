//! Shared tab-content panels, one per detail-screen tab that every aggregate renders the same way.
//! Each is a pure `fn(loc, data, …) -> Element` (SSR-testable without `AppCtx`, the `shared.rs`
//! idiom) parameterised only over the per-entity variance — the edit-form enum `E` and the
//! entity-specific dispatch callbacks — so the tab markup lives here once instead of being copied
//! across the twelve screen modules.

use vitni_ui::{CitationRefVm, HistoryEntryVm};

use super::prelude::*;

/// The Citations tab shared by every record's Citations tab: source (a link to the citation's source
/// when known) · page · [Backs] · confidence · evidence axes · a Detach action (ui-review Appendix A).
/// Generic over the screen's edit-form enum `E` for the actions cell (mirrors `row_actions_cell<E>`).
///
/// `show_backs` adds the "Backs" column — how many records the citation backs — for the record types
/// whose subject is plural (Person, Family); the singular-subject records (Event, Place, Media) omit
/// it (`show_backs: false`), per the canonical-shape rule.
pub fn citations_table<E: Clone + PartialEq + 'static>(
    loc: &Localizer,
    citations: &[CitationRefVm],
    show_backs: bool,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if citations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    let mut headers = vec![loc.field_label("source"), loc.field_label("page")];
    if show_backs {
        headers.push(loc.field_label("backs"));
    }
    headers.push(loc.field_label("confidence"));
    headers.push(loc.field_label("analysis"));
    headers.push(String::new());
    rsx! {
        Table {
            caption: loc.tab_label("citations"),
            headers,
            for citation in citations.iter() {
                tr {
                    td {
                        if let Some(source_id) = &citation.source_id {
                            RecordLink {
                                category: Category::Sources,
                                human_id: source_id.clone(),
                                label: citation.source.clone().unwrap_or_else(|| source_id.clone()),
                            }
                        } else {
                            {citation.source.clone().unwrap_or_else(|| citation.human_id.clone())}
                        }
                    }
                    td { class: "muted", {citation.page.clone().unwrap_or_else(|| "—".to_owned())} }
                    if show_backs {
                        td { class: "muted", "{citation.backs_count}" }
                    }
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
                    {row_actions_cell::<E>(
                        loc,
                        &citation.human_id,
                        None,
                        None,
                        citation.assertion_id.clone().map(|id| RowRetract { assertion_id: id, button_label: RowVerb::Detach, title: "detach-citation", detach: true }),
                        None,
                        onretract,
                    )}
                }
            }
        }
    }
}

/// The Tags tab's chip rendering, shared by every aggregate that carries tags: the applied tags as
/// name + colour-dot chips, each with a delete control that fires `on_remove` with `(tag id, tag
/// name)`. The name rides along because the removal opens a panel that takes an operator rationale
/// (issue #315) and that panel names the tag: tags are referenced by name; their UUID is never
/// rendered (data-model §9), and tags never retract — untag is the only removal. The "add tag" action
/// is the caller's [`tab_frame`] bar, not this fn — the two used to be one fn with the add button baked
/// in, which was the second `.tab-actions` code path this split exists to delete.
pub fn tags_panel(loc: &Localizer, tags: &[TagRef], on_remove: Callback<(String, String)>) -> Element {
    if tags.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    let untag_title = loc.action_title("untag");
    rsx! {
        div { class: "wrap",
            for tag in tags.iter() {
                {
                    let tag_id = tag.id.clone();
                    let tag_name = tag.name.clone();
                    let untag_title = untag_title.clone();
                    rsx! {
                        Chip {
                            key: "{tag.id}",
                            label: tag.name.clone(),
                            dot_color: tag.color.clone(),
                            delete_label: loc.action_remove_tag_named(&tag.name),
                            delete_title: untag_title,
                            ondelete: move |()| on_remove.call((tag_id.clone(), tag_name.clone())),
                        }
                    }
                }
            }
        }
    }
}

/// What a [`tab_frame`] action button does when clicked, generic over the screen's edit-form enum `E`.
pub enum TabActionTarget<E> {
    /// Arms the screen's side panel: writes `form` into `editing` (mirrors `row_actions_cell<E>`'s
    /// per-row edit-open). The overwhelming majority of tabs — 38 of them.
    Form(Signal<Option<E>>, E),
    /// Runs an arbitrary action instead of arming a side panel — the Research Notes tab opens a draft
    /// tab, not a form.
    Run(Callback<()>),
    /// No action: a read-only tab renders no bar at all.
    None,
}

/// Styling overrides for a [`tab_frame`] action button, everything defaulting to the mockups' base
/// case (`Default::default()`/`None`): `emphasis` swaps the button's default `Primary` variant for a
/// lower-emphasis one (the Tags bar's `Ghost`), and `title` adds a hover tooltip (the Place succession
/// card's own).
#[derive(Debug, Clone, Default)]
pub struct TabActionStyle {
    /// Overrides the button's default [`ButtonVariant::Primary`].
    pub emphasis: Option<ButtonVariant>,
    /// An additional hover tooltip, already localized.
    pub title: Option<String>,
}

/// A collection tab's action bar (`record-editing.html` §8): the single button that opens or runs the
/// tab's one action, above the tab's own `body` — the only fn in the crate that emits `.tab-actions`,
/// so every tab resolves its label through the [`ActionLabel`] its own [`DetailTab::action`] declares
/// (issue #314 slice 3), instead of a bare `ActionLabel` picked independently at each call site — the
/// drift that once left six labels wrong. `tab.action: None` or `target: TabActionTarget::None`
/// renders `body` alone, for a read-only tab.
pub fn tab_frame<E: Clone + PartialEq + 'static>(
    loc: &Localizer,
    tab: &DetailTab,
    target: TabActionTarget<E>,
    style: Option<TabActionStyle>,
    body: Element,
) -> Element {
    let Some(action) = tab.action else {
        return body;
    };
    let style = style.unwrap_or_default();
    let variant = style.emphasis.unwrap_or(ButtonVariant::Primary);
    let label = loc.action_button(action);
    let bar = match target {
        TabActionTarget::None => return body,
        TabActionTarget::Form(mut editing, form) => rsx! {
            Button {
                label,
                variant,
                small: true,
                title: style.title.clone(),
                onclick: move |_| editing.set(Some(form.clone())),
            }
        },
        TabActionTarget::Run(on_run) => rsx! {
            Button {
                label,
                variant,
                small: true,
                title: style.title.clone(),
                onclick: move |_| on_run.call(()),
            }
        },
    };
    rsx! {
        div { class: "tab-actions", {bar} }
        {body}
    }
}

/// A placeholder [`DetailTab`] for `tabs.get(active()).cloned().unwrap_or_else(...)`: `active()` can
/// point past the end of a shorter tab list (e.g. right after switching records), and the content
/// dispatcher's `match tab.id` falls through to its default arm regardless of `label`/`count`/`action`,
/// so only `id` need be real.
#[must_use]
pub fn fallback_tab(id: &'static str) -> DetailTab {
    DetailTab {
        id,
        label: String::new(),
        count: None,
        action: None,
    }
}

/// The Addresses tab, shared by every aggregate that carries postal addresses (Repository, Event):
/// one card per recorded address — street · region · postal · country · phone · email · fax · www,
/// plus a per-card Edit (opens the row's form pre-filled via `onedit`, which the caller wraps into its
/// own edit-form enum; Save supersedes by `AssertionId`) and Retract (opens the shared retract panel
/// via `onretract`, which the assertion stays in History — ADR 0004 §2). No assertion id is ever
/// rendered.
pub fn address_cards(
    loc: &Localizer,
    addresses: &[AddressVm],
    onedit: Callback<AddressVm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if addresses.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "grid-2",
            for card in addresses.iter() {
                {
                    let address = &card.address;
                    let label = address.locality.clone().unwrap_or_else(|| loc.section_label("contact"));
                    let seed = card.clone();
                    let assertion_id = card.assertion_id.clone();
                    let retract_label = label.clone();
                    rsx! {
                        Card { title: label.clone(),
                            div { class: "tab-actions",
                                Button {
                                    label: loc.action_button(ActionLabel::Edit),
                                    variant: ButtonVariant::Ghost,
                                    small: true,
                                    aria_label: loc.action_edit_row(&label),
                                    onclick: move |_| onedit.call(seed.clone()),
                                }
                                Button {
                                    label: loc.action_button(ActionLabel::Retract),
                                    variant: ButtonVariant::Ghost,
                                    small: true,
                                    title: loc.action_title("retract"),
                                    aria_label: loc.action_retract_row(&retract_label),
                                    onclick: move |_| onretract.call((assertion_id.clone(), retract_label.clone(), false)),
                                }
                            }
                            div { class: "stack",
                                div { class: "fact-row",
                                    span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"street\")}" }
                                    span { class: "grow", {address.lines.join(", ")} }
                                }
                                div { class: "fact-row",
                                    span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"region\")}" }
                                    span { class: "grow", {address.region.clone().unwrap_or_else(|| "—".to_owned())} }
                                }
                                div { class: "fact-row",
                                    span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"postal-code\")}" }
                                    span { class: "grow mono", {address.postal_code.clone().unwrap_or_else(|| "—".to_owned())} }
                                }
                                div { class: "fact-row",
                                    span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"country\")}" }
                                    span { class: "grow", {address.country.clone().unwrap_or_else(|| "—".to_owned())} }
                                }
                                div { class: "fact-row",
                                    span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"phone\")}" }
                                    span { class: "grow mono", {address.phone.clone().unwrap_or_else(|| "—".to_owned())} }
                                }
                                div { class: "fact-row",
                                    span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"email\")}" }
                                    span { class: "grow", {address.email.clone().unwrap_or_else(|| "—".to_owned())} }
                                }
                                div { class: "fact-row",
                                    span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"fax\")}" }
                                    span { class: "grow mono", {address.fax.clone().unwrap_or_else(|| "—".to_owned())} }
                                }
                                div { class: "fact-row",
                                    span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"www\")}" }
                                    if let Some(www) = address.www.clone() {
                                        a { class: "grow", href: "{www}", "{www}" }
                                    } else {
                                        span { class: "grow muted", "—" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The shared "Add/Edit address" form body: street/locality/region/postal/country/phone/email/fax/www
/// → the built [`Address`](vitni_app::Address) plus the [`ProvenanceDraft`]. `seed: None` adds a
/// new address; `Some(card)` edits (supersedes) an existing one — every field is pre-filled and the
/// draft's `supersedes` is seeded with the card's assertion id so Save supersedes (replaces) rather
/// than appends (ADR 0004 §2). The caller wraps the emitted `(Address, ProvenanceDraft)` into its own
/// edit command (`AddAddress`), supplying the record's `human_id`. Uses hooks, so it is only rendered
/// inside the [`AddressForm`] component scope (never directly in a conditional branch of a parent).
pub fn address_form(
    loc: &Localizer,
    seed: Option<&AddressVm>,
    onsubmit: EventHandler<(Address, ProvenanceDraft)>,
) -> Element {
    let seeded = seed.map(|card| card.address.clone());
    let supersedes = seed.map(|card| card.assertion_id.clone());
    let mut street = use_signal(|| {
        seeded
            .as_ref()
            .and_then(|address| address.lines.first().cloned())
            .unwrap_or_default()
    });
    let mut locality = use_signal(|| seeded.as_ref().and_then(|a| a.locality.clone()).unwrap_or_default());
    let mut region = use_signal(|| seeded.as_ref().and_then(|a| a.region.clone()).unwrap_or_default());
    let mut postal_code = use_signal(|| seeded.as_ref().and_then(|a| a.postal_code.clone()).unwrap_or_default());
    let mut country = use_signal(|| seeded.as_ref().and_then(|a| a.country.clone()).unwrap_or_default());
    let mut phone = use_signal(|| seeded.as_ref().and_then(|a| a.phone.clone()).unwrap_or_default());
    let mut email = use_signal(|| seeded.as_ref().and_then(|a| a.email.clone()).unwrap_or_default());
    let mut fax = use_signal(|| seeded.as_ref().and_then(|a| a.fax.clone()).unwrap_or_default());
    let mut www = use_signal(|| seeded.as_ref().and_then(|a| a.www.clone()).unwrap_or_default());
    let prov = use_signal(|| ProvenanceDraft {
        supersedes,
        ..ProvenanceDraft::default()
    });
    let save_label = loc.action_button(ActionLabel::Save);
    rsx! {
        Input { label: loc.field_label("street"), name: "street".to_owned(), value: street(), oninput: move |event: FormEvent| street.set(event.value()) }
        Input { label: loc.field_label("locality"), name: "locality".to_owned(), value: locality(), oninput: move |event: FormEvent| locality.set(event.value()) }
        Input { label: loc.field_label("region"), name: "region".to_owned(), value: region(), oninput: move |event: FormEvent| region.set(event.value()) }
        Input { label: loc.field_label("postal-code"), name: "postal-code".to_owned(), value: postal_code(), oninput: move |event: FormEvent| postal_code.set(event.value()) }
        Input { label: loc.field_label("country"), name: "country".to_owned(), value: country(), oninput: move |event: FormEvent| country.set(event.value()) }
        Input { label: loc.field_label("phone"), name: "phone".to_owned(), value: phone(), oninput: move |event: FormEvent| phone.set(event.value()) }
        Input { label: loc.field_label("email"), name: "email".to_owned(), value: email(), oninput: move |event: FormEvent| email.set(event.value()) }
        Input { label: loc.field_label("fax"), name: "fax".to_owned(), value: fax(), oninput: move |event: FormEvent| fax.set(event.value()) }
        Input { label: loc.field_label("www"), name: "www".to_owned(), value: www(), oninput: move |event: FormEvent| www.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let optional = |value: String| if value.trim().is_empty() { None } else { Some(value) };
                let street_value = street();
                let lines = if street_value.trim().is_empty() { Vec::new() } else { vec![street_value] };
                let address = Address {
                    lines,
                    locality: optional(locality()),
                    region: optional(region()),
                    postal_code: optional(postal_code()),
                    country: optional(country()),
                    phone: optional(phone()),
                    email: optional(email()),
                    fax: optional(fax()),
                    www: optional(www()),
                    original_text: None,
                };
                onsubmit.call((address, prov()));
            },
        }
    }
}

/// The shared address-form component: resolves the localizer from [`AppCtx`] and renders the
/// [`address_form`] body (giving its hooks an isolated scope, so a screen's edit panel can mount it
/// conditionally). The emitted `(Address, ProvenanceDraft)` is forwarded to `onsubmit`; the screen's
/// panel wraps it into its own `AddAddress` edit command.
#[component]
pub fn AddressForm(seed: Option<AddressVm>, onsubmit: EventHandler<(Address, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    address_form(loc, seed.as_ref(), onsubmit)
}

/// The seed for the shared [`participation_form`]: the current role and participant-scoped detail (age,
/// attributes, notes) to pre-fill, plus the assertion the Save supersedes. The person screen seeds this
/// from an [`EventRefVm`](vitni_ui::EventRefVm) (always an edit → `supersedes: Some`); the event
/// screen seeds it from a `ParticipantVm` for an edit or leaves it [`ParticipationSeed::empty`] for an
/// add (`supersedes: None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipationSeed {
    /// The participant's role to pre-select.
    pub role: ParticipantRole,
    /// The participant's age at the event, if recorded.
    pub age: Option<Age>,
    /// The participant-scoped typed attributes to preserve (the form appends at most one more).
    pub attributes: Vec<Attribute>,
    /// The `human_id`s of notes to preserve (the form appends at most one more via the picker).
    pub notes: Vec<String>,
    /// The `AssertionId` (a UUID string) the Save supersedes, or `None` to append a new participation.
    pub supersedes: Option<String>,
}

impl ParticipationSeed {
    /// An empty add-mode seed: the default role, no age/attributes/notes, and no supersede target.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            role: ParticipantRole::Primary,
            age: None,
            attributes: Vec::new(),
            notes: Vec::new(),
            supersedes: None,
        }
    }
}

/// Builds the participant's [`Age`] from the form's string inputs, preserving the seed's `bound` (the
/// form has no bound editor). Returns `None` when every part is absent so no age is asserted (ADR 0019).
fn build_participation_age(
    bound: Option<AgeBound>,
    years: &str,
    months: &str,
    days: &str,
    phrase: &str,
) -> Option<Age> {
    let parse = |value: &str| value.trim().parse::<u16>().ok();
    let phrase = {
        let trimmed = phrase.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    };
    let age = Age {
        bound,
        years: parse(years),
        months: parse(months),
        days: parse(days),
        phrase,
    };
    (!age.is_empty()).then_some(age)
}

/// The shared "Add/Edit participation" form body: role · age (4 inputs) · attributes (type/value) ·
/// notes (an existing-note picker) · provenance block → the built [`NewParticipation`] plus its
/// [`ProvenanceDraft`] (ADR 0019). The `seed` pre-fills every field and seeds the draft's `supersedes`
/// so a Save on an existing row replaces (supersedes) rather than appends (ADR 0004 §2); existing
/// attributes/notes are preserved and the type/value pair and picker append one more of each. It owns
/// neither the person-picker nor the event/person context — the caller renders the participant link
/// above it and wraps the emitted payload into its own edit command (person aggregate write, either
/// way — the *participation person-canonical* rule). Uses hooks, so it is only rendered inside a
/// component scope (the [`ParticipationForm`] wrapper, or an SSR test that supplies the picker).
pub fn participation_form(
    loc: &Localizer,
    seed: &ParticipationSeed,
    note_picker: &RecordPicker,
    onsubmit: EventHandler<(NewParticipation, ProvenanceDraft)>,
) -> Element {
    let choices = loc.participant_role_choices();
    let seed_index = choices.iter().position(|(role, _)| *role == seed.role).unwrap_or(0);
    let mut role_index = use_signal(|| seed_index);
    let options: Vec<SelectChoice> = choices
        .iter()
        .enumerate()
        .map(|(index, (_, label))| SelectChoice {
            value: index.to_string(),
            label: label.clone(),
        })
        .collect();
    let seed_age = seed.age.clone();
    let mut years = use_signal(|| {
        seed_age
            .as_ref()
            .and_then(|age| age.years)
            .map(|n| n.to_string())
            .unwrap_or_default()
    });
    let mut months = use_signal(|| {
        seed_age
            .as_ref()
            .and_then(|age| age.months)
            .map(|n| n.to_string())
            .unwrap_or_default()
    });
    let mut days = use_signal(|| {
        seed_age
            .as_ref()
            .and_then(|age| age.days)
            .map(|n| n.to_string())
            .unwrap_or_default()
    });
    let mut phrase = use_signal(|| seed_age.as_ref().and_then(|age| age.phrase.clone()).unwrap_or_default());
    let mut attr_type = use_signal(String::new);
    let mut attr_value = use_signal(String::new);
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.supersedes.clone(),
        ..ProvenanceDraft::default()
    });
    let save_label = loc.action_button(ActionLabel::Save);
    let seed_bound = seed.age.as_ref().and_then(|age| age.bound);
    let seed_attributes = seed.attributes.clone();
    let seed_notes = seed.notes.clone();
    let picker_for_save = note_picker.clone();
    rsx! {
        Select {
            label: loc.field_label("role"),
            name: "role".to_owned(),
            value: Some(seed_index.to_string()),
            options,
            onchange: move |event: FormEvent| role_index.set(event.value().parse().unwrap_or(0)),
        }
        Input { label: loc.field_label("age-years"), name: "age-years".to_owned(), value: years(), oninput: move |event: FormEvent| years.set(event.value()) }
        Input { label: loc.field_label("age-months"), name: "age-months".to_owned(), value: months(), oninput: move |event: FormEvent| months.set(event.value()) }
        Input { label: loc.field_label("age-days"), name: "age-days".to_owned(), value: days(), oninput: move |event: FormEvent| days.set(event.value()) }
        Input { label: loc.field_label("age-phrase"), name: "age-phrase".to_owned(), value: phrase(), oninput: move |event: FormEvent| phrase.set(event.value()) }
        Input { label: loc.field_label("attribute-type"), name: "attribute-type".to_owned(), value: attr_type(), oninput: move |event: FormEvent| attr_type.set(event.value()) }
        Input { label: loc.field_label("value"), name: "value".to_owned(), value: attr_value(), oninput: move |event: FormEvent| attr_value.set(event.value()) }
        {record_picker(loc, note_picker)}
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let role = choices.get(role_index()).map_or(ParticipantRole::Primary, |(role, _)| role.clone());
                let age = build_participation_age(seed_bound, &years(), &months(), &days(), &phrase());
                let mut attributes = seed_attributes.clone();
                let new_type = attr_type();
                if !new_type.trim().is_empty() {
                    attributes.push(Attribute { attribute_type: new_type, value: attr_value() });
                }
                let mut notes = seed_notes.clone();
                if let Some(id) = picker_selection_id(&picker_for_save) {
                    notes.push(id);
                }
                onsubmit.call((NewParticipation { role, age, attributes, notes }, prov()));
            },
        }
    }
}

/// The shared participation-form component: resolves the localizer + services from [`AppCtx`], builds
/// the existing-note picker, and renders the [`participation_form`] body (giving its hooks an isolated
/// scope). The emitted `(NewParticipation, ProvenanceDraft)` is forwarded to `onsubmit`; the screen's
/// panel wraps it into its own participation edit command.
#[component]
pub fn ParticipationForm(
    seed: ParticipationSeed,
    onsubmit: EventHandler<(NewParticipation, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let note_picker = use_existing_picker(
        services,
        Category::Notes,
        loc.field_label("note"),
        "note".to_owned(),
        loc.picker_entity(Category::Notes),
        Vec::new(),
    );
    participation_form(loc, &seed, &note_picker, onsubmit)
}

/// The History tab: the per-record audit timeline (who/when/why), each undoable entry carrying an
/// undo control. `on_undo` dispatches the pane's `XEdit::UndoAssertion` for an assertion id; pass
/// `None` for an aggregate with no retraction (Tag), which renders the timeline read-only.
pub fn history_panel(loc: &Localizer, entries: &[HistoryEntryVm], on_undo: Option<Callback<String>>) -> Element {
    if entries.is_empty() {
        return rsx! { EmptyState { symbol: "🕓".to_owned(), message: loc.history_empty() } };
    }
    let undo_text = loc.history_undo_short();
    let entries: Vec<HistoryEntry> = entries
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
    rsx! {
        div { class: "section-note", "{loc.history_note()}" }
        HistoryTimeline {
            entries,
            onundo: move |assertion_id: String| {
                if let Some(on_undo) = on_undo {
                    on_undo.call(assertion_id);
                }
            },
        }
    }
}
