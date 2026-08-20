//! The one-line label/value row every record pane is built from: a fixed-width label followed by
//! whatever the call site puts beside it (`.grow` value, chips, a confidence badge, a control). This
//! is the `.fact-row` the mockups draw — `record-editing.html:47` for the canonical 96px label,
//! `tag.html:93` for a narrower one. The label width stays a call-site decision because each record
//! page sizes it to its own longest label (72 tag, 80 repository, 90 media/note, 110 source, 120 note
//! language); [`DEFAULT_LABEL_WIDTH`] is the value to use when the page has no reason to differ.
//!
//! [`or_dash`] lives here too: the em dash is what a record pane shows for a fact it has no value
//! for, and every row that renders an `Option<String>` needs it.

use dioxus::prelude::*;

/// The canonical `.field-label` width, in pixels (`docs/mockups/record-editing.html:47`).
pub const DEFAULT_LABEL_WIDTH: u32 = 96;

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
