# 7. Plugin system via WebAssembly components

- **Status:** Accepted
- **Date:** 2026-06-18

## Context

The shippable surface today is the `vitni` CLI over `vitni-app` (ADR 0006). Whole
categories of feature — import and export (GEDCOM first), reports, analysis, integrity
checking and repair, integration with online archives and search engines — should be added
and shipped *independently* of the core, by us and by third parties, without forcing changes
to `vitni-core` or risking its invariants.

Third-party extension code is untrusted, so it must run in a **sandbox** and reach the rest
of the system only through capabilities it is explicitly granted. It must also respect the
event-sourced contract: a plugin that changes data does so by emitting events through the
same pure `decide()` path (ADR 0004), with provenance recorded — never by mutating
projections. Plugins also need to be localizable (ADR 0003) and distributable as
self-describing, verifiable artifacts.

## Decision

Adopt a **WebAssembly component plugin system** built on **Wasmtime + the Component Model**,
with interfaces defined in **WIT**.

1. **Runtime & ABI: Wasmtime, Component Model, WIT.** Plugins are WASM *components*
   (`wasm32-wasip2`), not bare modules. The host defines typed interfaces in WIT and
   generates Rust bindings with `bindgen!`. WIT records/variants map directly onto the
   domain's commands, events, and DTOs. The host is Rust-only, so Wasmtime's Rust-only
   embedding is not a constraint.

2. **Versioned host API worlds; plugins pin the host-interface version, not the app version.**
   The host API is a semver-versioned set of WIT worlds. A plugin declares the **host-interface
   version** it targets (e.g. `host-api 0.3`); the host instantiates against the matching world
   and keeps older plugins running (the Zed model). This decouples plugins from the app's
   release version — the app version is an implementation detail behind a stable, versioned WIT
   contract. Host-API evolution is additive wherever possible.

3. **Stable plugin identity and version-based override.** Every plugin carries a **stable id**
   (publisher-namespaced, e.g. `vitni.gedcom-import`) plus a semver **version**. The id —
   not the file path or load order — is the plugin's identity. When the same id appears in more
   than one source, the host resolves a single active plugin, letting a newer version override
   an older one. This is what lets a bundled plugin be updated out-of-band (ship a newer
   `vitni.gedcom-import` without a new app build).

4. **Plugins load from three layered sources** (mirroring the ADR 0003 / ADR 0005 override
   order, highest precedence first):
   - **workspace** — `<workspace-dir>/plugins/` (per-dataset extensions/overrides),
   - **app-wide** — `~/.config/vitni/plugins/` (user-installed, all workspaces),
   - **embedded** — built-in plugins shipped with the binary (the base set, e.g. GEDCOM).

   Resolution is by stable id: a higher layer overrides a lower one for the same id (normally
   to supply a newer version), exactly as the i18n `AssetsMultiplexor` layers catalogues.

5. **The plugin host is a new impure crate above the app layer.** A `vitni-plugin-host`
   crate owns Wasmtime, component loading/instantiation, capability wiring, and bundle
   handling. It sits *above* `vitni-app` because plugins read views (via the
   `vitni-db` `Store`) and send commands (via `vitni-app` use-cases + `Session`).
   `vitni-core` never links Wasmtime and stays pure; frontends drive plugins through the
   host.

6. **Capabilities are deny-by-default.** A plugin can do nothing outside its sandbox until
   granted. Host interfaces a plugin may request, each granted explicitly:
   - **query** — read views/aggregates, returning the same frontend-neutral DTOs use-cases
     return (no `PersonView`/cqrs-es/sqlx leakage).
   - **commands** — submit domain commands through `vitni-app` use-cases.
   - **files** — a WASI Preview 2 *preopened directory* handle, scoped to a single
     workspace-relative directory (e.g. `exports/`, `media/`); the handle confers access to
     that directory and nothing else.
   - **net** — outbound `wasi:http` restricted to a host allowlist (for online archives).
   - **log** — structured `tracing` from the guest.

   Resource limits (memory pages, fuel/epoch-based timeout) are applied to every instance.

7. **A plugin is a Software operator in the provenance model.** Commands a plugin emits carry
   `AgentKind::Software { name, version }` in their `EventContext` (ADR 0004 §1). They go
   through the unchanged `decide()` → events path, so the audit trail records *which plugin
   version asserted what*, for free.

8. **Bundle format and metadata.** A bundle is an archive carrying bundle metadata (bundle
   version, publisher, list of contained plugins) plus the plugins. Each plugin's metadata
   declares: its **stable id** and **version**, name/description, the **required host-interface
   version** (the WIT world it targets, §2 — *not* an app version), and the **set of required
   host interfaces** (the capabilities of §6 it requests). Each plugin ships its `.wasm`
   component and its i18n `.ftl` catalogues.

9. **Signing scoped to trust tier.** Bundled and sanctioned (publisher-trusted) plugins **must**
   be signed; the host verifies the signature before load and refuses a bad/missing signature
   for that tier. Unsanctioned third-party/local plugins may load unsigned but are treated as
   untrusted — never auto-granted sanctioned-only capabilities, and surfaced as unverified.
   Signing protects integrity/authenticity; it does not widen the sandbox (capabilities are
   still deny-by-default, §6).

10. **Plugin localization reuses ADR 0003.** A plugin's `.ftl` files are layered into the
    existing `i18n-embed` `AssetsMultiplexor` as a plugin-scoped layer, so plugin strings
    localize through the same Fluent path as the app, honoring the workspace > shared-app >
    embedded override order. `vitni-core` remains string-free.

11. **Base plugins ship on the same path as third-party plugins.** GEDCOM import/export (and
    future bundled reports/analysis) are WASM components loaded through this system, not native
    built-ins, and resolved from the embedded layer (§4). This dogfoods the sandbox and host
    API from day one and surfaces API gaps early.

12. **Coarse-grained boundary APIs.** Interfaces are designed to cross the host/guest boundary
    in batches (one call imports a file, generates a report, runs an analysis pass) rather than
    per-record, keeping boundary-copy cost negligible against per-call compute.

## Rationale

- **Proven for exactly this shape.** Zed extends a Rust desktop app via Wasmtime + Component
  Model + WIT with versioned worlds; Orbis does it for a genealogy-shaped desktop app. This is
  a mature 2025 pattern, not a research bet.
- **Performance is adequate by construction.** Plugin work here is coarse batch I/O, not
  latency-critical inner loops; ~1.6% execution overhead and microsecond instantiation are
  irrelevant at that granularity.
- **It strengthens, rather than bends, the existing model.** Capability security mirrors the
  audit-by-construction philosophy; the Software-agent provenance slot already exists; the
  pure core and DTO boundary (ADR 0006) are exactly the contract plugins consume.
- **WIT's type system matches the domain.** Commands, events, and DTOs are records/variants;
  expressing them as WIT keeps the boundary typed and the bindings generated, not hand-rolled.
- **Why Component Model over Extism.** Extism's bytes-in/bytes-out ABI is simpler and
  polyglot-host, but the host here is Rust-only and the domain is richly typed; WIT's typed
  interfaces and capability handles fit better and are the same tool Zed proves at scale.

## Consequences

### Positive

- The core stays stable while features ship and update independently, by us or third parties.
- Untrusted code runs sandboxed with deny-by-default, explicitly granted capabilities.
- Plugins can be authored in any language that targets the Component Model.
- Plugin-authored changes are fully audited as Software operators with no new machinery.
- Plugin localization and the DTO boundary reuse ADR 0003 / ADR 0006 verbatim.

### Negative / costs

- Wasmtime pulls in a substantial dependency tree, growing build time and binary size.
- Versioned WIT worlds are an ongoing maintenance commitment (the Zed v0.1→v0.8 burden).
- The component sandbox copies data across the boundary; very large no-copy data sharing is
  not a fit until Component Model async streams/shared heaps mature (Preview 3+).
- Signing, trust, and capability-grant policy are new infrastructure to design and operate.
- Bridging the async host runtime to guest calls adds wiring complexity
  (`add_to_linker_async`).

## Out of scope

- **Plugin-provided UI.** This ADR has plugins return structured result DTOs that frontends
  render. A declarative UI vocabulary (forms/lists/tables rendered natively by the host, the
  Orbis/Seed model) is deferred to its own ADR when the native/web frontend lands.
- Plugin distribution/marketplace service and update channels.
- Hot reload/unload of plugins at runtime.
- Inter-plugin composition (one plugin importing another's interface).
- The concrete WIT schemas, the signing-key management scheme, and the capability-grant UX.

## References

- ADR 0001 / 0004 — event sourcing and the pure `decide()` contract plugins emit through;
  `EventContext` provenance and the `AgentKind::Software` operator slot.
- ADR 0002 — engine-neutral `Store`; self-contained, versioned events.
- ADR 0003 — Fluent / `i18n-embed` `AssetsMultiplexor` layering reused for plugin catalogues.
- ADR 0005 / 0006 — workspace directories and the `vitni-app` use-cases + `Session` the
  host drives.
- Wasmtime Component Model embedding (`wasmtime::component`, `bindgen!`); WASI Preview 2
  capability model (preopened dirs, `wasi:http`).
- Zed WASM extension API (Wasmtime + Component Model + versioned WIT worlds); Orbis
  (WASM-sandboxed plugins for a Rust desktop app).
