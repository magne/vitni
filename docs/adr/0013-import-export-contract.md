# 13. Import/export contract: bulk worlds, streaming I/O, and progress

- **Status:** Accepted
- **Date:** 2026-06-22

## Context

ADR 0007 fixed import and export as WebAssembly **component** plugins, and ADR
0011 built the host that runs them — but only the thinnest slice Spike C needed: a
`gedcom-import` world that took a `list<u8>` and a `gedcom-export` world that
returned one. ADR 0011 §3 deliberately deferred real `files`/`net` and named the
broader mapping strategy (GEDCOM 7 / Gramps XML, `ExternalId` dedup) as a future
ADR (its "out of scope"). The roadmap gates Phase 4 — import/export breadth — on
that ADR.

Phase 4 adds more formats (GEDCOM 7, Gramps XML) and, separately, an assisted
single-record importer (Digitalarkivet). The two GEDCOM worlds do not generalize:
they are format-named, carry the whole document in a single `list<u8>` argument
or return value, and have no way to report how far a long operation has gotten.
Before adding a second and third bulk format, the host/plugin contract for **bulk
import/export** needs to be made format-neutral and given the I/O and progress
surface the breadth work (and a UI) requires.

This ADR fixes that contract. It does **not** restate ADR 0007 or 0011; it makes
concrete the import/export contract ADR 0011 named, and sits on the same host
(ADR 0011), DTO boundary (ADR 0006), and Software-agent provenance (ADR 0001/0004,
ADR 0007 §7). The assisted-import host capabilities (network fetch, media-file
storage, AI, interactive confirmation) are a separate, larger contract gated by a
later ADR; this ADR covers only the bulk path.

## Decision

1. **Format-neutral `bulk-import` / `bulk-export` worlds replace the GEDCOM
   worlds.** The host-API package bumps to `genealogy:host-api@0.3.0`. The
   `gedcom-import` / `gedcom-export` worlds are **removed** (the first world
   removal — they were spike-only and have no external plugins) and replaced by
   `bulk-import` (exports `run-import: func() -> result<u32, string>`) and
   `bulk-export` (exports `run-export: func() -> result<u32, string>`). The
   returned `u32` is the number of records imported/written. The format is no
   longer in the world name: GEDCOM, Gramps XML, and future formats all compile
   against the same two worlds. ADR 0011 §1's additive-evolution rule still holds
   for everything else; removing the spike worlds is a one-time documented break,
   not a new policy.

2. **The document streams through host-mediated source/sink capabilities; the host
   owns the path.** Two new capability interfaces carry the bytes instead of a
   `list<u8>` in the world signature:
   - `import-source` — `open()` readies the host-selected source; `read(len)`
     returns up to `len` bytes, an empty list signalling end of input.
   - `export-sink` — `open(suggested-name)` resolves and creates the destination
     (the host decides the real path; the guest only *proposes* a base name);
     `write(bytes)` appends; `finish()` flushes.
   One source and one sink back each instance — a plugin runs exactly one import
   **or** one export — so neither interface needs resource handles. The guest
   never names a real file: the frontend tells the host *which* file (a CLI path,
   a future UI file-picker), and the host streams it in or out. This keeps the
   capability deny-by-default and routes all filesystem naming through the host,
   rather than granting the guest an ambient WASI preopened directory.

3. **A `progress` capability reports coarse-grained progress and carries
   cancellation.** `report(step, processed, total)` lets a bulk operation report
   how far it has advanced; `total` is `option<u32>` because an importer often
   cannot know the record count until it has read the whole document. The host
   forwards each update to a frontend-supplied sink — the CLI prints a line to
   stderr; a future GUI updates a bar. The `step` string is the plugin's own
   vocabulary (e.g. `persons`, `families`), shown verbatim by the CLI for now.
   The report **returns a `control` value (`proceed` / `cancel`)**: each report is
   also a cancellation point, so the frontend can stop a long operation (a GUI
   cancel button, a CLI interrupt) without a separate capability. A guest must
   honor `cancel` by ending promptly and returning the count it has completed; for
   an import the records committed before the cancel remain (each is its own
   event), which the audit log reflects. The CLI's sink currently always returns
   `proceed` — the mechanism is wired end to end; triggering it from an interrupt
   is a follow-up.

4. **The new capabilities are deny-by-default, like every other.** `Progress`,
   `ImportSource`, and `ExportSink` are grants in the host's grant set (ADR 0011
   §2); a plugin that was not granted one gets `denied`. The host enforces the
   grant in the capability implementation, and the CLI grants a bulk plugin
   exactly the set its role needs (import: `commands` + `log` + `progress` +
   `import-source`; export: `query` + `log` + `progress` + `export-sink`). A
   per-plugin declared-capability manifest is deferred to ADR 0014.

5. **Shared guest support lives in a `genealogy-plugin-api` crate.** The format
   plugins repeat the same plumbing — drain the import source, write the export
   sink, report progress, log. That plumbing lives once in a guest-side library
   crate that generates the host-API **import** bindings (the `host-imports`
   world) and wraps them in helpers (`read_source_to_string`, `write_export`,
   `report`, `log_info`). Each plugin component maps the shared interfaces to this
   crate via wit-bindgen's `with` and only generates its own export. The crate is
   an rlib, not a component, so `cargo xtask build-plugins` skips it from
   component discovery (cargo builds it transitively as a dependency).

6. **Format logic stays in pure crates; the wasm glue stays thin.** As with
   `genealogy-gedcom` (ADR 0011), each format's parse/emit logic lives in a pure,
   workspace-tested crate; the `plugins/*` component is glue that bridges the
   format's intermediate model to the host capabilities. GEDCOM 7 breadth and a
   new `genealogy-gramps-xml` crate, plus `ExternalId`-based re-import
   idempotency, are built on this contract in the breadth work (see
   `docs/archive/phase-4-followups.md`); the mapping *strategy* — map external records to
   persona-level aggregates, carry the origin as an `ExternalId`, and resolve
   re-imports by `(authority, value)` against projections (data-model §11) — is
   fixed here, the per-format detail is implementation.

## Rationale

- **Format-neutral worlds (1).** Naming a world after one format does not scale to
  three; folding them into `bulk-import`/`bulk-export` means the host instantiates
  the same world for every format and the host code path is written once.
- **Host-mediated streaming, host owns the path (2).** Passing the whole document
  as a single `list<u8>` forces it through linear memory twice and caps the size
  at what the guest can allocate; a streamed source/sink does not. Letting the
  host own the path keeps filesystem access capability-gated and out of the
  guest's hands — the deny-by-default property ADR 0007 §6 wants — instead of
  granting a WASI preopened directory the guest could walk. A single implicit
  stream per instance matches the one-import-or-one-export reality and avoids the
  complexity of resource handles.
- **Optional `total` (3).** Forcing a total would make every importer pre-scan;
  letting it be absent lets a plugin report indeterminate progress and fill in a
  total once known.
- **One `plugin-api` crate (5).** The streaming/progress boilerplate is identical
  across plugins; sharing it via `with` mirrors how the host shares its own
  bindings (ADR 0011 §1) and keeps each new format plugin to its format logic.
- **Pure format crates (6).** Keeping parse/emit out of the wasm glue means the
  format logic is unit-tested through the normal `--workspace` path, never the
  slow component-build path, exactly as `genealogy-gedcom` already is.

## Consequences

### Positive

- One contract serves every bulk format; adding GEDCOM 7 or Gramps XML is a new
  pure crate plus thin glue, not a new world.
- Arbitrarily large documents stream through a chunked source/sink without being
  held whole in guest memory, and the host controls every path the guest touches.
- Long operations report progress to any frontend through one capability.
- The streaming/progress glue is written once in `genealogy-plugin-api`.

### Negative / costs

- Removing the GEDCOM worlds is a breaking change to the host-API package
  (`0.2.0` → `0.3.0`); acceptable because the worlds were spike-only with no
  external consumers, but it sets the precedent that pre-1.0 worlds may be removed
  with documentation rather than kept forever.
- Three more capabilities to grant and gate, and a small per-call grant check on
  each `read`/`write`/`report` (negligible at the coarse batch granularity).
- The single-stream-per-instance simplification means a future plugin that needs
  several files at once would require revisiting the source/sink shape.

## Out of scope

- **Assisted single-record import** — network fetch (`net`), media-library file
  storage, AI interpretation, and interactive present-and-confirm are a separate,
  larger host contract (the Digitalarkivet work), gated by a later ADR.
- **Plugin signing, trust tiers, and three-layer loading** — still ADR 0014. This
  ADR keeps ADR 0011 §6's directory loader (`load_by_id`).
- **`ExternalId` command/event wiring on the aggregates** — the dedup *strategy*
  is fixed here; the per-aggregate `AddExternalId`/`ExternalIdAdded` wiring is
  built in the breadth work.
- **Localizing the progress `step`** — steps are shown verbatim for now; treating
  them as Fluent message IDs (as the ui-panel does, ADR 0012) is a follow-up.

## References

- ADR 0001 / 0004 — event sourcing and the pure `decide()` path plugins emit
  through; `AgentKind::Software` provenance.
- ADR 0006 — the `genealogy-app` use-cases + DTOs the host capabilities mirror.
- ADR 0007 — the plugin system: §6 capabilities, §7 Software provenance, §9
  signing (deferred), §12 coarse-grained boundary.
- ADR 0011 — the host this ADR extends: §1 versioned worlds, §2 deny-by-default
  grants, §3 deferred files/net, §4 resource limits, §6 directory loading.
- `docs/data-model.md` §11 — `ExternalId` and re-import idempotency.
- `docs/roadmap.md` Phase 4 and `docs/archive/phase-4-followups.md` — the breadth work
  this contract carries.
