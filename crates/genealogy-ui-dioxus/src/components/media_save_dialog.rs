//! The "add a file to the media library" dialog (ADR 0017 §3; `media.html`).
//!
//! Proposes where an external file should be filed: a numbered category folder (picked from the
//! convention list unioned with folders that already exist under `<workspace>/media/`, or typed
//! freely), an optional subfolder, and a slugified filename. The live path preview and the emitted
//! target come from the framework-free [`MediaSaveDraft`](genealogy_ui::MediaSaveDraft); the host's
//! `media-store` writes the file under the media root and enforces path safety.

use dioxus::prelude::*;
use genealogy_ui::MediaSaveDraft;

use crate::components::{Button, ButtonVariant, Modal, SelectChoice, SelectInput, TextInput};

/// The already-localized labels the dialog renders (built by the caller from the `Localizer`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSaveLabels {
    /// The dialog heading.
    pub title: String,
    /// The "choose a category" quick-pick label.
    pub choose_category: String,
    /// The category field label.
    pub category: String,
    /// The subfolder field label.
    pub subfolder: String,
    /// The filename field label.
    pub filename: String,
    /// The live path-preview label.
    pub path_preview: String,
    /// The save action label.
    pub save: String,
    /// The cancel action label.
    pub cancel: String,
}

/// The controlled media-save dialog. The caller owns the `draft` signal (its
/// [`target_rel_path`](genealogy_ui::MediaSaveDraft::target_rel_path) is previewed live) and receives
/// the chosen relative path on Save.
#[component]
pub fn MediaSaveDialog(
    /// Whether the dialog is shown.
    open: bool,
    /// The already-localized labels.
    labels: MediaSaveLabels,
    /// The category folders offered by the quick-pick (convention ∪ existing folders).
    categories: Vec<String>,
    /// The controlled draft (category / subfolder / filename).
    draft: Signal<MediaSaveDraft>,
    /// Fired with the target relative path when the operator saves.
    onsave: EventHandler<String>,
    /// Fired when the operator cancels.
    oncancel: EventHandler<()>,
    /// An optional Back action's label (the assisted-import wizard sets it; other callers omit it).
    #[props(default)]
    back_label: Option<String>,
    /// Fired when the operator presses Back (only when [`back_label`] is set).
    #[props(default)]
    onback: Option<EventHandler<()>>,
) -> Element {
    let mut draft = draft;
    let preview = draft().target_rel_path();
    let save_disabled = preview.is_empty();

    let mut options = vec![SelectChoice {
        value: String::new(),
        label: "—".to_owned(),
    }];
    for category in &categories {
        options.push(SelectChoice {
            value: category.clone(),
            label: category.clone(),
        });
    }
    let selected_category = draft().category.clone();

    rsx! {
        Modal {
            title: labels.title.clone(),
            open,
            footer: rsx! {
                if let (Some(label), Some(onback)) = (back_label.clone(), onback) {
                    Button {
                        label,
                        variant: ButtonVariant::Ghost,
                        onclick: move |_| onback.call(()),
                    }
                }
                Button {
                    label: labels.cancel.clone(),
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| oncancel.call(()),
                }
                Button {
                    label: labels.save.clone(),
                    variant: ButtonVariant::Primary,
                    disabled: save_disabled,
                    onclick: move |_| onsave.call(draft().target_rel_path()),
                }
            },
            div { class: "field",
                label { r#for: "media-save-category-pick", "{labels.choose_category}" }
                SelectInput {
                    name: "media-save-category-pick",
                    options,
                    selected: selected_category,
                    onchange: move |event: FormEvent| draft.write().category = event.value(),
                }
            }
            div { class: "field",
                label { r#for: "media-save-category", "{labels.category}" }
                TextInput {
                    id: "media-save-category",
                    name: "media-save-category",
                    value: draft().category,
                    oninput: move |event: FormEvent| draft.write().category = event.value(),
                }
            }
            div { class: "field",
                label { r#for: "media-save-subfolder", "{labels.subfolder}" }
                TextInput {
                    id: "media-save-subfolder",
                    name: "media-save-subfolder",
                    value: draft().subfolder,
                    oninput: move |event: FormEvent| draft.write().subfolder = event.value(),
                }
            }
            div { class: "field",
                label { r#for: "media-save-filename", "{labels.filename}" }
                TextInput {
                    id: "media-save-filename",
                    name: "media-save-filename",
                    value: draft().filename,
                    oninput: move |event: FormEvent| draft.write().filename = event.value(),
                }
            }
            div { class: "media-save-preview",
                span { class: "field-label", "{labels.path_preview}" }
                span { class: "mono", "media/{preview}" }
            }
        }
    }
}
