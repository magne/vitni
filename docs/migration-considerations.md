# Migration considerations (open)

- **Status:** Notes / not yet designed
- **Date:** 2026-06-21
- **Audience:** anyone planning persistence or workspace-lifecycle work

This captures migration concerns surfaced during Phase 3 (Postgres backend + `genealogy rebuild`)
that are **not yet addressed**. None block Phase 3; each needs design (and likely a gating ADR)
before implementation. They are recorded here so they are not lost.

## What Phase 3 *does* cover

Projection rebuild via **`genealogy rebuild`** (ADR 0010): clear every read-model/view table and
replay the immutable event log (applying upcasters) back into the freshly-created tables. Combined
with additive, versioned events (ADR 0004 §4), this absorbs *event-shape* evolution and projection
*content* changes. Table DDL is idempotent `CREATE … IF NOT EXISTS`. There is no in-place `ALTER`
migration framework — projections are disposable; the log is the source of truth.

## 1. Database schema migration (new views / columns / indexes)

`rebuild` repopulates **existing** tables; it does not evolve their *shape*. Today a new projection
table is created on `Store::open` (idempotent DDL), and adding/removing a column or index on an
existing view table has no migration path — the DDL only runs `CREATE TABLE IF NOT EXISTS`, so a
table that already exists is left as-is.

Open questions:

- How do we apply a schema change to an existing workspace's view tables (add column, change
  index, rename) across both SQLite and Postgres?
- Is a versioned migration mechanism warranted, or does "drop all view tables + recreate + rebuild"
  suffice given projections are disposable? (Dropping and recreating, then `rebuild`, is the cheap
  path and may be all we need — but it must be a deliberate, tested operation, not implicit.)
- The event-store tables (`events`, `snapshots`) are owned by `cqrs-es`/`{sqlite,postgres}-es`;
  any change there is constrained by those crates' expectations.

## 2. Cross-database / cross-engine migration (e.g. SQLite → Postgres)

A workspace's `database_url` is **frozen at `init`** (`workspace.toml`); there is no way to move an
existing workspace's data to a new database, or to a different engine. The natural shape, given
event sourcing, is a command that **copies the event log** from a source workspace/database into a
fresh target (new `database_url`, possibly a different engine) and then runs `rebuild` to
reconstruct the projections — the log is engine-neutral JSON, so this is a faithful, lossless
transfer.

Open questions:

- A `genealogy migrate <from> <to>` (or `export`/`import` of the raw event stream) command:
  source → target engine, preserving aggregate ids, sequence, event versions, and metadata exactly.
- Is this a new workspace, or an in-place re-point of an existing one? (Freezing `database_url`
  suggests "new workspace, then switch the registry entry / default".)
- Interaction with the planned WASM import/export plugins (Phase 4) and the server-connected
  workspaces of Phase 11 — a remote workspace is another "engine" behind the same `Store` seam.

## 3. Configuration migration (e.g. new `HumanId` formats)

`HumanId` formats are workspace config (ADR 0005): a global default with per-workspace overrides,
resolved live at open. Changing a format does **not** re-render existing ids — `I0001` stays
`I0001` even if the format becomes `P-%04d`, and allocation simply continues numerically. There is
no operation to migrate existing user-facing ids to a new format.

Open questions:

- Should there be a command to **re-render** existing `HumanId`s when the format changes — e.g. if
  an id matches the *old* format, map it to the *new* format (`I0001` → `P-0001`), leaving
  non-matching ids untouched?
- `HumanId` changes are observable identifiers that may be cited externally; re-rendering must be
  an explicit, audited operation (a new event in the affected aggregate, not a silent rewrite), to
  keep the provenance trail intact.
- More general config migrations (privacy rules, surety scheme, data-language metadata) likely
  share this "explicit, audited, opt-in" shape.

## Sequencing

These are post–Phase 3. The cross-engine migration (2) pairs naturally with Phase 4
(import/export plugins) and Phase 11 (server-connected workspaces); schema migration (1) and config
migration (3) should each get a short design note / ADR in the cycle that implements them, grounded
in the working `rebuild` path rather than speculation.
