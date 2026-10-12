# 51. An unreadable event is reported, not fatal

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

Since #486 a replay fails when an event does not decode: `cqrs-es`'s `QueryReplay` hands each such
event to an error handler, and vitni turns those into a failed replay. `open` replays in two places
that then fail it as a whole: the rebuild of a view table predating its `human_id` (ADR 0032) or
`sequence` (ADR 0050) column, and the record origins fill (ADR 0037 §4). The fill runs in one
transaction that rolls back and stays unmarked, so every later open fails the same way. One corrupt
payload, or one a newer version of vitni wrote, locks the workspace: no export, no backup, no restore
over it (#535).

The error named no event either. `QueryReplay` decodes inside its stream, and the error it hands over
is the bare decoder message (``person_view: missing field `assertion_id` ``), with no aggregate id or
sequence.

## Decision

1. **vitni reads the log itself and names the event.** A replay lists an aggregate type's instances,
   reads each one's raw events (`get_events`), and upcasts and decodes them one at a time. The first
   that does not decode becomes an `UnreadableEvent` — aggregate type, id, sequence, stored event type
   and the decoder's reason — printed as `person event <id> #2 (NameAsserted): <reason>`. The backup
   check (`decode_raw_event`, ADR 0041) decodes through the same function.

2. **An aggregate's projection holds the events before its first unreadable one.** Each aggregate's
   readable prefix is dispatched as one batch through the replay's query, so its record shows what it
   was just before that event. No later event is folded, since it may depend on the one skipped (a
   retraction of the assertion it made).

3. **An aggregate holding an unreadable event stays in the write journal.** A replay that meets one
   journals the aggregate, unless an entry already names it. The journal already means "this
   aggregate's projection may be behind" (ADR 0050 §3). The open-time catch-up reads it like any
   journalled aggregate, keeps the entry and collects the event. The first catch-up dispatches the
   readable prefix through every query and records in the entry the sequence of the unreadable event
   it stopped at (`replayed_to`). A later open that finds the same one dispatches nothing, so an open
   over an unreadable aggregate writes nothing. Every open finds the unreadable events again without
   decoding the whole log, and a version that reads them folds the rest on its next open with no
   rebuild.

4. **Neither open nor rebuild fails on an unreadable event.** The stale-table rebuild and the record
   origins fill complete for everything readable, and the fill is marked complete. A rebuild returns
   the events it met. A failed projection write still fails either one (#486).

5. **The store reports them; the frontends warn.** `Store::unreadable_events` holds what the open's
   catch-up found. The CLI prints a localized warning on stderr naming each event, and the GUI shows a
   persistent banner under the top bar. Both say to update vitni if a newer version last used the
   workspace, and otherwise to restore an earlier backup — a backup taken now carries the event, and a
   restore refuses it.

## Rationale

- **The journal, not a new table.** The events are append-only, so an unreadable event stays
  unreadable until the program changes. The journal already replays an aggregate on every open until
  its replay succeeds; keeping the entry is exactly "until it reads". A table of unreadable events
  would have to be cleared by the same replay anyway.
- **Per-aggregate reads, not a stream.** Reading one aggregate's log at a time holds no cursor while
  the query writes, so the replay needs no read-ahead buffer on a one-connection pool (ADR 0044 §2),
  and each aggregate's view is loaded and written once rather than once per event.

## Consequences

- An unreadable event in an aggregate no replay touches is not seen until something replays it — a
  rebuild, or a command on that aggregate, which fails to load it as before.
- Each open reads the journalled unreadable aggregates' logs: one read per aggregate, no writes.
- The write journal gains a nullable `replayed_to` column, added to an existing journal on open.
- ADR 0050 §5 (a journalled aggregate that cannot be replayed is logged and stays journalled) is
  unchanged; an aggregate with an unreadable event is not a failed replay.

## References

- ADR 0010 — projection rebuild.
- ADR 0032 — projection schema evolution by replay.
- ADR 0037 §4 — the record origins index and its write journal.
- ADR 0041 — backup and restore.
- ADR 0050 — projections record their last event.
- #486, #535.
