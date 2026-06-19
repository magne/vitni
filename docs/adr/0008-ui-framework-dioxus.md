# 8. UI framework: Dioxus behind a framework-agnostic presentation layer

- **Status:** Accepted
- **Date:** 2026-06-18

## Context

The shippable frontend today is the `genealogy` CLI over `genealogy-app` (ADR 0006).
Validating the domain model and overall design needs a graphical app: navigation, rich
rendering of `Person`/`Family`/`Event`/`Place`/`Source` and their value objects
(`GenealogicalDate`, dated `PlaceName`, `RichText`, media), and eventually plugin-
contributed screens (ADR 0007). The app must be cross-platform (Windows/macOS/Linux),
reach handhelds (Android/iOS), optionally run on the web, look at least as polished as
Gramps and current genealogy products, stay light on resources, and be declarative
enough that sandboxed plugins can contribute UI.

Two further project constraints bind the choice. The workspace is licensed
**MIT OR Apache-2.0** (permissive), which the project keeps; and ADR 0003 fixes
localization on **Fluent / `i18n-embed`**, with `genealogy-core` string-free and every
user-facing string resolved through `fl!()`.

## Decision

Adopt **Dioxus** (MIT, RSX) as the first UI frontend, sitting behind a new
**framework-agnostic presentation layer** so the concrete framework is isolated and a
second framework is additive rather than a rewrite.

1. **Framework: Dioxus.** RSX is declarative; the crate is MIT (matches the permissive
   license); one Rust codebase targets desktop now and mobile/web later. UI strings
   live in Rust, so ADR 0003's `fl!()`/Fluent path applies directly — no gettext, no
   second i18n system.

2. **New framework-agnostic crate `genealogy-ui`.** It holds all presentation logic and
   **no framework types**: view-models derived from `genealogy-app` DTOs (e.g.
   formatting `GenealogicalDate`, building list/detail shapes), screen/navigation
   state, intent dispatch that calls `genealogy-app` use-cases, Fluent string
   resolution, and the plugin-UI **vocabulary types** (ADR 0007). It depends on
   `genealogy-app` only.

3. **Thin framework renderer `genealogy-ui-dioxus`.** The `genealogy` GUI binary. It
   binds view-models to RSX, routes UI events back to `genealogy-ui` intents, and hosts
   the vocabulary→widgets interpreter (point 5). It sits parallel to `genealogy-cli`
   and consumes `genealogy-app` through `genealogy-ui`; it does not re-implement domain
   rules or coordination. It owns its own `i18n/<lang>/*.ftl` and `RustEmbed` per
   ADR 0003.

4. **Boundary discipline.** Dependencies flow one way:
   `genealogy-app → genealogy-ui → genealogy-ui-<framework>`. No `dioxus::` (or future
   `slint::`) type appears above the renderer crate. This is what makes the framework
   swappable and gives the plugin vocabulary a framework-neutral home.

5. **Plugin UI uses a constrained vocabulary; the app's own screens do not.** The app's
   own screens are written as per-framework view code (RSX) over the shared view-models,
   keeping full access to the framework's rich components. Plugin-contributed screens
   instead emit a **constrained, serializable UI vocabulary** (forms/lists/tables)
   defined in `genealogy-ui`; each framework renderer maps that vocabulary to native
   widgets once, and every plugin reuses it. The whole app is never rendered through the
   vocabulary — that would rebuild a framework and discard why one was chosen.

6. **License posture preserved.** Dioxus's MIT license keeps the workspace permissive.
   No Gramps (GPLv2+) source is copied; the Gramps-derived model remains a clean-room
   reimplementation.

7. **A second framework is deferred but additive.** If a native footprint or runtime
   interpreter later matters, a `genealogy-ui-slint` (or other) renderer can be added,
   reusing `genealogy-ui` unchanged. Slint is viable for this via its royalty-free
   license. Only one frontend is built now — validating the design needs one app, and a
   second in parallel doubles cost for no extra validation.

## Rationale

- **License fit.** Dioxus is MIT, so the permissive workspace license is preserved with
  no `cargo deny` exception. Slint's open-source path is GPLv3 and its royalty-free
  path is a bespoke non-OSI license — both friction against the permissive policy.
- **i18n fit.** With Dioxus, user-facing strings live in Rust and resolve through the
  existing `fl!()`/`i18n-embed` path (ADR 0003) verbatim. Slint's built-in i18n is
  gettext/`.po`, which would either clash with ADR 0003 or force every string to be
  bound in from Rust as a property.
- **One stack, full reach.** Desktop + mobile + web from one Rust codebase keeps the
  project Rust-centric (web is welcome, not required) and avoids a JS frontend.
- **The presentation boundary pays for itself twice.** It isolates the framework *and*
  gives the ADR 0007 plugin vocabulary a framework-neutral home, so plugin UI is
  rendered through whichever framework renderer exists.
- **Why not Tauri.** It keeps Rust in the backend but moves the UI to web tech (JS/
  HTML/CSS), splitting the stack for a web capability that is not required; letting
  plugins inject HTML is a weaker security story than a constrained vocabulary.
- **Why not egui / Iced.** egui is immediate-mode with no serializable tree, a poor fit
  for a host-rendered plugin vocabulary; Iced is code-driven and lacks native mobile.

## Consequences

### Positive

- The workspace stays permissive; the open plugin ecosystem and reusable core are kept.
- Localization reuses ADR 0003 unchanged — strings in Rust, Fluent, no gettext.
- All presentation rules live in one framework-free crate; the framework is a thin,
  replaceable renderer, and a second framework is additive.
- The plugin-UI vocabulary has a neutral home and a single per-framework interpreter.
- Desktop now, mobile/web reachable from the same codebase.

### Negative / costs

- One extra crate (`genealogy-ui`) and the discipline to keep framework types out of it
  and to design view-models that assume no render model.
- Dioxus mobile renders via a webview today (heavier than native); its native renderer
  is still maturing.
- Maintaining two boundaries (app DTOs → view-models, view-models → widgets) is more
  layering than a single framework-coupled app.

## Out of scope

- The concrete plugin-UI vocabulary schema — deferred to the ADR 0007 follow-up ADR.
- A second framework renderer (e.g. `genealogy-ui-slint`).
- Native mobile/web build, signing, and distribution specifics.
- The concrete view-model set, screen inventory, and design system / theming.

## References

- ADR 0003 — Fluent / `i18n-embed` localization the UI reuses (strings in Rust).
- ADR 0005 / 0006 — workspace resolution and the `genealogy-app` use-cases + DTO
  boundary the presentation layer consumes.
- ADR 0007 — WASM-component plugin system; the deferred plugin-UI vocabulary this ADR
  gives a framework-neutral home.
- Dioxus (RSX, MIT; desktop/mobile/web; native Blitz/WGPU renderer in progress).
- Gramps — GPLv2-or-later; model/concepts/formats reused clean-room, no code copied.
