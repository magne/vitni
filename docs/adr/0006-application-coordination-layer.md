# 6. Application coordination layer (`genealogy-app`)

- **Status:** Accepted
- **Date:** 2026-06-18

## Context

The decision core is pure by contract (ADR 0004 §3): `decide(state, command, meta)` reads no
clock and generates no id. Everything non-deterministic — the clock (`occurred_at`), the
generated `AssertionId` and aggregate ids, and the operator `Agent` — must be produced
*outside* the core and passed in via `meta`. Likewise, `genealogy-db` exposes a generic event
store keyed by aggregate type; turning an operator's intent into a stored, projected fact
requires building an `AssertionMeta`, generating ids, resolving and opening a workspace,
allocating a user-facing `human_id`, and mapping framework errors back to a domain result.

This coordination has to live somewhere. Putting it in `genealogy-cli` would force the planned
native UI and web backend (CLAUDE.md) to re-implement it, splitting the one place
non-determinism enters across every frontend and inviting drift in how provenance is stamped.

`genealogy-db` complements this by exposing a single engine-neutral `Store` (opened from a
`database_url`, ADR 0002); the application owns the workspace **directory and manifest** (ADR 0005)
and resolves which `database_url` to open, while the database engine stays entirely inside
`genealogy-db`.

## Decision

Introduce a **`genealogy-app`** crate between `genealogy-core` / `genealogy-db` and every
frontend. It owns:

1. **The impure inputs.** A `Session` is the single place the clock is read and UUID v7 ids
   are generated; it builds `AssertionMeta` and new aggregate ids and carries the operator
   `Agent` resolved from config. Keeping this tiny and isolated makes the purity boundary of
   ADR 0004 §3 visible and auditable.
2. **Configuration and workspace lifecycle.** Loading the global config (operator + workspace
   registry), and creating/opening **workspace directories** with their manifest, database, and
   `exports/ backups/ media/` (ADR 0005). The engine-neutral store comes from `genealogy-db`
   ([`Store::open(database_url)`]); the app resolves the manifest's `database_url` and never names a
   database engine itself.
3. **Use-cases.** Coarse operations (`create_person`, `add_name`, `show_person`,
   `list_persons`, …) that build commands, execute them through the `genealogy-db` `Store`, query the
   read model, and return frontend-neutral DTOs. Domain and infrastructure types
   (`PersonView`, cqrs-es, sqlx) do not leak past this boundary; frontends get plain data.

Frontends (`genealogy-cli` today; UI and web later) are thin I/O over these use-cases.

**Backend features forward through the app.** `genealogy-app` declares `sqlite` (default) and
`postgres` features that re-export `genealogy-db`'s, so the top-level binary selects the
engine set and the choice stays a build-time decision per ADR 0002.

## Rationale

- The coordination is **required regardless** of how many frontends exist — it is not a
  speculative abstraction. What is anticipatory is only its *sharing*, and the multi-frontend
  intent is already recorded in CLAUDE.md, so a thin shared crate is the honest YAGNI line:
  one `Cargo.toml`, no premature generality.
- A crate boundary (not a `cli` module) **enforces** the layering: the CLI cannot reach into
  core internals or the store directly; it must go through the use-cases, which is exactly the
  contract the UI and web backend will depend on.

## Consequences

### Positive

- One audited place where non-determinism enters the system; the core stays pure and unit-
  testable, and provenance is stamped identically for every frontend.
- Frontends are thin and uniform; adding the UI or web backend reuses the use-cases verbatim.
- DTO boundary keeps cqrs-es / sqlx / `PersonView` details out of frontends, preserving the
  portability commitment of ADR 0002 §6.

### Negative / costs

- One more crate to build and version.
- Use-case DTOs partially restate read-model fields; the duplication is the price of not
  leaking infrastructure types.

## Out of scope

- The concrete use-case set beyond the Person slice (added per aggregate, same shape).
- Long-lived application state / caching across commands (each CLI invocation opens fresh).
- Async runtime policy beyond what cqrs-es requires.

## References

- ADR 0002 — framework-agnostic core, per-workspace engine selection, feature gating.
- ADR 0004 §3 — the pure decision core and the supplied non-deterministic inputs this crate
  produces.
- ADR 0005 — the configuration and workspace-resolution model this crate implements.
