use super::{ActivityVm, ChangeLogEntry, HashMap, Localizer, PersonSummary, QueuedMatchVm, RecordRef, WorkspaceCounts};
use crate::navigation::Category;
use vitni_app::{AggRef, CheckFinding, DecidableKind, MatchableKind};

/// A quick entry point on the dashboard ("Jump back in") — a recently touched record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpVm {
    /// The record to open.
    pub record: RecordRef,
}

/// The dashboard's headline counts and evidence-health gauge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardStats {
    /// Persons in the workspace.
    pub people: u64,
    /// Families in the workspace.
    pub families: u64,
    /// Events in the workspace.
    pub events: u64,
    /// Percent of facts backed by at least one source (0–100).
    pub evidence_health_pct: u8,
    /// How many facts lack any source (the no-source flag count).
    pub facts_without_source: usize,
    /// Total facts considered (the evidence-health denominator).
    pub facts_total: usize,
}

impl DashboardStats {
    /// Builds the stats from the per-aggregate counts and the persons' facts.
    ///
    /// Evidence health is the share of facts carrying at least one citation; with no facts it is
    /// reported as 100% (nothing is unsourced).
    #[must_use]
    pub fn build(counts: WorkspaceCounts, persons: &[PersonSummary]) -> Self {
        let mut facts_total = 0usize;
        let mut facts_with_source = 0usize;
        for person in persons {
            for fact in &person.facts {
                facts_total += 1;
                if !fact.citations.is_empty() {
                    facts_with_source += 1;
                }
            }
        }
        // With no facts, nothing is unsourced — report full health (checked_div yields None at 0).
        let evidence_health_pct = (facts_with_source * 100)
            .checked_div(facts_total)
            .and_then(|pct| u8::try_from(pct).ok())
            .unwrap_or(100);
        Self {
            people: counts.person,
            families: counts.family,
            events: counts.event,
            evidence_health_pct,
            facts_without_source: facts_total - facts_with_source,
            facts_total,
        }
    }
}

/// The dashboard view-model: stats, the recent-activity feed, and quick entry points.
///
/// The data-quality check results live in [`DataQualityVm`] instead, filled by a second load so the
/// dashboard renders its fast parts (counts, activity, evidence health, jump-back) without waiting on
/// the whole-workspace check pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardVm {
    /// The headline counts and evidence-health gauge.
    pub stats: DashboardStats,
    /// The most recent workspace-wide changes, newest first.
    pub recent: Vec<ActivityVm>,
    /// Quick entry points to recently touched records.
    pub jump_back: Vec<JumpVm>,
}

impl DashboardVm {
    /// Assembles the dashboard from counts, the persons (for evidence health), and recent activity.
    ///
    /// "Jump back in" is the distinct navigable records drawn from the most recent activity, capped
    /// at `jump_limit`.
    #[must_use]
    pub fn build(
        counts: WorkspaceCounts,
        persons: &[PersonSummary],
        activity: &[ChangeLogEntry],
        loc: &Localizer,
        jump_limit: usize,
    ) -> Self {
        let names: HashMap<String, String> = persons
            .iter()
            .filter_map(|person| person.display_name.clone().map(|name| (person.human_id.clone(), name)))
            .collect();
        let recent: Vec<ActivityVm> = activity
            .iter()
            .map(|entry| ActivityVm::from_entry(entry, loc, &names))
            .collect();
        let mut jump_back = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for row in &recent {
            let Some(record) = &row.record else { continue };
            if seen.insert(record.human_id.clone()) {
                jump_back.push(JumpVm { record: record.clone() });
                if jump_back.len() >= jump_limit {
                    break;
                }
            }
        }
        Self {
            stats: DashboardStats::build(counts, persons),
            recent,
            jump_back,
        }
    }
}

/// The dashboard's data-quality results — the slow, whole-workspace check pass filled by a second
/// load so the fast dashboard need not wait on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataQualityVm {
    /// Persons flagged by the death-before-birth check, as navigable record references (the Review
    /// action lists these; the row count is their number).
    pub death_before_birth: Vec<RecordRef>,
    /// The undecided possible matches of every decidable kind, the most similar first (the
    /// *Possible matches* card lists the strongest and opens the Matches tool).
    pub matches: Vec<QueuedMatchVm>,
    /// How many of [`matches`](Self::matches) each kind has, already localized ("Place: 2"), in
    /// kind order, kinds without one left out.
    pub match_counts: Vec<String>,
}

impl DataQualityVm {
    /// Groups the data-quality `findings` into the per-check shapes the card renders, resolving each
    /// flagged person's display name from `persons`.
    #[must_use]
    pub fn build(persons: &[PersonSummary], findings: &[CheckFinding], loc: &Localizer) -> Self {
        let names: HashMap<String, String> = persons
            .iter()
            .filter_map(|person| person.display_name.clone().map(|name| (person.human_id.clone(), name)))
            .collect();
        let mut death_before_birth = Vec::new();
        let mut matches = Vec::new();
        for finding in findings {
            match finding {
                CheckFinding::DeathBeforeBirth(record) => {
                    death_before_birth.push(record_ref(MatchableKind::Person, record, &names));
                }
                CheckFinding::PossibleDuplicate { kind, a, b, assessment } => {
                    if let Some(kind) = DecidableKind::from_matchable(*kind) {
                        matches.push(QueuedMatchVm::build(kind, (a, b), assessment, &names, loc));
                    }
                }
            }
        }
        let mut match_counts = Vec::new();
        for kind in DecidableKind::ALL {
            let count = matches.iter().filter(|queued| queued.kind == kind).count();
            if count > 0 {
                match_counts.push(loc.match_kind_count(kind, count));
            }
        }
        Self {
            death_before_birth,
            matches,
            match_counts,
        }
    }
}

/// A flagged record as a navigable reference: a person by display name, a tag by its name (it opens by
/// its id), anything else by its human id — the record link shows the live name.
pub(super) fn record_ref(kind: MatchableKind, record: &AggRef, names: &HashMap<String, String>) -> RecordRef {
    let (human_id, label) = match kind {
        MatchableKind::Tag => (record.id.clone(), record.human_id.clone()),
        MatchableKind::Person => {
            let label = names.get(&record.human_id).unwrap_or(&record.human_id);
            (record.human_id.clone(), label.clone())
        }
        MatchableKind::Family
        | MatchableKind::Event
        | MatchableKind::Place
        | MatchableKind::Source
        | MatchableKind::Repository
        | MatchableKind::Citation
        | MatchableKind::Media
        | MatchableKind::Note => (record.human_id.clone(), record.human_id.clone()),
    };
    RecordRef {
        category: Category::from_matchable_kind(kind),
        human_id,
        label,
    }
}
