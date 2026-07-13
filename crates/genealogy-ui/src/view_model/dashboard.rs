use super::{ActivityVm, ChangeLogEntry, HashMap, Localizer, PersonSummary, RecordRef, WorkspaceCounts};
use crate::navigation::Category;
use genealogy_app::{CheckFinding, CheckKind};

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

/// The dashboard view-model: stats, the recent-activity feed, quick entry points, and the
/// data-quality check results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardVm {
    /// The headline counts and evidence-health gauge.
    pub stats: DashboardStats,
    /// The most recent workspace-wide changes, newest first.
    pub recent: Vec<ActivityVm>,
    /// Quick entry points to recently touched records.
    pub jump_back: Vec<JumpVm>,
    /// Persons flagged by the death-before-birth check, as navigable record references (the Review
    /// action lists these; the row count is their number).
    pub death_before_birth: Vec<RecordRef>,
    /// How many possible-duplicate pairs the detector flagged (the row's Compare action routes into
    /// the merge wizard rather than to individual records).
    pub duplicate_count: usize,
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
        findings: &[CheckFinding],
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
        let mut death_before_birth = Vec::new();
        let mut duplicate_count = 0usize;
        for finding in findings {
            match finding.kind {
                CheckKind::DeathBeforeBirth => {
                    for record in &finding.records {
                        let label = names
                            .get(&record.human_id)
                            .cloned()
                            .unwrap_or_else(|| record.human_id.clone());
                        death_before_birth.push(RecordRef {
                            category: Category::People,
                            human_id: record.human_id.clone(),
                            label,
                        });
                    }
                }
                CheckKind::PossibleDuplicates => duplicate_count += 1,
            }
        }
        Self {
            stats: DashboardStats::build(counts, persons),
            recent,
            jump_back,
            death_before_birth,
            duplicate_count,
        }
    }
}
