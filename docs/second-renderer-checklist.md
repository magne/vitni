# Adding a second UI renderer

ADR 0008 fixes the UI as **Dioxus behind a framework-agnostic presentation layer**: dependencies
flow one way, `genealogy-app → genealogy-ui → genealogy-ui-<framework>`, and a second framework is
**additive** — a new renderer crate that reuses `genealogy-ui` unchanged (ADR 0008 §7). This is the
ordered checklist a `genealogy-ui-<framework>` crate follows. `genealogy-ui-dioxus` is the worked
example throughout.

The one rule everything else serves: **no framework type appears at or below `genealogy-ui`.** The
framework-free guard (`crates/genealogy-ui/tests/framework_free.rs`) makes that executable — it fails
if `genealogy-ui`'s dependency closure gains a `dioxus`/`slint` crate or a `src` file references a
framework namespace. A new renderer changes nothing in `genealogy-ui`, so that guard keeps passing.

## 1. Create the crate and layer it correctly (ADR 0008 §§3,4)

- New member crate `crates/genealogy-ui-<framework>`, parallel to `genealogy-ui-dioxus` and
  `genealogy-cli`. It is the GUI binary (a `[[bin]]`) plus a library so tests can render components
  without opening a window.
- Depend on **`genealogy-ui`** (view-models, intents, Fluent resolution, plugin-UI vocabulary) and
  **`genealogy-app`** (DTOs, use-cases). The concrete framework dependency and the plugin host
  (`genealogy-plugin-host`) live **only here** — never in `genealogy-ui`. This crate is the only
  layer that names a framework type.
- Put the system-library-heavy backend (e.g. a webview) behind an opt-in feature so the components,
  the interpreter, and the SSR test build without it. `genealogy-ui-dioxus` gates its webview behind
  a `desktop` feature; `default = []`.
- Duplicate the workspace lint set with the print relaxations an interactive GUI needs
  (`print_stdout`/`print_stderr` allowed), the way `genealogy-ui-dioxus/Cargo.toml` does — the
  `[lints]` table can't both inherit and override.

## 2. Bind view-models to views; route events to intents (ADR 0008 §3)

- The app's own screens are per-framework view code over the **shared view-models** from
  `genealogy-ui` — full access to the framework's rich components. Do not render the whole app
  through the plugin vocabulary (ADR 0008 §5); that would rebuild a framework and discard why one
  was chosen.
- UI events dispatch back through `genealogy-ui`'s intents/navigation, which call `genealogy-app`
  use-cases. The renderer holds no domain rules or coordination.
- Anything the renderer consumes from the app must be re-exported from the `genealogy-app` root
  `pub use` block (the app's public surface) — and, when it flows through the presentation layer,
  surfaced by `genealogy-ui`. Add those exports before wiring the consumer.
- **Keyboard shortcuts are a lookup against a resolved map, not a hardcoded matcher** (ADR 0030). Load
  the client-scope `[shortcuts]` overrides (`genealogy_app::ShortcutConfig`) and call
  `genealogy_ui::resolved_shortcuts` to get the one `Vec<Shortcut>` both the dispatcher and the `?`
  help overlay read; translate the framework's own key event into a `genealogy_ui::Chord`
  (`Modifier { command, shift, alt }` + `Key`) and look it up — never re-implement the chord matrix
  per renderer (mirror `genealogy-ui-dioxus/src/shell/keyboard.rs::shell_intent`). Only
  `ShortcutGroup::Global` actions are resolved this way; within-screen and `g`-prefix keys stay
  hardcoded, widget-owned chords.

## 3. Implement the plugin vocabulary→widgets interpreter (ADR 0008 §5)

- Plugin-contributed screens emit the constrained, serializable UI vocabulary defined in
  `genealogy-ui` (`vocabulary.rs`: `Panel` → `Form`/`Table`, field kinds, `Action`s, `SubmitResult`).
  Each renderer maps that vocabulary to native widgets **once**, and every plugin reuses it.
- Mirror the Dioxus interpreter (`genealogy-ui-dioxus/src/vocabulary_render.rs`): a top `Panel` view
  dispatching to form/table views, one widget per field kind, one button per `Action`, a form-value
  map seeded on render so an untouched form submits complete, and the submit round-trip that swaps in
  a returned replacement panel and announces success/failure.
- Resolve only what `genealogy-ui` says is a Fluent id — action labels, table column headers — via
  the shared resolution helpers (`resolve_panel`/`resolve_submit_result`). Never resolve field
  `name`s, action `id`s, or table row cells; those are literal data.
- Drive the plugin host directly from this crate under a software-operator session
  (`Session::software`, `AgentKind::Software`); the host's capabilities stay deny-by-default
  (ADR 0011 §5). `genealogy-ui` never sees a host type — the renderer hands it the plugin's JSON.

## 4. Localize through `fl!()` (ADR 0003, ADR 0008 §3)

- The renderer owns its own `i18n/<lang>/*.ftl` chrome catalogue embedded with `RustEmbed`, layered
  over runtime overrides (workspace > app-dir > embedded), exactly like `genealogy-ui-dioxus/i18n`.
- Chrome strings (window/navigation labels, renderer-level errors) resolve through this catalogue;
  data strings (names, field labels, application errors) come from `genealogy_ui::Localizer`. Every
  user-facing string goes through `fl!()` — never a framework's built-in i18n (no gettext).
- Keep catalogues complete against `en`; `cargo xtask i18n-check` gates it.

## 5. Test without a window (SSR precedent)

- Render components to a string in tests instead of opening a GUI, the way the Dioxus renderer does
  with `dioxus_ssr::render` over a `VirtualDom` (see `crates/genealogy-ui-dioxus/tests/`). Assert on
  roles, labels, and text — the accessibility contract, not pixels.
- Cover the interpreter end to end: run a plugin through the host, parse its JSON with
  `genealogy-ui`, render through the interpreter, assert the output (the `interpreter.rs` /
  `plugin_submit.rs` pattern). This needs the plugins built first: `cargo xtask build-plugins`.

## 6. Prove the boundary held

- `genealogy-ui` must be untouched. Run the framework-free guard —
  `cargo test -p genealogy-ui --test framework_free` — and the full gates
  (`cargo nextest run --workspace --all-features --all-targets`, clippy, fmt, i18n-check,
  `cargo deny check`). A second renderer that reused `genealogy-ui` as-is leaves every one green.
