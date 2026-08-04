//! Shell-wide navigation and UI state, provided as context.
//!
//! The rail, top bar, record tabstrip, status bar, keyboard dispatcher, and overlays all read and
//! write one [`NavState`] (a `Copy` bundle of signals) rather than threading props through six
//! components. The active [`Destination`] is the framework-neutral navigation key from
//! `genealogy-ui` (ADR 0008); the renderer merely interprets it.

use std::any::Any;
use std::collections::BTreeMap;
use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_app::{RecentItem, ThemeMode, push_recent};
use genealogy_ui::{Category, Destination, NavHistory, NavLocation, ProvenanceDraft, RecordDraft, RecordRef, Tool};

use crate::components::ToastKind;

/// The entity category a destination shows a list + editor for, or `None` when the destination is a
/// full-width screen with no Explorer/editor (a tool, the workspace Dashboard, or Help). This is the
/// single source of truth for the two shell shapes: `Some` ⇒ `rail | Explorer | editor`, `None` ⇒
/// `rail | screen`.
#[must_use]
pub fn entity_category(destination: Destination) -> Option<Category> {
    match destination {
        Destination::Category(Category::Dashboard) | Destination::Tool(_) | Destination::Help { .. } => None,
        Destination::Category(category) => Some(category),
    }
}

/// Which pane of a possibly-docked split is rendering: the single/active pane, or the second pane
/// docked beside it (`.master-detail.split-2`). Provided as context only by
/// [`DockedRecordDetail`](crate::screens::DockedRecordDetail), as its first hook, ahead of the record
/// it renders — an undocked pane never sees a `PaneRole` in context at all, and every callsite treats
/// that absence the same as [`Self::Active`] (an unprefixed id), so single-pane markup stays
/// byte-identical to before docking existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneRole {
    /// The single/active pane. Never provided as context — the absence of a `PaneRole` *is* this, in
    /// practice; the variant exists so a caller can still name it explicitly (e.g. a test standing up
    /// two panes side by side).
    Active,
    /// The second pane docked beside the active one.
    Docked,
}

impl PaneRole {
    /// The prefix this pane's tab/panel ids carry: empty for [`Self::Active`], `"docked-"` for
    /// [`Self::Docked`] — what keeps two mounted panes' `id`/`aria-controls` from colliding (#279).
    #[must_use]
    pub fn id_prefix(self) -> &'static str {
        match self {
            Self::Active => "",
            Self::Docked => "docked-",
        }
    }
}

/// Which overlay, if any, is layered over the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// No overlay.
    None,
    /// The command palette (`⌘K`).
    Palette,
    /// The keyboard-shortcuts help sheet (`?`).
    Help,
}

/// A close/quit operation armed behind the confirm dialog because it would discard unsaved work —
/// an unsaved draft, or an in-progress edit parked in [`NavState::edit_drafts`] — via `⌘W`/`⌘Q` or the
/// tabstrip `✕`. `None` on [`NavState::pending_close`] means no confirm is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseRequest {
    /// Close the record tab at this 0-based index (it holds unsaved work, or the confirm would not
    /// have armed).
    Tab(usize),
    /// Quit the application.
    Quit,
}

/// How a save run ends once every record it queued has saved.
///
/// Distinct from [`CloseRequest`], which is what the *confirm* was armed for: a Save all over a strip
/// where some record cannot be saved runs anyway, saves the ones it can, and then ends in
/// [`Self::StayOpen`] rather than the quit the confirm was raised by — the records it could not save
/// keep their work, on screen, in a running app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveThen {
    /// Close the record tab at this 0-based index.
    CloseTab(usize),
    /// Quit the application.
    Quit,
    /// Do nothing further: the run covered only part of the unsaved work.
    StayOpen,
}

/// The record whose save the shell has asked for, so a close/quit can keep the work instead of
/// discarding it (the confirm's **Save** / **Save all**).
///
/// The shell cannot save generically — save is per-screen and differently shaped per aggregate — so it
/// arms the request and the record's own screen runs its existing save closure
/// (`use_save_on_request`), reporting back through [`NavState::note_save_finished`]. The target tab is
/// activated first, so its pane is mounted at all: a record's pane exists only while its tab is active
/// (or, with a split open, docked), and a save target need not be either yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRequest {
    /// The editor being saved right now.
    pub key: EditKey,
    /// What to do once every record in the run has saved.
    pub then: SaveThen,
}

/// The active colour theme, mirrored onto `[data-theme]` at the shell root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    /// The dark palette (default).
    Dark,
    /// The light palette.
    Light,
}

impl Theme {
    /// The `[data-theme]` attribute value this theme renders with.
    #[must_use]
    pub fn attr(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    /// The window background `(r, g, b, a)` used to seed the native window before the first paint,
    /// matching this theme's `--bg` token (`tokens.css`: dark `#0f1419`, light `#f6f8fa`). Keep in
    /// sync with those tokens so there is no colour flash before the stylesheet applies.
    #[must_use]
    pub fn background_rgba(self) -> (u8, u8, u8, u8) {
        match self {
            Self::Dark => (15, 20, 25, 255),
            Self::Light => (246, 248, 250, 255),
        }
    }
}

/// Resolves a persisted [`ThemeMode`] to a concrete [`Theme`] for `[data-theme]` and the window
/// background. `System` queries the OS appearance on the desktop build; everywhere else (and on any
/// detection failure) it falls back to the historical dark default.
#[must_use]
pub fn resolve_theme(mode: ThemeMode) -> Theme {
    match mode {
        ThemeMode::Light => Theme::Light,
        ThemeMode::Dark => Theme::Dark,
        ThemeMode::System => detect_os_theme(),
    }
}

/// The next mode in the System → Light → Dark → System cycle (the top-bar control).
#[must_use]
pub fn next_theme_mode(mode: ThemeMode) -> ThemeMode {
    match mode {
        ThemeMode::System => ThemeMode::Light,
        ThemeMode::Light => ThemeMode::Dark,
        ThemeMode::Dark => ThemeMode::System,
    }
}

#[cfg(feature = "desktop")]
fn detect_os_theme() -> Theme {
    match dark_light::detect() {
        Ok(dark_light::Mode::Light) => Theme::Light,
        _ => Theme::Dark,
    }
}

#[cfg(not(feature = "desktop"))]
fn detect_os_theme() -> Theme {
    Theme::Dark
}

/// One open tab in the record strip: a saved record or an unsaved draft (a create form).
///
/// "Create is a tab": [`NavState::open_create`] appends a [`Self::Draft`]; committing it
/// ([`NavState::commit_draft`]) replaces it in place with the saved [`Self::Saved`], and cancelling
/// ([`NavState::cancel_draft`]) closes it. A draft has no `human_id` yet, so it never docks and is
/// never recorded in the "Jump back in" list; at most one draft per category is open at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenTab {
    /// A saved record backed by a stored aggregate.
    Saved(RecordRef),
    /// An unsaved draft create form for a category (nothing is stored until it commits).
    Draft(Category),
}

impl OpenTab {
    /// The aggregate category this tab belongs to.
    #[must_use]
    pub fn category(&self) -> Category {
        match self {
            Self::Saved(record) => record.category,
            Self::Draft(category) => *category,
        }
    }

    /// The tab's stable record id, or `None` for a draft (which has no stored aggregate yet).
    #[must_use]
    pub fn human_id(&self) -> Option<&str> {
        match self {
            Self::Saved(record) => Some(&record.human_id),
            Self::Draft(_) => None,
        }
    }

    /// The saved record this tab holds, or `None` when it is still a draft.
    #[must_use]
    pub fn as_saved(&self) -> Option<&RecordRef> {
        match self {
            Self::Saved(record) => Some(record),
            Self::Draft(_) => None,
        }
    }

    /// Whether this tab is an unsaved draft.
    #[must_use]
    pub fn is_draft(&self) -> bool {
        matches!(self, Self::Draft(_))
    }

    /// Whether this tab is the saved record keyed by `(category, human_id)`.
    #[must_use]
    fn is_saved_key(&self, category: Category, human_id: &str) -> bool {
        self.category() == category && self.human_id() == Some(human_id)
    }

    /// The key under which this tab's editor parks its in-progress edit ([`NavState::edit_drafts`]).
    #[must_use]
    pub fn edit_key(&self) -> EditKey {
        match self {
            Self::Saved(record) => EditKey::saved(record.category, &record.human_id),
            Self::Draft(category) => EditKey::draft(*category),
        }
    }
}

/// Which editor a [`StashedEdit`] belongs to: a saved record (`human_id` is `Some`) or a category's
/// create draft (`human_id` is `None` — at most one draft per category is open, see
/// [`NavState::open_create`]).
///
/// `Ord` follows `Category`'s declared rail order, then the id, so the map iterates in a stable,
/// user-meaningful sequence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EditKey {
    /// The aggregate category the editor belongs to.
    pub category: Category,
    /// The saved record's stable id, or `None` for the category's create draft.
    pub human_id: Option<String>,
}

impl EditKey {
    /// The key of the saved record `(category, human_id)`.
    #[must_use]
    pub fn saved(category: Category, human_id: &str) -> Self {
        Self {
            category,
            human_id: Some(human_id.to_owned()),
        }
    }

    /// The key of `category`'s create draft.
    #[must_use]
    pub fn draft(category: Category) -> Self {
        Self {
            category,
            human_id: None,
        }
    }
}

/// One record's in-progress edit, parked in the shell so it survives its pane unmounting.
///
/// A record's pane exists only while its tab is active or docked (`screens/record_detail.rs` keys the
/// detail pane on the record's id), so the edit buffer cannot live in the pane: leaving both would
/// drop it.
/// `draft` and `seed` are the pane's typed `D: RecordDraft` erased to [`Any`], because one map holds
/// every aggregate's draft type; recover them with [`NavState::stashed_edit`].
#[derive(Clone)]
pub struct StashedEdit {
    /// The live draft the form binds to (the typed `D`).
    pub draft: Rc<dyn Any>,
    /// The committed values `draft` is diffed against for dirtiness (the same `D`).
    pub seed: Rc<dyn Any>,
    /// The provenance (why / citations / evidence / confidence) collected for the pending save.
    pub prov: ProvenanceDraft,
    /// `RecordDraft::is_valid` for `draft`, recorded on the way in so the Save gate can be read
    /// without knowing the draft's type.
    pub valid: bool,
}

impl StashedEdit {
    /// Parks `draft` — diffed against `seed`, with the `prov` collected so far — recording the draft's
    /// own validity.
    #[must_use]
    pub fn new<D: RecordDraft>(draft: D, seed: D, prov: ProvenanceDraft) -> Self {
        let valid = draft.is_valid();
        Self {
            draft: Rc::new(draft),
            seed: Rc::new(seed),
            prov,
            valid,
        }
    }
}

/// A transient shell notice (a toast): an already-localized `message`, its [`ToastKind`] (info
/// auto-dismisses, error is sticky), and the `seq` ticket that makes a per-notice auto-dismiss timer
/// safe — [`NavState::expire_notice`] clears the live notice only when `seq` still matches, so a
/// superseded or manually dismissed toast's stale timer is a no-op rather than killing its successor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The already-localized message.
    pub message: String,
    /// Info (auto-dismissing) or error (sticky).
    pub kind: ToastKind,
    /// The ticket this notice was raised under.
    pub seq: u32,
}

/// Shell-wide navigation/UI state, provided as context so every shell region shares one source of
/// truth. All fields are signals, so reads subscribe the reading component and writes from the
/// keyboard dispatcher re-render only the subscribers.
#[derive(Clone, Copy)]
pub struct NavState {
    /// The destination the work area is showing (the rail's category/tool selection).
    pub active: Signal<Destination>,
    /// The open record tabs, in strip order (the in-app tabstrip; independent of [`Self::active`]).
    pub records: Signal<Vec<OpenTab>>,
    /// The index into [`Self::records`] of the active record tab, or `None` when none are open.
    pub active_record: Signal<Option<usize>>,
    /// The record docked side-by-side with the active record (`.master-detail.split-2`), stored by
    /// its `(category, human_id)` key so it stays stable across tab closes/reorders rather than by a
    /// fragile index. `None` when no split is open. Resolve to a live [`RecordRef`] with
    /// [`Self::docked_record_ref`].
    pub docked_record: Signal<Option<(Category, String)>>,
    /// The record tab currently being dragged (its `(category, human_id)` key), set on
    /// `dragstart` and cleared on `dragend`/drop. Drives the tabstrip's `dragging` affordance and is
    /// consumed by [`Self::complete_tab_drag`] on drop.
    pub dragging_tab: Signal<Option<(Category, String)>>,
    /// The back/forward navigation history over [`NavLocation`]s (destination + focused record).
    pub history: Signal<NavHistory>,
    /// A monotonically-increasing "workspace data changed" ticket — bumped after any mutation
    /// (create, edit, undo) so shell-wide views derived from the data (the rail count badges)
    /// refetch.
    pub data_version: Signal<u32>,
    /// Which overlay is open, if any.
    pub overlay: Signal<Overlay>,
    /// The persisted colour-theme mode (System / Light / Dark) the user selected.
    pub theme_mode: Signal<ThemeMode>,
    /// The resolved colour theme mirrored onto `[data-theme]` (System resolved to a concrete palette).
    pub theme: Signal<Theme>,
    /// The recently-opened records (newest first, capped), driving the dashboard "Jump back in" list.
    /// Seeded from the workspace manifest at startup and persisted on change.
    pub recent: Signal<Vec<RecentItem>>,
    /// The record `⌘Z` should undo, or `None` when no undo is armed. Set by [`Self::request_undo`] for
    /// the active record only; the detail pane whose `(category, human_id)` this names observes it
    /// (`use_record_undo`), retracts the newest undoable assertion of its already-loaded change log,
    /// and clears it. The address itself is the scoping: with a docked split open two panes are
    /// mounted, and only the addressed one answers — never the docked one (#279).
    pub pending_undo: Signal<Option<EditKey>>,
    /// A pending prev/next-record step (`[` = `-1`, `]` = `+1`), set by the keyboard dispatcher and
    /// consumed by the active master-detail screen to open the neighbouring record.
    pub pending_step: Signal<Option<i8>>,
    /// The query the command palette opens pre-seeded with (the top-bar search's Enter, `⌘F`); the
    /// palette copies it into its input on open and then clears it.
    pub palette_seed: Signal<String>,
    /// A transient shell notice (a toast), e.g. "Nothing to undo" or the redo-unavailable
    /// explanation. `None` hides it.
    pub notice: Signal<Option<Notice>>,
    /// The ticket the next raised [`Notice`] carries ([`Self::notify`]/[`Self::notify_error`]) —
    /// bumped on every notice so [`Self::expire_notice`] can tell a stale timer from the live notice's
    /// own.
    pub notice_seq: Signal<u32>,
    /// A `(human_id, name)` the Geography tool should pre-select in its rail on next mount (the Place
    /// Map tab's "Open in Geography ↗"), or `None`. Set by [`Self::open_geography_focused`];
    /// `GeographyScreen` consumes and clears it once, on mount.
    pub geography_focus: Signal<Option<(String, String)>>,
    /// The `(category, human_id)` a fresh research-note draft should be pre-seeded with as its subject
    /// (the "Research notes" reverse tab's Add on a Person / Family / Event / Place), or `None`. Set by
    /// [`Self::open_research_note_about`]; `ResearchNoteCreateRecord` consumes and clears it once, on
    /// mount — the same one-shot handoff as [`Self::geography_focus`].
    pub research_note_subject: Signal<Option<(Category, String)>>,
    /// A close-tab/quit operation awaiting confirmation because it would discard unsaved work, or
    /// `None` when the confirm dialog is not showing. Set by [`Self::request_close_tab`] /
    /// [`Self::request_quit`]; resolved by [`Self::confirm_close`] / [`Self::cancel_close`].
    pub pending_close: Signal<Option<CloseRequest>>,
    /// The in-progress edits parked per editor — the shell's edit-buffer store. A record's pane exists
    /// only while its tab is active or docked, so the buffer cannot live in the pane: `use_record_edit`
    /// hydrates from here on mount ([`Self::stashed_edit`]) and writes through on every change
    /// ([`Self::stash_edit`]), which is what lets several records be mid-edit at once.
    ///
    /// The **keyset is the dirty set**: the tabstrip's unsaved marker and the close/quit confirm both
    /// read it through [`Self::tab_has_unsaved`], so an edit can never be discarded silently. Keyed like
    /// [`Self::docked_record`] so it survives tab reorders and closes.
    pub edit_drafts: Signal<BTreeMap<EditKey, StashedEdit>>,
    /// The remembered active related-item tab per open editor (#209), keyed like [`Self::edit_drafts`].
    /// Index `0` is stored as *absence* — [`Self::remember_tab`]/[`Self::remembered_tab`] — so the map
    /// stays the size of the operator's actual deviations from Overview. A record's pane exists only
    /// while its tab is active or docked, so the index cannot live in the pane: `use_detail_tab` seeds
    /// from here on mount and writes through on every change, the same shape as [`Self::edit_drafts`].
    pub detail_tabs: Signal<BTreeMap<EditKey, usize>>,
    /// The record whose save is running right now (the confirm's Save / Save all), or `None` when no
    /// save run is in flight. The record's own screen observes it, saves, and reports back through
    /// [`Self::note_save_finished`] — see [`SaveRequest`].
    pub save_request: Signal<Option<SaveRequest>>,
    /// The records still to save in the current run, in strip order: Save all walks them one at a time,
    /// arming [`Self::save_request`] for each in turn, activating it first so its pane is mounted.
    pub save_queue: Signal<Vec<EditKey>>,
    /// A monotonically-increasing "quit the application" ticket, bumped once a quit is confirmed (or
    /// requested with nothing unsaved). The desktop-only `QuitManager` observes it and closes the
    /// native window; it is a no-op under SSR, which mounts no window.
    pub quit_requested: Signal<u32>,
}

impl Default for NavState {
    fn default() -> Self {
        Self::new()
    }
}

impl NavState {
    /// Creates the shell state on the Dashboard with no record open, following the OS theme.
    #[must_use]
    pub fn new() -> Self {
        Self::with_prefs(ThemeMode::System, resolve_theme(ThemeMode::System), Vec::new())
    }

    /// Creates the shell state seeded with the resolved startup theme and persisted recent list:
    /// `mode` is the persisted preference, `resolved` the concrete palette it resolves to (already
    /// computed by the caller so the shell and the pre-launch window background agree), `recent` the
    /// "Jump back in" list read from the workspace manifest.
    #[must_use]
    pub fn with_prefs(mode: ThemeMode, resolved: Theme, recent: Vec<RecentItem>) -> Self {
        let mut history = NavHistory::default();
        history.push(NavLocation {
            destination: Destination::Category(Category::Dashboard),
            record: None,
        });
        Self {
            active: Signal::new(Destination::Category(Category::Dashboard)),
            records: Signal::new(Vec::new()),
            active_record: Signal::new(None),
            docked_record: Signal::new(None),
            dragging_tab: Signal::new(None),
            history: Signal::new(history),
            data_version: Signal::new(0),
            overlay: Signal::new(Overlay::None),
            theme_mode: Signal::new(mode),
            theme: Signal::new(resolved),
            recent: Signal::new(recent),
            pending_undo: Signal::new(None),
            pending_step: Signal::new(None),
            palette_seed: Signal::new(String::new()),
            notice: Signal::new(None),
            notice_seq: Signal::new(0),
            geography_focus: Signal::new(None),
            research_note_subject: Signal::new(None),
            pending_close: Signal::new(None),
            edit_drafts: Signal::new(BTreeMap::new()),
            detail_tabs: Signal::new(BTreeMap::new()),
            save_request: Signal::new(None),
            save_queue: Signal::new(Vec::new()),
            quit_requested: Signal::new(0),
        }
    }

    /// Requests an undo of the active record (`⌘Z`) by addressing [`Self::pending_undo`] at it; the
    /// detail pane it names observes the address and retracts the newest undoable assertion. A no-op
    /// when the active tab is a draft — a draft has no undo hook, so arming one would stick.
    pub fn request_undo(&mut self) {
        let Some(record) = self.active_record_ref() else {
            return;
        };
        self.pending_undo
            .set(Some(EditKey::saved(record.category, &record.human_id)));
    }

    /// Requests a prev/next-record step (`[`/`]`) on the active master-detail screen.
    pub fn step_record(&mut self, delta: i8) {
        self.pending_step.set(Some(delta));
    }

    /// Shows a transient shell notice (a toast) that auto-dismisses (`ShellToast` arms a 6s timer for
    /// [`ToastKind::Info`]).
    pub fn notify(&mut self, message: String) {
        let seq = self.raise_notice_seq();
        self.notice.set(Some(Notice {
            message,
            kind: ToastKind::Info,
            seq,
        }));
    }

    /// Shows a transient shell error notice (a toast) that stays until [`Self::dismiss_notice`] —
    /// `ShellToast` arms no auto-dismiss timer for [`ToastKind::Error`].
    pub fn notify_error(&mut self, message: String) {
        let seq = self.raise_notice_seq();
        self.notice.set(Some(Notice {
            message,
            kind: ToastKind::Error,
            seq,
        }));
    }

    /// Bumps and returns the ticket the next raised notice carries.
    fn raise_notice_seq(&mut self) -> u32 {
        let next = self.notice_seq.peek().wrapping_add(1);
        self.notice_seq.set(next);
        next
    }

    /// Dismisses the shell notice, whichever kind it is (the toast's own Dismiss action).
    pub fn dismiss_notice(&mut self) {
        self.notice.set(None);
    }

    /// Clears the shell notice raised under `seq`, if it is still live — the auto-dismiss timer
    /// `ShellToast` arms for an info notice. A no-op when `seq` is stale (the notice was superseded or
    /// already dismissed) or the live notice is an error (sticky; no timer clears it).
    pub fn expire_notice(&mut self, seq: u32) {
        let live = self
            .notice
            .peek()
            .as_ref()
            .is_some_and(|notice| notice.seq == seq && notice.kind == ToastKind::Info);
        if live {
            self.notice.set(None);
        }
    }

    /// Opens the command palette pre-seeded with `query` (the top-bar search's Enter).
    pub fn open_palette_seeded(&mut self, query: String) {
        self.palette_seed.set(query);
        self.overlay.set(Overlay::Palette);
    }

    /// Advances the theme mode (System → Light → Dark → System) and re-resolves the rendered theme.
    /// Returns the new mode so the caller can persist it.
    pub fn cycle_theme(&mut self) -> ThemeMode {
        let next = next_theme_mode(*self.theme_mode.peek());
        self.theme_mode.set(next);
        self.theme.set(resolve_theme(next));
        next
    }

    /// Requests context-aware creation of a new record on the active screen (the top-bar `New` and
    /// `⌘N`) by opening a draft tab ([`Self::open_create`]). A no-op unless the active destination is
    /// an aggregate category (not the Dashboard, and not a tool).
    pub fn request_new(&mut self) {
        let Destination::Category(category) = *self.active.peek() else {
            return;
        };
        if category == Category::Dashboard {
            return;
        }
        self.open_create(category);
    }

    /// Reveals `category` and opens a draft tab there (the tabstrip's new-record menu, the command
    /// palette) — unlike [`Self::request_new`], this works from any destination.
    pub fn request_new_for(&mut self, category: Category) {
        self.go_to(Destination::Category(category));
        self.open_create(category);
    }

    /// Opens a create-form draft tab for `category` and makes it active. At most one draft per
    /// category is open at a time: an existing draft is re-focused rather than duplicated. Nothing is
    /// stored until the draft commits ([`Self::commit_draft`]).
    pub fn open_create(&mut self, category: Category) {
        let existing = self
            .records
            .read()
            .iter()
            .position(|tab| tab.is_draft() && tab.category() == category);
        if let Some(index) = existing {
            self.active_record.set(Some(index));
            return;
        }
        self.records.write().push(OpenTab::Draft(category));
        let last = self.records.read().len().saturating_sub(1);
        self.active_record.set(Some(last));
    }

    /// Opens a research-note create draft pre-seeded with `(category, human_id)` as its subject — the
    /// Add on a record's "Research notes" tab. Reveals the `ResearchNotes` category so the draft's tab is
    /// visible next to its list, exactly as the rail's own New does.
    pub fn open_research_note_about(&mut self, category: Category, human_id: String) {
        self.research_note_subject.set(Some((category, human_id)));
        self.request_new_for(Category::ResearchNotes);
    }

    /// Commits a draft in place: replaces the open draft tab for `record.category` with the saved
    /// `record`, keeping its position in the strip and making it active, and records it in the "Jump
    /// back in" list. Falls back to opening `record` as a fresh tab if no draft is open (e.g. the
    /// draft was closed mid-commit).
    ///
    /// The create buffer parked for the category is dropped: it has just been stored, so leaving it
    /// would mark the new record's tab unsaved and refill its form on the next mount.
    pub fn commit_draft(&mut self, record: RecordRef) {
        self.drop_edit(&EditKey::draft(record.category));
        if let Some(kind) = record.category.aggregate_kind() {
            push_recent(
                &mut self.recent.write(),
                RecentItem::Record {
                    kind: kind.to_owned(),
                    human_id: record.human_id.clone(),
                    label: record.label.clone(),
                },
            );
        }
        let draft = self
            .records
            .read()
            .iter()
            .position(|tab| tab.is_draft() && tab.category() == record.category);
        let Some(index) = draft else {
            self.open_record(record);
            return;
        };
        self.records.write()[index] = OpenTab::Saved(record);
        self.active_record.set(Some(index));
        let location = self.current_location();
        self.history.write().push(location);
    }

    /// Cancels the open draft tab for `category`, closing it (Cancel on a create form) — which drops
    /// the create buffer parked for it ([`Self::close_record`]).
    pub fn cancel_draft(&mut self, category: Category) {
        let draft = self
            .records
            .read()
            .iter()
            .position(|tab| tab.is_draft() && tab.category() == category);
        if let Some(index) = draft {
            self.close_record(index);
        }
    }

    /// Marks the workspace data as changed so shell-wide derived views (the rail count badges)
    /// refetch. Called after a create/edit/undo succeeds.
    pub fn mark_changed(&mut self) {
        let next = self.data_version.peek().wrapping_add(1);
        self.data_version.set(next);
    }

    /// Navigates the work area to `destination` (the rail's category/tool selection). This does not
    /// touch the open record tabs — opening a record is [`Self::open_record`]. The "Jump back in"
    /// list remembers records only, so this never pushes to it — see [`Self::open_record`].
    ///
    /// Skips the history push when the resulting location is a bare category list with no record
    /// focused ([`NavLocation::is_recordless_list`]) — otherwise back/forward would step through
    /// empty list views. [`Self::open_record`] and [`Self::activate_record`] always focus a record,
    /// so their pushes are never skipped.
    pub fn go_to(&mut self, destination: Destination) {
        self.active.set(destination);
        let location = self.current_location();
        if !location.is_recordless_list() {
            self.history.write().push(location);
        }
    }

    /// Navigates to the Geography tool with `(human_id, name)` pre-focused (the Place Map tab's "Open
    /// in Geography ↗"): stashes the target in [`Self::geography_focus`] so `GeographyScreen`
    /// pre-selects it in the rail on mount, then navigates there like any other tool.
    pub fn open_geography_focused(&mut self, human_id: String, name: String) {
        self.geography_focus.set(Some((human_id, name)));
        self.go_to(Destination::Tool(Tool::Geography));
    }

    /// Opens `record` as a tab — focusing the existing tab with the same `(category, human_id)` or
    /// appending a new one — makes it the active record, and records it in the "Jump back in" list.
    pub fn open_record(&mut self, record: RecordRef) {
        if let Some(kind) = record.category.aggregate_kind() {
            push_recent(
                &mut self.recent.write(),
                RecentItem::Record {
                    kind: kind.to_owned(),
                    human_id: record.human_id.clone(),
                    label: record.label.clone(),
                },
            );
        }
        let existing = self
            .records
            .read()
            .iter()
            .position(|open| open.is_saved_key(record.category, &record.human_id));
        if let Some(index) = existing {
            self.active_record.set(Some(index));
        } else {
            self.records.write().push(OpenTab::Saved(record));
            let last = self.records.read().len().saturating_sub(1);
            self.active_record.set(Some(last));
        }
        let location = self.current_location();
        self.history.write().push(location);
    }

    /// Opens `record` as a tab and, only when the editor is currently hidden (the active destination
    /// is not an entity category — a tool, the Dashboard, or Help), reveals the record's category so
    /// the editor becomes visible. From within an entity category the rail/Explorer list is left as
    /// is, so following a link opens the record beside the current list without switching it (the
    /// VS Code "open from search" behaviour). Used by the shared `RecordLink`; list-row selection
    /// calls [`Self::open_record`] directly (already on that category).
    pub fn reveal_record(&mut self, record: RecordRef) {
        if entity_category(*self.active.peek()).is_none() {
            self.go_to(Destination::Category(record.category));
        }
        self.open_record(record);
    }

    /// Activates the open record tab at the 0-based `index`, if it exists.
    pub fn activate_record(&mut self, index: usize) {
        if index >= self.records.read().len() {
            return;
        }
        self.active_record.set(Some(index));
        let location = self.current_location();
        self.history.write().push(location);
    }

    /// Switches to the 1-based record tab `n` (`⌘1…9`), if it exists.
    pub fn switch_record(&mut self, n: u8) {
        self.activate_record(usize::from(n).saturating_sub(1));
    }

    /// Closes the open record tab at `index`, falling back to a neighbouring tab and clearing the
    /// active record when none remain. Does not change the rail's [`Self::active`] destination. Any
    /// edit parked for the closed tab is dropped with it — the edit is gone (the caller confirmed that
    /// via [`Self::request_close_tab`]), so leaving the entry would block later closes.
    pub fn close_record(&mut self, index: usize) {
        if index >= self.records.read().len() {
            return;
        }
        let closed = self.records.write().remove(index);
        self.drop_edit(&closed.edit_key());
        self.forget_tab(&closed.edit_key());
        let remaining = self.records.read().len();
        if remaining == 0 {
            self.active_record.set(None);
        } else {
            // Closing a tab left of the active one shifts it left; keep the same record focused.
            let active = self.active_record.read().unwrap_or(0);
            let active = if index < active { active - 1 } else { active };
            self.active_record.set(Some(active.min(remaining - 1)));
        }
        // Drop the dock if its record is no longer open (the docked tab itself was closed).
        let docked_gone = self.docked_record.peek().as_ref().is_some_and(|(category, human_id)| {
            !self
                .records
                .peek()
                .iter()
                .any(|open| open.is_saved_key(*category, human_id))
        });
        if docked_gone {
            self.docked_record.set(None);
        }
    }

    /// Parks `edit` under `key`, replacing whatever was there — `use_record_edit` writes the buffer
    /// through on every change, so the entry always holds the editor's current draft.
    pub fn stash_edit(&mut self, key: EditKey, edit: StashedEdit) {
        self.edit_drafts.write().insert(key, edit);
    }

    /// The edit parked under `key` as `(draft, seed, prov)`, for a pane hydrating its buffer on mount.
    ///
    /// `None` when nothing is parked there, and also when the entry holds a draft type other than `D`:
    /// the store is heterogeneous over every aggregate's draft, so a mismatch is possible and resolves
    /// to "no parked edit" rather than a panic in a mount-time hook.
    #[must_use]
    pub fn stashed_edit<D: RecordDraft>(&self, key: &EditKey) -> Option<(D, D, ProvenanceDraft)> {
        let edits = self.edit_drafts.peek();
        let edit = edits.get(key)?;
        let draft = edit.draft.downcast_ref::<D>()?;
        let seed = edit.seed.downcast_ref::<D>()?;
        Some((draft.clone(), seed.clone(), edit.prov.clone()))
    }

    /// Drops the edit parked under `key` — it was saved, cancelled, or its record closed, so no stale
    /// entry is left to mark the tab unsaved or block a later close. Writes only when there is
    /// something to drop, so the clean re-runs of the write-through effect do not churn the tabstrip.
    pub fn drop_edit(&mut self, key: &EditKey) {
        if !self.edit_drafts.peek().contains_key(key) {
            return;
        }
        self.edit_drafts.write().remove(key);
    }

    /// The remembered active related-item tab for `key`'s editor (#209), or `0` (Overview) when
    /// nothing is remembered — index 0 is stored as absence, see [`Self::detail_tabs`].
    /// Peeks rather than reads, like [`Self::stashed_edit`]: its caller is `use_detail_tab`'s mount-time
    /// `use_hook`, and subscribing a pane to the whole map there would re-render every mounted pane on
    /// any other record's tab change.
    #[must_use]
    pub fn remembered_tab(&self, key: &EditKey) -> usize {
        self.detail_tabs.peek().get(key).copied().unwrap_or(0)
    }

    /// Remembers `index` as `key`'s active tab, or forgets it when `index` is `0` — the default, so
    /// storing it would only grow the map without changing what [`Self::remembered_tab`] returns.
    ///
    /// Writes only on an actual change: `use_detail_tab`'s effect re-runs on every pane render, and an
    /// unconditional `write()` would mark the signal dirty each time.
    pub fn remember_tab(&mut self, key: EditKey, index: usize) {
        if index == 0 {
            self.forget_tab(&key);
            return;
        }
        if self.detail_tabs.peek().get(&key) == Some(&index) {
            return;
        }
        self.detail_tabs.write().insert(key, index);
    }

    /// Forgets `key`'s remembered tab (its record closed). A no-op when nothing is remembered.
    pub fn forget_tab(&mut self, key: &EditKey) {
        if !self.detail_tabs.peek().contains_key(key) {
            return;
        }
        self.detail_tabs.write().remove(key);
    }

    /// Whether an in-progress edit is parked under `key`.
    ///
    /// Reads reactively so the tabstrip's unsaved marker re-renders as edits come and go; the
    /// `⌘W`/`⌘Q` callers below run from event handlers, outside any reactive scope, so the
    /// subscription is inert for them.
    #[must_use]
    pub fn has_unsaved(&self, key: &EditKey) -> bool {
        self.edit_drafts.read().contains_key(key)
    }

    /// Whether the open tab at `index` holds work that closing it would discard: an unsaved draft, or
    /// a saved record with an in-progress edit parked in [`Self::edit_drafts`]. A draft tab always
    /// counts — nothing about it is stored yet, whether or not anything has been typed.
    #[must_use]
    pub fn tab_has_unsaved(&self, index: usize) -> bool {
        let Some(tab) = self.records.read().get(index).cloned() else {
            return false;
        };
        match tab {
            OpenTab::Draft(_) => true,
            OpenTab::Saved(_) => self.has_unsaved(&tab.edit_key()),
        }
    }

    /// Requests closing the record tab at `index` (`⌘W`, the tabstrip `✕`): closes it immediately
    /// unless it holds unsaved work ([`Self::tab_has_unsaved`]), in which case the confirm dialog arms
    /// instead of discarding it silently. The single path both callers share, so unsaved work cannot
    /// be lost with one click.
    pub fn request_close_tab(&mut self, index: usize) {
        if self.tab_has_unsaved(index) {
            self.pending_close.set(Some(CloseRequest::Tab(index)));
        } else {
            self.close_record(index);
        }
    }

    /// Requests quitting the application (`⌘Q`): arms the confirm dialog if any open tab holds unsaved
    /// work ([`Self::tab_has_unsaved`] — a draft or an in-progress edit), otherwise bumps
    /// [`Self::quit_requested`] immediately (nothing to lose).
    pub fn request_quit(&mut self) {
        let open = self.records.peek().len();
        let unsaved = (0..open).any(|index| self.tab_has_unsaved(index));
        if unsaved {
            self.pending_close.set(Some(CloseRequest::Quit));
        } else {
            self.quit_now();
        }
    }

    /// Whether the confirm can offer **Save** for the open tab at `index`: an in-progress edit is
    /// parked for it and that edit is valid ([`StashedEdit::valid`]). A draft tab with nothing typed
    /// has no parked buffer, so there is nothing to save — the confirm says so rather than showing a
    /// dead button.
    #[must_use]
    pub fn tab_is_savable(&self, index: usize) -> bool {
        let Some(tab) = self.records.read().get(index).cloned() else {
            return false;
        };
        self.edit_drafts
            .read()
            .get(&tab.edit_key())
            .is_some_and(|edit| edit.valid)
    }

    /// Saves the open tab at `index` and then closes it (the close confirm's **Save**): activates the
    /// tab so its pane is mounted, arms the save, and dismisses the confirm. Nothing closes until the
    /// record's screen reports the save finished ([`Self::note_save_finished`]).
    pub fn save_then_close(&mut self, index: usize) {
        let Some(tab) = self.records.peek().get(index).cloned() else {
            return;
        };
        self.pending_close.set(None);
        self.save_queue.set(Vec::new());
        self.begin_save(tab.edit_key(), SaveThen::CloseTab(index));
    }

    /// Saves every open tab that *can* be saved ([`Self::tab_is_savable`]), in strip order, and then
    /// quits (the quit confirm's **Save all**): queues them and arms the first.
    ///
    /// The exception the name does not carry: the run quits only when it covered every tab holding
    /// unsaved work. A record that cannot be saved yet — an invalid edit, an untouched `⌘N` draft — is
    /// neither saved nor discarded; it stays open with its work intact and the app keeps running, so
    /// the run ends in [`SaveThen::StayOpen`] instead. With nothing savable at all the confirm is
    /// simply dismissed, unless nothing is unsaved either, in which case the quit fires.
    pub fn save_all_then_quit(&mut self) {
        let tabs = self.records.peek().clone();
        let mut queue = Vec::new();
        let mut unsaved = 0_usize;
        for (index, tab) in tabs.iter().enumerate() {
            if !self.tab_has_unsaved(index) {
                continue;
            }
            unsaved += 1;
            if self.tab_is_savable(index) {
                queue.push(tab.edit_key());
            }
        }
        self.pending_close.set(None);
        if queue.is_empty() {
            if unsaved == 0 {
                self.quit_now();
            }
            return;
        }
        let then = if queue.len() == unsaved {
            SaveThen::Quit
        } else {
            SaveThen::StayOpen
        };
        let first = queue.remove(0);
        self.save_queue.set(queue);
        self.begin_save(first, then);
    }

    /// Reports the outcome of the save the shell asked for: `(category, human_id)` names the editor
    /// that saved (`human_id` is `None` for a category's create draft), `ok` whether it succeeded. A
    /// no-op unless it names the armed request, so a Save the user clicked themselves never resolves a
    /// run.
    ///
    /// On success the record's parked edit is dropped and the next queued record is armed; once the
    /// queue empties the run's [`SaveThen`] terminus is applied. On failure the whole run is abandoned
    /// with every tab left open — the screen has already reported the error.
    pub fn note_save_finished(&mut self, category: Category, human_id: Option<&str>, ok: bool) {
        let Some(request) = self.save_request.peek().clone() else {
            return;
        };
        let reported = match human_id {
            Some(human_id) => EditKey::saved(category, human_id),
            None => EditKey::draft(category),
        };
        if request.key != reported {
            return;
        }
        if !ok {
            self.abandon_save_run();
            return;
        }
        self.drop_edit(&request.key);
        self.advance_save_run(&request);
    }

    /// Arms `key`'s save for the run ending in `then`, activating its tab first: a save target's pane
    /// may not be mounted yet (active or docked are the only mounted states), and the pane is what
    /// knows how to save.
    fn begin_save(&mut self, key: EditKey, then: SaveThen) {
        self.reveal_editor(&key);
        self.save_request.set(Some(SaveRequest { key, then }));
    }

    /// Brings `key`'s tab forward so its pane mounts: reveals the editor when the work area is showing
    /// something else entirely (a tool, the Dashboard, Help — none of which mount a record pane), then
    /// activates the tab.
    fn reveal_editor(&mut self, key: &EditKey) {
        if entity_category(*self.active.peek()).is_none() {
            self.go_to(Destination::Category(key.category));
        }
        let index = self.records.peek().iter().position(|tab| tab.edit_key() == *key);
        if let Some(index) = index {
            self.activate_record(index);
        }
    }

    /// Arms the next queued record, or — with the queue drained — applies the run's [`SaveThen`]
    /// terminus. `saved` is the request that just finished.
    fn advance_save_run(&mut self, saved: &SaveRequest) {
        let next = if self.save_queue.peek().is_empty() {
            None
        } else {
            Some(self.save_queue.write().remove(0))
        };
        if let Some(next) = next {
            self.begin_save(next, saved.then);
            return;
        }
        self.save_request.set(None);
        match saved.then {
            SaveThen::CloseTab(index) => {
                if let Some(index) = self.save_close_index(index, &saved.key) {
                    self.close_record(index);
                }
            }
            SaveThen::Quit => self.quit_now(),
            SaveThen::StayOpen => (),
        }
    }

    /// Abandons the run in flight, leaving every tab open and every remaining edit parked.
    fn abandon_save_run(&mut self) {
        self.save_request.set(None);
        self.save_queue.set(Vec::new());
    }

    /// The strip index a finished [`CloseRequest::Tab`] should close: the armed `index` when it still
    /// names the editor that saved, otherwise wherever that editor sits now (the strip may have moved
    /// under the save). `None` when the tab has since been closed, so there is nothing left to close.
    ///
    /// A committed create draft is the one case where no tab carries `key` any more: [`Self::commit_draft`]
    /// swapped the stored record into the draft's slot and made it active, so the active tab is the tab
    /// that was saved.
    fn save_close_index(&self, index: usize, key: &EditKey) -> Option<usize> {
        let records = self.records.peek();
        if records.get(index).is_some_and(|tab| tab.edit_key() == *key) {
            return Some(index);
        }
        if let Some(found) = records.iter().position(|tab| tab.edit_key() == *key) {
            return Some(found);
        }
        if key.human_id.is_some() {
            return None;
        }
        let active = (*self.active_record.peek())?;
        let committed = records
            .get(active)
            .is_some_and(|tab| !tab.is_draft() && tab.category() == key.category);
        committed.then_some(active)
    }

    /// Applies the pending close/quit, discarding the unsaved work (the confirm dialog's Discard) and
    /// clears it.
    pub fn confirm_close(&mut self) {
        let request = *self.pending_close.peek();
        self.pending_close.set(None);
        match request {
            Some(CloseRequest::Tab(index)) => self.close_record(index),
            Some(CloseRequest::Quit) => self.quit_now(),
            None => {}
        }
    }

    /// Dismisses the pending close/quit without applying it (the confirm dialog's cancel action),
    /// abandoning any save run it armed.
    pub fn cancel_close(&mut self) {
        self.pending_close.set(None);
        self.abandon_save_run();
    }

    /// Bumps [`Self::quit_requested`] so the desktop-only `QuitManager` closes the native window.
    fn quit_now(&mut self) {
        let next = self.quit_requested.peek().wrapping_add(1);
        self.quit_requested.set(next);
    }

    /// The active open tab (saved record or draft), if any.
    #[must_use]
    pub fn active_tab(&self) -> Option<OpenTab> {
        self.active_record
            .read()
            .and_then(|index| self.records.read().get(index).cloned())
    }

    /// The active *saved* record, if any (for the tabstrip, breadcrumb, status bar, and detail pane).
    /// Returns `None` when the active tab is an unsaved draft.
    #[must_use]
    pub fn active_record_ref(&self) -> Option<RecordRef> {
        self.active_tab().and_then(|tab| tab.as_saved().cloned())
    }

    /// Docks the record tab keyed by `(category, human_id)` side-by-side with the active record. A
    /// no-op unless the key names an open tab that is not itself the active record (a record cannot
    /// dock beside itself); docking the record already docked toggles the split off; otherwise it
    /// replaces the dock. Stored by key so it survives tab closes/reorders ([`Self::docked_record`]).
    pub fn dock_record(&mut self, category: Category, human_id: &str) {
        let is_open = self
            .records
            .peek()
            .iter()
            .any(|open| open.is_saved_key(category, human_id));
        if !is_open || self.is_active_key(category, human_id) {
            return;
        }
        let already_docked = self
            .docked_record
            .peek()
            .as_ref()
            .is_some_and(|(docked_category, docked_id)| *docked_category == category && docked_id == human_id);
        if already_docked {
            self.docked_record.set(None);
        } else {
            self.docked_record.set(Some((category, human_id.to_owned())));
        }
    }

    /// Docks the 1-based record tab `n` (`⌘⇧1…9`), if it exists — mirrors [`Self::switch_record`],
    /// resolving the index to its key before delegating to [`Self::dock_record`].
    pub fn dock_record_tab(&mut self, n: u8) {
        let index = usize::from(n).saturating_sub(1);
        let key = self
            .records
            .peek()
            .get(index)
            .and_then(|tab| tab.as_saved().map(|record| (record.category, record.human_id.clone())));
        if let Some((category, human_id)) = key {
            self.dock_record(category, &human_id);
        }
    }

    /// Closes the docked split (the undock `✕`), leaving the tabs untouched.
    pub fn undock_record(&mut self) {
        self.docked_record.set(None);
    }

    /// The docked record as a live [`RecordRef`] (fresh label resolved against the open tabs), or
    /// `None` when nothing is docked, the docked tab has since closed, or the docked tab is itself
    /// the active record (the split collapses while it is active, but the dock state survives so it
    /// returns when another record becomes active).
    #[must_use]
    pub fn docked_record_ref(&self) -> Option<RecordRef> {
        let (category, human_id) = self.docked_record.read().clone()?;
        // `read()` (not `peek()`) so the reader re-renders when the active record changes and the
        // split needs to collapse or return.
        let active = self
            .active_record
            .read()
            .and_then(|index| self.records.read().get(index).cloned());
        if active.is_some_and(|tab| tab.is_saved_key(category, &human_id)) {
            return None;
        }
        self.records
            .read()
            .iter()
            .find(|open| open.is_saved_key(category, &human_id))
            .and_then(|tab| tab.as_saved().cloned())
    }

    /// Begins a tab drag from the tab keyed by `(category, human_id)` (`dragstart`).
    pub fn begin_tab_drag(&mut self, category: Category, human_id: &str) {
        self.dragging_tab.set(Some((category, human_id.to_owned())));
    }

    /// Ends a tab drag without a drop (`dragend`).
    pub fn end_tab_drag(&mut self) {
        self.dragging_tab.set(None);
    }

    /// Completes a tab drag by docking the dragged tab (`drop`): clears the drag and, if one was
    /// live, docks its record. The whole drop is one transition so it is testable end-to-end.
    pub fn complete_tab_drag(&mut self) {
        let dragged = self.dragging_tab.peek().clone();
        self.dragging_tab.set(None);
        if let Some((category, human_id)) = dragged {
            self.dock_record(category, &human_id);
        }
    }

    /// Whether `(category, human_id)` is the key of the currently active record tab.
    fn is_active_key(&self, category: Category, human_id: &str) -> bool {
        self.active_record
            .peek()
            .and_then(|index| self.records.peek().get(index).cloned())
            .is_some_and(|active| active.is_saved_key(category, human_id))
    }

    /// Closes any open overlay.
    pub fn close_overlay(&mut self) {
        self.overlay.set(Overlay::None);
    }

    /// Dismisses the topmost dismissable layer (`Esc`). The close/quit confirm sits above every
    /// overlay, so while one is armed `Esc` runs its **Cancel** path ([`Self::cancel_close`]) — it must
    /// not discard the tab, and it must abandon any save run the confirm armed — and the overlay behind
    /// it stays open. With no confirm armed it closes the overlay as before.
    pub fn dismiss_topmost(&mut self) {
        if self.pending_close.peek().is_some() {
            self.cancel_close();
            return;
        }
        self.close_overlay();
    }

    /// Renames the label of the open record tab identified by `(category, human_id)`, if it is still
    /// open. A no-op when `label` is empty (a record is never renamed to a blank tab) or the record
    /// has since been closed.
    pub fn set_record_label(&mut self, category: Category, human_id: &str, label: String) {
        if label.is_empty() {
            return;
        }
        let mut records = self.records.write();
        let Some(OpenTab::Saved(record)) = records.iter_mut().find(|open| open.is_saved_key(category, human_id)) else {
            return;
        };
        record.label = label;
    }

    /// Re-keys the open record tab identified by `(category, old_human_id)` to `new_human_id` after a
    /// rename, so the tab (and the keyed detail pane it drives) follow the record to its new id. A
    /// no-op when the id is unchanged or the record has since been closed.
    pub fn rename_record(&mut self, category: Category, old_human_id: &str, new_human_id: String) {
        if old_human_id == new_human_id {
            return;
        }
        // Re-key the dock so the split follows the record to its new id.
        if let Some((docked_category, docked_id)) = self.docked_record.write().as_mut()
            && *docked_category == category
            && docked_id == old_human_id
        {
            docked_id.clone_from(&new_human_id);
        }
        Self::rekey_by_id(&mut self.edit_drafts, category, old_human_id, &new_human_id);
        Self::rekey_by_id(&mut self.detail_tabs, category, old_human_id, &new_human_id);
        self.rekey_save_run(category, old_human_id, &new_human_id);
        let mut records = self.records.write();
        let Some(OpenTab::Saved(record)) = records
            .iter_mut()
            .find(|open| open.is_saved_key(category, old_human_id))
        else {
            return;
        };
        record.human_id = new_human_id;
    }

    /// Moves the entry keyed by `(category, old_human_id)` in `map` to `new_human_id` (a rename), so it
    /// stays attached to the record for a record that changed id mid-edit — the shared re-key step
    /// every per-editor map needs ([`Self::edit_drafts`]'s parked edit, [`Self::detail_tabs`]'s
    /// remembered tab). A no-op when nothing is keyed under the old id.
    fn rekey_by_id<V>(
        map: &mut Signal<BTreeMap<EditKey, V>>,
        category: Category,
        old_human_id: &str,
        new_human_id: &str,
    ) where
        V: 'static,
    {
        let old = EditKey::saved(category, old_human_id);
        if !map.peek().contains_key(&old) {
            return;
        }
        let mut entries = map.write();
        if let Some(value) = entries.remove(&old) {
            entries.insert(EditKey::saved(category, new_human_id), value);
        }
    }

    /// Moves an armed/queued save from `old_human_id` to `new_human_id` (a rename), so a save that
    /// renamed its own record still reports back under a key the run recognises — otherwise the run
    /// would hang with the tab never closing. A no-op when no run names the old id.
    fn rekey_save_run(&mut self, category: Category, old_human_id: &str, new_human_id: &str) {
        let old = EditKey::saved(category, old_human_id);
        let new = EditKey::saved(category, new_human_id);
        if self
            .save_request
            .peek()
            .as_ref()
            .is_some_and(|request| request.key == old)
            && let Some(request) = self.save_request.write().as_mut()
        {
            request.key = new.clone();
        }
        let queued = self.save_queue.peek().iter().position(|key| *key == old);
        if let Some(index) = queued {
            self.save_queue.write()[index] = new;
        }
    }

    /// Moves the navigation history one step back and applies the resulting location, if any (`⌘←`
    /// browser-style navigation). A no-op at the start of history.
    pub fn history_back(&mut self) {
        let Some(location) = self.history.write().back() else {
            return;
        };
        self.apply(location);
    }

    /// Moves the navigation history one step forward and applies the resulting location, if any
    /// (`⌘→` browser-style navigation). A no-op at the end of history.
    pub fn history_forward(&mut self) {
        let Some(location) = self.history.write().forward() else {
            return;
        };
        self.apply(location);
    }

    /// Whether [`Self::history_back`] would move the history (there is an earlier entry).
    #[must_use]
    pub fn can_back(&self) -> bool {
        self.history.read().can_back()
    }

    /// Whether [`Self::history_forward`] would move the history (there is a later entry).
    #[must_use]
    pub fn can_forward(&self) -> bool {
        self.history.read().can_forward()
    }

    /// The current navigation location: the active destination plus the active record's
    /// `(category, human_id)`, if any.
    fn current_location(&self) -> NavLocation {
        NavLocation {
            destination: *self.active.peek(),
            record: self
                .active_record_ref()
                .map(|record| (record.category, record.human_id)),
        }
    }

    /// Applies a [`NavLocation`] pulled from history: sets the active destination and, if the
    /// location names a record still open, re-focuses it — without pushing a new history entry.
    fn apply(&mut self, location: NavLocation) {
        self.active.set(location.destination);
        let index = location.record.and_then(|(category, human_id)| {
            self.records
                .read()
                .iter()
                .position(|open| open.is_saved_key(category, &human_id))
        });
        self.active_record.set(index);
    }
}

/// Subscribes the calling reactive scope to [`NavState::data_version`] and returns the current ticket,
/// or 0 when no shell is mounted (host-free SSR probes provide no [`NavState`]).
///
/// Call it in the **synchronous** part of a `use_resource` closure — that is the part Dioxus subscribes
/// on — so the resource refetches after every create/edit/undo ([`NavState::mark_changed`]). Take the
/// state as an argument rather than consuming the context here: the context must be consumed in the
/// component/hook body, not inside the closure the resource re-runs.
#[must_use]
pub fn data_version_ticket(nav: Option<NavState>) -> u32 {
    nav.map_or(0, |nav| *nav.data_version.read())
}
