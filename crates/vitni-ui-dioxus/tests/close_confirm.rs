//! SSR-probe assertions for the close-tab/quit confirm flow on [`NavState`] (PR1 §1.4): closing a
//! *clean* saved tab is immediate; closing one that holds unsaved work — a draft, or a saved record
//! with an in-progress edit parked in [`NavState::edit_drafts`] (issue #200) — arms the confirm dialog
//! instead of discarding it silently. The same holds for the close the *window manager* starts — the
//! titlebar ✕, a session logout, `wmctrl -c` (issue #281) — which arrives as
//! [`NavState::request_window_close`] and answers whether the caller must stop it. Like `dock.rs`, each
//! probe drives `NavState` in `use_hook` and renders a small marker the test inspects.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_ui::{Category, ProvenanceDraft, RecordRef, TagDraft};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::screens::{use_record_edit, use_save_on_request};
use vitni_ui_dioxus::shell::ChromeCtx;
use vitni_ui_dioxus::shell::close_confirm::CloseConfirmDialog;
use vitni_ui_dioxus::shell::nav_state::{DraftId, EditKey, NavState, Overlay, SaveRequest, SaveThen, StashedEdit};
use vitni_ui_dioxus::shell::tabstrip::RecordTabstrip;

/// A chrome localizer for a single explicit language (deterministic for tests).
fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

fn record(human_id: &str, label: &str) -> RecordRef {
    RecordRef {
        category: Category::People,
        human_id: human_id.to_owned(),
        label: label.to_owned(),
    }
}

/// Parks an in-progress edit of the saved record `(category, human_id)` in the shell, the way a
/// mid-edit detail pane does. The draft type is immaterial to the confirm flow, so every probe here
/// parks a [`TagDraft`]; what matters is that the key is present.
fn mark_dirty(nav: &mut NavState, category: Category, human_id: &str) {
    nav.stash_edit(
        EditKey::saved(category, human_id),
        StashedEdit::new(edited(), TagDraft::new(), ProvenanceDraft::default()),
    );
}

/// Parks an in-progress edit that is dirty but **invalid** (a tag with its name cleared): Save is not
/// on offer for it, so the confirm must say so rather than showing a dead button.
fn mark_dirty_invalid(nav: &mut NavState, category: Category, human_id: &str) {
    let draft = TagDraft {
        priority: "7".to_owned(),
        ..TagDraft::new()
    };
    nav.stash_edit(
        EditKey::saved(category, human_id),
        StashedEdit::new(draft, TagDraft::new(), ProvenanceDraft::default()),
    );
}

/// Parks a dirty, valid buffer for the create draft `draft` — what a half-filled create form holds.
fn mark_draft_dirty(nav: &mut NavState, draft: DraftId) {
    nav.stash_edit(
        EditKey::draft(Category::People, draft),
        StashedEdit::new(edited(), TagDraft::new(), ProvenanceDraft::default()),
    );
}

/// What a create screen does on a successful commit ([`finish_draft_commit`](vitni_ui_dioxus::screens::finish_draft_commit)):
/// the draft becomes a stored record in its own slot, and the save reports back under the key that
/// record now has.
fn commit_and_report(nav: &mut NavState, draft: DraftId, human_id: &str, label: &str) {
    nav.commit_draft(draft, record(human_id, label));
    nav.note_save_finished(&EditKey::saved(Category::People, human_id), true);
}

/// Parks a create buffer for `draft` that names itself `name` — what typing into a create form writes
/// through, and what its tab (and this dialog) is then titled by.
fn name_draft(nav: &mut NavState, draft: DraftId, name: &str) {
    let typed = TagDraft {
        name: name.to_owned(),
        ..TagDraft::new()
    };
    nav.stash_edit(
        EditKey::draft(Category::People, draft),
        StashedEdit::new(typed, TagDraft::new(), ProvenanceDraft::default()),
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

/// Renders a probe and settles it, so `use_effect` bodies (and the cascade they dirty) run — what the
/// pane probes need, since [`use_save_on_request`] fires from an effect. Mirrors `edit_stash.rs`.
fn render_settled(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    for _ in 0..8 {
        vdom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
    dioxus_ssr::render(&vdom)
}

/// One editor key in [`EditKey`]'s own `category/id` form, a create draft as `category/#n`.
fn key_id(key: &EditKey) -> String {
    key.to_string()
}

/// The marker block: open-tab count, whether the confirm is armed, the quit ticket value, the keys
/// currently holding a parked in-progress edit (`category/human_id`), the save run — the editor
/// whose save is armed right now, the queue waiting behind it, and which tab is active (a run
/// activates each record in turn so its pane is mounted to save it) — and the shell notice, if one
/// is showing (e.g. the #302 incomplete-run notice).
fn probe(nav: &NavState) -> Element {
    let tabs = nav.records.read().len();
    let pending = if nav.pending_close.read().is_some() {
        "SOME"
    } else {
        "NONE"
    };
    let quit = *nav.quit_requested.read();
    let dirty = nav.edit_drafts.read().keys().map(key_id).collect::<Vec<_>>().join(",");
    let saving = nav
        .save_request
        .read()
        .as_ref()
        .map_or_else(|| "NONE".to_owned(), |request| key_id(&request.key));
    let queue = nav.save_queue.read().iter().map(key_id).collect::<Vec<_>>().join(",");
    let active = nav
        .active_record
        .read()
        .map_or_else(|| "-".to_owned(), |index| index.to_string());
    let notice = nav
        .notice
        .read()
        .as_ref()
        .map_or_else(|| "NONE".to_owned(), |notice| notice.message.clone());
    rsx! {
        div { "TABS:{tabs}" }
        div { "PENDING:{pending}" }
        div { "QUIT:{quit}" }
        div { "DIRTY:[{dirty}]" }
        div { "SAVING:{saving}" }
        div { "QUEUE:[{queue}]" }
        div { "ACTIVE:{active}" }
        div { "NOTICE:{notice}" }
    }
}

/// The markup of the dialog button whose visible label is `label`, from its `<button` to its
/// `</button>`. A bare `html.contains("disabled")` cannot tell which control carries the attribute, so
/// every Save/Save-all gate assertion below is made against this slice alone.
fn button_markup(html: &str, label: &str) -> String {
    let close = format!(">{label}</button>");
    let Some(end) = html.find(&close) else {
        return String::new();
    };
    let start = html[..end].rfind("<button").unwrap_or(0);
    html[start..end + close.len()].to_owned()
}

/// Reports the armed save as finished, `ok` or not, the way the record's own screen does once its
/// commit returns. A no-op when no save is armed.
fn finish_armed(nav: &mut NavState, ok: bool) {
    let Some(key) = nav.save_request.peek().as_ref().map(|request| request.key.clone()) else {
        return;
    };
    nav.note_save_finished(&key, ok);
}

fn close_saved_tab_is_immediate() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.request_close_tab(0);
    });
    probe(&nav)
}

#[test]
fn closing_a_saved_tab_is_immediate_with_no_confirm() {
    let html = render(close_saved_tab_is_immediate);
    assert!(html.contains("TABS:0"), "the saved tab closes immediately:\n{html}");
    assert!(
        html.contains("PENDING:NONE"),
        "no confirm armed for a saved tab:\n{html}"
    );
}

fn close_draft_tab_arms_confirm() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_close_tab(0);
    });
    probe(&nav)
}

#[test]
fn closing_a_draft_tab_arms_the_confirm_instead_of_discarding() {
    let html = render(close_draft_tab_arms_confirm);
    assert!(html.contains("TABS:1"), "the draft survives until confirmed:\n{html}");
    assert!(html.contains("PENDING:SOME"), "the confirm dialog is armed:\n{html}");
}

fn confirm_closes_the_pending_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_close_tab(0);
        nav.confirm_close();
    });
    probe(&nav)
}

#[test]
fn confirming_closes_the_pending_draft() {
    let html = render(confirm_closes_the_pending_draft);
    assert!(html.contains("TABS:0"), "confirming discards the draft:\n{html}");
    assert!(
        html.contains("PENDING:NONE"),
        "the confirm clears once applied:\n{html}"
    );
}

fn cancel_keeps_the_pending_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_close_tab(0);
        nav.cancel_close();
    });
    probe(&nav)
}

#[test]
fn cancelling_keeps_the_pending_draft_open() {
    let html = render(cancel_keeps_the_pending_draft);
    assert!(html.contains("TABS:1"), "cancelling keeps the draft tab open:\n{html}");
    assert!(
        html.contains("PENDING:NONE"),
        "the confirm clears without closing anything:\n{html}"
    );
}

fn quit_with_no_draft_quits_immediately() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.request_quit());
    probe(&nav)
}

#[test]
fn quitting_with_nothing_unsaved_quits_immediately() {
    let html = render(quit_with_no_draft_quits_immediately);
    assert!(html.contains("QUIT:1"), "quit fires with nothing to lose:\n{html}");
    assert!(
        html.contains("PENDING:NONE"),
        "no confirm needed with nothing unsaved:\n{html}"
    );
}

fn quit_with_draft_arms_confirm() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_quit();
    });
    probe(&nav)
}

#[test]
fn quitting_with_an_open_draft_arms_the_confirm() {
    let html = render(quit_with_draft_arms_confirm);
    assert!(html.contains("QUIT:0"), "quit does not fire until confirmed:\n{html}");
    assert!(html.contains("PENDING:SOME"), "the confirm dialog is armed:\n{html}");
}

fn quit_confirmed_after_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_quit();
        nav.confirm_close();
    });
    probe(&nav)
}

#[test]
fn confirming_a_pending_quit_fires_it() {
    let html = render(quit_confirmed_after_draft);
    assert!(html.contains("QUIT:1"), "confirming the quit fires it:\n{html}");
    assert!(
        html.contains("TABS:1"),
        "the draft tab itself is untouched by a quit:\n{html}"
    );
}

// ---- The WM-initiated close (issue #281) --------------------------------------------------------

/// A probe for [`NavState::request_window_close`], rendering the verdict it returned beside the usual
/// marker block: `BLOCKED:true` means the caller must stop the native close, `false` that dioxus's own
/// close may proceed.
fn window_close_probe(nav: &NavState, blocked: Signal<bool>) -> Element {
    rsx! {
        div { "BLOCKED:{blocked}" }
        {probe(nav)}
    }
}

fn window_close_with_nothing_unsaved() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let mut blocked = use_signal(|| false);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        blocked.set(nav.request_window_close());
    });
    window_close_probe(&nav, blocked)
}

#[test]
fn a_window_close_with_nothing_unsaved_is_let_through() {
    // Nothing to lose, so the native close proceeds — and the quit ticket stays untouched, because it
    // is dioxus's own `handle_close_requested` that closes the window, not `QuitManager`.
    let html = render(window_close_with_nothing_unsaved);
    assert!(html.contains("BLOCKED:false"), "the native close is allowed:\n{html}");
    assert!(html.contains("PENDING:NONE"), "no confirm armed:\n{html}");
    assert!(
        html.contains("QUIT:0"),
        "no quit ticket is spent on a close dioxus performs itself:\n{html}"
    );
}

fn window_close_with_an_open_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let mut blocked = use_signal(|| false);
    use_hook(move || {
        nav.open_create(Category::People);
        blocked.set(nav.request_window_close());
    });
    window_close_probe(&nav, blocked)
}

#[test]
fn a_window_close_over_an_open_draft_is_blocked_and_confirms() {
    let html = render(window_close_with_an_open_draft);
    assert!(
        html.contains("BLOCKED:true"),
        "the caller must stop the native close:\n{html}"
    );
    assert!(html.contains("PENDING:SOME"), "the confirm dialog is armed:\n{html}");
    assert!(
        html.contains("QUIT:0"),
        "nothing quits until the operator answers:\n{html}"
    );
    assert!(html.contains("TABS:1"), "the draft is untouched:\n{html}");
}

fn window_close_with_a_parked_edit() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let mut blocked = use_signal(|| false);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        blocked.set(nav.request_window_close());
    });
    window_close_probe(&nav, blocked)
}

#[test]
fn a_window_close_over_a_parked_edit_is_blocked_and_confirms() {
    let html = render(window_close_with_a_parked_edit);
    assert!(
        html.contains("BLOCKED:true"),
        "an in-progress edit of a stored record blocks the close too:\n{html}"
    );
    assert!(html.contains("PENDING:SOME"), "the confirm dialog is armed:\n{html}");
    assert!(
        html.contains("DIRTY:[people/I0001]"),
        "the edit is still parked, not discarded:\n{html}"
    );
}

fn window_close_confirmed() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let mut blocked = use_signal(|| false);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        blocked.set(nav.request_window_close());
        nav.confirm_close();
    });
    window_close_probe(&nav, blocked)
}

#[test]
fn confirming_a_blocked_window_close_quits() {
    // The window was only hidden, so it is `QuitManager` that has to finish the job.
    let html = render(window_close_confirmed);
    assert!(html.contains("QUIT:1"), "Discard all fires the quit:\n{html}");
}

fn window_close_cancelled() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let mut blocked = use_signal(|| false);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        blocked.set(nav.request_window_close());
        nav.cancel_close();
    });
    window_close_probe(&nav, blocked)
}

#[test]
fn cancelling_a_blocked_window_close_leaves_the_app_as_it_was() {
    let html = render(window_close_cancelled);
    assert!(html.contains("PENDING:NONE"), "the confirm clears:\n{html}");
    assert!(html.contains("QUIT:0"), "and nothing quits:\n{html}");
    assert!(html.contains("TABS:1"), "the tab stays open:\n{html}");
    assert!(
        html.contains("DIRTY:[people/I0001]"),
        "with its unsaved edit intact:\n{html}"
    );
}

fn close_dirty_saved_tab_arms_confirm() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_close_tab(0);
    });
    probe(&nav)
}

#[test]
fn closing_a_dirty_saved_tab_arms_the_confirm_instead_of_discarding_the_edit() {
    let html = render(close_dirty_saved_tab_arms_confirm);
    assert!(
        html.contains("TABS:1"),
        "the record with an unsaved edit stays open until confirmed:\n{html}"
    );
    assert!(
        html.contains("PENDING:SOME"),
        "an in-progress edit of a saved record arms the confirm:\n{html}"
    );
}

fn quit_with_dirty_saved_tab_arms_confirm() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_quit();
    });
    probe(&nav)
}

#[test]
fn quitting_with_a_dirty_saved_record_arms_the_confirm() {
    let html = render(quit_with_dirty_saved_tab_arms_confirm);
    assert!(
        html.contains("QUIT:0"),
        "quit does not fire while a saved record has an unsaved edit:\n{html}"
    );
    assert!(html.contains("PENDING:SOME"), "the confirm dialog is armed:\n{html}");
}

fn cancel_keeps_the_dirty_saved_tab() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_close_tab(0);
        nav.cancel_close();
    });
    probe(&nav)
}

#[test]
fn cancelling_keeps_the_dirty_tab_open_and_still_dirty() {
    let html = render(cancel_keeps_the_dirty_saved_tab);
    assert!(html.contains("TABS:1"), "cancelling keeps the tab open:\n{html}");
    assert!(html.contains("PENDING:NONE"), "the confirm clears:\n{html}");
    assert!(
        html.contains("DIRTY:[people/I0001]"),
        "the edit is still pending, so the next ⌘W confirms again:\n{html}"
    );
}

fn confirm_closes_the_dirty_saved_tab() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_close_tab(0);
        nav.confirm_close();
    });
    probe(&nav)
}

#[test]
fn confirming_closes_the_dirty_tab_and_drops_its_dirty_key() {
    let html = render(confirm_closes_the_dirty_saved_tab);
    assert!(html.contains("TABS:0"), "confirming discards the edit:\n{html}");
    assert!(
        html.contains("DIRTY:[]"),
        "the closed record leaves no stale dirty key behind:\n{html}"
    );
}

fn close_clean_saved_tab_beside_a_dirty_one() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        mark_dirty(&mut nav, Category::People, "I0002");
        nav.request_close_tab(0);
    });
    probe(&nav)
}

#[test]
fn a_clean_tab_closes_immediately_even_while_a_sibling_is_dirty() {
    let html = render(close_clean_saved_tab_beside_a_dirty_one);
    assert!(html.contains("TABS:1"), "the clean tab closes at once:\n{html}");
    assert!(
        html.contains("PENDING:NONE"),
        "dirtiness is per record, not shell-wide:\n{html}"
    );
    assert!(
        html.contains("DIRTY:[people/I0002]"),
        "closing a neighbour leaves the dirty record's key intact:\n{html}"
    );
}

fn quit_with_one_dirty_of_two_tabs() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        mark_dirty(&mut nav, Category::People, "I0002");
        nav.request_quit();
    });
    probe(&nav)
}

#[test]
fn quit_confirms_when_any_open_tab_is_dirty() {
    let html = render(quit_with_one_dirty_of_two_tabs);
    assert!(html.contains("QUIT:0"), "quit waits on the confirm:\n{html}");
    assert!(
        html.contains("PENDING:SOME"),
        "one dirty tab among many still arms the confirm:\n{html}"
    );
}

fn rename_rekeys_the_dirty_record() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.rename_record(Category::People, "I0001", "I0099".to_owned());
        nav.request_close_tab(0);
    });
    probe(&nav)
}

#[test]
fn renaming_a_dirty_record_follows_it_to_the_new_id() {
    let html = render(rename_rekeys_the_dirty_record);
    assert!(
        html.contains("DIRTY:[people/I0099]"),
        "the dirty key follows the record to its new id:\n{html}"
    );
    assert!(
        html.contains("PENDING:SOME"),
        "the confirm still fires under the new id:\n{html}"
    );
    assert!(html.contains("TABS:1"), "nothing closed:\n{html}");
}

fn dropping_the_parked_edit_makes_the_tab_closable() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.drop_edit(&EditKey::saved(Category::People, "I0001"));
        nav.request_close_tab(0);
    });
    probe(&nav)
}

#[test]
fn clearing_the_dirty_mark_restores_the_immediate_close() {
    // A save reseeds the draft and Cancel restores it; either way `use_record_edit`'s write-through
    // drops the parked edit, and the tab must go back to closing with no prompt.
    let html = render(dropping_the_parked_edit_makes_the_tab_closable);
    assert!(html.contains("TABS:0"), "a clean record closes immediately:\n{html}");
    assert!(html.contains("PENDING:NONE"), "no confirm for clean state:\n{html}");
}

// ---- Save from the confirm (issue #240) ----------------------------------------------------------

fn save_then_close_arms_the_run() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        mark_dirty(&mut nav, Category::People, "I0002");
        nav.activate_record(0);
        nav.request_close_tab(1);
        nav.save_then_close(1);
    });
    probe(&nav)
}

#[test]
fn saving_from_the_confirm_arms_the_record_and_leaves_its_tab_open() {
    let html = render(save_then_close_arms_the_run);
    assert!(
        html.contains("SAVING:people/I0002"),
        "the record's save is armed:\n{html}"
    );
    assert!(html.contains("QUEUE:[]"), "one tab is the whole run:\n{html}");
    assert!(
        html.contains("ACTIVE:1"),
        "the tab is activated so its pane mounts and can save:\n{html}"
    );
    assert!(html.contains("PENDING:NONE"), "the confirm is dismissed:\n{html}");
    assert!(
        html.contains("TABS:2"),
        "nothing closes until the save reports back:\n{html}"
    );
    assert!(
        html.contains("DIRTY:[people/I0002]"),
        "the edit stays parked until it is saved:\n{html}"
    );
}

fn finished_save_applies_the_pending_tab_close() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_close_tab(0);
        nav.save_then_close(0);
        finish_armed(&mut nav, true);
    });
    probe(&nav)
}

#[test]
fn a_finished_save_closes_the_tab_the_confirm_was_holding() {
    let html = render(finished_save_applies_the_pending_tab_close);
    assert!(html.contains("TABS:0"), "the saved tab closes:\n{html}");
    assert!(html.contains("DIRTY:[]"), "and its edit is spent, not parked:\n{html}");
    assert!(html.contains("SAVING:NONE"), "the run is over:\n{html}");
    assert!(html.contains("QUIT:0"), "closing a tab is not a quit:\n{html}");
}

fn finished_save_of_a_quit_run_quits() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_quit();
        nav.save_all_then_quit();
        finish_armed(&mut nav, true);
    });
    probe(&nav)
}

#[test]
fn the_last_finished_save_of_a_quit_run_fires_the_quit() {
    let html = render(finished_save_of_a_quit_run_quits);
    assert!(
        html.contains("QUIT:1"),
        "the quit fires once the work is saved:\n{html}"
    );
    assert!(html.contains("TABS:1"), "a quit closes no tabs itself:\n{html}");
    assert!(html.contains("SAVING:NONE"), "the run is over:\n{html}");
}

fn failed_save_aborts_the_run() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty(&mut nav, Category::People, "I0002");
        nav.request_quit();
        nav.save_all_then_quit();
        finish_armed(&mut nav, false);
    });
    probe(&nav)
}

#[test]
fn a_failed_save_aborts_the_run_and_leaves_every_tab_open() {
    // The screen has already toasted the error; the shell's job is to stop, not to quit over it.
    let html = render(failed_save_aborts_the_run);
    assert!(html.contains("TABS:2"), "nothing closed:\n{html}");
    assert!(html.contains("QUIT:0"), "and nothing quit:\n{html}");
    assert!(
        html.contains("DIRTY:[people/I0001,people/I0002]"),
        "both parked edits are intact:\n{html}"
    );
    assert!(html.contains("SAVING:NONE"), "the armed save is cleared:\n{html}");
    assert!(html.contains("QUEUE:[]"), "and so is the rest of the queue:\n{html}");
}

fn save_all_walks_the_dirty_records() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let mut trace = use_signal(String::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        nav.open_record(record("I0003", "Cy"));
        mark_dirty(&mut nav, Category::People, "I0003");
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty(&mut nav, Category::People, "I0002");
        nav.request_quit();
        nav.save_all_then_quit();
        let mut steps = Vec::new();
        for _ in 0..3 {
            let armed = nav
                .save_request
                .peek()
                .as_ref()
                .map_or_else(|| "NONE".to_owned(), |request| key_id(&request.key));
            let active = nav
                .active_record
                .peek()
                .map_or_else(|| "-".to_owned(), |index| index.to_string());
            steps.push(format!("{armed}@{active};"));
            finish_armed(&mut nav, true);
        }
        trace.set(steps.concat());
    });
    rsx! {
        div { "TRACE:{trace}" }
        {probe(&nav)}
    }
}

#[test]
fn save_all_walks_every_dirty_record_in_strip_order_and_quits_last() {
    let html = render(save_all_walks_the_dirty_records);
    assert!(
        html.contains("TRACE:people/I0001@0;people/I0002@1;people/I0003@2;"),
        "one record is armed at a time, in strip order, each activated first:\n{html}"
    );
    assert!(html.contains("QUIT:1"), "the quit waits for the last save:\n{html}");
    assert!(html.contains("DIRTY:[]"), "every edit was saved:\n{html}");
    assert!(html.contains("QUEUE:[]"), "the queue is drained:\n{html}");
}

fn cancel_clears_an_armed_save_run() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty(&mut nav, Category::People, "I0002");
        nav.request_quit();
        nav.save_all_then_quit();
        nav.cancel_close();
    });
    probe(&nav)
}

#[test]
fn cancelling_clears_the_armed_save_and_its_queue() {
    let html = render(cancel_clears_an_armed_save_run);
    assert!(html.contains("SAVING:NONE"), "no save stays armed:\n{html}");
    assert!(html.contains("QUEUE:[]"), "and nothing is left queued:\n{html}");
    assert!(html.contains("TABS:2"), "both tabs stay open:\n{html}");
}

fn save_then_close_a_create_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let draft = nav.open_create(Category::People);
        nav.stash_edit(
            EditKey::draft(Category::People, draft),
            StashedEdit::new(edited(), TagDraft::new(), ProvenanceDraft::default()),
        );
        nav.request_close_tab(0);
        nav.save_then_close(0);
    });
    rsx! {
        div { "ARMED:{key_id_of_armed(&nav)}" }
        {probe(&nav)}
    }
}

/// The armed editor's key, or `NONE` — read outside [`probe`] so a test can assert on it alone.
fn key_id_of_armed(nav: &NavState) -> String {
    nav.save_request
        .read()
        .as_ref()
        .map_or_else(|| "NONE".to_owned(), |request| key_id(&request.key))
}

#[test]
fn a_create_draft_is_queued_under_its_own_draft_id() {
    let html = render(save_then_close_a_create_draft);
    assert!(
        html.contains("ARMED:people/#1"),
        "a create draft is armed under its own draft id:\n{html}"
    );
    assert!(html.contains("TABS:1"), "the draft tab is still open:\n{html}");
}

fn create_draft_saved_then_closed() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let draft = nav.open_create(Category::People);
        nav.stash_edit(
            EditKey::draft(Category::People, draft),
            StashedEdit::new(edited(), TagDraft::new(), ProvenanceDraft::default()),
        );
        nav.request_close_tab(0);
        nav.save_then_close(0);
        // What the create screen does on a successful commit: the draft becomes a stored record in the
        // same slot, and the save reports back under the draft's key.
        nav.commit_draft(draft, record("I0001", "Ada"));
        nav.note_save_finished(&EditKey::saved(Category::People, "I0001"), true);
    });
    probe(&nav)
}

#[test]
fn a_saved_create_draft_closes_the_tab_the_confirm_was_holding() {
    // `commit_draft` swaps the stored record into the draft's slot, so the close has to follow the tab
    // rather than trust the index it armed with.
    let html = render(create_draft_saved_then_closed);
    assert!(html.contains("TABS:0"), "the committed record's tab closes:\n{html}");
    assert!(html.contains("DIRTY:[]"), "the create buffer is spent:\n{html}");
    assert!(html.contains("SAVING:NONE"), "the run is over:\n{html}");
}

fn renamed_record_finishes_its_save() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_close_tab(0);
        nav.save_then_close(0);
        // A whole-record save that changed the human id re-keys the tab; the screen reports back under
        // the id the record now has.
        nav.rename_record(Category::People, "I0001", "I0099".to_owned());
        nav.note_save_finished(&EditKey::saved(Category::People, "I0099"), true);
    });
    probe(&nav)
}

#[test]
fn a_save_that_renames_the_record_still_finishes_its_run() {
    let html = render(renamed_record_finishes_its_save);
    assert!(html.contains("TABS:0"), "the renamed record's tab closes:\n{html}");
    assert!(html.contains("SAVING:NONE"), "the run does not hang:\n{html}");
}

fn savable_flags_for_valid_and_invalid_edits() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        nav.open_create(Category::Tags);
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty_invalid(&mut nav, Category::People, "I0002");
    });
    let valid = nav.tab_is_savable(0);
    let invalid = nav.tab_is_savable(1);
    let untouched_draft = nav.tab_is_savable(2);
    rsx! {
        div { "SAVABLE:{valid}/{invalid}/{untouched_draft}" }
    }
}

#[test]
fn only_a_parked_valid_edit_is_savable_from_the_confirm() {
    let html = render(savable_flags_for_valid_and_invalid_edits);
    assert!(
        html.contains("SAVABLE:true/false/false"),
        "an invalid edit and a draft tab with nothing typed offer no Save:\n{html}"
    );
}

/// The confirm dialog, forced open for a pending draft-tab close.
fn dialog_open_for_draft() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_close_tab(0);
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn dialog_renders_the_close_tab_confirm_for_a_draft() {
    let html = render(dialog_open_for_draft);
    assert!(html.contains(r#"role="dialog""#), "confirm dialog role:\n{html}");
    assert!(html.contains("Close tab?"), "close-tab confirm title:\n{html}");
    assert!(html.contains("Cancel"), "cancel action label:\n{html}");
}

/// The confirm dialog, forced open for a pending quit.
fn dialog_open_for_quit() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_quit();
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn dialog_renders_the_quit_confirm() {
    let html = render(dialog_open_for_quit);
    assert!(html.contains("Quit?"), "quit confirm title:\n{html}");
}

/// The confirm dialog, forced open for a pending close of a saved record with an unsaved edit.
fn dialog_open_for_dirty_saved_record() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_close_tab(0);
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn dialog_body_describes_unsaved_edits_not_a_draft() {
    let html = render(dialog_open_for_dirty_saved_record);
    assert!(
        html.contains("Ada"),
        "the body names the record, not the entity type:\n{html}"
    );
    assert!(
        html.contains("unsaved changes"),
        "the body says the edits are unsaved, not that the record is:\n{html}"
    );
    assert!(
        !html.contains("hasn't been saved yet"),
        "the draft-only copy would be untrue for a stored record:\n{html}"
    );
    assert!(
        !html.contains("New People"),
        "a saved record is never labelled as a new draft:\n{html}"
    );
}

/// The confirm dialog, forced open for a quit whose only unsaved work is an edit of a saved record.
fn dialog_open_for_quit_with_dirty_edit() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_quit();
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn quit_dialog_body_describes_unsaved_edits_when_no_draft_is_open() {
    let html = render(dialog_open_for_quit_with_dirty_edit);
    assert!(html.contains("Quit?"), "quit confirm title:\n{html}");
    assert!(
        html.contains("unsaved changes"),
        "quitting over an edited record says the changes are unsaved:\n{html}"
    );
    assert!(
        !html.contains("haven't been saved yet"),
        "the draft-only quit copy would be untrue here:\n{html}"
    );
}

/// The close-tab confirm over a saved record whose parked edit is valid — Save is on offer.
fn dialog_with_savable_edit() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_close_tab(0);
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn the_close_tab_dialog_offers_save_discard_and_cancel() {
    let html = render(dialog_with_savable_edit);
    assert!(
        html.contains("Save"),
        "Save keeps the work rather than losing it:\n{html}"
    );
    assert!(
        html.contains("Discard changes"),
        "Discard names what it throws away:\n{html}"
    );
    assert!(html.contains("Cancel"), "Cancel backs out:\n{html}");
    assert!(
        !html.contains("disabled"),
        "a valid edit's Save is live, not dead:\n{html}"
    );
}

/// The close-tab confirm over a saved record whose parked edit is invalid — Save cannot run.
fn dialog_with_invalid_edit() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty_invalid(&mut nav, Category::People, "I0001");
        nav.request_close_tab(0);
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn an_invalid_edit_disables_save_and_says_why() {
    let html = render(dialog_with_invalid_edit);
    assert!(
        html.contains("disabled"),
        "Save is disabled, not silently dead:\n{html}"
    );
    assert!(
        // The apostrophe in "can't" renders HTML-escaped, so the assertion stops short of it.
        html.contains("is missing required fields"),
        "the body gives the reason:\n{html}"
    );
    assert!(
        html.contains("Discard changes"),
        "discarding is still available:\n{html}"
    );
}

/// The close-tab confirm over a draft tab with nothing typed into it — there is no buffer to save.
fn dialog_for_untouched_draft() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_close_tab(0);
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn a_draft_with_nothing_typed_has_nothing_to_save() {
    let html = render(dialog_for_untouched_draft);
    assert!(html.contains("disabled"), "Save is disabled:\n{html}");
    assert!(
        html.contains("nothing to save"),
        "the body says why rather than leaving a dead button:\n{html}"
    );
    assert!(html.contains("Discard draft"), "a draft is discarded whole:\n{html}");
}

/// The quit confirm with three dirty records open: two stored, one draft.
fn quit_dialog_over_three_dirty_records() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        nav.open_record(record("I0003", "Cy"));
        let draft = nav.open_create(Category::Tags);
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty(&mut nav, Category::People, "I0003");
        nav.stash_edit(
            EditKey::draft(Category::Tags, draft),
            StashedEdit::new(edited(), TagDraft::new(), ProvenanceDraft::default()),
        );
        nav.request_quit();
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn the_quit_dialog_lists_every_record_with_unsaved_work() {
    let html = render(quit_dialog_over_three_dirty_records);
    assert!(html.contains("Ada"), "the first dirty record is named:\n{html}");
    assert!(html.contains("Cy"), "and so is the third:\n{html}");
    assert!(
        html.contains("<li>edited</li>"),
        "the draft is named as the tabstrip names it — by what was typed into it:\n{html}"
    );
    assert!(
        !html.contains("Bob"),
        "the clean record is not at stake, so it is not listed:\n{html}"
    );
    assert!(html.contains("Save all"), "Save all keeps every one of them:\n{html}");
    assert!(
        html.contains("Discard all"),
        "Discard all is the losing option:\n{html}"
    );
    assert!(html.contains("Cancel"), "Cancel backs out:\n{html}");
}

/// The quit confirm where one of the dirty records cannot be saved and the other can.
fn quit_dialog_with_one_invalid_record() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty_invalid(&mut nav, Category::People, "I0002");
        nav.request_quit();
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn save_all_runs_when_any_unsaved_record_is_savable() {
    // Ada's edit is valid; Bob's is not. Save all keeps Ada's work and leaves Bob open, which is worth
    // offering — disabling it over Bob would make Ada's only outcomes Discard all or Cancel.
    let html = render(quit_dialog_with_one_invalid_record);
    assert!(
        !button_markup(&html, "Save all").contains("disabled"),
        "Save all runs for the records it can save:\n{html}"
    );
    assert!(
        html.contains("<li>Bob — can"),
        "the list marks the record that will be left open:\n{html}"
    );
    assert!(
        html.contains("<li>Ada</li>"),
        "the savable record is listed plainly:\n{html}"
    );
    assert!(
        html.contains("the rest are left open"),
        "the body says what Save all does with the records it cannot save:\n{html}"
    );
}

/// The quit confirm where nothing unsaved can be saved: an invalid edit beside an untouched draft.
fn quit_dialog_with_nothing_savable() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0002", "Bob"));
        nav.open_create(Category::People);
        mark_dirty_invalid(&mut nav, Category::People, "I0002");
        nav.request_quit();
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn save_all_is_disabled_when_no_unsaved_record_is_savable() {
    let html = render(quit_dialog_with_nothing_savable);
    assert!(
        button_markup(&html, "Save all").contains("disabled"),
        "with nothing savable Save all is dead, not silently inert:\n{html}"
    );
    assert!(
        html.contains("&#34;Bob&#34; is missing required fields"),
        "the body names the record standing in the way:\n{html}"
    );
}

/// The quit confirm from issue #261: a valid edit of a stored record beside an untouched `⌘N` draft.
fn quit_dialog_with_a_valid_record_and_an_untouched_draft() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_create(Category::People);
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_quit();
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn an_untouched_draft_does_not_block_save_all_for_a_valid_record() {
    // A `⌘N` draft is unsaved by definition and savable never, so gating on it would mean no ⌘Q over an
    // open draft could ever save anything.
    let html = render(quit_dialog_with_a_valid_record_and_an_untouched_draft);
    assert!(
        !button_markup(&html, "Save all").contains("disabled"),
        "an untouched draft does not speak for the records beside it:\n{html}"
    );
    assert!(
        html.contains("<li>New People — can"),
        "the draft is the entry marked as staying open:\n{html}"
    );
    assert!(
        html.contains("<li>Ada</li>"),
        "the valid record is listed plainly:\n{html}"
    );
}

fn save_all_over_a_mixed_strip() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_create(Category::Tags);
        nav.open_record(record("I0003", "Cecil"));
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty(&mut nav, Category::People, "I0003");
        nav.request_quit();
        nav.save_all_then_quit();
    });
    probe(&nav)
}

#[test]
fn save_all_queues_only_the_savable_records() {
    // Queueing by "unsaved" instead would put the untouched draft in the run, where it fails its own
    // `can_save()` gate and aborts the run with Ada already saved.
    let html = render(save_all_over_a_mixed_strip);
    assert!(
        html.contains("SAVING:people/I0001"),
        "the first savable record in strip order is armed:\n{html}"
    );
    assert!(
        html.contains("QUEUE:[people/I0003]"),
        "the untouched draft is not part of the run:\n{html}"
    );
}

fn partial_save_all_run() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        let draft = nav.open_create(Category::Tags);
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.stash_edit(
            EditKey::draft(Category::Tags, draft),
            StashedEdit::new(TagDraft::new(), TagDraft::new(), ProvenanceDraft::default()),
        );
        nav.request_quit();
        nav.save_all_then_quit();
        finish_armed(&mut nav, true);
    });
    probe(&nav)
}

#[test]
fn a_partial_save_all_leaves_the_unsavable_tab_open_without_quitting() {
    // The run covered what it could; the record it could not save keeps its work, on screen, in a
    // running app — the one thing that must never happen is losing it without a Discard all.
    let html = render(partial_save_all_run);
    assert!(
        html.contains("QUIT:0"),
        "a run that could not cover everything does not quit:\n{html}"
    );
    assert!(
        html.contains("TABS:2"),
        "the unsavable record's tab stays open:\n{html}"
    );
    assert!(
        html.contains("DIRTY:[tags/#1]"),
        "its work is still parked, and only Ada's was spent:\n{html}"
    );
    assert!(html.contains("SAVING:NONE"), "the run is over:\n{html}");
    assert!(html.contains("QUEUE:[]"), "with nothing left queued:\n{html}");
}

/// A stand-in for an aggregate's detail pane: the shared edit buffer plus the save-on-request wiring,
/// counting how often the screen's own save closure is called.
#[component]
fn SavePane(human_id: String) -> Element {
    let state = use_record_edit::<TagDraft>(Category::Tags, &human_id, &TagDraft::new());
    let mut saves = use_signal(|| 0_u32);
    let on_save = use_callback(move |()| saves += 1);
    use_save_on_request(EditKey::saved(Category::Tags, &human_id), state, on_save);
    rsx! {
        div { "SAVES:{saves}" }
    }
}

fn pane_saves_on_an_armed_request() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(RecordRef {
            category: Category::Tags,
            human_id: "T0001".to_owned(),
            label: "Ada".to_owned(),
        });
        nav.stash_edit(
            EditKey::saved(Category::Tags, "T0001"),
            StashedEdit::new(edited(), TagDraft::new(), ProvenanceDraft::default()),
        );
        nav.request_close_tab(0);
        nav.save_then_close(0);
    });
    rsx! {
        SavePane { human_id: "T0001" }
    }
}

#[test]
fn an_armed_save_runs_the_records_own_save_closure_exactly_once() {
    // The pane is where the save lives, so the shell arming a request has to reach it — once, however
    // many render passes the effect cascade takes.
    let html = render_settled(pane_saves_on_an_armed_request);
    assert!(
        html.contains("SAVES:1"),
        "the armed request runs the pane's save exactly once:\n{html}"
    );
}

fn pane_ignores_another_records_save() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(RecordRef {
            category: Category::Tags,
            human_id: "T0001".to_owned(),
            label: "Ada".to_owned(),
        });
        nav.stash_edit(
            EditKey::saved(Category::Tags, "T0001"),
            StashedEdit::new(edited(), TagDraft::new(), ProvenanceDraft::default()),
        );
        nav.stash_edit(
            EditKey::saved(Category::Tags, "T0002"),
            StashedEdit::new(edited(), TagDraft::new(), ProvenanceDraft::default()),
        );
        nav.request_close_tab(0);
        nav.save_then_close(0);
        // Re-arm for a record this pane is not showing.
        nav.save_request.set(Some(SaveRequest {
            key: EditKey::saved(Category::Tags, "T0002"),
            then: SaveThen::Quit,
        }));
    });
    rsx! {
        SavePane { human_id: "T0001" }
    }
}

#[test]
fn a_pane_only_saves_when_the_request_names_its_own_record() {
    let html = render_settled(pane_ignores_another_records_save);
    assert!(
        html.contains("SAVES:0"),
        "another record's save request is not this pane's to run:\n{html}"
    );
}

// ---- Esc, the click-away scrim, and re-opening (issue #201) -------------------------------------

/// The marker block for the overlay/confirm interaction: which overlay is showing beside the open-tab
/// count and whether the confirm is armed.
fn overlay_probe(nav: &NavState) -> Element {
    let overlay = match *nav.overlay.read() {
        Overlay::None => "NONE",
        Overlay::Palette => "PALETTE",
        Overlay::Help => "HELP",
        Overlay::NewRecord => "NEW_RECORD",
    };
    rsx! {
        div { "OVERLAY:{overlay}" }
        {probe(nav)}
    }
}

fn escape_with_a_pending_close() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_close_tab(0);
        nav.dismiss_topmost();
    });
    overlay_probe(&nav)
}

#[test]
fn escape_cancels_a_pending_close_without_discarding_the_tab() {
    let html = render(escape_with_a_pending_close);
    assert!(html.contains("PENDING:NONE"), "Esc dismisses the confirm:\n{html}");
    assert!(
        html.contains("TABS:1"),
        "Esc takes the Cancel path, so the tab stays open:\n{html}"
    );
    assert!(
        html.contains("DIRTY:[people/I0001]"),
        "and its unsaved edit is untouched:\n{html}"
    );
}

fn escape_with_a_pending_quit() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_quit();
        nav.dismiss_topmost();
    });
    overlay_probe(&nav)
}

#[test]
fn escape_cancels_a_pending_quit_without_quitting() {
    let html = render(escape_with_a_pending_quit);
    assert!(html.contains("PENDING:NONE"), "Esc dismisses the quit confirm:\n{html}");
    assert!(html.contains("QUIT:0"), "and does not fire the quit:\n{html}");
    assert!(html.contains("TABS:1"), "the draft survives:\n{html}");
}

fn escape_with_a_pending_close_abandons_the_save_run() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_quit();
        nav.save_all_then_quit();
        nav.request_quit();
        nav.dismiss_topmost();
    });
    overlay_probe(&nav)
}

#[test]
fn escape_on_the_confirm_abandons_an_armed_save_run() {
    // Esc must be the Cancel path in full, not just a `pending_close` clear.
    let html = render(escape_with_a_pending_close_abandons_the_save_run);
    assert!(html.contains("SAVING:NONE"), "no save stays armed:\n{html}");
    assert!(html.contains("QUEUE:[]"), "and nothing is left queued:\n{html}");
}

fn escape_with_a_pending_close_leaves_the_overlay_alone() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.overlay.set(Overlay::Help);
        nav.request_close_tab(0);
        nav.dismiss_topmost();
    });
    overlay_probe(&nav)
}

#[test]
fn escape_resolves_the_confirm_before_any_open_overlay() {
    // The confirm is the topmost dialog, so one Esc dismisses it and leaves the sheet behind it.
    let html = render(escape_with_a_pending_close_leaves_the_overlay_alone);
    assert!(html.contains("PENDING:NONE"), "the confirm is dismissed:\n{html}");
    assert!(
        html.contains("OVERLAY:HELP"),
        "one Esc closes one thing, the topmost:\n{html}"
    );
}

fn escape_with_no_pending_close() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.overlay.set(Overlay::Palette);
        nav.dismiss_topmost();
    });
    overlay_probe(&nav)
}

#[test]
fn escape_still_closes_an_overlay_when_nothing_is_pending() {
    let html = render(escape_with_no_pending_close);
    assert!(
        html.contains("OVERLAY:NONE"),
        "Esc closes the palette as before:\n{html}"
    );
}

fn close_requested_again_after_a_cancel() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        nav.request_close_tab(0);
        nav.dismiss_topmost();
        nav.request_close_tab(0);
    });
    overlay_probe(&nav)
}

#[test]
fn the_confirm_re_arms_after_an_escape_cancel() {
    let html = render(close_requested_again_after_a_cancel);
    assert!(
        html.contains("PENDING:SOME"),
        "a cancelled confirm can be re-opened:\n{html}"
    );
    assert!(html.contains("TABS:1"), "nothing closed in between:\n{html}");
}

#[test]
fn the_confirm_dialog_renders_a_click_away_scrim_and_focus_guards() {
    let html = render(dialog_open_for_draft);
    assert!(
        html.contains(r#"class="modal-scrim""#),
        "clicking outside the confirm cancels it:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Dismiss""#),
        "the scrim's accessible name is localized chrome, not a literal:\n{html}"
    );
    assert!(
        html.contains(r#"data-focus-trap="true""#),
        "the confirm is a trapped dialog:\n{html}"
    );
    assert_eq!(
        html.matches("data-focus-guard").count(),
        2,
        "the three-button footer is bracketed by both guards:\n{html}"
    );
}

/// The confirm dialog with nothing pending.
fn dialog_closed() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    use_context_provider(NavState::new);
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn dialog_renders_nothing_when_not_pending() {
    let html = render(dialog_closed);
    assert!(html.trim().is_empty(), "no pending close renders nothing:\n{html}");
}

// ---- A save run over several drafts of one category (issue #260) ----------------------------------

fn save_all_over_two_drafts_of_one_category() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let first = nav.open_create(Category::People);
        let second = nav.open_create(Category::People);
        mark_draft_dirty(&mut nav, first);
        mark_draft_dirty(&mut nav, second);
        nav.request_quit();
        nav.save_all_then_quit();
        commit_and_report(&mut nav, first, "I0001", "Ada");
        commit_and_report(&mut nav, second, "I0002", "Bess");
    });
    probe(&nav)
}

#[test]
fn save_all_walks_two_drafts_of_one_category() {
    // A committed draft reports back under the id the record it just stored now has, not under the draft
    // key the run armed — so the run only advances if `commit_draft` re-keyed it. Without that the run
    // hangs on the first draft and the quit never fires.
    let html = render(save_all_over_two_drafts_of_one_category);
    assert!(
        html.contains("QUIT:1"),
        "the quit fires once both drafts are stored:\n{html}"
    );
    assert!(html.contains("DIRTY:[]"), "both create buffers are spent:\n{html}");
    assert!(html.contains("QUEUE:[]"), "the queue is drained:\n{html}");
    assert!(html.contains("SAVING:NONE"), "and nothing is left armed:\n{html}");
    assert!(html.contains("TABS:2"), "a quit closes no tabs itself:\n{html}");
}

fn save_then_close_the_second_of_two_drafts() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let first = nav.open_create(Category::People);
        let second = nav.open_create(Category::People);
        mark_draft_dirty(&mut nav, first);
        mark_draft_dirty(&mut nav, second);
        nav.activate_record(0);
        nav.request_close_tab(1);
        nav.save_then_close(1);
    });
    rsx! {
        div { "ARMED:{key_id_of_armed(&nav)}" }
        {probe(&nav)}
    }
}

#[test]
fn save_then_close_arms_the_named_draft_not_the_first() {
    let html = render(save_then_close_the_second_of_two_drafts);
    assert!(
        html.contains("ARMED:people/#2"),
        "the confirm's Save arms the tab it was raised for:\n{html}"
    );
    assert!(
        html.contains("ACTIVE:1"),
        "and reveals that draft's own pane, not its sibling's:\n{html}"
    );
}

fn a_report_from_the_other_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let first = nav.open_create(Category::People);
        let second = nav.open_create(Category::People);
        mark_draft_dirty(&mut nav, first);
        mark_draft_dirty(&mut nav, second);
        nav.request_quit();
        nav.save_all_then_quit();
        // The draft the run is *not* waiting on reports in.
        nav.note_save_finished(&EditKey::draft(Category::People, second), true);
    });
    probe(&nav)
}

#[test]
fn a_report_from_another_draft_does_not_advance_the_run() {
    // Two drafts of one category are two editors, so "a draft of People finished" is not an answer to
    // "did *this* draft finish" — a Save the operator clicked on the other form must not step the run.
    let html = render(a_report_from_the_other_draft);
    assert!(
        html.contains("SAVING:people/#1"),
        "the run stays armed on the draft it asked:\n{html}"
    );
    assert!(
        html.contains("QUEUE:[people/#2]"),
        "with the other still queued behind it:\n{html}"
    );
    assert!(html.contains("QUIT:0"), "and nothing quits:\n{html}");
    assert!(
        html.contains("DIRTY:[people/#1,people/#2]"),
        "neither buffer is spent:\n{html}"
    );
}

fn a_committed_draft_while_another_dirty_tab_is_active() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let draft = nav.open_create(Category::People);
        nav.open_record(record("I0009", "Cy"));
        mark_draft_dirty(&mut nav, draft);
        mark_dirty(&mut nav, Category::People, "I0009");
        nav.request_close_tab(0);
        nav.save_then_close(0);
        nav.commit_draft(draft, record("I0001", "Ada"));
        // The commit is asynchronous, so the operator can bring another tab forward before it lands.
        nav.activate_record(1);
        nav.note_save_finished(&EditKey::saved(Category::People, "I0001"), true);
    });
    probe(&nav)
}

#[test]
fn a_committed_draft_closes_the_tab_it_saved_not_whichever_is_active() {
    // The close has to follow the record the run saved. Guessing "the active tab, if it is a saved record
    // of this category" instead closes Cy's tab here — silently, with his unsaved edit in it.
    let html = render(a_committed_draft_while_another_dirty_tab_is_active);
    assert!(html.contains("TABS:1"), "one tab closed:\n{html}");
    assert!(
        html.contains("DIRTY:[people/I0009]"),
        "and it is the stored draft's tab that went — Cy keeps his tab and his edit:\n{html}"
    );
    assert!(html.contains("SAVING:NONE"), "the run is over:\n{html}");
}

fn a_failed_commit_of_the_first_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let first = nav.open_create(Category::People);
        let second = nav.open_create(Category::People);
        mark_draft_dirty(&mut nav, first);
        mark_draft_dirty(&mut nav, second);
        nav.request_quit();
        nav.save_all_then_quit();
        // A commit that failed stored nothing, so the report names the *draft* — there is no record id to
        // name. This is the asymmetry with the success above (`commit_and_report`), and it is what lets a
        // failed run be recognised at all.
        nav.note_save_finished(&EditKey::draft(Category::People, first), false);
    });
    probe(&nav)
}

#[test]
fn a_failed_draft_commit_abandons_the_run_and_keeps_both_drafts() {
    let html = render(a_failed_commit_of_the_first_draft);
    assert!(html.contains("QUIT:0"), "a failed run does not quit:\n{html}");
    assert!(html.contains("TABS:2"), "and closes nothing:\n{html}");
    assert!(
        html.contains("DIRTY:[people/#1,people/#2]"),
        "both drafts keep their work:\n{html}"
    );
    assert!(html.contains("SAVING:NONE"), "the armed save is cleared:\n{html}");
    assert!(html.contains("QUEUE:[]"), "and so is the rest of the queue:\n{html}");
}

// ---- The confirm names a tab exactly as the strip does (issue #260) -------------------------------

/// The quit confirm over two People drafts, neither typed into.
fn quit_dialog_over_two_empty_drafts() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.open_create(Category::People);
        nav.request_quit();
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn the_quit_dialog_names_two_empty_drafts_apart() {
    // The at-stake list is the operator's only view of what Discard all destroys, so two entries reading
    // the same thing would make the choice unanswerable.
    let html = render(quit_dialog_over_two_empty_drafts);
    assert_eq!(
        html.matches("<li>New People</li>").count(),
        1,
        "one entry is the unnumbered draft:\n{html}"
    );
    assert_eq!(
        html.matches("<li>New People (2)</li>").count(),
        1,
        "and the other carries its ordinal:\n{html}"
    );
}

/// The quit confirm over a draft with a name typed into it.
fn quit_dialog_over_a_named_draft() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let draft = nav.open_create(Category::People);
        name_draft(&mut nav, draft, "Ada Lovelace");
        nav.request_quit();
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn the_quit_dialog_names_a_typed_draft_by_its_name() {
    let html = render(quit_dialog_over_a_named_draft);
    assert!(
        html.contains("<li>Ada Lovelace</li>"),
        "the draft is listed by what was typed into it:\n{html}"
    );
    assert!(
        !html.contains("New People"),
        "and not also by the generic new-record label:\n{html}"
    );
}

/// The close confirm for a named draft's own tab.
fn close_dialog_over_a_named_draft() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let draft = nav.open_create(Category::People);
        name_draft(&mut nav, draft, "Ada Lovelace");
        nav.request_close_tab(0);
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn the_close_confirm_names_a_typed_draft_by_its_name() {
    let html = render(close_dialog_over_a_named_draft);
    assert!(
        html.contains("Ada Lovelace"),
        "the body names the record the operator is about to lose:\n{html}"
    );
    assert!(html.contains("Discard draft"), "and it is still a draft:\n{html}");
}

/// The close confirm for the *second* of two untyped drafts.
fn close_dialog_over_the_second_empty_draft() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.open_create(Category::People);
        nav.request_close_tab(1);
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn the_close_confirm_names_the_second_empty_draft_by_its_ordinal() {
    let html = render(close_dialog_over_the_second_empty_draft);
    assert!(
        html.contains("New People (2)"),
        "the confirm says which of the two drafts it is about:\n{html}"
    );
}

/// Both the record strip and the confirm, over one strip of two untyped drafts.
fn tabstrip_and_dialog_over_two_empty_drafts() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.open_create(Category::People);
        nav.request_quit();
    });
    rsx! {
        RecordTabstrip {}
        CloseConfirmDialog {}
    }
}

#[test]
fn the_strip_and_the_confirm_name_a_tab_identically() {
    // The test that fails if a second tab-naming function ever reappears: the strip and the dialog are
    // rendered over one `NavState`, and each tab has to read the same in both.
    let html = render(tabstrip_and_dialog_over_two_empty_drafts);
    assert_eq!(
        html.matches(r#"aria-label="Close New People""#).count(),
        1,
        "the strip has exactly one unnumbered New People tab:\n{html}"
    );
    assert_eq!(
        html.matches("<li>New People</li>").count(),
        1,
        "and the dialog lists it under the same name:\n{html}"
    );
    assert_eq!(
        html.matches(r#"aria-label="Close New People (2)""#).count(),
        1,
        "the numbered tab likewise:\n{html}"
    );
    assert_eq!(
        html.matches("<li>New People (2)</li>").count(),
        1,
        "and the dialog agrees on its number:\n{html}"
    );
}

// ---- A save run whose target leaves the strip mid-run (issue #302) --------------------------------

/// Seeds the already-localized incomplete-run notice the shell seeds at mount (`NavState` carries no
/// localizer of its own) — every probe below that expects `save_incomplete_notice` to fire has to set
/// this up itself, exactly as `root.rs` does from `ChromeCtx`.
fn seed_incomplete_notice(nav: &mut NavState) {
    nav.save_incomplete_notice.set(Some("incomplete".to_owned()));
}

fn save_all_with_a_queued_tab_closed_mid_run() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        nav.open_record(record("I0003", "Cy"));
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty(&mut nav, Category::People, "I0002");
        mark_dirty(&mut nav, Category::People, "I0003");
        nav.request_quit();
        nav.save_all_then_quit();
        // Ada's save lands, which arms Bob's — Cy's is still only queued behind it.
        finish_armed(&mut nav, true);
        // Cy's tab closes mid-run (the tabstrip ✕, a cancel) and takes his own unsaved work with it.
        nav.close_record(2);
        // Bob's save lands too, draining the queue.
        finish_armed(&mut nav, true);
    });
    probe(&nav)
}

#[test]
fn a_save_run_does_not_hang_when_a_queued_target_closes_mid_run() {
    // The issue's regression case: once every *remaining* target has actually saved the run still
    // quits, rather than waiting forever on a target that can no longer report back.
    let html = render(save_all_with_a_queued_tab_closed_mid_run);
    assert!(html.contains("QUIT:1"), "the run still quits:\n{html}");
    assert!(html.contains("SAVING:NONE"), "and nothing is left armed:\n{html}");
    assert!(html.contains("QUEUE:[]"), "with the queue drained:\n{html}");
    assert!(
        html.contains("TABS:2"),
        "Cy's closed tab is gone, Ada's and Bob's saved ones remain:\n{html}"
    );
    assert!(html.contains("DIRTY:[]"), "every remaining edit was saved:\n{html}");
}

fn save_all_where_the_queued_target_goes_invalid() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty(&mut nav, Category::People, "I0002");
        seed_incomplete_notice(&mut nav);
        nav.request_quit();
        nav.save_all_then_quit();
        // Bob's parked edit turns invalid while Ada's save is still in flight — still open, still
        // dirty, just no longer savable.
        mark_dirty_invalid(&mut nav, Category::People, "I0002");
        finish_armed(&mut nav, true);
    });
    probe(&nav)
}

#[test]
fn a_queued_target_that_stops_being_savable_is_dropped_and_the_run_ends_without_quitting() {
    let html = render(save_all_where_the_queued_target_goes_invalid);
    assert!(
        html.contains("QUIT:0"),
        "Bob's unsaved work remains, so the quit does not fire:\n{html}"
    );
    assert!(html.contains("SAVING:NONE"), "the run is over:\n{html}");
    assert!(html.contains("QUEUE:[]"), "and nothing is left queued:\n{html}");
    assert!(html.contains("TABS:2"), "both tabs stay open:\n{html}");
    assert!(
        html.contains("DIRTY:[people/I0002]"),
        "Bob's now-invalid edit is still parked, unsaved — only Ada's was spent:\n{html}"
    );
    assert!(
        html.contains("NOTICE:incomplete"),
        "the incomplete-run notice is raised instead of hanging:\n{html}"
    );
}

fn closing_the_armed_tab_advances_the_run() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty(&mut nav, Category::People, "I0002");
        nav.request_quit();
        nav.save_all_then_quit();
        // Ada's tab — the one currently armed — closes directly (a cancel, or the tabstrip ✕) before
        // her save ever reports back.
        nav.close_record(0);
    });
    probe(&nav)
}

#[test]
fn closing_the_armed_target_directly_advances_the_run_instead_of_hanging() {
    let html = render(closing_the_armed_tab_advances_the_run);
    assert!(
        html.contains("SAVING:people/I0002"),
        "the run moves on to the next queued target:\n{html}"
    );
    assert!(html.contains("QUEUE:[]"), "which was the last one queued:\n{html}");
    assert!(html.contains("TABS:1"), "Ada's tab is gone:\n{html}");
}

fn note_unsavable_on_the_armed_key() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty(&mut nav, Category::People, "I0002");
        nav.request_quit();
        nav.save_all_then_quit();
        // Ada's own pane found her parked edit no longer passes `can_save()` before attempting a save
        // at all (`use_save_on_request`'s `else` branch).
        nav.note_save_unsavable(&EditKey::saved(Category::People, "I0001"));
    });
    probe(&nav)
}

#[test]
fn note_save_unsavable_drops_the_armed_target_and_advances_the_run() {
    let html = render(note_unsavable_on_the_armed_key);
    assert!(
        html.contains("SAVING:people/I0002"),
        "the run moves on to the next queued target:\n{html}"
    );
    assert!(
        html.contains("TABS:2"),
        "Ada's own tab stays open — she was dropped from the run, not closed:\n{html}"
    );
    assert!(
        html.contains("DIRTY:[people/I0001,people/I0002]"),
        "and her unsaved edit is untouched:\n{html}"
    );
}

fn close_tab_target_goes_unsavable_before_it_saves() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        mark_dirty(&mut nav, Category::People, "I0001");
        seed_incomplete_notice(&mut nav);
        nav.request_close_tab(0);
        nav.save_then_close(0);
        // Ada's own pane found her edit no longer savable before it could save.
        nav.note_save_unsavable(&EditKey::saved(Category::People, "I0001"));
    });
    probe(&nav)
}

#[test]
fn a_close_tab_run_whose_target_goes_unsavable_leaves_the_tab_open_and_notifies() {
    let html = render(close_tab_target_goes_unsavable_before_it_saves);
    assert!(
        html.contains("TABS:1"),
        "the tab stays open — its work was never actually saved:\n{html}"
    );
    assert!(html.contains("SAVING:NONE"), "the run is over:\n{html}");
    assert!(
        html.contains("DIRTY:[people/I0001]"),
        "and its unsaved edit is untouched:\n{html}"
    );
    assert!(
        html.contains("NOTICE:incomplete"),
        "closing would have discarded unsaved work, so the notice fires instead:\n{html}"
    );
}
