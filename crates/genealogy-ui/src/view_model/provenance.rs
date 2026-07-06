use super::{
    ConfidenceLevel, EvidenceAnalysis, EvidenceKind, InformationKind, MutationMeta, Provenance, SourceQuality,
};

/// The operator-intent a save collects once and applies to every assertion the edit form emits
/// (`record-editing.html` §5b): the free-text rationale ("why"), the confidence, the backing citation
/// `human_id`s, and the optional Evidence Explained analysis (three axes). Bound to the edit panel's
/// provenance block; reset whenever the panel opens so a prior save's rationale never leaks into the
/// next. [`Self::meta`] borrows the citations into a [`MutationMeta`] a `dispatch_*_edit` passes to
/// the use-case; operator and timestamp come from the [`Session`](genealogy_app::Session), never
/// typed. Never supersedes — corrections are PR27 territory.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProvenanceDraft {
    /// Why the change is being made (free text; blank ⇒ no rationale recorded).
    pub rationale: String,
    /// The operator's surety in the asserted claim(s).
    pub confidence: ConfidenceLevel,
    /// Citation `human_id`s backing the assertion(s).
    pub citations: Vec<String>,
    /// The source-quality axis, if chosen.
    pub source: Option<SourceQuality>,
    /// The information-kind axis, if chosen.
    pub information: Option<InformationKind>,
    /// The evidence-kind axis, if chosen.
    pub evidence: Option<EvidenceKind>,
}

impl ProvenanceDraft {
    /// Builds the [`Provenance`] this draft describes: a blank/whitespace rationale becomes `None`
    /// (trimmed otherwise), and an evidence analysis is produced only when all three axes are chosen.
    #[must_use]
    pub fn provenance(&self) -> Provenance {
        let rationale = {
            let trimmed = self.rationale.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        };
        let evidence_analysis = match (self.source, self.information, self.evidence) {
            (Some(source), Some(information), Some(evidence)) => Some(EvidenceAnalysis {
                source,
                information,
                evidence,
            }),
            _ => None,
        };
        Provenance {
            confidence: self.confidence.into(),
            rationale,
            evidence_analysis,
        }
    }

    /// Bundles this draft into the [`MutationMeta`] a non-create mutation use-case takes, borrowing
    /// the citation ids. Never supersedes (PR27).
    #[must_use]
    pub fn meta(&self) -> MutationMeta<'_> {
        MutationMeta {
            provenance: self.provenance(),
            citations: &self.citations,
            supersedes: None,
        }
    }
}
