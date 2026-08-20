//! The one-line label/value row every record pane is built from: a fixed-width label followed by
//! whatever the call site puts beside it (`.grow` value, chips, a confidence badge, a control). This
//! is the `.fact-row` the mockups draw — `record-editing.html:47` for the canonical 96px label,
//! `family.html:138` for a narrower one.
//!
//! **The width is a floor, and it has to clear the card's own longest label.** `.fact-row >
//! .field-label` is `min-width: max-content`, so a label wider than the declared column raises that
//! one row's floor rather than overlapping its value — which keeps the row readable but takes that
//! row's value out of line with the rest of the card. So the column belongs to the *card*, sized to
//! the longest label the card renders **in every shipped locale**: Norwegian is routinely longer than
//! English (`RESTRIKSJONER` 102px against `RESTRICTIONS` 92px at the shared `.field-label` type).
//!
//! Hence two constants. [`DEFAULT_LABEL_WIDTH`] (96) is the canonical width for a card of ordinary
//! fact rows, whose labels come from the data. [`RECORD_LABEL_WIDTH`] (110) is the floor for a
//! *whole-record* card, because every one of them carries the Restrictions row. Four record pages
//! need more than that and name their own const beside the screen — person and citation 140,
//! DNA match 140, tag 130.
//!
//! [`or_dash`] lives here too: the em dash is what a record pane shows for a fact it has no value
//! for, and every row that renders an `Option<String>` needs it.

use dioxus::prelude::*;

/// The canonical `.field-label` width, in pixels (`docs/mockups/record-editing.html:47`).
pub const DEFAULT_LABEL_WIDTH: u32 = 96;

/// The `.field-label` width a whole-record card needs: every record carries the Restrictions row, and
/// `RESTRIKSJONER` renders 102px, so [`DEFAULT_LABEL_WIDTH`] would leave that one row's pills out of
/// line with the card's other values. A record page whose own labels are longer still overrides it.
pub const RECORD_LABEL_WIDTH: u32 = 110;

/// The em dash a record pane shows in place of a value it does not have.
const DASH: &str = "—";

/// `value`, or the em dash placeholder when there is none.
#[must_use]
pub fn or_dash(value: Option<String>) -> String {
    value.unwrap_or_else(|| DASH.to_owned())
}

/// One `.fact-row`: the label, then the call site's `children` (see module docs).
#[component]
pub fn FactRow(
    /// The row's already-localized label.
    label: String,
    /// The label column's width in pixels.
    #[props(default = DEFAULT_LABEL_WIDTH)]
    label_width: u32,
    /// The id of the control this row labels. Set it to render a real `<label for>`; leave it unset
    /// for a read-only row, which gets a `<span>` instead.
    #[props(default)]
    name: Option<String>,
    /// The row's content, rendered after the label.
    children: Element,
) -> Element {
    rsx! {
        div { class: "fact-row",
            if let Some(name) = name {
                label {
                    r#for: "{name}",
                    class: "field-label",
                    style: "width:{label_width}px;margin:0",
                    "{label}"
                }
            } else {
                span { class: "field-label", style: "width:{label_width}px;margin:0", "{label}" }
            }
            {children}
        }
    }
}
