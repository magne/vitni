# Project roadmap

- **Status:** Draft
- **Date:** 2026-06-19
- **Audience:** anyone planning or sequencing work on the genealogy workspace

This roadmap says **what to build next** and **in what order**. It is derived from the current
code, [`docs/data-model.md`](data-model.md), and the accepted ADRs
([0001](adr/0001-use-event-sourcing-for-the-domain-core.md)–
[0008](adr/0008-ui-framework-dioxus.md)). It references those decisions; it never restates them
as new ones (ADRs are immutable). A visual companion lives in [`roadmap.html`](roadmap.html).

## Guiding principle: eliminate unknowns early

The architecture is decided; most of the *risk* is in proving the hard, unbuilt parts actually
work together. So the strategy is **risk-first vertical spikes, then breadth**:

1. **Phase 1 — spikes.** Build the smallest end-to-end slice that proves each frontier unknown
   (cross-aggregate model, event evolution, the WASM plugin host, the UI split). Each spike exists
   to kill one unknown, not to ship a finished feature.
2. **Phases 2–6 — breadth.** Once no major unknown remains, fill out the remaining aggregates,
   backends, importers/exporters, and UI screens by repeating patterns the spikes proved.

Horizon: **full vision to 1.0**. Two constraints from the project owner shape the plan:

- **Import and export are WASM plugins**, not native code (consistent with ADR 0007 §9: base
  plugins ship as components). The GEDCOM import/export plugin *is* the proof of the plugin host.
- **Second-locale catalogues stay complete as we go** — the i18n completeness checker and localized
  date/number formatting land in Phase 1, not at the end.

## Phase 0 — Current baseline (done)

The workspace builds clean (zero warnings), and 69 tests pass across four crates.

| Crate | State |
| --- | --- |
| `genealogy-core` | **Person + Family aggregates only** (2 of 12). Full value-object catalog, pure `decide`/`evolve`, `EventContext` + `AssertionId` carried in the payload. |
| `genealogy-db` | **SQLite working** (`cqrs-es` + `sqlite-es`), projections, `HumanId` allocation. **Postgres stubbed** (returns `Unsupported`). |
| `genealogy-app` | Use-cases for Person + Family, `Session` (the sole clock/UUID-v7 boundary), config + workspace lifecycle, frontend-neutral DTOs. |
| `genealogy-cli` | `init`, `person create/add-name/show/list`, `family create/add-partner/remove-partner/add-child/remove-child/show/list`. Fluent i18n (`en`, `no`). |

**Unknowns still open (what the roadmap exists to close):** the 10 remaining aggregates (Event,
Place, Source, Citation, Repository, Media, Note, Tag, DnaTest, DnaMatch); cross-aggregate id links
and projection-based invariant checks (the "aggregate tax", data-model §9); `PersonsMerged`;
event-version upcasting; projection rebuild-from-log; the Postgres backend; `genealogy-plugin-host`
(WASM); import/export; `genealogy-ui` + `genealogy-ui-dioxus`; the plugin-UI vocabulary.

> Note: `EventContext.citations` already exists in `genealogy-core`, but the **Citation aggregate
> does not** — provenance links currently have nothing to point at. Spike A closes this.

## Phase 1 — De-risking spikes

Each spike is the thinnest end-to-end slice that proves a hard unknown. Ordered by dependency:
Spike A unblocks the provenance link used everywhere else.

### Spike A — Cross-aggregate model ✅ done

**Goal.** Add minimal **Event**, **Place**, **Source**, and **Citation** aggregates — only as much
as needed to prove the multi-aggregate machinery.

**What it must prove.**

- Cross-aggregate references live as ids **in the event payload**, never implicit in a stream key
  (ADR 0002 self-contained-events rule; data-model §9).
- Person↔Event participation via `ParticipationAsserted` (links a Person to an Event with a
  `ParticipantRole`).
- The Citation→Source link, and wiring `EventContext.citations` to **real** Citation aggregates.
- **Projection-based invariant checks** — `UnknownPlace` (an Event's `LinkPlace` to an unknown
  place) and `UnknownSource` (a Citation against a missing Source), validated against
  possibly-lagging projections rather than transactionally (the §9 "aggregate tax", §10.1 errors).

**Also lands here (i18n, moved up).** Event and Place are the first **date-bearing** aggregates, so
this is where localized `GenealogicalDate` rendering is first exercised: ICU4X (ICU 2.x) date/number
formatting, with genealogical date qualifiers as Fluent terms (ADR 0003). With two locales already
present (`en`, `no`), the **multi-locale completeness checker** also lands now — ADR 0003 deferred
it only until a second locale existed; that condition is met, and landing it before string volume
grows keeps the `no` catalogue from drifting.

**Crates/types touched.** `genealogy-core` (new `event`, `place`, `source`, `citation` modules
following the Person/Family template); `genealogy-db` (projections + invariant-check queries);
`genealogy-app` (use-cases + DTOs); `genealogy-cli` (commands + `.ftl` strings); i18n tooling.

**Exit criteria.** An Event can be created, linked to a Place and to participant Persons, and cited;
`UnknownPlace`/`UnknownSource` are returned for dangling refs; a localized date renders correctly in
both `en` and `no`; the completeness checker fails CI when a key is missing from `no`.

### Spike B — Event evolution + projection rebuild ✅ done

**Goal.** Prove schema evolution and read-model rebuild before the event log grows large.

**What it must prove.**

- Add a `v2` to one event variant, write an **upcaster**, and confirm historical `v1` events still
  decode (ADR 0004 §4: variant-name + version, internally-tagged JSON, additive only).
- A **rebuild** path that drops projections and replays them from the event log.

**Why now.** Upcasting and rebuild are cheap with a handful of aggregates and brutal once the log is
large and the schema has drifted. ADR 0004 fixed the *encoding* on day one precisely so this is
possible; this spike proves the *tooling*.

**Crates/types touched.** `genealogy-core` (versioned event + upcaster); `genealogy-db` (rebuild
routine over the event store).

**Exit criteria.** A workspace written with the `v1` event schema reads back correctly after the
`v2` upcaster is added; a rebuild reproduces identical projections from the log.

### Spike C — Plugin host (WASM) + GEDCOM import/export plugin

**Goal.** Stand up the plugin system and prove import/export through it — the single biggest
technical unknown.

**What it must prove.**

- `genealogy-plugin-host` (new crate above `genealogy-app`): Wasmtime + Component Model, **one
  versioned WIT host world**, deny-by-default capabilities, per-instance resource limits (memory,
  fuel/epoch timeout) — ADR 0007.
- A **GEDCOM import plugin** and a **GEDCOM export plugin** as `wasm32-wasip2` components: import
  `INDI`/`FAM` → persona Persons + a Family via the pure `decide` path, attributed to a
  `Software` operator; export Person/Family → GEDCOM.
- The DTO boundary holds for plugins (no `cqrs-es`/`sqlx` leakage), and machine-made claims are
  audited as `AgentKind::Software` in `EventContext` (data-model §11, §13).

**Why this shape.** The project owner's constraint is that import/export *are* plugins. Folding them
into the plugin spike means one slice proves the host, the WIT/DTO boundary, Software-agent
provenance, and a real import/export round-trip at once.

**Crates touched.** New `genealogy-plugin-host`; two plugin component crates; `genealogy-app`
(use-case entry points the host calls).

**Exit criteria.** A GEDCOM file imports into a workspace as personas + family with Software-agent
provenance, and re-exports to GEDCOM; capabilities are denied unless granted; a runaway plugin is
stopped by the resource limit.

### Spike D — UI layer (Dioxus)

**Goal.** Prove the framework-agnostic presentation split and the plugin-UI vocabulary.

**What it must prove.**

- `genealogy-ui` (new crate, depends on `genealogy-app` only, **zero framework types**): view-models
  derived from DTOs for a person list + detail, intent dispatch to use-cases, Fluent resolution.
- `genealogy-ui-dioxus` (new GUI binary, parallel to the CLI): one real screen — person list →
  detail — in RSX, routing events to `genealogy-ui` intents.
- A **minimal plugin-UI vocabulary** type (a serializable form/list description) plus a per-framework
  interpreter that renders one plugin-supplied form (ADR 0007 follow-up, ADR 0008).
- The dependency direction `app → ui → ui-<framework>` holds: no `dioxus::` type appears above the
  renderer crate.

**Exit criteria.** The Dioxus binary lists persons from a real workspace and opens a detail view; a
plugin-described form renders through the vocabulary interpreter; `genealogy-ui` compiles with no
framework dependency.

> After Phase 1, no major unknown remains. Phases 2–6 repeat proven patterns.

## Phase 2 — Complete the domain (breadth)

Fill the remaining aggregates using the Person/Family template
(`command`/`event`/`state`/`view`/`decide`/`error` + app use-cases + CLI):

1. Finish **Place**, **Source**, **Citation** (started as stubs in Spike A).
2. Add **Repository**, **Media**, **Note**, **Tag**.
3. Add **DnaTest** and **DnaMatch** (data-model §12), keeping the relationship *inference* as a
   citing `FactAsserted`/`AssociationAsserted`, not a field on the match.

Add the cross-cutting Person/Family operations that are proven viable but not yet built:
`PersonsMerged` (non-destructive merge, data-model §9), `AssociationAsserted`, privacy, and the
universal retract/supersede pair across all aggregates.

## Phase 3 — Persistence hardening

- Implement the **Postgres** backend behind the existing `PersistedEventRepository`/`Store`
  abstraction; prove per-workspace engine selection end-to-end (ADR 0002).
- Promote Spike B's rebuild into a real maintenance command + a migration story.
- **Snapshotting only if measured to be needed** — ADR 0004 defers it until replay cost warrants it.

## Phase 4 — Import/export breadth (all as WASM plugins)

Build out on the Spike C foundation:

- **Digitalarkivet** importer plugin (consuming the `genealogy-import` fixtures — never reformat
  them).
- **Gramps XML** import/export plugin; full **GEDCOM 7** round-trip.
- `ExternalId`-based re-import idempotency, deduplication, and sync (data-model §11).
- Capability-grant UX, plugin signing, and three-layer plugin loading (workspace > app-dir >
  embedded), mirroring the ADR 0003/0005 override model.

## Phase 5 — UI breadth

- Full screen inventory: CRUD for every entity, pedigree/tree views, citation/evidence editing, DNA
  match views, and the non-destructive merge UI.
- A design system and the complete plugin-UI vocabulary.
- Second-framework readiness check: a new renderer must reuse `genealogy-ui` unchanged (ADR 0008).

## Phase 6 — 1.0 hardening

- Configurable surety scheme (data-model §17).
- `ResearchNote`/`Argument` aggregate for proof arguments (data-model §17).
- DNA depth: Y/mtDNA markers, haplogroup detail, triangulation groups (data-model §17).
- Performance profiling; packaging and distribution.

> Moved up: localized date/number formatting and the second-locale completeness checker were
> originally end-stage items; they now land in **Spike A** so catalogues stay complete from the
> first date-bearing aggregate onward.

## Risk register

Each frontier unknown maps to the spike that kills it.

| Unknown | Killed by | Status |
| --- | --- | --- |
| Cross-aggregate id refs + projection invariant checks ("aggregate tax") | Spike A | ✅ Done |
| Dangling `EventContext.citations` (no Citation aggregate) | Spike A | ✅ Done |
| Localized dates + second-locale catalogue drift | Spike A | ✅ Done |
| Event-version upcasting (schema evolution) | Spike B | ✅ Done |
| Projection rebuild from the log | Spike B | ✅ Done |
| WASM plugin host (Wasmtime, WIT world, capabilities) | Spike C | Planned |
| Import/export round-trip as plugins; Software-agent provenance | Spike C | Planned |
| Framework-agnostic UI split; plugin-UI vocabulary | Spike D | Planned |
| Postgres backend / per-workspace engine selection | Phase 3 | Planned |
| Non-destructive merge (`PersonsMerged`) | Phase 2 | Planned |

## New ADRs required

The existing ADRs deliberately deferred several decisions (their "out of scope" sections). Those
decisions must be **made as ADRs** before the phases that depend on them — the roadmap cannot land a
spike on an undecided contract. Proposed numbers are sequential and ordered by when each is needed;
they are confirmed when the ADR is written.

| Proposed | Decision to make | Gates | Deferred by |
| --- | --- | --- | --- |
| [ADR 0009](adr/0009-read-model-and-projection-schema.md) — **accepted** | Concrete read-model / projection schema | Spike A | ADR 0002, 0004 |
| [ADR 0010](adr/0010-event-version-upcasting-and-projection-rebuild.md) — **accepted** | Event-version upcasting mechanism + projection rebuild | Spike B | ADR 0002, 0004 §4 |
| ADR 0011 | Plugin host WIT world versioning + capability-grant model + resource limits | Spike C | ADR 0007 |
| ADR 0012 | Plugin-UI vocabulary schema (the named ADR 0007 follow-up) | Spike D | ADR 0007, 0008 |
| ADR 0013 | Import/export mapping strategy (GEDCOM 7 / Gramps XML, ExternalId dedup) | Phase 4 | data-model §16–17 |
| ADR 0014 | Plugin signing, trust tiers, and distribution | Phase 4 | ADR 0007 |

Conditional — write an ADR only if/when the option is adopted (direction already fixed, so not
blocking):

- **Snapshotting** (Phase 3) — only if replay cost is measured to warrant it (ADR 0004 defers until
  measured).
- **Configurable surety scheme** (Phase 6) — data-model §17; the fixed five-level `Confidence` ships
  first.
- **DB-backed operator aggregate, authentication, record signing** (Phase 5–6) — ADR 0005 fixed the
  direction; an implementation ADR follows when built.

Sequencing rule: **write the gating ADR in the same cycle as the spike it unblocks**, not before.
The spike informs the decision (e.g. building the Spike A projections reveals the right schema for
ADR 0009), keeping ADRs grounded in working code rather than speculation.

## Dependency notes

- Spikes B, C, and D all build on the cross-aggregate machinery proven in **Spike A**.
- **Spike C** sits above the `genealogy-app` DTO boundary (ADR 0006) and reuses the provenance model
  (ADR 0001/0004) for Software agents.
- **Spike D** consumes `genealogy-app` DTOs (ADR 0006) and Fluent strings (ADR 0003), and is where
  the plugin-UI vocabulary (ADR 0007 follow-up, ADR 0008) first appears.
- Phase ordering honors the four decided constraints: full vision to 1.0, de-risk all frontiers
  first, risk-first then breadth, and import/export as WASM plugins.
