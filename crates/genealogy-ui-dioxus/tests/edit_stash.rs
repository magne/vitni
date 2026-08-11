//! SSR-probe assertions for the shell's edit stash (issue #239): an in-progress edit is parked in
//! [`NavState::edit_drafts`], keyed per editor, so it survives its pane unmounting when another record
//! tab becomes active. Only the active tab's pane is mounted (`screens/record_detail.rs`), so the
//! buffer cannot live in the pane — the pane hydrates from the shell on mount and writes through on
//! every change.
//!
//! Two probe styles. The [`NavState`]-only probes drive the methods in `use_hook` and inspect the
//! `DIRTY:` / `RESTORED-*` markers, like `close_confirm.rs`. The pane probes mount [`SavedPane`] /
//! [`CreatePane`] — stand-ins for a real detail pane — over the same [`NavState`], so hydration and the
//! write-through effect are exercised end to end. Those need [`render_settled`]: `use_effect` bodies
//! run only after a render pass.

use dioxus::prelude::*;
use genealogy_ui::{Category, NoteDraft, ProvenanceDraft, RecordRef, TagDraft};
use genealogy_ui_dioxus::screens::{use_record_create, use_record_edit};
use genealogy_ui_dioxus::shell::nav_state::{DraftId, EditKey, NavState, StashedEdit};

/// The committed name of the record every probe edits.
const COMMITTED: &str = "Ada";
/// The name a probe types into the draft, making it dirty.
const TYPED: &str = "Ada Lovelace";
/// The name a *second* draft is typed with, so the two buffers can be told apart.
const OTHER: &str = "Bess Hopper";

fn record(human_id: &str, label: &str) -> RecordRef {
    RecordRef {
        category: Category::Tags,
        human_id: human_id.to_owned(),
        label: label.to_owned(),
    }
}

/// The committed values a parked edit is diffed against.
fn committed() -> TagDraft {
    TagDraft {
        name: COMMITTED.to_owned(),
        ..TagDraft::new()
    }
}

/// A dirty draft: [`committed`] with the name typed over.
fn typed() -> TagDraft {
    TagDraft {
        name: TYPED.to_owned(),
        ..TagDraft::new()
    }
}

/// The provenance a probe parks alongside the draft, so the round-trip has something to compare.
fn why() -> ProvenanceDraft {
    ProvenanceDraft {
        rationale: "census".to_owned(),
        ..ProvenanceDraft::default()
    }
}

/// Parks [`typed`] against [`committed`] under `key`.
fn park(nav: &mut NavState, key: EditKey) {
    nav.stash_edit(key, StashedEdit::new(typed(), committed(), why()));
}

/// Renders a probe component to an HTML string, without settling: only the first render pass runs, so
/// nothing a `use_effect` does is visible. What a pane shows here is what it shows on mount.
fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// Renders a probe and settles it: `use_effect` bodies run only *after* a render pass, and an effect
/// that writes a signal dirties another scope, so the probe pumps the virtual DOM until the cascade
/// stops. This is what makes the write-through effect observable in an SSR test.
fn render_settled(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    for _ in 0..8 {
        vdom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
    dioxus_ssr::render(&vdom)
}

/// The keys currently holding a parked edit, in [`EditKey`]'s own `category/id` form (a create draft
/// reads `category/#n`) — the shell's dirty set.
fn keys(nav: &NavState) -> String {
    nav.edit_drafts
        .read()
        .keys()
        .map(EditKey::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// The shell-level marker block: the open-tab count, the parked-edit keys, and whether each open tab
/// reports unsaved work.
fn probe(nav: &NavState) -> Element {
    let tabs = nav.records.read().len();
    let unsaved = (0..tabs)
        .map(|index| if nav.tab_has_unsaved(index) { "Y" } else { "N" })
        .collect::<String>();
    rsx! {
        div { "TABS:{tabs}" }
        div { "DIRTY:[{keys(nav)}]" }
        div { "UNSAVED:{unsaved}" }
    }
}

/// Reads the edit parked under `key` back out as a [`TagDraft`] — the draft, the seed it is diffed
/// against, the rationale, and the recorded validity — or `RESTORED:NONE` when nothing is parked there
/// (including when the entry holds a different draft type).
fn restored(nav: &NavState, key: &EditKey) -> Element {
    let valid = nav
        .edit_drafts
        .read()
        .get(key)
        .map_or_else(|| "NONE".to_owned(), |edit| edit.valid.to_string());
    match nav.stashed_edit::<TagDraft>(key) {
        None => rsx! {
            div { "RESTORED:NONE" }
            div { "RESTORED-VALID:{valid}" }
        },
        Some((draft, seed, prov)) => rsx! {
            div { "RESTORED-DRAFT:{draft.name}" }
            div { "RESTORED-SEED:{seed.name}" }
            div { "RESTORED-WHY:{prov.rationale}" }
            div { "RESTORED-VALID:{valid}" }
        },
    }
}

/// The name held in the draft parked under `key`, or `NONE` when nothing is parked there — how a probe
/// shows *which* buffer a key holds, rather than only that it holds one.
fn parked_name(nav: &NavState, key: &EditKey) -> String {
    nav.stashed_edit::<TagDraft>(key)
        .map_or_else(|| "NONE".to_owned(), |(draft, _, _)| draft.name)
}

// ---- The stash itself ----------------------------------------------------------------------------

fn stash_round_trip() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || park(&mut nav, EditKey::saved(Category::Tags, "T0001")));
    restored(&nav, &EditKey::saved(Category::Tags, "T0001"))
}

#[test]
fn a_parked_edit_reads_back_as_its_draft_seed_and_provenance() {
    let html = render(stash_round_trip);
    assert!(
        html.contains(&format!("RESTORED-DRAFT:{TYPED}")),
        "the parked draft comes back:\n{html}"
    );
    assert!(
        html.contains(&format!("RESTORED-SEED:{COMMITTED}")),
        "so does the seed it is diffed against:\n{html}"
    );
    assert!(
        html.contains("RESTORED-WHY:census"),
        "and the provenance collected with it:\n{html}"
    );
    assert!(
        html.contains("RESTORED-VALID:true"),
        "the recorded validity is the draft's own (the Save gate):\n{html}"
    );
}

fn stash_round_trip_wrong_type() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let key = EditKey::saved(Category::Tags, "T0001");
    use_hook({
        let key = key.clone();
        move || park(&mut nav, key)
    });
    let mismatched = if nav.stashed_edit::<NoteDraft>(&key).is_some() {
        "SOME"
    } else {
        "NONE"
    };
    rsx! {
        div { "MISMATCHED:{mismatched}" }
    }
}

#[test]
fn reading_a_parked_edit_as_the_wrong_draft_type_is_none_not_a_panic() {
    // The stash is heterogeneous over every aggregate's draft type, so the downcast can miss. A miss
    // must read as "nothing parked here" — a panic in a mount-time hook would take the whole shell down.
    let html = render(stash_round_trip_wrong_type);
    assert!(
        html.contains("MISMATCHED:NONE"),
        "a type mismatch resolves to None:\n{html}"
    );
}

fn two_records_dirty_at_once() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        nav.open_record(record("T0002", "Bob"));
        park(&mut nav, EditKey::saved(Category::Tags, "T0001"));
        park(&mut nav, EditKey::saved(Category::Tags, "T0002"));
    });
    probe(&nav)
}

#[test]
fn two_records_can_hold_an_in_progress_edit_at_the_same_time() {
    let html = render(two_records_dirty_at_once);
    assert!(
        html.contains("DIRTY:[tags/T0001,tags/T0002]"),
        "both records keep their own parked edit:\n{html}"
    );
    assert!(
        html.contains("UNSAVED:YY"),
        "both tabs report unsaved work, not just the active one:\n{html}"
    );
}

fn activate_other_tab_keeps_the_parked_edit() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        nav.open_record(record("T0002", "Bob"));
        park(&mut nav, EditKey::saved(Category::Tags, "T0001"));
        nav.activate_record(1);
    });
    probe(&nav)
}

#[test]
fn activating_another_tab_leaves_the_parked_edit_in_place() {
    // The #239 regression: `use_record_edit`'s `use_drop` cleared the record's dirty mark when its pane
    // unmounted, so switching tabs silently discarded the edit — no marker, no confirm.
    let html = render(activate_other_tab_keeps_the_parked_edit);
    assert!(
        html.contains("DIRTY:[tags/T0001]"),
        "leaving a record's tab keeps its edit parked:\n{html}"
    );
    assert!(
        html.contains("UNSAVED:YN"),
        "the record left behind still reports unsaved work:\n{html}"
    );
}

fn close_one_of_two_dirty_records() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        nav.open_record(record("T0002", "Bob"));
        park(&mut nav, EditKey::saved(Category::Tags, "T0001"));
        park(&mut nav, EditKey::saved(Category::Tags, "T0002"));
        nav.close_record(0);
    });
    probe(&nav)
}

#[test]
fn closing_a_dirty_tab_drops_only_its_own_parked_edit() {
    let html = render(close_one_of_two_dirty_records);
    assert!(html.contains("TABS:1"), "one tab closed:\n{html}");
    assert!(
        html.contains("DIRTY:[tags/T0002]"),
        "the closed record's edit is gone and its neighbour's is untouched:\n{html}"
    );
}

fn rename_moves_the_parked_edit() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        park(&mut nav, EditKey::saved(Category::Tags, "T0001"));
        nav.rename_record(Category::Tags, "T0001", "T0099".to_owned());
    });
    rsx! {
        {probe(&nav)}
        {restored(&nav, &EditKey::saved(Category::Tags, "T0099"))}
    }
}

#[test]
fn renaming_a_record_moves_its_parked_edit_to_the_new_id() {
    let html = render(rename_moves_the_parked_edit);
    assert!(
        html.contains("DIRTY:[tags/T0099]"),
        "the parked edit follows the record to its new id:\n{html}"
    );
    assert!(
        html.contains(&format!("RESTORED-DRAFT:{TYPED}")),
        "and its contents survive the move:\n{html}"
    );
}

fn commit_draft_drops_the_create_entry() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let draft = nav.open_create(Category::Tags);
        park(&mut nav, EditKey::draft(Category::Tags, draft));
        nav.commit_draft(draft, record("T0001", "Ada"));
    });
    probe(&nav)
}

#[test]
fn committing_a_draft_drops_its_parked_create_edit() {
    let html = render(commit_draft_drops_the_create_entry);
    assert!(
        html.contains("DIRTY:[]"),
        "the committed draft's buffer is spent, not left parked:\n{html}"
    );
    assert!(
        html.contains("UNSAVED:N"),
        "the saved record that replaced it is clean:\n{html}"
    );
}

fn cancel_draft_drops_the_create_entry() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let draft = nav.open_create(Category::Tags);
        park(&mut nav, EditKey::draft(Category::Tags, draft));
        nav.cancel_draft(draft);
    });
    probe(&nav)
}

#[test]
fn cancelling_a_draft_drops_its_parked_create_edit() {
    let html = render(cancel_draft_drops_the_create_entry);
    assert!(html.contains("TABS:0"), "the draft tab closed:\n{html}");
    assert!(html.contains("DIRTY:[]"), "and took its buffer with it:\n{html}");
}

fn create_draft_survives_a_switch_to_a_saved_tab() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let draft = nav.open_create(Category::Tags);
        park(&mut nav, EditKey::draft(Category::Tags, draft));
        nav.open_record(record("T0001", "Ada"));
    });
    probe(&nav)
}

#[test]
fn a_create_drafts_buffer_is_keyed_by_its_draft_id_and_survives_a_tab_switch() {
    let html = render(create_draft_survives_a_switch_to_a_saved_tab);
    assert!(
        html.contains("DIRTY:[tags/#1]"),
        "a create draft is keyed by its own draft id, not a human_id:\n{html}"
    );
    assert!(html.contains("TABS:2"), "the draft tab is still open:\n{html}");
}

// ---- Pane hydration and write-through ------------------------------------------------------------

/// What the pane probe does to its buffer on mount, standing in for the user action a real detail pane
/// would take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneAction {
    /// Nothing — just report the buffer it came up with.
    Report,
    /// Enter edit mode and type a new name into the draft (an in-progress edit). Edit mode is not
    /// optional: while the pane is read-first the reseed effect keeps the draft on the committed record.
    Type,
    /// Cancel the edit session ([`genealogy_ui_dioxus::screens::RecordEditState::cancel`]).
    Cancel,
    /// Leave edit mode the way Save does, letting the reseed restore the committed values.
    LeaveEditMode,
}

/// A stand-in for an aggregate's detail pane: the shared edit buffer for a saved record, one mount-time
/// action, and markers for what the buffer holds. `use_record_edit` is the whole point — this is where
/// hydration and the write-through effect are exercised.
#[component]
fn SavedPane(human_id: String, action: PaneAction) -> Element {
    let state = use_record_edit::<TagDraft>(Category::Tags, &human_id, &committed());
    use_hook(move || {
        let mut state = state;
        match action {
            PaneAction::Report => {}
            PaneAction::Type => {
                state.begin_edit();
                TYPED.clone_into(&mut state.draft.write().name);
            }
            PaneAction::Cancel => state.cancel(),
            PaneAction::LeaveEditMode => state.editing.set(false),
        }
    });
    rsx! {
        div { "PANE-NAME:{state.draft.read().name}" }
        div { "PANE-SEED:{state.seed.read().name}" }
        div { "PANE-EDITING:{state.editing.read()}" }
        div { "PANE-WHY:{state.prov.read().rationale}" }
    }
}

/// The same stand-in for a create form: one draft's create buffer, keyed by the draft's own
/// [`DraftId`] rather than by its category, which is what keeps two drafts of one category apart.
/// `name` is what [`PaneAction::Type`] types into it.
#[component]
fn CreatePane(draft: DraftId, action: PaneAction, name: String) -> Element {
    let state = use_record_create::<TagDraft>(Category::Tags, draft);
    use_hook(move || {
        let mut state = state;
        if action == PaneAction::Type {
            state.draft.write().name = name;
        }
    });
    rsx! {
        div { "PANE-NAME:{state.draft.read().name}" }
        div { "PANE-EDITING:{state.editing.read()}" }
    }
}

fn pane_hydrates_from_the_stash() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        park(&mut nav, EditKey::saved(Category::Tags, "T0001"));
    });
    rsx! {
        SavedPane { human_id: "T0001", action: PaneAction::Report }
    }
}

#[test]
fn a_pane_shows_the_parked_edit_on_its_very_first_render() {
    // Hydration happens in `use_hook`, not an effect: `render` runs one pass only, so if the buffer
    // were restored from an effect this would still show the committed value.
    let html = render(pane_hydrates_from_the_stash);
    assert!(
        html.contains(&format!("PANE-NAME:{TYPED}")),
        "the pane comes up holding the parked draft:\n{html}"
    );
    assert!(
        html.contains(&format!("PANE-SEED:{COMMITTED}")),
        "diffed against the seed it was parked with:\n{html}"
    );
    assert!(
        html.contains("PANE-EDITING:true"),
        "a restored buffer holds unsaved changes, so the pane comes up in edit mode:\n{html}"
    );
    assert!(
        html.contains("PANE-WHY:census"),
        "the provenance collected before the pane unmounted is restored too:\n{html}"
    );
}

fn pane_writes_the_edit_through() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.open_record(record("T0001", "Ada")));
    rsx! {
        {probe(&nav)}
        {restored(&nav, &EditKey::saved(Category::Tags, "T0001"))}
        SavedPane { human_id: "T0001", action: PaneAction::Type }
    }
}

#[test]
fn typing_in_a_pane_parks_the_edit_in_the_shell() {
    let html = render_settled(pane_writes_the_edit_through);
    assert!(
        html.contains("DIRTY:[tags/T0001]"),
        "the dirty buffer is written through to the shell:\n{html}"
    );
    assert!(
        html.contains(&format!("RESTORED-DRAFT:{TYPED}")),
        "with the draft as typed:\n{html}"
    );
    assert!(
        html.contains(&format!("RESTORED-SEED:{COMMITTED}")),
        "and the committed seed it is diffed against:\n{html}"
    );
    assert!(html.contains("UNSAVED:Y"), "so the tab reports unsaved work:\n{html}");
}

/// A pane that is swapped for a fresh instance of itself once it has parked an edit — the remount a
/// tab switch causes. The flip is driven by an effect watching the stash, so it cannot run before the
/// first pane's write-through does.
fn pane_remounts_and_rehydrates() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.open_record(record("T0001", "Ada")));
    let mut remounted = use_signal(|| false);
    use_effect(move || {
        if nav.has_unsaved(&EditKey::saved(Category::Tags, "T0001")) {
            remounted.set(true);
        }
    });
    // A different `key` is what makes Dioxus tear the pane down and build a fresh instance, exactly as
    // switching record tabs does.
    let (phase, action) = if remounted() {
        ("remounted", PaneAction::Report)
    } else {
        ("first", PaneAction::Type)
    };
    rsx! {
        SavedPane { key: "{phase}", human_id: "T0001", action }
        div { "PHASE:{phase}" }
    }
}

#[test]
fn a_remounted_pane_picks_the_edit_back_up_where_it_left_off() {
    let html = render_settled(pane_remounts_and_rehydrates);
    assert!(html.contains("PHASE:remounted"), "the pane was replaced:\n{html}");
    assert!(
        html.contains(&format!("PANE-NAME:{TYPED}")),
        "the fresh pane instance restores the edit rather than the committed record:\n{html}"
    );
    assert!(
        html.contains("PANE-EDITING:true"),
        "and comes back up in edit mode:\n{html}"
    );
}

fn pane_cancel_clears_the_stash() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        park(&mut nav, EditKey::saved(Category::Tags, "T0001"));
    });
    rsx! {
        {probe(&nav)}
        SavedPane { human_id: "T0001", action: PaneAction::Cancel }
    }
}

#[test]
fn cancelling_an_edit_clears_its_parked_entry() {
    let html = render_settled(pane_cancel_clears_the_stash);
    assert!(
        html.contains("DIRTY:[]"),
        "Cancel restores the seed, so the buffer is clean and nothing stays parked:\n{html}"
    );
    assert!(html.contains("UNSAVED:N"), "the tab closes without a confirm:\n{html}");
    assert!(
        html.contains(&format!("PANE-NAME:{COMMITTED}")),
        "and the pane shows the committed record again:\n{html}"
    );
}

fn pane_save_reseed_clears_the_stash() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("T0001", "Ada"));
        park(&mut nav, EditKey::saved(Category::Tags, "T0001"));
    });
    rsx! {
        {probe(&nav)}
        SavedPane { human_id: "T0001", action: PaneAction::LeaveEditMode }
    }
}

#[test]
fn a_save_reseed_clears_the_parked_entry() {
    // Save hands the draft off and leaves edit mode; the reseed then brings the buffer back in line with
    // the committed record, which is what drops the entry — no extra wiring on the save path.
    let html = render_settled(pane_save_reseed_clears_the_stash);
    assert!(
        html.contains("DIRTY:[]"),
        "a reseeded buffer is clean, so nothing stays parked:\n{html}"
    );
    assert!(html.contains("UNSAVED:N"), "the tab reports no unsaved work:\n{html}");
}

fn create_pane_writes_the_draft_through() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let draft = use_hook(move || nav.open_create(Category::Tags));
    rsx! {
        {probe(&nav)}
        CreatePane { draft, action: PaneAction::Type, name: TYPED }
    }
}

#[test]
fn typing_in_a_create_form_parks_the_draft_under_its_draft_id() {
    let html = render_settled(create_pane_writes_the_draft_through);
    assert!(
        html.contains("DIRTY:[tags/#1]"),
        "a create form parks its buffer under its own draft id:\n{html}"
    );
}

fn create_pane_hydrates_from_the_stash() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let draft = use_hook(move || {
        let draft = nav.open_create(Category::Tags);
        park(&mut nav, EditKey::draft(Category::Tags, draft));
        draft
    });
    rsx! {
        CreatePane { draft, action: PaneAction::Report, name: TYPED }
    }
}

#[test]
fn a_create_form_comes_back_up_holding_its_parked_draft() {
    let html = render(create_pane_hydrates_from_the_stash);
    assert!(
        html.contains(&format!("PANE-NAME:{TYPED}")),
        "returning to a create tab restores what was typed:\n{html}"
    );
    assert!(
        html.contains("PANE-EDITING:true"),
        "a create form is in edit mode either way:\n{html}"
    );
}

// ---- Several drafts of one category (issue #260) --------------------------------------------------

/// Two drafts of one category, each with its own mounted create pane, typed apart.
fn two_drafts_typed_apart() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let (first, second) = use_hook(move || (nav.open_create(Category::Tags), nav.open_create(Category::Tags)));
    rsx! {
        {probe(&nav)}
        div { "PARKED-1:{parked_name(&nav, &EditKey::draft(Category::Tags, first))}" }
        div { "PARKED-2:{parked_name(&nav, &EditKey::draft(Category::Tags, second))}" }
        CreatePane { draft: first, action: PaneAction::Type, name: TYPED }
        CreatePane { draft: second, action: PaneAction::Type, name: OTHER }
    }
}

#[test]
fn two_drafts_of_one_category_keep_separate_buffers() {
    // The point of #260: sketching two new records side by side is worthless if they share one buffer.
    let html = render_settled(two_drafts_typed_apart);
    assert!(
        html.contains("DIRTY:[tags/#1,tags/#2]"),
        "each draft parks its own buffer, under its own id:\n{html}"
    );
    assert!(
        html.contains("UNSAVED:YY"),
        "both draft tabs report unsaved work:\n{html}"
    );
    assert!(
        html.contains(&format!("PARKED-1:{TYPED}")) && html.contains(&format!("PARKED-2:{OTHER}")),
        "and each buffer holds what was typed into that draft:\n{html}"
    );
}

/// A typed draft's pane beside a second draft's freshly-mounted, still-empty one.
fn a_typed_draft_beside_a_fresh_one() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let (first, second) = use_hook(move || (nav.open_create(Category::Tags), nav.open_create(Category::Tags)));
    rsx! {
        {probe(&nav)}
        CreatePane { draft: first, action: PaneAction::Type, name: TYPED }
        CreatePane { draft: second, action: PaneAction::Report, name: TYPED }
    }
}

#[test]
fn a_second_drafts_clean_pane_does_not_drop_the_firsts_buffer() {
    // This is why only one draft per category used to be allowed: a clean pane's write-through calls
    // `drop_edit` on its own key, and under a key that was the category alone that deleted the *other*
    // draft's typed buffer — the second ⌘N silently wiping the first form.
    let html = render_settled(a_typed_draft_beside_a_fresh_one);
    assert!(
        html.contains("DIRTY:[tags/#1]"),
        "the fresh pane drops only its own (absent) entry:\n{html}"
    );
    assert!(
        html.contains(&format!("PANE-NAME:{TYPED}")) && html.contains("PANE-NAME:</div>"),
        "and the second draft comes up empty rather than showing the first's text:\n{html}"
    );
}
