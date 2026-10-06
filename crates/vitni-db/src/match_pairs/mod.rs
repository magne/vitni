//! The `match_pairs` projection (ADR 0048): every pair of matchable records of one kind the engine
//! scores at least possibly the same, with its band and score, so the duplicate check and the review
//! queue read their pairs instead of scoring every candidate pair on each show.
//!
//! The scores need the installed packs and settings, which the app layer loads, so this crate stores
//! them but never computes them. It keeps four tables, three of them the twins of the `match_keys` index's:
//!
//! - `match_pairs (kind, a, b, band, score)` — the pairs, `a` the lower aggregate id, written by the
//!   app layer. A pair is the engine's raw judgement of two records: no identity decision is applied
//!   when it is written. A read leaves out each pair with a merged member and each pair of two clusters
//!   held distinct, from the identity index (ADR 0039 §4), so a decision or its undo needs no rescore;
//! - `match_pairs_dirty (aggregate_type, aggregate_id, generation)` — every matchable aggregate a
//!   commit touched since the app layer last refreshed its pairs, fed by the same query as
//!   `match_dirty` and cleared the same way, but on its own: a lookup refreshing the keys leaves it;
//! - `match_pair_counts (kind, band, n)` — how many pairs each kind holds per band, kept by every write,
//!   so counting reads a few rows and only the decided pairs, found from the identity index, are
//!   subtracted;
//! - `match_pairs_state (fingerprint)` — one row naming the keys, engine and settings the pairs were
//!   scored under. No row means every pair must be scored again: a new workspace, or one whose
//!   projections were rebuilt.

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

use std::collections::BTreeMap;

use vitni_core::matching::{MatchBand, MatchableKind};

use crate::store::DbError;

/// The pairs table.
const MATCH_PAIRS_TABLE: &str = "match_pairs";

/// The table of records touched since their pairs were refreshed.
pub(crate) const MATCH_PAIRS_DIRTY_TABLE: &str = "match_pairs_dirty";

/// The single-row table naming what the pairs were scored under.
const MATCH_PAIRS_STATE_TABLE: &str = "match_pairs_state";

/// How many pairs each kind holds per band, kept with every write so a count reads a few rows.
const MATCH_PAIR_COUNTS_TABLE: &str = "match_pair_counts";

/// Rows per multi-row `INSERT`: five values each, well under either engine's bound-parameter limit.
const INSERT_CHUNK: usize = 500;

/// Two records of one kind the engine scores at least possibly the same.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchPair {
    /// The kind of both records.
    pub kind: MatchableKind,
    /// The lower aggregate id.
    pub a: String,
    /// The higher aggregate id.
    pub b: String,
    /// The band the engine put the pair in.
    pub band: MatchBand,
    /// The engine's score, in `0..=1`.
    pub score: f64,
}

/// Which stored pairs a refresh replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairScope {
    /// Every pair of the kind.
    Kind,
    /// Every pair naming one of these records (aggregate ids).
    Records(Vec<String>),
}

/// One kind's part of a refresh: the pairs `scope` covers, replaced by `pairs`.
#[derive(Debug, Clone, PartialEq)]
pub struct PairRefresh {
    /// The kind refreshed.
    pub kind: MatchableKind,
    /// The stored pairs replaced.
    pub scope: PairScope,
    /// The pairs written in their place.
    pub pairs: Vec<MatchPair>,
}

/// The band as stored: ordered, so a read can filter and sort by it.
const fn band_rank(band: MatchBand) -> i64 {
    match band {
        MatchBand::Unlikely => 0,
        MatchBand::Possible => 1,
        MatchBand::Probable => 2,
        MatchBand::Deterministic => 3,
    }
}

/// The band a stored rank names.
fn band_of(rank: i64) -> Result<MatchBand, DbError> {
    match rank {
        0 => Ok(MatchBand::Unlikely),
        1 => Ok(MatchBand::Possible),
        2 => Ok(MatchBand::Probable),
        3 => Ok(MatchBand::Deterministic),
        other => Err(DbError::Backend(format!("unknown stored match band {other}"))),
    }
}

/// The kind named by a stored aggregate type.
fn kind_of(aggregate_type: &str) -> Result<MatchableKind, DbError> {
    MatchableKind::parse(aggregate_type)
        .ok_or_else(|| DbError::Backend(format!("unknown matchable kind `{aggregate_type}`")))
}

/// The common table `held`: every distinction as the pair of its records' cluster roots `(x, y)`.
const HELD: &str = "WITH held AS MATERIALIZED ( \
     SELECT d.kind AS kind, COALESCE(lr.root, d.record) AS x, COALESCE(lo.root, d.other) AS y \
     FROM identity_distinctions d \
     LEFT JOIN identity_links lr ON lr.kind = d.kind AND lr.member = d.record \
     LEFT JOIN identity_links lo ON lo.kind = d.kind AND lo.member = d.other)";

/// The query listing the undecided pairs of the kind `$2` from band `$1` up, strongest first, at most
/// `limit`. It walks the `(kind, band, score)` index in order and stops at the limit, testing each pair
/// against the identity index as it goes: a pair is decided when either record is a merged member, or
/// when its two records are the roots of clusters a distinction holds apart, recorded on any member of
/// either.
fn list_query(placeholder: fn(usize) -> String, limit: Option<usize>) -> String {
    let (min, kind) = (placeholder(1), placeholder(2));
    let limit = limit
        .map(|limit| format!(" LIMIT {}", i64::try_from(limit).unwrap_or(i64::MAX)))
        .unwrap_or_default();
    format!(
        "{HELD} SELECT p.kind, p.a, p.b, p.band, p.score FROM {MATCH_PAIRS_TABLE} p \
         WHERE p.kind = {kind} AND p.band >= {min} \
           AND NOT EXISTS (SELECT 1 FROM identity_links l WHERE l.kind = p.kind AND l.member = p.a) \
           AND NOT EXISTS (SELECT 1 FROM identity_links l WHERE l.kind = p.kind AND l.member = p.b) \
           AND NOT EXISTS (SELECT 1 FROM held r WHERE r.kind = p.kind AND r.x = p.a AND r.y = p.b) \
           AND NOT EXISTS (SELECT 1 FROM held r WHERE r.kind = p.kind AND r.x = p.b AND r.y = p.a) \
         ORDER BY p.band DESC, p.score DESC, p.a, p.b{limit}"
    )
}

/// The query counting, per kind, the stored pairs from band `$1` up that a decision leaves out. It is
/// driven from the identity index, which holds only the records a user decided about, never from the
/// pairs: `CROSS JOIN` keeps that order (SQLite takes it as written), and `band + 0` keeps the lookup
/// on the record's own index rather than the band's.
fn decided_query(placeholder: fn(usize) -> String) -> String {
    let min = placeholder(1);
    let pairs_of = |from: &str, on: &str| {
        format!(
            "SELECT p.kind, p.a, p.b FROM {from} CROSS JOIN {MATCH_PAIRS_TABLE} p WHERE {on} AND p.band + 0 >= {min}"
        )
    };
    let branches = [
        pairs_of("identity_links l", "p.kind = l.kind AND p.a = l.member"),
        pairs_of("identity_links l", "p.kind = l.kind AND p.b = l.member"),
        pairs_of("held h", "p.kind = h.kind AND p.a = h.x AND p.b = h.y"),
        pairs_of("held h", "p.kind = h.kind AND p.a = h.y AND p.b = h.x"),
    ];
    format!(
        "{HELD}, decided AS ({}) SELECT kind, COUNT(*) AS n FROM decided GROUP BY kind",
        branches.join(" UNION ")
    )
}

/// The query of the stored pairs from band `$1` up, per kind, from the counts kept beside them.
fn stored_count_query(placeholder: fn(usize) -> String) -> String {
    format!(
        "SELECT kind, CAST(SUM(n) AS BIGINT) AS n FROM {MATCH_PAIR_COUNTS_TABLE} WHERE band >= {} GROUP BY kind",
        placeholder(1)
    )
}

/// How many of `pairs` each kind and band holds: what writing them adds to the kept counts.
fn tally(pairs: &[MatchPair]) -> BTreeMap<(MatchableKind, i64), i64> {
    let mut counts = BTreeMap::new();
    for pair in pairs {
        *counts.entry((pair.kind, band_rank(pair.band))).or_insert(0) += 1;
    }
    counts
}

/// The stored counts less the decided ones, for the kinds left with any, in kind name order.
fn undecided_counts(
    stored: Vec<(MatchableKind, i64)>,
    decided: &[(MatchableKind, i64)],
) -> Result<Vec<(MatchableKind, usize)>, DbError> {
    let mut counts = BTreeMap::new();
    for (kind, n) in stored {
        let left = n - decided
            .iter()
            .filter(|(each, _)| *each == kind)
            .map(|(_, n)| n)
            .sum::<i64>();
        let left = usize::try_from(left).map_err(|e| DbError::Backend(format!("a negative pair count: {e}")))?;
        if left > 0 {
            counts.insert(kind.as_str(), (kind, left));
        }
    }
    Ok(counts.into_values().collect())
}

/// `listed`, the pairs of several kinds each listed strongest first, merged strongest first whatever
/// their kind, at most `limit` of them.
fn strongest(mut listed: Vec<MatchPair>, limit: Option<usize>) -> Vec<MatchPair> {
    listed.sort_by(|x, y| {
        band_rank(y.band)
            .cmp(&band_rank(x.band))
            .then_with(|| y.score.total_cmp(&x.score))
            .then_with(|| (x.kind.as_str(), &x.a, &x.b).cmp(&(y.kind.as_str(), &y.a, &y.b)))
    });
    if let Some(limit) = limit {
        listed.truncate(limit);
    }
    listed
}

#[cfg(test)]
mod tests {

    use vitni_core::matching::{MatchBand, MatchableKind};

    use super::{band_of, band_rank, list_query, undecided_counts};

    #[test]
    fn every_band_round_trips_through_its_rank_in_order() {
        let bands = [
            MatchBand::Unlikely,
            MatchBand::Possible,
            MatchBand::Probable,
            MatchBand::Deterministic,
        ];
        for pair in bands.windows(2) {
            assert!(band_rank(pair[0]) < band_rank(pair[1]));
        }
        for band in bands {
            assert_eq!(band_of(band_rank(band)).unwrap(), band);
        }
        assert!(band_of(4).is_err());
    }

    #[test]
    fn a_list_limits_only_when_asked() {
        let every = list_query(|i| format!("${i}"), None);
        assert!(every.contains("p.kind = $2") && !every.contains("LIMIT"), "{every}");
        let some = list_query(|i| format!("${i}"), Some(5));
        assert!(some.ends_with("LIMIT 5"), "{some}");
        let unbounded = list_query(|i| format!("${i}"), Some(usize::MAX));
        assert!(unbounded.ends_with(&format!("LIMIT {}", i64::MAX)), "{unbounded}");
    }

    #[test]
    fn the_decided_pairs_are_subtracted_and_an_emptied_kind_is_left_out() {
        let stored = vec![(MatchableKind::Place, 2), (MatchableKind::Person, 5)];
        let decided = [(MatchableKind::Place, 2), (MatchableKind::Person, 1)];
        assert_eq!(
            undecided_counts(stored, &decided).unwrap(),
            [(MatchableKind::Person, 4)]
        );
        assert!(undecided_counts(vec![(MatchableKind::Person, 1)], &[(MatchableKind::Person, 2)]).is_err());
    }
}
