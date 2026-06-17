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

## Workspace layout

Cargo workspace; member crates live in `crates/*` and inherit shared package
metadata and lints from the root `Cargo.toml`.

| Crate              | Role                                                                                                                                                                                          |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `genealogy-core`   | Domain model + event-sourcing engine (aggregates, events, event context/audit, projections). Pure logic — no I/O frontends.                                                                   |
| `genealogy-cli`    | The `genealogy` binary. Interactive terminal frontend; stdout/stderr are the interface.                                                                                                       |
| `genealogy-import` | *(planned)* Importers. Test fixtures under `crates/genealogy-import/tests/fixtures/` are verbatim Digitalarkivet captures — **never reformat them** (prek skips whitespace/EOF fixers there). |
| `genealogy-db`     | *(planned)* Persistence. Owns everything database-related: initial table creation, schema migrations, and the event-store / projection storage backing `genealogy-core`. Supports Postgres (server/multi-user) and SQLite (local single-user) selected per workspace at runtime via cqrs-es backends — see ADR 0002. |

When adding a frontend (native UI, web), it consumes `genealogy-core`; it does
not re-implement domain rules.

## Commands

```bash
cargo build                                                  # build workspace
cargo run -p genealogy-cli                                   # run the `genealogy` binary
cargo test                                                   # all tests
cargo test -p genealogy-core <name>                          # single test by name in one crate
cargo clippy --all-targets --all-features -- -D warnings     # lint (zero warnings)
cargo fmt                                                     # format
cargo deny check                                             # advisories, licenses, bans
prek run                                                     # run git hooks manually
```

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

<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool                        | Use when                                               |
| --------------------------- | ------------------------------------------------------ |
| `detect_changes`            | Reviewing code changes — gives risk-scored analysis    |
| `get_review_context`        | Need source snippets for review — token-efficient      |
| `get_impact_radius`         | Understanding blast radius of a change                 |
| `get_affected_flows`        | Finding which execution paths are impacted             |
| `query_graph`               | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes`     | Finding functions/classes by name or keyword           |
| `get_architecture_overview` | Understanding high-level codebase structure            |
| `refactor_tool`             | Planning renames, finding dead code                    |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.
