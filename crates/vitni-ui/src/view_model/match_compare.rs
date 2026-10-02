use super::{Localizer, MediaRefVm, rect_css};

/// One side of a pair to compare, as the host has it (ADR 0038 §3): a stored record or, in an import,
/// an incoming one. Borrowed, so a host builds it from whatever DTO it already holds.
#[derive(Debug, Clone, Copy)]
pub struct CompareSide<'a> {
    /// The record's user-facing id.
    pub human_id: &'a str,
    /// The record's display label (a person's name, a place's title).
    pub label: &'a str,
    /// The dataset record an import created it from, if any (ADR 0037 §4).
    pub origin: Option<&'a vitni_app::RecordOrigin>,
    /// The media attached to the record; the first image with a region is its evidence snippet.
    pub media: &'a [MediaRefVm],
}

/// The shared match-compare view's view-model (ADR 0038 §3, ADR 0039): the two records side by side,
/// one row per term the engine compared, and the assessment the decision records.
///
/// The rows are the engine's own terms, so every kind gets its own rows in the engine's order and the
/// view explains the score with the score's own evidence — there is no second explanation to drift.
/// Deciding is one atomic identity decision on the pair, never a field-by-field reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchCompareVm {
    /// The left record: the one that survives a *Same* decision.
    pub left: CompareSideVm,
    /// The right record.
    pub right: CompareSideVm,
    /// One row per compared term, in the engine's order — except a term neither record has a value for,
    /// which shows nothing and moved nothing.
    pub rows: Vec<CompareRowVm>,
    /// The engine's assessment of the pair, recorded on whichever decision the operator takes
    /// (ADR 0039 §2).
    pub assessment: vitni_app::MatchEvidence,
    /// The localized one-line summary of [`assessment`](Self::assessment): score, band, engine.
    pub assessment_line: String,
    /// The live decision already taken between the two records' clusters, if any (ADR 0039 §4): the
    /// view shows a [`Distinct`](vitni_app::PairDecision::Distinct) one and offers to undo it and merge.
    pub earlier_decision: Option<vitni_app::PairDecision>,
}

/// One side's header in the compare view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareSideVm {
    /// The record's user-facing id.
    pub human_id: String,
    /// The record's display label.
    pub label: String,
    /// Where an import took the record from; `None` for a record entered by hand.
    pub origin: Option<OriginChipVm>,
    /// The region of a scan the record was read from, if one is attached.
    pub evidence: Option<EvidenceSnippetVm>,
}

/// The chip naming the dataset record a side was imported from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginChipVm {
    /// The localized chip text: dataset scheme and record id.
    pub label: String,
    /// The localized tooltip naming the full dataset.
    pub title: String,
}

/// A scan with the region the record was read from (the Digitalarkivet line crop).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceSnippetVm {
    /// The URL the renderer loads the image from.
    pub src: String,
    /// The region's inline CSS over its frame ([`rect_css`]).
    pub crop_css: String,
    /// The localized caption naming the media object.
    pub caption: String,
}

/// How one term compared — the non-colour cue each row carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowOutcome {
    /// The values agree.
    Agree,
    /// The values are close.
    Partial,
    /// The values are implausibly far apart.
    Disagree,
    /// A side has no value.
    Missing,
    /// The values cannot both be true of one record.
    Conflict,
}

/// One compared term: the feature, each side's value, how they compared and why it moved the score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareRowVm {
    /// The localized feature label (e.g. "Given name").
    pub feature: String,
    /// The left record's value, as the record states it.
    pub left: Option<String>,
    /// The right record's value.
    pub right: Option<String>,
    /// How the values compared.
    pub outcome: RowOutcome,
    /// The localized name of [`outcome`](Self::outcome), for assistive technology.
    pub outcome_label: String,
    /// The localized explanation with the term's weight: "Same given name (+3.0)".
    pub explanation: String,
}

impl MatchCompareVm {
    /// Builds the view-model from the two sides and the engine's assessment of them. The earlier
    /// decision starts unset; the host fills it from the pair's identity decision.
    #[must_use]
    pub fn build(
        left: CompareSide<'_>,
        right: CompareSide<'_>,
        assessment: &vitni_app::MatchAssessment,
        loc: &Localizer,
    ) -> Self {
        let evidence = assessment.evidence();
        let mut rows = Vec::with_capacity(assessment.features.len());
        for (term, recorded) in assessment.features.iter().zip(&evidence.features) {
            if term.left.is_none() && term.right.is_none() && recorded.outcome == vitni_app::OutcomeEvidence::Missing {
                continue;
            }
            rows.push(CompareRowVm {
                feature: loc.match_row_label(term.feature),
                left: term.left.as_ref().map(|value| loc.match_value(value)),
                right: term.right.as_ref().map(|value| loc.match_value(value)),
                outcome: RowOutcome::from(recorded.outcome),
                outcome_label: loc.match_outcome_label(recorded.outcome),
                explanation: loc.match_row_explanation(recorded),
            });
        }
        Self {
            left: CompareSideVm::build(left, loc),
            right: CompareSideVm::build(right, loc),
            rows,
            assessment_line: loc.identity_assessment(&evidence),
            assessment: evidence,
            earlier_decision: None,
        }
    }
}

impl CompareSideVm {
    fn build(side: CompareSide<'_>, loc: &Localizer) -> Self {
        Self {
            human_id: side.human_id.to_owned(),
            label: side.label.to_owned(),
            origin: side.origin.map(|origin| OriginChipVm {
                label: loc.match_origin_label(origin),
                title: loc.match_origin_title(origin),
            }),
            evidence: evidence_snippet(side.media, loc),
        }
    }
}

/// The first attached image that carries a region and can be served.
fn evidence_snippet(media: &[MediaRefVm], loc: &Localizer) -> Option<EvidenceSnippetVm> {
    for reference in media {
        let (Some(crop), true) = (reference.crop, reference.is_image()) else {
            continue;
        };
        let Some(src) = reference.src() else {
            continue;
        };
        return Some(EvidenceSnippetVm {
            src,
            crop_css: rect_css(&crop),
            caption: loc.match_evidence_caption(&reference.caption_or_id()),
        });
    }
    None
}

impl From<vitni_app::OutcomeEvidence> for RowOutcome {
    fn from(outcome: vitni_app::OutcomeEvidence) -> Self {
        match outcome {
            vitni_app::OutcomeEvidence::Agree => Self::Agree,
            vitni_app::OutcomeEvidence::Partial(_) => Self::Partial,
            vitni_app::OutcomeEvidence::Disagree => Self::Disagree,
            vitni_app::OutcomeEvidence::Missing => Self::Missing,
            vitni_app::OutcomeEvidence::Conflict => Self::Conflict,
        }
    }
}
