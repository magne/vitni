//! SSR assertions for the Explorer's *Saved* menu, the marked filter chip and the palette's *Saved
//! searches* group (ADR 0052 §8-10, #680), rendered over signals as `list_columns.rs` and
//! `list_filter.rs` do. The live behaviour (apply, a marked clause stopping the load) is in
//! `saved_search_state.rs` and the `saved-searches` gui-pass scenario.

#![expect(clippy::panic, reason = "a missing fixture aborts the test")]

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use uuid::Uuid;
use vitni_app::{ListedKind, SavedSearchId, SavedSearchList, SavedSearchSummary, UnreadableSavedSearch};
use vitni_ui::{ActiveFilter, Category, Localizer, PaletteEntry, StaleClauseVm};
use vitni_ui::{NameProblem, NamingMode, SavedNaming, name_problem};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::shell::list_columns::ListMenu;
use vitni_ui_dioxus::shell::list_filters::{FilterBar, FilterEditing};
use vitni_ui_dioxus::shell::nav_state::{NavState, Overlay};
use vitni_ui_dioxus::shell::palette::CommandPalette;
use vitni_ui_dioxus::shell::saved_menu::{SavedMenu, renamed_search};
use vitni_ui_dioxus::shell::{ChromeCtx, DataLocCtx};

fn contexts() {
    let language = "en".parse::<LanguageIdentifier>().unwrap_or_default();
    use_context_provider(|| ChromeCtx(Rc::new(Chrome::with_languages(None, std::slice::from_ref(&language)))));
    use_context_provider(|| DataLocCtx(Rc::new(Localizer::with_languages(None, &[language]))));
}

fn id(n: u128) -> SavedSearchId {
    SavedSearchId::from_uuid(Uuid::from_u128(n))
}

fn summary(n: u128, name: &str, kind: ListedKind) -> SavedSearchSummary {
    SavedSearchSummary {
        id: id(n),
        name: name.to_owned(),
        kind,
    }
}

fn list() -> SavedSearchList {
    SavedSearchList {
        searches: vec![
            summary(1, "Born before 1800", ListedKind::Person),
            summary(2, "Untagged places", ListedKind::Place),
            summary(3, "Fana families", ListedKind::Person),
        ],
        unreadable: Vec::new(),
    }
}

#[component]
fn Menu(
    open: Option<ListMenu>,
    applied: Option<(SavedSearchId, String)>,
    naming: Option<SavedNaming>,
    list: SavedSearchList,
) -> Element {
    contexts();
    let list = use_signal(move || list.clone());
    let applied = use_signal(move || applied.clone());
    let open = use_signal(move || open);
    let naming = use_signal(move || naming.clone());
    rsx! {
        SavedMenu { kind: ListedKind::Person, list, applied, open, naming }
    }
}

fn render_menu(
    open: Option<ListMenu>,
    applied: Option<(SavedSearchId, String)>,
    naming: Option<SavedNaming>,
    list: SavedSearchList,
) -> String {
    let mut vdom = VirtualDom::new_with_props(
        Menu,
        MenuProps {
            open,
            applied,
            naming,
            list,
        },
    );
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn applied_search() -> (SavedSearchId, String) {
    (id(1), "Born before 1800".to_owned())
}

/// The opening tag of the button that carries `label`, up to the label.
fn button_of<'a>(html: &'a str, label: &str) -> &'a str {
    let at = html
        .find(&format!(">{label}<"))
        .unwrap_or_else(|| panic!("no {label:?} in:\n{html}"));
    let start = html[..at]
        .rfind("<button")
        .unwrap_or_else(|| panic!("{label:?} is not in a button:\n{html}"));
    &html[start..at]
}

#[test]
fn the_trigger_is_an_icon_button_with_an_empty_star_when_nothing_is_applied() {
    let html = render_menu(None, None, None, list());

    assert!(html.contains("icon-btn"), "an icon button, not a text button:\n{html}");
    assert!(html.contains(">☆<"), "an empty star while nothing is applied:\n{html}");
    assert!(!html.contains(">★<"), "{html}");
    assert!(
        html.contains(r#"aria-label="Saved searches""#),
        "named for assistive tech:\n{html}"
    );
    assert!(html.contains(r#"title="Saved searches""#), "and by tooltip:\n{html}");
    assert!(html.contains(r#"aria-expanded="false""#), "closed:\n{html}");
    assert!(!html.contains(r#"role="menu""#), "no menu while closed:\n{html}");
}

#[test]
fn the_trigger_shows_a_filled_star_while_a_saved_search_is_applied() {
    let html = render_menu(None, Some(applied_search()), None, list());

    assert!(html.contains(">★<"), "a filled star:\n{html}");
    assert!(!html.contains(">☆<"), "{html}");
}

#[test]
fn the_open_menu_lists_this_kinds_searches_then_the_actions() {
    let html = render_menu(Some(ListMenu::Saved), None, None, list());

    assert!(
        html.contains(r#"data-hook="saved-menu""#),
        "the hook the gui-pass uses:\n{html}"
    );
    assert!(html.contains(r#"role="menu""#), "an open menu:\n{html}");
    assert!(
        !html.contains("Untagged places"),
        "another kind's search is not offered:\n{html}"
    );
    let order: Vec<usize> = [
        ">Born before 1800<",
        ">Fana families<",
        ">Save as new<",
        ">Update<",
        ">Rename<",
        ">Delete<",
    ]
    .iter()
    .map(|needle| {
        html.find(needle)
            .unwrap_or_else(|| panic!("expected {needle:?}:\n{html}"))
    })
    .collect();
    assert!(order.is_sorted(), "searches first, then the actions in order:\n{html}");
    assert!(!html.contains("0190"), "no id is shown:\n{html}");
}

#[test]
fn update_rename_and_delete_are_disabled_until_a_search_is_applied() {
    let none = render_menu(Some(ListMenu::Saved), None, None, list());
    for label in ["Update", "Rename", "Delete"] {
        assert!(
            button_of(&none, label).contains(r#"aria-disabled="true""#),
            "{label} is disabled:\n{none}"
        );
    }
    assert!(
        !button_of(&none, "Save as new").contains(r#"aria-disabled="true""#),
        "Save as new always works:\n{none}"
    );

    let applied = render_menu(Some(ListMenu::Saved), Some(applied_search()), None, list());
    for label in ["Update", "Rename", "Delete"] {
        assert!(
            !button_of(&applied, label).contains(r#"aria-disabled="true""#),
            "{label} is offered:\n{applied}"
        );
    }
}

#[test]
fn an_unreadable_entry_is_listed_disabled() {
    let mut with_unreadable = list();
    with_unreadable.unreadable.push(UnreadableSavedSearch {
        position: 3,
        name: Some("Old search".to_owned()),
    });
    let html = render_menu(Some(ListMenu::Saved), None, None, with_unreadable);

    assert!(
        button_of(&html, "Old search: cannot be read").contains(r#"aria-disabled="true""#),
        "cannot be applied, and says why:\n{html}"
    );
}

#[test]
fn a_nameless_unreadable_entry_is_labelled_by_its_zero_based_toml_position() {
    let mut with_unreadable = list();
    with_unreadable.unreadable.push(UnreadableSavedSearch {
        position: 2,
        name: None,
    });
    let html = render_menu(Some(ListMenu::Saved), None, None, with_unreadable);

    assert!(
        button_of(&html, "saved_searches[2]: cannot be read").contains(r#"aria-disabled="true""#),
        "the label matches the CLI's:\n{html}"
    );
}

#[test]
fn save_as_new_opens_an_inline_name_field_with_save_and_cancel() {
    let naming = SavedNaming {
        mode: NamingMode::New,
        name: String::new(),
        problem: None,
    };
    let html = render_menu(Some(ListMenu::Saved), None, Some(naming), list());

    assert!(html.contains(r#"id="saved-name""#), "the name field:\n{html}");
    assert!(html.contains(">Save<"), "{html}");
    assert!(html.contains(">Cancel<"), "{html}");
    assert!(
        !html.contains(r#"aria-invalid="true""#),
        "no error before one is found:\n{html}"
    );
    assert!(!html.contains(r#"id="saved-name-error""#), "{html}");
}

#[test]
fn rename_opens_the_field_on_the_current_name() {
    let naming = SavedNaming {
        mode: NamingMode::Rename,
        name: "Born before 1800".to_owned(),
        problem: None,
    };
    let html = render_menu(Some(ListMenu::Saved), Some(applied_search()), Some(naming), list());

    assert!(html.contains(r#"id="saved-name""#), "{html}");
    assert!(html.contains(r#"value="Born before 1800""#), "prefilled:\n{html}");
}

#[test]
fn an_empty_or_duplicate_name_shows_the_inline_error_on_the_field() {
    for problem in [NameProblem::Empty, NameProblem::Duplicate] {
        let naming = SavedNaming {
            mode: NamingMode::New,
            name: "Fana families".to_owned(),
            problem: Some(problem),
        };
        let html = render_menu(Some(ListMenu::Saved), None, Some(naming), list());

        assert!(
            html.contains(r#"aria-invalid="true""#),
            "{problem:?} marks the field:\n{html}"
        );
        assert!(
            html.contains(r#"aria-describedby="saved-name-error""#),
            "{problem:?} links the field to its message:\n{html}"
        );
        assert!(
            html.contains(r#"id="saved-name-error""#),
            "{problem:?} shows the message:\n{html}"
        );
    }
}

#[test]
fn a_name_is_refused_when_empty_or_taken_ignoring_case() {
    let list = list();

    assert_eq!(name_problem(&list, "", None), Some(NameProblem::Empty));
    assert_eq!(name_problem(&list, "   ", None), Some(NameProblem::Empty));
    assert_eq!(
        name_problem(&list, "  fana FAMILIES ", None),
        Some(NameProblem::Duplicate)
    );
    assert_eq!(name_problem(&list, "Brand new", None), None);
}

#[test]
fn a_name_held_by_an_unreadable_entry_is_taken() {
    let mut list = list();
    list.unreadable.push(UnreadableSavedSearch {
        position: 0,
        name: Some("Broken".into()),
    });

    assert_eq!(name_problem(&list, " broken ", None), Some(NameProblem::Duplicate));
}

#[test]
fn renaming_a_search_to_its_own_name_is_not_a_duplicate() {
    let list = list();

    assert_eq!(name_problem(&list, "born BEFORE 1800", Some(id(1))), None);
    assert_eq!(
        name_problem(&list, "Fana families", Some(id(1))),
        Some(NameProblem::Duplicate),
        "but another search's name still is"
    );
}

#[test]
fn only_a_rename_exempts_the_applied_search_from_the_duplicate_check() {
    let applied = (id(1), "Born before 1800".to_owned());

    assert_eq!(renamed_search(NamingMode::Rename, Some(&applied)), Some(id(1)));
    assert_eq!(renamed_search(NamingMode::New, Some(&applied)), None);
    assert_eq!(renamed_search(NamingMode::Rename, None), None);
}

#[component]
fn Bar(stale: Vec<StaleClauseVm>) -> Element {
    contexts();
    let filters = use_signal(Vec::<ActiveFilter>::new);
    let stale = use_signal(move || stale.clone());
    let editing = use_signal(|| None::<FilterEditing>);
    rsx! {
        FilterBar { kind: ListedKind::Person, filters, stale, editing }
    }
}

fn render_bar(stale: Vec<StaleClauseVm>) -> String {
    let mut vdom = VirtualDom::new_with_props(Bar, BarProps { stale });
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn a_marked_clause_is_a_marked_chip_that_edits_and_removes() {
    let label = "Shoe size: not a column of this list".to_owned();
    let html = render_bar(vec![StaleClauseVm {
        column: "shoe size".to_owned(),
        label: label.clone(),
    }]);

    assert!(
        html.contains("chip stale"),
        "marked apart from a working clause:\n{html}"
    );
    assert!(html.contains(&label), "{html}");
    for needle in [
        format!(r#"aria-label="Edit filter: {label}""#),
        format!(r#"aria-label="Remove filter: {label}""#),
    ] {
        assert!(html.contains(&needle), "expected {needle:?}:\n{html}");
    }
    assert!(!html.contains("0190"), "no id in the markup:\n{html}");
}

#[test]
fn no_marked_clause_leaves_no_marked_chip() {
    let html = render_bar(Vec::new());

    assert!(!html.contains("chip stale"), "{html}");
}

fn palette_with_saved() -> Element {
    contexts();
    let mut nav = use_context_provider(NavState::new);
    use_hook(|| {
        nav.saved_search_entries.set(vec![
            PaletteEntry::SavedSearch {
                id: id(1),
                name: "Born before 1800".to_owned(),
                category: Category::People,
            },
            PaletteEntry::SavedSearch {
                id: id(2),
                name: "Untagged places".to_owned(),
                category: Category::Places,
            },
        ]);
        nav.overlay.set(Overlay::Palette);
    });
    rsx! {
        CommandPalette {}
    }
}

#[test]
fn the_palette_lists_saved_searches_with_their_kind() {
    let mut vdom = VirtualDom::new(palette_with_saved);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Saved searches"), "the group heading:\n{html}");
    assert!(html.contains("Born before 1800"), "{html}");
    assert!(html.contains("Untagged places"), "{html}");
    let places = html.find("Untagged places").unwrap_or_default();
    assert!(
        html[places..].contains(r#"class="p-kind">Places<"#),
        "the kind's rail label sits beside the name:\n{html}"
    );
    let commands = html.find("Commands").unwrap_or_default();
    let saved = html.find("Saved searches").unwrap_or_default();
    assert!(commands < saved, "after Commands:\n{html}");
}

#[test]
fn star_is_filled_while_a_search_is_applied_before_the_list_has_loaded() {
    let html = render_menu(None, Some(applied_search()), None, SavedSearchList::default());
    assert!(html.contains(">★<"), "{html}");
}
