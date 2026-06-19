# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

@~/.claude/standards-rust.md

## Project

A genealogy program in Rust, inspired by and based on the data model of
[Gramps](https://github.com/gramps-project/gramps) (targeting Gramps **v6**).
Differentiators from Gramps:

- **Event-sourced core.** State is derived by replaying a log of events, not
  mutated in place. Each event is grouped under an **aggregate** and carries an
  **event context** recording the **operator** (user) who caused it — the system
  is auditable by construction (who changed what, when, and why).
- **CLI first.** The shippable interface today is the `genealogy` binary. A
  native UI and a web app are planned later, so keep domain logic in
  `genealogy-core`, free of any CLI/UI concerns.

## Domain model & architecture

Two docs are the source of truth; read them before changing the core or its wiring.

- **`docs/data-model.md` — the domain vocabulary.** Entities, value objects, the
  command/event catalog, and the `EventContext` provenance envelope. The model is
  **evidence/conclusion**: the **event log is the assertion/evidence layer** (each
  event is a claim by an operator), and **projections are the conclusion layer**,
  shaped as the Gramps entities and rebuildable from the log. There are **12
  aggregates** — the 10 Gramps primaries (Person, Family, Event, Place, Source,
  Citation, Repository, Media, Note, Tag) plus `DnaTest` and `DnaMatch`.
- **`docs/adr/` — architecture decisions. ADRs are immutable**: never edit an
  accepted ADR; supersede it with a new one.
- **`docs/roadmap.md` (+ `roadmap.html`) — what to build next.** Risk-first de-risking
  spikes then breadth to a 1.0 vision; import/export are WASM plugins; flags six required
  follow-up ADRs (0009–0014). Spikes A–D are done; ADRs 0009–0012 accepted.

Binding invariants from the ADRs:

- **Pure decision core (ADR 0004 §3).** `decide(state, command) -> Result<Vec<Event>, Error>`
  — no clock, no id generation, no I/O. The non-deterministic inputs (clock
  `occurred_at`, `AssertionId`, new aggregate ids, operator `Agent`) are produced
  by the application layer and passed in.
- **Provenance in the payload (ADR 0004 §1).** Every event embeds its
  `EventContext` (operator, when, why, confidence, citations). cqrs-es
  `metadata: HashMap<String,String>` is ops/tracing only (correlation/trace/request/host).
- **Corrections by `AssertionId` (ADR 0004 §2).** Each assertion carries an
  `AssertionId` (UUID v7) in its payload; `AssertionRetracted`/`AssertionSuperseded`
  reference it by that id, never by `(aggregate, sequence)`.
- **Self-contained, versioned events (ADR 0002, 0004 §4).** Every identifier you
  might query by lives in the payload. Encoding is serde **internally-tagged JSON**;
  changes are **additive / append-only** so every historical event stays decodable.
  Aggregate ids and `AssertionId` are UUID v7.
- **Storage (ADR 0002).** `cqrs-es`; **SQLite is the default**, Postgres is
  feature-gated, and the engine is selected **per workspace at runtime** behind one
  `PersistedEventRepository` trait.
- **App model (ADR 0005, 0006).** A workspace is a **directory** with a `workspace.toml`
  manifest. Global config at `~/.config/genealogy/config.toml`: `[workspaces.<name>]`,
  `[operator]`, `[defaults]` (app-level, frozen at use), `[workspace-defaults]` (live
  fallback). Workspaces referenced by name. `genealogy-app` owns the impure inputs (the
  `Session` — sole place a clock is read and a UUID v7 minted), config + workspace
  lifecycle, and use-cases returning frontend-neutral DTOs.
- **UI framework (ADR 0008).** The GUI is **Dioxus** (MIT, RSX) behind a
  framework-agnostic presentation crate. Dependency direction is one-way:
  `genealogy-app → genealogy-ui → genealogy-ui-<framework>` — no `dioxus::` (or future
  `slint::`) type appears above the renderer crate. The app's own screens are
  per-framework view code over shared view-models; only **plugin** screens use the
  constrained, serializable UI vocabulary (the ADR 0007 follow-up) rendered by a
  per-framework interpreter. A second framework is additive — a new renderer crate that
  reuses `genealogy-ui` unchanged.

## Workspace layout

Cargo workspace; member crates live in `crates/*` and inherit shared package
metadata and lints from the root `Cargo.toml`. The WASM plugin **component**
crates under `plugins/*` are **excluded** from the workspace (they build only
for `wasm32-wasip2`); build them with `cargo xtask build-plugins` (ADR 0007, 0011).

| Crate                   | Role                                                                                                                                                                                                                                                                                                                          |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `genealogy-core`        | Domain model + event-sourcing engine (aggregates, events, event context/audit, projections). Pure logic — no I/O frontends, no user-facing strings.                                                                                                                                                                           |
| `genealogy-app`         | Application coordination layer (ADR 0006). Owns the impure inputs (clock, UUID v7 ids, operator `Agent`), config + workspace resolution (ADR 0005), and the use-cases frontends call. The only layer that reads a clock or generates an id; returns DTOs.                                                                     |
| `genealogy-cli`         | The `genealogy` binary. Interactive terminal frontend over `genealogy-app`; stdout/stderr are the interface. Commands: `init <name> <path>`, `person create/add-name/show/list` (workspace via `--workspace`/`GENEALOGY_WORKSPACE`). Localized — see i18n note.                                                               |
| `genealogy-import`      | *(planned)* Importers. Test fixtures under `crates/genealogy-import/tests/fixtures/` are verbatim Digitalarkivet captures — **never reformat them** (prek skips whitespace/EOF fixers there).                                                                                                                                 |
| `genealogy-db`          | Persistence. Owns everything database-related: initial table creation, schema migrations, and the event-store / projection storage backing `genealogy-core` (ADR 0002).                                                                                                                                                       |
| `genealogy-plugin-host` | WASM component plugin host (ADR 0007, 0011). Owns Wasmtime, loads/instantiates plugin components, wires the deny-by-default capability interfaces (`log`/`query`/`commands`) over one versioned WIT world, applies fuel + memory limits. Sits above `genealogy-app`; drives use-cases under an `AgentKind::Software` session. |
| `genealogy-gedcom`      | Pure GEDCOM parse/emit over a small intermediate model — the format logic of the GEDCOM plugins, free of WASM/host types so it is unit-tested via `--workspace`. The `plugins/gedcom-*` wasm glue depends on it.                                                                                                              |
| `genealogy-ui`          | *(planned)* Framework-agnostic presentation layer (ADR 0008). View-models derived from `genealogy-app` DTOs, screen/navigation state, intent dispatch to use-cases, Fluent resolution, and the plugin-UI vocabulary types. **No framework types.** Depends on `genealogy-app` only.                                           |
| `genealogy-ui-dioxus`   | *(planned)* Thin Dioxus renderer (ADR 0008). The GUI binary: binds view-models to RSX, routes events to `genealogy-ui` intents, hosts the vocabulary→widgets interpreter. Parallel to `genealogy-cli`; consumes `genealogy-app` through `genealogy-ui`.                                                                       |
| `xtask`                 | Repository task runner (`cargo xtask <cmd>`), not shipped. Home of project automation; today `i18n-check` (locale-catalogue completeness, also a prek hook + CI step). Aliased in `.cargo/config.toml`.                                                                                                                       |

When adding a frontend (native UI, web), it consumes `genealogy-app` (and through
it `genealogy-core`); it does not re-implement domain rules or coordination. A GUI
frontend goes through `genealogy-ui` (ADR 0008), never `genealogy-app` directly.

## Commands

```bash
cargo build --workspace                                              # build every crate
cargo run -p genealogy-cli                                           # run the `genealogy` binary
cargo nextest run --workspace --all-features --all-targets           # all tests (see note below)
cargo test -p genealogy-core <name>                                  # single test by name in one crate
cargo clippy --workspace --all-targets --all-features -- -D warnings # lint (zero warnings)
cargo fmt --all                                                      # format every crate
cargo deny check                                                     # advisories, licenses, bans
cargo xtask i18n-check                                               # locale catalogues complete vs `en`
cargo xtask build-plugins                                            # lint + build plugins/* → target/plugins (wasm32-wasip2)
prek run                                                             # run git hooks manually
```

> **Plugin components.** `plugins/*` are workspace-excluded `wasm32-wasip2`
> crates, so `--workspace` never builds or lints them. `cargo xtask build-plugins`
> is the only path that compiles them (clippy `-D warnings` + build → `target/plugins/<id>.wasm`);
> CI runs it before tests, and `genealogy-plugin-host`'s integration tests load
> the built components from there. The `wasm32-wasip2` target comes from
> `rust-toolchain.toml`.

> **Always pass `--workspace` / `--all`.** `Cargo.toml` sets
> `default-members = ["crates/genealogy-cli"]`, so any cargo command without
> `-p`/`--workspace` (`--all` for `fmt`) operates on the CLI crate only:
> - `cargo test` / `cargo nextest run` → ~27 of ~144 tests, skipping core/app/db.
> - `cargo clippy` → lints CLI + path-dep **lib** code, but not the `#[cfg(test)]`
>   targets of core/app/db, and never `xtask`.
> - `cargo build` → CLI + its dep libs, not other crates' test targets or `xtask`.
> - `cargo fmt` → formats the CLI crate only; `cargo fmt --all` covers all.
>
> `cargo deny check` (whole dep graph) and `cargo xtask …` (explicit) are
> unaffected. `nextest` is the local test runner; CI uses `cargo test` (nextest
> is not installed there) and runs doctests separately, which `--all-targets`
> and `nextest` do not.

## Conventions specific to this repo

- **Lints are deny-level guardrails, not suggestions.** The workspace denies
  `unwrap_used`, `panic`, `todo`, `unimplemented`, `exit`, `dbg_macro`, etc. Do
  not silence them with `#[allow(...)]` — `allow_attributes` is itself denied.
  Fix the code instead. `expect_used` is `warn`; justify any use.
- `print_stdout`/`print_stderr` are denied workspace-wide but **allowed in
  `genealogy-cli`** (its stdout/stderr is the UI). Domain code in
  `genealogy-core` must use `tracing`, never `print!`.
- The `[lints]` table can't both inherit and override, so `genealogy-cli`
  duplicates the workspace lint set with its print relaxations. Keep the two
  lists in sync when changing lints.
- Event-sourcing invariant: events are the source of truth and are append-only.
  Never edit derived/projected state directly — emit a new event so the audit
  trail (operator + context) stays complete.
- **Every user-facing string is localized (ADR 0003).** All frontend text —
  stdout/stderr, labels, prompts, errors mapped from core types — goes through Fluent
  (`fl!()`; CLI lookups in `genealogy-cli/src/i18n.rs`, catalogues under
  `crates/genealogy-cli/i18n/<lang>/`), never a hardcoded literal. Baseline is
  runtime-overridable (workspace > shared app > embedded). UI strings live in Rust and
  resolve via `fl!()` (ADR 0008) — never a framework's built-in i18n (Dioxus/Slint
  gettext). `genealogy-core` emits no user-facing strings — typed errors only, English
  `tracing` for developers.
- **License: workspace is `MIT OR Apache-2.0` (permissive). Keep it that way.** New
  dependencies must be permissive-compatible; `cargo deny check` enforces this. **Never
  copy Gramps (GPLv2+) source** — the Gramps-derived model is a clean-room
  reimplementation; copying its code would force a copyleft relicense (ADR 0008).
- **Presentation vs data localization are distinct.** ADR 0003 (Fluent/`i18n-embed`)
  is the *UI chrome*. The *data* language metadata — `LanguageTag`,
  `RichText.language`, `PlaceName`, `PersonName.transliterations` (data-model §14) —
  describes what language a *record* is in. They share only the BCP-47 vocabulary
  (`unic-langid`).

## MCP Tools: code-review-graph

A Tree-sitter knowledge graph (auto-updated on file changes) backs this repo. **Prefer
its tools over Grep/Glob/Read** for exploration, impact analysis, and review — they give
structural context (callers, dependents, tests) file scanning can't. Use
`semantic_search_nodes`/`query_graph` to find code and relations, `get_impact_radius`/
`get_affected_flows` for blast radius, `get_review_context` for risk-scored review,
`get_architecture_overview`/`refactor_tool` for structure and renames. Fall back to
Grep/Glob/Read when the graph doesn't cover the need.
