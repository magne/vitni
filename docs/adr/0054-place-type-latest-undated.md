# 54. A place's current type is its latest undated statement; Custom labels upgrade by Gramps rule

- **Status:** Accepted
- **Date:** 2026-10-23

## Context

ADR 0031 §3 replaces the closed `PlaceType` enum with a dated, accumulating type statement of an optional
`kind`, an optional `level` and an optional `type_key`. Building it (#714) met four points the ADR leaves
open or states wrongly:

- ADR 0026 §1 resolves a dated attribute as of a date and falls back to the **first** undated statement.
  That suits names. For a type it picks the oldest guess over the user's latest correction.
- A text-only date carries `sort_value: 0`, so two unrelated text-only types would look like the same date.
- ADR 0031 §10 maps the old `Custom(label)` to `custom:<label>`. A fresh Gramps import of the same label
  does better: it knows the 20 standard labels, and it treats `Unknown` as no type.
- ADR 0031 §10 says `PlacesMerged` and `PlacesDistinguished` change shape. They do not.

## Decision

1. **The latest live undated type wins when no dated type qualifies.** This amends ADR 0026 §1 for place
   types only; names keep the first-undated rule. With no date context the current type is the one resolved
   as of the latest date, so a dated statement beats an undated one. "Same date" means equal `sort_value`,
   the value `resolve_as_of` compares.

2. **A place type's date is a calendar date.** `decide` refuses a text-only date on `SetPlaceType` and
   `CreatePlace`. A statement with a date but no kind, level or key is refused on both commands
   (`EmptyPlaceType`), since a date with nothing stated says nothing.

3. **An empty upgraded statement is a live assertion that holds no type.** An old `PlaceTypeSet(Custom("place"))`
   upgrades to an empty `PlaceTypeSet`. It can be retracted or superseded, but it no longer hides an earlier
   type as it did under last-writer-wins. This is a deliberate consequence.

4. **A `Custom(label)` upgrades by the rule a Gramps import uses.** The 20 Gramps standard labels become their
   `std:` key, so `Custom("Hamlet")` becomes `std:hamlet`. `Custom("Unknown")`, blank text and `Custom("place")`
   become no type. Any other text becomes `custom:<text>`. The result is a pure function of the old value. This
   departs from ADR 0031 §10's table for `Custom("Unknown")`, `Custom("")`, whitespace-only text, `Custom("place")`
   and standard-label text. A Gramps file whose custom type is literally `place` or `Unknown` imports untyped.

5. **A same-date retype supersedes the live same-date statement by `AssertionId`.** The `PlaceCreated` entry is
   never superseded: a plain `PlaceTypeSet` outranks it by rule 1, so undoing the first retype restores the
   created type. The choice is made in the app layer; `decide` stays pure.

6. **The Explorer `type` cell shows the key's label for `std` and `custom:` keys and the bare key for any
   other key.** With no key it shows the kind label, else the level label, so a stated type is never an empty
   cell. The record view keeps ADR 0031 §4's kind label and key together.

7. **`PlacesMerged` and `PlacesDistinguished` keep event version "1.0".** Stored evidence holds only the
   feature, the outcome and the weight, so the place-type change does not alter their shape. This corrects
   ADR 0031 §10. `PlaceCreated` and `PlaceTypeSet` move to "2.0".

8. **Gramps round-trip.** Export strips one `custom:` prefix and writes any other key as full text. A typed-in
   `std:foo` or `custom:Sokn` imports as `custom:std:foo` and `custom:custom:Sokn`. An untyped place exports as
   `Unknown`. A key such as `no:fylke` re-imports as `custom:no:fylke`, which is the limit of ADR 0031 §4's
   "restores exactly" until imports can recognise a vocabulary.
