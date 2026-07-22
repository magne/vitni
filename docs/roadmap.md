# Project roadmap

- **Status:** Draft
- **Date:** 2026-07-17
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
2. **Phases 2–10 — breadth.** Once no major unknown remains, fill out the remaining aggregates,
   backends, importers/exporters, and UI screens by repeating patterns the spikes proved.

Horizon: **full vision to 1.0**, with a **post-1.0 expansion sketched in Phase 13** (a backend
server + web frontend, and server-connected workspaces). Two constraints from the project owner
shape the plan:

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

### Spike C — Plugin host (WASM) + GEDCOM import/export plugin ✅ done

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

### Spike D — UI layer (Dioxus) ✅ done

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

> After Phase 1, no major unknown remains. Phases 2–12 repeat proven patterns.

## Phase 2 — Complete the domain (breadth) ✅ done

All 12 aggregates are implemented, landed via PRs #28–#37.

Fill the remaining aggregates using the Person/Family template
(`command`/`event`/`state`/`view`/`decide`/`error` + app use-cases + CLI):

1. ✅ Finish **Place**, **Source**, **Citation** (started as stubs in Spike A). (PR #29, #37, #31)
2. ✅ Add **Repository**, **Media**, **Note**, **Tag**. (PRs #30, #33, #34)
3. ✅ Add **DnaTest** and **DnaMatch** (data-model §12), keeping the relationship *inference* as a
   citing `FactAsserted`/`AssociationAsserted`, not a field on the match. (PRs #35, #36)

Cross-cutting operations added alongside the aggregate breadth:

4. ✅ **`PersonsMerged`** (non-destructive merge, data-model §9) — `MergePersons` command +
   `PersonsMerged` event in the Person aggregate.
5. ✅ **`AssociationAsserted`** — `AssertAssociation` command + `AssociationAsserted` event in the
   Person aggregate.
6. ✅ **Universal retract/supersede** (`AssertionRetracted` / `AssertionSuperseded`) — present
   across all 12 aggregates.
7. ✅ **Privacy as a universal `Restriction` set** — the `private` boolean became a
   `BTreeSet<Restriction>` (GEDCOM v7 `RESN`: Confidential/Locked/Privacy), with a uniform
   `SetRestrictions` command + `RestrictionsChanged` event on **all 12 aggregates** (data-model
   §6, §7, §16). Round-trips through the GEDCOM/Gramps plugins for person/family (host-api 0.9.0);
   per-record round-trip for the remaining records is a documented follow-up (data-model §17).

✅ **Wiring refactor (issue #38) done.** The monolithic registries (db store, CLI i18n) that every
Phase 2 aggregate had to edit are split into per-aggregate x-macro registries, so adding an aggregate
no longer touches a central list. See <https://github.com/magne/genealogy/issues/38>.

## Phase 3 — Persistence hardening ✅ done

- ✅ Implemented the **Postgres** backend behind the existing `PersistedEventRepository`/`Store`
  abstraction (the `postgres` feature; `postgres-es` + `sqlx`). `genealogy-cli` compiles **both**
  backends, so one binary selects the engine **per workspace at runtime** from each workspace's
  `database_url` (ADR 0002). A workspace is created on Postgres with `genealogy init --database-url
  postgres://…`, or by setting `[defaults].database_url` in the global config (precedence: flag >
  config > the `engine` default). Postgres is exercised in CI against a containerized server
  (`test-containers-util`/`testcontainers`, image `postgres:18-alpine`), each test isolated in its
  own database.
- ✅ Promoted Spike B's rebuild into the **`genealogy rebuild`** maintenance command
  (`Workspace::rebuild_projections` → the engine-neutral `Store::rebuild_projections`).
- **Migration story.** The event log is immutable and append-only, so the migration model is:
  schema *evolution* is **additive events + upcasters** (ADR 0004 §4 / ADR 0010), and any
  read-model/projection-schema change is absorbed by **`genealogy rebuild`** — drop the projections
  and replay the log (with upcasters) into the freshly-created tables. Table DDL is idempotent
  `CREATE … IF NOT EXISTS`; there is deliberately **no in-place `ALTER` migration framework**
  (YAGNI — projections are disposable, the log is the source of truth). Migration concerns this does
  **not** yet cover — new-view/table schema changes, moving a workspace between databases or engines
  (e.g. SQLite → Postgres), and config migrations such as re-rendering `HumanId`s to a new format —
  are captured in [`docs/migration-considerations.md`](migration-considerations.md) for a future
  cycle (and a gating ADR before the work).
- **Snapshotting** remains out of scope — ADR 0004 defers it until replay cost is *measured* to
  warrant it.

## Phase 4 — Import/export breadth (all as WASM plugins) ✅ done

Built out on the Spike C foundation, delivered as a sequence of PRs each landing with its gating ADR.
Both bullets below are complete; the work that originally sat here but is not import-format breadth
moved to later phases (see the note). Detailed history/remaining-work checklist:
[`docs/phase-4-followups.md`](archive/phase-4-followups.md).

- ✅ **Bulk import/export foundation.** The format-named `gedcom-import`/`gedcom-export` WIT worlds
  are generalized into format-neutral **`bulk-import`/`bulk-export`** worlds with a **`progress`**
  capability (step / processed / optional total) and **host-mediated streaming** source/sink (the
  host owns the path; the plugin streams a chunk at a time). Shared guest plumbing lives in a new
  **`genealogy-plugin-api`** crate; the GEDCOM plugins are migrated onto it; the CLI gains
  **`genealogy import`/`export`** commands that render progress. Host-API package → `@0.3.0`
  ([ADR 0013](adr/0013-import-export-contract.md)).
- ✅ **Gramps XML and GEDCOM 7 round-trip with idempotent re-import.** A pure `genealogy-gramps-xml`
  crate + `gramps-import`/`gramps-export` plugins; full **GEDCOM 7** round-trip (structured names,
  the complete date grammar, addresses, the civil/common event set, INDI attributes, associations,
  and owner-recoverable export); and `ExternalId`-based re-import **idempotency and deduplication**
  (resolve-or-create by `(authority, value)`, owner-gated record creation), so re-importing an
  identical file produces no new events (data-model §11). Host-API → `@0.8.0`. *(PRs #41/#45;
  [ADR 0013](adr/0013-import-export-contract.md), [ADR 0018](adr/0018-round-trip-owner-links-and-host-api-0.8.md).)*

> True merge / **sync** (reconciling divergent values, not just additive append) and the smaller
> round-trip gaps are research-quality concerns folded into **Phase 10**, not Phase 4. The
> Digitalarkivet assisted importer moved to **Phase 8**, and plugin signing/loading to **Phase 11**.
> Remaining-work detail: [`docs/phase-4-followups.md`](archive/phase-4-followups.md).

## Phase 5 — UI breadth ✅ done

Delivered across PRs #24–#45 (draft-based creates, unified record form, correction model,
structured dates, common-tab/table parity, a11y/CSS parity, DNA-cited inference, person timeline).
Residue: plugin-UI vocabulary *extensions* (ADR 0022 out-of-scope) and DNA match views (→ Phase 12).

- ✅ Full screen inventory: CRUD for every entity, pedigree/tree views, citation/evidence editing,
  and the non-destructive merge UI. (DNA match views move to **Phase 12** with the rest of the DNA
  work.)
- ✅ A design system and the plugin-UI vocabulary (ADR 0012/0022; further vocabulary extensions
  are a documented follow-up).
- ✅ Second-framework readiness check: a new renderer must reuse `genealogy-ui` unchanged
  (ADR 0008), enforced by `crates/genealogy-ui/tests/framework_free.rs`.

## Phase 6 — Place map MVP (read-only point) ✅ done

A small near-term slice: show a Place's existing point coordinate on a **read-only** map. Deliberately
minimal — one marker, one fixed OpenStreetMap tile layer with attribution, a clean empty state, and
**no** editing, geometry-model change, or provider choice. It ships the first geographic visual early
and de-risks embedding a JS map library (Leaflet) in the WebKitGTK webview before the full geography
phase (Phase 9). **No gating ADR** — its only new behaviour (an outbound tile request) is noted, not a
contract. Plan: [`docs/archive/plans/place-map-mvp.md`](archive/plans/place-map-mvp.md); mockup the **Map** tab of
[`docs/mockups/place.html`](mockups/place.html).

✅ **Delivered** (branch `feat/place-map-mvp`): a read-only **Map** tab on the Place screen renders
one marker at the existing coordinate over OpenStreetMap raster tiles (Leaflet 1.9.4, vendored locally
so only tiles are fetched), with a dashed empty state when the place has no coordinate. A framework-free
`MapPointVm` on `PlaceDetail` (parsed from the existing `coordinates` DTO string) drives it; no core,
DTO, or event-log change. The marker/init runs via `document::eval`; SSR tests cover the container,
attribution, and empty state.

## Phase 7 — Configuration split & storage ✅ done

Pulled forward from the Phase 13 server/web prerequisite: separate the entangled configuration axes
now, while the config surface is small, and give config a storage seam so it can later live in a
database. Gated by **ADR 0015** (written in this cycle). Three scopes replace today's two-axis
`workspace.toml` + global-table entanglement (ADR 0005):

- **Workspace-functionality** — the dataset and how it behaves: `id_formats`, the operators list,
  privacy/`Restriction` rules, data-language metadata, the surety scheme. Shared; for a remote
  workspace this lives server-side with the data, identical for every client.
- **Operator / user** — the acting operator `Agent` identity and the per-user preferences that
  follow a user across clients. On a server this is per-authenticated-user.
- **Client / presentation** — how *this* CLI/GUI/web session presents the workspace: UI locale,
  theme, view preferences, keyboard shortcuts, and the endpoint (or local `database_url`) the client
  connects through. Local to the client.
- **Storage seam.** A `ConfigStore` abstraction so each scope reads/writes either a TOML file
  (embedded — today's `workspace.toml` + `~/.config/genealogy/config.toml`) or a database. This
  phase ships the split, the trait, and the **file** backend; the **database** backend (operator +
  presentation config, per authenticated user) is implemented in Phase 13 with the server, which
  owns authentication.

Why now: it is the prerequisite Phase 13 depends on, and separating the axes early unblocks the
Ease-of-use presentation-config items (env-var precedence, customizable shortcuts, theme/view prefs)
without re-touching a later-entangled config surface.

✅ **Delivered** (branch `feat/config-split-storage`): configuration is grouped by owner into the
three scopes behind a `ConfigStore` trait ([ADR 0015](adr/0015-configuration-split-and-storage.md)),
with a `FileConfigStore` backend over the two existing TOML files; the callers (workspace registry,
CLI open/import, the GUI startup, and the Preferences/theme/window/recent/plugin save paths) go
through it. The inverted env-var precedence is fixed: a workspace's configured `ui_language` now
outranks a bare `LANGUAGE`/`LANG`, and `GENEALOGY_LANGUAGE` outranks both (plain env < config <
`GENEALOGY_`-prefixed), via a pure resolver wired into every localizer-building site. The database
backend stays **Phase 13**; the on-disk layout is retained (a clean break was permitted but no
consumer needed one) and no new config fields were added (YAGNI).

## Phase 8 — Assisted import & external search (Digitalarkivet) ✅ done

Online, record-by-record assisted import — searching an external archive, resolving scans, and
turning a found record into low-confidence Software-agent assertions the user then reviews. Gated by
a new **ADR 0017** (assisted-import host capabilities), which fixes the host capabilities ADR 0011 §3
deferred and ADR 0013 left out of scope.

- **New host capabilities.** A `net` capability (outbound HTTP with a host allowlist, deny-by-default)
  to fetch source pages and resolve the scan-image URL chain; a `media-store` capability (the host
  writes downloaded bytes under the workspace `media/` dir, checksums them, returns a relative path
  \— the Media aggregate stays metadata-only); and a pluggable, **named, multi-provider `ai`** capability
  (config declares `[ai.providers.<name>]` entries, each `kind = "command"` or `"vision-api"`, with
  an `[ai].default`; no hardcoded provider).
- **`digitalarkivet-import` plugin** + a pure `genealogy-digitalarkivet` crate that parses census and
  churchbook pages and resolves the scan URL chain, consuming the `genealogy-import` fixtures (never
  reformat them). Flow: fetch source page → store scan → parse transcribed fields or AI-interpret the
  scan → import as low-confidence Person/Source/Citation/Media with an `ExternalId` back to the record
  URL.
- **Interactive present-and-confirm (GUI).** A host `present` capability shows the interpreted record
  **and the scan** for confirm/edit before import. It suspends on a frontend presenter and carries a
  typed, versioned assisted-import payload rendered by a **first-party `Tool::Import` wizard** in the
  Dioxus GUI (ADR 0008); `genealogy-ui` parses the payload — it is not the ADR 0022 plugin-UI
  vocabulary. The earlier sketch of a CLI rendering the image inline (kitty graphics / sixel) is
  **dropped** (owner decision, 2026-07-19); `present` stays frontend-neutral so a CLI presenter could
  be added later, but none ships in Phase 8 (ADR 0017, Out of scope).

✅ **Delivered** (branches/PRs #153–#160): four deny-by-default host capabilities land under
[ADR 0017](adr/0017-assisted-import-host-capabilities.md) (WIT `genealogy:host-api` 0.15.0 → 0.19.0)
— **`net`** (GET-only, HTTPS, an allowlist re-checked on every redirect hop, an honest non-crawler
User-Agent), **`media-store`** (SHA-256 checksums, path-safe writes under the workspace `media/` root,
path+checksum dedup), a config-declared multi-provider **`ai`** (client-scope `[ai.providers.<name>]`
entries, each `command` — argv, no shell — or `vision-api`, plus a reserved `plugin` kind), and a
suspending **`present`** carrying a typed, versioned assisted-import payload. On top of them: the
**`assisted-import` world** with a `Confidence::Low` provenance template; the pure
**`genealogy-digitalarkivet`** crate that parses census/church-book pages and resolves the scan-URL
chain over verbatim fixtures (HTML-first — the research doc found no anonymous public API); **crop
plumbing end-to-end** (`MediaRef.crop`/caption through app, DTO, and WIT, with the Gramps `<region>`
round-trip proven on import) plus a GUI crop tool, media viewer, and media-save dialog; the
first-party **`Tool::Import` wizard**; and the **`digitalarkivet-import` plugin** with an idempotent
end-to-end import test. Per the owner's decision (2026-07-19) the flow is **GUI-only** — the CLI
inline-scan sketch (kitty/sixel) is dropped and `present` stays frontend-neutral. Honest residuals:
the `ai` capability ships and is tested but the plugin does not yet invoke it (census HTML is
reliable; a gothic-transcription path is future work); church-book IIIF scans carry no permanent
image and degrade to a no-scan import; and the assisted session is screen-local, so navigating away
cancels the run. Plan: [`docs/archive/plans/assisted-import.md`](archive/plans/assisted-import.md).

## Phase 9 — Places: geography & temporal model ✅ done

Make places geographically and historically accurate. Sits after the `net` capability (Phase 8) so the
pluggable provider can geocode; the geometry model and view work depend on nothing new. Gated by
**ADR 0024** (place geometry & spatial storage), **ADR 0025** (geography view & pluggable map
provider), and **ADR 0026** (place succession & temporal resolution).

- **Geometry beyond a point** (ADR 0024): a typed `PlaceGeometry` — point, polygon, multi-polygon
  (islands / exclaves), line — over integer microdegrees, **dated and accumulating** (the 1801 and
  1900 boundaries coexist). The projection materialises WKB behind a SQLite R\*Tree for viewport
  queries; GeoJSON is the import/export interchange (closes the deferred `PLAC.MAP` round-trip gap).
- **Places change over time** (ADR 0026): a date-aware resolution rule selects the name / parent /
  geometry in effect at a date, and **succession links** (`Merged` / `Split` / `Absorbed` /
  `Elevated`) record identity changes — Aker + Kristiania → Oslo (1948), a county split — distinct
  from a rename (a dated name on the same aggregate). Names-over-time and jurisdiction-over-time
  already work today.
- **The geography view** (ADR 0025): a framework-free map view-model + a MapLibre GL JS renderer in
  the webview; place markers + event-at-place pins; a **time slider**; and **in-map editing**
  (drop/move a point, draw/edit polygons) that writes the same audited `GeometryAsserted` events
  through the existing change-set path. The **transitive place-hierarchy walk** (a `docs/issues.md`
  item) lands here. The map **provider** is a declarative presentation-config descriptor (client
  scope, Phase 7), with geocoding and a possible `map-provider` plugin over `net`.

Plan: [`docs/plans/places-geography-temporal.md`](plans/places-geography-temporal.md); mockup
[`docs/mockups/geography.html`](mockups/geography.html).

✅ **Delivered** (stacked branches `feat/place-geometry-storage` → `feat/place-succession-temporal` →
`feat/geography-view`). Three slices, one per gating ADR:

- **Geometry & spatial storage (ADR 0024).** A typed `PlaceGeometry` (`Point`/`Polygon`) over integer
  `Microdegrees` in `genealogy-core`, asserted **dated and accumulating** via
  `AssertGeometry`/`GeometryAsserted` (the old undated `CoordinatesAsserted` folds as the `Point` case).
  `genealogy-db` materialises geometry as **WKB behind a SQLite R\*Tree** with a `places_in_bbox` query,
  rebuildable by `genealogy rebuild`. GeoJSON is the interchange (permissive GeoRust crates:
  `geo-types`, `geozero` with `with-geo`/`with-gpkg`, `geojson`); GEDCOM `PLAC.MAP` / Gramps `<coord>`
  points round-trip. `LineString`/`Multi*` variants and the Postgres GiST mirror stay additive
  follow-ups.
- **Succession & temporal resolution (ADR 0026).** A pure **effective-from** resolver (latest dated
  assertion ≤ target, else the undated/primary) selects name / enclosing parent / geometry as of a
  date — one rule shared by the generated title, the **transitive, cycle-aware, date-aware
  place-hierarchy walk** (`genealogy-app/src/place.rs`, the old `docs/issues.md` item), and the time
  slider. `AssertSuccession`/`SuccessionAsserted { from, to, kind, date }`
  (`Merged`/`Split`/`Absorbed`/`Elevated`/`Renamed`, modelled like Person `AssociationAsserted`) records
  identity change, projected as a symmetric predecessor/successor relation with the aggregate-tax
  existence check; a plain rename stays a dated `PlaceName` on the same aggregate. Explicit
  `[from, until)` validity intervals remain the documented additive follow-up.
- **Geography view, editing & provider (ADR 0025).** A framework-free `GeographyVm` in `genealogy-ui`
  (markers, event-at-place pins, viewport, selected year, provider descriptor) and a **MapLibre GL JS
  5.24.0** component (vendored) in `genealogy-ui-dioxus` behind a persistent `document::eval`
  click-stream. In-map editing (drop/move a point, draw polygon vertices, "new place here") emits the
  picked geometry through the **existing** `PlaceEdit`/`PlaceChangeSetRequest` change-set path — same
  audited `GeometryAsserted` event, no separate map-write path. A time slider resolves as-of a year via
  `show_place_as_of`, and the map **provider** is a declarative `[map]` client/presentation-config
  descriptor. Research: [`docs/research/geography-rendering.md`](research/geography-rendering.md).
  Honest residuals (scoped follow-ups, not blockers): true mouse-drag / mid-ring vertex insertion,
  pin-click selection, polygon-drawn place creation, the `maplibre-style`/`google` provider sub-forms,
  viewport-scoped `places_in_bbox` loading, defaulting the edit date to the slider year, dated
  `add_place_name`/`assert_place_enclosed_by` use-cases, and the `map-provider` plugin world +
  geocoding (ADR 0025 §4).

## Phase 10 — Research rigor & import sync

The evidence/conclusion model's research-quality layer (all data-model §17): make the surety scheme
configurable, add an explicit proof-argument aggregate, and complete import beyond additive append.

- **Configurable surety scheme** (data-model §17) — the fixed five-level `Confidence` ships first; a
  gating ADR precedes making it configurable.
- **`ResearchNote`/`Argument` aggregate** for proof arguments (data-model §17) — recording the
  reasoning that ties evidence to a conclusion.
- **Import true merge / sync** — re-import is additive-only today (an identical value is a no-op, a
  new value is added, a *conflicting* single-valued fact is left untouched). True merge reconciles
  divergent values, never overriding a fact asserted *after* the file's export date (its HEAD
  `1 DATE`). *(Deferred from Phase 4; [`docs/phase-4-followups.md`](archive/phase-4-followups.md).)*
- **Remaining round-trip gaps** (data-model §17): GEDCOM `REPO` records/pointer, `FAM`-level
  `SOUR`/`OBJE`/`NOTE`, place `MAP`/coordinates, multi-`NAME`, `FAMS`/`FAMC` back-refs, event-level
  witnesses, `SUBM`, media `FORM`, citation `CALN`, and Gramps `<tagref>` on the person/family record.

## Phase 11 — 1.0 hardening

- Plugin **signing, trust tiers, capability-grant UX, and three-layer loading** (workspace > app-dir
  > embedded), mirroring the ADR 0003/0005 override model. *(**ADR 0014**; moved here from Phase 4.)*
- Performance profiling.
- Packaging and distribution.

## Phase 12 — DNA breadth & depth

All DNA-specific functionality, pulled together so the match model and its views land as one cohesive
slice (data-model §17).

- **DNA match views** in the UI (moved from Phase 5).
- **DNA depth:** Y/mtDNA markers, haplogroup detail, and triangulation groups (data-model §17).

> Moved up: localized date/number formatting and the second-locale completeness checker were
> originally end-stage items; they now land in **Spike A** so catalogues stay complete from the
> first date-bearing aggregate onward.

## Phase 13 — Beyond 1.0: server backend + web frontend

Direction set by the project owner; sketched here so the 1.0 architecture stays compatible, **not**
scheduled. After 1.0 the app gains a third deployment shape alongside today's embedded
(workspace = local directory + database):

1. **Backend server.** Run `genealogy-app` as a long-lived server process that owns one or more
   workspaces and exposes the existing use-cases over the network. This is an additive frontend over
   the same coordination layer (ADR 0006) — the server re-exposes use-cases and DTOs; it does not
   re-implement domain rules. The `Session` (clock + UUID v7 + operator `Agent`) stays the impure
   boundary, now resolving the operator from an **authenticated** principal (the direction ADR 0005
   already fixed: operator → authenticated user, operator aggregate in the event store).
2. **Web frontend.** A browser client over the server, reusing `genealogy-ui` view-models and
   intents unchanged (ADR 0008's promise: a second renderer reuses `genealogy-ui` as-is; the ordered
   steps are in [`second-renderer-checklist.md`](second-renderer-checklist.md), and the framework-free
   guard in `crates/genealogy-ui/tests/framework_free.rs` keeps the boundary honest). The web
   renderer is a new crate parallel to `genealogy-ui-dioxus` — Dioxus already targets web, so this
   may be a web target of the same renderer or a sibling, decided when built.
3. **Server-connected workspaces.** `genealogy init`/the GUI gains the ability to register a
   workspace that points at a **server endpoint** instead of a local `database_url`. The CLI/GUI
   then act as **clients**: use-case calls travel to the server rather than to a local event store.
   The `PersistedEventRepository`/`Store` trait (ADR 0002) is the natural seam — a remote
   transport becomes another implementation, or the seam moves up to a use-case transport,
   decided in the gating ADR.

**Configuration storage (builds on Phase 7).** The split into workspace-functionality / operator /
client-presentation scopes and the `ConfigStore` seam land in **Phase 7**. The server adds the seam's
**database** backend: the operator and client/presentation scopes persist **per authenticated
principal** server-side (the workspace-functionality scope already lives with the data), so a browser
client carries no local config file. The embedded build keeps the file backend unchanged.

> These three pieces are deliberately additive: the embedded build keeps working unchanged, the
> server is a new frontend over `genealogy-app`, and the web client reuses `genealogy-ui`.

## Risk register

Each frontier unknown maps to the spike that kills it.

| Unknown | Killed by | Status |
| --- | --- | --- |
| Cross-aggregate id refs + projection invariant checks ("aggregate tax") | Spike A | ✅ Done |
| Dangling `EventContext.citations` (no Citation aggregate) | Spike A | ✅ Done |
| Localized dates + second-locale catalogue drift | Spike A | ✅ Done |
| Event-version upcasting (schema evolution) | Spike B | ✅ Done |
| Projection rebuild from the log | Spike B | ✅ Done |
| WASM plugin host (Wasmtime, WIT world, capabilities) | Spike C | ✅ Done |
| Import/export round-trip as plugins; Software-agent provenance | Spike C | ✅ Done |
| Framework-agnostic UI split; plugin-UI vocabulary | Spike D | ✅ Done |
| Postgres backend / per-workspace engine selection | Phase 3 | ✅ Done |
| Non-destructive merge (`PersonsMerged`) | Phase 2 | ✅ Done |

## New ADRs required

The existing ADRs deliberately deferred several decisions (their "out of scope" sections). Those
decisions must be **made as ADRs** before the phases that depend on them — the roadmap cannot land a
spike on an undecided contract. Proposed numbers are sequential and ordered by when each is needed;
they are confirmed when the ADR is written.

| Proposed | Decision to make | Gates | Deferred by |
| --- | --- | --- | --- |
| [ADR 0009](adr/0009-read-model-and-projection-schema.md) — **accepted** | Concrete read-model / projection schema | Spike A | ADR 0002, 0004 |
| [ADR 0010](adr/0010-event-version-upcasting-and-projection-rebuild.md) — **accepted** | Event-version upcasting mechanism + projection rebuild | Spike B | ADR 0002, 0004 §4 |
| [ADR 0011](adr/0011-plugin-host-wit-world-and-capabilities.md) — **accepted** | Plugin host WIT world versioning + capability-grant model + resource limits | Spike C | ADR 0007 |
| [ADR 0012](adr/0012-plugin-ui-vocabulary-schema.md) — **accepted** | Plugin-UI vocabulary schema (the named ADR 0007 follow-up) | Spike D | ADR 0007, 0008 |
| [ADR 0013](adr/0013-import-export-contract.md) — **accepted** | Import/export contract: bulk worlds + streaming I/O + progress; mapping strategy (GEDCOM 7 / Gramps XML, ExternalId dedup) | Phase 4 | data-model §16–17 |
| [ADR 0015](adr/0015-configuration-split-and-storage.md) — **accepted** | Config split: workspace-functionality vs operator vs client/presentation config, and the file/DB storage seam | Phase 7 | ADR 0005 |
| [ADR 0017](adr/0017-assisted-import-host-capabilities.md) — **accepted** | Assisted-import host capabilities (`net` fetch, `media-store`, pluggable `ai`, `present`) — the Digitalarkivet importer | Phase 8 | ADR 0007, 0011 |
| [ADR 0024](adr/0024-place-geometry-and-spatial-storage.md) — **accepted** | Place geometry (point/polygon/multi-polygon), the event-log encoding, the SQLite R\*Tree projection index, and the GeoJSON interchange | Phase 9 | ADR 0002, 0004, 0009 |
| [ADR 0025](adr/0025-geography-view-and-pluggable-map-provider.md) — **accepted** | Geography view rendering, in-map editing, and the pluggable map provider | Phase 9 | ADR 0008, 0024 |
| [ADR 0026](adr/0026-place-succession-and-temporal-resolution.md) — **accepted** | Place succession (merge/split) + the date-aware resolution rule | Phase 9 | ADR 0004, 0024 |
| ADR 0014 | Plugin signing, trust tiers, and distribution (and three-layer loading) | Phase 11 | ADR 0007 |
| ADR 0016 | Server backend + web frontend + server-connected workspaces (transport, auth) | Phase 13 | ADR 0002, 0005, 0006, 0008 |

Conditional — write an ADR only if/when the option is adopted (direction already fixed, so not
blocking):

- **Snapshotting** (Phase 3) — only if replay cost is measured to warrant it (ADR 0004 defers until
  measured).
- **Configurable surety scheme** (Phase 10) — data-model §17; the fixed five-level `Confidence` ships
  first.
- **DB-backed operator aggregate, authentication, record signing** (Phase 11–13) — ADR 0005 fixed the
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
