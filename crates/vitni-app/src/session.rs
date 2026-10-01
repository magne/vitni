//! [`Session`] — the one place non-determinism enters the system (ADR 0004 §3, ADR 0006).
//!
//! The decision core is pure: it reads no clock and generates no id. The `Session` supplies those
//! inputs — it stamps the operator [`Agent`], reads the wall clock for `occurred_at`, and mints
//! UUID v7 ids for assertions and new aggregates — so the core stays unit-testable and provenance
//! is recorded identically for every frontend. Keep this type deliberately small: everything that
//! is hard to test lives here and nowhere else.

use std::sync::Arc;

use time::OffsetDateTime;
use uuid::Uuid;
use vitni_core::ids::{AgentId, AssertionId};
use vitni_core::provenance::{Agent, AgentKind, AssertionMeta, EventContext, EvidenceRef, Timestamp};

use crate::aggregates::for_each_aggregate;
use crate::origin_gate::{DryRun, PendingRun};
use crate::use_case::Provenance;

/// Per-invocation context carrying the operator identity and the impure id/clock sources.
#[derive(Debug, Clone)]
pub struct Session {
    operator: Agent,
    import_run: Option<Arc<PendingRun>>,
    dry_run: Option<Arc<DryRun>>,
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
        Self {
            operator,
            import_run: None,
            dry_run: None,
        }
    }

    /// This session, writing as part of `run`: every write the origin gate lets through starts the
    /// run if it has not started yet (ADR 0037 §5).
    #[must_use]
    pub fn with_import_run(mut self, run: Arc<PendingRun>) -> Self {
        self.import_run = Some(run);
        self
    }

    /// The import run this session writes as part of, if any.
    #[must_use]
    pub fn import_run(&self) -> Option<&Arc<PendingRun>> {
        self.import_run.as_ref()
    }

    /// This session, recording into `dry_run` every write the origin gate would let through instead
    /// of executing any (ADR 0040 §2).
    #[must_use]
    pub fn with_dry_run(mut self, dry_run: Arc<DryRun>) -> Self {
        self.dry_run = Some(dry_run);
        self
    }

    /// The dry run this session records into, if any.
    #[must_use]
    pub fn dry_run(&self) -> Option<&Arc<DryRun>> {
        self.dry_run.as_ref()
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

    /// Mints an [`AssertionId`] for an assertion a command carries beyond its own — an external id
    /// recorded at creation (UUID v7, ADR 0004 §5).
    #[must_use]
    pub fn new_assertion_id(&self) -> AssertionId {
        AssertionId::from_uuid(Uuid::now_v7())
    }

    /// Mints the lineage id of a new file-lineage dataset (`gedcom:<uuid>` — ADR 0037 §3).
    #[must_use]
    pub fn new_dataset_lineage(&self) -> Uuid {
        Uuid::now_v7()
    }

    /// Reads the wall clock — for a record of *when* something ran that is not an assertion (a
    /// backup's creation time, ADR 0041), so the clock is still read here and nowhere else.
    #[must_use]
    pub fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    /// Builds the supplied non-deterministic inputs for one command (ADR 0004 §3).
    ///
    /// Generates a fresh [`AssertionId`], reads the clock for `occurred_at`, copies in the configured
    /// operator, and stamps the operator-supplied [`Provenance`] (confidence · rationale · evidence
    /// analysis) and `citations` onto the [`EventContext`] the core copies verbatim onto its events.
    #[must_use]
    pub fn new_meta(&self, provenance: Provenance, citations: Vec<EvidenceRef>) -> AssertionMeta {
        AssertionMeta {
            assertion_id: self.new_assertion_id(),
            context: EventContext {
                operator: self.operator.clone(),
                occurred_at: Timestamp::new(OffsetDateTime::now_utc()),
                rationale: provenance.rationale,
                confidence: provenance.confidence,
                citations,
                evidence_analysis: provenance.evidence_analysis,
                origin: provenance.origin.map(Box::new),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Session;
    use crate::use_case::Provenance;
    use uuid::Uuid;
    use vitni_core::ids::AgentId;
    use vitni_core::provenance::{
        Agent, AgentKind, Confidence, EvidenceAnalysis, EvidenceKind, InformationKind, SourceQuality,
    };

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
            confidence: Some(Confidence::Normal),
            rationale: Some("note".to_owned()),
            evidence_analysis: None,
            origin: None,
        };
        let meta = session().new_meta(provenance, Vec::new());
        assert_eq!(meta.context.operator.id, AgentId::from_uuid(Uuid::from_u128(42)));
        assert_eq!(meta.context.rationale.as_deref(), Some("note"));
    }

    #[test]
    fn new_meta_records_no_confidence_when_none_is_supplied() {
        let meta = session().new_meta(Provenance::default(), Vec::new());
        assert_eq!(
            meta.context.confidence, None,
            "a default (mechanical) provenance records no surety judgment (ADR 0021 §5)"
        );
    }

    #[test]
    fn new_meta_threads_the_evidence_analysis_into_the_context() {
        let analysis = EvidenceAnalysis {
            source: SourceQuality::Original,
            information: InformationKind::Primary,
            evidence: EvidenceKind::Direct,
        };
        let provenance = Provenance {
            confidence: Some(Confidence::High),
            rationale: None,
            evidence_analysis: Some(analysis),
            origin: None,
        };
        let meta = session().new_meta(provenance, Vec::new());
        assert_eq!(
            meta.context.evidence_analysis,
            Some(analysis),
            "the supplied evidence analysis lands in the EventContext"
        );
        assert_eq!(meta.context.confidence, Some(Confidence::High));
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
