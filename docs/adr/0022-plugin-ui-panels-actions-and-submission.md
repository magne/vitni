# 22. Plugin-UI panels, actions, and submission round-trip

- **Status:** Accepted
- **Date:** 2026-07-13

## Context

ADR 0012 fixed the first plugin-UI vocabulary: a single-screen **form** — a title, an
ordered list of typed fields (`text`, `number`, `checkbox`, `select`), and a submit label
— emitted as JSON, parsed by `genealogy-ui`, and rendered once per framework. It
deliberately left two things out of scope: **richer top-level descriptions** (lists,
tables) and **form submission back to the host** (the command-capability round-trip). It
named both as additive follow-ups "when a real plugin needs them" (ADR 0012 §1, Out of
scope).

Both are now needed to make plugin UI useful rather than a read-only demonstration. A
plugin that shows a form must be able to *do something* with it — create a note, run a
check, preview a result — and it needs a way to display the outcome and a read-only list.
This ADR fixes the panel taxonomy, the two new field kinds a real form needs, and the
submission round-trip, staying within the layering ADR 0012/0011/0008 set: the vocabulary
lives in `genealogy-ui`, the host carries every payload opaquely, and mutations flow only
through `genealogy-app` use-cases under a Software operator.

The app's own screens remain out of scope — they are per-framework view code over shared
view-models (ADR 0008 §5). Only plugin-contributed screens use this vocabulary.

## Decision

1. **A top-level `Panel` replaces the bare `Form`.** The vocabulary's root is now an
   internally-tagged enum `Panel` (`{"kind":"form"|"table"}`), matching the project's serde
   convention (ADR 0004 §4). `Form{title, fields, actions}` — the ADR 0012 `submit: String`
   is **replaced** (no dual format; ADR 0007 first-party fleet, disposable workspaces) by
   `actions: Vec<Action{id, label}>`, so a form can offer more than one button (Save,
   Preview, …). Two new field kinds land: `textarea` (a multi-line text input with an
   optional placeholder) and `date` (a plain date input whose wire value is an ISO-8601
   `YYYY-MM-DD` string or `""`; deliberately **not** the app's structured `GenealogicalDate`
   `DatePicker` — a plugin gets a plain date, not the calendar/quality/modifier cluster).
   `Table{title, columns, rows}` is the read-only list description ADR 0012 deferred:
   `title` and each `columns` entry are Fluent message IDs, and `rows` are literal data
   cells that are **never** resolved.

2. **Submission is a second export that round-trips values through the host.** The
   `ui-panel` world gains `handle-action: func(action: string, values: string) -> result<string, string>`.
   `action` is the activated `Action.id`; `values` is a JSON object keyed by each field's
   `name`: `text`/`textarea`/`select`/`date` → string, `checkbox` → bool, `number` →
   number-or-null. The frontend **seeds** every field on render (text-likes `""`, checkbox
   `false`, number `null`, select = its first option's value) so an untouched form still
   submits a complete object. The `Ok` payload is a JSON string parsed as an
   internally-tagged `SubmitResult`:
   `{"kind":"success","message":<optional Fluent ID>,"panel":<optional replacement Panel>}`
   or `{"kind":"failure","message":<Fluent ID>}`. The WIT `Err(string)` stays a
   **technical-failure** channel only (a trap, a denied capability, a malformed payload),
   rendered through the existing `plugin_error` chrome; **validation feedback rides
   `failure`**, not `Err`. The host stays opaque to every payload, exactly as it is for the
   form JSON and for GEDCOM bytes.

3. **`ui-panel` imports `commands`, gated at submit only.** Because a submission mutates
   state, the `ui-panel` world now imports the `commands` capability. Mutations flow only
   through `genealogy-app` use-cases driven by a `Session` whose operator is
   `AgentKind::Software` (ADR 0011 §2, §5; ADR 0007 §7), so a plugin-authored change is
   audited like any other. Capabilities stay **deny-by-default**: the render invocation
   (`run-ui-panel`) grants only `log`, and the submit invocation (`handle-action`) grants
   `log` + `commands`. A plugin that is never granted `commands` receives the host's
   `denied` result on any command call and reports it as a `failure` (or the host maps a
   hard denial to `Err`). The host adds no new way to mutate state.

4. **The host-API package bumps `0.14.0 → 0.15.0`.** A new required export
   (`handle-action`) on an existing world is a deliberate break — a `0.14.0` `ui-panel`
   plugin no longer satisfies the world. This is acceptable and documented for the same
   reason ADR 0013 removed the GEDCOM worlds at `0.2.0 → 0.3.0`: the first-party plugin
   fleet has no external consumers and is rebuilt in lockstep by
   `cargo xtask build-plugins`. No shims, no dual export set (a pre-1.0 world may change
   with documentation rather than be kept forever). Every plugin's pinned host-API version
   is swept to `@0.15.0`.

5. **Resolution extends ADR 0012 §5.** `genealogy_ui::resolve_panel(panel, …)` and
   `resolve_submit_result(result, …)` resolve Fluent message IDs against the plugin's own
   catalogue with the same nb/nn→no→en fallback and verbatim-on-miss behaviour as
   `resolve_form`. What they resolve: panel/form/table titles, field labels and
   placeholders, select-option labels, action labels, table **columns**, and the
   `SubmitResult.message`. What they **never** touch: field `name`s, action `id`s, select
   option `value`s, and table **row cells** — those are machine keys or literal data, not
   translatable chrome.

## Rationale

- **A tagged `Panel` over a second entry point.** Making the root a `kind`-tagged enum lets
  a plugin return either a form or a table (and a submission return a replacement panel of
  either kind) through one parse path, and leaves room for further top-level kinds
  additively — the same reasoning ADR 0012 used for a JSON tree over WIT records.
- **`Table` is the honest read-only list today.** The design system's `ListRow` is a
  `<button>` with activation semantics; wiring plugin-driven navigation to it is a separate
  design question. A `Table` of literal cells is a truthful read-only list that needs no
  navigation contract, so it ships now and list/detail navigation waits.
- **Values as a seeded JSON object.** Keying by `name` mirrors an HTML form submission and
  keeps the guest contract language-neutral (a non-Rust plugin reads the same JSON). Seeding
  on render means the guest never has to distinguish "untouched" from "empty", so an
  untouched form is always a complete, valid object.
- **`failure` vs `Err`.** Splitting expected validation feedback (`SubmitResult::failure`,
  a localized message the user should see) from technical failure (WIT `Err`, a developer
  string) keeps the user-facing path localized and the error path diagnostic, and matches
  how the host already renders `plugin_error`.
- **Break rather than shim.** ADR 0007 §2 and ADR 0013 already set the precedent that
  pre-1.0 worlds evolve by version bump with the fleet rebuilt together; a compatibility
  shim would be dead weight for a fleet with no external consumers (YAGNI).

## Consequences

### Positive

- A plugin form can now *do* something: submit values through audited `commands`, show a
  localized success/failure message, and replace itself with a follow-up panel.
- The read-only `Table` panel lets a plugin present a list without a navigation contract.
- Deny-by-default is preserved and testable: render grants only `log`, submit grants
  `commands`, and an ungranted command returns `denied`.
- The host learns nothing new about presentation; the layering and capability model
  (ADR 0011 §2) are intact.

### Negative / costs

- A breaking host-API bump (`0.14.0 → 0.15.0`) that requires rebuilding every plugin —
  cheap here (first-party fleet), but a recurring pre-1.0 cost.
- Two panel representations exist transiently (the plugin's JSON and the parsed `Panel`),
  and the guest's JSON is kept conformant by tests, not the compiler (as in ADR 0012).
- The vocabulary is still intentionally narrow: no repeating groups, nested forms, or
  per-field validation vocabulary yet.

## Out of scope

- **Repeating groups and nested forms** — additive when a plugin needs them.
- **`List`/detail descriptions and plugin-driven navigation** — the design-system `ListRow`
  is a `<button>` (activation semantics); `Table` is the honest read-only list today, and
  navigation waits on its own design.
- **Per-field validation vocabulary** — validation stays inside the plugin, surfaced as a
  `SubmitResult::failure` message.
- **Plugin-prefilled field values** — every field is seeded to its empty default on render.
- **The `query` capability for `ui-panel`** — a submit round-trip needs `commands`, not
  reads; a query-driven panel is a later step.
- **Long-running or streaming actions** — `handle-action` is a single request/response;
  progress reporting (the `progress` capability, ADR 0013) is not wired here.
- **Multi-panel pages** — one panel per invocation.
- **The app's own screens**, which are per-framework view code, not vocabulary (ADR 0008 §5).

## References

- ADR 0003 — Fluent/`i18n-embed` localization; governs how message IDs resolve.
- ADR 0007 — WASM-component plugin system; §2 versioned worlds, §7 Software provenance.
- ADR 0008 — Dioxus behind `genealogy-ui`; the framework-neutral home for the vocabulary.
- ADR 0011 — the `genealogy:host-api` WIT package, §2 deny-by-default capabilities, §5
  commands through the `genealogy-app` boundary under a Software `Session`.
- ADR 0012 — the plugin-UI form vocabulary this ADR extends (panels, actions, submission).
- ADR 0013 — the `0.2.0 → 0.3.0` precedent for a documented pre-1.0 world break with no shim.
