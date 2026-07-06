use super::{
    CitationChangeSetRequest, CitationSourceRequest, CitationSummary, ConfidenceLevel, DetailTab, EvidenceAnalysis,
    EvidenceAxisVm, EvidenceKind, HistoryEntryVm, InformationKind, Localizer, RestrictionKind, RowVm, SourceQuality,
    TagRef, evidence_axes, non_blank,
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

/// How the citation create form's source is set: an existing source or one created inline (§6b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CitationSourceKind {
    /// Cite an existing source (by `human_id`) — the default.
    #[default]
    Existing,
    /// Create a source inline and cite it.
    New,
}

/// The create form's in-memory draft for a new citation (`record-editing.html` §6): a required source
/// (existing or created inline — §6b), a page, and the citation's own confidence + Evidence Explained
/// analysis (distinct from the provenance block). Create-only; record date editing is PR29.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CitationDraft {
    /// How the source is set.
    pub source_kind: CitationSourceKind,
    /// The existing source's `human_id` (when `source_kind` is `Existing`).
    pub existing_source: String,
    /// The inline source's title (when `source_kind` is `New`).
    pub new_source_title: String,
    /// The page / locator.
    pub page: String,
    /// The citation's own confidence, if chosen.
    pub confidence: Option<ConfidenceLevel>,
    /// The source-quality evidence axis, if chosen.
    pub source_quality: Option<SourceQuality>,
    /// The information-kind evidence axis, if chosen.
    pub information: Option<InformationKind>,
    /// The evidence-kind axis, if chosen.
    pub evidence_kind: Option<EvidenceKind>,
}

impl CitationDraft {
    /// A fresh draft for creating a new citation.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the required source field is invalid — an existing-source selection with a blank id.
    #[must_use]
    pub fn source_invalid(&self) -> bool {
        self.source_kind == CitationSourceKind::Existing && non_blank(&self.existing_source).is_none()
    }

    /// Whether the draft is valid — the required source resolves (an existing id is given, or a new
    /// source is being created).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.source_invalid()
    }

    /// Whether the operator has entered anything — the Save gate (with [`Self::is_valid`]).
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.source_kind == CitationSourceKind::New
            || non_blank(&self.existing_source).is_some()
            || non_blank(&self.new_source_title).is_some()
            || non_blank(&self.page).is_some()
            || self.confidence.is_some()
            || self.source_quality.is_some()
            || self.information.is_some()
            || self.evidence_kind.is_some()
    }

    /// Builds the [`CitationChangeSetRequest`] the app commits on Save, or `None` when the required
    /// source is missing.
    #[must_use]
    pub fn to_request(&self) -> Option<CitationChangeSetRequest> {
        if !self.is_valid() {
            return None;
        }
        let source = match self.source_kind {
            CitationSourceKind::Existing => CitationSourceRequest::Existing(self.existing_source.trim().to_owned()),
            CitationSourceKind::New => CitationSourceRequest::New {
                title: non_blank(&self.new_source_title),
            },
        };
        let evidence = match (self.source_quality, self.information, self.evidence_kind) {
            (Some(source), Some(information), Some(evidence)) => Some(EvidenceAnalysis {
                source,
                information,
                evidence,
            }),
            _ => None,
        };
        Some(CitationChangeSetRequest {
            source,
            page: non_blank(&self.page),
            confidence: self.confidence,
            evidence,
        })
    }
}

#[cfg(test)]
mod citation_draft_tests {
    use super::{CitationDraft, CitationSourceKind};
    use crate::navigation::CitationSourceRequest;

    #[test]
    fn a_fresh_draft_is_invalid_and_not_dirty() {
        let draft = CitationDraft::new();
        assert!(!draft.is_valid(), "the default existing-source selection needs an id");
        assert!(!draft.is_dirty());
        assert!(draft.to_request().is_none());
    }

    #[test]
    fn an_existing_source_id_maps_through() {
        let draft = CitationDraft {
            existing_source: "S0001".to_owned(),
            ..CitationDraft::new()
        };
        assert!(draft.is_valid());
        assert_eq!(
            draft.to_request().expect("valid").source,
            CitationSourceRequest::Existing("S0001".to_owned())
        );
    }

    #[test]
    fn a_new_source_is_valid_even_without_a_title() {
        let draft = CitationDraft {
            source_kind: CitationSourceKind::New,
            ..CitationDraft::new()
        };
        assert!(draft.is_valid());
        match draft.to_request().expect("valid").source {
            CitationSourceRequest::New { title } => assert_eq!(title, None),
            CitationSourceRequest::Existing(_) => panic!("expected a New source"),
        }
    }
}
