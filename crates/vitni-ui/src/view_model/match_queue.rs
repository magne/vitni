//! The Matches tool's view-models (ADR 0039 §3): the possible-matches queue, its run filter's choices,
//! and one queued pair, which the Dashboard's *Possible matches* card shows too.

use super::dashboard::record_ref;
use super::{HashMap, Localizer, RecordRef};
use vitni_app::{AggRef, DecidableKind, ImportRunId, ImportRunSummary, MatchEvidence, MatchQueue};

/// One undecided pair: both records, navigable, the kind, and the engine's view of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedMatchVm {
    /// The kind of both records.
    pub kind: DecidableKind,
    /// The already-localized kind label.
    pub kind_label: String,
    /// The first record.
    pub a: RecordRef,
    /// The second record.
    pub b: RecordRef,
    /// The engine's score as a whole percentage — a probability, never an asserted confidence.
    pub percent: u8,
    /// The already-localized band the engine put the pair in.
    pub band: String,
    /// The already-localized reasons behind the score, the strongest first.
    pub reasons: Vec<String>,
}

impl QueuedMatchVm {
    /// Builds the pair from the engine's `evidence`, labelling a person by its name in `names` and
    /// any other record by its id (the record link shows the live name).
    #[must_use]
    pub fn build(
        kind: DecidableKind,
        (a, b): (&AggRef, &AggRef),
        evidence: &MatchEvidence,
        names: &HashMap<String, String>,
        loc: &Localizer,
    ) -> Self {
        Self {
            kind,
            kind_label: loc.match_kind(kind),
            a: record_ref(kind.matchable(), a, names),
            b: record_ref(kind.matchable(), b, names),
            percent: evidence.percent(),
            band: loc.match_band(evidence.band),
            reasons: loc.match_reasons(evidence),
        }
    }
}

/// One choice of the run filter: an import run, named by its source and the day it started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptionVm {
    /// The run.
    pub id: ImportRunId,
    /// The already-localized label.
    pub label: String,
}

/// How many of the strongest undecided pairs the Matches tool lists; the rest are counted.
pub const LISTED_MATCHES: usize = 100;

/// The Matches tool's table: the queued pairs under the current filter, and the runs it can be
/// narrowed to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchQueueVm {
    /// How many undecided pairs the filter admits, listed or not.
    pub total: usize,
    /// The strongest undecided pairs, at most [`LISTED_MATCHES`], the most similar first.
    pub pairs: Vec<QueuedMatchVm>,
    /// Every import run, oldest first.
    pub runs: Vec<RunOptionVm>,
}

impl MatchQueueVm {
    /// Builds the table from the app's queue and import runs.
    #[must_use]
    pub fn build(queue: &MatchQueue, runs: &[ImportRunSummary], loc: &Localizer) -> Self {
        let names = HashMap::new();
        let mut pairs = Vec::with_capacity(queue.pairs.len());
        for queued in &queue.pairs {
            pairs.push(QueuedMatchVm::build(
                queued.kind,
                (&queued.a, &queued.b),
                &queued.assessment.evidence(),
                &names,
                loc,
            ));
        }
        let mut options = Vec::with_capacity(runs.len());
        for run in runs {
            options.push(RunOptionVm {
                id: run.id,
                label: loc.match_run_option(&run.source_label, &run.started_at.into_inner().date().to_string()),
            });
        }
        Self {
            total: queue.total,
            pairs,
            runs: options,
        }
    }

    /// How many undecided pairs the filter admits beyond those listed.
    #[must_use]
    pub fn unlisted(&self) -> usize {
        self.total.saturating_sub(self.pairs.len())
    }
}
