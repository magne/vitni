use super::prelude::*;

/// A clickable link to another record's detail screen: opens it as a tab and navigates to its
/// category (resolving `NavState` from context, so any screen can drop it in). Shared by the
/// dashboard feed/jump-back and every detail tab that references a record.
///
/// `icon` prefixes the entity emoji (off for table cells); `button` renders the button-chip style
/// (the jump-back pills) instead of the inline link style.
#[component]
pub fn RecordLink(
    category: Category,
    human_id: String,
    label: String,
    #[props(default)] icon: bool,
    #[props(default)] button: bool,
) -> Element {
    let mut nav = use_context::<NavState>();
    let record = RecordRef {
        category,
        human_id,
        label: label.clone(),
    };
    let class = if button { "btn" } else { "src-link" };
    rsx! {
        button {
            class,
            r#type: "button",
            onclick: move |_| {
                nav.go_to(Destination::Category(record.category));
                nav.open_record(record.clone());
            },
            if icon {
                span { aria_hidden: "true", "{category.icon()} " }
            }
            "{label}"
        }
    }
}

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
                    td {
                        if let Some(source_id) = &citation.source_id {
                            RecordLink {
                                category: Category::Sources,
                                human_id: source_id.clone(),
                                label: citation.source.clone().unwrap_or_else(|| source_id.clone()),
                            }
                        } else {
                            {citation.source.clone().unwrap_or_else(|| citation.human_id.clone())}
                        }
                    }
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
