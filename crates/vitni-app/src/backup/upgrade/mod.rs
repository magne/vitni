//! Restore-time upgraders (ADR 0041 §3, §4): the ADR 0010 upcaster idea moved to the backup
//! boundary.
//!
//! An archive in an older format is brought to the current one by a chain of pure `vN → vN+1`
//! steps over `serde_json::Value`, the manifest and every event row alike, before anything is
//! inserted. The live store never holds or needs an old shape. [`chain_for`] also enforces the
//! compatibility window: before 1.0 the current format and the two before it.

mod pre_release;

use serde_json::Value;

use crate::backup::error::BackupError;
use crate::backup::format::FormatVersion;

/// How many formats a restore reads: the current one and the two before it (ADR 0041 §4).
const WINDOW: usize = 3;

/// One step from a format to the next, over the manifest and over each event row.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Upgrader {
    /// Upgrades `manifest.json`.
    pub(crate) manifest: fn(Value) -> Result<Value, String>,
    /// Upgrades one `events.jsonl` row.
    pub(crate) row: fn(Value) -> Result<Value, String>,
}

/// A format some release wrote, with the step that upgrades it to the next.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FormatRecord {
    /// The format.
    pub(crate) version: FormatVersion,
    /// The first app version that wrote it.
    pub(crate) first_app_version: &'static str,
    /// The step to the next format; `None` only for the current one.
    pub(crate) upgrade: Option<Upgrader>,
}

/// The format this version writes.
pub(crate) fn current() -> FormatVersion {
    current_in(pre_release::HISTORY)
}

/// The last format in `history`.
fn current_in(history: &[FormatRecord]) -> FormatVersion {
    let Some(last) = history.last() else {
        unreachable!("the format history names at least the current format");
    };
    last.version
}

/// The upgrade steps from one archive's format to the current one, in order.
#[derive(Debug)]
pub(crate) struct Chain {
    steps: Vec<Upgrader>,
}

impl Chain {
    /// Brings a manifest up to the current format.
    pub(crate) fn manifest(&self, mut manifest: Value) -> Result<Value, String> {
        for step in &self.steps {
            manifest = (step.manifest)(manifest)?;
        }
        Ok(manifest)
    }

    /// Brings one event row up to the current format.
    pub(crate) fn row(&self, mut row: Value) -> Result<Value, String> {
        for step in &self.steps {
            row = (step.row)(row)?;
        }
        Ok(row)
    }
}

/// The chain that upgrades an archive in format `found` to the current format.
///
/// # Errors
///
/// [`BackupError::UnknownFormat`] for a malformed version or one no release wrote,
/// [`BackupError::FormatTooNew`] for a format newer than the current one, and
/// [`BackupError::FormatTooOld`] for one outside the window.
pub(crate) fn chain_for(found: &str) -> Result<Chain, BackupError> {
    chain_in(pre_release::HISTORY, found)
}

/// [`chain_for`] over an explicit `history`, so the window rules are testable on any history.
fn chain_in(history: &[FormatRecord], found: &str) -> Result<Chain, BackupError> {
    let current = current_in(history);
    let version: FormatVersion = found
        .parse()
        .map_err(|_| BackupError::UnknownFormat(found.to_owned()))?;
    if version > current {
        return Err(BackupError::FormatTooNew {
            found: found.to_owned(),
            current: current.to_string(),
        });
    }
    let Some(index) = history.iter().position(|record| record.version == version) else {
        return Err(BackupError::UnknownFormat(found.to_owned()));
    };
    let behind = history.len() - 1 - index;
    if behind >= WINDOW {
        return Err(BackupError::FormatTooOld {
            found: found.to_owned(),
            oldest_supported: history[history.len() - WINDOW].version.to_string(),
            readable_before: history[index + WINDOW].first_app_version.to_owned(),
        });
    }
    let mut steps = Vec::with_capacity(behind);
    for record in &history[index..history.len() - 1] {
        let Some(step) = record.upgrade else {
            unreachable!("every format but the current one has an upgrader (checked by a test)");
        };
        steps.push(step);
    }
    Ok(Chain { steps })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{FormatRecord, Upgrader, WINDOW, chain_in, pre_release};
    use crate::backup::error::BackupError;
    use crate::backup::format::FormatVersion;

    fn tag(mut value: Value, label: &str) -> Result<Value, String> {
        let Some(steps) = value.get_mut("steps").and_then(Value::as_array_mut) else {
            return Err("no steps".to_owned());
        };
        steps.push(label.into());
        Ok(value)
    }

    fn step_1_to_2(value: Value) -> Result<Value, String> {
        tag(value, "1→2")
    }

    fn step_2_to_3(value: Value) -> Result<Value, String> {
        tag(value, "2→3")
    }

    fn step_3_to_4(value: Value) -> Result<Value, String> {
        tag(value, "3→4")
    }

    fn record(minor: u32, app: &'static str, step: Option<fn(Value) -> Result<Value, String>>) -> FormatRecord {
        FormatRecord {
            version: FormatVersion::pre_release(minor),
            first_app_version: app,
            upgrade: step.map(|row| Upgrader { manifest: row, row }),
        }
    }

    /// Four formats, 0.1 to 0.4, first written by apps 0.1.0 to 0.4.0.
    fn history() -> Vec<FormatRecord> {
        vec![
            record(1, "0.1.0", Some(step_1_to_2)),
            record(2, "0.2.0", Some(step_2_to_3)),
            record(3, "0.3.0", Some(step_3_to_4)),
            record(4, "0.4.0", None),
        ]
    }

    fn steps_after(found: &str) -> Value {
        let chain = chain_in(&history(), found).expect("in the window");
        let row = chain.row(json!({ "steps": [] })).expect("upgrades");
        let manifest = chain.manifest(json!({ "steps": [] })).expect("upgrades");
        assert_eq!(row, manifest, "manifest and rows take the same steps");
        row["steps"].clone()
    }

    #[test]
    fn the_current_format_needs_no_step() {
        assert_eq!(steps_after("0.4"), json!([]));
    }

    #[test]
    fn the_two_previous_formats_are_upgraded_in_order() {
        assert_eq!(steps_after("0.3"), json!(["3→4"]));
        assert_eq!(steps_after("0.2"), json!(["2→3", "3→4"]));
    }

    #[test]
    fn a_format_three_behind_is_too_old_and_names_where_to_read_it() {
        let error = chain_in(&history(), "0.1").expect_err("outside the window");
        let BackupError::FormatTooOld {
            found,
            oldest_supported,
            readable_before,
        } = error
        else {
            panic!("expected FormatTooOld, got {error:?}");
        };
        assert_eq!(found, "0.1");
        assert_eq!(oldest_supported, "0.2");
        assert_eq!(readable_before, "0.4.0", "0.4.0 introduced the format that dropped 0.1");
    }

    #[test]
    fn a_newer_format_is_too_new() {
        assert!(matches!(
            chain_in(&history(), "0.5"),
            Err(BackupError::FormatTooNew { found, current }) if found == "0.5" && current == "0.4"
        ));
        assert!(matches!(
            chain_in(&history(), "1"),
            Err(BackupError::FormatTooNew { .. })
        ));
    }

    #[test]
    fn malformed_and_never_written_formats_are_unknown() {
        for found in ["0.0", "banana", "", "0.04"] {
            assert!(
                matches!(chain_in(&history(), found), Err(BackupError::UnknownFormat(text)) if text == found),
                "{found:?}"
            );
        }
    }

    #[test]
    fn a_failing_step_reports_its_reason() {
        let chain = chain_in(&history(), "0.2").expect("in the window");
        assert_eq!(chain.row(json!({})), Err("no steps".to_owned()));
    }

    #[test]
    fn the_real_history_is_ordered_and_every_old_format_has_an_upgrader() {
        let history = pre_release::HISTORY;
        assert!(!history.is_empty());
        for pair in history.windows(2) {
            assert!(
                pair[0].version < pair[1].version,
                "{:?} before {:?}",
                pair[0].version,
                pair[1].version
            );
        }
        let (current, older) = history.split_last().expect("non-empty");
        assert!(current.upgrade.is_none(), "the current format upgrades to nothing");
        for record in older.iter().rev().take(WINDOW - 1) {
            assert!(record.upgrade.is_some(), "{} needs an upgrader", record.version);
        }
    }
}
