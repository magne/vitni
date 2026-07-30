//! SSR-probe assertions for the close-tab/quit confirm flow on [`NavState`] (PR1 §1.4): closing a
//! *clean* saved tab is immediate; closing one that holds unsaved work — a draft, or a saved record
//! with an in-progress edit parked in [`NavState::edit_drafts`] (issue #200) — arms the confirm dialog
//! instead of discarding it silently. Like `dock.rs`, each probe drives `NavState` in `use_hook` and renders a
//! small marker the test inspects.

use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_ui::{Category, ProvenanceDraft, RecordRef, TagDraft};
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::shell::ChromeCtx;
use genealogy_ui_dioxus::shell::close_confirm::CloseConfirmDialog;
use genealogy_ui_dioxus::shell::nav_state::{EditKey, NavState, StashedEdit};
use unic_langid::LanguageIdentifier;

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
    let seed = TagDraft::new();
    let draft = TagDraft {
        name: "edited".to_owned(),
        ..TagDraft::new()
    };
    nav.stash_edit(
        EditKey::saved(category, human_id),
        StashedEdit::new(draft, seed, ProvenanceDraft::default()),
    );
}

/// Renders a probe component to an HTML string.
fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// The marker block: open-tab count, whether the confirm is armed, the quit ticket value, and the
/// keys currently holding a parked in-progress edit (`category/human_id`).
fn probe(nav: &NavState) -> Element {
    let tabs = nav.records.read().len();
    let pending = if nav.pending_close.read().is_some() {
        "SOME"
    } else {
        "NONE"
    };
    let quit = *nav.quit_requested.read();
    let dirty = nav
        .edit_drafts
        .read()
        .keys()
        .map(|key| {
            let id = key.human_id.clone().unwrap_or_else(|| "*".to_owned());
            format!("{}/{id}", key.category.id())
        })
        .collect::<Vec<_>>()
        .join(",");
    rsx! {
        div { "TABS:{tabs}" }
        div { "PENDING:{pending}" }
        div { "QUIT:{quit}" }
        div { "DIRTY:[{dirty}]" }
    }
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
