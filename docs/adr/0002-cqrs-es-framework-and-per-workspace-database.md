# 2. cqrs-es event store with per-workspace Postgres or SQLite

- **Status:** Accepted
- **Date:** 2026-06-17

## Context

ADR 0001 decided that the domain core is event-sourced, and deliberately left two
follow-up choices open: the event-store framework, and the database engine. This ADR
settles both.

The deciding factor is the deployment spread. One product must serve two very different
installs:

- a **server / multi-user** install backed by an enterprise-grade database, and
- a **local single-user** install that runs with no setup, easing UI deployment.

The application manages **workspaces** — each workspace is an independent genealogy
dataset — and may hold several open at once, including a local one and a shared server
one simultaneously. The database engine is therefore a **per-workspace property chosen at
runtime**, not a deploy-time switch: a binary that has more than one engine compiled in
decides per workspace. *Which* engines are compiled in is a build-time choice (see the
Cargo features in the Decision), so a lean local build can ship SQLite alone.

## Decision

1. **Framework: `cqrs-es`.** DDD aggregates; per-stream `(aggregate, sequence)` optimistic
   concurrency.
2. **Databases: Postgres and SQLite, gated by Cargo features; SQLite is the default.**
   Two features on `vitni-db` — `sqlite` (default) and `postgres` — gate the `sqlite-es`
   and `postgres-es` backends. The default build ships SQLite only (zero-setup local);
   `postgres` (or both) is opt-in for server / multi-user builds. Both backends implement the
   same cqrs-es `PersistedEventRepository` trait, so when more than one feature is enabled the
   `vitni-db` crate dispatches to the right one **per workspace at runtime** behind that
   single trait — no recompilation. Postgres serves server / multi-user / enterprise
   workspaces; SQLite serves embedded / zero-setup / single-user local workspaces.
3. **Portability commitment (research §6).** To keep a future move to `disintegrate` (the
   DCB model) a contained project rather than archaeology, we commit to:
   - a **framework-agnostic decision core** (`state + command -> events | error`), with the
     cqrs-es `Aggregate` impl as a thin adapter over it;
   - **self-contained events** — every identifier we might ever query by lives in the event
     payload, never implicit in a stream key (the one thing that cannot be retrofitted);
   - **plain JSON payloads** plus **explicit event versioning**.

## Rationale

Why `cqrs-es` over `disintegrate`:

- **The both-engines-in-one-binary requirement is decisive.** Per-workspace runtime
  selection means both backends must live in the same binary. `cqrs-es` already ships
  official, trait-compatible Postgres and SQLite backends (research §5), so this is a direct
  fit. `disintegrate` has only a Postgres backend; a SQLite backend would be a
  weekend-to-week DIY build (§5), and shipping without it cannot satisfy the requirement.
- **Maturity.** `cqrs-es` is the more battle-tested option, with more backends and examples
  (§3).
- **Accepted trade-off.** `cqrs-es`'s fixed aggregates make cross-entity invariants pay the
  "aggregate tax" — checking a second entity's invariant against a possibly-lagging
  projection rather than transactionally (§3). Genealogy invariants are largely
  within-aggregate; where they are not, we accept projection-based checks for now. The §6
  habits above keep the door open to `disintegrate` if cross-entity invariants later justify
  the switch.

## Consequences

### Positive

- The default build is lean — SQLite only, zero setup, no Postgres dependency tree.
- A build with both features serves server and local workspaces from one binary; the engine
  is chosen per workspace at runtime.
- Backend selection is isolated behind a single `PersistedEventRepository` trait, so engine
  differences do not leak into domain code.

### Negative / costs

- **SQLite is single-writer** — no concurrent write throughput (acceptable for local
  single-user workspaces).
- **`sqlite-es` has low adoption** (~172 recent downloads) — pin it and skim its source.
- **Enabling the `postgres` feature compiles its dependency tree** on top of SQLite, growing
  build time and binary size; the default SQLite-only build avoids it.
- **Cross-entity invariants rely on lagging projections** (the aggregate tax), not
  transactional checks.
- **The §6 portability discipline is an ongoing cost** — every event must carry its
  identifiers and be versioned, even when the current code path does not need it.

## Out of scope

- Concrete projection / read-model schema.
- Migration and event-version upcasting tooling.
- The eventual `disintegrate`-migration ADR, if and when cross-entity invariants trigger it.

## References

- ADR 0001 — use event sourcing for the domain core (this ADR fills the framework + engine
  it deferred).
- `docs/research/event-sourcing-rust.md` — §3 (`cqrs-es` vs `disintegrate`), §5 (SQLite
  support and the dual trait-compatible backends), §6 (portability habits). Read against
  `cqrs-es` 0.5 / `sqlite-es` 0.5; exact versions are resolved at `cargo add` time.
