//! Duplicate-person detection (Phase 5 PR 19): a typed finding over the Person projection so the
//! Compare/merge screen's "possible duplicates" table — and, later, the dashboard's own duplicates
//! check (`docs/phase5/plan.md`, "Possible duplicates overlaps PR19") — can reuse the same scan.
//!
//! This is a pure, in-memory heuristic over already-projected [`PersonSummary`]s: no new events, no
//! extra I/O beyond [`list_persons`]. It flags two kinds of pairs (data, not display strings — the
//! frontend localizes [`MatchKind`], keeping this crate string-free per ADR 0003):
//! - [`MatchKind::NameVariant`] — same normalized surname, given names close but not identical
//!   (e.g. "Smith"/"Smyth" surname typo, or "Jon"/"Jonathan" given-name variants).
//! - [`MatchKind::SameBirthYear`] — identical normalized name, birth years within
//!   [`BIRTH_YEAR_WINDOW`] of each other.
//!
//! O(n²) over persons; acceptable for a v1 heuristic (no performance claim is made — see the task
//! brief). A dependency-free Levenshtein distance drives the "close but not identical" name check
//! rather than pulling in a string-similarity crate for one small comparison.

use genealogy_core::enums::FactType;

use crate::dto::{AggRef, year_of_fact};
use crate::error::AppError;
use crate::person::{PersonSummary, list_persons};
use crate::workspace::Workspace;

/// How many years apart two identically-named persons' birth years may be and still count as
/// [`MatchKind::SameBirthYear`] (inclusive).
const BIRTH_YEAR_WINDOW: i32 = 3;

/// The maximum Levenshtein distance between two normalized given or surname strings that still
/// counts as a "variant" of each other for [`MatchKind::NameVariant`].
const NAME_VARIANT_MAX_DISTANCE: usize = 2;

/// Why a pair of persons was flagged as a possible duplicate — a small closed enum the frontend
/// localizes to its own display text (ADR 0003 keeps this crate string-free).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// The names are close but not identical (a likely transcription/spelling variant).
    NameVariant,
    /// The (normalized) names are identical and the birth years are close.
    SameBirthYear,
}

/// A candidate duplicate pair: two persons, why they were flagged, and a rough confidence score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateCandidate {
    /// The first person in the pair.
    pub a: AggRef,
    /// The second person in the pair.
    pub b: AggRef,
    /// Why the pair was flagged.
    pub kind: MatchKind,
    /// A rough confidence score in `0..=100` (higher = more likely a true duplicate). Not a
    /// [`genealogy_core::provenance::Confidence`] — this is a heuristic match score, not an
    /// operator's asserted surety.
    pub score: u8,
}

/// Scans every unordered pair of persons in the workspace for possible duplicates.
///
/// Returns an empty vector for a workspace with fewer than two persons. Pairs with clearly distinct
/// names and clearly distinct (or unknown) birth years are never flagged.
///
/// # Errors
///
/// A store/read-model error from the underlying [`list_persons`] scan.
pub async fn find_duplicate_candidates(workspace: &Workspace) -> Result<Vec<DuplicateCandidate>, AppError> {
    let persons = list_persons(workspace).await?;
    let mut candidates = Vec::new();
    for i in 0..persons.len() {
        for j in (i + 1)..persons.len() {
            if let Some(candidate) = compare(&persons[i], &persons[j]) {
                candidates.push(candidate);
            }
        }
    }
    Ok(candidates)
}

/// Compares two persons, returning a [`DuplicateCandidate`] if either heuristic fires.
///
/// Checked in order: an identical normalized name with close birth years fires
/// [`MatchKind::SameBirthYear`] first (the stronger signal); otherwise a close-but-not-identical name
/// fires [`MatchKind::NameVariant`].
fn compare(a: &PersonSummary, b: &PersonSummary) -> Option<DuplicateCandidate> {
    let name_a = normalized_name(a)?;
    let name_b = normalized_name(b)?;
    let birth_a = year_of_fact(a, &FactType::Birth);
    let birth_b = year_of_fact(b, &FactType::Birth);

    if name_a == name_b {
        let (Some(year_a), Some(year_b)) = (birth_a, birth_b) else {
            return None;
        };
        if (year_a - year_b).abs() <= BIRTH_YEAR_WINDOW {
            let score = same_birth_year_score(year_a, year_b);
            return Some(candidate(a, b, MatchKind::SameBirthYear, score));
        }
        return None;
    }

    let distance = levenshtein(&name_a, &name_b);
    if (1..=NAME_VARIANT_MAX_DISTANCE).contains(&distance) {
        let score = name_variant_score(distance);
        return Some(candidate(a, b, MatchKind::NameVariant, score));
    }
    None
}

/// Builds the [`DuplicateCandidate`] DTO for a flagged pair.
fn candidate(a: &PersonSummary, b: &PersonSummary, kind: MatchKind, score: u8) -> DuplicateCandidate {
    DuplicateCandidate {
        a: AggRef {
            human_id: a.human_id.clone(),
            id: a.human_id.clone(),
        },
        b: AggRef {
            human_id: b.human_id.clone(),
            id: b.human_id.clone(),
        },
        kind,
        score,
    }
}

/// The case/whitespace-normalized "given surname" display name, or `None` if the person has none.
fn normalized_name(summary: &PersonSummary) -> Option<String> {
    let name = summary.display_name.as_ref()?;
    let normalized = name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    (!normalized.is_empty()).then_some(normalized)
}

/// A rough confidence score for a same-name, close-birth-year match: highest for an exact year
/// match, decreasing with the gap.
fn same_birth_year_score(year_a: i32, year_b: i32) -> u8 {
    let gap = (year_a - year_b).unsigned_abs();
    let penalty = u8::try_from(gap.saturating_mul(10)).unwrap_or(u8::MAX);
    95_u8.saturating_sub(penalty)
}

/// A rough confidence score for a name-variant match: highest for a 1-character difference,
/// decreasing with the edit distance.
fn name_variant_score(distance: usize) -> u8 {
    let penalty = u8::try_from(distance.saturating_mul(20)).unwrap_or(u8::MAX);
    80_u8.saturating_sub(penalty)
}

/// The Levenshtein (edit) distance between two strings, hand-rolled to avoid pulling in a
/// string-similarity crate for this one heuristic (`~O(len_a * len_b)`, fine for short names).
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous_row: Vec<usize> = (0..=b.len()).collect();
    let mut current_row = vec![0_usize; b.len() + 1];

    for (i, &char_a) in a.iter().enumerate() {
        current_row[0] = i + 1;
        for (j, &char_b) in b.iter().enumerate() {
            let cost = usize::from(char_a != char_b);
            current_row[j + 1] = (previous_row[j] + cost)
                .min(previous_row[j + 1] + 1)
                .min(current_row[j] + 1);
        }
        std::mem::swap(&mut previous_row, &mut current_row);
    }
    previous_row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::{MatchKind, find_duplicate_candidates, levenshtein};
    use crate::config::{AppDefaults, IdFormats, OperatorConfig, WorkspaceDefaults};
    use crate::event::{DateInput, build_genealogical_date};
    use crate::person::{NewFact, NewPerson, PersonNameParts, assert_fact, create_person};
    use crate::session::Session;
    use crate::use_case::Provenance;
    use crate::workspace::Workspace;
    use genealogy_core::date::{
        Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody,
    };
    use genealogy_core::enums::{EvidenceLevel, FactType};
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
        )
        .await
        .expect("create person")
    }

    fn birth_year(year: i32) -> GenealogicalDate {
        build_genealogical_date(DateInput {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            body: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
                year: Some(year),
                month: None,
                day: None,
            })),
            new_year_begins: None,
            original_text: None,
        })
    }

    async fn with_birth_year(workspace: &Workspace, session: &Session, human_id: &str, year: i32) {
        assert_fact(
            workspace,
            session,
            human_id,
            NewFact {
                fact_type: FactType::Birth,
                value: None,
                date: Some(birth_year(year)),
            },
            Provenance {
                confidence: Confidence::Normal,
                rationale: None,
            },
            &[],
        )
        .await
        .expect("assert birth");
    }

    #[test]
    fn levenshtein_counts_single_character_edits() {
        assert_eq!(levenshtein("smith", "smith"), 0);
        assert_eq!(levenshtein("smith", "smyth"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
    }

    #[tokio::test]
    async fn empty_workspace_returns_no_candidates() {
        let (workspace, _session, _dir) = setup().await;
        let candidates = find_duplicate_candidates(&workspace).await.expect("scan");
        assert!(candidates.is_empty());
    }

    #[tokio::test]
    async fn flags_a_name_variant_pair() {
        let (workspace, session, _dir) = setup().await;
        let a = person(&workspace, &session, "John", "Smith").await;
        let b = person(&workspace, &session, "John", "Smyth").await;

        let candidates = find_duplicate_candidates(&workspace).await.expect("scan");
        let pair = candidates
            .iter()
            .find(|c| (c.a.human_id == a && c.b.human_id == b) || (c.a.human_id == b && c.b.human_id == a))
            .expect("name-variant pair flagged");
        assert_eq!(pair.kind, MatchKind::NameVariant);
    }

    #[tokio::test]
    async fn flags_a_same_birth_year_pair() {
        let (workspace, session, _dir) = setup().await;
        let a = person(&workspace, &session, "Mary", "Doe").await;
        let b = person(&workspace, &session, "Mary", "Doe").await;
        with_birth_year(&workspace, &session, &a, 1900).await;
        with_birth_year(&workspace, &session, &b, 1902).await;

        let candidates = find_duplicate_candidates(&workspace).await.expect("scan");
        let pair = candidates
            .iter()
            .find(|c| (c.a.human_id == a && c.b.human_id == b) || (c.a.human_id == b && c.b.human_id == a))
            .expect("same-birth-year pair flagged");
        assert_eq!(pair.kind, MatchKind::SameBirthYear);
    }

    #[tokio::test]
    async fn does_not_flag_clearly_distinct_persons() {
        let (workspace, session, _dir) = setup().await;
        let a = person(&workspace, &session, "Alice", "Anderson").await;
        let b = person(&workspace, &session, "Bob", "Baker").await;
        with_birth_year(&workspace, &session, &a, 1900).await;
        with_birth_year(&workspace, &session, &b, 1980).await;

        let candidates = find_duplicate_candidates(&workspace).await.expect("scan");
        assert!(
            candidates
                .iter()
                .all(|c| { !((c.a.human_id == a && c.b.human_id == b) || (c.a.human_id == b && c.b.human_id == a)) }),
            "clearly distinct persons must not be flagged: {candidates:?}"
        );
    }
}
