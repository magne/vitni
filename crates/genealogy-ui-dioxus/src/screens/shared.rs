use genealogy_app::Rect;
use genealogy_ui::{
    EVIDENCE_KINDS, EvidenceAxis, INFORMATION_KINDS, MediaRefVm, PickerState, ProvenanceDraft, SOURCE_QUALITIES,
    rect_css, tab_label,
};

use super::prelude::*;
use crate::components::{MediaViewer, MediaViewerLabels, PickerOptions, ProvenanceAxis, ProvenanceBlock};
use crate::i18n::Chrome;
use crate::services::{Services, resolve_record_name};
use crate::shell::{CachedName, NameCache, NameState};

/// The workspace-registration field set's state: whether the (Preferences-only) disclosure is open,
/// and the (trimmed on submit) name and optional directory/database URL. Grouped into one struct so
/// [`super::preferences::preferences_view`]'s signature stays readable (mirrors `LocaleFields`).
/// Shared by the Preferences "Register workspace…" card and the bulk-import wizard's new-workspace
/// target (issue #191) — one form, one `prefs-register-*` key set.
#[derive(Debug, Clone, Copy)]
pub struct RegisterFields {
    /// Whether the Preferences disclosure form is open. Unused by the bulk-import wizard, which shows
    /// the fields unconditionally once its own target radio picks "new workspace".
    pub open: Signal<bool>,
    /// The workspace name (required; trimmed on submit).
    pub name: Signal<String>,
    /// The optional workspace directory (empty ⇒ the default data directory).
    pub directory: Signal<String>,
    /// The optional Postgres connection URL (empty ⇒ the default SQLite engine). Kept on the struct
    /// unconditionally so the submit-handler plumbing needs no `cfg`; only its field in
    /// [`register_fields_form`] is gated behind the `postgres` feature, since a default build never
    /// lets a GUI user reach it (`genealogy-app`'s postgres backend isn't compiled in either).
    pub database_url: Signal<String>,
}

/// The reusable workspace-registration field set: Name, Directory (with a default-data-dir hint), and
/// an opt-in Database URL behind the `postgres` feature. No submit action of its own — the caller
/// (Preferences' disclosure, or the bulk-import wizard's Run button) supplies that.
pub fn register_fields_form(chrome: &Chrome, register: RegisterFields) -> Element {
    let RegisterFields {
        mut name,
        mut directory,
        database_url,
        ..
    } = register;
    rsx! {
        Input {
            label: chrome.prefs_register_name_label(),
            name: "register-name".to_owned(),
            value: Some(name()),
            oninput: move |event: FormEvent| name.set(event.value()),
        }
        Input {
            label: chrome.prefs_register_path_label(),
            name: "register-directory".to_owned(),
            value: Some(directory()),
            oninput: move |event: FormEvent| directory.set(event.value()),
        }
        div { class: "muted", style: "font-size:var(--fs-sm)", "{chrome.prefs_register_path_hint()}" }
        {database_url_field(chrome, database_url)}
    }
}

/// The optional "Database URL" field: freezes a Postgres connection string into the manifest at
/// registration (mirrors `genealogy init --database-url`); empty keeps the default SQLite engine.
/// Gated behind the `postgres` feature — off by default, so the field never appears unless the
/// binary was built to support it.
#[cfg(feature = "postgres")]
fn database_url_field(chrome: &Chrome, mut database_url: Signal<String>) -> Element {
    rsx! {
        Input {
            label: chrome.prefs_register_database_url_label(),
            name: "register-database-url".to_owned(),
            value: Some(database_url()),
            oninput: move |event: FormEvent| database_url.set(event.value()),
        }
        div { class: "muted", style: "font-size:var(--fs-sm)", "{chrome.prefs_register_database_url_hint()}" }
    }
}

/// The `postgres`-off counterpart of [`database_url_field`]: renders nothing, so [`register_fields_form`]
/// stays unconditional while the field itself disappears from a default build.
#[cfg(not(feature = "postgres"))]
fn database_url_field(_chrome: &Chrome, _database_url: Signal<String>) -> Element {
    rsx! {}
}

/// Wires the `[`/`]` prev/next-record navigation into a master-detail screen: observes
/// [`NavState::pending_step`] and, when the keyboard dispatcher arms a step, opens the neighbouring
/// record in the list's current (filtered + sorted) order via [`genealogy_ui::step_row`].
///
/// Only the mounted screen runs this effect, so the step always targets the screen the operator is
/// looking at. Call it beside the screen's `pending_create` effect, passing the screen's `category`,
/// its loaded `list` resource, and its `query`/`selected` signals. A no-op until the list has loaded.
pub fn use_record_step(
    mut nav: NavState,
    category: Category,
    list: Resource<ScreenData>,
    query: Signal<genealogy_ui::ListQuery>,
    selected: Signal<Option<String>>,
) {
    use_effect(move || {
        let Some(delta) = *nav.pending_step.read() else {
            return;
        };
        nav.pending_step.set(None);
        let rows = match &*list.read_unchecked() {
            Some(ScreenData::Loaded(IntentOutcome::List(rows))) => rows.clone(),
            _ => return,
        };
        let current = selected.peek().clone();
        let target = genealogy_ui::step_row(&rows, &query.peek(), current.as_deref(), isize::from(delta));
        if let Some(row) = target {
            nav.open_record(RecordRef {
                category,
                human_id: row.id,
                label: row.title,
            });
        }
    });
}

/// Wires the `⌘Z` record-scoped undo into a detail pane: observes [`NavState::pending_undo`] and,
/// when the keyboard dispatcher arms an undo, retracts the newest undoable assertion of the pane's
/// already-loaded change log (`docs/mockups` locked decision — undo is active-record-scoped, not
/// workspace-global; there is no redo because the log is append-only).
///
/// `busy` guards while an edit form / side panel is open (native text undo or the open form takes
/// precedence — the record undo is skipped). `history` is the pane's loaded change log; the newest
/// [`genealogy_ui::first_undoable`] entry is retracted via `on_undo` (which dispatches the pane's own
/// `XEdit::UndoAssertion`). When nothing is undoable, `nothing_to_undo` is shown as a shell notice.
pub fn use_record_undo(
    mut nav: NavState,
    busy: Memo<bool>,
    history: Memo<Vec<genealogy_ui::HistoryEntryVm>>,
    nothing_to_undo: String,
    on_undo: Callback<String>,
) {
    let mut seen = use_signal(|| *nav.pending_undo.peek());
    use_effect(move || {
        let ticket = *nav.pending_undo.read();
        if ticket == *seen.peek() {
            return;
        }
        seen.set(ticket);
        if *busy.peek() {
            return;
        }
        let entries = history.peek().clone();
        match genealogy_ui::first_undoable(&entries) {
            Some(entry) => on_undo.call(entry.assertion_id.clone()),
            None => nav.notify(nothing_to_undo.clone()),
        }
    });
}

/// A clickable link to another record's detail screen: opens it as a tab and navigates to its
/// category (resolving `NavState` from context, so any screen can drop it in). Shared by the
/// dashboard feed/jump-back and every detail tab that references a record.
///
/// The link shows the record's **current** name, resolved live through the shared [`NameCache`] and
/// falling back to the human id when the record has no name (the [`tab_label`] rule). The supplied
/// `label` is only the placeholder shown until resolution lands (and the sole text under bare SSR,
/// where no cache/[`AppCtx`] is present). `icon` prefixes the entity emoji (off for table cells);
/// `button` renders the button-chip style (the jump-back pills) instead of the inline link style.
#[component]
pub fn RecordLink(
    category: Category,
    human_id: String,
    label: String,
    #[props(default)] icon: bool,
    #[props(default)] button: bool,
) -> Element {
    let mut nav = use_context::<NavState>();
    let version = *nav.data_version.read();
    let cache = try_consume_context::<NameCache>();
    let services = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().clone()),
        _ => None,
    };
    let key = (category.id().to_owned(), human_id.clone());

    // The resolved name (id fallback via `tab_label`) when the cache holds an entry for this data
    // version; otherwise the supplied label, or the id when even that is blank.
    let resolved = cache
        .and_then(|cache| cache.0.read().get(&key).cloned())
        .filter(|entry| entry.version == version)
        .map(|entry| entry.state);
    let display = match resolved {
        Some(NameState::Ready(name)) => tab_label(name.as_deref(), &human_id),
        _ if !label.is_empty() => label.clone(),
        _ => human_id.clone(),
    };

    // Resolve on a miss and re-resolve after a data change, off the render via an effect. A no-op
    // when there is no cache or no ready workspace (bare SSR): the link then keeps its placeholder.
    let key_for_effect = key.clone();
    let human_for_effect = human_id.clone();
    use_effect(move || {
        let version = *nav.data_version.read();
        let (Some(mut cache), Some(services)) = (cache, services.clone()) else {
            return;
        };
        if cache
            .0
            .peek()
            .get(&key_for_effect)
            .is_some_and(|entry| entry.version == version)
        {
            return;
        }
        cache.0.write().insert(
            key_for_effect.clone(),
            CachedName {
                version,
                state: NameState::Loading,
            },
        );
        let services = services.clone();
        let key = key_for_effect.clone();
        let human_id = human_for_effect.clone();
        spawn(async move {
            let name = resolve_record_name(services, category, human_id).await;
            cache.0.write().insert(
                key,
                CachedName {
                    version,
                    state: NameState::Ready(name),
                },
            );
        });
    });

    let record = RecordRef {
        category,
        human_id,
        label: display.clone(),
    };
    let class = if button { "btn" } else { "src-link" };
    rsx! {
        button {
            class,
            r#type: "button",
            // Reveal (not switch): opening a linked record keeps the current Explorer list unless the
            // editor is hidden (a tool/Dashboard/Help), in which case it reveals the record's category.
            onclick: move |_| nav.reveal_record(record.clone()),
            if icon {
                span { aria_hidden: "true", "{category.icon()} " }
            }
            "{display}"
        }
    }
}

/// A "Jump back in" pill for a persisted recent item: a record link (the shared [`RecordLink`]). An
/// unknown kind (e.g. after a vocabulary change) renders nothing.
#[component]
pub fn JumpButton(item: RecentItem) -> Element {
    let RecentItem::Record { kind, human_id, label } = item;
    match Category::from_aggregate_kind(&kind) {
        Some(category) => rsx! {
            RecordLink { category, human_id, label, icon: true, button: true }
        },
        None => rsx! {},
    }
}

/// A minimal list of related-item ids, or an empty-state when there are none. When `detach` is
/// `Some`, each row carries a ghost Detach button that fires `(assertion_id, human_id, true)` — the
/// attach `AssertionId` a Detach retracts (ADR 0004 §2), the row label, and the detach flag.
pub fn id_list(loc: &Localizer, items: &[AttachedRefVm], detach: Option<Callback<(String, String, bool)>>) -> Element {
    if items.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        ul { class: "id-list",
            for item in items.iter() {
                li {
                    "{item.human_id}"
                    if let Some(cb) = detach {
                        Button {
                            label: loc.action_label("detach"),
                            variant: ButtonVariant::Ghost,
                            small: true,
                            title: loc.action_title("detach-note"),
                            aria_label: loc.action_detach_row(&item.human_id),
                            onclick: {
                                let assertion_id = item.assertion_id.clone();
                                let human_id = item.human_id.clone();
                                move |_| cb.call((assertion_id.clone(), human_id.clone(), true))
                            },
                        }
                    }
                }
            }
        }
    }
}

/// The wiring a screen's Media tab needs beyond the gallery: the shared viewer's open state, the
/// callback that opens it for a card, and the callback that supersedes a media reference's crop when
/// the viewer commits (Set/Clear region → the owner's `SetMediaRegion` intent, carrying
/// `(attach assertion id, new crop, caption to preserve)`).
#[derive(Clone, Copy)]
pub struct MediaTabState {
    /// The media reference currently open in the viewer, if any (owned by the screen).
    pub viewing: Signal<Option<MediaRefVm>>,
    /// Opens the viewer for a gallery card.
    pub on_view: Callback<MediaRefVm>,
    /// Supersedes a media reference's crop: `(attach assertion id, new crop, caption)`.
    pub on_region: Callback<(String, Option<Rect>, Option<String>)>,
}

/// The Media tab: the thumbnail gallery plus the crop-viewer overlay it opens (ADR 0017 §GUI). The
/// gallery shows a real thumbnail (or a glyph for a non-image), the caption, and a dashed outline for
/// any existing crop; clicking a card opens the viewer, whose Set/Clear region supersedes the crop.
pub fn media_tab(
    loc: &Localizer,
    media: &[MediaRefVm],
    detach: Option<Callback<(String, String, bool)>>,
    state: MediaTabState,
) -> Element {
    rsx! {
        {media_gallery(loc, media, detach, Some(state.on_view))}
        {media_viewer_overlay(loc, state)}
    }
}

/// The Media tab's thumbnail gallery: one card per attached media object with its thumbnail, caption,
/// and crop outline. `detach`, when `Some`, adds a ghost Detach button firing `(assertion_id,
/// human_id, true)`; `onview`, when `Some`, makes the thumbnail a button that opens the viewer.
pub fn media_gallery(
    loc: &Localizer,
    media: &[MediaRefVm],
    detach: Option<Callback<(String, String, bool)>>,
    onview: Option<Callback<MediaRefVm>>,
) -> Element {
    if media.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "gallery",
            for item in media.iter() {
                {media_card(loc, item, detach, onview)}
            }
        }
    }
}

/// One gallery card: the framed thumbnail (an `<img>` for an image, a glyph otherwise) with any crop
/// outline, the caption, and an optional Detach button.
fn media_card(
    loc: &Localizer,
    item: &MediaRefVm,
    detach: Option<Callback<(String, String, bool)>>,
    onview: Option<Callback<MediaRefVm>>,
) -> Element {
    let caption = item.caption_or_id();
    let frame_kind = if item.is_image() { "img-photo" } else { "img-scan" };
    let thumb = rsx! {
        div { class: "crop-frame img-frame {frame_kind}", style: "height:150px",
            if let (true, Some(src)) = (item.is_image(), item.src()) {
                img { class: "media-thumb", src: "{src}", alt: "{caption}", loading: "lazy" }
            } else {
                div { class: "img-glyph", aria_hidden: "true", "🗎" }
            }
            if let Some(crop) = &item.crop {
                div { class: "crop-outline", style: rect_css(crop) }
            }
        }
    };
    let framed = match onview {
        Some(view) => {
            let item = item.clone();
            rsx! {
                button {
                    class: "media-open",
                    r#type: "button",
                    aria_label: loc.media_viewer_open(&caption),
                    onclick: move |_| view.call(item.clone()),
                    {thumb}
                }
            }
        }
        None => thumb,
    };
    rsx! {
        div { class: "card media-card",
            {framed}
            div { class: "muted media-caption", "{caption}" }
            if let Some(cb) = detach {
                Button {
                    label: loc.action_label("detach"),
                    variant: ButtonVariant::Ghost,
                    small: true,
                    title: loc.action_title("detach-media"),
                    aria_label: loc.action_detach_row(&item.human_id),
                    onclick: {
                        let assertion_id = item.assertion_id.clone();
                        let human_id = item.human_id.clone();
                        move |_| cb.call((assertion_id.clone(), human_id.clone(), true))
                    },
                }
            }
        }
    }
}

/// The localized labels the media viewer renders, built from the data localizer.
pub fn media_viewer_labels(loc: &Localizer) -> MediaViewerLabels {
    MediaViewerLabels {
        zoom_group: loc.media_viewer_zoom_group(),
        fit: loc.media_viewer_fit(),
        zoom_100: loc.media_viewer_zoom(100),
        zoom_150: loc.media_viewer_zoom(150),
        zoom_200: loc.media_viewer_zoom(200),
        region: loc.media_viewer_region(),
        no_region: loc.media_viewer_no_region(),
        set_region: loc.media_viewer_set_region(),
        clear_region: loc.media_viewer_clear_region(),
        close: loc.action_label("close"),
    }
}

/// Renders the media viewer over the tab when a card has been opened; committing Set/Clear region
/// supersedes the reference's crop and closes the viewer.
fn media_viewer_overlay(loc: &Localizer, state: MediaTabState) -> Element {
    let mut viewing = state.viewing;
    let Some(item) = viewing() else {
        return rsx! {};
    };
    let on_region = state.on_region;
    let set_item = item.clone();
    let clear_item = item.clone();
    rsx! {
        MediaViewer {
            item: item.clone(),
            labels: media_viewer_labels(loc),
            onset: move |crop: Rect| {
                on_region.call((set_item.assertion_id.clone(), Some(crop), set_item.caption.clone()));
                viewing.set(None);
            },
            onclear: move |()| {
                on_region.call((clear_item.assertion_id.clone(), None, clear_item.caption.clone()));
                viewing.set(None);
            },
            onclose: move |()| viewing.set(None),
        }
    }
}

/// A read-only Tags list: each applied tag as a name + colour-dot chip (never its UUID —
/// data-model §9). The dispatching [`tags_panel`](super::tabs::tags_panel) supersedes it on the
/// record screens; kept for the tag-chip rendering the design-system tests exercise.
pub fn tag_chips(loc: &Localizer, tags: &[TagRef]) -> Element {
    if tags.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "wrap",
            for tag in tags.iter() {
                Chip { label: tag.name.clone(), dot_color: tag.color.clone() }
            }
        }
    }
}

/// Returns `None` for a blank field (so an absent field is not asserted), else the value as typed.
#[must_use]
pub fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() { None } else { Some(value) }
}

/// Builds an optional-enum select's options (a leading unset "—" then one per value) and the value
/// string for the currently-selected item, for a create form's `Select`. Keeps the per-select
/// boilerplate out of the field-row fns (which are line-capped).
pub fn optional_enum_select<T: PartialEq>(
    unset: String,
    items: &[T],
    selected: Option<&T>,
    label: impl Fn(&T) -> String,
) -> (Vec<SelectChoice>, String) {
    let mut options = vec![SelectChoice {
        value: String::new(),
        label: unset,
    }];
    let mut selected_value = String::new();
    for (index, item) in items.iter().enumerate() {
        options.push(SelectChoice {
            value: index.to_string(),
            label: label(item),
        });
        if selected == Some(item) {
            selected_value = index.to_string();
        }
    }
    (options, selected_value)
}

/// Builds an optional-enum [`DraftSelect`]'s parts: the options (a leading unset "—" then one per
/// value, index-valued), the current draft value, and the committed `original` value — both as the
/// index string, or `""` when unset. The record-editor counterpart of [`optional_enum_select`] (which
/// serves a create `Select` and does not track an `original`).
pub fn record_enum_select<T: PartialEq>(
    unset: String,
    items: &[T],
    current: Option<&T>,
    original: Option<&T>,
    label: impl Fn(&T) -> String,
) -> (Vec<SelectChoice>, String, String) {
    let mut options = vec![SelectChoice {
        value: String::new(),
        label: unset,
    }];
    let mut current_value = String::new();
    let mut original_value = String::new();
    for (index, item) in items.iter().enumerate() {
        options.push(SelectChoice {
            value: index.to_string(),
            label: label(item),
        });
        if current == Some(item) {
            current_value = index.to_string();
        }
        if original == Some(item) {
            original_value = index.to_string();
        }
    }
    (options, current_value, original_value)
}

/// The create-form record header (`record-editing.html` §6): the "New <entity>" title and a
/// "draft · not saved" badge, shown above a create form's fields in the detail pane. Both strings are
/// already localized by the caller. `actions` fills the sticky header's right-aligned slot (Cancel /
/// Save); pass an empty `rsx! {}` for a form that still carries its actions below the fields.
pub fn create_record_header(title: &str, draft_badge: &str, actions: Element) -> Element {
    rsx! {
        div { class: "detail-head",
            div { class: "detail-id",
                h1 { class: "detail-title", "{title}" }
                div { class: "wrap", style: "margin-top:8px",
                    span { class: "badge", "{draft_badge}" }
                }
            }
            div { class: "head-actions", {actions} }
        }
    }
}

/// The create pane's whole frame (`record-editing.html` §6): the draft header plus the body inside
/// the same `.tab-body` container edit mode renders into, so create and edit share one geometry —
/// the fields card and the provenance block sit at the same inset in both modes.
pub fn create_record_frame(title: &str, draft_badge: &str, actions: Element, body: Element) -> Element {
    rsx! {
        {create_record_header(title, draft_badge, actions)}
        div { class: "tab-body", {body} }
    }
}

/// The evidence-first source cue: a source-count link, or a no-source flag when unsourced.
pub fn source_cue(loc: &Localizer, source_count: usize) -> Element {
    if source_count > 0 {
        rsx! { SourceLink { label: loc.source_count(source_count), onclick: move |_| {} } }
    } else {
        rsx! { NoSourceFlag { label: loc.no_source() } }
    }
}

/// A shared citations table (Event/Place Citations tab): source · page · surety · evidence axes.
pub fn citation_table(loc: &Localizer, citations: &[CitationRefVm]) -> Element {
    if citations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            caption: loc.tab_label("citations"),
            headers: vec![
                loc.field_label("source"),
                loc.field_label("page"),
                loc.field_label("confidence"),
                loc.field_label("evidence"),
            ],
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
                }
            }
        }
    }
}

/// The evidence-first source cue with a "Why we believe" popover: a source-count link that, on
/// activation, floats the claim's citations beside it; or a no-source flag when the claim is
/// unsourced. The Overview-pane counterpart of [`source_cue`] (which stays plain for the tabs).
pub fn provenance_cue(loc: &Localizer, title: String, citations: &[CitationRefVm]) -> Element {
    if citations.is_empty() {
        rsx! { NoSourceFlag { label: loc.no_source() } }
    } else {
        rsx! {
            ProvenanceTrigger {
                label: loc.source_count(citations.len()),
                title,
                dismiss_label: loc.action_label("dismiss"),
                citations: citations.to_vec(),
            }
        }
    }
}

/// A source-count link that toggles an anchored "Why we believe" popover listing the claim's
/// citations. Self-contained: owns its open state, dismissed by Esc or by clicking the backdrop.
#[component]
pub fn ProvenanceTrigger(
    /// The already-localized source-count text (e.g. "2 sources").
    label: String,
    /// The already-localized popover heading (e.g. "Why we believe: Birth").
    title: String,
    /// The already-localized accessible name for the dismiss backdrop.
    dismiss_label: String,
    /// The claim's citations, rendered as provenance rows.
    citations: Vec<CitationRefVm>,
) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        span { class: "prov-anchor",
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    open.set(false);
                }
            },
            button {
                class: "src-link",
                r#type: "button",
                style: "border:0;background:none;font:inherit;cursor:pointer",
                aria_haspopup: "dialog",
                aria_expanded: if open() { "true" } else { "false" },
                onclick: move |_| {
                    let now = open();
                    open.set(!now);
                },
                "❝ {label}"
            }
            if open() {
                button {
                    class: "prov-backdrop",
                    r#type: "button",
                    aria_label: dismiss_label,
                    onclick: move |_| open.set(false),
                }
                ProvenancePopover { title,
                    for citation in citations.iter() {
                        {provenance_claim_row(citation)}
                    }
                }
            }
        }
    }
}

/// One provenance claim row inside the popover: surety badge, the backing source (link + page), the
/// Evidence Explained axes, and the "asserted by" line. Reuses the shared evidence components.
pub fn provenance_claim_row(citation: &CitationRefVm) -> Element {
    let label = citation
        .source
        .clone()
        .unwrap_or_else(|| citation.source_id.clone().unwrap_or_else(|| citation.human_id.clone()));
    rsx! {
        div { class: "prov-claim",
            if let (Some(level), Some(confidence_label)) = (citation.confidence, citation.confidence_label.clone()) {
                ConfidenceBadge { level, label: confidence_label }
            }
            div {
                div {
                    if let Some(source_id) = &citation.source_id {
                        RecordLink { category: Category::Sources, human_id: source_id.clone(), label }
                    } else {
                        "{label}"
                    }
                    if let Some(page) = &citation.page {
                        span { class: "muted", " — {page}" }
                    }
                }
                if !citation.evidence_axes.is_empty() {
                    div { class: "wrap", style: "margin-top:4px",
                        for chip in citation.evidence_axes.iter() {
                            EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() }
                        }
                    }
                }
                if let Some(asserted_by) = &citation.asserted_by {
                    div { class: "tl-who", "{asserted_by}" }
                }
            }
        }
    }
}

/// The provenance block (`record-editing.html` §5b) for an edit form, bound to `draft`: resolves
/// every label via `loc` and hands the controlled block its options. Rendered directly above a form's
/// Save button; the form reads `draft()` when it dispatches the save. The confidence select and the
/// three evidence-axis selects are index-valued (into [`ConfidenceLevel::all`] and the axis consts),
/// with a leading unset "—" on each axis.
pub fn provenance_block(loc: &Localizer, draft: Signal<ProvenanceDraft>) -> Element {
    provenance_block_impl(loc, draft, false)
}

/// The provenance block for a Person/Family relationship assertion, which additionally offers the
/// "cite a DNA match" evidence picker (data-model §12, ADR 0023). Otherwise identical to
/// [`provenance_block`]; a DNA match cannot back the other aggregates' claims, so they use the plain
/// block.
pub fn provenance_block_dna(loc: &Localizer, draft: Signal<ProvenanceDraft>) -> Element {
    provenance_block_impl(loc, draft, true)
}

/// Shared body of the provenance-block wrappers: builds the confidence + evidence-axis options and
/// renders [`ProvenanceBlock`], toggling the DNA-match evidence picker via `allow_dna_evidence`.
fn provenance_block_impl(loc: &Localizer, draft: Signal<ProvenanceDraft>, allow_dna_evidence: bool) -> Element {
    let mut confidence_options: Vec<SelectChoice> = vec![SelectChoice {
        value: String::new(),
        label: loc.confidence_label_opt(None),
    }];
    for (index, level) in ConfidenceLevel::all().iter().enumerate() {
        confidence_options.push(SelectChoice {
            value: index.to_string(),
            label: loc.confidence_label(*level),
        });
    }
    let unset = loc.evidence_axis_unset();
    let axes = vec![
        ProvenanceAxis {
            axis: EvidenceAxis::Source,
            aria_label: loc.evidence_axis_label(EvidenceAxis::Source),
            options: axis_options(
                &unset,
                SOURCE_QUALITIES.iter().map(|value| loc.evidence_source_label(*value)),
            ),
        },
        ProvenanceAxis {
            axis: EvidenceAxis::Information,
            aria_label: loc.evidence_axis_label(EvidenceAxis::Information),
            options: axis_options(
                &unset,
                INFORMATION_KINDS
                    .iter()
                    .map(|value| loc.evidence_information_label(*value)),
            ),
        },
        ProvenanceAxis {
            axis: EvidenceAxis::Evidence,
            aria_label: loc.evidence_axis_label(EvidenceAxis::Evidence),
            options: axis_options(
                &unset,
                EVIDENCE_KINDS.iter().map(|value| loc.evidence_kind_label(*value)),
            ),
        },
    ];
    rsx! {
        ProvenanceBlock {
            draft,
            heading: loc.provenance_heading(),
            reason_label: loc.provenance_reason_label(),
            reason_hint: loc.provenance_reason_hint(),
            confidence_label: loc.field_label("confidence"),
            evidence_label: loc.field_label("evidence"),
            confidence_options,
            axes,
            allow_dna_evidence,
        }
    }
}

/// Builds an axis select's options: the unset "—" (value "") first, then one option per axis value
/// with its index as the value.
fn axis_options(unset: &str, labels: impl Iterator<Item = String>) -> Vec<SelectChoice> {
    let mut options = vec![SelectChoice {
        value: String::new(),
        label: unset.to_owned(),
    }];
    for (index, label) in labels.enumerate() {
        options.push(SelectChoice {
            value: index.to_string(),
            label,
        });
    }
    options
}

/// A lightweight Retract/Detach side panel (`record-editing.html` §8): the row being acted on, a
/// "stays in History" note, a rationale-only input, and a Danger confirm button. Reused by all 11
/// screens for both Retract (a collection row) and Detach (an attachment) — the only difference is the
/// title/note/button strings the caller passes. A pure fn (the rationale signal + callback are passed
/// in) so the SSR tests render it without `AppCtx`; the caller builds a `ProvenanceDraft{rationale}`
/// from the signal and dispatches `*Edit::UndoAssertion`. Never renders the target's `AssertionId`.
#[expect(
    clippy::too_many_arguments,
    reason = "a self-contained panel takes its localized strings flat"
)]
pub fn retract_panel(
    loc: &Localizer,
    title: &str,
    row_label: &str,
    accessible_name: String,
    note: &str,
    button_label: String,
    rationale: Signal<String>,
    onconfirm: Callback<()>,
) -> Element {
    let mut rationale = rationale;
    rsx! {
        div { class: "stack",
            h3 { style: "font-size:var(--fs-lg);margin:0", "{title}" }
            div { class: "muted", "{row_label}" }
            div { class: "field",
                label { r#for: "retract-reason", "{loc.provenance_reason_label()}" }
                TextInput {
                    id: "retract-reason",
                    name: "retract-reason",
                    value: "{rationale}",
                    oninput: move |event: FormEvent| rationale.set(event.value()),
                }
            }
            div { class: "muted", style: "font-size:var(--fs-sm)", "{note}" }
            Button {
                label: button_label,
                variant: ButtonVariant::Danger,
                aria_label: accessible_name,
                onclick: move |_| onconfirm.call(()),
            }
        }
    }
}

/// A row's armed retract, for the shared [`retract_side_panel`]. Carries the assertion to retract
/// plus the row label + detach flag (the panel wording). The retract always targets the active
/// record's aggregate — the sole owner of every row on its tabs (ADR 0004 §2).
#[derive(Clone, PartialEq)]
pub struct RetractTarget {
    /// The `AssertionId` the confirm retracts.
    pub assertion_id: String,
    /// The row label shown in the panel and the accessible name.
    pub label: String,
    /// Whether this is a Detach of an attachment (drives Detach vs Retract wording).
    pub detach: bool,
}

/// The Retract/Detach side panel (`record-editing.html` §8), shared by every screen with retractable
/// rows: when a row's action is armed (`retract` is `Some`), renders the shared [`retract_panel`]
/// inside a [`SidePanel`], binding the rationale to `reason`; confirming calls `on_confirm` (which
/// dispatches `UndoAssertion`). `detach_note_id` is the `action_title` id for the Detach note — the
/// only per-screen difference (e.g. `"detach-citation"`, `"detach-media"`, `"detach-note"`).
pub fn retract_side_panel(
    loc: &Localizer,
    mut retract: Signal<Option<RetractTarget>>,
    reason: Signal<String>,
    on_confirm: Callback<()>,
    detach_note_id: &str,
) -> Element {
    let Some(RetractTarget { label, detach, .. }) = retract() else {
        return rsx! {};
    };
    let (title_id, button_id, note, accessible) = if detach {
        (
            "detach",
            "detach",
            loc.action_title(detach_note_id),
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
            onclose: move |()| retract.set(None),
            footer: rsx! {},
            {retract_panel(loc, &loc.panel_title(title_id), &label, accessible, &note, loc.action_label(button_id), reason, on_confirm)}
        }
    }
}

/// The retract/remove/unlink/detach spec for a collection row's actions cell (`record-editing.html`
/// §8). All four verbs dispatch the same non-destructive `UndoAssertion`; they differ only in the
/// button label, the mockup tooltip, and whether the panel says "Detach" or "Retract".
pub struct RowRetract {
    /// The `AssertionId` (a UUID string) the action retracts (the sub-record's or attachment's).
    pub assertion_id: String,
    /// The `action_label` id for the button text (`"retract"`, `"remove"`, `"unlink"`, `"detach"`).
    pub button_label: &'static str,
    /// The `action_title` id for the hover tooltip (the mockup sentence).
    pub title: &'static str,
    /// Whether this is a Detach of an attachment (drives the panel's Detach vs Retract wording).
    pub detach: bool,
}

/// A collection row's actions cell (`record-editing.html` §8), generic over a screen's edit-form type
/// `E`: an optional ghost **Edit** (opens the row's form pre-filled via `onedit`; Save supersedes by
/// `AssertionId`), an optional **Cite** (opens a provenance-only form via `onedit`; Save re-asserts
/// the row unchanged with fresh citations), and an optional **Retract/Remove/Unlink/Detach** (opens
/// the shared retract panel via `onretract`, which receives `(assertion_id, label, detach)`). Each
/// button carries the mockup tooltip and a row-scoped accessible name; no assertion UUID is ever
/// rendered. `edit`/`cite` are `(form-to-open, tooltip id)`.
pub fn row_actions_cell<E: Clone + PartialEq + 'static>(
    loc: &Localizer,
    label: &str,
    edit: Option<(E, Option<&str>)>,
    cite: Option<(E, &str)>,
    retract: Option<RowRetract>,
    onedit: Option<Callback<E>>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    let edit_button = edit.zip(onedit).map(|((form, title_id), onedit)| {
        let title = title_id.map(|id| loc.action_title(id));
        let accessible = loc.action_edit_row(label);
        rsx! {
            Button {
                label: loc.action_label("edit"),
                variant: ButtonVariant::Ghost,
                small: true,
                title,
                aria_label: accessible,
                onclick: move |_| onedit.call(form.clone()),
            }
        }
    });
    let cite_button = cite.zip(onedit).map(|((form, title_id), onedit)| {
        let title = loc.action_title(title_id);
        let accessible = loc.action_cite_row(label);
        rsx! {
            Button {
                label: loc.action_label("cite"),
                variant: ButtonVariant::Ghost,
                small: true,
                title,
                aria_label: accessible,
                onclick: move |_| onedit.call(form.clone()),
            }
        }
    });
    let retract_button = retract.map(|spec| {
        let label_owned = label.to_owned();
        let accessible = match spec.button_label {
            "detach" => loc.action_detach_row(label),
            "remove" => loc.action_remove_row(label),
            "unlink" => loc.action_unlink_row(label),
            _ => loc.action_retract_row(label),
        };
        rsx! {
            Button {
                label: loc.action_label(spec.button_label),
                variant: ButtonVariant::Ghost,
                small: true,
                title: loc.action_title(spec.title),
                aria_label: accessible,
                onclick: move |_| onretract.call((spec.assertion_id.clone(), label_owned.clone(), spec.detach)),
            }
        }
    });
    rsx! {
        td { class: "row-actions",
            {edit_button}
            {cite_button}
            {retract_button}
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Record-picker side-panel forms
// ---------------------------------------------------------------------------------------------------

/// Builds an existing-only record picker for a side-panel link field (`edit-patterns.html` §c): loads
/// `category`'s rows once via [`load_picker_rows`], owns the live [`PickerState`], and wires no-op
/// pick/clear callbacks — the picked id is read from the returned picker's `state.selection` at submit.
/// "+ New" is never offered; a side panel commits one immediate command, so inline creation there is
/// the flagged follow-up. A custom hook (loads rows, holds state), so callers get a ready picker.
pub fn use_existing_picker(
    services: Services,
    category: Category,
    label: String,
    name: String,
    entity_label: String,
    exclude: Vec<String>,
) -> RecordPicker {
    let state = use_signal(PickerState::default);
    let rows = use_resource(move || {
        let services = services.clone();
        async move { load_picker_rows(services, category).await }
    });
    let options: PickerOptions = picker_options(rows.read_unchecked().as_ref());
    RecordPicker {
        config: PickerConfig {
            label,
            name,
            entity_label,
            allow_new: false,
        },
        state,
        options,
        exclude,
        callbacks: PickerCallbacks {
            onpick: use_callback(|_: PickerSelection| {}),
            onclear: use_callback(|()| {}),
            onnew: use_callback(|_: String| {}),
        },
    }
}

/// The picked record's `human_id` from a picker's live state, or `None` when nothing is picked yet.
#[must_use]
pub fn picker_selection_id(picker: &RecordPicker) -> Option<String> {
    picker
        .state
        .read()
        .selection
        .as_ref()
        .map(|selection| selection.human_id.clone())
}

/// A side-panel attach/link form body over an existing-only record picker: the picker, optional
/// `extra` fields (a role select, a call-number input, relationship selects), the provenance block, and
/// a Save button disabled until a record is picked. The caller's `onsave` reads the picked id from the
/// picker's state ([`picker_selection_id`]) plus any extra-field signals and dispatches the one `*Edit`
/// command. A pure fn (the picker + prov signals passed in) so the SSR tests render it without `AppCtx`.
pub fn attach_picker_form(
    loc: &Localizer,
    picker: &RecordPicker,
    extra: Element,
    prov: Signal<ProvenanceDraft>,
    onsave: Callback<()>,
) -> Element {
    let disabled = picker.state.read().selection.is_none();
    rsx! {
        {record_picker(loc, picker)}
        {extra}
        {provenance_block(loc, prov)}
        Button {
            label: loc.action_label("save"),
            variant: ButtonVariant::Primary,
            disabled,
            onclick: move |_| onsave.call(()),
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Event slice
// ---------------------------------------------------------------------------------------------------

/// The source media types offered by the link forms (a common subset; the model has more).
#[must_use]
pub fn source_media_type_choices() -> [SourceMediaType; 6] {
    [
        SourceMediaType::Book,
        SourceMediaType::Film,
        SourceMediaType::Electronic,
        SourceMediaType::Fiche,
        SourceMediaType::Manuscript,
        SourceMediaType::Photo,
    ]
}
