//! Shared tab-content panels, one per detail-screen tab that every aggregate renders the same way.
//! Each is a pure `fn(loc, data, …) -> Element` (SSR-testable without `AppCtx`, the `shared.rs`
//! idiom) parameterised only over the per-entity variance — the edit-form enum `E` and the
//! entity-specific dispatch callbacks — so the tab markup lives here once instead of being copied
//! across the twelve screen modules.

use vitni_ui::{CitationRefVm, HistoryEntryVm};

use super::prelude::*;

/// The record an [`AttachedRow`] is about, rendered as the row's leading link cell.
pub struct AttachedLink {
    /// The record's entity category — where the link navigates.
    pub category: Category,
    /// The record's user-facing id (e.g. `N0001`) — the link target.
    pub human_id: String,
    /// The placeholder link text until [`RecordLink`] resolves the record's current name through the
    /// shared name cache (and the sole text under bare SSR, where no cache is present).
    pub label: String,
}

/// One row of the shared attached-records table ([`attached_table`]).
pub struct AttachedRow {
    /// The leading cell: `Ok` renders a [`RecordLink`] to the row's record, `Err` renders the carried
    /// label as plain text — for the one row that names a record the projection cannot resolve (a
    /// citation with no cited source), where a link would have nothing to open.
    pub link: Result<AttachedLink, String>,
    /// The domain columns between the link and the actions cell, already `<td>`s. The caller owns
    /// these *and* the headers naming them, so the two counts cannot drift apart.
    pub cells: Element,
    /// The row-actions `<td>` ([`row_actions_cell`]), or `rsx! {}` for a read-only table — which then
    /// passes no trailing empty header either.
    pub actions: Element,
}

/// The shared attached-records table: every "records attached to (or referencing) this record" tab
/// renders through this one spine (issue #304) — the leading link cell, the caller's middle columns,
/// the caller's actions cell, and the empty state a collection tab shows when it holds nothing.
///
/// The spine owns only what every such table has in common, which is exactly the part that used to be
/// answered four different ways: the notes pane rendered cards with an unclickable heading, the two
/// reverse-lookup tables rendered no link at all, and an attached note could therefore not be opened
/// from the record that referenced it. The chips, mono ids and extra columns stay *caller* cells, so
/// the header count is always the call site's own decision rather than a flag this fn interprets.
///
/// The media **grid** is the deliberate exception and does not come through here: a thumbnail is the
/// point of that tab, not a row.
pub fn attached_table(loc: &Localizer, caption: String, headers: Vec<String>, rows: Vec<AttachedRow>) -> Element {
    if rows.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table { caption, headers,
            for row in rows {
                tr {
                    td {
                        {match row.link {
                            Ok(link) => rsx! {
                                RecordLink { category: link.category, human_id: link.human_id, label: link.label }
                            },
                            Err(label) => rsx! { "{label}" },
                        }}
                    }
                    {row.cells}
                    {row.actions}
                }
            }
        }
    }
}

/// The Notes tab shared by every record that carries record-level notes: note (a link to the note's
/// own record) · type · language · content · a Detach action.
///
/// The note's type, language and body are columns rather than a card heading because the body is what
/// makes a citation's transcribed evidence readable — a transcription is an attached
/// `NoteType::Transcript` note rather than a Citation field (data-model §6), so this pane is where
/// those words surface (issue #316) — and the link is what makes the note *openable* from the record
/// that references it (issue #304). The content cell wraps normally, so a long transcription still
/// reads as prose.
///
/// When `detach` is `Some`, each row carries a Detach that fires `(assertion_id, human_id, true)` —
/// the attach `AssertionId` a Detach retracts (ADR 0004 §2), the row label, and the detach flag.
/// Detach is the only action here: the note's own text is edited on the Note record, not from the
/// owner that attached it.
pub fn notes_table(
    loc: &Localizer,
    notes: &[AttachedRefVm],
    detach: Option<Callback<(String, String, bool)>>,
) -> Element {
    let mut headers = vec![
        loc.field_label("note"),
        loc.field_label("type"),
        loc.field_label("language"),
        loc.field_label("content"),
    ];
    if detach.is_some() {
        headers.push(String::new());
    }
    let mut rows = Vec::with_capacity(notes.len());
    for note in notes {
        let actions = match detach {
            Some(onretract) => row_actions_cell::<()>(
                loc,
                &note.human_id,
                None,
                None,
                Some(RowRetract {
                    assertion_id: note.assertion_id.clone(),
                    button_label: RowVerb::Detach,
                    title: "detach-note",
                    detach: true,
                }),
                None,
                onretract,
            ),
            None => rsx! {},
        };
        rows.push(AttachedRow {
            link: Ok(AttachedLink {
                category: Category::Notes,
                human_id: note.human_id.clone(),
                label: note.human_id.clone(),
            }),
            cells: rsx! {
                td {
                    if let Some(label) = note.type_label.clone() {
                        Chip { label }
                    } else {
                        span { class: "muted", "—" }
                    }
                }
                td { class: "muted", {or_dash(note.language.clone())} }
                td { {or_dash(note.text.clone())} }
            },
            actions,
        });
    }
    attached_table(loc, loc.tab_label("notes"), headers, rows)
}

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
    let mut headers = vec![loc.field_label("source"), loc.field_label("page")];
    if show_backs {
        headers.push(loc.field_label("backs"));
    }
    headers.push(loc.field_label("confidence"));
    headers.push(loc.field_label("analysis"));
    headers.push(String::new());
    let mut rows = Vec::with_capacity(citations.len());
    for citation in citations {
        rows.push(AttachedRow {
            link: match &citation.source_id {
                Some(source_id) => Ok(AttachedLink {
                    category: Category::Sources,
                    human_id: source_id.clone(),
                    label: citation.source.clone().unwrap_or_else(|| source_id.clone()),
                }),
                None => Err(citation.source.clone().unwrap_or_else(|| citation.human_id.clone())),
            },
            cells: rsx! {
                td { class: "muted", {or_dash(citation.page.clone())} }
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
            },
            actions: row_actions_cell::<E>(
                loc,
                &citation.human_id,
                None,
                None,
                citation.assertion_id.clone().map(|id| RowRetract {
                    assertion_id: id,
                    button_label: RowVerb::Detach,
                    title: "detach-citation",
                    detach: true,
                }),
                None,
                onretract,
            ),
        });
    }
    attached_table(loc, loc.tab_label("citations"), headers, rows)
}

/// The Tags tab's chip rendering, shared by every aggregate that carries tags: the applied tags as
/// name + colour-dot chips, each with a delete control that fires `on_remove` with `(tag id, tag
/// name)`. The name rides along because the removal opens a panel that takes an operator rationale
/// (issue #315) and that panel names the tag: tags are referenced by name; their UUID is never
/// rendered (data-model §9), and tags never retract — untag is the only removal. The "add tag" action
/// is the caller's [`tab_frame`] bar, not this fn — the two used to be one fn with the add button baked
/// in, which was the second `.tab-actions` code path this split exists to delete.
///
/// `.tag-chips` on the container reads these chips a size larger than the chips inside a table cell
/// (issue #303): here a chip is the tab's whole content and carries a control, not a label in a row.
pub fn tags_panel(loc: &Localizer, tags: &[TagRef], on_remove: Callback<(String, String)>) -> Element {
    if tags.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    let untag_title = loc.action_title("untag");
    rsx! {
        div { class: "wrap tag-chips",
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

/// The shared record-tab frame (`record-editing.html` §8): the tab's explanation, then the single
/// button that opens or runs its one action, then the tab's own `body` — the three-part shape every
/// one of the 13 detail screens gets from one fn (issue #303).
///
/// The explanation comes from [`Localizer::tab_note`] keyed by [`DetailTab::id`], so the six tabs
/// every aggregate renders the same way are explained once here rather than per screen, and — being
/// emitted *outside* `body` — it survives the `EmptyState` each body early-returns, which is exactly
/// when a new operator has nothing else to infer the tab's meaning from. A tab a screen owns itself
/// resolves no note and renders none.
///
/// This is also the only fn in the crate that emits a *tab's* `.tab-actions` (an address card's own
/// Edit/Retract pair in [`address_cards`] is the one other user of the class), so every tab resolves
/// its label through the [`ActionLabel`] its own [`DetailTab::action`] declares (issue #314 slice 3)
/// instead of a bare `ActionLabel` picked independently at each call site — the drift that once left
/// six labels wrong. `tab.action: None` or `target: TabActionTarget::None` renders the explanation and
/// `body` with no bar between them, for a read-only tab such as History.
pub fn tab_frame<E: Clone + PartialEq + 'static>(
    loc: &Localizer,
    tab: &DetailTab,
    target: TabActionTarget<E>,
    style: Option<TabActionStyle>,
    body: Element,
) -> Element {
    let note = loc.tab_note(tab.id);
    let bar = tab_action_bar(loc, tab, target, style);
    rsx! {
        if let Some(note) = note {
            div { class: "section-note", "{note}" }
        }
        {bar}
        {body}
    }
}

/// The frame's action bar, or nothing for a read-only tab — split out of [`tab_frame`] so the frame
/// reads as its three parts and neither part can early-return past the other.
fn tab_action_bar<E: Clone + PartialEq + 'static>(
    loc: &Localizer,
    tab: &DetailTab,
    target: TabActionTarget<E>,
    style: Option<TabActionStyle>,
) -> Element {
    let Some(action) = tab.action else {
        return rsx! {};
    };
    let style = style.unwrap_or_default();
    let variant = style.emphasis.unwrap_or(ButtonVariant::Primary);
    let label = loc.action_button(action);
    let button = match target {
        TabActionTarget::None => return rsx! {},
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
        div { class: "tab-actions", {button} }
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

/// One detail screen's participation in the six tabs every aggregate renders the same way — the
/// argument [`shared_tab`] dispatches on (issue #322).
///
/// The ctx describes *which* shared tabs the screen has and what to render them over; it never
/// decides *whether* one is shown. A screen still declares its own tab list, and `shared_tab` is
/// reached only from its dispatcher's `_` arm, so a tab the screen matches explicitly always wins.
/// A shared tab whose arm is absent here resolves to `None` and falls through to the screen's
/// overview, which is what makes the ctx and the tab list unable to disagree about ownership.
///
/// The four form tabs are grouped under [`FormTabs`] rather than sitting flat, because they share
/// the `editing` signal their action bars write to and only they need it — Tag, which carries no
/// edit-form enum at all, says `forms: None` in one word instead of manufacturing a signal it has
/// nothing to put in.
pub struct SharedTabCtx<'a, E> {
    /// The Citations/Media/Notes/Tags tabs this screen carries, or `None` for a screen with no side
    /// panel to arm. None of the four is universal: `Citation` exists on 5 of the 12 edit-form
    /// enums, `Media` on 6, `Note` on 10, `Tag` on 12 — so each arm is separately optional.
    pub forms: Option<FormTabs<'a, E>>,
    /// The Research notes tab, for the four aggregates a research note can be *about*.
    pub research_notes: Option<ResearchNotesArm<'a>>,
    /// The audit timeline. Not optional: all 13 screens carry History.
    pub history: &'a [HistoryEntryVm],
    /// Undoes an assertion from the History tab, forwarded to [`history_panel`] as it stands.
    /// `None` for an aggregate with no retraction (Tag).
    pub on_undo: Option<Callback<String>>,
}

/// The shared tabs that arm a side-panel form, and the one signal all of them write to.
///
/// `editing` sits here rather than on [`SharedTabCtx`] so that a screen cannot name a form tab
/// without supplying the signal its button targets: the invariant is the type's, not a runtime
/// `let Some(editing) = … else { return None }` that would silently drop a tab a screen meant to
/// show. Each arm carries its own detach callback rather than the ctx carrying one shared field,
/// because the research-note screen has no plain `on_retract` to give and Tag has none at all.
pub struct FormTabs<'a, E> {
    /// The screen's side-panel form slot, which each tab's action bar writes its own variant into.
    pub editing: Signal<Option<E>>,
    /// The Citations tab — the citations attached to this record.
    pub citations: Option<CitationsArm<'a, E>>,
    /// The Media tab — the media objects attached to this record.
    pub media: Option<MediaArm<'a, E>>,
    /// The Notes tab — the notes attached to this record.
    pub notes: Option<NotesArm<'a, E>>,
    /// The Tags tab — the tags applied to this record.
    pub tags: Option<TagsArm<'a, E>>,
}

/// The Citations tab's contents and wiring.
pub struct CitationsArm<'a, E> {
    /// The edit-form variant the "Attach citation" bar arms.
    pub form: E,
    /// The attached citations, as [`citations_table`] renders them.
    pub rows: &'a [CitationRefVm],
    /// Whether to render the "Backs" column — set for the plural-subject records (Person, Family),
    /// clear for the singular-subject ones (Event, Place, Media).
    pub show_backs: bool,
    /// Detaches a row: `(assertion_id, human_id, true)`.
    pub on_detach: Callback<(String, String, bool)>,
}

/// The Media tab's contents and wiring.
pub struct MediaArm<'a, E> {
    /// The edit-form variant the "Attach media" bar arms.
    pub form: E,
    /// The attached media references, as [`media_tab`] renders them.
    pub rows: &'a [MediaRefVm],
    /// The screen's viewer state and crop-supersede wiring.
    pub state: MediaTabState,
    /// Detaches a card: `(assertion_id, human_id, true)`.
    pub on_detach: Callback<(String, String, bool)>,
}

/// The Notes tab's contents and wiring.
pub struct NotesArm<'a, E> {
    /// The edit-form variant the "Attach note" bar arms.
    pub form: E,
    /// The attached notes, as [`notes_table`] renders them.
    pub rows: &'a [AttachedRefVm],
    /// Detaches a row: `(assertion_id, human_id, true)`.
    pub on_detach: Callback<(String, String, bool)>,
}

/// The Tags tab's contents and wiring.
pub struct TagsArm<'a, E> {
    /// The edit-form variant the "Add tag" bar arms.
    pub form: E,
    /// The applied tags, as [`tags_panel`] renders them.
    pub rows: &'a [TagRef],
    /// Arms the untag panel for a chip's ×: `(tag_id, tag name)`. Not a detach — tags never
    /// retract (data-model §9), so this is the one form tab whose removal has its own shape.
    pub on_remove: Callback<(String, String)>,
}

/// The Research notes tab's contents: what the notes would be *about*, and the notes already about
/// it. There is no `editing` here because the tab's action opens a draft tab rather than a side
/// panel — [`ResearchNotesTab`] builds that from `NavState` itself.
pub struct ResearchNotesArm<'a> {
    /// The subject record's entity category, for the "New research note" the tab's bar opens.
    pub category: Category,
    /// The subject record's user-facing id (e.g. `I0001`).
    pub human_id: &'a str,
    /// The research notes already naming this record as a subject.
    pub rows: &'a [RowVm],
}

/// Renders one of the six tabs every aggregate shares (Citations, Media, Notes, Tags, Research
/// notes, History), or `None` when `tab` is not one of them — or is one the `ctx` does not carry.
///
/// This is the arm the 13 detail screens used to spell out 51 times between them, differing only in
/// the screen's own edit-form variant. Each dispatcher now ends `_ => shared_tab(loc, tab, ctx)
/// .unwrap_or_else(|| …overview…)`, so the shared frame's shape is one edit rather than 13.
///
/// It takes the whole [`DetailTab`] rather than just its id because [`tab_frame`] needs the tab's
/// `action` (which resolves the bar's label) and its `id` (which keys the tab's explanation).
pub fn shared_tab<E: Clone + PartialEq + 'static>(
    loc: &Localizer,
    tab: &DetailTab,
    ctx: &SharedTabCtx<'_, E>,
) -> Option<Element> {
    match tab.id {
        "research-notes" => {
            let arm = ctx.research_notes.as_ref()?;
            Some(rsx! {
                ResearchNotesTab {
                    tab: tab.clone(),
                    category: arm.category,
                    human_id: arm.human_id.to_owned(),
                    rows: arm.rows.to_vec(),
                }
            })
        }
        "history" => Some(tab_frame::<()>(
            loc,
            tab,
            TabActionTarget::None,
            None,
            history_panel(loc, ctx.history, ctx.on_undo),
        )),
        _ => shared_form_tab(loc, tab, ctx.forms.as_ref()?),
    }
}

/// The four shared tabs that arm a side-panel form — split out of [`shared_tab`] so neither fn
/// carries the whole six-way dispatch plus its per-arm `Option` unwrapping.
fn shared_form_tab<E: Clone + PartialEq + 'static>(
    loc: &Localizer,
    tab: &DetailTab,
    forms: &FormTabs<'_, E>,
) -> Option<Element> {
    let editing = forms.editing;
    match tab.id {
        "citations" => {
            let arm = forms.citations.as_ref()?;
            Some(tab_frame(
                loc,
                tab,
                TabActionTarget::Form(editing, arm.form.clone()),
                None,
                rsx! {
                    {citations_table::<E>(loc, arm.rows, arm.show_backs, arm.on_detach)}
                },
            ))
        }
        "media" => {
            let arm = forms.media.as_ref()?;
            Some(tab_frame(
                loc,
                tab,
                TabActionTarget::Form(editing, arm.form.clone()),
                None,
                rsx! {
                    {media_tab(loc, arm.rows, Some(arm.on_detach), arm.state)}
                },
            ))
        }
        "notes" => {
            let arm = forms.notes.as_ref()?;
            Some(tab_frame(
                loc,
                tab,
                TabActionTarget::Form(editing, arm.form.clone()),
                None,
                rsx! {
                    {notes_table(loc, arm.rows, Some(arm.on_detach))}
                },
            ))
        }
        "tags" => {
            let arm = forms.tags.as_ref()?;
            Some(tab_frame(
                loc,
                tab,
                TabActionTarget::Form(editing, arm.form.clone()),
                Some(TabActionStyle {
                    emphasis: Some(ButtonVariant::Ghost),
                    ..Default::default()
                }),
                tags_panel(loc, arm.rows, arm.on_remove),
            ))
        }
        _ => None,
    }
}

impl From<&DetailTab> for TabItem {
    /// The tab strip's item for one [`DetailTab`]: the same id, label and count, with `id` owned
    /// because the design-system component is generic over strips whose ids are not `'static`.
    ///
    /// The conversion lives here rather than in `components/nav.rs` so the design system stays
    /// ignorant of `vitni_ui`'s view-models — the pedigree screen builds its own [`TabItem`]s from
    /// something else entirely, and a `From` in the component module would tie the strip to the
    /// record-detail vocabulary it happens to serve most often. `DetailTab::action` has no counterpart:
    /// it is the tab's *content* affordance ([`tab_frame`]'s bar), never part of the strip.
    fn from(tab: &DetailTab) -> Self {
        Self {
            id: tab.id.to_owned(),
            label: tab.label.clone(),
            count: tab.count,
        }
    }
}

/// The label column of an address card's rows (`docs/mockups/repository.html:163-185`,
/// `event.html:239-246`). The canonical [`DEFAULT_LABEL_WIDTH`]: the 90px both mockups drew is 1px
/// short of `POSTNUMMER`, which took that row's value out of line with the card's others.
const ADDRESS_LABEL_WIDTH: u32 = DEFAULT_LABEL_WIDTH;

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
                                FactRow { label: loc.field_label("street"), label_width: ADDRESS_LABEL_WIDTH,
                                    span { class: "grow", {address.lines.join(", ")} }
                                }
                                FactRow { label: loc.field_label("region"), label_width: ADDRESS_LABEL_WIDTH,
                                    span { class: "grow", {or_dash(address.region.clone())} }
                                }
                                FactRow { label: loc.field_label("postal-code"), label_width: ADDRESS_LABEL_WIDTH,
                                    span { class: "grow mono", {or_dash(address.postal_code.clone())} }
                                }
                                FactRow { label: loc.field_label("country"), label_width: ADDRESS_LABEL_WIDTH,
                                    span { class: "grow", {or_dash(address.country.clone())} }
                                }
                                FactRow { label: loc.field_label("phone"), label_width: ADDRESS_LABEL_WIDTH,
                                    span { class: "grow mono", {or_dash(address.phone.clone())} }
                                }
                                FactRow { label: loc.field_label("email"), label_width: ADDRESS_LABEL_WIDTH,
                                    span { class: "grow", {or_dash(address.email.clone())} }
                                }
                                FactRow { label: loc.field_label("fax"), label_width: ADDRESS_LABEL_WIDTH,
                                    span { class: "grow mono", {or_dash(address.fax.clone())} }
                                }
                                FactRow { label: loc.field_label("www"), label_width: ADDRESS_LABEL_WIDTH,
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
///
/// The tab's explanation is [`tab_frame`]'s, not this fn's — it used to be emitted here, below the
/// empty-state early return, so a record with no changes yet was told nothing at all (issue #303).
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
