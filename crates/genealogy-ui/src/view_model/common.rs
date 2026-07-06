use super::{ConfidenceLevel, EvidenceAnalysis, EvidenceAxis, Localizer, friendly_timestamp};

/// Trims a form field and maps a blank value to `None` — the "not reported" convention every create
/// draft applies to an optional field so an empty box writes nothing (`record-editing.html` §6).
#[must_use]
pub(crate) fn non_blank(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// One asserted name variant, for the Names tab — carrying its evidence cues (surety + source count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameVm {
    /// The localized name-type label.
    pub type_label: String,
    /// The rendered `given surname(s)` display string.
    pub display: String,
    /// The given name, if any.
    pub given: Option<String>,
    /// The primary surname, if any.
    pub surname: Option<String>,
    /// The nickname, if any.
    pub nickname: Option<String>,
    /// The localized date this name was in use, if known.
    pub date: Option<String>,
    /// The BCP-47 language tag of this name, if known.
    pub language: Option<String>,
    /// The name's confidence, as a presentation level (drives the badge colour token).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: String,
    /// How many citations back this name (its source count).
    pub source_count: usize,
}

impl NameVm {
    /// Whether the name has at least one backing source (drives the no-source flag).
    #[must_use]
    pub fn has_source(&self) -> bool {
        self.source_count > 0
    }
}

/// One asserted fact, for the Facts tab — the evidence-first row (confidence + source count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactVm {
    /// The localized fact-type label.
    pub type_label: String,
    /// The fact's free-text value, if any.
    pub value: Option<String>,
    /// The localized rendered date, if any.
    pub date: Option<String>,
    /// The fact's confidence, as a presentation level (drives the badge colour token).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: String,
    /// How many citations back this fact (its source count).
    pub source_count: usize,
    /// The fact's citations, for the provenance popover (source · page · surety · evidence axes).
    pub citations: Vec<CitationRefVm>,
}

impl FactVm {
    /// Whether the fact has at least one backing source (drives the no-source flag).
    #[must_use]
    pub fn has_source(&self) -> bool {
        self.source_count > 0
    }
}

/// One event participation, for the Events tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRefVm {
    /// The event's user-facing id (e.g. `E0001`).
    pub event_id: String,
    /// The localized participant-role label.
    pub role_label: String,
    /// The localized rendered event date, if known.
    pub date: Option<String>,
}

/// One person-to-person association, for the Associations tab — with its evidence cues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationVm {
    /// The other person's user-facing id.
    pub other_id: String,
    /// The localized association-role label.
    pub role_label: String,
    /// The association's confidence, as a presentation level (drives the badge colour token).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: String,
    /// How many citations back this association (its source count).
    pub source_count: usize,
}

impl AssociationVm {
    /// Whether the association has at least one backing source (drives the no-source flag).
    #[must_use]
    pub fn has_source(&self) -> bool {
        self.source_count > 0
    }
}

/// One citation backing a record, for the Citations tab — its source, page, surety, and evidence axes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationRefVm {
    /// The citation's user-facing id (e.g. `C0001`).
    pub human_id: String,
    /// The cited source's display label (its title, or `human_id`), if resolved.
    pub source: Option<String>,
    /// The cited source's user-facing id (e.g. `S0001`), for navigating to it.
    pub source_id: Option<String>,
    /// The page / locator within the cited source, if set.
    pub page: Option<String>,
    /// The citation's confidence, if set (drives the badge).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label, if set.
    pub confidence_label: Option<String>,
    /// The Evidence Explained axis chips (empty when the citation records no analysis).
    pub evidence_axes: Vec<EvidenceAxisVm>,
    /// The localized "asserted by {who} · {when}" provenance line, if the creation operator is known.
    pub asserted_by: Option<String>,
}

/// Builds a [`CitationRefVm`] from an app [`CitationRef`](genealogy_app::CitationRef) — the joined
/// citation row used by the Event/Place Citations tabs (source label, page, surety, evidence axes).
#[must_use]
pub fn citation_ref_from_ref(reference: &genealogy_app::CitationRef, loc: &Localizer) -> CitationRefVm {
    let confidence = reference.confidence.map(ConfidenceLevel::from);
    let source = reference
        .source_title
        .clone()
        .or_else(|| reference.source.as_ref().map(|s| s.human_id.clone()));
    CitationRefVm {
        human_id: reference.human_id.clone(),
        source,
        source_id: reference.source.as_ref().map(|s| s.human_id.clone()),
        page: reference.page.clone(),
        confidence,
        confidence_label: confidence.map(|level| loc.confidence_label(level)),
        evidence_axes: evidence_axes(reference.analysis.as_ref(), loc),
        asserted_by: reference.asserted_by.as_ref().map(|who| {
            let when = reference.asserted_at.as_deref().map(friendly_timestamp);
            loc.provenance_asserted_by(who, when.as_deref())
        }),
    }
}

/// One Evidence Explained axis chip: which axis it is (drives the hue) and its localized value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceAxisVm {
    /// The axis (source / information / evidence).
    pub axis: EvidenceAxis,
    /// The already-localized axis value (e.g. "Original", "Primary", "Direct").
    pub label: String,
}

/// Builds the three Evidence Explained axis chips from a citation's [`EvidenceAnalysis`], localizing
/// each value via `loc`. Returns an empty vec when no analysis is recorded.
#[must_use]
pub fn evidence_axes(analysis: Option<&EvidenceAnalysis>, loc: &Localizer) -> Vec<EvidenceAxisVm> {
    let Some(analysis) = analysis else {
        return Vec::new();
    };
    vec![
        EvidenceAxisVm {
            axis: EvidenceAxis::Source,
            label: loc.evidence_source_label(analysis.source),
        },
        EvidenceAxisVm {
            axis: EvidenceAxis::Information,
            label: loc.evidence_information_label(analysis.information),
        },
        EvidenceAxisVm {
            axis: EvidenceAxis::Evidence,
            label: loc.evidence_kind_label(analysis.evidence),
        },
    ]
}
