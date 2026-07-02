//! The in-app help browser (ADR 0008 §5): a topic index on the left, the selected article on the
//! right, both in the existing master-detail layout. Articles are the framework-neutral
//! [`HelpDoc`](genealogy_ui::HelpDoc) vocabulary; this module is the block→RSX interpreter.
//!
//! Block prose resolves through the `genealogy-ui` [`Localizer`] catalogue, which owns the help
//! content (ADR 0003); the renderer [`Chrome`] supplies only the index list chrome. The illustrative
//! *specimens* are the one exception: their interior is diagrammatic sample data hard-coded here
//! (like an `aria-hidden` icon), so a translator never maintains the fake census rows — only captions.

use super::prelude::*;
use genealogy_ui::{
    Cell, EvidenceAxis, HelpBlock, HelpDoc, HelpTopicId, ListQuery, Run, SpecimenKind, help_doc, help_topics,
};

use crate::i18n::Chrome;

/// The help browser screen. `topic` is the navigation target (`None` = the default/landing topic);
/// an unknown id degrades to the default rather than panicking. Article content resolves through the
/// `genealogy-ui` [`Localizer`] (it owns the help catalogue); the index list chrome comes from the
/// renderer [`Chrome`].
#[component]
pub fn HelpScreen(topic: Option<HelpTopicId>) -> Element {
    let mut nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let active = topic.unwrap_or_else(HelpTopicId::default_topic);

    let rows: Vec<RowVm> = help_topics()
        .into_iter()
        .map(|meta| RowVm {
            id: meta.id.id().to_owned(),
            title: loc.help_text(meta.title_id),
            subtitle: Some(loc.help_text(meta.section.label_id())),
            avatar: None,
            ..RowVm::default()
        })
        .collect();
    let query = use_signal(ListQuery::default);
    let selected = use_signal(move || Some(active.id().to_owned()));

    let list = rsx! {
        ListPane {
            rows,
            query,
            selected,
            chrome: help_list_chrome(&chrome.0),
            onselect: move |row: RowVm| nav.go_to(Destination::Help { topic: HelpTopicId::from_id(&row.id) }),
        }
    };
    let detail = render_doc(&help_doc(active), loc);
    rsx! {
        MasterDetail { list, detail }
    }
}

/// The localized chrome for the help index list (read-only — no create affordance).
fn help_list_chrome(chrome: &Chrome) -> ListChrome {
    ListChrome {
        list_label: chrome.help_index_label(),
        filter_placeholder: chrome.help_filter(),
        empty: chrome.help_empty(),
    }
}

/// Renders a help article: every block in order, inside the scrolling `.doc` prose column. Article
/// text resolves through the `genealogy-ui` [`Localizer`] (which owns the help catalogue).
pub fn render_doc(doc: &HelpDoc, loc: &Localizer) -> Element {
    rsx! {
        div { class: "doc",
            for block in doc.blocks.iter() {
                {render_block(block, loc)}
            }
        }
    }
}

/// Renders one article block to its design-system markup.
fn render_block(block: &HelpBlock, loc: &Localizer) -> Element {
    match block {
        HelpBlock::Lede(runs) => rsx! { p { class: "lede", {render_runs(runs, loc)} } },
        HelpBlock::Heading(id) => rsx! { h2 { "{loc.help_text(id)}" } },
        HelpBlock::Paragraph(runs) => rsx! { p { {render_runs(runs, loc)} } },
        HelpBlock::Contrast { most, ours } => rsx! {
            div { class: "contrast",
                div { class: "most",
                    span { class: "tag", "{loc.help_contrast_most()}" }
                    "{loc.help_text(most)}"
                }
                div { class: "ours",
                    span { class: "tag", "{loc.help_contrast_ours()}" }
                    {render_runs(ours, loc)}
                }
            }
        },
        HelpBlock::Specimen { kind, caption } => rsx! {
            div { class: "specimen",
                if let Some(caption) = caption {
                    div { class: "label", "{loc.help_text(caption)}" }
                }
                {render_specimen(*kind)}
            }
        },
        HelpBlock::Table { headers, rows } => render_table(headers, rows, loc),
    }
}

/// Renders a sequence of inline runs. Text runs resolve through Fluent; `kbd`/`mono` are literal.
fn render_runs(runs: &[Run], loc: &Localizer) -> Element {
    rsx! {
        for run in runs.iter() {
            {match run {
                Run::Text(id) => rsx! { "{loc.help_text(id)}" },
                Run::Bold(id) => rsx! { b { "{loc.help_text(id)}" } },
                Run::Italic(id) => rsx! { i { "{loc.help_text(id)}" } },
                Run::Kbd(glyph) => rsx! { kbd { "{glyph}" } },
                Run::Mono(text) => rsx! { span { class: "mono", "{text}" } },
                Run::TopicLink { topic, label } => rsx! {
                    HelpTopicLink { topic: *topic, label: loc.help_text(label) }
                },
            }}
        }
    }
}

/// An in-prose link that navigates the help browser to another topic (the contents-page rows).
#[component]
fn HelpTopicLink(topic: HelpTopicId, label: String) -> Element {
    let mut nav = use_context::<NavState>();
    rsx! {
        button {
            class: "help-link",
            onclick: move |_| nav.go_to(Destination::Help { topic: Some(topic) }),
            "{label}"
        }
    }
}

/// Renders the "At a glance" comparison table.
fn render_table(headers: &[&'static str], rows: &[Vec<Cell>], loc: &Localizer) -> Element {
    rsx! {
        table { class: "tbl",
            thead {
                tr {
                    for header in headers.iter() {
                        th { "{loc.help_text(header)}" }
                    }
                }
            }
            tbody {
                for row in rows.iter() {
                    tr {
                        for cell in row.iter() {
                            {render_cell(cell, loc)}
                        }
                    }
                }
            }
        }
    }
}

/// Renders one table cell: a graded confidence badge, a muted value, or plain text.
fn render_cell(cell: &Cell, loc: &Localizer) -> Element {
    let text = loc.help_text(cell.text);
    match cell.badge {
        Some(level) => rsx! {
            td {
                ConfidenceBadge { level, label: text }
            }
        },
        None if cell.muted => rsx! { td { class: "muted", "{text}" } },
        None => rsx! { td { "{text}" } },
    }
}

/// Draws an illustrative specimen. The interior is fixed sample data (a diagram of the real
/// component), deliberately exempt from localization — see the module docs.
fn render_specimen(kind: SpecimenKind) -> Element {
    match kind {
        SpecimenKind::Timeline => render_timeline_specimen(),
        SpecimenKind::FactRows => render_fact_rows_specimen(),
        SpecimenKind::EvidenceAxes => render_evidence_axes_specimen(),
        SpecimenKind::MergeGrid => render_merge_grid_specimen(),
        SpecimenKind::Provenance => render_provenance_specimen(),
        SpecimenKind::CapabilityBadges => render_capability_badges_specimen(),
    }
}

fn render_timeline_specimen() -> Element {
    rsx! {
        div { class: "timeline",
            div { class: "tl-item",
                div { class: "tl-when", "2026-06-22 14:35" }
                div { class: "tl-what",
                    "Birth date asserted: "
                    b { "12 Apr 1850" }
                    " (was 1850)"
                }
                div { class: "tl-who", "magne · confidence High" }
                div { class: "tl-why", "“Baptism register gives exact date”" }
            }
            div { class: "tl-item",
                div { class: "tl-when", "2026-06-18 09:10" }
                div { class: "tl-what", "Persona I0042b merged into John Smith" }
                div { class: "tl-who", "magne · reversible" }
            }
            div { class: "tl-item",
                div { class: "tl-when", "2026-06-10 11:02" }
                div { class: "tl-what",
                    "142 records imported from "
                    span { class: "mono", "tree.ged" }
                }
                div { class: "tl-who", "gedcom-import (software agent)" }
            }
        }
    }
}

fn render_fact_rows_specimen() -> Element {
    rsx! {
        div { class: "stack",
            div { class: "fact-row",
                span { class: "field-label", style: "width:84px;margin:0", "Birth" }
                span { class: "grow", "12 Apr 1850 · New York, USA" }
                ConfidenceBadge { level: ConfidenceLevel::High, label: "High".to_owned() }
                span { class: "src-link", "❝ 2 sources" }
            }
            div { class: "fact-row",
                span { class: "field-label", style: "width:84px;margin:0", "Death" }
                span { class: "grow", "3 Nov 1920 · Brooklyn, NY" }
                ConfidenceBadge { level: ConfidenceLevel::VeryHigh, label: "Very high".to_owned() }
                span { class: "src-link", "❝ 1 source" }
            }
            div { class: "fact-row",
                span { class: "field-label", style: "width:84px;margin:0", "Occupation" }
                span { class: "grow", "Carpenter" }
                ConfidenceBadge { level: ConfidenceLevel::Low, label: "Low".to_owned() }
                NoSourceFlag { label: "no source".to_owned() }
            }
        }
    }
}

fn render_evidence_axes_specimen() -> Element {
    rsx! {
        div { class: "stack",
            div { class: "fact-row",
                span { class: "field-label", style: "width:96px;margin:0", "Source" }
                span { class: "grow muted", "Is it the record itself or a copy?" }
                span { class: "wrap",
                    EvidenceAxisChip { axis: EvidenceAxis::Source, label: "original".to_owned() }
                    EvidenceAxisChip { axis: EvidenceAxis::Source, label: "derivative".to_owned() }
                }
            }
            div { class: "fact-row",
                span { class: "field-label", style: "width:96px;margin:0", "Information" }
                span { class: "grow muted", "First-hand knowledge, or hearsay?" }
                span { class: "wrap",
                    EvidenceAxisChip { axis: EvidenceAxis::Information, label: "primary".to_owned() }
                    EvidenceAxisChip { axis: EvidenceAxis::Information, label: "secondary".to_owned() }
                }
            }
            div { class: "fact-row",
                span { class: "field-label", style: "width:96px;margin:0", "Evidence" }
                span { class: "grow muted", "Does it answer directly, or by inference?" }
                span { class: "wrap",
                    EvidenceAxisChip { axis: EvidenceAxis::Evidence, label: "direct".to_owned() }
                    EvidenceAxisChip { axis: EvidenceAxis::Evidence, label: "indirect".to_owned() }
                    EvidenceAxisChip { axis: EvidenceAxis::Evidence, label: "negative".to_owned() }
                }
            }
        }
    }
}

fn render_merge_grid_specimen() -> Element {
    rsx! {
        div { class: "merge-grid",
            div { class: "merge-col survivor",
                div { class: "field-label", "Survivor · I0042" }
                div { b { "John Smith" } }
                div { class: "muted",
                    "b. "
                    span { class: "diff", "12 Apr 1850" }
                }
                div { class: "muted", "Carpenter" }
            }
            div { class: "merge-pick",
                span { class: "radio on" }
            }
            div { class: "merge-col",
                div { class: "field-label", "Duplicate · I0042b" }
                div { b { "John Smith" } }
                div { class: "muted",
                    "b. "
                    span { class: "diff", "1850" }
                }
                div { class: "muted", "—" }
            }
        }
    }
}

fn render_provenance_specimen() -> Element {
    rsx! {
        ProvenancePopover { title: "Why we believe: Birth 12 Apr 1850".to_owned(),
            div { class: "prov-claim",
                ConfidenceBadge { level: ConfidenceLevel::High, label: String::new() }
                div {
                    div { "1850 U.S. Federal Census, New York — age 0" }
                    div { class: "wrap", style: "margin-top:4px",
                        EvidenceAxisChip { axis: EvidenceAxis::Source, label: "derivative".to_owned() }
                        EvidenceAxisChip { axis: EvidenceAxis::Information, label: "primary".to_owned() }
                        EvidenceAxisChip { axis: EvidenceAxis::Evidence, label: "indirect".to_owned() }
                    }
                    div { class: "tl-who", "asserted by magne · 22 Jun 2026" }
                }
            }
            div { class: "prov-claim",
                ConfidenceBadge { level: ConfidenceLevel::VeryHigh, label: String::new() }
                div {
                    div { "Baptism register, Trinity Church — 14 Apr 1850" }
                    div { class: "wrap", style: "margin-top:4px",
                        EvidenceAxisChip { axis: EvidenceAxis::Source, label: "original".to_owned() }
                        EvidenceAxisChip { axis: EvidenceAxis::Information, label: "primary".to_owned() }
                        EvidenceAxisChip { axis: EvidenceAxis::Evidence, label: "direct".to_owned() }
                    }
                    div { class: "tl-who", "asserted by magne · 22 Jun 2026" }
                }
            }
        }
    }
}

fn render_capability_badges_specimen() -> Element {
    rsx! {
        div { class: "wrap", style: "align-items:center",
            span { class: "badge", style: "border-color:var(--ok);color:var(--ok)", "first-party" }
            EvidenceAxisChip { axis: EvidenceAxis::Information, label: "log".to_owned() }
            EvidenceAxisChip { axis: EvidenceAxis::Information, label: "query".to_owned() }
            EvidenceAxisChip { axis: EvidenceAxis::Evidence, label: "commands".to_owned() }
            EvidenceAxisChip { axis: EvidenceAxis::Source, label: "media-store".to_owned() }
            span { class: "chip", title: "capability denied",
                span { class: "dot", style: "background:var(--warn)" }
                "net (denied)"
            }
            span { class: "muted", "·" }
            span { class: "chip mono", "nb-NO" }
            span { class: "faint", "→" }
            span { class: "chip mono", "no" }
            span { class: "faint", "→" }
            span { class: "chip mono", "en" }
        }
    }
}
