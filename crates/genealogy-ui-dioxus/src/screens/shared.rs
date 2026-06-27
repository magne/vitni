use super::prelude::*;

/// A minimal list of related-item ids, or an empty-state when there are none.
pub fn id_list(loc: &Localizer, ids: &[String]) -> Element {
    if ids.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        ul { class: "id-list",
            for id in ids.iter() {
                li { "{id}" }
            }
        }
    }
}

/// The Media tab: a thumbnail gallery, one placeholder card per attached media id.
pub fn media_gallery(loc: &Localizer, media: &[String]) -> Element {
    if media.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "grid-3",
            for id in media.iter() {
                div { class: "card", style: "text-align:center",
                    div {
                        class: "faint",
                        style: "height:120px;background:var(--panel-2);border-radius:var(--r-md);display:grid;place-items:center",
                        "🖼"
                    }
                    div { style: "margin-top:8px", "{id}" }
                }
            }
        }
    }
}

/// The Tags tab: each applied tag as a chip. (Tag editing is a later slice.)
pub fn tags_panel(loc: &Localizer, tags: &[String]) -> Element {
    if tags.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "wrap",
            for tag in tags.iter() {
                Chip { label: tag.clone() }
            }
        }
    }
}

/// Returns `None` for a blank field (so an absent field is not asserted), else the value as typed.
#[must_use]
pub fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() { None } else { Some(value) }
}

/// The evidence-first source cue: a source-count link, or a no-source flag when unsourced.
pub fn source_cue(loc: &Localizer, source_count: usize) -> Element {
    if source_count > 0 {
        rsx! { SourceLink { label: loc.source_count(source_count), onclick: move |_| {} } }
    } else {
        rsx! { NoSourceFlag { label: loc.no_source() } }
    }
}

/// The Media tab: a thumbnail gallery, one card per attached media object (caption or id).
pub fn family_media_gallery(loc: &Localizer, media: &[FamilyMediaVm]) -> Element {
    if media.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "grid-3",
            for item in media.iter() {
                div { class: "card", style: "text-align:center",
                    div {
                        class: "faint",
                        style: "height:120px;background:var(--panel-2);border-radius:var(--r-md);display:grid;place-items:center",
                        "🖼"
                    }
                    div { style: "margin-top:8px", {item.caption.clone().unwrap_or_else(|| item.human_id.clone())} }
                }
            }
        }
    }
}

/// A shared citations table (Event/Place Citations tab): source · page · surety · evidence axes.
pub fn citation_table(loc: &Localizer, citations: &[CitationRefVm]) -> Element {
    if citations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("source"),
                loc.field_label("page"),
                loc.field_label("surety"),
                loc.field_label("evidence"),
            ],
            for citation in citations.iter() {
                tr {
                    td { {citation.source.clone().unwrap_or_else(|| citation.human_id.clone())} }
                    td { class: "muted", {citation.page.clone().unwrap_or_else(|| "—".to_owned())} }
                    td {
                        if let (Some(level), Some(label)) = (citation.confidence, citation.confidence_label.clone()) {
                            ConfidenceBadge { level, label }
                        } else {
                            span { class: "muted", "—" }
                        }
                    }
                    td { class: "wrap",
                        for chip in citation.evidence_axes.iter() {
                            EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Event slice
// ---------------------------------------------------------------------------------------------------

/// The source media types offered by the link forms (a common subset; the model has more).
#[must_use]
pub fn source_media_type_choices() -> [SourceMediaType; 6] {
    [
        SourceMediaType::Book,
        SourceMediaType::Film,
        SourceMediaType::Electronic,
        SourceMediaType::Fiche,
        SourceMediaType::Manuscript,
        SourceMediaType::Photo,
    ]
}
