use super::{
    CitationRefVm, ConfidenceLevel, DetailTab, FamilyMediaVm, HistoryEntryVm, Localizer, RestrictionKind, RowVm,
    TagRef, citation_ref_from_ref,
};

/// One asserted place name (Names tab): text, language, date, surety, and source count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceNameVm {
    /// The name text.
    pub text: String,
    /// The BCP-47 language tag, if recorded.
    pub language: Option<String>,
    /// The localized date the name was in use, if known.
    pub date: Option<String>,
    /// The operator's surety in the name assertion (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the name assertion.
    pub source_count: usize,
}

/// One enclosing place (Hierarchy tab): the place, its type, the dated link, and surety.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceHierarchyVm {
    /// The enclosing place's user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The enclosing place's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The enclosing place's display name (falls back to the `human_id`).
    pub name: String,
    /// The enclosing place's localized type label, if resolved.
    pub type_label: Option<String>,
    /// The localized dated link (when the enclosing relationship was valid), if dated.
    pub date: Option<String>,
    /// The operator's surety in the enclosing-by assertion (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
}

/// A place's detail view — type/coordinates/code facts, name history, the jurisdiction chain,
/// citations, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceDetail {
    /// The user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The stable `PlaceId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the place's primary name (falls back to the `human_id`).
    pub title: String,
    /// The localized place-type label, if set.
    pub type_label: Option<String>,
    /// The place's coordinates rendered as `lat, long`, if asserted.
    pub coordinates: Option<String>,
    /// The operator's surety in the coordinates, if asserted.
    pub coordinates_confidence: Option<ConfidenceLevel>,
    /// The localized coordinates confidence label, if asserted.
    pub coordinates_confidence_label: Option<String>,
    /// The coordinate assertion's citations, for the provenance popover.
    pub coordinate_citations: Vec<CitationRefVm>,
    /// The place's code, if set.
    pub code: Option<String>,
    /// The asserted names, with language/date + surety.
    pub names: Vec<PlaceNameVm>,
    /// The jurisdiction chain (enclosing places), nearest first.
    pub hierarchy: Vec<PlaceHierarchyVm>,
    /// The citations backing the place, with source · page · surety · evidence axes.
    pub citations: Vec<CitationRefVm>,
    /// The attached media objects.
    pub media: Vec<FamilyMediaVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The place's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The place's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl PlaceDetail {
    /// Builds a detail view from a [`PlaceSummary`](genealogy_app::PlaceSummary), localizing labels,
    /// dates, and confidence. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::PlaceSummary, loc: &Localizer) -> Self {
        let coordinates_confidence = summary.coordinates_confidence.map(ConfidenceLevel::from);
        let names = summary
            .names
            .iter()
            .map(|name| {
                let confidence = ConfidenceLevel::from(name.confidence);
                PlaceNameVm {
                    text: name.text.clone(),
                    language: name.language.clone(),
                    date: name.date.as_ref().map(|date| loc.date(date)),
                    confidence,
                    confidence_label: loc.confidence_label(confidence),
                    source_count: name.source_count,
                }
            })
            .collect();
        let hierarchy = summary
            .enclosing
            .iter()
            .map(|enclosing| {
                let confidence = ConfidenceLevel::from(enclosing.confidence);
                PlaceHierarchyVm {
                    human_id: enclosing.human_id.clone(),
                    id: enclosing.id.clone(),
                    name: enclosing.name.clone().unwrap_or_else(|| enclosing.human_id.clone()),
                    type_label: enclosing.place_type.as_ref().map(|t| loc.place_type_label(t)),
                    date: enclosing.date.as_ref().map(|date| loc.date(date)),
                    confidence,
                    confidence_label: loc.confidence_label(confidence),
                }
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: place_title(summary),
            type_label: summary.place_type.as_ref().map(|t| loc.place_type_label(t)),
            coordinates: summary.coordinates.clone(),
            coordinates_confidence,
            coordinates_confidence_label: coordinates_confidence.map(|level| loc.confidence_label(level)),
            coordinate_citations: summary
                .coordinate_citations
                .iter()
                .map(|c| citation_ref_from_ref(c, loc))
                .collect(),
            code: summary.code.clone(),
            names,
            hierarchy,
            citations: summary
                .citations
                .iter()
                .map(|citation| citation_ref_from_ref(citation, loc))
                .collect(),
            media: summary
                .media
                .iter()
                .map(|media| FamilyMediaVm {
                    human_id: media.human_id.clone(),
                    caption: media.caption.clone(),
                })
                .collect(),
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// The place's primary name for the header (its first asserted name), or the `human_id` fallback.
fn place_title(summary: &genealogy_app::PlaceSummary) -> String {
    summary
        .names
        .first()
        .map_or_else(|| summary.human_id.clone(), |name| name.text.clone())
}

/// Builds a generic list row from a [`PlaceSummary`](genealogy_app::PlaceSummary): the primary name,
/// a `type · enclosing` subtitle, and a per-type avatar.
#[must_use]
pub fn place_row(summary: &genealogy_app::PlaceSummary, loc: &Localizer) -> RowVm {
    let type_label = summary.place_type.as_ref().map(|t| loc.place_type_label(t));
    let enclosing = summary
        .enclosing
        .first()
        .map(|e| e.name.clone().unwrap_or_else(|| e.human_id.clone()));
    let subtitle = match (type_label, enclosing) {
        (Some(type_label), Some(enclosing)) => Some(format!("{type_label} · {enclosing}")),
        (Some(type_label), None) => Some(type_label),
        (None, Some(enclosing)) => Some(enclosing),
        (None, None) => None,
    };
    RowVm {
        id: summary.human_id.clone(),
        title: place_title(summary),
        subtitle,
        avatar: Some(place_avatar(summary.place_type.as_ref())),
        ..RowVm::default()
    }
}

/// The decorative avatar glyph for a place row, by type (a generic pin otherwise).
fn place_avatar(place_type: Option<&genealogy_app::PlaceType>) -> String {
    use genealogy_app::PlaceType;
    match place_type {
        Some(PlaceType::Parish) => "⛪",
        _ => "📍",
    }
    .to_owned()
}

/// The tab strip for a place's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn place_tabs(detail: &PlaceDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("names", Some(detail.names.len())),
        tab("hierarchy", Some(detail.hierarchy.len())),
        tab("citations", Some(detail.citations.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}
