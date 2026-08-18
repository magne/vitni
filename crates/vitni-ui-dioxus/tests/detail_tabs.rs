//! SSR-probe assertions for per-record tab memory (issue #209): a detail pane's active related-item
//! tab is remembered in [`NavState`] so it survives the pane unmounting (the same "only the active
//! tab's pane is mounted" constraint `edit_stash.rs` documents), and each open record remembers its
//! own tab independently.
//!
//! Plus the shared attached-records table [`attached_table`] the related-item tabs all render through
//! (issue #304): the spine's own contract — the leading link cell, the caller's middle columns, the
//! optional actions cell and the empty state — tested once here rather than per screen.
//!
//! Helpers copied from `tests/edit_stash.rs` (no shared helper module between SSR test binaries).

use dioxus::prelude::*;
use vitni_ui::{Category, Localizer, RecordRef};
use vitni_ui_dioxus::screens::{AttachedLink, AttachedRow, attached_table, create_record_frame, use_detail_tab};
use vitni_ui_dioxus::shell::nav_state::{EditKey, NavState};

fn record(human_id: &str, label: &str) -> RecordRef {
    RecordRef {
        category: Category::Tags,
        human_id: human_id.to_owned(),
        label: label.to_owned(),
    }
}

/// The key every probe here remembers a tab under.
fn key(human_id: &str) -> EditKey {
    EditKey::saved(Category::Tags, human_id)
}

/// Renders a probe component to an HTML string, without settling (see `edit_stash.rs`).
fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// Renders a probe and settles it: `use_effect` bodies run only after a render pass (see
/// `edit_stash.rs`).
fn render_settled(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    for _ in 0..8 {
        vdom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
    dioxus_ssr::render(&vdom)
}

/// A stand-in detail pane: mounts [`use_detail_tab`] and, in a mount-time hook, optionally picks a
/// tab — standing in for a `Tabs` `onselect`.
#[component]
fn TabPane(human_id: String, #[props(default)] pick: Option<usize>) -> Element {
    let active = use_detail_tab(Category::Tags, &human_id);
    if let Some(pick) = pick {
        use_hook(move || {
            let mut active = active;
            active.set(pick);
        });
    }
    rsx! { div { "ACTIVE:{active}" } }
}

fn fresh_pane() -> Element {
    let _nav = use_context_provider(NavState::new);
    rsx! {
        TabPane { human_id: "T0001" }
    }
}

#[test]
fn a_fresh_record_opens_on_tab_0() {
    let html = render(fresh_pane);
    assert!(html.contains("ACTIVE:0"), "nothing remembered yet, so tab 0:\n{html}");
}

/// Reads the shell's remembered tab **reactively**, which is what a rendering probe needs:
/// `NavState::remembered_tab` deliberately peeks — it serves `use_detail_tab`'s mount-time `use_hook`,
/// where subscribing would re-render every mounted pane on any other record's tab change — so a probe
/// built on it would not re-render when a pane writes its pick through.
fn shown(nav: &NavState, human_id: &str) -> usize {
    nav.detail_tabs.read().get(&key(human_id)).copied().unwrap_or(0)
}

fn pick_records_in_shell() -> Element {
    let nav = use_context_provider(NavState::new);
    rsx! {
        TabPane { human_id: "T0001", pick: Some(2) }
        div { "REMEMBERED:{shown(&nav, \"T0001\")}" }
    }
}

#[test]
fn a_pick_is_remembered_in_the_shell() {
    let html = render_settled(pick_records_in_shell);
    assert!(
        html.contains("REMEMBERED:2"),
        "picking a tab writes it through to the shell:\n{html}"
    );
}

fn remount_shows_remembered_tab() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.remember_tab(key("T0001"), 2));
    rsx! {
        TabPane { human_id: "T0001" }
    }
}

#[test]
fn a_remounted_pane_comes_back_on_the_tab_it_left() {
    // Seeded in a `use_hook`, not an effect: `render` runs one pass only, so this would still show 0
    // if the seed came from an effect (a flash of Overview before the remembered tab took over).
    let html = render(remount_shows_remembered_tab);
    assert!(
        html.contains("ACTIVE:2"),
        "the remembered tab is already right on the very first render:\n{html}"
    );
}

fn two_records_remember_independently() -> Element {
    let nav = use_context_provider(NavState::new);
    rsx! {
        TabPane { human_id: "T0001", pick: Some(1) }
        TabPane { human_id: "T0002", pick: Some(3) }
        div { "R1:{shown(&nav, \"T0001\")}" }
        div { "R2:{shown(&nav, \"T0002\")}" }
    }
}

#[test]
fn two_records_remember_their_tab_independently() {
    let html = render_settled(two_records_remember_independently);
    assert!(html.contains("R1:1"), "the first record's own pick:\n{html}");
    assert!(
        html.contains("R2:3"),
        "the second record's own pick, untouched by the first:\n{html}"
    );
}

fn closing_forgets_the_tab() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        nav.remember_tab(key("T0001"), 2);
        nav.close_record(0);
    });
    rsx! { div { "REMEMBERED:{shown(&nav, \"T0001\")}" } }
}

#[test]
fn closing_a_record_forgets_its_remembered_tab() {
    let html = render(closing_forgets_the_tab);
    assert!(
        html.contains("REMEMBERED:0"),
        "a forgotten tab reads back as the default:\n{html}"
    );
}

fn renaming_rekeys_the_tab() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        nav.remember_tab(key("T0001"), 2);
        nav.rename_record(Category::Tags, "T0001", "T0099".to_owned());
    });
    rsx! {
        div { "OLD:{shown(&nav, \"T0001\")}" }
        div { "NEW:{shown(&nav, \"T0099\")}" }
    }
}

#[test]
fn renaming_a_record_moves_its_remembered_tab_to_the_new_id() {
    let html = render(renaming_rekeys_the_tab);
    assert!(html.contains("OLD:0"), "the old id remembers nothing any more:\n{html}");
    assert!(
        html.contains("NEW:2"),
        "the pick followed the record to its new id:\n{html}"
    );
}

fn remembering_tab_zero_stores_nothing() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.remember_tab(key("T0001"), 2));
    use_hook(move || nav.remember_tab(key("T0001"), 0));
    rsx! { div { "SIZE:{nav.detail_tabs.read().len()}" } }
}

#[test]
fn remembering_tab_zero_is_stored_as_absence() {
    let html = render(remembering_tab_zero_stores_nothing);
    assert!(
        html.contains("SIZE:0"),
        "index 0 never occupies the map, keeping it the size of actual deviations:\n{html}"
    );
}

fn draft_frame() -> Element {
    let body = rsx! { p { "fields" } };
    create_record_frame("New Tag", "draft · not saved", rsx! {}, body)
}

#[test]
fn a_draft_frame_renders_no_tabs_so_never_arms_a_remembered_tab() {
    // `create_record_frame` is every create form's whole body (`tests/record_form.rs:252`); it never
    // renders a `Tabs`, so a draft never calls `use_detail_tab` and never remembers a tab.
    let html = render(draft_frame);
    assert!(
        !html.contains(r#"role="tablist""#),
        "a create form has no tab strip to remember a tab for:\n{html}"
    );
}

// ---- The shared attached-records table (issue #304) ------------------------------------------------

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

/// One row whose record resolves, with two caller cells and an actions cell.
fn attached_with_actions() -> Element {
    use_context_provider(NavState::new);
    let loc = loc();
    let rows = vec![AttachedRow {
        link: Ok(AttachedLink {
            category: Category::Notes,
            human_id: "N0001".to_owned(),
            label: "N0001".to_owned(),
        }),
        cells: rsx! {
            td { "MIDDLE-A" }
            td { "MIDDLE-B" }
        },
        actions: rsx! {
            td { class: "row-actions", button { "Detach" } }
        },
    }];
    attached_table(
        &loc,
        "Attached".to_owned(),
        vec!["Object".to_owned(), "A".to_owned(), "B".to_owned(), String::new()],
        rows,
    )
}

/// The read-only variant: no actions cell, and no trailing header for one either.
fn attached_read_only() -> Element {
    use_context_provider(NavState::new);
    let loc = loc();
    let rows = vec![AttachedRow {
        link: Ok(AttachedLink {
            category: Category::People,
            human_id: "I0042".to_owned(),
            label: "John Smith".to_owned(),
        }),
        cells: rsx! { td { "MIDDLE-A" } },
        actions: rsx! {},
    }];
    attached_table(
        &loc,
        "Attached".to_owned(),
        vec!["Object".to_owned(), "A".to_owned()],
        rows,
    )
}

/// A row whose record does not resolve: the leading cell degrades to plain text, never a dead link.
fn attached_unresolvable() -> Element {
    use_context_provider(NavState::new);
    let loc = loc();
    let rows = vec![AttachedRow {
        link: Err("Unsourced observation".to_owned()),
        cells: rsx! { td { "MIDDLE-A" } },
        actions: rsx! {},
    }];
    attached_table(
        &loc,
        "Attached".to_owned(),
        vec!["Object".to_owned(), "A".to_owned()],
        rows,
    )
}

fn attached_empty() -> Element {
    let loc = loc();
    attached_table(&loc, "Attached".to_owned(), vec!["Object".to_owned()], Vec::new())
}

#[test]
fn an_attached_row_leads_with_a_link_then_the_callers_cells_then_its_actions() {
    let html = render(attached_with_actions);
    assert!(
        html.contains(r#"<button class="src-link" type="button">N0001</button>"#),
        "the leading cell opens the row's record:\n{html}"
    );
    // The order is load-bearing: the header count is fixed by the caller, so a cell rendered out of
    // order (or a missing one) would silently mislabel every column after it.
    let found: Vec<usize> = ["src-link", "MIDDLE-A", "MIDDLE-B", "row-actions"]
        .iter()
        .filter_map(|needle| html.find(needle))
        .collect();
    assert_eq!(found.len(), 4, "every cell of the row is rendered:\n{html}");
    assert!(
        found.is_sorted(),
        "link · caller cells · actions, in that order:\n{html}"
    );
}

#[test]
fn an_attached_tables_headers_match_the_cells_it_renders() {
    // The spine owns the link cell and the caller owns the rest, so the two counts can only agree if
    // the caller's `cells` line up with the headers it passed — this pins that they do.
    let html = render(attached_with_actions);
    assert_eq!(html.matches("</th>").count(), 4, "four headers were passed:\n{html}");
    assert_eq!(html.matches("</td>").count(), 4, "four cells are rendered:\n{html}");
    assert!(
        html.contains(r#"<th><span class="sr-only">"#),
        "the trailing empty header is the row-actions column, named for screen readers:\n{html}"
    );
}

#[test]
fn a_read_only_attached_table_renders_no_actions_column() {
    let html = render(attached_read_only);
    assert_eq!(html.matches("</th>").count(), 2, "two headers were passed:\n{html}");
    assert_eq!(html.matches("</td>").count(), 2, "two cells are rendered:\n{html}");
    assert!(
        !html.contains("row-actions"),
        "a read-only table has no actions cell at all:\n{html}"
    );
}

#[test]
fn an_unresolvable_row_renders_its_label_as_text_not_a_link() {
    let html = render(attached_unresolvable);
    assert!(
        html.contains("Unsourced observation"),
        "the label is still shown:\n{html}"
    );
    assert!(
        !html.contains("src-link"),
        "there is nothing to open, so there is no link:\n{html}"
    );
}

#[test]
fn an_attached_table_with_no_rows_is_the_shared_empty_state() {
    let html = render(attached_empty);
    assert!(
        html.contains("Nothing here yet."),
        "no rows renders the shared empty state, not an empty table:\n{html}"
    );
    assert!(
        !html.contains("<table"),
        "an empty table's headers would state columns with nothing under them:\n{html}"
    );
}
