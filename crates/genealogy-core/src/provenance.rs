//! Provenance value objects: who asserted a claim, when, why, how sure, and on what evidence.
//!
//! Every domain event embeds an [`EventContext`] in its payload (ADR 0004 §1), making the
//! audit trail a property of the architecture rather than a bolt-on. [`AssertionMeta`] bundles
//! the context with the pre-generated [`AssertionId`]; it is the *supplied non-deterministic
//! input* the application layer builds before calling the pure `decide` core (ADR 0004 §3).

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::ids::{AgentId, AssertionId, CitationId, DnaMatchId};

/// An assertion timestamp (the moment a claim was recorded), serialized as RFC 3339.
///
/// Distinct from any subject date inside an event payload (e.g. a birth date). Produced by the
/// application clock and passed in, never sampled by the decision core (ADR 0004 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(#[serde(with = "time::serde::rfc3339")] OffsetDateTime);

impl Timestamp {
    /// Wraps an application-supplied instant.
    #[must_use]
    pub const fn new(at: OffsetDateTime) -> Self {
        Self(at)
    }

    /// Returns the wrapped instant.
    #[must_use]
    pub const fn into_inner(self) -> OffsetDateTime {
        self.0
    }

    /// Parses an RFC 3339 timestamp string into a [`Timestamp`], or `None` if it is not valid RFC
    /// 3339 — reuses the same `serde(with = "time::serde::rfc3339")` decoding every stored assertion
    /// timestamp already goes through. A plugin-host capability boundary (ADR 0029 §2) uses this to
    /// decode the file's own export date; the `None` degradation is the "honest about carrying no
    /// structure" stance for a malformed input (ADR 0029 §3).
    #[must_use]
    pub fn parse_rfc3339(value: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(value.to_owned())).ok()
    }
}

/// What kind of actor made an assertion (data-model §7, §13).
///
/// Distinguishing a human claim from a software match or an AI suggestion is what makes
/// imports and machine-generated assertions attributable and auditable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AgentKind {
    /// A human researcher.
    Human,
    /// An automated process (importer, match engine), with its name and version.
    Software {
        /// The software's name (e.g. `genealogy-import`).
        name: String,
        /// The software's version, for reproducibility.
        version: String,
    },
    /// An AI model, with its name and version, so you can audit which model asserted what.
    AiModel {
        /// The model's name (e.g. `claude-opus-4-8`).
        name: String,
        /// The model's version.
        version: String,
    },
}

/// Who caused an event: the operator behind an assertion (data-model §7, §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    /// The actor's category (human / software / AI).
    pub kind: AgentKind,
    /// A stable identity for the actor.
    pub id: AgentId,
    /// An optional human-readable display name.
    pub display: Option<String>,
}

/// The operator's surety in a single assertion: Gramps' five levels (data-model §7).
///
/// Aligns with GEDCOM `QUAY 0-3` and GEDCOM X `ConfidenceLevel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Confidence {
    /// Lowest surety.
    VeryLow,
    /// Low surety.
    Low,
    /// The default, middling surety.
    Normal,
    /// High surety.
    High,
    /// Highest surety.
    VeryHigh,
}

/// Whether a source is an original record or a derivative (Evidence Explained — data-model §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceQuality {
    /// An original record.
    Original,
    /// A derivative of an original (transcription, index, abstract).
    Derivative,
}

/// Whether the information was recorded by a primary or secondary informant (data-model §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InformationKind {
    /// First-hand knowledge.
    Primary,
    /// Second-hand knowledge.
    Secondary,
}

/// How directly the evidence bears on the claim (data-model §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceKind {
    /// Directly answers the question.
    Direct,
    /// Answers only in combination with other evidence.
    Indirect,
    /// The absence of an expected record is itself evidence.
    Negative,
}

/// The three *Evidence Explained* analysis axes, carried alongside [`Confidence`] (data-model §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAnalysis {
    /// Original vs. derivative source.
    pub source: SourceQuality,
    /// Primary vs. secondary information.
    pub information: InformationKind,
    /// Direct / indirect / negative evidence.
    pub evidence: EvidenceKind,
}

/// A link to the evidence backing a claim: a `Citation` aggregate (a documentary source) or a
/// `DnaMatch` aggregate (a genetic observation supporting a relationship inference) — data-model §7,
/// §12. The envelope's `citations` list is the sole evidence channel (ADR 0020); this union makes
/// its target polymorphic (ADR 0023) so a DNA match is cited exactly as a source is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceRef {
    /// A cited `Citation` aggregate (a documentary source).
    Citation(CitationId),
    /// A cited `DnaMatch` aggregate backing a relationship inference (data-model §12).
    DnaMatch(DnaMatchId),
}

impl EvidenceRef {
    /// The cited `CitationId`, or `None` when this reference points at a DNA match.
    #[must_use]
    pub fn as_citation(self) -> Option<CitationId> {
        match self {
            Self::Citation(id) => Some(id),
            Self::DnaMatch(_) => None,
        }
    }

    /// The cited `DnaMatchId`, or `None` when this reference points at a citation.
    #[must_use]
    pub fn as_dna_match(self) -> Option<DnaMatchId> {
        match self {
            Self::DnaMatch(id) => Some(id),
            Self::Citation(_) => None,
        }
    }
}

/// The provenance envelope embedded in every event payload (data-model §8, ADR 0004 §1).
///
/// Records who / when / why / how sure / on what evidence. Because it lives on the event,
/// surety and provenance are per-assertion — fixing the Gramps limitation where confidence
/// lived only on the citation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventContext {
    /// Who caused the change (human, software, or AI model).
    pub operator: Agent,
    /// When the assertion was recorded.
    pub occurred_at: Timestamp,
    /// Why the change was made (free text; GENTECH rationale / GEDCOM X change message).
    pub rationale: Option<String>,
    /// The operator's surety in this specific claim. `None` = no judgment recorded (ADR 0021 §5) —
    /// mechanical acts (`Tagged`, `RestrictionsChanged`, colour/path/checksum setters) record none.
    #[serde(default)]
    pub confidence: Option<Confidence>,
    /// Zero or more evidence references backing this claim (citations and/or DNA matches — the sole
    /// evidence channel, ADR 0020; polymorphic target, ADR 0023).
    pub citations: Vec<EvidenceRef>,
    /// The optional Evidence Explained analysis for this claim.
    pub evidence_analysis: Option<EvidenceAnalysis>,
}

impl EventContext {
    /// The `CitationId`s backing this claim (the `Citation` variants of [`Self::citations`]).
    pub fn citation_ids(&self) -> impl Iterator<Item = CitationId> + '_ {
        self.citations.iter().filter_map(|evidence| evidence.as_citation())
    }

    /// The `DnaMatchId`s backing this claim (the `DnaMatch` variants of [`Self::citations`]).
    pub fn dna_match_ids(&self) -> impl Iterator<Item = DnaMatchId> + '_ {
        self.citations.iter().filter_map(|evidence| evidence.as_dna_match())
    }
}

/// The supplied non-deterministic inputs for one command (ADR 0004 §3).
///
/// The application layer generates the [`AssertionId`] (UUID v7) and builds the [`EventContext`]
/// (operator, clock, citations) *before* calling `decide`, which copies this verbatim onto every
/// event it emits. This is what keeps `decide` pure and unit-testable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertionMeta {
    /// The identity assigned to the assertion this command produces.
    pub assertion_id: AssertionId,
    /// The provenance envelope to stamp onto the emitted events.
    pub context: EventContext,
}

#[cfg(test)]
mod tests {
    use super::{
        Agent, AgentKind, AssertionMeta, Confidence, EventContext, EvidenceAnalysis, EvidenceKind, InformationKind,
        SourceQuality, Timestamp,
    };
    use crate::ids::{AgentId, AssertionId};
    use time::macros::datetime;
    use uuid::Uuid;

    fn sample_meta() -> AssertionMeta {
        AssertionMeta {
            assertion_id: AssertionId::from_uuid(Uuid::from_u128(1)),
            context: EventContext {
                operator: Agent {
                    kind: AgentKind::Human,
                    id: AgentId::from_uuid(Uuid::from_u128(2)),
                    display: Some("Ada".to_owned()),
                },
                occurred_at: Timestamp::new(datetime!(2026-06-17 12:00:00 UTC)),
                rationale: Some("parish register".to_owned()),
                confidence: Some(Confidence::High),
                citations: Vec::new(),
                evidence_analysis: Some(EvidenceAnalysis {
                    source: SourceQuality::Original,
                    information: InformationKind::Primary,
                    evidence: EvidenceKind::Direct,
                }),
            },
        }
    }

    #[test]
    fn assertion_meta_round_trips_through_json() {
        let meta = sample_meta();
        let json = serde_json::to_string(&meta).unwrap();
        let back: AssertionMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, back);
    }

    #[test]
    fn timestamp_serializes_as_rfc3339() {
        let ts = Timestamp::new(datetime!(2026-06-17 12:00:00 UTC));
        let json = serde_json::to_string(&ts).unwrap();
        assert_eq!(json, "\"2026-06-17T12:00:00Z\"");
    }

    #[test]
    fn parse_rfc3339_round_trips_a_valid_timestamp() {
        let ts = Timestamp::new(datetime!(2026-06-17 12:00:00 UTC));
        assert_eq!(Timestamp::parse_rfc3339("2026-06-17T12:00:00Z"), Some(ts));
    }

    #[test]
    fn parse_rfc3339_rejects_a_malformed_string() {
        assert_eq!(Timestamp::parse_rfc3339("not a date"), None);
        assert_eq!(
            Timestamp::parse_rfc3339("2026-06-17"),
            None,
            "a bare date has no time component"
        );
    }

    #[test]
    fn agent_kind_is_tagged_by_kind() {
        let kind = AgentKind::Software {
            name: "importer".to_owned(),
            version: "1.0".to_owned(),
        };
        let json = serde_json::to_value(&kind).unwrap();
        assert_eq!(json["kind"], "Software");
        assert_eq!(json["name"], "importer");
    }

    #[test]
    fn confidence_orders_from_very_low_to_very_high() {
        assert!(Confidence::VeryLow < Confidence::Normal);
        assert!(Confidence::Normal < Confidence::VeryHigh);
    }

    #[test]
    fn event_context_without_confidence_decodes_as_none() {
        let mut value = serde_json::to_value(sample_meta().context).unwrap();
        value
            .as_object_mut()
            .expect("context serializes as a JSON object")
            .remove("confidence");
        let context: EventContext = serde_json::from_value(value).unwrap();
        assert_eq!(context.confidence, None);
    }

    #[test]
    fn legacy_confidence_string_decodes_as_some() {
        let mut value = serde_json::to_value(sample_meta().context).unwrap();
        value
            .as_object_mut()
            .expect("context serializes as a JSON object")
            .insert("confidence".to_owned(), serde_json::json!("Normal"));
        let context: EventContext = serde_json::from_value(value).unwrap();
        assert_eq!(context.confidence, Some(Confidence::Normal));
    }

    #[test]
    fn none_confidence_round_trips_through_json() {
        let mut meta = sample_meta();
        meta.context.confidence = None;
        let json = serde_json::to_string(&meta).unwrap();
        let back: AssertionMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, back);
    }
}
