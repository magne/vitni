//! Data-quality checks (Phase 5 PR 34): a small, string-free framework of scans over the workspace
//! projections, each returning a typed finding the dashboard turns into a counted, navigable row.
//!
//! A check is a pure function over already-projected read models: it names its [`CheckKind`] and the
//! record(s) it flags as [`AggRef`]s, so the frontend can localize the label and build navigable
//! targets (ADR 0003 keeps this crate free of display strings). New checks (orphaned records,
//! implausible ages, …) slot in as another per-check function plus a line in [`run_checks`] — no
//! registry, no trait objects.
//!
//! Two checks ship here:
//! - [`CheckKind::DeathBeforeBirth`] — a per-person date-sanity scan flagging anyone whose known
//!   death year precedes their known birth year.
//! - [`CheckKind::PossibleDuplicates`] — delegated to [`find_duplicate_candidates`], one finding per
//!   flagged pair (the same pairs the Compare/merge screen shows — the detector is built once there).

use crate::dto::AggRef;
use crate::duplicates::find_duplicate_candidates;
use crate::error::AppError;
use crate::person::{PersonSummary, list_persons};
use crate::workspace::Workspace;

/// Which data-quality check produced a finding — a closed enum the frontend localizes to its own
/// display text (ADR 0003 keeps this crate string-free).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    /// A person whose known death year precedes their known birth year.
    DeathBeforeBirth,
    /// A pair of persons flagged as a possible duplicate.
    PossibleDuplicates,
}

/// A single data-quality finding: which check fired and the record(s) it flags.
///
/// `records` holds one [`AggRef`] for a per-record check ([`CheckKind::DeathBeforeBirth`]) and the
/// pair for [`CheckKind::PossibleDuplicates`], so the frontend can build a navigable target for each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckFinding {
    /// Which check produced this finding.
    pub kind: CheckKind,
    /// The record(s) the finding flags (one for a per-record check, a pair for duplicates).
    pub records: Vec<AggRef>,
}

/// Runs every data-quality check against the workspace, returning one [`CheckFinding`] per flag.
///
/// A pure scan over projections plus the duplicate detector — no new events, no I/O beyond the list
/// scans. Findings for different [`CheckKind`]s are interleaved in check order; the caller groups by
/// kind.
///
/// # Errors
///
/// A store/read-model error from the underlying [`list_persons`]/[`find_duplicate_candidates`] scans.
pub async fn run_checks(workspace: &Workspace) -> Result<Vec<CheckFinding>, AppError> {
    let persons = list_persons(workspace).await?;
    let mut findings = death_before_birth(&persons);
    for candidate in find_duplicate_candidates(workspace).await? {
        findings.push(CheckFinding {
            kind: CheckKind::PossibleDuplicates,
            records: vec![candidate.a, candidate.b],
        });
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
            findings.push(CheckFinding {
                kind: CheckKind::DeathBeforeBirth,
                records: vec![AggRef {
                    human_id: person.human_id.clone(),
                    id: person.human_id.clone(),
                }],
            });
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::{CheckKind, death_before_birth, run_checks};
    use crate::config::{AppDefaults, IdFormats, OperatorConfig, WorkspaceDefaults};
    use crate::event::{DateParts, NewEvent, assert_event_date, create_event};
    use crate::person::{
        NewParticipation, NewPerson, PersonNameParts, assert_participation, create_person, list_persons,
    };
    use crate::session::Session;
    use crate::use_case::{MutationMeta, Provenance};
    use crate::workspace::Workspace;
    use genealogy_core::enums::{EventType, EvidenceLevel, ParticipantRole};
    use genealogy_core::ids::AgentId;
    use genealogy_core::provenance::{Agent, AgentKind, Confidence};
    use tempfile::TempDir;
    use uuid::Uuid;

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
                },
                citations: &[],
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
        assert_eq!(findings[0].kind, CheckKind::DeathBeforeBirth);
        assert_eq!(findings[0].records[0].human_id, subject);
    }

    #[tokio::test]
    async fn does_not_flag_death_after_birth() {
        let (workspace, session, _dir) = setup().await;
        let subject = person(&workspace, &session, "Bo", "Ordered").await;
        with_vital_year(&workspace, &session, &subject, EventType::Birth, 1880).await;
        with_vital_year(&workspace, &session, &subject, EventType::Death, 1950).await;

        let persons = list_persons(&workspace).await.expect("list");
        assert!(death_before_birth(&persons).is_empty());
    }

    #[tokio::test]
    async fn does_not_flag_same_birth_and_death_year() {
        let (workspace, session, _dir) = setup().await;
        let subject = person(&workspace, &session, "Cy", "Sameyear").await;
        with_vital_year(&workspace, &session, &subject, EventType::Birth, 1900).await;
        with_vital_year(&workspace, &session, &subject, EventType::Death, 1900).await;

        let persons = list_persons(&workspace).await.expect("list");
        assert!(death_before_birth(&persons).is_empty());
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
        assert!(death_before_birth(&persons).is_empty());
    }

    #[tokio::test]
    async fn empty_workspace_has_no_findings() {
        let (workspace, _session, _dir) = setup().await;
        assert!(run_checks(&workspace).await.expect("run checks").is_empty());
    }

    #[tokio::test]
    async fn run_checks_surfaces_duplicate_pairs() {
        let (workspace, session, _dir) = setup().await;
        let a = person(&workspace, &session, "John", "Smith").await;
        let b = person(&workspace, &session, "John", "Smyth").await;

        let duplicates = crate::duplicates::find_duplicate_candidates(&workspace)
            .await
            .expect("duplicates");
        let findings = run_checks(&workspace).await.expect("run checks");
        let duplicate_findings: Vec<_> = findings
            .iter()
            .filter(|finding| finding.kind == CheckKind::PossibleDuplicates)
            .collect();
        assert_eq!(duplicate_findings.len(), duplicates.len());
        assert!(
            duplicate_findings.iter().any(|finding| {
                (finding.records[0].human_id == a && finding.records[1].human_id == b)
                    || (finding.records[0].human_id == b && finding.records[1].human_id == a)
            }),
            "the Smith/Smyth pair must surface as a duplicates finding: {findings:?}"
        );
    }
}
