//! [`Session`] — the one place non-determinism enters the system (ADR 0004 §3, ADR 0006).
//!
//! The decision core is pure: it reads no clock and generates no id. The `Session` supplies those
//! inputs — it stamps the operator [`Agent`], reads the wall clock for `occurred_at`, and mints
//! UUID v7 ids for assertions and new aggregates — so the core stays unit-testable and provenance
//! is recorded identically for every frontend. Keep this type deliberately small: everything that
//! is hard to test lives here and nowhere else.

use genealogy_core::ids::{AgentId, AssertionId};
use genealogy_core::provenance::{Agent, AgentKind, AssertionMeta, CitationRef, EventContext, Timestamp};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::aggregates::for_each_aggregate;
use crate::use_case::Provenance;

/// Per-invocation context carrying the operator identity and the impure id/clock sources.
#[derive(Debug, Clone)]
pub struct Session {
    operator: Agent,
}

/// Generates one UUID-v7 id minter per aggregate (ADR 0004 §5) from the canonical registry.
macro_rules! session_minters {
    ($(($snake:ident, $noun:literal, $Id:ty, $id_fn:ident, $Err:ty, $domain:ident, $nf:ident, $msg:literal)),+ $(,)?) => {
        impl Session {
            $(
                #[doc = concat!("Mints an id for a new ", $noun, " aggregate (UUID v7, time-sortable — ADR 0004 §5).")]
                #[must_use]
                pub fn $id_fn(&self) -> $Id {
                    <$Id>::from_uuid(Uuid::now_v7())
                }
            )+
        }
    };
}

for_each_aggregate!(session_minters);

impl Session {
    /// Creates a session for `operator` (resolved from configuration, ADR 0005).
    #[must_use]
    pub fn new(operator: Agent) -> Self {
        Self { operator }
    }

    /// Creates a session whose operator is a software agent (ADR 0007 §7): every change a plugin
    /// makes through this session is audited as `AgentKind::Software`. The agent id is minted here
    /// (UUID v7), keeping this crate the sole impure boundary (ADR 0006).
    #[must_use]
    pub fn software(name: impl Into<String>, version: impl Into<String>) -> Self {
        let name = name.into();
        Self::new(Agent {
            kind: AgentKind::Software {
                name: name.clone(),
                version: version.into(),
            },
            id: AgentId::from_uuid(Uuid::now_v7()),
            display: Some(name),
        })
    }

    /// Builds the supplied non-deterministic inputs for one command (ADR 0004 §3).
    ///
    /// Generates a fresh [`AssertionId`], reads the clock for `occurred_at`, copies in the configured
    /// operator, and stamps the operator-supplied [`Provenance`] (confidence · rationale · evidence
    /// analysis) and `citations` onto the [`EventContext`] the core copies verbatim onto its events.
    #[must_use]
    pub fn new_meta(&self, provenance: Provenance, citations: Vec<CitationRef>) -> AssertionMeta {
        AssertionMeta {
            assertion_id: AssertionId::from_uuid(Uuid::now_v7()),
            context: EventContext {
                operator: self.operator.clone(),
                occurred_at: Timestamp::new(OffsetDateTime::now_utc()),
                rationale: provenance.rationale,
                confidence: provenance.confidence,
                citations,
                evidence_analysis: provenance.evidence_analysis,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Session;
    use crate::use_case::Provenance;
    use genealogy_core::ids::AgentId;
    use genealogy_core::provenance::{
        Agent, AgentKind, Confidence, EvidenceAnalysis, EvidenceKind, InformationKind, SourceQuality,
    };
    use uuid::Uuid;

    fn session() -> Session {
        Session::new(Agent {
            kind: AgentKind::Human,
            id: AgentId::from_uuid(Uuid::from_u128(42)),
            display: Some("Ada".to_owned()),
        })
    }

    #[test]
    fn new_meta_stamps_the_configured_operator() {
        let provenance = Provenance {
            confidence: Confidence::Normal,
            rationale: Some("note".to_owned()),
            evidence_analysis: None,
        };
        let meta = session().new_meta(provenance, Vec::new());
        assert_eq!(meta.context.operator.id, AgentId::from_uuid(Uuid::from_u128(42)));
        assert_eq!(meta.context.rationale.as_deref(), Some("note"));
    }

    #[test]
    fn new_meta_threads_the_evidence_analysis_into_the_context() {
        let analysis = EvidenceAnalysis {
            source: SourceQuality::Original,
            information: InformationKind::Primary,
            evidence: EvidenceKind::Direct,
        };
        let provenance = Provenance {
            confidence: Confidence::High,
            rationale: None,
            evidence_analysis: Some(analysis),
        };
        let meta = session().new_meta(provenance, Vec::new());
        assert_eq!(
            meta.context.evidence_analysis,
            Some(analysis),
            "the supplied evidence analysis lands in the EventContext"
        );
        assert_eq!(meta.context.confidence, Confidence::High);
    }

    #[test]
    fn successive_assertion_ids_are_distinct_and_time_ordered() {
        let session = session();
        let first = session.new_meta(Provenance::default(), Vec::new()).assertion_id;
        let second = session.new_meta(Provenance::default(), Vec::new()).assertion_id;
        assert_ne!(first, second, "each assertion gets its own id");
        assert!(first.as_uuid() <= second.as_uuid(), "UUID v7 ids are monotonic by time");
    }
}
