//! Targets: how a scenario names the element a step acts on or an assertion measures, and how a
//! [`Snapshot`] of the running GUI's elements resolves one to window pixels.
//!
//! A scenario never spells a window pixel. A step's `at`/`from` is a [`Point`] — an element and, for a
//! canvas, an offset into it — and an assertion's `region` is an [`Area`]. Both name the element by its
//! DOM `id`, its `data-hook` attribute or its ARIA `role` (a button by its visible text, as a user
//! finds it), optionally narrowed by `text`, an enclosing `within` element, and an `index` among what
//! is left (see [`Matcher`]). The GUI's probe (see [`super::probe`]) reports every such element, and
//! resolution is this module's pure functions over that list, so a layout change moves the coordinates
//! without touching a scenario.

use std::fmt;

use serde::{Deserialize, Serialize};

/// One element the probe reported, its rect already in window pixels (see [`Snapshot`]).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Element {
    /// The element's DOM `id`, if it has one.
    pub id: Option<String>,
    /// The element's `data-hook` attribute, if it has one.
    pub hook: Option<String>,
    /// Its ARIA role: the `role` attribute, else the one its tag implies (`button`, `link`, `textbox`…).
    #[serde(default)]
    pub role: Option<String>,
    /// Its rendered text, whitespace collapsed and truncated by the probe.
    #[serde(default)]
    pub text: String,
    /// Its `aria-label` (or `title`), for an icon button with no text of its own.
    #[serde(default)]
    pub label: String,
    /// `[x, y, width, height]`.
    pub rect: [f64; 4],
    /// The ids, hooks and roles (explicit or implied) of its ancestors, nearest first.
    #[serde(default)]
    pub within: Vec<String>,
}

/// What the probe saw at one moment: whether the page is up and focused, and every hooked element.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Snapshot {
    /// The document has loaded and the app's root has rendered.
    pub ready: bool,
    /// The document has keyboard focus (`document.hasFocus()`).
    pub focused: bool,
    /// Every map container on the page holds a `MapLibre` map that has loaded its style and sources and
    /// is not moving — nothing queued that has yet to reach its canvas. True when no map is on screen.
    pub maps_idle: bool,
    /// The focused element's `id`, else its hook, else its role, else its tag name (`body`).
    #[serde(default)]
    pub active: Option<String>,
    /// The webview's `[innerWidth, innerHeight]`.
    pub viewport: [f64; 2],
    pub elements: Vec<Element>,
}

impl Snapshot {
    /// Moves every rect from viewport into window pixels for a window `height` pixels tall.
    ///
    /// Dioxus attaches a GTK menu bar above the webview on Linux, inside the window's client area, so
    /// the viewport starts that bar's height below the window's top edge; nothing sits beside it.
    #[must_use]
    pub fn in_window(self, height: u32) -> Self {
        let shift = f64::from(height) - self.viewport[1];
        let mut elements = Vec::new();
        for mut element in self.elements {
            element.rect[1] += shift;
            elements.push(element);
        }
        Self { elements, ..self }
    }
}

/// Which attribute a [`Matcher`] names its element by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum By {
    /// A DOM `id`.
    Id(String),
    /// A `data-hook` attribute value.
    Hook(String),
    /// An ARIA role, explicit or implied by the tag.
    Role(String),
}

/// The element a target names: an `id`, a `hook` or a `role`, narrowed by `text`, `within` and `index`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matcher {
    pub by: By,
    /// A substring of the element's text or `aria-label`.
    pub text: Option<String>,
    /// An `id`, hook or role (explicit or implied) one of the element's ancestors carries.
    pub within: Option<String>,
    /// Which match to take, from 0 in document order, when more than one is left.
    pub index: Option<usize>,
}

impl fmt::Display for Matcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.by {
            By::Id(id) => write!(f, "{{id = {id:?}")?,
            By::Hook(hook) => write!(f, "{{hook = {hook:?}")?,
            By::Role(role) => write!(f, "{{role = {role:?}")?,
        }
        if let Some(text) = &self.text {
            write!(f, ", text = {text:?}")?;
        }
        if let Some(within) = &self.within {
            write!(f, ", within = {within:?}")?;
        }
        if let Some(index) = self.index {
            write!(f, ", index = {index}")?;
        }
        write!(f, "}}")
    }
}

/// A step's point: the centre of an element, or `offset` pixels from its top-left corner.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawPoint")]
pub struct Point {
    pub matcher: Matcher,
    pub offset: Option<[i32; 2]>,
}

/// An assertion's region: an element's rect, or the `size` sub-rectangle `offset` pixels into it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawArea")]
pub struct Area {
    pub matcher: Matcher,
    pub offset: Option<[i32; 2]>,
    pub size: Option<[u32; 2]>,
}

/// A point's TOML shape, checked into a [`Point`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPoint {
    id: Option<String>,
    hook: Option<String>,
    role: Option<String>,
    text: Option<String>,
    within: Option<String>,
    index: Option<usize>,
    offset: Option<[i32; 2]>,
}

/// An area's TOML shape, checked into an [`Area`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArea {
    id: Option<String>,
    hook: Option<String>,
    role: Option<String>,
    text: Option<String>,
    within: Option<String>,
    index: Option<usize>,
    offset: Option<[i32; 2]>,
    size: Option<[u32; 2]>,
}

impl TryFrom<RawPoint> for Point {
    type Error = String;

    fn try_from(raw: RawPoint) -> Result<Self, String> {
        let by = by(raw.id, raw.hook, raw.role)?;
        let matcher = Matcher {
            by,
            text: raw.text,
            within: raw.within,
            index: raw.index,
        };
        Ok(Self {
            matcher,
            offset: raw.offset,
        })
    }
}

impl TryFrom<RawArea> for Area {
    type Error = String;

    fn try_from(raw: RawArea) -> Result<Self, String> {
        let by = by(raw.id, raw.hook, raw.role)?;
        let matcher = Matcher {
            by,
            text: raw.text,
            within: raw.within,
            index: raw.index,
        };
        Ok(Self {
            matcher,
            offset: raw.offset,
            size: raw.size,
        })
    }
}

/// Exactly one of `id`, `hook` and `role`.
fn by(id: Option<String>, hook: Option<String>, role: Option<String>) -> Result<By, String> {
    match (id, hook, role) {
        (Some(id), None, None) => Ok(By::Id(id)),
        (None, Some(hook), None) => Ok(By::Hook(hook)),
        (None, None, Some(role)) => Ok(By::Role(role)),
        _ => Err("a target names its element by exactly one of `id`, `hook` and `role`".to_owned()),
    }
}

/// The one element `matcher` names in `elements`, or why there is none.
///
/// # Errors
///
/// Fails, listing what the probe did report, when nothing matches, when several do and no `index`
/// picks one, or when `index` is past the last match.
pub fn find<'a>(matcher: &Matcher, elements: &'a [Element]) -> Result<&'a Element, String> {
    let mut named = Vec::new();
    for element in elements {
        let carries = match &matcher.by {
            By::Id(id) => element.id.as_deref() == Some(id.as_str()),
            By::Hook(hook) => element.hook.as_deref() == Some(hook.as_str()),
            By::Role(role) => element.role.as_deref() == Some(role.as_str()),
        };
        if carries {
            named.push(element);
        }
    }
    let mut matches = Vec::new();
    for element in &named {
        if narrowed(matcher, element) {
            matches.push(*element);
        }
    }
    match (matches.as_slice(), matcher.index) {
        ([], _) => Err(format!("no element matches {matcher}; {}", listing(&named))),
        ([only], None) => Ok(only),
        (several, None) => Err(format!(
            "{} elements match {matcher} — narrow it with `text`, `within` or `index`; {}",
            several.len(),
            listing(several)
        )),
        (all, Some(index)) => all.get(index).copied().ok_or_else(|| {
            format!(
                "{matcher} asks for index {index}, but {} match; {}",
                all.len(),
                listing(all)
            )
        }),
    }
}

/// Whether `element` passes `matcher`'s `text` and `within`.
fn narrowed(matcher: &Matcher, element: &Element) -> bool {
    let text = matcher
        .text
        .as_deref()
        .is_none_or(|text| element.text.contains(text) || element.label.contains(text));
    let within = matcher
        .within
        .as_deref()
        .is_none_or(|within| element.within.iter().any(|ancestor| ancestor == within));
    text && within
}

/// How a failure lists the elements it chose among.
fn listing(elements: &[&Element]) -> String {
    if elements.is_empty() {
        return "the probe reported no element with that id, hook or role".to_owned();
    }
    let mut seen = Vec::new();
    for element in elements {
        let [x, y, w, h] = element.rect;
        let shown = if element.text.is_empty() {
            &element.label
        } else {
            &element.text
        };
        seen.push(format!("{shown:?} at {w:.0}x{h:.0}+{x:.0}+{y:.0}"));
    }
    format!("the probe saw: {}", seen.join(", "))
}

/// The window pixel `point` lands on, for a window of `window` size.
///
/// # Errors
///
/// Fails as [`find`] does, or when the point falls outside its element or the window.
pub fn resolve_point(point: &Point, elements: &[Element], window: (u32, u32)) -> Result<[i32; 2], String> {
    let element = find(&point.matcher, elements)?;
    let [x, y, w, h] = element.rect;
    let (px, py) = match point.offset {
        Some([dx, dy]) => {
            if f64::from(dx) >= w || f64::from(dy) >= h || dx < 0 || dy < 0 {
                return Err(format!(
                    "offset [{dx}, {dy}] lands outside {} ({w:.0}x{h:.0})",
                    point.matcher
                ));
            }
            (x + f64::from(dx), y + f64::from(dy))
        }
        None => (x + w / 2.0, y + h / 2.0),
    };
    let (px, py) = (pixel(px), pixel(py));
    if px < 0 || py < 0 || px >= signed(window.0) || py >= signed(window.1) {
        return Err(format!(
            "{} resolves to [{px}, {py}], outside the {}x{} window",
            point.matcher, window.0, window.1
        ));
    }
    Ok([px, py])
}

/// The `[x, y, w, h]` window rectangle `area` covers, clipped to a window of `window` size.
///
/// # Errors
///
/// Fails as [`find`] does, or when nothing of the area is left inside the window.
pub fn resolve_area(area: &Area, elements: &[Element], window: (u32, u32)) -> Result<[u32; 4], String> {
    let element = find(&area.matcher, elements)?;
    let [x, y, w, h] = element.rect;
    let [dx, dy] = area.offset.unwrap_or([0, 0]);
    let left = pixel(x) + dx;
    let top = pixel(y) + dy;
    let (width, height) = match area.size {
        Some([width, height]) => (signed(width), signed(height)),
        None => (pixel(w) - dx, pixel(h) - dy),
    };
    let right = (left + width).min(signed(window.0));
    let bottom = (top + height).min(signed(window.1));
    let (left, top) = (left.max(0), top.max(0));
    if right <= left || bottom <= top {
        return Err(format!(
            "{} covers nothing inside the {}x{} window",
            area.matcher, window.0, window.1
        ));
    }
    Ok([
        unsigned(left),
        unsigned(top),
        unsigned(right - left),
        unsigned(bottom - top),
    ])
}

/// A probe coordinate rounded to the nearest pixel.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a window coordinate is far inside i32; the probe reports CSS pixels of an on-screen window"
)]
fn pixel(value: f64) -> i32 {
    value.round() as i32
}

/// A window dimension as a signed pixel count, saturating far past any real window.
fn signed(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

/// A clipped, non-negative pixel count.
fn unsigned(value: i32) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{Area, By, Element, Matcher, Point, Snapshot, find, resolve_area, resolve_point};

    const WINDOW: (u32, u32) = (1800, 1200);

    fn element(id: Option<&str>, hook: Option<&str>, text: &str, rect: [f64; 4]) -> Element {
        Element {
            id: id.map(str::to_owned),
            hook: hook.map(str::to_owned),
            text: text.to_owned(),
            label: String::new(),
            rect,
            within: Vec::new(),
            role: None,
        }
    }

    fn hooked(hook: &str, text: &str, rect: [f64; 4]) -> Element {
        element(None, Some(hook), text, rect)
    }

    fn matcher(by: By) -> Matcher {
        Matcher {
            by,
            text: None,
            within: None,
            index: None,
        }
    }

    fn point(toml: &str) -> Point {
        #[derive(serde::Deserialize)]
        struct Holder {
            at: Point,
        }
        let holder: Holder = toml::from_str(toml).expect("the point parses");
        holder.at
    }

    fn area(toml: &str) -> Area {
        #[derive(serde::Deserialize)]
        struct Holder {
            region: Area,
        }
        let holder: Holder = toml::from_str(toml).expect("the area parses");
        holder.region
    }

    fn parse_error(toml: &str) -> String {
        #[derive(Debug, serde::Deserialize)]
        struct Holder {
            #[expect(dead_code, reason = "only the parse failure is under test")]
            at: Point,
        }
        let error = toml::from_str::<Holder>(toml).expect_err("the point must not parse");
        error.to_string()
    }

    fn rail() -> Vec<Element> {
        vec![
            hooked("rail-item", "Dashboard", [0.0, 120.0, 230.0, 30.0]),
            hooked("rail-item", "People 2", [0.0, 160.0, 230.0, 30.0]),
            hooked("rail-item", "Places 1", [0.0, 268.0, 230.0, 30.0]),
        ]
    }

    #[test]
    fn a_point_parses_by_id_or_by_hook_with_its_narrowing_fields() {
        let by_id = point(r#"at = { id = "geography-map", offset = [400, 300] }"#);
        assert_eq!(by_id.matcher.by, By::Id("geography-map".to_owned()));
        assert_eq!(by_id.offset, Some([400, 300]));
        let by_hook = point(r#"at = { hook = "button", text = "Save", within = "side-panel", index = 1 }"#);
        assert_eq!(by_hook.matcher.by, By::Hook("button".to_owned()));
        assert_eq!(by_hook.matcher.text.as_deref(), Some("Save"));
        assert_eq!(by_hook.matcher.within.as_deref(), Some("side-panel"));
        assert_eq!(by_hook.matcher.index, Some(1));
        assert_eq!(by_hook.offset, None);
    }

    #[test]
    fn a_point_names_exactly_one_of_id_hook_and_role() {
        assert!(parse_error(r#"at = { id = "a", hook = "b" }"#).contains("exactly one"));
        assert!(parse_error(r#"at = { hook = "a", role = "button" }"#).contains("exactly one"));
        assert!(parse_error(r#"at = { text = "Save" }"#).contains("exactly one"));
    }

    #[test]
    fn a_role_finds_an_element_by_its_aria_role_and_text() {
        let mut save = element(None, None, "Save", [1729.0, 139.0, 51.0, 24.0]);
        save.role = Some("button".to_owned());
        let mut cancel = element(None, None, "Cancel", [1657.0, 139.0, 60.0, 24.0]);
        cancel.role = Some("button".to_owned());
        let elements = vec![cancel, save];
        let target = point(r#"at = { role = "button", text = "Save" }"#);
        assert_eq!(target.matcher.by, By::Role("button".to_owned()));
        assert_eq!(resolve_point(&target, &elements, WINDOW), Ok([1755, 151]));
        assert!(target.matcher.to_string().contains("role = \"button\""));
    }

    #[test]
    fn within_also_names_an_ancestors_role() {
        let mut dialog = element(None, None, "Save", [1000.0, 300.0, 60.0, 24.0]);
        dialog.role = Some("button".to_owned());
        dialog.within = vec!["dialog".to_owned()];
        let mut header = element(None, None, "Save", [1729.0, 139.0, 51.0, 24.0]);
        header.role = Some("button".to_owned());
        let elements = vec![header, dialog];
        let target = point(r#"at = { role = "button", text = "Save", within = "dialog" }"#);
        assert_eq!(resolve_point(&target, &elements, WINDOW), Ok([1030, 312]));
    }

    #[test]
    fn a_misspelt_target_key_is_rejected() {
        assert!(parse_error(r#"at = { hook = "button", txet = "Save" }"#).contains("txet"));
        assert!(
            parse_error(r#"at = { hook = "button", size = [1, 1] }"#).contains("size"),
            "a point has no size"
        );
    }

    #[test]
    fn an_area_parses_its_offset_and_size() {
        let parsed = area(r#"region = { id = "place-map", offset = [10, 20], size = [200, 40] }"#);
        assert_eq!(parsed.matcher.by, By::Id("place-map".to_owned()));
        assert_eq!(parsed.offset, Some([10, 20]));
        assert_eq!(parsed.size, Some([200, 40]));
    }

    #[test]
    fn an_id_finds_its_element() {
        let elements = vec![
            element(Some("global-search"), None, "", [900.0, 50.0, 300.0, 30.0]),
            element(Some("geography-map"), None, "", [500.0, 200.0, 1250.0, 700.0]),
        ];
        let found = find(&matcher(By::Id("geography-map".to_owned())), &elements).expect("found");
        assert_eq!(found.rect, [500.0, 200.0, 1250.0, 700.0]);
    }

    #[test]
    fn text_picks_one_of_a_hooks_elements_by_substring() {
        let elements = rail();
        let people = Matcher {
            text: Some("People".to_owned()),
            ..matcher(By::Hook("rail-item".to_owned()))
        };
        assert_eq!(find(&people, &elements).expect("found").text, "People 2");
    }

    #[test]
    fn text_also_matches_an_aria_label() {
        let mut close = hooked("record-tab-close", "", [700.0, 95.0, 16.0, 16.0]);
        close.label = "Close Kristiansand".to_owned();
        let elements = vec![close];
        let target = Matcher {
            text: Some("Kristiansand".to_owned()),
            ..matcher(By::Hook("record-tab-close".to_owned()))
        };
        assert!(find(&target, &elements).is_ok());
    }

    #[test]
    fn within_keeps_only_elements_inside_the_named_ancestor() {
        let mut header = hooked("button", "Save", [1700.0, 140.0, 60.0, 24.0]);
        header.within = vec!["detail-header".to_owned(), "main".to_owned()];
        let mut panel = hooked("button", "Save", [1440.0, 700.0, 60.0, 24.0]);
        panel.within = vec!["side-panel".to_owned(), "main".to_owned()];
        let elements = vec![header, panel];
        let target = Matcher {
            text: Some("Save".to_owned()),
            within: Some("side-panel".to_owned()),
            ..matcher(By::Hook("button".to_owned()))
        };
        assert_eq!(find(&target, &elements).expect("found").rect[0], 1440.0);
    }

    #[test]
    fn several_matches_without_an_index_is_an_error_listing_them() {
        let error = find(&matcher(By::Hook("rail-item".to_owned())), &rail()).expect_err("ambiguous");
        assert!(error.contains("3 elements match"), "{error}");
        assert!(error.contains("People 2"), "the candidates are listed: {error}");
        assert!(error.contains("{hook = \"rail-item\"}"), "the target is named: {error}");
    }

    #[test]
    fn an_index_picks_among_matches_in_document_order() {
        let second = Matcher {
            index: Some(1),
            ..matcher(By::Hook("rail-item".to_owned()))
        };
        assert_eq!(find(&second, &rail()).expect("found").text, "People 2");
        let past = Matcher {
            index: Some(3),
            ..matcher(By::Hook("rail-item".to_owned()))
        };
        let error = find(&past, &rail()).expect_err("past the end");
        assert!(error.contains("index 3") && error.contains("3 match"), "{error}");
    }

    #[test]
    fn nothing_matching_is_an_error_naming_the_target_and_what_the_probe_saw() {
        let target = Matcher {
            text: Some("Sources".to_owned()),
            ..matcher(By::Hook("rail-item".to_owned()))
        };
        let error = find(&target, &rail()).expect_err("no such item");
        assert!(error.contains("no element matches"), "{error}");
        assert!(error.contains("text = \"Sources\""), "{error}");
        assert!(error.contains("Places 1"), "the hook's elements are listed: {error}");
    }

    #[test]
    fn a_point_without_offset_is_the_elements_rounded_centre() {
        let target = point(r#"at = { hook = "rail-item", text = "People" }"#);
        assert_eq!(resolve_point(&target, &rail(), WINDOW), Ok([115, 175]));
    }

    #[test]
    fn an_offset_counts_from_the_elements_top_left() {
        let elements = vec![element(Some("geography-map"), None, "", [500.0, 200.0, 1250.0, 700.0])];
        let target = point(r#"at = { id = "geography-map", offset = [400, 300] }"#);
        assert_eq!(resolve_point(&target, &elements, WINDOW), Ok([900, 500]));
    }

    #[test]
    fn an_offset_past_the_element_is_an_error() {
        let elements = vec![element(Some("geography-map"), None, "", [500.0, 200.0, 1250.0, 700.0])];
        let target = point(r#"at = { id = "geography-map", offset = [1300, 10] }"#);
        let error = resolve_point(&target, &elements, WINDOW).expect_err("outside the canvas");
        assert!(error.contains("outside"), "{error}");
    }

    #[test]
    fn a_point_outside_the_window_is_an_error() {
        let elements = vec![hooked("explorer-row", "Kristiansand", [232.0, 1300.0, 300.0, 40.0])];
        let target = point(r#"at = { hook = "explorer-row" }"#);
        let error = resolve_point(&target, &elements, WINDOW).expect_err("scrolled out of view");
        assert!(error.contains("outside the 1800x1200 window"), "{error}");
    }

    #[test]
    fn an_area_without_offset_is_the_elements_rect() {
        let elements = vec![element(Some("geography-map"), None, "", [500.4, 199.6, 1250.0, 700.0])];
        let target = area(r#"region = { id = "geography-map" }"#);
        assert_eq!(resolve_area(&target, &elements, WINDOW), Ok([500, 200, 1250, 700]));
    }

    #[test]
    fn an_area_offset_and_size_cut_a_sub_rectangle() {
        let elements = vec![element(Some("geography-map"), None, "", [500.0, 200.0, 1250.0, 700.0])];
        let sized = area(r#"region = { id = "geography-map", offset = [10, 20], size = [200, 40] }"#);
        assert_eq!(resolve_area(&sized, &elements, WINDOW), Ok([510, 220, 200, 40]));
        let rest = area(r#"region = { id = "geography-map", offset = [50, 100] }"#);
        assert_eq!(
            resolve_area(&rest, &elements, WINDOW),
            Ok([550, 300, 1200, 600]),
            "no size runs to the element's far corner"
        );
    }

    #[test]
    fn an_area_is_clipped_to_the_window() {
        let elements = vec![hooked("detail-pane", "", [532.0, 130.0, 1400.0, 1200.0])];
        let target = area(r#"region = { hook = "detail-pane" }"#);
        assert_eq!(resolve_area(&target, &elements, WINDOW), Ok([532, 130, 1268, 1070]));
        let gone = vec![hooked("detail-pane", "", [1900.0, 130.0, 100.0, 100.0])];
        assert!(
            resolve_area(&target, &gone, WINDOW).is_err(),
            "nothing left inside the window"
        );
    }

    #[test]
    fn a_snapshot_moves_rects_below_the_menu_bar() {
        let snapshot = Snapshot {
            ready: true,
            focused: true,
            maps_idle: true,
            active: None,
            viewport: [1800.0, 1170.0],
            elements: rail(),
        };
        let moved = snapshot.in_window(1200);
        assert_eq!(moved.elements[1].rect, [0.0, 190.0, 230.0, 30.0]);
    }
}
