# Development

Platform setup, the test layers, and the repository's own tooling. [`README.md`](../README.md) has
the quickstart; this is the detail it links out to.

## Prerequisites

- **Rust** — latest stable via [`rustup`](https://rustup.rs). The `wasm32-wasip2` target is declared
  in `rust-toolchain.toml` and installs automatically.
- **Desktop GUI only** — a system webview, below. The CLI, the plugin host and the whole test suite
  need none of it.

### Desktop GUI system dependencies

`vitni-ui-dioxus` renders through a system webview (Dioxus desktop → `wry`/`tao`). The webview
sits behind a non-default **`desktop`** feature, so building the rest of the workspace and running
the tests needs no system libraries — only running the GUI does.

**Linux** — WebKitGTK and GTK development packages:

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

**Windows** — WebView2. The runtime ships with Windows 11 and current Windows 10; on older systems
install the
[Evergreen WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/). Building
needs no extra step (it links the system WebView2 loader).

**macOS** — the built-in WebKit; nothing to install.

## Everyday commands

```bash
cargo build --workspace                                              # every crate
cargo run -p vitni-cli                                           # the `vitni` binary
cargo run -p vitni-ui-dioxus --features desktop                  # the GUI
cargo nextest run --workspace --all-features --lib --bins --tests    # tests
cargo test -p vitni-core <name>                                  # one test in one crate
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all
cargo deny --all-features check                                      # advisories, licences, bans
cargo xtask check                                                    # every static check, in one pass
cargo xtask build-plugins                                            # plugins/* → target/plugins
cargo xtask icons                                                    # SVG icon sources → installed PNGs
prek run                                                             # the git hooks, by hand
```

**Always pass `--workspace`** (or `-p`, or `--all` for `fmt`). `default-members` is
`crates/vitni-cli`, so a bare `cargo test` or `cargo clippy` silently covers that one crate and
skips everything else, including `xtask`.

`--lib --bins --tests` deliberately excludes `benches/`: the `vitni-db` benchmarks take about
140 s each. Clippy still lints them through `--all-targets`. Run them deliberately:

```bash
cargo bench -p vitni-db --features sqlite
```

`cargo xtask` also exposes the individual checks (`i18n-check`, `css-check`, `input-guard`,
`licence-check`, `icons --check`) plus `issue-sync`, `labels`, `package` (the Linux release tarball)
and `screenshots` (the README images, below).

## The app icon and the brand art

`crates/vitni-ui-dioxus/assets/icon/` holds five SVG sources and the PNGs generated from them;
`assets/brand/` holds the two lockups and theirs. The sources are the design; the PNGs are committed
because a `.deb` and an AppImage install files rather than render vectors, and GitHub renders an
`<img>`. Edit an SVG, run `cargo xtask icons`, and commit both.

The mark is a **V with the weight a broad nib gives it** — heavy descending stroke, light ascending
one, three nodes — so the monogram is also a two-generation pedigree fragment. It stands in **three
ruled lines**: the record the conclusion is derived from. The heavy upper terminal is a **seal**, and
both it and the ruled lines are disclosed by size:

| Size | Source | The seal | The record |
| --- | --- | --- | --- |
| 256, 128 | `vitni.svg` | impressed ring + a chevron | three ruled lines |
| 64 | `vitni-notch.svg` | one cut flat on the rim | — |
| 48 | `vitni-plain.svg` | an ordinary node | — |
| 32, 24, 16 | `vitni-small.svg` | an ordinary node, mark 1.15× | — |
| any | `vitni-symbolic.svg` | never — monochrome, plate-less, for GNOME and the tray | — |

Palette, flat: plate `#142132`, mark `#e0a92e`, ruled lines `#8a6a26`. The gold matters — the mark
used to be the app's own `--accent`, which made the icon read as a chip from its own toolbar.

The impression is **subtracted** from the terminal through a mask, never added to it, so the
silhouette is the plain V's at every size and at 16 px the icon is indistinguishable from it. The seal
sits on the *heavy* stroke, which makes it exactly as wide as the stroke it caps, so it cannot read as
a notification badge.

`assets/brand/vitni-wordmark.svg` is the mark beside the name for `README.md`, and
`vitni-splash.svg` is 1280×640 — GitHub's social-preview size, and a ground for an About dialog or
splash when the app grows one. **Their letters are geometry, not text**: `resvg` is built without text
shaping, and a wordmark that depended on system fonts would render differently on every machine. Each
capital therefore takes the same nib logic as the mark; the layout parameters are in the wordmark's
header comment, and both files carry the lockup because SVG has no include.

`cargo xtask icons --check` (part of `cargo xtask check`) re-reads every committed raster: decodable,
the right size, and not fully transparent — which is what the previous stub was.

## Testing the GUI

Two layers, and they catch different things.

**SSR tests** (`crates/vitni-ui-dioxus/tests/*.rs`) render components to markup and assert over
it. Fast, and they cover view logic. They cannot see anything that only exists in a live webview:
`document::eval`, CSS, the map canvas, *which element a handler is attached to*, or *where focus
actually lands*.

**`cargo xtask gui-pass`** covers that layer. It runs the real GUI on its own **Xvfb** display,
drives it with `xdotool`, and asserts over screenshots — so it needs `xvfb`, `xdotool` and
`imagemagick`, but no desktop session. It is also *more* reliable than driving the GUI on a real
desktop, where the compositor hands synthetic input to whatever it thinks is focused rather than to
the window you aimed at.

```bash
cargo xtask gui-pass                     # every scenario
cargo xtask gui-pass map-canvas          # one, by name
cargo xtask gui-pass --reset             # wipe the fixture workspace, isolated home and old shots
cargo xtask gui-pass --keep              # leave it running; attach with `x11vnc -display :99`
```

Scenarios are **TOML, not Rust** — `crates/vitni-ui-dioxus/tests/gui-pass/*.toml` — so adding one
needs no rebuild. Each lists `[[step]]`s (`shot`, `click`, `key`, `drag`, `wheel`, `wait`,
`await-exit`) and `[[assert]]`s over the shots by name. Runs are isolated by default: a throwaway
`XDG_CONFIG_HOME`/`XDG_DATA_HOME` and a freshly seeded fixture workspace under `target/gui-pass/`,
because a scripted click run writes real events. Shots land in
`target/gui-pass/shots/<scenario>/` and the GUI's own log in `gui.log` beside them.

When a screenshot disagrees with your reading of it, column-scan instead of squinting:

```bash
convert <shot> -crop 1xH+X+Y +repage txt:-     # exact pixel rows
convert <in> -crop WxH+X+Y +repage <out>       # crop a region to inspect
```

Some things remain human-only, and the `manual-verify` label in
[`issue-tracking.md`](issue-tracking.md) reserves them: pan and zoom smoothness, click latency,
motion. Software GL is not a GPU, and a still image has no frame rate.

## The README screenshots

`cargo xtask screenshots` regenerates the images in [`assets/`](assets/) that `README.md` shows. It is
the same harness as `gui-pass` over a second fixture, so it wants the same `xvfb`, `xdotool` and
`imagemagick`:

```bash
cargo xtask screenshots                  # reseed, drive the GUI, rewrite docs/assets/*.png
cargo xtask screenshots --keep           # leave it running; attach with `x11vnc -display :99`
```

The scenario is `crates/vitni-ui-dioxus/tests/screenshots/readme.toml` and takes one `shot` per image;
`IMAGES` in `xtask/src/screenshots.rs` maps each shot name to its committed PNG and the width it is
scaled to. There are no `[[assert]]`s in it — it exists to produce pixels.

**Two runs must produce no diff**, or the command is not a refresh path. Three things would otherwise
make each run differ, and the command pins all three: the **clock** (every assertion carries the
instant it was made, and the Dashboard's activity feed, the History tab and the *Why we believe*
popover all render it, so the seeded event log is restamped to fixed instants and the projections
rebuilt from it), the **operator** (`init` names it after the OS user), and the **locale**
(`VITNI_LANGUAGE=en`). Aggregate ids stay random per run — they are UUIDs no screen renders — and human
ids are pinned by the seed script.

The demo workspace is **invented data**, seeded from scratch on every run: seven people over three
generations, two families, eleven dated and placed events, and one archive → source → citations chain
whose surety deliberately varies. No personal genealogy belongs in the repository, and the fixture is
isolated (`target/screenshots/`, a throwaway `XDG_CONFIG_HOME`) so a run cannot reach real data.

## Repository conventions

The ones that will fail a review if missed:

- **Lints are guardrails, not suggestions.** `unwrap_used`, `panic`, `todo`, `unimplemented`,
  `exit`, `dbg_macro` and others are denied workspace-wide, and `allow_attributes` is denied too —
  so an `#[allow(…)]` is not the way out. Fix the code. `expect_used` warns; justify it if you use
  it. `print_stdout`/`print_stderr` are denied everywhere except `vitni-cli`, whose stdout *is*
  the interface.
- **Events are append-only.** Never edit projected state directly; emit an event, so the audit trail
  keeps its operator and reason. Event payloads are self-contained and versioned, and changes are
  additive, so every historical event stays decodable.
- **Every user-facing string is localized** through Fluent — no hardcoded literals, no framework
  i18n. `vitni-core` emits no user-facing strings at all: typed errors, and English `tracing`
  for developers. The CLI's per-language catalogue is *generated* from tracked fragments by
  `build.rs`; edit a fragment, never the concatenated file.
- **Every UI change updates [`mockups/`](mockups/) in the same change.** The mockups are the design
  source of truth and describe shipped behaviour, so a change the mockups still contradict is
  incomplete. `mockups/assets/components.css` is the superset — the app sheet must not introduce a
  rule the mockups lack.
- **Never commit to `main`.** Feature branches and pull requests, merged `--no-ff`. Install the hooks
  with `prek install`.

[`docs/issues.md`](issues.md) is the backlog of record; its *Decided — no action needed* section
records deliberate non-tasks, so check there before "fixing" something.
[`issue-tracking.md`](issue-tracking.md) explains the labels, milestones and the doc ↔ tracker
linkage that `cargo xtask issue-sync` enforces.

> **Note on CI.** The workflows under `.github/workflows/` are committed and lint-clean but do not
> currently run: Actions billing is disabled for this repository. Run the checks locally — `prek run`
> plus the commands above — and reconcile labels with `cargo xtask labels --apply` rather than
> through `labels.yml`.
