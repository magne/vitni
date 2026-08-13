use super::{Category, ChangeLogEntry, HashMap, Localizer, OperatorKind, RecordRef};

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
}

impl HistoryEntryVm {
    /// Builds a history view-model from an app [`ChangeLogEntry`], localizing the summary + operator.
    #[must_use]
    pub fn from_entry(entry: &ChangeLogEntry, loc: &Localizer) -> Self {
        Self {
            when: friendly_timestamp(&entry.occurred_at),
            what: loc.change_summary(entry),
            who: loc.operator_line(entry),
            why: entry.rationale.clone(),
            assertion_id: entry.assertion_id.clone(),
            can_undo: entry.can_undo,
        }
    }
}

/// The newest undoable entry of a record's change log (the `⌘Z` target), or `None` when nothing can
/// be undone. Change logs are newest-first, so this is the first entry with `can_undo` — a collapsed
/// import run (`can_undo == false`) or an already-retracted assertion is skipped.
#[must_use]
pub fn first_undoable(entries: &[HistoryEntryVm]) -> Option<&HistoryEntryVm> {
    entries.iter().find(|entry| entry.can_undo)
}

/// Builds the History-tab rows, collapsing consecutive same-software-agent runs (e.g. an import) into
/// one `"N records imported"` entry — the same grouping as the dashboard activity feed. A collapsed
/// run is not individually undoable (it stands for many assertions), so it carries no undo control.
#[must_use]
pub fn collapse_history(entries: &[ChangeLogEntry], loc: &Localizer) -> Vec<HistoryEntryVm> {
    let mut rows = Vec::new();
    let mut index = 0;
    while index < entries.len() {
        let entry = &entries[index];
        let run = software_run_len(entries, index);
        if run >= 2 {
            rows.push(HistoryEntryVm {
                when: friendly_timestamp(&entry.occurred_at),
                what: loc.activity_import_batch(run),
                who: loc.operator_line(entry),
                why: None,
                assertion_id: String::new(),
                can_undo: false,
            });
            index += run;
        } else {
            rows.push(HistoryEntryVm::from_entry(entry, loc));
            index += 1;
        }
    }
    rows
}

/// The length of the run of consecutive software-agent events starting at `start` that share the
/// same operator; `1` (or `0` past the end) for a non-software or lone entry.
fn software_run_len(entries: &[ChangeLogEntry], start: usize) -> usize {
    let Some(first) = entries.get(start) else {
        return 0;
    };
    if first.operator_kind != OperatorKind::Software {
        return 1;
    }
    let mut end = start + 1;
    while entries.get(end).is_some_and(|next| {
        next.operator_kind == OperatorKind::Software && next.operator_display == first.operator_display
    }) {
        end += 1;
    }
    end - start
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
}

impl ActivityVm {
    /// Builds an activity row from an app [`ChangeLogEntry`], linking the affected record by name/id.
    #[must_use]
    pub(crate) fn from_entry(entry: &ChangeLogEntry, loc: &Localizer, names: &HashMap<String, String>) -> Self {
        Self {
            when: friendly_timestamp(&entry.occurred_at),
            what: loc.change_summary(entry),
            who: loc.operator_line(entry),
            record: record_for(entry, names),
        }
    }
}

/// The navigable record an entry affected, across every aggregate. People are labelled by display
/// name (from `names`); other aggregates fall back to their `human_id`. A synthetic collapsed-import
/// row (no kind, no id) and a record without a resolved `human_id` are not navigable.
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
