use vitni_app::{ActivityDetail, RunRef, RunResume};

use super::{Category, ChangeLogEntry, HashMap, Localizer, RecordRef};

/// One change-log entry, for the History tab — who changed what, when, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntryVm {
    /// The localized timestamp (e.g. `2026-06-22 14:35`).
    pub when: String,
    /// The localized summary of what changed (e.g. `Name asserted`).
    pub what: String,
    /// The localized operator line (e.g. `magne · High` or `gedcom-import (software agent)`).
    pub who: String,
    /// The operator's rationale, if recorded.
    pub why: Option<String>,
    /// The assertion this entry recorded (the undo target).
    pub assertion_id: String,
    /// Whether this entry can be undone (drives the undo control).
    pub can_undo: bool,
    /// For an import-run row, the localized count of the changes it folds (e.g. `4 changes`), shown
    /// muted beside it; `None` for any other entry.
    pub count: Option<String>,
    /// For an identity decision made on the matching engine's assessment, its localized summary
    /// (ADR 0039 §2); `None` for any other entry.
    pub evidence: Option<String>,
    /// For an import-run row, the entries it folds, newest first; empty otherwise.
    pub children: Vec<HistoryEntryVm>,
}

impl HistoryEntryVm {
    /// Builds a history view-model from an app [`ChangeLogEntry`], localizing the summary + operator.
    ///
    /// An import-run row counts the changes it folds on this one record, not the records the run
    /// imported, which would overstate what happened here (issue #306).
    #[must_use]
    pub fn from_entry(entry: &ChangeLogEntry, loc: &Localizer) -> Self {
        let (count, children) = match &entry.detail {
            Some(ActivityDetail::ImportRun { count, children, .. }) => (
                Some(loc.import_run_changes(*count)),
                children.iter().map(|child| Self::from_entry(child, loc)).collect(),
            ),
            Some(ActivityDetail::Fact { .. } | ActivityDetail::IdentityDecision { .. }) | None => (None, Vec::new()),
        };
        let evidence = match &entry.detail {
            Some(ActivityDetail::IdentityDecision { assessment }) => Some(loc.identity_assessment(assessment)),
            Some(ActivityDetail::Fact { .. } | ActivityDetail::ImportRun { .. }) | None => None,
        };
        Self {
            when: friendly_timestamp(&entry.occurred_at),
            what: loc.change_summary(entry),
            who: loc.operator_line(entry),
            why: entry.rationale.clone().or_else(|| superseded_why(entry, loc)),
            evidence,
            assertion_id: entry.assertion_id.clone(),
            can_undo: entry.can_undo,
            count,
            children,
        }
    }
}

/// The generated reason on an import run's supersession: the file's export date, which is what let
/// it replace a value recorded no later (ADR 0049). `None` for any other entry.
fn superseded_why(entry: &ChangeLogEntry, loc: &Localizer) -> Option<String> {
    if entry.event_type != "AssertionSuperseded" {
        return None;
    }
    let run = entry.run.as_ref()?;
    let exported = run.file_asserted_at.as_deref()?;
    // A file's export date often carries no time of day, so the time the timestamp holds is not shown.
    let day = exported.get(..10).unwrap_or(exported);
    Some(loc.import_superseded_why(&run.source_label, day))
}

/// The muted text beside a run row: *interrupted* for a run that can be resumed, else the records it
/// imported once it ended, else the changes the row folds.
fn run_count(run: &RunRef, changes: u32, loc: &Localizer) -> String {
    if run.resume.is_some() {
        return loc.import_run_interrupted();
    }
    run.records.map_or_else(
        || loc.import_run_changes(changes),
        |records| loc.import_run_records(records),
    )
}

/// The newest undoable entry of a record's change log (the `⌘Z` target), or `None` when nothing can
/// be undone. Change logs are newest-first, so this is the first entry with `can_undo` — an
/// already-retracted assertion is skipped, but a collapsed import run is not: it carries its run's
/// newest assertion's `can_undo`, so `⌘Z` retracts that assertion rather than jumping past the run
/// (issue #306).
#[must_use]
pub fn first_undoable(entries: &[HistoryEntryVm]) -> Option<&HistoryEntryVm> {
    entries.iter().find(|entry| entry.can_undo)
}

/// Builds the History-tab rows, folding each import run's entries into one row via the shared
/// [`vitni_app::group_runs`] — the same grouping the dashboard activity feed gets from the app. The
/// run row is stamped from its newest entry, so it stays undoable like any other entry.
#[must_use]
pub fn collapse_history(entries: &[ChangeLogEntry], loc: &Localizer) -> Vec<HistoryEntryVm> {
    vitni_app::group_runs(entries)
        .iter()
        .map(|entry| HistoryEntryVm::from_entry(entry, loc))
        .collect()
}

/// Shortens an RFC 3339 timestamp to `YYYY-MM-DD HH:MM` for display, or returns it unchanged when it
/// is not in the expected shape.
pub(crate) fn friendly_timestamp(rfc3339: &str) -> String {
    match (rfc3339.len() >= 16, rfc3339.get(..16)) {
        (true, Some(head)) => head.replacen('T', " ", 1),
        _ => rfc3339.to_owned(),
    }
}

/// One row in the dashboard's recent-activity feed (a workspace-wide change-log entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityVm {
    /// The localized timestamp.
    pub when: String,
    /// The localized summary of what changed.
    pub what: String,
    /// The localized operator line.
    pub who: String,
    /// The affected record, when it resolves to a navigable detail (any aggregate with a `human_id`).
    pub record: Option<RecordRef>,
    /// For an import-run row, the localized count shown muted beside it: the records the run
    /// imported once it has ended (e.g. `142 records`), else the changes the row folds.
    pub count: Option<String>,
    /// For an import-run row, the entries it folds, newest first; empty otherwise.
    pub children: Vec<ActivityVm>,
    /// For an interrupted bulk run's row, its *Resume* (ADR 0040 §5).
    pub resume: Option<ResumeVm>,
}

/// An interrupted import run's *Resume* (ADR 0040 §5): what it re-runs, and the button's accessible
/// name, which names the run's file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeVm {
    /// What *Resume* re-runs.
    pub run: RunResume,
    /// The button's accessible name.
    pub label: String,
}

impl ResumeVm {
    /// The *Resume* of a run that imported `source_label`, when `resume` says it can be resumed.
    pub(crate) fn of(resume: Option<&RunResume>, source_label: &str, loc: &Localizer) -> Option<Self> {
        resume.map(|run| Self {
            run: run.clone(),
            label: loc.import_run_resume_aria(source_label),
        })
    }
}

impl ActivityVm {
    /// Builds an activity row from an app [`ChangeLogEntry`], linking the affected record by name/id.
    #[must_use]
    pub(crate) fn from_entry(entry: &ChangeLogEntry, loc: &Localizer, names: &HashMap<String, String>) -> Self {
        let (count, children, resume) = match &entry.detail {
            Some(ActivityDetail::ImportRun { run, count, children }) => (
                Some(run_count(run, *count, loc)),
                children
                    .iter()
                    .map(|child| Self::from_entry(child, loc, names))
                    .collect(),
                ResumeVm::of(run.resume.as_ref(), &run.source_label, loc),
            ),
            Some(ActivityDetail::Fact { .. } | ActivityDetail::IdentityDecision { .. }) | None => {
                (None, Vec::new(), None)
            }
        };
        Self {
            when: friendly_timestamp(&entry.occurred_at),
            what: loc.change_summary(entry),
            who: loc.operator_line(entry),
            record: record_for(entry, names),
            count,
            children,
            resume,
        }
    }
}

/// The navigable record an entry affected, across every aggregate. People are labelled by display
/// name (from `names`); other aggregates fall back to their `human_id`. An import-run row (no kind,
/// no id) and a record without a resolved `human_id` are not navigable.
fn record_for(entry: &ChangeLogEntry, names: &HashMap<String, String>) -> Option<RecordRef> {
    let human_id = entry.aggregate_human_id.as_ref()?;
    let category = Category::from_aggregate_kind(&entry.aggregate_kind)?;
    Some(RecordRef {
        category,
        label: names.get(human_id).cloned().unwrap_or_else(|| human_id.clone()),
        human_id: human_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::{HistoryEntryVm, first_undoable};

    fn entry(assertion_id: &str, can_undo: bool) -> HistoryEntryVm {
        HistoryEntryVm {
            when: "2026-06-22 14:35".to_owned(),
            what: "Name asserted".to_owned(),
            who: "magne · High".to_owned(),
            why: None,
            assertion_id: assertion_id.to_owned(),
            can_undo,
            count: None,
            evidence: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn first_undoable_picks_the_newest_undoable_entry() {
        // Newest-first order: the first `can_undo` entry is the newest undoable one.
        let entries = vec![entry("a", false), entry("b", true), entry("c", true)];
        assert_eq!(first_undoable(&entries).map(|e| e.assertion_id.as_str()), Some("b"));
    }

    #[test]
    fn first_undoable_is_none_when_nothing_can_be_undone() {
        let entries = vec![entry("a", false), entry("b", false)];
        assert!(first_undoable(&entries).is_none());
        assert!(first_undoable(&[]).is_none());
    }
}
