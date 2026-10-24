//! Gate for #714: the place mockup's Sort and Columns menus list the Kind and Level columns the place
//! list ships beside the text Type column, labelled Kind and Level.

#![expect(clippy::panic, reason = "tests abort on a missing mockup section")]

use std::fs;
use std::path::PathBuf;

fn place_mockup() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/mockups/place.html");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("reading {}: {error}", path.display()))
}

/// The sort menu's markup: from its `Change sort order` menu to its Descending entry.
fn sort_menu(html: &str) -> &str {
    let start = html
        .find("role=\"menu\" aria-label=\"Change sort order\"")
        .unwrap_or_else(|| panic!("place.html has no sort menu"));
    let rest = &html[start..];
    &rest[..rest.find("Descending").unwrap_or(rest.len())]
}

#[test]
fn the_sort_menu_lists_kind_and_level_with_the_shipped_labels() {
    let html = place_mockup();
    let menu = sort_menu(&html);
    for label in ["Kind", "Level"] {
        assert!(
            menu.contains(&format!("<span>{label}</span>")),
            "the Sort menu lacks {label}: {menu}"
        );
    }
}

#[test]
fn the_columns_menu_lists_kind_and_level_with_the_shipped_labels() {
    let html = place_mockup();
    for (column, label) in [("kind", "Kind"), ("level", "Level")] {
        let entry = format!("id=\"list-column-{column}\"");
        let at = html
            .find(&entry)
            .unwrap_or_else(|| panic!("the Columns menu lacks {entry}"));
        let row = &html[at..at + html[at..].find("</div>").unwrap_or(0)];
        assert!(row.contains(&format!("<span>{label}</span>")), "{row}");
    }
}
