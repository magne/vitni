//! The `match_keys` blocking index (ADR 0038 §7, justified under ADR 0009 §4): the loose keys each
//! matchable record is indexed under, so candidate pairs come from shared keys rather than from every
//! pair.
//!
//! The keys depend on the installed name-culture packs, which the app layer loads, so this crate
//! stores them but never computes them. It keeps three tables:
//!
//! - `match_keys (kind, key, aggregate_id)` — the index itself, written by the app layer;
//! - `match_dirty (aggregate_type, aggregate_id, generation)` — every matchable aggregate a commit
//!   touched since the app layer last keyed it, fed by [`sqlite::MatchDirtyQuery`] on every commit.
//!   Each touch bumps `generation`, and the app layer clears a row only at the generation it read. A
//!   row found at another generation, or already cleared by another refresh, is marked dirty again, so
//!   neither a commit landing mid-rekey nor a slower refresh writing older keys is left unnoticed;
//! - `match_keys_state (fingerprint)` — one row naming the keying rules and packs the index was built
//!   under. A rebuild that finds its own fingerprint already there lost a race and writes nothing. No row means the index must be built from scratch: a new workspace, or one whose
//!   projections were rebuilt ([`sqlite::clear_state`]).

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

use vitni_core::matching::{MatchableKind, Probe, prefix_end};

/// The index table.
const MATCH_KEYS_TABLE: &str = "match_keys";

/// The table of records touched since they were keyed.
const MATCH_DIRTY_TABLE: &str = "match_dirty";

/// The single-row table naming what the index was built under.
const MATCH_KEYS_STATE_TABLE: &str = "match_keys_state";

/// Rows per multi-row `INSERT`, well under either engine's bound-parameter limit.
const INSERT_CHUNK: usize = 500;

/// A record a commit touched since the app layer last keyed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyRecord {
    /// The record's kind.
    pub kind: MatchableKind,
    /// The record's aggregate id.
    pub aggregate_id: String,
    /// How many times it was touched; the row is cleared only at the generation read.
    pub generation: i64,
}

/// A record's keys, to be written in place of those it had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyedRecord {
    /// The record's kind.
    pub kind: MatchableKind,
    /// The record's aggregate id.
    pub aggregate_id: String,
    /// Its keys; empty removes the record from the index.
    pub keys: Vec<String>,
}

/// The query of the `aggregate_id` of every key of a kind `probe` meets, with the values to bind after
/// the kind, which is the first parameter. Each exact-key list and each prefix range is its own
/// `SELECT`, joined by `UNION ALL`: SQLite plans an `OR` of them as a scan of every key of the kind,
/// but each branch alone as a search of the `(kind, key)` primary key. A record meeting several
/// branches appears once per branch; `None` when the probe meets nothing.
fn probe_query(probe: &Probe, placeholder: fn(usize) -> String) -> Option<(String, Vec<String>)> {
    let mut values: Vec<String> = Vec::new();
    let bind = |value: String, values: &mut Vec<String>| {
        values.push(value);
        placeholder(1 + values.len())
    };
    let kind = placeholder(1);
    let mut branches = Vec::new();
    if !probe.exact.is_empty() {
        let mut list = Vec::with_capacity(probe.exact.len());
        for key in &probe.exact {
            list.push(bind(key.clone(), &mut values));
        }
        branches.push(format!("key IN ({})", list.join(", ")));
    }
    for prefix in &probe.prefixes {
        let lo = bind(prefix.clone(), &mut values);
        let hi = bind(prefix_end(prefix), &mut values);
        branches.push(format!("key >= {lo} AND key < {hi}"));
    }
    let mut selects = Vec::with_capacity(branches.len());
    for branch in branches {
        selects.push(format!(
            "SELECT aggregate_id FROM {MATCH_KEYS_TABLE} WHERE kind = {kind} AND {branch}"
        ));
    }
    (!selects.is_empty()).then(|| (selects.join(" UNION ALL "), values))
}

/// The kind named by a stored aggregate type.
fn kind_of(aggregate_type: &str) -> Result<MatchableKind, crate::store::DbError> {
    MatchableKind::parse(aggregate_type)
        .ok_or_else(|| crate::store::DbError::Backend(format!("unknown matchable kind `{aggregate_type}`")))
}

#[cfg(test)]
mod tests {
    use vitni_core::matching::Probe;

    use super::probe_query;

    #[test]
    fn a_probe_becomes_one_indexed_select_per_branch_with_its_values_in_order() {
        let probe = Probe {
            exact: vec!["a".to_owned(), "b".to_owned()],
            prefixes: vec!["c@".to_owned()],
        };
        let (sql, values) = probe_query(&probe, |i| format!("${i}")).unwrap();
        assert_eq!(
            sql,
            "SELECT aggregate_id FROM match_keys WHERE kind = $1 AND key IN ($2, $3) \
             UNION ALL SELECT aggregate_id FROM match_keys WHERE kind = $1 AND key >= $4 AND key < $5"
        );
        assert_eq!(values, ["a", "b", "c@", "cA"]);
        assert_eq!(probe_query(&Probe::default(), |i| format!("?{i}")), None);
    }
}
