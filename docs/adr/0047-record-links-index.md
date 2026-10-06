# 47. A record links index follows references backwards

- **Status:** Accepted
- **Date:** 2026-10-06

## Context

A matching profile (ADR 0038 §2) reaches past its own record. A person's profile holds their parents,
partners and children, which only the families they belong to name. An event's profile holds every
person taking part, which only those persons' projections name, since participation is the person's
(data-model §10). Rekeying the `match_keys` index (ADR 0038 §7) after a commit has the same problem.
A renamed place changes the keys of the events at it, and a retitled source changes the keys of its
citations.

Each of these references is held by the record that points, not the record pointed at. The only way to
follow one backwards was to list every projection of the pointing kind. So `find_similar` read every
person, event, place and family to score the few hundred candidates the index yields. That took about
1.1 s at 100k persons (#492), too slow for a picker or for the hint shown while a record is typed in.

## Decision

1. **A derived `record_links` table holds one row per live reference, `(relation, source, target)`.**
   The relations are:
   - family → partner
   - family → child
   - person → event (participation)
   - event → place
   - citation → source

   It is indexed by `(relation, target)` so a reference can be followed backwards.

2. **It is mirrored per source record from that record's projection.** A query is appended after the
   person, family, event and citation projections. When an event that can change the record's
   references commits, the query replaces that record's rows. Like `identity_links` (ADR 0039 §4), it
   is rebuilt by `rebuild_projections` and filled from the projections when a workspace that predates
   it is opened. A failed write fails the command that caused it, as a failed projection write does
   (#486).

3. **Profiles are read by record.** The store reads views by aggregate id, and creating origins for
   named aggregates. `Profiles::include` reads one record together with what its profile needs: its
   families, relatives and their events, its places with every enclosing place, its participants, its
   source and repositories. It also reads the other members of the record's identity cluster, because a
   distinction is stored on one side only.
   - `find_similar`, the draft hint, `assess` and the single-record profile builders read only the
     target and the candidates.
   - The rekey after a commit reads only the records whose keys the commit can change.
   - `similar_pairs`, the index reset and an import's matcher still read whole kinds, since they score
     every record.

## Rationale

- **An index, not a scan of JSON.** SQLite cannot index into a JSON array. A `json_each` scan over
  every family to find one person's families still reads the whole table on each lookup.
- **Mirrored from projections, not from events.** The projection already applies retractions and
  supersessions, so the index holds exactly the live references and never re-implements that folding.
- **Same shape as `identity_links`.** It is a side table fed by a query and rebuilt with the
  projections, so it adds no new kind of machinery.

## Consequences

- `find_similar` at 100k persons drops from about 1.1 s to the cost of the records it scores. The bench
  numbers are in #492's pull request.
- A commit to a person, family, event or citation that changes its references writes the index too.
  Other commits skip it.
- A new kind of reference that a profile follows needs a relation added here.

## References

- ADR 0010 (rebuildable derived state), ADR 0038 §2 and §7 (profiles and blocking index), ADR 0039 §4
  (identity clusters).
- Issues #486, #492.
