# 36. API keys resolve from the environment, then `.env` files

- **Status:** Accepted
- **Date:** 2026-09-26

## Context

Two places accept an API key, and both name an **environment variable** rather than holding the
secret: `[map.providers.*]`'s `api-key-env` (ADR 0033), for a keyed `MapLibre` style and the Google Map
Tiles session, and `[ai.providers.*]`'s `api_key_env` (ADR 0017 §4), for every `vision-api` provider.
Naming a variable keeps the key out of config files, logs and the event log. Both sites read the
variable with `std::env::var` when they need it.

That works from a shell. A GUI started from a desktop launcher (`.desktop` entry, AppImage, `.deb`)
inherits no shell profile, so the variable is missing and no keyed provider can be reached from an
installed build. Config cannot fill the gap without holding the secret, which is what naming a variable
was meant to avoid (#296).

## Decision

### 1. Lookup order

A variable named by `api-key-env`/`api_key_env` resolves, first match wins, from:

1. the process environment;
2. `<workspace>/.env`, in the directory of the workspace in use;
3. `~/.config/vitni/.env`, beside the global `config.toml`.

An empty value counts as unset at every step. A missing file is skipped. The shell always wins, so
`VAR=… vitni` still overrides whatever a file says.

`vitni_app::require_secret_env(name, workspace_dir)` is the single implementation, and both
`vitni-app`'s map-source resolution and `vitni-plugin-host`'s `vision-api` provider call it.

### 2. Looked up at use time, never loaded into the process environment

The files are parsed (`dotenvy::from_path_iter`) whenever a key is needed. They are never loaded with
`set_var` at startup. Two reasons:

- `std::env::set_var` is `unsafe` in edition 2024, because it is unsound once other threads run. By the
  time a workspace is known, the tokio runtime and the webview's threads already exist.
- The GUI can switch workspace while it runs. A startup load would keep serving the first workspace's
  keys, and would carry them into the next.

The files are small and are read only when a map provider resolves or an AI call starts. Both of those
already make a network request, so re-reading a file costs nothing that matters.

### 3. A `.env` value never reaches an error or a log

An unset key produces an error that names the variable and the two files checked. A malformed file
produces an error that names the file and nothing else: the parser's own error carries the offending
line, which may be the secret, so it is dropped instead of wrapped.

## Consequences

### Positive

- A keyed map provider or AI provider works from an installed build. The key goes in a file only its
  owner reads, and no config holds it.
- The key can differ per workspace (a project's own Google key) with a global fallback.

### Negative / costs

- A `.env` file holds a plaintext secret. The repository's `.gitignore` covers `.env`/`.env.*` for a
  workspace kept beside the source, but a workspace placed under version control elsewhere needs its
  own ignore rule. Vitni does not write one, because it does not create workspaces as repositories.
- A malformed `.env` that has to be consulted fails the lookup even if a later file would have
  supplied the key. The failure names the file to fix, which beats silently skipping it.

## Out of scope

- The OS keyring (Secret Service, Keychain). It would add a platform dependency and a UI for entering
  keys. `.env` covers the launcher case without either.
- Variable names other than those configured. Nothing else in the app reads these files.

## References

- ADR 0017 §4 — the `vision-api` provider and its `api_key_env`.
- ADR 0033 — named map providers, the Google session adapter, and `api-key-env`.
- ADR 0005 — the global config directory the second file lives in.
- Issue #296 — the backlog statement this decides; `docs/issues.md` → *Packaging & release*.
