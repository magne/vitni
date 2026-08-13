# 15. Configuration split and storage

- **Status:** Accepted
- **Date:** 2026-07-19

## Context

ADR 0005 fixed the application configuration model: a global `~/.config/vitni/config.toml`
(operator identity, the named-workspace registry, app-level `[defaults]`, live
`[workspace-defaults]`) and a per-workspace `workspace.toml` manifest (`database_url`, `id_formats`,
`operators`, UI preferences, locale overrides, plugin toggles). That model split configuration along
a **physical** axis — global file vs workspace file — not along a **conceptual** one. As the surface
grew (theme, window geometry, recent list, locale, plugins) the two files came to mix settings that
belong to different owners: the *dataset*, the *operator*, and the *client session*. Phase 13 (a
server backend with server-connected workspaces) needs those owners separated so operator and
presentation config can move to a database keyed by authenticated user, while workspace-functionality
config stays with the data. Pulling the split forward now — while the surface is small — avoids
re-touching a later-entangled config surface.

A concrete defect motivates the presentation half. The frontends build their Fluent localizers from
the raw environment request (`DesktopLanguageRequester::requested_languages()`, which reads
`LANGUAGE`/`LANG`) and **never consult the configured `ui_language`**. A workspace configured for
Norwegian therefore renders in English whenever a bare `LANGUAGE` is present in the environment —
stored config is silently outranked by an ambient env var. The locale-resolution intent of ADR 0003
is that configuration wins over the ambient system locale, so this is a bug, not a policy.

This ADR does not restate the configuration/workspace model (ADR 0005), the coordination-layer seam
(ADR 0006), or the localization layering (ADR 0003); it extends them: it groups configuration into
three named scopes, introduces a storage seam, and fixes the env-precedence order.

## Decision

1. **Three configuration scopes.** Configuration is grouped by *owner*, not by file:
   - **Workspace-functionality** — the dataset and how it behaves: the `database_url`, the
     per-aggregate `id_formats`, the workspace `operators` list, and plugin enable/disable. Plus the
     global registry (`[workspaces.<name>]`, `default`) and app `[defaults]` (the `engine` frozen at
     `init`) and the live `[workspace-defaults.id_formats]`. Shared: for a remote workspace this
     lives server-side with the data, identical for every client.
   - **Operator** — the acting operator `Agent` identity (`id`, `display`, `email`), global
     `[operator]`. On a server this is per-authenticated-user.
   - **Client / presentation** — how *this* session presents the workspace: UI theme, native-window
     geometry, the "Jump back in" recent list, and the locale overrides (`ui_language`,
     `data_locale`, `date_format`, `number_format`). Plus the live `[workspace-defaults.ui]` /
     `[workspace-defaults.locale]` app-level fallbacks. Local to the client.

2. **A `ConfigStore` seam, file backend now.** A `ConfigStore` trait exposes the three scopes as
   three method groups (load/store per scope). One `FileConfigStore { config_path, workspace_dir }`
   implements it over the two TOML files. The trait is the seam a **database** backend plugs into in
   Phase 13 (operator + presentation config, per authenticated user, behind the server that owns
   authentication). Only the file backend ships now.

3. **`database_url` stays in the workspace manifest.** It is the ADR 0005 portability anchor, frozen
   at `init` (a database's location cannot move). It is therefore workspace-functionality scope, not
   client scope. The client-scope **`endpoint`** — the address a client connects *through* to reach a
   server-hosted workspace — does not exist until the Phase 13 server; it is named here as deferred so
   the scope boundary is unambiguous when it lands.

4. **Env-var precedence for the UI language: plain env < config < prefixed env.** The resolved
   request order, highest last:
   1. **plain environment** (`LANGUAGE` / `LANG`, via `DesktopLanguageRequester`) — the ambient
      system locale, the weakest signal;
   2. **configured `ui_language`** — the workspace override resolved over the live app default; wins
      over the ambient env (the bug fix);
   3. **`VITNI_LANGUAGE`** — an explicit, app-scoped override; wins over everything, so a user can
      force a language for one run without editing config.

   A pure resolver `resolve_requested_languages(config_ui_language, plain_env, prefixed_env)` encodes
   the order (`[prefixed]` else `[config]` else `plain_env`); the frontends supply `plain_env` from
   `DesktopLanguageRequester` (keeping `vitni-app` free of `i18n_embed`) and read
   `VITNI_LANGUAGE` through the app. `vitni-core`/`vitni-app` still emit no user-facing
   strings — this is the request the frontends' Fluent loaders negotiate against.

5. **On-disk layout: the two ADR 0005 files are retained.** A clean file-format break is permitted
   (workspaces are disposable; re-`init` is acceptable — no back-compat with prior files is owed), but
   the existing `config.toml` + `workspace.toml` tables already map cleanly onto the three scopes
   (§1), and no consumer needs a different byte layout. The scope split is realized in the **type
   system** — three scope types behind the `ConfigStore` trait — rather than by rewriting the file
   shape (YAGNI). A future DB backend serializes the same scope types; it does not read the TOML.

## Rationale

Grouping by owner (not by file) is what Phase 13 needs: the server hosts workspace-functionality and,
per authenticated user, operator + presentation config; a client keeps only its own presentation
scope and the endpoint. Fixing that boundary now, behind a trait, means the server work adds a backend
rather than re-slicing config. Keeping the file layout stable keeps the change reviewable and avoids a
migration no user benefits from.

The precedence fix follows the principle already in ADR 0003 — explicit configuration beats the
ambient system locale — and adds one explicit escape hatch (`VITNI_LANGUAGE`) above config for a
one-off override, matching how `VITNI_WORKSPACE` already overrides the default workspace.

## Consequences

### Positive

- The three owners are separated before the surface grows, so the Phase 13 server adds a `ConfigStore`
  backend instead of re-entangling config.
- Configured `ui_language` now wins over an ambient `LANGUAGE`, so a workspace renders in its
  configured language regardless of the host env; `VITNI_LANGUAGE` gives a clean per-run override.
- The pure resolver is unit-tested for every precedence case; `vitni-app` stays free of
  `i18n_embed`.

### Negative / costs

- The `ConfigStore` trait initially wraps the same two files the free functions already read/write —
  a thin layer whose value is the seam and the scope typing, realized fully only when the DB backend
  lands.
- The scope split is conceptual/type-level, not physical: the TOML still holds more than one scope per
  file, so "which file" and "which scope" are not one-to-one until Phase 13.

## Out of scope

- The **database** `ConfigStore` backend and server-connected workspaces (Phase 13, ADR 0016).
- The client **`endpoint`** field (no server to connect to yet — Phase 13).
- **No new config fields.** Privacy/`Restriction` rules, data-language metadata, and a configurable
  surety scheme are named as workspace-functionality scope by ADR 0005 / the roadmap but have no
  consumer yet, so they are not added here (YAGNI).
- A **general environment-overlay seam.** Only the one env key that exists (`LANGUAGE` /
  `VITNI_LANGUAGE`) is resolved, by a single typed resolver; a general `VITNI_*`-over-config
  overlay is documented intent, not built (a deliberate scope trim — there is one key to resolve).

## References

- ADR 0005 — the configuration and workspace-resolution model this ADR groups into scopes and puts
  behind a store.
- ADR 0006 — the `vitni-app` coordination layer / `Session` seam the store and resolver live in.
- ADR 0003 — the Fluent locale resolution the env-precedence fix plugs into.
