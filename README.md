# Vitni

**An evidence-first genealogy program.** Every fact is a recorded claim — who asserted it, when,
from which source, and with what confidence. The family tree you read is a *conclusion* derived from
those claims, not a database edited in place. Nothing is ever silently overwritten: a correction is a
new assertion that supersedes an old one, and both stay in the record.

*Vitni* is Old Norse for **witness**.

## What makes it different

- **Evidence and conclusions are separate layers.** The event log is the evidence: each entry is an
  assertion by a named operator, carrying its date, reason, confidence and citations. The entities
  you browse — persons, families, places, sources — are projections rebuilt from that log at any
  time. Two researchers can disagree in the record without one of them losing.
- **Corrections keep the trail intact.** Retracting or superseding an assertion references it by
  identity and appends a new entry. There is no destructive edit and no "who changed this?" that the
  program cannot answer.
- **Plugins are sandboxed WebAssembly components.** Import, export and plugin-contributed UI run as
  `wasm32-wasip2` components with deny-by-default capabilities — a plugin reaches the network, the
  media library or the command surface only where it was granted that capability, and signed
  first-party bundles are distinguished from untrusted ones.
- **Localized down to the message.** Interface text resolves through Fluent, including labels a
  plugin contributes, so a plugin's form is translated like the rest of the app. English and
  Norwegian ship today.

## Status

Pre-1.0 and usable. The domain model, persistence, import/export, the desktop GUI, the geography and
map views, and the plugin trust model are implemented; DNA breadth and a server-backed web frontend
are the remaining work. Release artifacts are **Linux-first** — a tarball, a `.deb` and an AppImage
(see [`docs/release.md`](docs/release.md)). The CLI and the desktop GUI are both thin frontends over
the same application layer, so they stay in step by construction; two known gaps in either direction
are tracked in [`docs/issues.md`](docs/issues.md) rather than papered over.

## Quickstart

Needs a current stable Rust toolchain via [`rustup`](https://rustup.rs); the `wasm32-wasip2` target
installs itself from `rust-toolchain.toml`.

```bash
# Create a workspace, add a person, list what is there
cargo run -p vitni-cli -- init demo /path/to/demo-ws
cargo run -p vitni-cli -- --workspace demo person create --given Ada --surname Lovelace
cargo run -p vitni-cli -- --workspace demo person list

# Build the import/export and UI plugins into target/plugins/
cargo xtask build-plugins

# Run the desktop GUI against that workspace
VITNI_WORKSPACE=demo cargo run -p vitni-ui-dioxus --features desktop
```

A workspace is a directory with a `workspace.toml` manifest, referred to by name; global settings
live in `~/.config/vitni/config.toml`. The CLI has one subcommand-bearing verb per record type
(`person`, `family`, `place`, `source`, `citation`, `event`, `note`, `media`, `tag`, `repository`,
`dna-test`, `dna-match`, `research-note`) plus `init`, `rebuild`, `import`, `export` and `plugin`.

The GUI renders through a **system webview** and therefore needs WebKitGTK on Linux; the CLI needs no
extra system libraries. Platform-by-platform setup is in
[`docs/development.md`](docs/development.md).

Interface language follows the system locale, or set it explicitly:

```bash
LANGUAGE=no cargo run -p vitni-ui-dioxus --features desktop
```

## Data model

The entity vocabulary — persons, families, events, places, sources, citations, repositories, media,
notes, tags, plus DNA tests, DNA matches and research notes — is informed by **[Gramps][gramps] v6**
and by other tools and standards in this space, including [webtrees][webtrees], Gramps Web, GEDCOM
and GEDCOM-X. Interchange today is GEDCOM and Gramps XML, both as plugins. The model is a clean-room
reimplementation: no Gramps source is copied.

[`docs/data-model.md`](docs/data-model.md) is the reference for the entities, the command and event
catalog, and the provenance envelope; [`docs/adr/`](docs/adr/) records the architecture decisions and
[`docs/roadmap.md`](docs/roadmap.md) the plan.

## Architecture

A Cargo workspace. Domain logic never depends on a frontend, and no UI framework type appears above
the renderer crate, so a second frontend is additive rather than a rewrite. The WASM plugin
components under `plugins/*` are excluded from the workspace — they build only for `wasm32-wasip2`,
via `cargo xtask build-plugins`.

| Crate | Role |
| --- | --- |
| `vitni-core` | Domain model and event-sourcing engine. Pure: no I/O, no clock, no user-facing strings. |
| `vitni-db` | Persistence: event store, projections, migrations. SQLite by default, Postgres feature-gated. |
| `vitni-app` | Coordination: config, workspace lifecycle, the impure inputs (clock, ids, operator) and the use-cases returning frontend-neutral DTOs. |
| `vitni-cli` | The `vitni` binary. |
| `vitni-ui` | Framework-agnostic presentation: view-models, navigation intents, Fluent resolution, the plugin-UI vocabulary. |
| `vitni-ui-dioxus` | The `vitni-gui` binary — a Dioxus renderer over `vitni-ui`. |
| `vitni-plugin-host` | Wasmtime component host: capability grants, fuel and memory limits, bundle signature verification. |
| `vitni-i18n` | Fluent plumbing: the workspace → shared → embedded override chain and locale fallback. |
| `vitni-interchange` | The format-neutral leaf value vocabulary shared by the interchange formats. |
| `vitni-gedcom`, `vitni-gramps-xml`, `vitni-digitalarkivet` | Pure parse/emit crates behind the corresponding plugins. |
| `xtask` | Repository task runner (`cargo xtask …`), not shipped. |

## Development

```bash
cargo build --workspace
cargo nextest run --workspace --all-features --lib --bins --tests
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all
cargo deny --all-features check          # advisories, licences, bans
cargo xtask check                        # i18n completeness, CSS tokens, input-handling guard
cargo xtask build-plugins
cargo xtask gui-pass                     # drive the real GUI headless and assert over screenshots
```

> Pass `--workspace` (or `-p <crate>`): `default-members` is the CLI alone, so a bare `cargo test`
> skips most of the repo.

The workspace denies `unwrap_used`, `panic`, `todo` and friends at deny level, and silencing them
with `#[allow(…)]` is itself denied — warnings get fixed, not suppressed. Every user-facing string
goes through Fluent, and every UI change updates [`docs/mockups/`](docs/mockups/) in the same commit.
Platform setup, the two GUI test layers and the rest of the conventions are in
[`docs/development.md`](docs/development.md).

## Licence

Split by layer:

- The **interchange crates** — `vitni-interchange`, `vitni-gedcom`,
  `vitni-gramps-xml`, `vitni-i18n` and the bundled plugin sources — are
  **`MIT OR Apache-2.0`** ([`LICENSE-MIT`](LICENSE-MIT), [`LICENSE-APACHE`](LICENSE-APACHE)), so
  anything can reuse them, including a GPLv2-only project.
- The **application** is **`AGPL-3.0-or-later`** ([`LICENSE-AGPL`](LICENSE-AGPL)), with an additional
  permission under section 7: a WebAssembly component that talks to the host only through the
  versioned plugin interface is not required to be AGPL. Third-party plugins, including proprietary
  ones, are welcome.

[`NOTICE`](NOTICE) has the crate-by-crate mapping, and
[ADR 0034](docs/adr/0034-licence-split-agpl-application-permissive-interchange.md) records why the
split is shaped this way; the longer analysis is in
[`docs/research/licensing-and-monetization.md`](docs/research/licensing-and-monetization.md).

A **commercial licence** is available for anyone who needs to embed the application layer in a closed
product — [`COMMERCIAL.md`](COMMERCIAL.md) covers what it does and does not include, and the cases
that need no licence at all.

Contributions come in under the grant in [`CONTRIBUTING.md`](CONTRIBUTING.md), which also explains
why that grant is as broad as it is.

[gramps]: https://github.com/gramps-project/gramps
[webtrees]: https://webtrees.net/
