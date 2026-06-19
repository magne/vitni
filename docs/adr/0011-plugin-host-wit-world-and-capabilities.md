# 11. Plugin host WIT world, capability-grant model, and resource limits

- **Status:** Accepted
- **Date:** 2026-06-19

## Context

ADR 0007 adopted a WebAssembly **component** plugin system (Wasmtime + the
Component Model + WIT) and fixed its shape: a versioned host-API world, plugins
that pin the host-interface version, deny-by-default capabilities, Software-agent
provenance, and three-layer loading. It deliberately left three contracts "out
of scope" — *the concrete WIT schemas, the capability-grant model, and resource
limits* — to be decided when the host was actually built.

Spike C (`docs/roadmap.md`) is the slice that builds the host and proves it with
a real GEDCOM import/export round-trip; it is the work that informs this
decision. The roadmap gates Spike C on this ADR ("Plugin host WIT world
versioning + capability-grant model + resource limits"), to be written in the
same cycle so the decision is grounded in working code rather than speculation.

This ADR fixes the three deferred contracts at the granularity the spike needs
and no further. It does **not** restate ADR 0007; it makes concrete what ADR
0007 named. Like the host crate, it sits above the `genealogy-app` DTO boundary
(ADR 0006) and reuses the provenance model (ADR 0001/0004) for Software agents.

## Decision

1. **One versioned host-API package; per-role worlds within it.** The host
   interfaces live in a single semver-versioned WIT package,
   `genealogy:host-api@0.1.0`. Within it, capabilities are separate
   **interfaces** (`log`, `query`, `commands`) and each plugin role is a
   **world** that imports exactly the capability interfaces it needs and exports
   one entry point (`gedcom-import` exports `run-import`; `gedcom-export` exports
   `run-export`). A plugin pins the **host-API package version** (ADR 0007 §2),
   never the application version; the host instantiates against the matching
   world and keeps older worlds available. Host-API evolution is **additive**:
   new interfaces, new functions, or new optional record fields bump the minor
   version; an incompatible change bumps the major version and is a new world the
   host instantiates alongside the old one (the Zed model). WIT records/variants
   mirror the `genealogy-app` DTOs (ADR 0006); no `cqrs-es`/`sqlx`/`PersonView`
   type crosses the boundary.

2. **Capabilities are deny-by-default, enforced per instance by a grant set.**
   Every host capability interface is **always linked** into the component, but
   each host-side implementation consults the instance's **grant set** before
   doing any work and returns a `denied` error variant when the requested
   capability was not granted. The grant set starts **empty**; the frontend
   opts a plugin into exactly the capabilities its metadata declares (ADR 0007
   §6, §8). Gating in the implementation — rather than by omitting the import —
   is chosen so a denied call is an observable, testable `result::err(denied)`
   the guest can report, not an instantiation failure. A plugin is **never
   auto-granted** a capability it did not declare.

3. **`files` and `net` are denied by construction in the spike via an empty
   WASI context.** The host links WASI Preview 2 with a `WasiCtx` that has **no
   preopened directories and no socket access**, so the filesystem and network
   capabilities ADR 0007 §6 names are denied without additional policy code.
   The spike's GEDCOM data crosses the boundary as **bytes in / bytes out**
   through the `run-import`/`run-export` signatures, so no `files`/`net` grant is
   needed to prove the round-trip. Real `files` (a scoped WASI preopened
   directory) and `net` (`wasi:http` with a host allowlist) are deferred to the
   import/export breadth phase (roadmap Phase 4).

4. **Resource limits: fuel for the timeout guard, a memory cap via store
   limits.** Every instance runs with **fuel metering** enabled
   (`Config::consume_fuel`) and a per-instance fuel budget; a runaway guest
   exhausts its fuel and traps, which the host maps to a typed limit error. Fuel
   is chosen for the spike because it is **deterministic and testable** without a
   background timer thread. Linear-memory growth is bounded by a Wasmtime
   `StoreLimits` memory cap (`ResourceLimiter`). **Epoch-based interruption**
   (`Config::epoch_interruption` with a host ticker) is the production
   wall-clock-timeout mechanism and is named here as the intended successor/
   complement to fuel; it is not wired in the spike.

5. **Capability host functions run through the existing app boundary.** The
   `commands` interface submits domain commands through `genealogy-app`
   use-cases driven by a `Session` whose operator is
   `AgentKind::Software { name, version }` (ADR 0004 §1, ADR 0007 §7), so every
   plugin-authored change is audited as a Software operator through the unchanged
   `decide()` path. The `query` interface returns the same frontend-neutral DTOs
   the use-cases return. The `log` interface forwards structured records to host
   `tracing`. The host adds no new way to mutate state.

6. **Spike loading is directory-based; full three-layer override is deferred.**
   The host loads components from a known directory (the spike's stand-in for ADR
   0007 §4's *embedded* layer) by stable id. The three-layer override
   (workspace > app-dir > embedded), plugin **signing** and trust tiers (ADR 0007
   §9), and bundle metadata verification are **not** implemented here; they land
   with import/export breadth and distribution (roadmap Phase 4, ADR 0014). This
   keeps the spike to the thinnest slice that proves the host, the WIT/DTO
   boundary, capability gating, resource limits, and a real round-trip.

## Rationale

- **One package, per-role worlds (1).** A single versioned package keeps the
  host-API contract in one place and lets the version comparison ADR 0007 §2
  requires be unambiguous, while per-role worlds keep each plugin importing only
  what it needs — the smallest surface that still expresses deny-by-default at
  the world level.
- **Gate in the implementation, not by omission (2).** Omitting an unimported
  interface would make a denied capability an instantiation error, opaque to the
  guest and awkward to test. A `denied` result variant makes the policy
  observable and lets a plugin degrade gracefully, and it matches ADR 0007 §6's
  "granted explicitly" framing.
- **Empty `WasiCtx` for files/net (3).** It demonstrates deny-by-default for the
  two ambient capabilities for free, with no policy code, and the bytes-in/
  bytes-out boundary is enough to prove the round-trip — so the spike does not
  pay for WASI filesystem-handle wiring it does not yet need.
- **Fuel before epoch (4).** A fuel budget traps a runaway guest deterministically
  in a unit test, with no timer thread or flakiness; epoch interruption is the
  right wall-clock mechanism for production but adds a host ticker the spike's
  exit criteria do not require.
- **Reuse the app boundary (5).** Driving commands/queries through
  `genealogy-app` means plugins inherit the audit trail, the pure core, and the
  DTO boundary with no new machinery — exactly the contract ADR 0006/0007 set up.
- **Defer loading/signing (6).** The override order and signing are real, decided
  directions (ADR 0007 §4, §9) but are distribution concerns; building them now
  would not kill the host unknown the spike exists to kill.

## Consequences

### Positive

- The host-API contract is concrete and versioned: plugins compile against
  `genealogy:host-api@0.1.0` and evolution rules are fixed.
- Deny-by-default is enforced and **testable** — a capability call without its
  grant returns `denied`, and a runaway guest is stopped by fuel.
- Plugin-authored changes are audited as Software operators with no new code, and
  no storage/framework type leaks across the boundary.

### Negative / costs

- A versioned WIT world is an ongoing maintenance commitment (the Zed
  v0.1→v0.8 burden ADR 0007 already noted).
- Gating every capability call against the grant set is a small per-call check on
  the host side; negligible at the coarse-grained batch granularity (ADR 0007
  §12) but present.
- Fuel metering adds execution overhead and requires choosing a budget per
  operation; the budget is a tuning knob that will need revisiting as plugins do
  more work.

## Out of scope

- **The three-layer override order, plugin signing, trust tiers, and bundle
  verification** — directions fixed by ADR 0007 §4/§8/§9; implemented with
  distribution (roadmap Phase 4, ADR 0014).
- **`files` and `net` capabilities** — the WASI preopened-directory and
  `wasi:http` wiring; roadmap Phase 4.
- **Epoch-based interruption and a wall-clock timeout** — named as the production
  successor to fuel; not wired here.
- **The plugin-UI vocabulary** — the separate ADR 0007 follow-up gated by Spike D
  (proposed ADR 0012), unrelated to the host contract this ADR fixes.
- **GEDCOM mapping breadth** — the import/export mapping strategy (GEDCOM 7 /
  Gramps XML, `ExternalId` dedup) is proposed ADR 0013; this ADR fixes only the
  host contract the spike's minimal GEDCOM plugins run on.

## References

- ADR 0001 / 0004 — event sourcing and the pure `decide()` contract plugins emit
  through; `EventContext` provenance and the `AgentKind::Software` operator slot.
- ADR 0002 — engine-neutral `Store`; self-contained, versioned events.
- ADR 0006 — the `genealogy-app` use-cases + `Session` and the frontend-neutral
  DTOs the host's `query`/`commands` interfaces mirror and drive.
- ADR 0007 — the plugin system this ADR makes concrete: §2 versioned worlds, §4
  layered loading, §6 capabilities, §7 Software provenance, §8 bundle metadata,
  §9 signing, §12 coarse-grained boundary.
- `docs/roadmap.md` — Spike C (the slice this ADR gates) and the "New ADRs
  required" table.
- Wasmtime Component Model embedding (`wasmtime::component::bindgen!`), WASI
  Preview 2 capability model, fuel metering (`Config::consume_fuel`) and store
  resource limits (`StoreLimits`).
