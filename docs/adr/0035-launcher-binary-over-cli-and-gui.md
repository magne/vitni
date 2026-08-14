# 35. A `vitni` launcher binary over the CLI and GUI libraries

- **Status:** Accepted
- **Date:** 2026-08-14

## Context

The name a user types is the product's interface, and until this ADR `vitni` was the **CLI**:
`crates/vitni-cli` declared `[[bin]] name = "vitni"`, so the obvious command opened a terminal
frontend while the GUI hid behind `vitni-gui`. Two binaries, no shared entry point, and the more
discoverable name went to the less discoverable frontend.

Collapsing the two into one binary would fix the name and break the other half of the requirement: a
headless install would then pull `libwebkit2gtk-4.1-0`, `libgtk-3-0` and
`libayatana-appindicator3-1` to run `vitni person list`. `vitni-cli`'s `.deb` is plain
`depends = "$auto"` today, and keeping a webview-free CLI is worth more than binary-count tidiness.

Both entry points were already nearly thin enough to be called as libraries. `vitni-ui-dioxus` was a
library plus a 54-line `main.rs` whose real content was ~40 lines of window/theme setup;
`vitni-cli` already split `main` from an `async fn run(cli)`. What was missing was the crate that
calls them.

Two runtime facts constrain the answer. `vitni-cli`'s entry point is `#[tokio::main] async fn`, and
`dioxus::desktop` needs the **main thread** for `tao` — so a launcher that opened a webview from
inside a tokio runtime would be launching a window off the thread `tao` requires. The launcher's
`main` therefore has to stay sync, with the CLI owning its runtime internally.

ADR 0008 fixes the UI dependency direction as one-way `vitni-app → vitni-ui →
vitni-ui-<framework>`. A launcher is not covered by that rule: it sits **above** two sibling frontend
crates rather than inside the chain, which is why this needed a decision of its own rather than a
reading of 0008.

## Decision

### 1. Two shipped binaries, from three crates

| Binary | Crate | Links the webview stack |
| --- | --- | --- |
| `vitni` | new `crates/vitni` (launcher; `gui` feature on by default) | yes |
| `vitni-cli` | `vitni-cli` (renamed from `vitni`) | no |

`vitni-ui-dioxus` becomes **library-only**: its `pub fn run_desktop()` is what the launcher calls,
and the `vitni-gui` binary is gone.

Dropping `vitni-gui` rather than keeping it is the part worth recording. Keeping it would have cost
nothing in source but would have made the release tarball carry the CLI code twice and the UI code
twice — the launcher is the union of both — for a binary whose only remaining purpose was symmetry:
once `vitni.desktop` says `Exec=vitni`, nothing a user does reaches `vitni-gui`. Removing it also
means `cargo run -p vitni` is the single dev GUI command, and that the harness which drives the real
GUI (`cargo xtask gui-pass`) drives **the launcher**, so the no-argument dispatch path below is
covered by every existing scenario instead of being the one shipped path with no test.

### 2. Dispatch is an argument-count check, in-process

```rust
fn main() -> ExitCode {
    if std::env::args_os().nth(1).is_some() {
        return vitni_cli::run();
    }
    vitni_ui_dioxus::run_desktop();
    ExitCode::SUCCESS
}
```

Both arms are **library calls, not a spawned child**: no second process, no argv re-quoting, and the
CLI's `ExitCode` is returned rather than laundered through a wait status.

The check is argument *presence*, nothing finer. So `vitni --workspace demo` — arguments, no
subcommand — reaches clap and gets "subcommand required", and `vitni --help` / `--version` show the
CLI's. The rejected alternative was teaching the launcher `--workspace` so that invocation opened the
GUI on that workspace: better for one invocation, at the price of the launcher holding a second,
partial copy of the CLI's global argument surface that has to stay in step with it. A launcher that
parses nothing cannot drift from what it dispatches to.

### 3. The CLI owns its runtime; the launcher's `main` is sync

`vitni-cli` exposes `#[tokio::main] pub async fn run() -> ExitCode`, which is a *sync*
`pub fn run() -> ExitCode` to its caller: the runtime is built and dropped inside the CLI. The
launcher never names tokio, so the GUI arm runs on the process main thread as `tao` requires (§Context).

The macro also keeps the deny-level lints satisfied: a hand-rolled `Runtime::new()` returns a
`Result` this workspace may not `unwrap`, and the runtime is not a fallible input the caller could
act on.

### 4. The launcher carries no user-facing strings

It has no Fluent catalogue and no `i18n.toml`, because it emits nothing to localize (ADR 0003). The
one string in this area — the message a build without the `desktop` feature prints instead of opening
a window — stays in `vitni-ui-dioxus`, inside the `#[cfg(not(feature = "desktop"))]` arm of
`run_desktop()`, where it already lived. `cargo xtask i18n-check` discovers catalogues by the presence
of an `i18n.toml` and only errors on an `i18n/` directory without one, so a catalogue-less crate is
skipped rather than reported.

### 5. The launcher enables `vitni-ui-dioxus/postgres`

```toml
[features]
default = ["gui"]
gui = ["vitni-ui-dioxus/desktop", "vitni-ui-dioxus/postgres"]
```

`vitni-cli` enables `vitni-app`'s `postgres` feature unconditionally (ADR 0002: the shipped binary
picks the engine per workspace at runtime), so feature unification gives the launcher Postgres
support whether or not it asks for it. Leaving `vitni-ui-dioxus/postgres` off would then hide
Preferences' "Register workspace…" Database URL field in a binary that can open and create Postgres
workspaces — a GUI describing itself as less capable than it is. `docs/mockups/preferences.html`
already shows that field.

### 6. `default-members` stays `["crates/vitni-cli"]`

A bare `cargo build` continues to compile the CLI alone, and so continues to need no webview
libraries. The convenience of `cargo run` launching the GUI is not worth making the default build
pull wry/tao; `cargo run -p vitni` is explicit and costs one flag. Every documented workspace command
already passes `--workspace`/`-p` for unrelated reasons.

### 7. The `.deb` split follows the binaries

| Package | Contents | Depends |
| --- | --- | --- |
| `vitni` (from `crates/vitni`) | `/usr/bin/vitni`, `vitni.desktop`, the icon theme, the signed plugin fleet | `$auto, libwebkit2gtk-4.1-0, libgtk-3-0, libayatana-appindicator3-1` |
| `vitni-cli` (from `vitni-cli`) | `/usr/bin/vitni-cli`, the signed plugin fleet | `$auto` — installable headless |

`vitni.desktop` says `Exec=vitni`. The `.desktop` file and the icon set stay under
`crates/vitni-ui-dioxus/assets/` — `cargo xtask icons` writes there and `docs/development.md`
documents that path — and the launcher's asset globs reach them with `../vitni-ui-dioxus/assets/…`,
the same relative form the plugin fleet already uses.

## Consequences

### Positive

- The command a user is told to type opens the application, and the same command is a complete CLI.
- A headless or server install still gets a binary with no webview dependency at all.
- The launcher's GUI path is exercised by `cargo xtask gui-pass` (which now spawns
  `target/debug/vitni` with no arguments) and its CLI path by the same harness's fixture seeding
  (which invokes that binary with arguments), so both arms are covered by tests that already existed.
- `vitni` is now a crate with code in it rather than a reserved name, which is the distinction the
  crates.io usage policy draws — publishing becomes a package, not a name grab.
- One GUI entry point instead of two: `cargo run -p vitni`.

### Negative / costs

- **A plain `cargo build --workspace` now compiles wry/tao**, because the launcher's `gui` feature is
  default. CI already ran `--all-features` on every job, so CI cost is unchanged; a contributor
  building the whole workspace needs the webview libraries, where before only `--all-features` did.
- **`cargo run -p vitni-ui-dioxus --features desktop` no longer runs anything** — the crate has no
  binary. Every doc that named it now names `cargo run -p vitni`.
- **The renderer crate is no longer symmetric with `vitni-cli`**, which
  `docs/second-renderer-checklist.md` used as its worked example. A second renderer is a library plus
  a launcher feature, not another sibling binary; the checklist says so now.
- **`vitni-cli --help` prints `Usage: vitni …`.** One clap `name` serves both binaries, and `vitni` is
  the one users are meant to type. Naming the invoked binary instead would make the help of the
  primary command read as the secondary one's.
- **The two `.deb`s both install `/usr/lib/vitni/plugins/*`**, so installing both fails in dpkg. This
  is inherited, not introduced — the `vitni` and `vitni-gui` packages collided the same way — and is
  recorded in the backlog rather than fixed here; the fix is `Conflicts`/`Replaces` or a shared
  `vitni-plugins` package.

## Out of scope

- **Cross-platform launchers.** 1.0 is Linux-first (#215); a macOS `.app` whose `CFBundleExecutable`
  is the launcher, and the Windows console-vs-GUI subsystem question (a GUI-subsystem binary has no
  stdout to be a CLI on), are that cycle's problems.
- **Publishing to crates.io.** This ADR makes `vitni` a real package; whether and when it is published
  is a separate decision.
- **A launcher that selects among several renderers.** With one renderer, `gui` is a feature; ADR 0016
  or a second-renderer ADR decides what selection looks like when there is something to select.
- **The plugin-fleet path collision** between the two packages (§Consequences).
- **Merging the two binaries.** Explicitly rejected in §Context: it is the one shape that would put a
  webview dependency on a headless install.

## References

- ADR 0008 — the one-way `vitni-app → vitni-ui → vitni-ui-<framework>` UI layering this extends
  rather than supersedes; the launcher adds a layer above two frontends and names no framework type.
- ADR 0003 — why a crate with no user-facing strings needs no Fluent catalogue (§4).
- ADR 0002 — the per-workspace runtime engine choice that makes `vitni-cli` link Postgres
  unconditionally, which is what §5 follows from.
- ADR 0006 — `vitni-app` as the coordination layer both frontends sit on; the launcher reaches it
  through neither.
- ADR 0014 §4, §7 — the embedded plugin layer beside the binary, and the packaging the `.deb` split in
  §7 belongs to.
- ADR 0034 — the per-crate licence split; `crates/vitni` is an application crate, so
  `AGPL-3.0-or-later` plus the §7 plugin permission.
- Issue #331 — the backlog statement this decides; `docs/issues.md` → *Packaging & release*.
