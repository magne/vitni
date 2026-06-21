//! [`Session`] — the one place non-determinism enters the system (ADR 0004 §3, ADR 0006).
//!
//! The decision core is pure: it reads no clock and generates no id. The `Session` supplies those
//! inputs — it stamps the operator [`Agent`], reads the wall clock for `occurred_at`, and mints
//! UUID v7 ids for assertions and new aggregates — so the core stays unit-testable and provenance
//! is recorded identically for every frontend. Keep this type deliberately small: everything that
//! is hard to test lives here and nowhere else.

use genealogy_core::ids::{
    AssertionId, CitationId, DnaMatchId, DnaTestId, EventId, FamilyId, MediaId, NoteId, PersonId, PlaceId,
    RepositoryId, SourceId, TagId,
};
use genealogy_core::provenance::{Agent, AssertionMeta, CitationRef, Confidence, EventContext, Timestamp};
use time::OffsetDateTime;
use uuid::Uuid;

/// Per-invocation context carrying the operator identity and the impure id/clock sources.
#[derive(Debug, Clone)]
pub struct Session {
    operator: Agent,
}

impl Session {
    /// Creates a session for `operator` (resolved from configuration, ADR 0005).
    #[must_use]
    pub fn new(operator: Agent) -> Self {
        Self { operator }
    }

    /// Mints an id for a new Person aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_person_id(&self) -> PersonId {
        PersonId::from_uuid(Uuid::now_v7())
    }

    /// Mints an id for a new Family aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_family_id(&self) -> FamilyId {
        FamilyId::from_uuid(Uuid::now_v7())
    }

    /// Mints an id for a new Place aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_place_id(&self) -> PlaceId {
        PlaceId::from_uuid(Uuid::now_v7())
    }

    /// Mints an id for a new Source aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_source_id(&self) -> SourceId {
        SourceId::from_uuid(Uuid::now_v7())
    }

    /// Mints an id for a new Citation aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_citation_id(&self) -> CitationId {
        CitationId::from_uuid(Uuid::now_v7())
    }

    /// Mints an id for a new Event aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_event_id(&self) -> EventId {
        EventId::from_uuid(Uuid::now_v7())
    }

    /// Mints an id for a new `DnaTest` aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_dna_test_id(&self) -> DnaTestId {
        DnaTestId::from_uuid(Uuid::now_v7())
    }

    /// Mints an id for a new `DnaMatch` aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_dna_match_id(&self) -> DnaMatchId {
        DnaMatchId::from_uuid(Uuid::now_v7())
    }

    /// Mints an id for a new Repository aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_repository_id(&self) -> RepositoryId {
        RepositoryId::from_uuid(Uuid::now_v7())
    }

    /// Mints an id for a new Note aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_note_id(&self) -> NoteId {
        NoteId::from_uuid(Uuid::now_v7())
    }

    /// Mints an id for a new Media aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_media_id(&self) -> MediaId {
        MediaId::from_uuid(Uuid::now_v7())
    }

    /// Mints an id for a new Tag aggregate (UUID v7, time-sortable — ADR 0004 §5).
    #[must_use]
    pub fn new_tag_id(&self) -> TagId {
        TagId::from_uuid(Uuid::now_v7())
    }

    /// Builds the supplied non-deterministic inputs for one command (ADR 0004 §3).
    ///
    /// Generates a fresh [`AssertionId`], reads the clock for `occurred_at`, and copies in the
    /// configured operator. `evidence_analysis` is left unset; the CLI does not collect it yet.
    #[must_use]
    pub fn new_meta(
        &self,
        confidence: Confidence,
        rationale: Option<String>,
        citations: Vec<CitationRef>,
    ) -> AssertionMeta {
        AssertionMeta {
            assertion_id: AssertionId::from_uuid(Uuid::now_v7()),
            context: EventContext {
                operator: self.operator.clone(),
                occurred_at: Timestamp::new(OffsetDateTime::now_utc()),
                rationale,
                confidence,
                citations,
                evidence_analysis: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Session;
    use genealogy_core::ids::AgentId;
    use genealogy_core::provenance::{Agent, AgentKind, Confidence};
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
        let meta = session().new_meta(Confidence::Normal, Some("note".to_owned()), Vec::new());
        assert_eq!(meta.context.operator.id, AgentId::from_uuid(Uuid::from_u128(42)));
        assert_eq!(meta.context.rationale.as_deref(), Some("note"));
    }

    #[test]
    fn successive_assertion_ids_are_distinct_and_time_ordered() {
        let session = session();
        let first = session.new_meta(Confidence::Normal, None, Vec::new()).assertion_id;
        let second = session.new_meta(Confidence::Normal, None, Vec::new()).assertion_id;
        assert_ne!(first, second, "each assertion gets its own id");
        assert!(first.as_uuid() <= second.as_uuid(), "UUID v7 ids are monotonic by time");
    }
}
