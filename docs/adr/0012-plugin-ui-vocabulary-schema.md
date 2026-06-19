# 12. Plugin-UI vocabulary: a serializable form schema rendered per framework

- **Status:** Accepted
- **Date:** 2026-06-19

## Context

ADR 0007 deferred *plugin-provided UI* ("a declarative UI vocabulary … rendered natively
by the host … deferred to its own ADR when the native/web frontend lands") and ADR 0008
named that follow-up explicitly ("the concrete plugin-UI vocabulary schema — deferred to
the ADR 0007 follow-up ADR") while fixing where it lives: the framework-neutral
`genealogy-ui` crate, with each framework renderer mapping it to native widgets once
(ADR 0008 §5). Spike D builds the first GUI (`genealogy-ui-dioxus`) and must prove a
plugin can contribute UI rendered by the host. This ADR fixes that vocabulary.

The app's own screens are **not** in scope: they are written as per-framework view code
over the shared view-models and keep full access to the framework's components (ADR 0008
§5). Only **plugin-contributed** screens use this constrained vocabulary, because plugins
are sandboxed WASM components (ADR 0007) that cannot link a UI framework and must not
inject arbitrary markup.

## Decision

1. **A small, serializable form vocabulary, defined in `genealogy-ui`.** The first
   vocabulary is a single-screen **form**: a title, an ordered list of typed fields, and a
   submit label. Field kinds are the minimum that proves the mechanism — `text`, `number`,
   `checkbox`, `select` (with options). The Rust types live in
   `genealogy-ui` (`vocabulary.rs`), the framework-neutral home ADR 0008 chose. The
   vocabulary is deliberately additive: new field kinds and new top-level descriptions
   (lists, tables) are added later without breaking older renderers or plugins.

2. **JSON is the wire format; the schema is the contract.** A plugin emits the form as a
   JSON document matching this schema. The plugin does **not** share the Rust types — it
   is a sandboxed component that may be written in any language, so the contract is the
   documented JSON shape, not a linked crate (mirroring how `genealogy-gedcom` keeps the
   format logic separate from the WASM glue). `genealogy-ui::vocabulary::parse` validates
   the JSON into the `Form` type; conformance of a real plugin's output is covered by a
   test. Serde uses the project's **internally-tagged** convention (a `kind` discriminator
   on each field), matching the event encoding (ADR 0004 §4).

3. **The host stays UI-agnostic; it carries the form as an opaque payload.** The
   plugin-UI capability is a new **`ui-panel` world** in the existing `genealogy:host-api`
   WIT package, exporting `run-ui-panel: func() -> result<string, string>` and importing
   only `log`. The host instantiates the world and returns the JSON **string** unparsed —
   exactly as `run-gedcom-export` returns GEDCOM bytes. `genealogy-plugin-host` therefore
   gains no dependency on `genealogy-ui`; the renderer crate
   (`genealogy-ui-dioxus`), which already depends on both, calls the host, parses with
   `genealogy-ui`, and renders. This is additive to the WIT package, bumping it from
   `@0.1.0` to `@0.2.0` (ADR 0011 §1: additive change → minor bump; existing worlds and
   plugins are untouched).

4. **Each framework renderer interprets the vocabulary once.** `genealogy-ui-dioxus`
   contains a `Form → RSX` interpreter. A second framework (ADR 0008 §7) adds its own
   interpreter over the same `genealogy-ui` types; no plugin changes.

5. **Plugin-supplied UI text is the plugin's responsibility.** ADR 0003 (`fl!()`/Fluent)
   governs the *app's* chrome. The labels inside a plugin form are data the plugin
   provides; localizing them is the plugin's concern (a future host capability can pass
   the negotiated locale to the plugin). `genealogy-ui` localizes only its own chrome
   around the rendered form.

## Rationale

- **Minimal but real.** A four-field form is enough to prove host → plugin → vocabulary →
  framework end to end without modelling the entire widget space up front (YAGNI). The
  additive design lets lists/tables follow when a real plugin needs them.
- **The boundary already exists.** Returning an opaque payload from a per-role world is
  the established host pattern (GEDCOM bytes); reusing it keeps the host free of
  presentation knowledge and the dependency direction intact (`app → ui → ui-framework`;
  `host → app`).
- **JSON over WIT records for the form tree.** Modelling a recursive, evolving widget
  tree as WIT records would duplicate the schema in `.wit` and force a major bump on every
  new field kind. A JSON string keeps the single authoritative schema in `genealogy-ui`
  and matches the "coarse-grained boundary" guidance (ADR 0007 §12): one call returns the
  whole form.
- **Language-neutral contract.** Because the plugin emits JSON rather than linking the
  Rust types, a non-Rust plugin contributes UI the same way — the property a plugin
  ecosystem needs.

## Consequences

### Positive

- Plugins can contribute a screen rendered by whichever framework renderer exists, with
  one interpreter per framework and zero per-plugin renderer work.
- The host gains a UI capability without learning anything about presentation; the
  layering and the deny-by-default capability model (ADR 0011 §2) are preserved.
- The schema evolves additively; old plugins and old renderers keep working.

### Negative / costs

- Two representations of a form exist transiently — the plugin's JSON and the parsed
  `Form` — and the plugin's JSON must be kept conformant by tests rather than the
  compiler.
- The vocabulary is intentionally narrow; rich plugin UIs wait on additive extensions.
- Plugin-content localization is unsolved here (deferred to a future host capability).

## Out of scope

- List/table/detail descriptions and richer field kinds (date pickers, repeating groups)
  — additive extensions when a real plugin needs them.
- Form **submission** back to the host (a command-capability round-trip) — this ADR
  renders a plugin form; wiring submit actions to `commands` is a later step.
- Passing the negotiated locale to plugins for content localization.
- The app's own screens, which are per-framework view code, not vocabulary (ADR 0008 §5).

## References

- ADR 0003 — Fluent/`i18n-embed` localization (governs app chrome, not plugin content).
- ADR 0007 — WASM-component plugin system; the deferred plugin-provided UI this ADR fixes.
- ADR 0008 — Dioxus behind `genealogy-ui`; the framework-neutral home for the vocabulary.
- ADR 0011 — the `genealogy:host-api` WIT package and its additive (minor-bump) evolution
  and deny-by-default capability model.
