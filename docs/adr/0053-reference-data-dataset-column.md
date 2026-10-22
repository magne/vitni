# 53. Reference data is told apart by the datasets a record came from

- **Status:** Accepted
- **Date:** 2026-10-22

## Context

The Norwegian reference import (#697, [`gis-norway.md`](../research/gis-norway.md) phased plan P3)
brings in about 1,100 municipalities and 1,165 parishes. They land in the same Places list as a user's
handful of family farms (risk 9, open question 2), and the list today cannot tell the two apart. The
research named three options:

- **A tag convention.** The importer tags every reference place, and the list filters on the tag
  (ADR 0052's *has none of*). It works today with no model change. But a tag is editorial: the user can
  remove it, by accident or in a bulk untag, and the place silently rejoins their own. And a tag says
  only *reference*, not *which* dataset.
- **A list filter on where the record came from.** The importer already records that: every assertion
  it writes carries a `RecordOrigin` naming its dataset (ADR 0037 §1), and `record_origins` indexes
  them. It is a fact the user cannot remove by mistake, and it names the dataset.
- **A reference-data partition**: a separate store, or a flag that keeps reference records out of
  lists by default. A large change to every read path, for a separation the filter already gives.

Two more questions wait on the same import (open question 3, risk 10): whether the data is bundled in
the plugin or fetched, and where its CC BY 4.0 and NLOD 1.0 attribution is shown.

ADR 0052 §1 fixes the column value types (text, date, number, choice, tags, record) and §2 the
operators each one takes. A dataset is none of them, so the second option extends that list.

## Decision

1. **A new column value type, *datasets*: a set of `DatasetId`** (ADR 0037 §3). Its operators are the
   tags type's: *has any of*, *has all of*, *has none of*. A value is held by its `DatasetId` and shown
   by the dataset's label (ADR 0037 §5, the label of its first run), so a relabel does not break a
   query. This extends ADR 0052 §1 and §2.

2. **Every listed kind that `record_origins` covers has a `dataset` column of that type.** Its value is
   the set of datasets with at least one **live** `record_origins` row for the record. A record entered
   only by hand has the empty set. The column is materialised in the kind's list table (ADR 0052 §5) and
   recomputed when the record's origin rows change: an import adds one, a correction flips `live`.
   Rejected rules:
   - *The creating assertion's dataset only* hides that a later import added claims to the record, so
     "the persons Digitalarkivet contributed to" would miss every person it enriched but did not create.
   - *Every assertion, live or not* counts a dataset whose every claim the user has retracted.

3. **A reference importer uses one global dataset id per published product**, such as `ssb-klass` for
   SSB's classification codes and `ssb-kart` for its boundaries. One id covers every vintage, so a
   filter written today still matches next year's import. Each vintage is its own `Source` plus
   `Citation` (publisher, licence, version date, URL), which every assertion from it cites (ADR 0020).

4. **Nothing is hidden by default.** The user narrows the Places list with *dataset has none of
   [SSB …]*, and can save that as a search (ADR 0052 §8). Pickers are unchanged: they match the typed
   text and cap their rows, so a reference place appears only when its name is typed — which is what a
   user linking a farm to its municipality wants.

5. **Reference data is fetched at runtime, never bundled.** The plugin reads KLASS and `kart.ssb.no`
   over `net` (ADR 0017), so no plugin bundle redistributes the data. Its attribution is the dataset's
   `Source`: reachable from each place through its citations, and listed with the other sources. There
   is no separate attribution view.

6. **A dataset a clause names that has no record left matches nothing**, as a deleted tag does
   (ADR 0052 §10): *has any of* it finds no one, *has none of* it finds everyone.

## Rationale

- **A fact, not a convention (1, 2).** The separation rests on provenance the importer must write
  anyway (ADR 0037 §1), so it cannot drift from what was imported. A tag would be a second, editable
  copy of the same fact.
- **Any live origin (2)** answers "which datasets does this record rest on", the question both a
  reference filter and a researcher's "what came from Digitalarkivet" ask, and it follows corrections.
- **One id per product (3)** keeps filters and saved searches stable across vintages, while the
  `Source` per vintage keeps the evidence precise.
- **A runtime fetch (5)** follows the research's recommendation: the payloads fit `net`'s cap, nothing
  is redistributed, and the attribution obligation becomes a data fact rather than a UI afterthought.

## Consequences

### Positive

- A user's own places are one filter away, and the filter cannot be undone by an untag.
- The same column serves every imported kind: persons from Digitalarkivet, events from one GEDCOM file.
- No partition, no new read path, and no change to pickers.

### Negative / costs

- The list tables gain a column per kind, filled from `record_origins`, and an origin write now
  recomputes the record's list row. A workspace predating it rebuilds its list tables on open.
- The column shows only what an importer recorded. A reference place the user entered by hand is not
  reference data to the filter.
- The reference import needs the network, and breaks if SSB withdraws unauthenticated reads (research
  open question 1).
- The origin is in no export (ADR 0037 §2), so the separation does not survive a GEDCOM or Gramps
  round-trip; the `Source` and `Citation` do.

## Out of scope

- A reference-data partition, or hiding reference records by default.
- Ranking a user's own records ahead of reference ones in pickers.
- A dedicated attribution or licences view.
- How the ecclesiastical layer is delivered: its only official geometry is SOSI, and the research keeps
  `import-source` as its fallback.

## References

- ADR 0017 — the `net` capability the reference plugin reads through.
- ADR 0020 — the citations an assertion's `EventContext` carries.
- ADR 0037 — `RecordOrigin`, `DatasetId`, `record_origins` and the dataset label.
- ADR 0052 — the column catalogue, value types, list tables and saved searches this extends.
- [`gis-norway.md`](../research/gis-norway.md) — open questions 2 and 3, risks 9 and 10.
- #696, the design question this answers; #697, the import that follows it.
