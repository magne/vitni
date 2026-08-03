//! SSR assertions for the map's zoom readout (#253): the level it prints, the class the toolbar CSS
//! keys off, its accessible name, and that both come from the Fluent catalogue rather than a literal.
//!
//! What SSR cannot reach here, and what `tests/gui-pass/map-zoom.toml` covers instead: that the readout
//! sits *in* either toolbar (the Geography toolbar needs `Services` + a reactive `Memo`, the Place
//! toolbar needs `AppCtx`), that the `NavigationControl`/`ScaleControl` render or move the camera, and
//! that a tile is still drawn at the camera's zoom ceiling. SSR renders no canvas and runs no script.

use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::screens::MapZoomReadout;
use genealogy_ui_dioxus::shell::ChromeCtx;
use unic_langid::LanguageIdentifier;

fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

/// Renders the readout at a fixed level under one language, the way a toolbar would.
fn readout(tag: &'static str, level: f64) -> String {
    #[component]
    fn Harness(tag: String, level: f64) -> Element {
        use_context_provider(|| ChromeCtx(chrome(&tag)));
        let zoom = use_signal(|| level);
        rsx! {
            MapZoomReadout { zoom }
        }
    }
    let mut vdom = VirtualDom::new_with_props(
        Harness,
        HarnessProps {
            tag: tag.to_owned(),
            level,
        },
    );
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn the_readout_prints_the_current_level_to_one_decimal() {
    let html = readout("en", 14.234);
    assert!(
        html.contains("z14.2"),
        "the level is shown, rounded to one decimal:\n{html}"
    );
}

#[test]
fn the_readout_carries_the_class_the_toolbar_sizes_it_by() {
    let html = readout("en", 14.2);
    assert!(
        html.contains(r#"class="map-zoom-readout""#),
        "the fixed-width tabular-figures class is what keeps the toolbar buttons still:\n{html}"
    );
}

/// `z14.2` alone is not a name — a screen reader needs to be told what the number measures.
#[test]
fn the_readout_names_itself_for_assistive_tech() {
    let html = readout("en", 14.2);
    assert!(
        html.contains(r#"aria-label="Zoom level 14.2""#),
        "the readout has an accessible name naming the level:\n{html}"
    );
}

/// ADR 0003: the readout's text is a Fluent message in every locale, not a literal.
#[test]
fn the_readouts_accessible_name_is_localized() {
    let html = readout("no", 14.2);
    assert!(
        html.contains("Zoomnivå 14.2"),
        "the Norwegian catalogue names the level in Norwegian:\n{html}"
    );
}

/// The camera is bounded to the zooms the tiles exist at, so a level past the ceiling can only be a
/// stale or nonsense reading — the readout still must not print one.
#[test]
fn the_readout_never_shows_a_level_the_camera_cannot_reach() {
    let html = readout("en", 25.0);
    assert!(
        html.contains("z19.0"),
        "an out-of-range level is shown clamped, not verbatim:\n{html}"
    );
}
