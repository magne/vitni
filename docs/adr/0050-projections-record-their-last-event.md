# 50. A projection records the last event it holds

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

`cqrs-es` commits a command's events, then dispatches them to the aggregate's queries: its projection
(ADR 0009), the side indexes read from that projection (ADR 0024, 0026, 0039, 0047), the record origins
index (ADR 0037) and the match dirty marks (ADR 0038). A process killed between the commit and the
projection write leaves the view without the newest events until `vitni rebuild` (#536).

The record origins index already closes the same window with a write journal: a command journals its
aggregate before it runs and withdraws the entry after every query has run, and the next open replays
each aggregate still journalled. That journal therefore already names every aggregate whose projection
may be behind. A projection could not use it: `GenericQuery` folds every event it is given onto the
stored view, so a replay of the aggregate's log would apply the events it already holds a second time.
The view's integer `version` (ADR 0009 §1) counted the batches written, not the events, so it could not
tell which events those were.

## Decision

1. **A view row holds the sequence of the last event folded into it.** The view table's columns are
   `(view_id, sequence, payload)` plus the generated `human_id` (ADR 0032). `sequence` replaces
   `version`. A table without the column is dropped and rebuilt on open by the ADR 0032 path.

2. **vitni writes its projections with its own query, not `GenericQuery`.** It folds only the events
   after the row's sequence, and fails, writing nothing, when the first new event is not the next one
   (a view missing events the batch does not carry). Its write is conditional on the sequence it read;
   a write another writer beat reads the row again and folds what is still missing. Replaying a log the
   view already holds writes nothing.

3. **The write journal covers every query of a commit.** The next open replays each journalled
   aggregate's whole log through exactly the queries a command runs: the projection folds what it lacks,
   the side indexes are re-derived from it, record origins rows are keyed by event, and the match dirty
   marks are set again.

4. **A commit whose projection failed stays journalled.** The command still reports the failure, and the
   next open repairs the view, where before only a rebuild did.

ADR 0009 §1 (an integer version, the `GenericQuery` + `ViewRepository` shape) and ADR 0010 §5 (rebuild
through the same `GenericQuery`) are not edited. This ADR supersedes them only as far as the column and
the writing query go. A rebuild still replays through the query the live store uses.

## Rationale

- **Repair in place, not rebuild.** Deleting the row and replaying the whole log would also repair it,
  but a second process committing to the same aggregate during that replay could apply its events onto
  the rebuilt row a second time. A row that knows its last event cannot be folded twice, whoever writes.
- **The sequence, not a count.** Sequences are contiguous per aggregate, so the last one says exactly
  which events a view holds. A rebuilt view's old `version` already equalled its last sequence, so the
  backup projection digest (ADR 0041 §5) is unchanged.

## Consequences

- An open replays only journalled aggregates; a workspace with none pays nothing.
- Opening a workspace created before this ADR rebuilds every projection once.
- `sqlite-es`/`postgres-es` remain the event stores; their view repositories are no longer used.

## References

- ADR 0009 — read-model and projection schema.
- ADR 0010 — projection rebuild.
- ADR 0032 — projection schema evolution by replay.
- ADR 0037 §4 — the record origins index and its write journal.
- #536.
