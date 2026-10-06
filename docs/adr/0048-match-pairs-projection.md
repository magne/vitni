# 48. Match pairs are a projection refreshed from what changed

- **Status:** Accepted
- **Date:** 2026-10-06

## Context

The duplicate check on the Dashboard and the Matches tool list every pair of records the matching
engine judges possibly the same (ADR 0038 §8, ADR 0039 §3). Both called `similar_pairs`, which scored
every candidate pair the `match_keys` index yields (ADR 0038 §7) on every show. On the bench's name
pools that is about 86 candidates per person. At 100k persons one scan took 452 s on one core before it
was spread over the cores, and the assessments it held ran to gigabytes (#493).

A pair's score reads more than the two records. A person's profile holds their parents, partners and
children, with the places of their vital events and the countries those lie in. An event's profile
holds everyone taking part. A family's holds its partners' profiles and its marriage, and a citation's
its source with the repositories holding it. So an edit to one record changes the scores of pairs it is
not in.

## Decision

1. **A `match_pairs` table holds every pair scored at least `Possible`: `(kind, a, b, band, score)`.**
   `a` is the lower aggregate id. Only the band and score are stored. A consumer that shows a pair's
   terms assesses that pair again, so only the pairs on screen are assessed. The Dashboard lists the
   five strongest and the Matches tool the hundred strongest, and both count the rest.

2. **It is refreshed from the records committed to since, and from every record whose profile reads
   one.** Every commit to a matchable record marks it in `match_pairs_dirty`, beside `match_dirty`, with
   the same generations (ADR 0038 §7). The duplicate check and the queue refresh the pairs before they
   read them. A lookup such as `find_similar` refreshes only the keys. From each dirty record the refresh
   follows the references a profile reads backwards to every record whose profile reads it:
   - a place reaches the places it encloses, transitively, and the events at any of them;
   - an event reaches the people taking part;
   - those people reach their events, the families they are partners or children in, and the other
     members of those families;
   - those members reach the families they belong to, since a partner's profile in a family holds the
     partner's parents and lineage;
   - an event reaches the families it is linked to;
   - a repository reaches the sources it holds;
   - a source reaches its citations.

   A reference a record drops is gone from the index, so it cannot be followed back. When the record
   links index drops one, it marks the record the reference pointed at, for its keys and its pairs.
   A child removed from a family, or a person who no longer takes part in an event, is refreshed that
   way.

   Each such record's pairs are deleted and scored again against the candidates its keys meet. The
   meet of two records' keys is symmetric, so a record's own candidates find every pair it is in. A
   kind with 1000 or more such records is scored whole across the cores instead.

3. **The index follows three more references.** The relations `place-enclosure` (place → enclosing
   place), `family-event` (family → linked event) and `source-repository` (source → repository) are
   added to the record links index (ADR 0047), as its consequences foresee.

4. **Identity decisions are applied when the pairs are read, not when they are written.** A stored pair
   is the engine's raw judgement of two records. A read leaves out each pair with a merged member, from
   `identity_links`. It also leaves out each pair of two cluster roots held distinct. Those come from a
   new `identity_distinctions` table, mirrored per record beside the merge edges (ADR 0039 §4). A
   decision or its undo therefore changes the listed pairs at once, without a rescore. A merge is
   recorded only on the survivor's stream, so a merged record is never itself dirty.

5. **The pairs record what they were scored under.** `match_pairs_state` holds a fingerprint of the
   keys' fingerprint, the engine version and the `[matching]` settings. When it differs, or is missing
   after a projection rebuild, every kind is scored whole again. A rebuild that finds its own
   fingerprint already written lost a race and writes nothing, as for the keys.

## Rationale

- **A projection, not a cache.** The pairs are derived state, rebuildable from the log and the packs
  (ADR 0010). They live beside the keys and follow the same dirty and fingerprint rules, so they add no
  new kind of machinery.
- **Raw pairs, decisions on read.** If the decisions were applied on write, the refresh would have to
  find every record a decision or its undo touches. An undone merge is recorded on the survivor alone,
  and the merged record cannot be found from the survivor once the merge is undone. Reading through
  the two identity tables is a join over small tables.
- **Band and score only.** Storing every pair's terms would grow the table by the gigabytes the scan
  used to hold in memory, for terms that only a handful of pairs ever show.
- **Follow references backwards rather than recording what each profile read.** Recording, for every
  record, each record its profile read would cost tens of rows per person. The references a profile
  follows are few and already indexed, and a test fixes the closure. That test makes an edit along
  each reference and checks that the refreshed pairs equal the pairs scored from scratch.

## Consequences

- The Dashboard and the Matches tool read their pairs from the table. A refresh after one edit rescores
  only the few records the edit reaches. The bench numbers are in #493's pull request.
- The first read after a projection rebuild, a pack or settings change, or a large import scores
  every kind whole. That is the old cost, paid once rather than on every show.
- A workspace opened with projections that predate the new relations is missing their rows until
  `vitni rebuild` refills the record links index.
- A new reference that a profile follows needs a record links relation and a step in the refresh
  closure, and the closure test needs an edit along it.

## References

- ADR 0010 (rebuildable derived state), ADR 0038 §7 and §8 (blocking index, one engine for every
  consumer), ADR 0039 §3 and §4 (review queue, identity clusters), ADR 0047 (record links index).
- Issue #493.
