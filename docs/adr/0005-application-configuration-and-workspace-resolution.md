# 5. Application configuration and workspace resolution

- **Status:** Accepted
- **Date:** 2026-06-18

## Context

ADR 0002 established that the application manages **workspaces** — each an independent genealogy
dataset whose database engine is chosen per workspace at runtime — but left the runtime mechanics
open: what a workspace *is* on disk, how a binary finds one, where configuration lives, and how
the operator identity is determined. ADR 0004 §3 requires the operator `Agent` to be supplied by
the application layer on every assertion, without saying where it comes from.

The first shippable frontend is the `genealogy` CLI, so these need answers now. They are not
CLI-specific: a native UI and a web backend resolve the same workspaces and need the same operator
identity, so the model is defined once at the application layer (ADR 0006) and reused.

## Decision

1. **A workspace is a directory**, not a single file. It contains:
   - `workspace.toml` — the workspace **manifest**;
   - the database (for SQLite, a file inside the directory by default);
   - `exports/`, `backups/`, `media/` — working subdirectories.

   The manifest records:
   - **`database_url`** — the workspace's database. For SQLite, a file reference
     (`sqlite://genealogy.sqlite3`); a **relative** path is resolved against the workspace
     directory, so a workspace is portable/relocatable. For Postgres, a connection URL.
   - **`id_formats`** — the per-aggregate `HumanId` formats (see §4).
   - **`operators`** — the operators known to this workspace (see §3).

2. **The global config** (`~/.config/genealogy/config.toml`, resolved via the `directories` crate —
   XDG on Linux) holds:
   - a **registry of named workspaces** — `[workspaces.<name>]` with the workspace's `path` — and
     the **default** workspace by name (`default = "<name>"`, the last one created);
   - the **default operator** identity at top level (`[operator]`, §3);
   - a **`[defaults]`** table of *application* defaults — settings about app behavior / how new
     things are created, consumed at the relevant action and **not** live fallbacks. Today:
     `engine` (the DB engine a new workspace is created with; read once at `init` and frozen into the
     workspace's `database_url` — a database's location can't move).
   - a **`[workspace-defaults]`** table of *per-workspace configuration* defaults, every field a
     **live fallback** (§4): a workspace manifest may *override* a field, but an unset one resolves
     from `[workspace-defaults]` every time the workspace is opened — so editing a global default
     takes effect immediately for every workspace that hasn't pinned its own. Today: `id_formats`.
     Future per-workspace settings (privacy, locale, …) join here.

   ```toml
   default = "gen"

   [workspaces.gen]
   path = "/home/magne/gen"

   [operator]
   id = "019ed99c-…"
   display = "Magne Rasmussen"

   [defaults]               # app-level (frozen at use)
   engine = "sqlite"

   [workspace-defaults]     # per-workspace config defaults (live fallback)
   [workspace-defaults.id_formats]
   person = "I%04d"
   ```

3. **Workspaces are referenced by name.** `genealogy init <name> <path>` creates the directory,
   registers `name → path`, and makes it the default. Resolution for other commands, highest
   precedence first: the `--workspace <name>` flag → the `GENEALOGY_WORKSPACE` environment variable
   → the configured default; the name is looked up in the registry.

4. **Standard locations** come from `directories`, never hard-coded: the global config under the
   config dir; a workspace's default location under the data dir (`…/genealogy/workspaces/<name>`).

## Operator identity (direction, partially implemented)

The operator is informational provenance today (ADR 0004) and will become the **authenticated
user**. To avoid blocking that, this ADR fixes the direction now:

- **Implemented now:** the global config holds the default operator
  (`{ id (UUID v7), display, email }`, bootstrapped on first run from the OS user). On first use of
  a workspace the operator is **recorded in that workspace's manifest** (`[operators]`, keyed by
  id), so the id is never a loose value with no record. `email` is the **portable identity** — it
  lets the same human be recognized across machines even though `id` is generated locally.
- **Documented for later (not yet built):** the authoritative operator registry moves into the
  workspace **event store** (an Operator/Agent aggregate) when authentication lands; the
  configured operator becomes the authenticated principal; and **record signing** by operator
  (cryptographic attribution) is a future, additive extension. The current design — operator in the
  payload, recorded per workspace, identified portably by email — leaves room for all three without
  rework.

## Configurable HumanId formats

`HumanId`s (the Gramps `gramps_id` analog) use **per-aggregate printf formats** (Gramps: Person
`I%04d`, Family `F%04d`, …). The effective format is resolved **live** at open: a workspace
manifest `id_formats` *override* if present, else the global `[workspace-defaults].id_formats`;
`genealogy-core::IdFormat` parses `{prefix}%0{width}d{suffix}` (prefix and suffix may both be
non-empty; bare `%d` is unpadded) and is the single place ids are rendered and their numeric part
extracted. Allocation is numeric (not lexicographic), so it is correct across width growth
(`I9999` → `I10000`) and arbitrary prefix/suffix.

## Consequences

### Positive

- One workspace-resolution, identity, and id-format model serves the CLI, UI, and web backend.
- A workspace is a self-contained, relocatable directory (relative `database_url`); backups/exports
  have a defined home.
- First `init` bootstraps the operator and a default workspace, so the zero-setup local install
  works with no manual configuration.
- The operator direction is fixed, so auth/portability/signing are additive later.

### Negative / costs

- More on-disk structure (a directory + manifest) than a bare file.
- A bootstrapped operator id is only as meaningful as the OS account until authentication exists;
  multi-operator setups set `email`/identity explicitly.

## Out of scope

- The per-workspace config file beyond the manifest (no consumer yet).
- A DB-backed operator aggregate, authentication, and record signing (direction fixed above).
- Secret handling for Postgres connection strings (URL in the manifest for now; no keyring).
- CLI commands to edit the operator or id formats (edit the files directly for now).

## References

- ADR 0002 — workspaces and per-workspace engine selection (this ADR fills in the runtime model).
- ADR 0004 §3 — the operator `Agent` is supplied by the application layer (this ADR says where from).
- ADR 0006 — the application coordination layer that owns config + workspace lifecycle.
