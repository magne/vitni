use super::{
    CitationSummary, ConfidenceLevel, DetailTab, EvidenceAxisVm, HistoryEntryVm, Localizer, RestrictionKind, RowVm,
    TagRef, evidence_axes,
};

/// Builds a generic list row from a [`CitationSummary`]: the cited source (or the citation id) as the
/// title, the page as the subtitle, and the quote glyph as the avatar.
#[must_use]
pub fn citation_row(summary: &CitationSummary, _loc: &Localizer) -> RowVm {
    RowVm {
        id: summary.human_id.clone(),
        title: summary
            .source
            .as_ref()
            .map_or_else(|| summary.human_id.clone(), |s| s.human_id.clone()),
        subtitle: summary.page.clone(),
        avatar: Some("❝".to_owned()),
        ..RowVm::default()
    }
}

/// A citation's detail view — its evidence axes, confidence, source, page, date, attributes, and
/// attachments. The research-grade-citation differentiator (Evidence Explained axes) lives here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationDetail {
    /// The user-facing id (e.g. `C0001`).
    pub human_id: String,
    /// The cited source's `human_id`, if resolved.
    pub source: Option<String>,
    /// The page / locator within the source.
    pub page: Option<String>,
    /// The localized date of the cited record.
    pub date: Option<String>,
    /// The citation's confidence as a presentation level, if set (drives the badge).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: Option<String>,
    /// The Evidence Explained axis chips (empty when no analysis is recorded).
    pub evidence_axes: Vec<EvidenceAxisVm>,
    /// The citation's privacy restrictions (GEDCOM `RESN`), as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The recorded attributes, as `(type, value)` pairs.
    pub attributes: Vec<(String, String)>,
    /// The `human_id`s of the media objects attached to this citation.
    pub media: Vec<String>,
    /// The `human_id`s of the notes attached to this citation.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The citation's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl CitationDetail {
    /// Builds a detail view from a [`CitationSummary`], localizing labels and the date via `loc`.
    ///
    /// The History tab starts empty and is filled by the dispatcher
    /// ([`dispatch`](crate::intent::dispatch)), which has the change-log data.
    #[must_use]
    pub fn from_summary(summary: &CitationSummary, loc: &Localizer) -> Self {
        let confidence = summary.confidence.map(ConfidenceLevel::from);
        Self {
            human_id: summary.human_id.clone(),
            source: summary.source.as_ref().map(|s| s.human_id.clone()),
            page: summary.page.clone(),
            date: summary.date.as_ref().map(|date| loc.date(date)),
            confidence,
            confidence_label: confidence.map(|level| loc.confidence_label(level)),
            evidence_axes: evidence_axes(summary.evidence_analysis.as_ref(), loc),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            attributes: summary.attributes.clone(),
            media: summary.media.clone(),
            notes: summary.notes.clone(),
            tags: summary.tags.clone(),
            history: Vec::new(),
        }
    }
}

/// The tab strip for a citation's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn citation_tabs(detail: &CitationDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("attributes", Some(detail.attributes.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}
