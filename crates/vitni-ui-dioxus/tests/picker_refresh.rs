//! SSR-probe coverage for #266: a record picker (and the command palette) loads its rows in a
//! `use_resource`, so it only refetches when the closure reads a signal that changed. Every one of
//! them read none, which made a record created while the picker stayed open invisible to it.
//!
//! [`data_version_ticket`] is the seam those closures now call. The probes here pin its two
//! contracts: it is safe without a shell (the host-free SSR test files provide no [`NavState`]), and a
//! read of it inside a `use_resource` closure genuinely restarts the resource when
//! [`NavState::mark_changed`] fires — the load-bearing one, since a subscription that does not
//! subscribe compiles and reads exactly the same.
//!
//! Host-free style, like `create_refresh.rs`: a bare [`NavState`] provided as context, mutated from a
//! hook/effect, with the observable rendered as a string marker. [`render_settled`] is
//! `edit_stash.rs`'s — a resource restart is an effect cascade, so one render pass is not enough.

use std::cell::Cell;
use std::rc::Rc;

use dioxus::prelude::*;
use vitni_ui_dioxus::shell::nav_state::{NavState, data_version_ticket};

/// Renders a probe and settles it: a `use_resource` restart is driven by a spawned task and a
/// following render pass, so the probe pumps the virtual DOM until the cascade stops.
fn render_settled(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    for _ in 0..8 {
        vdom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
    dioxus_ssr::render(&vdom)
}

fn no_shell() -> Element {
    let ticket = data_version_ticket(None);
    rsx! {
        div { "TICKET:{ticket}" }
    }
}

#[test]
fn the_ticket_is_zero_without_a_shell() {
    let html = render_settled(no_shell);
    assert!(
        html.contains("TICKET:0"),
        "a picker rendered without a shell (every host-free SSR test) must still load its rows:\n{html}"
    );
}

fn bumped_ticket() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.mark_changed();
        nav.mark_changed();
    });
    let ticket = data_version_ticket(Some(nav));
    rsx! {
        div { "TICKET:{ticket}" }
    }
}

#[test]
fn the_ticket_reports_the_data_version() {
    let html = render_settled(bumped_ticket);
    assert!(
        html.contains("TICKET:2"),
        "the ticket is `NavState::data_version`, which `mark_changed` bumps once per mutation:\n{html}"
    );
}

/// A stand-in for a picker: a `use_resource` that reads the ticket in its synchronous part and counts
/// how many times its closure ran. The count lives in a `Cell` outside the reactive graph, so
/// observing it never subscribes anything and cannot itself cause the restart under test.
fn picker_probe() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let runs: Rc<Cell<u32>> = use_hook(|| Rc::new(Cell::new(0)));
    let counted = Rc::clone(&runs);
    let rows = use_resource(move || {
        let _ = data_version_ticket(Some(nav));
        counted.set(counted.get() + 1);
        let loaded = counted.get();
        async move { loaded }
    });
    // One mutation, from an effect so it lands after the first render pass — `peek` (not `read`), so
    // this effect never re-runs on its own bump.
    use_effect(move || {
        if *nav.data_version.peek() == 0 {
            nav.mark_changed();
        }
    });
    // Read the resource in render, so a restart dirties this scope and the settle loop keeps pumping.
    let loaded = rows.read_unchecked().unwrap_or(0);
    rsx! {
        div { "RUNS:{runs.get()}" }
        div { "LOADED:{loaded}" }
    }
}

#[test]
fn a_ticket_read_in_a_resource_closure_refetches_on_a_mutation() {
    let html = render_settled(picker_probe);
    assert!(
        html.contains("RUNS:2"),
        "the closure must re-run once the workspace changes — a picker that loads its rows once \
         cannot see a record created while it stays open (#266):\n{html}"
    );
}
