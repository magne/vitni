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
//!   Each touch bumps `generation`, and the app layer clears a row only at the generation it read, so
//!   a commit landing while a record is being rekeyed marks it dirty again rather than being lost;
//! - `match_keys_state (fingerprint)` — one row naming the keying rules and packs the index was built
//!   under. No row means the index must be built from scratch: a new workspace, or one whose
//!   projections were rebuilt ([`sqlite::clear_state`]).

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

use vitni_core::matching::{MatchableKind, Probe};

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

/// The first string after every string starting with `prefix`, for a range scan an index serves.
fn prefix_end(prefix: &str) -> String {
    let mut end: Vec<char> = prefix.chars().collect();
    while let Some(last) = end.pop() {
        if let Some(next) = char::from_u32(u32::from(last) + 1) {
            end.push(next);
            return end.into_iter().collect();
        }
    }
    String::from(char::MAX)
}

/// The `WHERE` condition over `key` a probe meets, with its bound values in order, each placeholder
/// spelled by `placeholder` from its 1-based position after `offset`; `None` when the probe meets
/// nothing.
fn probe_condition(probe: &Probe, offset: usize, placeholder: fn(usize) -> String) -> Option<(String, Vec<String>)> {
    let mut values: Vec<String> = Vec::new();
    let mut clauses = Vec::new();
    let bind = |value: String, values: &mut Vec<String>| {
        values.push(value);
        placeholder(offset + values.len())
    };
    if !probe.exact.is_empty() {
        let mut list = Vec::with_capacity(probe.exact.len());
        for key in &probe.exact {
            list.push(bind(key.clone(), &mut values));
        }
        clauses.push(format!("key IN ({})", list.join(", ")));
    }
    for prefix in &probe.prefixes {
        let lo = bind(prefix.clone(), &mut values);
        let hi = bind(prefix_end(prefix), &mut values);
        clauses.push(format!("(key >= {lo} AND key < {hi})"));
    }
    (!clauses.is_empty()).then(|| (format!("({})", clauses.join(" OR ")), values))
}

/// The kind named by a stored aggregate type.
fn kind_of(aggregate_type: &str) -> Result<MatchableKind, crate::store::DbError> {
    MatchableKind::parse(aggregate_type)
        .ok_or_else(|| crate::store::DbError::Backend(format!("unknown matchable kind `{aggregate_type}`")))
}

#[cfg(test)]
mod tests {
    use vitni_core::matching::Probe;

    use super::{prefix_end, probe_condition};

    #[test]
    fn a_prefix_range_ends_just_past_every_string_it_starts() {
        assert_eq!(prefix_end("t:ole@"), "t:oleA");
        assert_eq!(prefix_end("src:1#"), "src:1$");
        assert!("t:ole@185".as_bytes() < prefix_end("t:ole@").as_bytes());
        assert!("t:olea@185".as_bytes() >= prefix_end("t:ole@").as_bytes());
    }

    #[test]
    fn a_probe_becomes_one_condition_with_its_values_in_order() {
        let probe = Probe {
            exact: vec!["a".to_owned(), "b".to_owned()],
            prefixes: vec!["c@".to_owned()],
        };
        let (sql, values) = probe_condition(&probe, 1, |i| format!("${i}")).unwrap();
        assert_eq!(sql, "(key IN ($2, $3) OR (key >= $4 AND key < $5))");
        assert_eq!(values, ["a", "b", "c@", "cA"]);
        assert_eq!(probe_condition(&Probe::default(), 0, |_| "?".to_owned()), None);
    }
}
