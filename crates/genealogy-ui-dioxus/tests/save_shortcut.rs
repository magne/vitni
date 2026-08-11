//! SSR-probe assertions for [`NavState::request_save_active`] (issue #206): `⌘S`/`ShortcutAction::SaveRecord`
//! targets the active record's editor, falling back to a docked one when the active pane has nothing
//! savable, and reuses the close/quit confirm's save-request machinery so the pane runs its own save
//! (`use_save_on_request`) and drops out of edit mode once it reports success. Follows `close_confirm.rs`'s
//! probe shape (`record`/`mark_dirty`/`edited`/`render`/`render_settled`/`SavePane`), minus the chrome
//! localizer — no probe here renders a dialog's localized text.

use dioxus::prelude::*;
use genealogy_ui::{Category, ProvenanceDraft, RecordRef, TagDraft};
use genealogy_ui_dioxus::screens::{use_record_edit, use_save_on_request};
use genealogy_ui_dioxus::shell::nav_state::{EditKey, NavState, StashedEdit};

/// A record in the `Tags` category, matching `SavePane`'s category below.
fn record(human_id: &str, label: &str) -> RecordRef {
    RecordRef {
        category: Category::Tags,
        human_id: human_id.to_owned(),
        label: label.to_owned(),
    }
}

/// Parks a dirty, valid in-progress edit of the saved record `human_id`, the way a mid-edit detail
/// pane does.
fn mark_dirty(nav: &mut NavState, human_id: &str) {
    nav.stash_edit(
        EditKey::saved(Category::Tags, human_id),
        StashedEdit::new(edited(), TagDraft::new(), ProvenanceDraft::default()),
    );
}

/// A dirty, valid draft — the buffer a mid-edit pane holds.
fn edited() -> TagDraft {
    TagDraft {
        name: "edited".to_owned(),
        ..TagDraft::new()
    }
}

/// Renders a probe component to an HTML string.
fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// Renders a probe and settles it, so `use_effect` bodies (and the cascade they dirty) run — needed
/// whenever a `SavePane` is mounted, since [`use_save_on_request`] fires from an effect.
fn render_settled(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    for _ in 0..8 {
        vdom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
    dioxus_ssr::render(&vdom)
}

/// One editor key in [`EditKey`]'s own `category/id` form.
fn key_id(key: &EditKey) -> String {
    key.to_string()
}

/// The marker block: whether `request_save_active` armed a save, and which editor's save is armed.
fn probe(nav: &NavState, armed: bool) -> Element {
    let saving = nav
        .save_request
        .read()
        .as_ref()
        .map_or_else(|| "NONE".to_owned(), |request| key_id(&request.key));
    let dirty = nav.edit_drafts.read().keys().map(key_id).collect::<Vec<_>>().join(",");
    rsx! {
        div { "ARMED:{armed}" }
        div { "SAVING:{saving}" }
        div { "DIRTY:[{dirty}]" }
    }
}

/// A stand-in for an aggregate's detail pane: the shared edit buffer plus the save-on-request wiring.
/// `on_save` reseeds the buffer from the draft (what a real save's reload eventually does), so a
/// successful save leaves the pane clean as well as out of edit mode.
#[component]
fn SavePane(human_id: String) -> Element {
    let mut state = use_record_edit::<TagDraft>(Category::Tags, &human_id, &TagDraft::new());
    let mut saves = use_signal(|| 0_u32);
    let mut nav = use_context::<NavState>();
    let id = human_id.clone();
    let on_save = use_callback(move |()| {
        state.seed.set(state.draft.read().clone());
        nav.note_save_finished(&EditKey::saved(Category::Tags, &id), true);
        saves += 1;
    });
    use_save_on_request(EditKey::saved(Category::Tags, &human_id), state, on_save);
    let editing = *state.editing.read();
    rsx! {
        div { "{human_id}:SAVES:{saves}" }
        div { "{human_id}:EDITING:{editing}" }
    }
}

fn save_active_with_a_clean_tab() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let mut armed = use_signal(|| false);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        let result = nav.request_save_active();
        armed.set(result);
    });
    probe(&nav, *armed.read())
}

#[test]
fn request_save_active_is_false_with_a_clean_tab() {
    let html = render(save_active_with_a_clean_tab);
    assert!(html.contains("ARMED:false"), "nothing savable, nothing armed:\n{html}");
    assert!(html.contains("SAVING:NONE"), "no save request in flight:\n{html}");
}

fn save_active_with_a_dirty_valid_tab() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let mut armed = use_signal(|| false);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        mark_dirty(&mut nav, "T0001");
        let result = nav.request_save_active();
        armed.set(result);
    });
    rsx! {
        {probe(&nav, *armed.read())}
        SavePane { human_id: "T0001".to_owned() }
    }
}

#[test]
fn request_save_active_drives_the_active_panes_own_save() {
    let html = render_settled(save_active_with_a_dirty_valid_tab);
    assert!(
        html.contains("ARMED:true"),
        "a dirty, valid edit arms the save:\n{html}"
    );
    assert!(
        html.contains("T0001:SAVES:1"),
        "the active pane's own save closure ran:\n{html}"
    );
}

fn save_active_falls_back_to_the_docked_record() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let mut armed = use_signal(|| false);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        nav.open_record(record("T0002", "Bob"));
        mark_dirty(&mut nav, "T0002");
        nav.activate_record(0);
        // `dock_record` refuses to dock the active record, so Ada (index 0) must already be active
        // before Bob (T0002) can be docked beside it.
        nav.dock_record(Category::Tags, "T0002");
        let result = nav.request_save_active();
        armed.set(result);
    });
    rsx! {
        {probe(&nav, *armed.read())}
        SavePane { human_id: "T0001".to_owned() }
        SavePane { human_id: "T0002".to_owned() }
    }
}

#[test]
fn request_save_active_picks_the_docked_record_when_only_it_is_savable() {
    // The active tab (Ada) is clean; only the docked one (Bob) has a parked, valid edit.
    let html = render_settled(save_active_falls_back_to_the_docked_record);
    assert!(
        html.contains("ARMED:true"),
        "the docked record's edit is savable, so a save arms:\n{html}"
    );
    assert!(
        html.contains("T0002:SAVES:1"),
        "the docked pane's own save closure ran:\n{html}"
    );
    assert!(
        html.contains("T0001:SAVES:0"),
        "the clean active pane's save never runs:\n{html}"
    );
}

fn save_active_leaves_the_pane_settled() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        mark_dirty(&mut nav, "T0001");
        nav.request_save_active();
    });
    rsx! {
        {probe(&nav, true)}
        SavePane { human_id: "T0001".to_owned() }
    }
}

#[test]
fn a_successful_save_leaves_the_pane_out_of_edit_mode_with_no_parked_buffer() {
    let html = render_settled(save_active_leaves_the_pane_settled);
    assert!(
        html.contains("T0001:EDITING:false"),
        "the pane returns to view mode once its save reports success:\n{html}"
    );
    assert!(
        html.contains("DIRTY:[]"),
        "the reseeded, clean draft leaves no parked buffer behind:\n{html}"
    );
}
