# 30. Customizable keyboard shortcuts

- **Status:** Accepted
- **Date:** 2026-07-27

## Context

`crates/vitni-ui/src/shortcuts.rs` defines `shortcuts()`, a framework-free, declarative
`Chord`/`ShortcutAction`/`Shortcut` map. It is **decorative only**: it feeds the `?` help overlay and
nothing else. `crates/vitni-ui-dioxus/src/shell/keyboard.rs`'s `shell_intent()` re-implements the
same matrix in `dioxus` `Key`/`Code`/`Modifiers` terms, hardcoded, and is what actually runs. There is
no `Chord → ShortcutAction → behavior` lookup anywhere, so the help overlay can drift from what the
dispatcher does, and an operator cannot change a binding — `docs/issues.md` (then §Ease of use) has carried
"customizable keyboard shortcuts" since it was first named, deferred to "the Phase 7 config split
(ADR 0015)".

ADR 0015 groups configuration into three owner scopes (workspace-functionality, operator,
client/presentation) behind a `ConfigStore` trait, and its Out of scope explicitly says "no new config
fields" pending a consumer. A user-rebindable keymap is that consumer: it is a **client/presentation**
setting — how *this* session's keyboard behaves — not a property of the dataset or the operator
identity, matching the `[ai]`/`[map]`/`[plugin-trust]` sections already living in the global
`~/.config/vitni/config.toml` (client-scope, global-only: a keymap is machine/user-local, the same
way AI provider credentials and plugin trust pins are).

Two constraints from prior ADRs bound the design. ADR 0008 fixed `vitni-ui` as framework-free —
`Chord`'s current `Modifier` enum (`None`/`Command`/`CommandShift`) is already too narrow to add
`Alt`, and no `dioxus::` type may enter `vitni-ui`. ADR 0003 fixed shortcut descriptions as Fluent
message ids resolved by the renderer's chrome catalogue, not literal strings — a rebound chord must
still resolve through the same catalogue, unchanged.

## Decision

1. **One resolved map, one source of truth.** `ShortcutAction` is the canonical, stable command id.
   `vitni-ui::resolved_shortcuts(overrides)` merges the default map with a workspace's overrides
   into the one `Vec<Shortcut>` both the dispatcher (`shell_intent`) and the `?` overlay
   (`help_overlay.rs`) read. The two-implementations situation this ADR exists to close ends here: the
   dispatcher becomes a lookup against the resolved map rather than a second hardcoded matrix.

2. **Only the `Global` group is rebindable.** The 9 existing Global actions plus the 2 added by the
   quit/close-tab PR (11 total) can be rebound. Within-screen keys (↑↓, Enter, `[`/`]`, ←→, Home, End,
   `s`, `e`) and the `g`-prefix navigation keys stay fixed. They are widget-owned and coupled to roving
   focus (`shell/roving.rs`) and the `g`-prefix state machine (`consume_g_prefix`) rather than dispatched
   through the flat chord table; making them configurable would mean redesigning those mechanisms for a
   use case (a genealogy desktop app, not an IDE) with no demonstrated demand for it. If a real workflow
   later needs it, a follow-up ADR revisits those mechanisms specifically.

3. **`[shortcuts]` lives in the global `~/.config/vitni/config.toml`, client scope, not the
   workspace manifest.** This follows the `[ai]`/`[map]`/`[plugin-trust]` precedent exactly (ADR 0015
   §1's client/presentation scope, materialized as client-scope-but-global-only tables): a keymap is
   tied to the person typing at this machine, not to the dataset. It has no live
   `[workspace-defaults.*]` counterpart — a keymap has no workspace-level fallback to override, unlike
   theme or locale.

4. **Chord syntax: a canonical parsed string, `mod+shift+alt+<key>`.** `mod` resolves to `⌘` on macOS
   and `Ctrl` elsewhere (unchanged from today), matching the platform-primary-modifier convention
   `shell/keyboard.rs::primary_modifier` already established. `Chord` implements `FromStr` and
   `Display` with one canonical round-tripping form, so a stored override string is unambiguous and a
   resolved chord can be rendered back into the same syntax for the preferences UI and `?` overlay.
   Unlisted actions keep their default chord. A conflicting override (colliding with another resolved
   binding) or an unparsable chord string is a **typed error surfaced in the UI** next to the offending
   binding — never a silent drop — matching this codebase's fail-fast convention (`CLAUDE.md`
   "Root causes"); the affected action keeps its default so one bad override cannot break the rest of
   the map.

5. **`Modifier` is replaced, not extended.** The two-variant `Modifier` enum (`None`/`Command`/
   `CommandShift`) cannot express `Alt` or independent shift without combinatorial variants; it becomes
   a `Copy + Eq` struct of three independent flags (`command`, `shift`, `alt`). This is a breaking
   change to `vitni-ui`'s public type with no shim or dual form (repo philosophy) — every consumer
   (`help_overlay.rs`'s `render_chord`/`primary_glyph`/`key_glyph`) is fixed in the same change.

6. **`vitni-app` stores the override map as untyped strings.** `ShortcutConfig { bindings:
   BTreeMap<String, String> }` (action config id → chord string) lives on `Config`, following the
   `AiConfig`/`MapConfig`/`PluginTrustConfig` shape. `vitni-app` must not depend on `vitni-ui`
   (app → ui is the fixed dependency direction, ADR 0008); all chord parsing, validation, and conflict
   detection lives in `vitni-ui::resolved_shortcuts`, which the `vitni-ui-dioxus` renderer calls
   with the loaded `BTreeMap`.

## Rationale

Unifying on one resolved map is what actually fixes the defect this ADR is named for — the help
overlay and the dispatcher provably agree because they read the same `Vec<Shortcut>`, rather than two
authors keeping two matrices in sync by convention. Scoping to client/global config (not workspace)
matches every other machine-local preference already shipped (`[ai]`, `[map]`, `[plugin-trust]`) and
needs no new `ConfigStore` method shape — `load_shortcuts`/`store_shortcuts` are one more pair beside
`load_map`/`store_map` in the same client-scope block. Limiting rebinding to the `Global` group keeps
the change to the flat chord table where a lookup genuinely replaces a `match`; the within-screen and
`g`-prefix mechanisms are structurally different (roving focus, a timed two-key state machine) and
redesigning them for configurability is not justified by any request on file.

## Consequences

### Positive

- The help overlay and the actual keyboard behavior can no longer drift apart — they share one
  resolved map.
- An operator who prefers different global chords (e.g. avoiding a conflict with a screen reader or
  window manager binding) can rebind them without a rebuild, taking effect live once saved
  (`StartupPrefs` held as a `Signal` in context, re-resolved on save — no restart required).
- Fits the existing client-scope config shape exactly; no new `ConfigStore` seam, no workspace manifest
  change.

### Negative / costs

- `Modifier`'s replacement is a breaking public-API change in `vitni-ui`, requiring every direct
  consumer (`help_overlay.rs`) to be updated in the same PR.
- The dispatcher's digit-range (`⌘1…9`) and `g`-prefix handling stay special-cased outside the flat
  lookup (they are physical-code-based and two-key respectively) — the lookup replaces the letter-chord
  `match` arms, not the whole dispatcher.
- One more small surface (`ShortcutsVm`, the preferences card, two `ConfigStore` methods, i18n keys) to
  keep in sync with the shortcut map when a future action is added.

## Out of scope

- **A VS Code-style *when* context** (named as future work by the "Global keys fire inside text
  controls" completed item, now in `docs/archive/completed-work.md`) — this ADR resolves the map lookup,
  not conditional activation
  by focus/mode. A future ADR would need to define the context predicates before this is revisited.
- **Rebinding within-screen or `g`-prefix keys** — see Decision §2.
- **Chord sequences** beyond the existing `g`-prefix two-key case — no second sequence mechanism is
  introduced.
- **A live key-capture widget** (press-a-key-to-bind) — the rebinding UI is a text field taking the
  canonical chord string (`cargo xtask input-guard` forbids a raw `input {}`, and a capture widget is
  untestable under SSR, where keyboard events are inert). Typing the canonical syntax is the only input
  method for now; a capture widget is a presentation-layer follow-up, not a data-model one.
- **A workspace-level or per-dataset keymap default** — see Decision §3; no `[workspace-defaults]`
  counterpart is added.

## References

- ADR 0015 §1–2 — the client/presentation configuration scope and the `ConfigStore` trait this ADR's
  `[shortcuts]` section slots into.
- ADR 0008 — the framework-free `vitni-ui` boundary `Chord`/`Modifier`/`resolved_shortcuts` must
  respect; the app → ui dependency direction that keeps parsing out of `vitni-app`.
- ADR 0003 — the Fluent label-id convention the resolved map's descriptions continue to follow.
- ADR 0027 — the prior "gate a config extension with an ADR" precedent this ADR follows structurally.
- `docs/issues.md` (then §Ease of use / Completed; now §Frontend & interaction → Keyboard & shortcuts,
  with the delivered entries in `docs/archive/completed-work.md`) — the backlog items this ADR unblocks.
