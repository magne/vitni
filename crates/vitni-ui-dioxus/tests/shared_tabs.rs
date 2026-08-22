//! SSR assertions for [`shared_tab`], the one arm the 13 detail screens reach the six shared tabs
//! through (issue #322). `tab_actions.rs` covers the *frame* those tabs render inside; this file
//! covers the *dispatch* above it — which is a refactor of 51 hand-copied `match` arms, so the
//! load-bearing assertion is byte identity: for every shared tab, `shared_tab` must render exactly
//! the string the arm it replaced rendered, character for character.
//!
//! The comparison is what the screens' own suites cannot give. Every `*_detail.rs` test calls the
//! body helpers (`notes_table`, `history_panel`, …) directly — the dispatchers are private and
//! nothing in CI ever renders one — so a wrong `TabActionStyle`, a dropped `rsx!` fragment wrapper
//! or a swapped `show_backs` would pass the whole suite. These tests render both shapes over one
//! fixture and diff them.

use dioxus::prelude::*;
use vitni_app::TagRef;
use vitni_ui::{
    ActionLabel, AttachedRefVm, Category, CitationRefVm, ConfidenceLevel, DetailTab, HistoryEntryVm, Localizer,
    MediaRefVm, RowVm,
};
use vitni_ui_dioxus::components::{ButtonVariant, TabItem};
use vitni_ui_dioxus::screens::{
    CitationsArm, FormTabs, MediaArm, MediaTabState, NotesArm, ResearchNotesArm, SharedTabCtx, TabActionStyle,
    TabActionTarget, TagsArm, citations_table, history_panel, media_tab, notes_table, shared_tab, tab_frame,
    tags_panel,
};

/// The stand-in edit-form enum a screen parameterises its shared tabs over — one variant per form
/// tab, as every real `*EditForm` has.
#[derive(Clone, PartialEq)]
enum TestForm {
    Citation,
    Media,
    Note,
    Tag,
}

/// Which shape rendered the tab: the arm as the 13 screens each spelled it out, or the one
/// [`shared_tab`] they now share. The byte-identity tests render both and compare.
#[derive(Clone, Copy, PartialEq)]
enum Shape {
    /// The arm copied into the screen, reproduced here verbatim from `person.rs` before #322.
    Copied,
    /// The shared dispatch, over a ctx describing the same tab.
    Shared,
}

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

/// The tab a shape renders, with the action label its real screen declares — the frame resolves its
/// button from `DetailTab::action`, so an identity test that left it `None` would compare two
/// bar-less tabs and prove nothing about the bar.
fn tab(id: &'static str, action: Option<ActionLabel>) -> DetailTab {
    DetailTab {
        id,
        label: "Label".to_owned(),
        count: Some(1),
        action,
    }
}

fn citation_rows() -> Vec<CitationRefVm> {
    vec![CitationRefVm {
        human_id: "C0001".to_owned(),
        source: Some("Kirkebok".to_owned()),
        source_id: Some("S0001".to_owned()),
        page: Some("p. 41".to_owned()),
        backs_count: 3,
        confidence: Some(ConfidenceLevel::High),
        confidence_label: Some("High".to_owned()),
        evidence_axes: Vec::new(),
        asserted_by: None,
        assertion_id: Some("11111111-1111-7111-8111-111111111111".to_owned()),
    }]
}

fn media_rows() -> Vec<MediaRefVm> {
    vec![MediaRefVm {
        human_id: "O0001".to_owned(),
        assertion_id: "22222222-2222-7222-8222-222222222222".to_owned(),
        caption: Some("Portrait".to_owned()),
        crop: None,
        path: Some("/tmp/portrait.jpg".to_owned()),
        mime: Some("image/jpeg".to_owned()),
    }]
}

fn note_rows() -> Vec<AttachedRefVm> {
    vec![AttachedRefVm {
        human_id: "N0001".to_owned(),
        note_type: None,
        type_label: Some("Transcript".to_owned()),
        text: Some("Døpt 3. mai".to_owned()),
        language: Some("no".to_owned()),
        assertion_id: "33333333-3333-7333-8333-333333333333".to_owned(),
    }]
}

fn tag_rows() -> Vec<TagRef> {
    vec![TagRef {
        id: "44444444-4444-7444-8444-444444444444".to_owned(),
        name: "Direct ancestor".to_owned(),
        color: Some("#e5534b".to_owned()),
        priority: None,
    }]
}

fn research_note_rows() -> Vec<RowVm> {
    vec![RowVm {
        id: "R0001".to_owned(),
        title: "Which Ann?".to_owned(),
        subtitle: Some("Open".to_owned()),
        avatar: None,
        dot_color: None,
        id_label: None,
    }]
}

fn history_rows() -> Vec<HistoryEntryVm> {
    vec![HistoryEntryVm {
        when: "2026-06-22 14:35".to_owned(),
        what: "Name asserted".to_owned(),
        who: "magne".to_owned(),
        why: Some("Read off the register".to_owned()),
        assertion_id: "55555555-5555-7555-8555-555555555555".to_owned(),
        can_undo: true,
    }]
}

/// Renders one shared tab in one shape, inside a component scope so `media_tab`'s viewer state and
/// the frame's `editing` signal have a runtime. `show_backs` covers the Citations tab's one real
/// variance (plural-subject records render the Backs column, singular-subject ones do not) and
/// `undoable` covers History's (`None` for Tag, which has no retraction).
fn render(id: &'static str, action: Option<ActionLabel>, shape: Shape, show_backs: bool, undoable: bool) -> String {
    #[component]
    fn Harness(
        id: &'static str,
        action: Option<ActionLabel>,
        shape: Shape,
        show_backs: bool,
        undoable: bool,
    ) -> Element {
        let loc = loc();
        let tab = tab(id, action);
        let editing = use_signal(|| None::<TestForm>);
        let viewing = use_signal(|| None::<MediaRefVm>);
        let media_state = MediaTabState {
            viewing,
            on_view: Callback::new(|_: MediaRefVm| {}),
            on_region: Callback::new(|_: (String, Option<vitni_app::Rect>, Option<String>)| {}),
        };
        let on_detach = Callback::new(|_: (String, String, bool)| {});
        let on_tag_remove = Callback::new(|_: (String, String)| {});
        let on_undo = Callback::new(|_: String| {});
        let citations = citation_rows();
        let media = media_rows();
        let notes = note_rows();
        let tags = tag_rows();
        let history = history_rows();
        match shape {
            Shape::Copied => copied_arm(
                &loc,
                &tab,
                editing,
                &CopiedFixture {
                    citations: &citations,
                    media: &media,
                    notes: &notes,
                    tags: &tags,
                    history: &history,
                    show_backs,
                    undoable,
                    media_state,
                    on_detach,
                    on_tag_remove,
                    on_undo,
                },
            ),
            Shape::Shared => {
                let ctx = SharedTabCtx {
                    forms: Some(FormTabs {
                        editing,
                        citations: Some(CitationsArm {
                            form: TestForm::Citation,
                            rows: &citations,
                            show_backs,
                            on_detach,
                        }),
                        media: Some(MediaArm {
                            form: TestForm::Media,
                            rows: &media,
                            state: media_state,
                            on_detach,
                        }),
                        notes: Some(NotesArm {
                            form: TestForm::Note,
                            rows: &notes,
                            on_detach,
                        }),
                        tags: Some(TagsArm {
                            form: TestForm::Tag,
                            rows: &tags,
                            on_remove: on_tag_remove,
                        }),
                    }),
                    research_notes: None,
                    history: &history,
                    on_undo: undoable.then_some(on_undo),
                };
                shared_tab(&loc, &tab, &ctx).unwrap_or_else(|| rsx! { div { "UNMATCHED" } })
            }
        }
    }

    let mut vdom = VirtualDom::new_with_props(
        Harness,
        HarnessProps {
            id,
            action,
            shape,
            show_backs,
            undoable,
        },
    );
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// The fixture rows and callbacks the copied arms close over — one struct so the reproduction below
/// reads as the arm bodies alone.
#[derive(Clone, Copy)]
struct CopiedFixture<'a> {
    citations: &'a [CitationRefVm],
    media: &'a [MediaRefVm],
    notes: &'a [AttachedRefVm],
    tags: &'a [TagRef],
    history: &'a [HistoryEntryVm],
    show_backs: bool,
    undoable: bool,
    media_state: MediaTabState,
    on_detach: Callback<(String, String, bool)>,
    on_tag_remove: Callback<(String, String)>,
    on_undo: Callback<String>,
}

/// The five shared arms exactly as the screens spelled them out before #322 — the `rsx! { {…} }`
/// fragment wrapper on Citations/Media/Notes, the bare element on Tags and History, the Ghost
/// emphasis on the Tags bar and `tab_frame::<()>` on History. This is the reference the shared
/// dispatch is diffed against, so it is deliberately a transcription and not a tidy-up.
fn copied_arm(loc: &Localizer, tab: &DetailTab, editing: Signal<Option<TestForm>>, fixture: &CopiedFixture) -> Element {
    let &CopiedFixture {
        citations,
        media,
        notes,
        tags,
        history,
        show_backs,
        undoable,
        media_state,
        on_detach,
        on_tag_remove,
        on_undo,
    } = fixture;
    match tab.id {
        "citations" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, TestForm::Citation),
            None,
            rsx! {
                {citations_table::<TestForm>(loc, citations, show_backs, on_detach)}
            },
        ),
        "media" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, TestForm::Media),
            None,
            rsx! {
                {media_tab(loc, media, Some(on_detach), media_state)}
            },
        ),
        "notes" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, TestForm::Note),
            None,
            rsx! {
                {notes_table(loc, notes, Some(on_detach))}
            },
        ),
        "tags" => tab_frame(
            loc,
            tab,
            TabActionTarget::Form(editing, TestForm::Tag),
            Some(TabActionStyle {
                emphasis: Some(ButtonVariant::Ghost),
                ..Default::default()
            }),
            tags_panel(loc, tags, on_tag_remove),
        ),
        "history" => tab_frame::<()>(
            loc,
            tab,
            TabActionTarget::None,
            None,
            history_panel(loc, history, undoable.then_some(on_undo)),
        ),
        _ => rsx! { div { "UNMATCHED" } },
    }
}

/// Renders both shapes over the same fixture and asserts the markup is identical — the acceptance
/// test for #322. A refactor that changed any rendered byte would show here and nowhere else.
fn assert_identical(id: &'static str, action: Option<ActionLabel>, show_backs: bool, undoable: bool) {
    let copied = render(id, action, Shape::Copied, show_backs, undoable);
    let shared = render(id, action, Shape::Shared, show_backs, undoable);
    assert!(!copied.contains("UNMATCHED"), "the reference arm must match {id}");
    assert!(!shared.is_empty(), "{id} renders something");
    assert_eq!(shared, copied, "{id} must render byte-identically through shared_tab");
}

#[test]
fn the_citations_tab_renders_byte_identically() {
    assert_identical("citations", Some(ActionLabel::AttachCitation), true, true);
}

/// The singular-subject records (Event, Place, Media) pass `show_backs: false`, so the flag has to
/// survive the trip through `CitationsArm` rather than being fixed at the shared call site.
#[test]
fn the_citations_tab_renders_byte_identically_without_backs() {
    assert_identical("citations", Some(ActionLabel::AttachCitation), false, true);
}

#[test]
fn the_media_tab_renders_byte_identically() {
    assert_identical("media", Some(ActionLabel::AttachMedia), true, true);
}

#[test]
fn the_notes_tab_renders_byte_identically() {
    assert_identical("notes", Some(ActionLabel::AttachNote), true, true);
}

#[test]
fn the_tags_tab_renders_byte_identically() {
    assert_identical("tags", Some(ActionLabel::AddTag), true, true);
}

#[test]
fn the_history_tab_renders_byte_identically() {
    assert_identical("history", None, true, true);
}

/// Tag is the one aggregate with no retraction, so its ctx passes `on_undo: None`. That `Option`
/// has to reach `history_panel` unchanged through the shared arm.
#[test]
fn the_history_tab_renders_byte_identically_without_undo() {
    assert_identical("history", None, true, false);
}

/// `on_undo: None` is invisible in the markup: the entry's undo control is drawn from
/// `HistoryEntryVm::can_undo`, and the callback only decides whether pressing it does anything.
/// Asserting that here pins what the byte-identity comparison above can and cannot see — an
/// `on_undo` dropped on the floor by the shared arm would not show up as a rendering difference, so
/// the wiring is checked by reading the one call site rather than by SSR.
#[test]
fn the_undo_control_follows_can_undo_not_the_callback() {
    let wired = render("history", None, Shape::Shared, true, true);
    let inert = render("history", None, Shape::Shared, true, false);
    assert!(wired.contains("↩ Undo"), "an undoable entry offers Undo:\n{wired}");
    assert_eq!(wired, inert, "and the callback changes no rendered byte");
}

/// `shared_tab` is reached only from a dispatcher's `_` arm, so it must decline every tab a screen
/// owns — otherwise the fallthrough would swallow an id the screen means to handle itself.
#[test]
fn a_screen_owned_tab_is_not_a_shared_tab() {
    for id in ["names", "attributes", "participants", "segments", "overview"] {
        let html = render(id, Some(ActionLabel::AddFact), Shape::Shared, true, true);
        assert!(
            html.contains("UNMATCHED"),
            "{id} belongs to the screen and shared_tab must return None:\n{html}"
        );
    }
}

/// Source renders its own Citations tab — a reverse index over the citations *of* the source, a
/// different collection from the shared table's attached ones — so its ctx leaves `citations: None`
/// and the shared dispatch must decline the id even though the screen carries the other three.
#[test]
fn a_form_tab_the_ctx_omits_is_declined() {
    fn source_shaped() -> Element {
        let loc = loc();
        let tab = tab("citations", Some(ActionLabel::AttachCitation));
        let editing = use_signal(|| None::<TestForm>);
        let notes = note_rows();
        let tags = tag_rows();
        let history = history_rows();
        let ctx = SharedTabCtx {
            forms: Some(FormTabs {
                editing,
                citations: None,
                media: None,
                notes: Some(NotesArm {
                    form: TestForm::Note,
                    rows: &notes,
                    on_detach: Callback::new(|_: (String, String, bool)| {}),
                }),
                tags: Some(TagsArm {
                    form: TestForm::Tag,
                    rows: &tags,
                    on_remove: Callback::new(|_: (String, String)| {}),
                }),
            }),
            research_notes: None,
            history: &history,
            on_undo: Some(Callback::new(|_: String| {})),
        };
        match shared_tab(&loc, &tab, &ctx) {
            Some(element) => element,
            None => rsx! { div { "DECLINED" } },
        }
    }

    let mut vdom = VirtualDom::new(source_shaped);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains("DECLINED"),
        "a ctx without a Citations arm declines the tab:\n{html}"
    );
}

/// Tag carries no edit-form enum, no `editing` signal and no detach callbacks at all: its whole
/// participation is `forms: None`. History still has to resolve, and every form tab has to decline
/// without the screen constructing a dummy signal to say so.
#[test]
fn a_ctx_without_form_tabs_resolves_history_and_nothing_else() {
    #[component]
    fn TagShaped(id: &'static str) -> Element {
        let loc = loc();
        let tab = tab(id, None);
        let history = history_rows();
        let ctx = SharedTabCtx::<()> {
            forms: None,
            research_notes: None,
            history: &history,
            on_undo: None,
        };
        match shared_tab(&loc, &tab, &ctx) {
            Some(element) => element,
            None => rsx! { div { "DECLINED" } },
        }
    }

    let render_tag = |id: &'static str| {
        let mut vdom = VirtualDom::new_with_props(TagShaped, TagShapedProps { id });
        vdom.rebuild_in_place();
        dioxus_ssr::render(&vdom)
    };
    let history = render_tag("history");
    assert!(
        history.contains("Name asserted"),
        "History resolves without form tabs:\n{history}"
    );
    for id in ["citations", "media", "notes", "tags"] {
        let html = render_tag(id);
        assert!(html.contains("DECLINED"), "{id} declines without form tabs:\n{html}");
    }
}

/// The Research notes tab is the one shared tab whose body is a component (it reads `NavState` to
/// open a draft about this record). Under bare SSR it has no context and renders nothing, so the
/// assertion here is the dispatch: an arm-carrying ctx resolves the id, an arm-less one declines it.
#[test]
fn the_research_notes_tab_is_dispatched_only_when_the_ctx_carries_it() {
    #[component]
    fn ResearchShaped(carried: bool) -> Element {
        let loc = loc();
        let tab = tab("research-notes", Some(ActionLabel::NewResearchNote));
        let rows = research_note_rows();
        let history = history_rows();
        let ctx = SharedTabCtx::<()> {
            forms: None,
            research_notes: carried.then(|| ResearchNotesArm {
                category: Category::People,
                human_id: "I0001",
                rows: &rows,
            }),
            history: &history,
            on_undo: None,
        };
        match shared_tab(&loc, &tab, &ctx) {
            Some(_) => rsx! { div { "RESOLVED" } },
            None => rsx! { div { "DECLINED" } },
        }
    }

    let render_research = |carried: bool| {
        let mut vdom = VirtualDom::new_with_props(ResearchShaped, ResearchShapedProps { carried });
        vdom.rebuild_in_place();
        dioxus_ssr::render(&vdom)
    };
    assert!(render_research(true).contains("RESOLVED"), "a carried arm resolves");
    assert!(render_research(false).contains("DECLINED"), "an absent arm declines");
}

/// The tab strip's item is the tab's own id, label and count — the conversion 13 screens used to
/// spell out by hand.
#[test]
fn a_detail_tab_converts_to_its_strip_item() {
    let source = DetailTab {
        id: "citations",
        label: "Citations".to_owned(),
        count: Some(4),
        action: Some(ActionLabel::AttachCitation),
    };
    let item = TabItem::from(&source);
    assert_eq!(item.id, "citations");
    assert_eq!(item.label, "Citations");
    assert_eq!(item.count, Some(4));
}
