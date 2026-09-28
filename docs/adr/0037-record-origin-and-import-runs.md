# 37. Record origin: source-record keys on assertions, and import runs

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

An import should be idempotent: importing the same record twice is a no-op, and a record that has
changed since the last run updates only what changed. Today that holds only partly, and only for two
of the thirteen aggregates:

- **Only Person and Family resolve on re-import.** Their `ExternalId` is the resolve-or-create key
  (ADR 0013 §6, data-model §11). Every other aggregate is deduplicated, at best, *inside one run* by
  guest-side maps (GEDCOM place names, source xrefs). So a second import of the same GEDCOM or Gramps
  file duplicates every Place and Source (`docs/issues.md` → *Bulk import, export & sync*).
  Digitalarkivet dedups its source, repository and media by matching titles, names and paths through
  `query`, which is a heuristic rather than an identity.
- **File-local keys are used as if they were global.** GEDCOM resolves by authority `gedcom-xref`
  with the xref as value (`@I1@`), and Gramps by `gramps-id` (`I0001`). Neither is scoped to a file.
  So importing a second, unrelated file resolves its `@I1@` onto the first file's `@I1@` person and
  silently attaches one person's data to another.
- **Nothing records which record a claim came from.** A Digitalarkivet census page yields a person,
  an occupation and a citation. After import, nothing says "these three assertions were read from
  record `pf01073902000464`". Without that link, the importer cannot:
  - skip an unchanged record
  - notice a user retraction and avoid re-asserting the retracted value (*Retraction resurrection
    blocks recurring imports*, `docs/issues.md`)
  - let the user see which record a value came from
- **An import run has no identity.** History folds consecutive Software-agent entries into a
  display-only `ImportBatch` row (`vitni-app/src/history.rs`, `collapse_runs`). There is no persisted
  "this import, of this file, by this operator, at this time".

The model already puts provenance in the payload (ADR 0004 §1): `EventContext` records who, when, why,
how sure, and on what evidence. What it lacks is the *mechanical* provenance of an import: which
dataset, which record in it, which entity in that record, and which run.

## Decision

1. **`EventContext` gains an optional `origin: RecordOrigin`.** The field is additive and
   `#[serde(default)]`, so every historical event still decodes (ADR 0004 §4). The type:

   ```rust
   pub struct RecordOrigin {
       /// The dataset the record belongs to (§3).
       pub dataset: DatasetId,
       /// The record's id within the dataset (a Digitalarkivet `pf…`/`pd…`/`bf…` id, a GEDCOM xref,
       /// a Gramps handle).
       pub record: String,
       /// The entity within the record this assertion describes, when a record yields more than one
       /// (`person`, `participant:father`, `event:census`, `place:birthplace`, …). Chosen by the
       /// importer; stable across runs of the same importer over the same record.
       pub item: Option<String>,
       /// A digest of the item's canonical incoming fields, for no-op detection (§4).
       pub digest: Option<ContentDigest>,
       /// The import run that wrote the assertion (§5).
       pub run: ImportRunId,
   }
   ```

   Every assertion an importer derives from a record carries its origin. An assertion made by a person
   at the keyboard carries none. `(dataset, record, item)` identifies **one entity in one record**. It
   is the key a re-import resolves by.

2. **The origin is internal provenance: in backups, never in exports.**
   - Because it lives in the event payload, it is in the event log, and so it is in every backup
     (ADR 0041).
   - It is in no WIT `*-dto`. Exporters see only what the DTOs carry (ADR 0011, 0013), so GEDCOM and
     Gramps exports never contain it.
   - The UI shows it read-only, as "from record *X* in *dataset*", on a claim's *Why we believe*
     popover and in History. When the dataset has a URL form (Digitalarkivet), the record links out.

3. **Datasets scope file-local keys.** `DatasetId` is a string namespace:
   - **`digitalarkivet`** is global, because its record ids are archive-wide.
   - **`gedcom:<id>`** and **`gramps:<id>`** name one family-tree file *lineage*: the same tree
     re-exported over time. The `<id>` is a UUID v7 minted when a file is first imported.
     - On a later import, the importer proposes the matching dataset from the file's header
       fingerprint and key overlap. The fingerprint is GEDCOM `HEAD.SOUR` / `HEAD.FILE`, or the Gramps
       header. The key overlap is the share of the file's `_UID`s or handles already present under
       that dataset.
     - The user confirms the proposal or declares a new dataset. The CLI takes `--dataset <label>` or
       `--new-dataset`.
     - This is the same "the tool proposes, the user decides" rule as matching (ADR 0038).

   File-local keys (a GEDCOM xref, a Gramps id or handle) become **origin records only**. They stop
   being written as `ExternalId`s. `ExternalId` is kept for identifiers that mean something outside the
   file: GEDCOM `_UID`/`EXID`, a FamilySearch id, a Digitalarkivet record URL. It stays the
   user-visible cross-system identity (data-model §11). This fixes the xref collision structurally,
   rather than by prefixing authority strings.

4. **Re-import resolves by origin, for every aggregate.**
   - A projection index, `record_origins`, holds one row per originated assertion: `(dataset, record,
     item, field_key, aggregate_kind, aggregate_id, assertion_id, digest, run, live)`.
     - It is maintained by one query processor per aggregate and rebuildable by replay (ADR 0010 §5),
       on SQLite and Postgres alike.
     - `field_key` is derived by `vitni-app` from the event: `person.name`, `person.sex`,
       `person.fact.Occupation`, `event.date`, …. It is not stored in the payload.
     - The index is the escape hatch ADR 0009 §4 allows: a secondary lookup the JSON path cannot serve
       at import scale.
   - Resolution then reads the index:
     - **Resolve-or-create:** the aggregate whose *creating* assertion has `(dataset, record, item)`,
       or whose resolution the import run recorded (§5), is the entity. This covers all thirteen
       aggregates, including Place and Source.
     - **No-op:** the incoming item's digest equals the last recorded digest for that origin. Nothing
       is written.
     - **Update:** the digest differs. Each field is reconciled by ADR 0029's timestamp-gated rule.
       Multi-valued fields stay additive.
     - **Tombstone:** a field whose same-origin assertion was *retracted* is never re-asserted with the
       same value. A retraction is editorial judgement, and an import run does not overrule it. A
       *different* incoming value is still offered, because the source changed.
       Tag setters have no assertion chain and cannot be retracted (data-model §8), so no tombstone
       arises for Tag.

5. **An import run is an aggregate: `ImportRun`.** It is the fourteenth, added through the
   `for_each_aggregate!` recipe. Its events:
   - `ImportRunStarted { run, plugin, plugin_version, dataset, dataset_label, source_label,
     file_asserted_at }`
   - `ItemResolved { dataset, record, item, kind, aggregate_id, decision }`, which records a resolution
     that created nothing: an incoming Place resolved to an existing Place by the user or by
     deterministic identity (ADR 0039, 0040)
   - `ImportRunFinished { counts }` or `ImportRunAbandoned { reason }`

   Its operator is the invoking Human, while each imported assertion keeps the `Software` agent
   (ADR 0007 §7). The run is therefore the answer to "who ran this import". Datasets are a projection
   over runs, with the label taken from the first run. History renders a real run row with its counts
   and children, replacing the display-only `collapse_runs` grouping.

6. **The run id is minted by `Session`, like every other id** (ADR 0004 §3). `vitni-core` stays pure:
   `decide` receives the origin in the `AssertionMeta` it is handed, and never computes one.

## Rationale

- **It is provenance, so it belongs in the envelope.** The origin describes *where an assertion came
  from*, exactly like `operator` and `citations`. Putting it on `EventContext` rather than on each
  aggregate's state means one mechanism covers all fourteen aggregates. Every assertion kind gets
  no-op, update and tombstone behaviour without a per-aggregate `ExternalId` field.
- **Separating identity from provenance removes a category error.** `ExternalId` answers "who is this
  in another system?". An origin answers "which record did this claim come from?". GEDCOM xrefs were
  answering the second question while being filed as the first, which is exactly the collision bug.
- **The item key makes multi-entity records addressable.** A church-book record has a child, a father,
  a mother and godparents. A census household has a residence event, a place and several people. A
  record-level id alone cannot tell them apart. Keying on the record alone collapses a record's
  participants onto one person.
- **Datasets are proposed, not guessed silently.** A header fingerprint is often missing or reused
  (many exporters write no `HEAD.FILE`). An automatic guess would reintroduce the collision in a
  subtler form. Asking once per file is cheap and auditable.
- **Recording runs as an aggregate makes the audit trail complete.** It records who started an import,
  over what file, when, and how it ended, and it survives backup and restore like any other history.

## Consequences

### Positive

- Re-importing an unchanged file or record is a true no-op, for every kind, not only Person and Family.
- A user's retraction survives every later import run, which unblocks recurring imports.
- Two unrelated GEDCOM files can no longer merge silently.
- Every imported value can say which record it came from, and link to it.
- History gets real import runs with counts and children.
- The origin is available to the matching engine (ADR 0038) as a deterministic signal. Two assertions
  with the same `(dataset, record, item)` describe the same entity. Two different items of the same
  record describe different entities: a household's members are not each other.

### Negative / costs

- **A fourteenth aggregate.** It costs the full registry recipe: core module, DB tables, app minters,
  CLI verb generation, i18n.
- **One more projection table**, `record_origins`, with its own maintenance on retract/supersede. This
  is its `live` flag.
- **Every importer must choose stable item keys.** A plugin that renames its item keys between versions
  makes its next run look like new data. Item keys therefore become part of each importer's contract,
  tested by a re-run fixture per importer.
- **A dataset prompt on the first import of a file.** The only new interaction, asked once per file
  lineage.
- The Source/Place "resolve-or-create prerequisite" backlog bullets shrink to their remaining ADR 0029
  parts: the WIT setters and a field-level `AssertionId` + `occurred_at` read path.

## Out of scope

- **Editing or removing an origin.** It is a fact of the log, like `occurred_at`.
- **Per-record change dates** (GEDCOM `CHAN`), as in ADR 0029.
- **Exporting origins** in any interchange format. A future sync-oriented export could opt in; none
  does.

## References

- ADR 0004 §1, §3, §4 — provenance in the payload; the pure core; additive events.
- ADR 0007 §7 — Software-agent attribution of imported claims.
- ADR 0009 §4 — the secondary-index escape hatch `record_origins` uses.
- ADR 0010 §5 — projection rebuild by replay.
- ADR 0013 §6 — resolve-or-create by `ExternalId`, narrowed here to true external identifiers.
- ADR 0029 — timestamp-gated reconciliation, applied per field on a changed digest.
- ADR 0038, 0039, 0040, 0041 — matching, identity decisions, staged import, and backup, all built on
  the origin.
- `docs/data-model.md` §8 (`EventContext`), §11 (external sources and import).
