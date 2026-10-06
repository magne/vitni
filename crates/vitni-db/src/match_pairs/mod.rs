//! The `match_pairs` projection (ADR 0048): every pair of matchable records of one kind the engine
//! scores at least possibly the same, with its band and score, so the duplicate check and the review
//! queue read their pairs instead of scoring every candidate pair on each show.
//!
//! The scores need the installed packs and settings, which the app layer loads, so this crate stores
//! them but never computes them. It keeps three tables, the twins of the `match_keys` index's:
//!
//! - `match_pairs (kind, a, b, band, score)` — the pairs, `a` the lower aggregate id, written by the
//!   app layer. A pair is the engine's raw judgement of two records: no identity decision is applied
//!   when it is written. A read leaves out each pair with a merged member and each pair of two clusters
//!   held distinct, from the identity index (ADR 0039 §4), so a decision or its undo needs no rescore;
//! - `match_pairs_dirty (aggregate_type, aggregate_id, generation)` — every matchable aggregate a
//!   commit touched since the app layer last refreshed its pairs, fed by the same query as
//!   `match_dirty` and cleared the same way, but on its own: a lookup refreshing the keys leaves it;
//! - `match_pairs_state (fingerprint)` — one row naming the keys, engine and settings the pairs were
//!   scored under. No row means every pair must be scored again: a new workspace, or one whose
//!   projections were rebuilt.

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

use vitni_core::matching::{MatchBand, MatchableKind};

use crate::store::DbError;

/// The pairs table.
const MATCH_PAIRS_TABLE: &str = "match_pairs";

/// The table of records touched since their pairs were refreshed.
pub(crate) const MATCH_PAIRS_DIRTY_TABLE: &str = "match_pairs_dirty";

/// The single-row table naming what the pairs were scored under.
const MATCH_PAIRS_STATE_TABLE: &str = "match_pairs_state";

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

/// The `FROM … WHERE` of a read of the undecided pairs from band `$min` up, with `kind_filter` (a
/// condition on `p.kind`, or empty) appended. A pair is decided when either record is a merged member,
/// or when its two records are the roots of clusters a distinction holds apart, recorded on any member
/// of either.
fn undecided(min: &str, kind_filter: &str) -> String {
    format!(
        "FROM {MATCH_PAIRS_TABLE} p \
         WHERE p.band >= {min}{kind_filter} \
           AND NOT EXISTS (SELECT 1 FROM identity_links l WHERE l.kind = p.kind AND l.member = p.a) \
           AND NOT EXISTS (SELECT 1 FROM identity_links l WHERE l.kind = p.kind AND l.member = p.b) \
           AND NOT EXISTS (SELECT 1 FROM held r WHERE r.kind = p.kind AND r.x = p.a AND r.y = p.b) \
           AND NOT EXISTS (SELECT 1 FROM held r WHERE r.kind = p.kind AND r.x = p.b AND r.y = p.a)"
    )
}

/// The common table `held`: every distinction as the pair of its records' cluster roots `(x, y)`.
const HELD: &str = "WITH held AS MATERIALIZED ( \
     SELECT d.kind AS kind, COALESCE(lr.root, d.record) AS x, COALESCE(lo.root, d.other) AS y \
     FROM identity_distinctions d \
     LEFT JOIN identity_links lr ON lr.kind = d.kind AND lr.member = d.record \
     LEFT JOIN identity_links lo ON lo.kind = d.kind AND lo.member = d.other) ";

/// The query listing the undecided pairs from band `$1` up of the `kinds` bound from `$2` on, strongest
/// first, at most `limit`.
fn list_query(placeholder: fn(usize) -> String, kinds: usize, limit: Option<usize>) -> String {
    let mut listed = Vec::with_capacity(kinds);
    for kind in 0..kinds {
        listed.push(placeholder(kind + 2));
    }
    let kind_filter = format!(" AND p.kind IN ({})", listed.join(", "));
    let limit = limit
        .map(|limit| format!(" LIMIT {}", i64::try_from(limit).unwrap_or(i64::MAX)))
        .unwrap_or_default();
    format!(
        "{HELD}SELECT p.kind, p.a, p.b, p.band, p.score {} \
         ORDER BY p.band DESC, p.score DESC, p.kind, p.a, p.b{limit}",
        undecided(&placeholder(1), &kind_filter)
    )
}

/// The query counting the undecided pairs from band `$1` up, per kind.
fn count_query(placeholder: fn(usize) -> String) -> String {
    format!(
        "{HELD}SELECT p.kind, COUNT(*) AS n {} GROUP BY p.kind ORDER BY p.kind",
        undecided(&placeholder(1), "")
    )
}

#[cfg(test)]
mod tests {
    use vitni_core::matching::MatchBand;

    use super::{band_of, band_rank, list_query};

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
    fn a_list_names_its_kinds_and_limits_only_when_asked() {
        let every = list_query(|i| format!("${i}"), 2, None);
        assert!(
            every.contains("p.kind IN ($2, $3)") && !every.contains("LIMIT"),
            "{every}"
        );
        let one = list_query(|i| format!("${i}"), 1, Some(5));
        assert!(one.contains("p.kind IN ($2)") && one.ends_with("LIMIT 5"), "{one}");
        let unbounded = list_query(|i| format!("${i}"), 1, Some(usize::MAX));
        assert!(unbounded.ends_with(&format!("LIMIT {}", i64::MAX)), "{unbounded}");
    }
}
