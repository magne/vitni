//! SSR assertions for the Explorer's *Columns* and sort menus (ADR 0052 §7, #679): the triggers name the
//! layout, the *Columns* menu ticks the shown columns in their order with a move-up and a move-down per
//! column, and the sort menu offers the `human_id` order and each sortable column with a direction.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_app::{ListPreferences, ListSort, ListedKind, SortDirection};
use vitni_ui::{ListLayout, Localizer};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::shell::list_columns::{ColumnsMenu, ListMenu, SortMenu};
use vitni_ui_dioxus::shell::{ChromeCtx, DataLocCtx};

fn contexts() {
    let language = "en".parse::<LanguageIdentifier>().unwrap_or_default();
    use_context_provider(|| ChromeCtx(Rc::new(Chrome::with_languages(None, std::slice::from_ref(&language)))));
    use_context_provider(|| DataLocCtx(Rc::new(Localizer::with_languages(None, &[language]))));
}

fn layout(sort: Option<ListSort>) -> ListLayout {
    let stored = ListPreferences {
        columns: Some(vec!["death".to_owned(), "sex".to_owned()]),
        sort,
    };
    ListLayout::resolve(ListedKind::Person, Some(&stored))
}

#[component]
fn Menus(open: Option<ListMenu>, sort: Option<ListSort>) -> Element {
    contexts();
    let layout = use_signal(move || layout(sort.clone()));
    let open = use_signal(move || open);
    rsx! {
        ColumnsMenu { layout, open }
        SortMenu { layout, open }
    }
}

fn render(open: Option<ListMenu>, sort: Option<ListSort>) -> String {
    let mut vdom = VirtualDom::new_with_props(Menus, MenusProps { open, sort });
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn birth_descending() -> ListSort {
    ListSort {
        column: "birth".to_owned(),
        direction: SortDirection::Descending,
    }
}

#[test]
fn closed_the_triggers_name_the_menus_and_the_sort() {
    let html = render(None, Some(birth_descending()));

    assert!(html.contains(">Columns<"), "the Columns trigger:\n{html}");
    assert!(
        html.contains("Sort: Birth ↓"),
        "the sort trigger names the sort:\n{html}"
    );
    assert!(
        html.contains(r#"aria-haspopup="menu""#),
        "the sort trigger opens a menu:\n{html}"
    );
    assert!(html.contains(r#"aria-expanded="false""#), "both closed:\n{html}");
    assert!(!html.contains(r#"class="list-menu""#), "no menu shown:\n{html}");
    assert!(render(None, None).contains("Sort: ID ↑"), "no sort is the id order");
}

#[test]
fn the_columns_menu_ticks_the_shown_columns_in_their_order() {
    let html = render(Some(ListMenu::Columns), None);

    assert!(html.contains(r#"class="list-menu""#), "the menu is open:\n{html}");
    assert!(html.contains(r#"aria-label="Columns""#), "it is named:\n{html}");
    let death = html.find(">Death<").expect("death offered");
    let sex = html.find(">Sex<").expect("sex offered");
    let given = html.find(">Given name<").expect("given offered");
    assert!(
        death < sex && sex < given,
        "shown columns first, in their order:\n{html}"
    );
    assert!(!html.contains(">Name<"), "the title column is never offered:\n{html}");
    for needle in [
        r#"aria-label="Move Death up""#,
        r#"aria-label="Move Sex down""#,
        r#"id="list-column-death""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?}:\n{html}");
    }
    assert_eq!(html.matches("checked").count(), 2, "two columns ticked:\n{html}");
}

#[test]
fn the_sort_menu_offers_the_id_order_each_sortable_column_and_a_direction() {
    let html = render(Some(ListMenu::Sort), Some(birth_descending()));

    assert!(html.contains(r#"role="menu""#), "the menu is open:\n{html}");
    for needle in [
        r#"role="menuitemradio""#,
        ">ID<",
        ">Surname<",
        ">Ascending<",
        ">Descending<",
    ] {
        assert!(html.contains(needle), "expected {needle:?}:\n{html}");
    }
    assert!(!html.contains(">Tags<"), "a tag set does not sort:\n{html}");
    assert_eq!(
        html.matches(r#"aria-checked="true""#).count(),
        2,
        "Birth and Descending are checked:\n{html}"
    );
    let id_order = render(Some(ListMenu::Sort), None);
    assert!(
        id_order.contains(r#"aria-disabled="true""#),
        "the id order has no direction to pick:\n{id_order}"
    );
}
