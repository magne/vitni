# genealogy

An event-sourced genealogy program in Rust, inspired by and based on the data model of
[Gramps](https://github.com/gramps-project/gramps) (targeting Gramps **v6**).

Differentiators from Gramps:

- **Event-sourced core.** State is derived by replaying a log of events, not mutated in
  place. Each event carries an **event context** recording the **operator** who caused it —
  the system is auditable by construction (who changed what, when, and why).
- **CLI first.** The shippable interface today is the `genealogy` binary; a Dioxus desktop
  GUI (`genealogy-gui`) and a web app are planned. Domain logic lives in `genealogy-core`,
  free of any CLI/UI concern.
- **Plugins are WebAssembly components.** Import/export (GEDCOM today) and plugin-contributed
  UI run as sandboxed `wasm32-wasip2` components with deny-by-default capabilities.

The architecture and decisions are documented under [`docs/`](docs/): the domain model
([`data-model.md`](docs/data-model.md)), the architecture decision records
([`docs/adr/`](docs/adr/)), and the [`roadmap`](docs/roadmap.md).

## Workspace layout

A Cargo workspace; member crates live in `crates/*`. The WASM plugin components under
`plugins/*` are **excluded** from the workspace (they build only for `wasm32-wasip2`).

| Crate | Role |
| --- | --- |
| `genealogy-core` | Domain model + event-sourcing engine. Pure logic, no I/O, no user-facing strings. |
| `genealogy-db` | Persistence: event store + projections (SQLite default; Postgres feature-gated). |
| `genealogy-app` | Application coordination: config, workspace lifecycle, the impure inputs (clock, ids, operator), and the use-cases that return DTOs. |
| `genealogy-cli` | The `genealogy` binary — interactive terminal frontend. |
| `genealogy-ui` | Framework-agnostic presentation layer: view-models, intents, Fluent resolution, the plugin-UI vocabulary. No framework types. |
| `genealogy-ui-dioxus` | The `genealogy-gui` binary — a thin Dioxus renderer over `genealogy-ui`. |
| `genealogy-plugin-host` | WASM component plugin host (Wasmtime): loads plugins, wires capabilities, applies fuel/memory limits. |
| `genealogy-gedcom` | Pure GEDCOM parse/emit used by the GEDCOM plugins. |
| `xtask` | Repository task runner (`cargo xtask …`). |

## Prerequisites

- **Rust** — latest stable via [`rustup`](https://rustup.rs). The `wasm32-wasip2` target is
  declared in `rust-toolchain.toml` and installed automatically.
- **Desktop GUI only** — a system **webview** (see below). The CLI needs no extra system
  libraries.

### Desktop GUI system dependencies

`genealogy-ui-dioxus` renders through a system webview (Dioxus desktop → `wry`/`tao`). The
webview is behind a non-default **`desktop`** feature, so building the rest of the
workspace, the plugin host, and all tests needs none of these — only running the GUI does.

**Linux** — install the WebKitGTK and GTK development packages:

```bash
# Debian / Ubuntu (24.04+)
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev \
  libayatana-appindicator3-dev librsvg2-dev

# Fedora
sudo dnf install -y webkit2gtk4.1-devel gtk3-devel libxdo-devel \
  libappindicator-gtk3-devel librsvg2-devel

# Arch
sudo pacman -S --needed webkit2gtk-4.1 gtk3 xdotool libayatana-appindicator librsvg
```

**Windows** — the GUI uses **WebView2**. The runtime ships with Windows 11 and current
Windows 10; on older systems install the
[Evergreen WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/). No
extra steps to build (it links the system WebView2 loader).

**macOS** — uses the built-in WebKit; no extra system dependencies.

## Build and run

```bash
# Build everything (CLI, app, host, ui — desktop GUI excluded unless its feature is on)
cargo build --workspace

# Run the CLI: create a workspace, add a person, list
cargo run -p genealogy-cli -- init demo /path/to/demo-ws
cargo run -p genealogy-cli -- --workspace demo person create --given Ada --surname Lovelace
cargo run -p genealogy-cli -- --workspace demo person list

# Build the plugin components (GEDCOM import/export, the ui-panel demo) → target/plugins/
cargo xtask build-plugins

# Run the desktop GUI against a workspace (needs the webview deps above)
GENEALOGY_WORKSPACE=demo cargo run -p genealogy-ui-dioxus --features desktop
```

The GUI lists the workspace's persons in a sidebar, opens a person's detail on the right,
and — under **Plugin form** — runs the `ui-panel` WASM plugin and renders the form it emits
through the plugin-UI vocabulary interpreter.

### Localization

The UI is localized with Fluent (ADR 0003). The interface language follows the system
locale; override it per run, e.g. Norwegian:

```bash
LANGUAGE=no cargo run -p genealogy-ui-dioxus --features desktop
```

App chrome, data labels, and plugin form labels all resolve through Fluent — plugins ship
their own catalogues and return message IDs, so a plugin form is localized too.

## Development

```bash
cargo nextest run --workspace --all-features --all-targets   # tests (nextest locally)
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all
cargo deny check                                             # advisories, licenses, bans
cargo xtask i18n-check                                       # locale catalogues complete vs `en`
cargo xtask build-plugins                                    # build plugins → target/plugins
```

> Always pass `--workspace` (or `-p <crate>`): `default-members` is the CLI only, so a bare
> `cargo test`/`clippy` skips most crates. Building or testing with `--all-features` enables
> the GUI's `desktop` feature and therefore needs the webview system libraries above.

## License

`MIT OR Apache-2.0` at your option (the `license` field in the workspace `Cargo.toml`). The
workspace is kept permissive; new dependencies must be permissive-compatible (`cargo deny
check` enforces this). No Gramps (GPLv2+) source is copied — the Gramps-derived model is a
clean-room reimplementation.
