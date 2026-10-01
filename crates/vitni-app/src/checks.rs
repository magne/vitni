//! Data-quality checks (Phase 5 PR 34): a small, string-free framework of scans over the workspace
//! projections, each returning a typed finding the dashboard turns into a counted, navigable row.
//!
//! A check is a pure function over already-projected read models: its [`CheckFinding`] variant names
//! the check and the record(s) it flags as [`AggRef`]s, so the frontend can localize the label and build navigable
//! targets (ADR 0003 keeps this crate free of display strings). New checks (orphaned records,
//! implausible ages, …) slot in as another per-check function plus a line in [`run_checks`] — no
//! registry, no trait objects.
//!
//! Two checks ship here:
//! - [`CheckFinding::DeathBeforeBirth`] — a per-person date-sanity scan flagging anyone whose known
//!   death year precedes their known birth year.
//! - [`CheckFinding::PossibleDuplicate`] — the matching engine's [`similar_pairs`] of every
//!   [`MatchableKind`] at least [`MatchBand::Possible`], one finding per pair with the engine's evidence
//!   (ADR 0038 §8). A pair the user already decided is left out (ADR 0039 §3).

use vitni_core::matching::{MatchBand, MatchEvidence, MatchableKind};

use crate::dto::AggRef;
use crate::error::AppError;
use crate::person::{PersonSummary, list_persons};
use crate::similar::similar_pairs;
use crate::workspace::Workspace;

/// A single data-quality finding: which check fired, the record(s) it flags, and what it found.
///
/// A closed enum the frontend localizes to its own display text (ADR 0003 keeps this crate
/// string-free), and builds a navigable target for each record from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckFinding {
    /// A person whose known death year precedes their known birth year.
    DeathBeforeBirth(AggRef),
    /// Two records of one kind the engine judges possibly the same.
    PossibleDuplicate {
        /// The kind of both records.
        kind: MatchableKind,
        /// The first record, the lower aggregate id.
        a: AggRef,
        /// The second record.
        b: AggRef,
        /// The engine's assessment of the pair: its score, band and the terms behind them.
        assessment: MatchEvidence,
    },
}

/// Runs every data-quality check against the workspace, returning one [`CheckFinding`] per flag.
///
/// A scan over projections plus the matching engine — no new events.
///
/// # Errors
///
/// A store/read-model error, or the matching engine's error when its data or settings cannot be
/// loaded.
pub async fn run_checks(workspace: &Workspace) -> Result<Vec<CheckFinding>, AppError> {
    let persons = list_persons(workspace).await?;
    check_records(workspace, &persons).await
}

/// Runs every data-quality check, reading the persons from an already-loaded projection.
///
/// The core of [`run_checks`], exposed so a caller that already holds the person list (the dashboard,
/// which also needs it for evidence health and activity names) runs the checks without a second
/// [`list_persons`] load. The death-before-birth findings come first, then the duplicates of every
/// kind, the most similar first.
///
/// # Errors
///
/// A store/read-model error, or the matching engine's error when its data or settings cannot be
/// loaded.
pub async fn check_records(workspace: &Workspace, persons: &[PersonSummary]) -> Result<Vec<CheckFinding>, AppError> {
    let mut duplicates = Vec::new();
    for kind in MatchableKind::ALL {
        for pair in similar_pairs(workspace, kind, MatchBand::Possible).await? {
            duplicates.push((kind, pair.a, pair.b, pair.assessment.evidence()));
        }
    }
    duplicates.sort_by_key(|(_, _, _, evidence)| std::cmp::Reverse((evidence.band, evidence.score_bp)));
    let mut findings = death_before_birth(persons);
    for (kind, a, b, assessment) in duplicates {
        findings.push(CheckFinding::PossibleDuplicate { kind, a, b, assessment });
    }
    Ok(findings)
}

/// Flags each person whose known death year precedes their known birth year.
///
/// A person with an unknown birth or death year is never flagged — the check needs both to compare.
fn death_before_birth(persons: &[PersonSummary]) -> Vec<CheckFinding> {
    let mut findings = Vec::new();
    for person in persons {
        let (Some(birth), Some(death)) = (person.birth_year(), person.death_year()) else {
            continue;
        };
        if death < birth {
            findings.push(CheckFinding::DeathBeforeBirth(AggRef {
                human_id: person.human_id.clone(),
                id: person.human_id.clone(),
            }));
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::{CheckFinding, death_before_birth, run_checks};
    use crate::config::{AppDefaults, IdFormats, OperatorConfig, WorkspaceDefaults};
    use crate::event::{DateParts, NewEvent, assert_event_date, create_event};
    use crate::person::{
        NewParticipation, NewPerson, PersonNameParts, assert_participation, create_person, list_persons,
    };
    use crate::session::Session;
    use crate::use_case::{MutationMeta, Provenance};
    use crate::workspace::Workspace;
    use tempfile::TempDir;
    use uuid::Uuid;
    use vitni_core::enums::{EventType, EvidenceLevel, ParticipantRole};
    use vitni_core::ids::AgentId;
    use vitni_core::matching::{MatchBand, MatchableKind};
    use vitni_core::provenance::{Agent, AgentKind, Confidence};

    fn operator() -> OperatorConfig {
        OperatorConfig {
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
            email: None,
        }
    }

    fn defaults() -> WorkspaceDefaults {
        WorkspaceDefaults {
            id_formats: IdFormats {
                person: "I%04d".to_owned(),
                ..IdFormats::default()
            },
            ..Default::default()
        }
    }

    fn session() -> Session {
        Session::new(Agent {
            kind: AgentKind::Human,
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
        })
    }

    async fn setup() -> (Workspace, Session, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
        let workspace = Workspace::open(&ws, &operator(), &defaults()).await.expect("open");
        (workspace, session(), dir)
    }

    async fn person(workspace: &Workspace, session: &Session, given: &str, surname: &str) -> String {
        create_person(
            workspace,
            session,
            NewPerson {
                human_id: None,
                name: Some(PersonNameParts::simple(
                    Some(given.to_owned()),
                    Some(surname.to_owned()),
                )),
                evidence_level: EvidenceLevel::Conclusion,
                external_ids: Vec::new(),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("create person")
    }

    /// Asserts a dated vital event with the person as its Primary participant — the promoted shape a
    /// vital claim takes (ADR 0021 §2), so the scan reads its year from the event projection.
    async fn with_vital_year(
        workspace: &Workspace,
        session: &Session,
        human_id: &str,
        event_type: EventType,
        year: i32,
    ) {
        let event_id = create_event(
            workspace,
            session,
            NewEvent {
                human_id: None,
                event_type,
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("create vital event");
        assert_event_date(
            workspace,
            session,
            &event_id,
            DateParts {
                year,
                month: None,
                day: None,
            },
            MutationMeta {
                provenance: Provenance::default(),
                citations: &[],
                dna_matches: &[],
                supersedes: None,
            },
        )
        .await
        .expect("assert event date");
        assert_participation(
            workspace,
            session,
            human_id,
            &event_id,
            NewParticipation::with_role(ParticipantRole::Primary),
            MutationMeta {
                provenance: Provenance {
                    confidence: Some(Confidence::Normal),
                    rationale: None,
                    evidence_analysis: None,
                    origin: None,
                },
                citations: &[],
                dna_matches: &[],
                supersedes: None,
            },
        )
        .await
        .expect("assert participation");
    }

    #[tokio::test]
    async fn flags_a_person_whose_death_precedes_birth() {
        let (workspace, session, _dir) = setup().await;
        let subject = person(&workspace, &session, "Ada", "Reversed").await;
        with_vital_year(&workspace, &session, &subject, EventType::Birth, 1900).await;
        with_vital_year(&workspace, &session, &subject, EventType::Death, 1880).await;

        let persons = list_persons(&workspace).await.expect("list");
        let findings = death_before_birth(&persons);
        assert_eq!(
            findings.len(),
            1,
            "the reversed-lifespan person must be flagged: {findings:?}"
        );
        let CheckFinding::DeathBeforeBirth(record) = &findings[0] else {
            panic!("a death-before-birth finding: {findings:?}");
        };
        assert_eq!(record.human_id, subject);
    }

    #[tokio::test]
    async fn does_not_flag_death_after_birth() {
        let (workspace, session, _dir) = setup().await;
        let subject = person(&workspace, &session, "Bo", "Ordered").await;
        with_vital_year(&workspace, &session, &subject, EventType::Birth, 1880).await;
        with_vital_year(&workspace, &session, &subject, EventType::Death, 1950).await;

        let persons = list_persons(&workspace).await.expect("list");
        let checked = death_before_birth(&persons);
        assert!(checked.is_empty(), "{checked:?}");
    }

    #[tokio::test]
    async fn does_not_flag_same_birth_and_death_year() {
        let (workspace, session, _dir) = setup().await;
        let subject = person(&workspace, &session, "Cy", "Sameyear").await;
        with_vital_year(&workspace, &session, &subject, EventType::Birth, 1900).await;
        with_vital_year(&workspace, &session, &subject, EventType::Death, 1900).await;

        let persons = list_persons(&workspace).await.expect("list");
        let checked = death_before_birth(&persons);
        assert!(checked.is_empty(), "{checked:?}");
    }

    #[tokio::test]
    async fn does_not_flag_when_a_year_is_missing() {
        let (workspace, session, _dir) = setup().await;
        let birth_only = person(&workspace, &session, "Di", "Birthonly").await;
        with_vital_year(&workspace, &session, &birth_only, EventType::Birth, 1900).await;
        let death_only = person(&workspace, &session, "El", "Deathonly").await;
        with_vital_year(&workspace, &session, &death_only, EventType::Death, 1880).await;
        person(&workspace, &session, "Fi", "Neither").await;

        let persons = list_persons(&workspace).await.expect("list");
        let checked = death_before_birth(&persons);
        assert!(checked.is_empty(), "{checked:?}");
    }

    #[tokio::test]
    async fn empty_workspace_has_no_findings() {
        let (workspace, _session, _dir) = setup().await;
        let checked = run_checks(&workspace).await.expect("run checks");
        assert!(checked.is_empty(), "{checked:?}");
    }

    #[tokio::test]
    async fn run_checks_surfaces_duplicate_pairs() {
        let (workspace, session, _dir) = setup().await;
        let a = person(&workspace, &session, "John", "Smith").await;
        let b = person(&workspace, &session, "John", "Smyth").await;

        let duplicates = crate::similar::similar_pairs(&workspace, MatchableKind::Person, MatchBand::Possible)
            .await
            .expect("duplicates");
        let findings = run_checks(&workspace).await.expect("run checks");
        let mut duplicate_findings = Vec::new();
        for finding in &findings {
            if let CheckFinding::PossibleDuplicate {
                kind: MatchableKind::Person,
                a: first,
                b: second,
                assessment,
            } = finding
            {
                duplicate_findings.push((first.human_id.clone(), second.human_id.clone(), assessment.clone()));
            }
        }
        assert_eq!(duplicate_findings.len(), duplicates.len());
        assert!(
            duplicate_findings.iter().any(|(first, second, assessment)| {
                ((*first == a && *second == b) || (*first == b && *second == a))
                    && assessment.band >= MatchBand::Possible
            }),
            "the Smith/Smyth pair must surface as a duplicates finding: {findings:?}"
        );
    }
}
