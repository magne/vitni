# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

@~/.claude/standards-rust.md

## Project

A genealogy program in Rust, based on the data model of
[Gramps](https://github.com/gramps-project/gramps) (targeting Gramps **v6**). What differs:

- **Event-sourced core.** State is derived by replaying a log of events, not mutated in place.
  Each event is grouped under an **aggregate** and carries an **event context** naming the
  **operator** who caused it — auditable by construction (who changed what, when, and why).
- **Two frontends over one core.** The `genealogy` CLI binary and the Dioxus GUI both sit on
  `genealogy-app` (a web app is planned); domain logic stays in `genealogy-core`.

## Domain model & architecture

These docs are the source of truth; read them before changing the core or its wiring.

- **`docs/data-model.md` — the domain vocabulary.** Entities, value objects, the
  command/event catalog, and the `EventContext` provenance envelope. The model is
  **evidence/conclusion**: the **event log is the assertion/evidence layer** (each event is a
  claim by an operator), and **projections are the conclusion layer**, shaped as the Gramps
  entities and rebuildable from the log. There are **13 aggregates** — the 10 Gramps primaries
  (Person, Family, Event, Place, Source, Citation, Repository, Media, Note, Tag) plus `DnaTest`,
  `DnaMatch`, and `ResearchNote` (ADR 0028); `for_each_aggregate!` in
  `crates/genealogy-app/src/aggregates.rs` is the registry that lists them.
- **`docs/adr/` — architecture decisions. ADRs are immutable**: never edit an accepted ADR;
  supersede it with a new one.
- **`docs/roadmap.md` (+ `roadmap.html`) owns phase/progress state** — read it rather than
  tracking progress here. **`docs/issues.md` is the backlog**; its *Decided — no action needed*
  section records deliberate non-tasks, so check there before "fixing" something. Completed work
  is in `docs/archive/completed-work.md`, the issue ↔ GitHub linkage (enforced by `cargo xtask
  issue-sync`) in `docs/issue-tracking.md`. Also `release.md`, `second-renderer-checklist.md`,
  `mockups/`, `research/`.

Binding invariants from the ADRs — the load-bearing part; open the ADR for the rest:

- **Pure decision core (ADR 0004 §3).** `decide(state, command) -> Result<Vec<Event>, Error>`
  — no clock, no id generation, no I/O. Every non-deterministic input (`occurred_at`,
  `AssertionId`, new aggregate ids, operator `Agent`) is produced by the app layer and passed in.
- **Provenance in the payload (ADR 0004 §1).** Every event embeds its `EventContext` (operator,
  when, why, confidence, citations). cqrs-es `metadata` is ops/tracing only.
- **Corrections by `AssertionId` (ADR 0004 §2).** `AssertionRetracted`/`AssertionSuperseded`
  reference the assertion by its `AssertionId`, never by `(aggregate, sequence)`.
- **Self-contained, versioned events (ADR 0002, 0004 §4).** Every identifier you might query by
  lives in the payload; encoding is serde **internally-tagged JSON** and changes are **additive /
  append-only**, so every historical event stays decodable. Aggregate ids and `AssertionId` are
  UUID v7.
- **Storage (ADR 0002).** `cqrs-es`; **SQLite is the default**, Postgres feature-gated, engine
  chosen **per workspace at runtime** behind one `PersistedEventRepository` trait.
- **App model (ADR 0005, 0006).** A workspace is a **directory** with a `workspace.toml`
  manifest, referenced by name; global config at `~/.config/genealogy/config.toml` layers
  `[defaults]` (frozen at use) under `[workspace-defaults]` (live fallback). `genealogy-app`
  owns the impure inputs (the `Session` — sole place a clock is read and a UUID v7 minted),
  workspace lifecycle, and the use-cases returning frontend-neutral DTOs.
- **UI layering (ADR 0008).** One-way dependency `genealogy-app → genealogy-ui →
  genealogy-ui-<framework>`: no `dioxus::` (or future `slint::`) type appears above the renderer
  crate, so a second framework is purely additive. App screens are per-framework view code over
  shared view-models; only **plugin** screens use the serializable UI vocabulary (ADR 0012, 0022)
  rendered by a per-framework interpreter.

## Workspace layout

Cargo workspace; `members = ["crates/*", "xtask"]`, inheriting shared package metadata and
lints from the root `Cargo.toml`. The WASM plugin **component** crates under `plugins/*` are
**excluded** from the workspace (they build only for `wasm32-wasip2`, a target from
`rust-toolchain.toml`); `--workspace` never builds or lints them — `cargo xtask
build-plugins` is the only path that does, and CI runs it before tests (ADR 0007, 0011).

Each crate's `lib.rs` module header holds the authoritative description; this table is the map.

| Crate | Role |
| --- | --- |
| `genealogy-core` | Domain model + event-sourcing engine. Pure — no I/O, no user-facing strings. |
| `genealogy-app` | Coordination and use-cases (ADR 0005, 0006); returns DTOs. |
| `genealogy-db` | Persistence (ADR 0002): tables, migrations, event store, projection storage. |
| `genealogy-cli` | The `genealogy` binary; its stdout/stderr is the interface. |
| `genealogy-ui` | Framework-agnostic presentation (ADR 0008): view-models, navigation/intents, Fluent, plugin-UI vocabulary. |
| `genealogy-ui-dioxus` | The Dioxus renderer + GUI binary. Library + binary, so SSR tests render without a window; the entry point is behind the `desktop` feature. |
| `genealogy-i18n` | Shared Fluent plumbing (ADR 0003): the workspace > shared-app > embedded override chain and locale fallback. |
| `genealogy-plugin-host` | WASM component host (ADR 0007, 0011, 0014): Wasmtime, deny-by-default capabilities over one versioned WIT world, fuel + memory limits. Sits above `genealogy-app`, driving use-cases under an `AgentKind::Software` session. |
| `genealogy-interchange` | The format-neutral leaf value vocabulary shared by the interchange formats — simple and serde-free; richer concerns stay in core. |
| `genealogy-gedcom`, `genealogy-gramps-xml`, `genealogy-digitalarkivet` | Pure parse/emit crates — the format logic of the `plugins/*` glue, free of WASM/host types so `--workspace` unit-tests them. Digitalarkivet fixtures (`…/tests/fixtures/`) are verbatim captures — **never reformat them** (prek skips its whitespace/EOF fixers there). |
| `xtask` (repo root, not `crates/*`) | Repository task runner, not shipped. Aliased in `.cargo/config.toml`. |

A new frontend consumes `genealogy-app` and never re-implements domain rules or coordination;
a GUI frontend goes through `genealogy-ui`, never `genealogy-app` directly.

The root `pub use` block in `genealogy-app/src/lib.rs` **is** the app's public surface — any
use-case, DTO, or re-exported core type a frontend consumes must be re-exported there (each one
is its own export). When wiring a new app→UI path, add the `pub use` first, or the consumer hits
`no X in the root`.

## Commands

```bash
cargo build --workspace                                              # build every crate
cargo run -p genealogy-cli                                           # run the `genealogy` binary
cargo run -p genealogy-ui-dioxus --features desktop                  # run the GUI
cargo nextest run --workspace --all-features --lib --bins --tests    # all tests (see note below)
cargo test -p genealogy-core <name>                                  # single test by name in one crate
cargo clippy --workspace --all-targets --all-features -- -D warnings # lint (zero warnings)
cargo fmt --all                                                      # format every crate
cargo deny --all-features check                                      # advisories, licenses, bans
cargo xtask check                                                    # i18n-check + css-check + input-guard
cargo xtask build-plugins                                            # lint + build plugins/* → target/plugins
cargo xtask gui-pass                                                 # drive the real GUI headless (below)
prek run                                                             # run git hooks manually
```

`cargo xtask` also runs the individual checks (`i18n-check`, `css-check`, `input-guard`) plus
`issue-sync`, `labels`, and `package` (Linux release tarball).

## Testing the GUI

SSR tests (`crates/genealogy-ui-dioxus/tests/*.rs`) assert markup and are the default — fast, and they
cover view logic. They cannot reach anything that only exists in a live webview: `document::eval`,
CSS, the MapLibre canvas, **which element a handler is attached to**, or **where focus actually goes**.
Those last two are not theoretical — the first scenarios written found three shipped defects that every
SSR test passed: `Esc` dismissed no overlay (the dispatcher is on `.app`, the overlays are siblings of
it), `?` never opened the help sheet (its chord is declared with no modifiers, but typing `?` always
reports Shift), and the help sheet's `autofocus` never took, leaving focus on `body`.

**`cargo xtask gui-pass` is how you test that layer.** It runs the real GUI on its own **Xvfb**
display, drives it with `xdotool`, and asserts over screenshots. Requires `xvfb`, `xdotool` and
`imagemagick`, so it needs a graphical-capable machine but no desktop session — and it is *more*
reliable than driving the GUI on your desktop, where mutter gives synthetic input to whatever the
compositor focused, not to the window you aimed at.

```bash
cargo xtask gui-pass                     # every scenario
cargo xtask gui-pass map-canvas          # one, by name
cargo xtask gui-pass --reset             # wipe the fixture workspace, isolated home and old shots
cargo xtask gui-pass --keep              # leave it up; attach with `x11vnc -display :99`
cargo xtask gui-pass --workspace gen     # drive your own config + workspace instead of the fixture
```

Scenarios are **TOML, not Rust** — `crates/genealogy-ui-dioxus/tests/gui-pass/*.toml`, so adding one
needs no rebuild. Each lists `[[step]]`s (`shot`, `click`, `key`, `drag`, `wheel`, `await-exit` to wait
for the GUI process to quit) and `[[assert]]`s over the shots by name: `differ` for "the UI reacted",
`match` for "the UI came back to this state", both with an RMSE tolerance and an optional
`region = [x, y, w, h]` to compare one window sub-rectangle instead of the whole shot; `manifest`
checks `target/gui-pass/workspace/workspace.toml` on disk for a substring instead, proving a write
reached disk rather than only an in-memory signal (unavailable under `--real-config`, whose workspace
path is the caller's own). Read the PNGs under `target/gui-pass/shots/<scenario>/`; crop with
`convert <in> -crop WxH+X+Y +repage <out>`.

Writing one:

- **Coordinates are window pixels at the scenario's `window`, 1800×1200 by default**, read straight off
  an earlier shot. Re-read them when the rail or a toolbar moves, and never carry one scenario's
  coordinates into another `window` size — a narrow-window layout reflows, it doesn't just crop.
- **`match` against the shot taken immediately before the change**, never against the first shot — focus
  rings are real pixels and move as a scenario runs.
- **`region` when a whole-window compare can't isolate the change** — e.g. a repaint elsewhere in the
  window (the tabstrip on every Save) would otherwise mask or fake a `differ`/`match` result.
- Runs are **isolated by default**: a throwaway `XDG_CONFIG_HOME`/`XDG_DATA_HOME` plus a seeded fixture
  workspace under `target/gui-pass/`. Keep it that way — a scripted click run writes events, and
  `--real-config`/`--workspace` point it at real genealogy data.
- Each scenario gets a **fresh GUI process, an empty shot directory, and a fresh copy of the seeded
  workspace**, so order never matters and stale shots cannot masquerade as part of a run.

Still human-only, and what `manual-verify` in [`docs/issue-tracking.md`](docs/issue-tracking.md)
reserves: pan/zoom smoothness, click latency, motion. Software GL is not a GPU, and a still image has
no frame rate.

The CLI's top-level commands are `init`, `rebuild`, `import`, `export`, `plugin`, plus one
subcommand-bearing verb per aggregate, generated from `for_each_cli_command!` in
`crates/genealogy-cli/src/main.rs`. Workspace selection is `--workspace`/`GENEALOGY_WORKSPACE`.

> **Always pass `--workspace` / `--all`.** `default-members = ["crates/genealogy-cli"]`, so a
> cargo command without `-p`/`--workspace` (`--all` for `fmt`) silently covers the CLI crate
> alone, skipping most tests, the other crates' `#[cfg(test)]` targets, and `xtask`. `cargo deny`
> and `cargo xtask` are unaffected. `nextest` is the local runner; CI uses `cargo test` and runs
> doctests separately, which neither `--lib --bins --tests` nor `nextest` covers.

`--lib --bins --tests` deliberately excludes `benches/`: each `genealogy-db` bench takes ~140 s
(15 of them, run `30783699764`), so nextest running them costs ~18 minutes it doesn't need to.
Clippy still runs `--all-targets`, so the bench code stays linted. Run benches deliberately with
`cargo bench -p genealogy-db --features sqlite`.

## Git

- Never commit to `main` — use feature branches and PRs.
- `--no-ff` for all feature branch merges to `main`.

## Conventions specific to this repo

- **Lints are deny-level guardrails, not suggestions.** The workspace denies `unwrap_used`,
  `panic`, `todo`, `unimplemented`, `exit`, `dbg_macro`, etc. Do not silence them with
  `#[allow(...)]` — `allow_attributes` is itself denied. Fix the code instead. `expect_used`
  is `warn`; justify any use.
- `print_stdout`/`print_stderr` are denied workspace-wide but **allowed in `genealogy-cli`**
  (its stdout/stderr is the UI). Domain code in `genealogy-core` must use `tracing`, never
  `print!`.
- The `[lints]` table can't both inherit and override, so `genealogy-cli` duplicates the
  workspace lint set with its print relaxations. Keep the two lists in sync.
- Event-sourcing invariant: events are the source of truth and are append-only. Never edit
  derived/projected state directly — emit a new event so the audit trail (operator + context)
  stays complete.
- **Every user-facing string is localized (ADR 0003).** All frontend text — stdout/stderr,
  labels, prompts, errors mapped from core types — goes through Fluent (`fl!()`), never a
  hardcoded literal and never a framework's built-in i18n. The CLI's per-`<lang>`
  `genealogy-cli.ftl` is **generated** by `build.rs` from tracked per-module fragments and is
  gitignored — edit a fragment, never the concatenated file. `genealogy-core` emits no
  user-facing strings: typed errors only, English `tracing` for developers.
- **Every UI change updates `docs/mockups/` in the same change.** The mockups are the design source
  of truth and must describe *shipped* behaviour, never follow-up work — so a shipped change that the
  mockups still contradict is an incomplete change. `docs/mockups/assets/components.css` is the
  superset: the app sheet must not introduce a rule the mockups lack.
- **Presentation vs data localization are distinct.** ADR 0003 is the *UI chrome*. The *data*
  language metadata (`LanguageTag`, `RichText.language`, `PlaceName`,
  `PersonName.transliterations`, data-model §14) describes what language a *record* is in. They
  share only the BCP-47 vocabulary (`unic-langid`).
- **License: the workspace is `MIT OR Apache-2.0` (permissive). Keep it that way.** New
  dependencies must be permissive-compatible; `cargo deny check` enforces this. **Never copy
  Gramps (GPLv2+) source** — the Gramps-derived model is a clean-room reimplementation; copying
  its code would force a copyleft relicense.

## Code navigation

A Tree-sitter knowledge graph (auto-updated on file changes) backs this repo via the
**code-review-graph** MCP server. Preference order, overriding the global "prefer LSP" default:
**the graph** for structure, callers/dependents, impact radius, and review context
(`semantic_search_nodes`, `query_graph`, `get_impact_radius`, `get_affected_flows`,
`get_review_context`, `get_architecture_overview`, `refactor_tool`); then **LSP** for definitions,
references, and types; then **`rg`/`ast-grep`/Read** for literal text and config values, or
whenever the graph doesn't cover the need.
