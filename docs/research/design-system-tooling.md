# A design-system platform for Vitni — Storybook, its alternatives, and the Rust problem

> Researched 2026-07-15 from fetched primary sources (npm, GitHub, storybook.js.org, docs.rs,
> tool pricing pages). Version claims accurate to that date: **Storybook 10.5.0** (MIT),
> `@storybook/server-webpack5` 10.4.6 (still published), `dioxus-web-component` 0.4.0,
> `lookbook` 0.2.0-alpha.1. Several SEO comparison sites surfaced during search contradicted npm/
> GitHub data and were discarded; every claim below traces to a primary source. Links were live
> when fetched.
>
> Companion reports: [`web-frontend-strategy.md`](web-frontend-strategy.md) (the web-component
> path matters more once a web target exists) and [`tailwind-css.md`](tailwind-css.md).

---

## 1. What you have, and what's actually missing

Today's "design system" is `docs/mockups/design-system.html` — a hand-authored specimen gallery —
plus 25 sibling mockup pages sharing `tokens.css`/`components.css`, and, on the code side, ~50
controlled RSX components whose accessibility contract is asserted by ~55 SSR tests
(`dioxus_ssr::render` over a `VirtualDom`; `tests/components.rs` already builds a `gallery()` of
every component).

Measured against what you said you value in Storybook, the gaps are:

1. **Controls** — no way to poke every prop of a component interactively; specimens are frozen.
2. **Stories** — no named, browsable states-per-component catalogue driving docs and tests.
3. **Addons** — no automated a11y (axe) pass, no visual regression, no generated docs.
4. **Single source of truth** — specimens are hand-written HTML that can drift from the real RSX
   components; the CSS is shared verbatim, the markup is not.

The key structural fact: your components are **Rust, not JS**. Every mainstream design-system tool
assumes a JS component source, so the real question is not "Storybook or an alternative?" but
**"which adapter gets a Rust component library into any of these tools?"** Two concrete paths
exist (§3, §4); everything else falls out of that.

---

## 2. Storybook today (10.5.0, verified 2026-07-15)

- **License MIT**, unambiguous; ~90 K stars; releasing weekly (10.5.0 published 2026-07-10).
  Storybook 9 (2025-05) was the testing-focused release (Vitest bridge — stories become test
  cases, 48 % smaller bundle); 10.5 adds experimental AI/agentic review and Claude/Codex
  integrations (interesting for this project, but experimental — don't weigh it yet).
- **Frameworks** (Vite builder): React, Vue 3, **Web Components**, **HTML**, Svelte, SvelteKit,
  Qwik, Solid. The feature-support matrix gives Web Components full parity with React for
  Controls, A11y, Interactions, Docs, Viewport, test runner.
- **`@storybook/web-components-vite` is Lit-agnostic**: it drives *any* standards-based custom
  element by tag name + attributes. Controls are fed by a **Custom Elements Manifest**
  (`custom-elements.json`); `@wc-toolkit/storybook-helpers` generates `args`/`argTypes`/template
  from it. So "custom element in, full Storybook out" is a paved road — the question is producing
  the custom element from Rust (§3).
- **`@storybook/server` exists and still ships** (`server-webpack5` 10.4.6, published 2026-06-16,
  tracking the main release cadence) but has been demoted out of the primary framework table —
  second-tier and thinly documented, not deprecated. Mechanism: Storybook fetches
  `${server.url}/${story.id}` and renders the returned **HTML string**; story `args` are appended
  as query params automatically, so Controls work; `fetchStoryHtml` is overridable.

---

## 3. Path A — Dioxus components as web components

There is exactly one concrete implementation: **`dioxus-web-component`**
(ilaborie, MIT OR Apache-2.0). It is real and functional — a `#[web_component]` macro replaces
`#[component]`, `wasm-pack` ships the result as an npm package, and one WASM module registers many
custom elements (so it's one bundle for the library, not one per component). Props map to string
**attributes** or typed JS **properties**; `EventHandler`s dispatch `CustomEvent`s, which
Storybook's Actions addon picks up natively. Dioxus core even merged an upstream fix specifically
to support it (issue #3011 → PR #3012: mount onto a `web_sys::Node`, enabling Shadow DOM roots).

The honest costs:

- **Single-maintainer, pre-1.0, last released 2025-01** (0.4.0), breaking changes expected, tiny
  download count. Committing ~50 components to it means carrying that dependency risk plus
  per-component glue: complex props (your enums, view-model structs) each need
  `js_type`/`TryFrom<JsValue>` conversions; property getters return **Promises** (Dioxus's async
  render), which complicates Storybook's two-way Controls binding; no JS-callable methods; only
  `#[component]`-macro components qualify.
- You maintain a **second compiled artifact** (the WASM bundle) whose only consumer is the
  gallery — until a web target exists, at which point custom elements become independently useful
  and the cost amortizes (see [`web-frontend-strategy.md`](web-frontend-strategy.md)).

What it buys over Path B: **live interactivity** — real event handlers, play functions
(interaction tests), focus/keyboard behavior in the browser. That is the full Storybook
experience.

Verdict on Path A: workable but not proportionate *today*. Re-evaluate when the web target lands.

---

## 4. Path B — `@storybook/server` over `dioxus_ssr` (the near-free bridge)

The mechanism in §2 fits what you already have with almost eerie precision: a small axum dev
server exposing `GET /story/{component}?prop=...` that decodes query params into props and returns
`dioxus_ssr::render(&vdom)` — which is **the exact code path your SSR tests run today**, repointed
from an assertion to an HTTP response.

What works unmodified on top of that: **Controls** (args → query params), **a11y addon** (axe
runs against the returned DOM), **docs/MDX**, **visual regression** (§6) — all over the *real*
markup your production components emit, with zero JS reimplementation and zero WASM shims. Your
`tokens.css` + `[data-theme]` theming loads via `preview-head.html` and a theme decorator — a
standard Storybook pattern.

The hard limit, inherent to HTML-over-HTTP: **no client interactivity**. Event handlers are Rust
closures that don't exist without the `dioxus-web` runtime, so play functions reduce to static DOM
checks. Interaction/behavior coverage stays where it lives today: the desktop app plus (optionally)
Playwright e2e — which is exactly how the Dioxus core team covers its own components (§5).

Caveats to spike before committing (~1 day): `@storybook/server` is absent from the official
feature-support matrix, so confirm Controls + a11y end-to-end in the current major; check the
webpack5-only builder is acceptable; decide the props-from-query-params encoding convention once
(serde makes this cheap).

---

## 5. The alternatives, disposed of quickly

| Tool | Status 2026-07-15 | Why it does / doesn't apply |
| --- | --- | --- |
| Histoire | active (1.0.0-beta.1, 2026-01) | Vue/Svelte only; no HTML/server/custom-element mode → irrelevant |
| Ladle | active-ish (5.1.1, 2025-11) | React-only by design → irrelevant |
| React Cosmos | active (7.3.0, 2026-04) | React-only → irrelevant |
| Backlight | **dead** (shut down 2025-06-01) | drop |
| Fractal / Pattern Lab | no 2026 signal found | legacy static-pattern-library era; no reason to prefer over what you have |
| UXPin Merge | active, proprietary | requires a React component source; a full React shim layer would cost as much as Path A for a paid tool |
| Zeroheight | active, $59–99+/editor/mo | docs-*publishing* layer, not a component runner; a possible later additive layer, never the runner |
| Chromatic | active SaaS | not an alternative — Storybook's paired visual-testing service (§6) |
| **`lookbook`** (Dioxus) | stale (0.2.0-alpha.1, 2024-09) | the one Rust-native Storybook-alike (`#[preview]` macro, prop controls); web-only, alpha, single-maintainer — evidence the idea works, not a foundation |
| **Hand-rolled preview app** | — | what **DioxusLabs itself does** for `dioxus-components`: a plain Dioxus `preview` crate served with `dx serve` (web or desktop), published as a static gallery, behavior covered by Playwright, CSS linted with stylelint. No Storybook, no web components. With 0.7's `subsecond` hot-patching (default-on since 2026-05), a gallery binary gets sub-second Rust reload — most of Storybook's feedback loop, no JS toolchain |

The DioxusLabs precedent is the strongest single signal in this research: the team that builds the
framework, facing your exact problem for its own component library, chose the hand-rolled gallery
\+ Playwright over every tool above. Also note the canonical tracking issue
(DioxusLabs/dioxus#1173, "Component preview like Storybook", open since 2023): there is no
first-party answer coming soon.

---

## 6. Visual regression, whichever runner wins

- **Playwright `toHaveScreenshot()`** — actively maintained, framework-agnostic, documented both
  against Storybook's iframe (`/iframe.html?id=...&viewMode=story`) *and* against any plain
  gallery page. Baselines are PNGs in git, diffed in your existing CI. Zero new services. For ~50
  components this is the obvious first choice.
- **Chromatic** — polished (PR-embedded diffs, UI review) but SaaS, snapshot-billed (free 5 000/mo,
  then $179+/mo), Storybook-coupled. Only if Storybook becomes the primary workflow and the polish
  earns its fee.
- **Lost Pixel** — open-source, self-hostable middle ground (Storybook/Ladle/Histoire/Playwright
  modes). Escalation option if plain Playwright outgrows itself.
- **BackstopJS** — last release 2024-09; skip.

---

## 7. Migrating the mockups themselves — screens, not just specimens

Both paths scale past single components: **a mockup page is just a screen component plus a
fixture view-model**, and the SSR test suite already renders whole screens (`person`, `pedigree`,
…) with fixture data. Full-page stories are standard Storybook practice, so the §4 story server
can serve `PersonScreen` + a fixture `PersonVm` exactly as it serves a `Chip` — with variants
("person with three names", "empty person", `[data-theme]` dark) as named stories, and the a11y
addon + screenshot regression (§6) running over entire screens. That replaces the mockup *pages*
for every screen that exists in code. The one thing no component-driven tool replaces is the
mockups' other job: **designing screens that don't exist yet** — you can't render components that
aren't written. Future-screen exploration stays hand-authored HTML, or becomes throwaway RSX in
the preview crate (arguably faster, since the building blocks exist).

### How much interactivity survives migration?

The mockups' `shell.js` gives them clickable tabs and popovers; server-rendered stories don't get
that for free — an inactive tab pane isn't merely hidden, it is **not in the HTML** (Rust renders
only the active tab). The options, in ascending cost:

1. **State as a Storybook control** (free): make `active_tab` a story arg. `@storybook/server`
   re-fetches the story URL when args change, so clicking a radio in the Controls panel re-renders
   the screen through the real Rust code path. Correct, but the click lives in the Controls panel,
   not on the tab.
2. **A small JS re-fetch bridge** (~50 lines of decorator glue): intercept the tab click, rewrite
   the story's query params, re-fetch the HTML from the story server, swap the DOM — htmx-style.
   Clicks land on the real tab and every state is really rendered by Rust. Hacky but viable
   because the story server is live; fold into the §8 spike.
3. **The preview crate, web build** (the §5 DioxusLabs pattern, promoted from fallback to
   complement): screen registry + fixture view-models compiled with `dioxus-web` and run via
   `dx serve`. Full interactivity — tabs, popovers, keyboard, focus — because the real event
   handlers run. This is "the migrated mockups, alive", with zero glue.
4. **A `dioxus-liveview` iframe inside Storybook** (spike-grade, UNVERIFIED for these
   components): liveview serves the same components over a websocket with no WASM; a server story
   could return an iframe pointing at a liveview-hosted screen, putting *full* interactivity under
   the Storybook roof. Elegant if it works; verify liveview's 0.7 status against the shell's
   assumptions before counting on it.
5. **Path A WASM web components** (§3): native in-browser interactivity, at the per-component
   glue cost already described.

The practical combination: **Storybook-server for the catalogued static states (controls, a11y,
visual regression) + the preview crate's web build for clickable walkthroughs.** Both consume the
same fixture view-model builders (extract them from the SSR tests once), so the second consumer is
nearly free.

### Retirement path

Whichever mix wins, **`design-system.html` should be retired first, not maintained in parallel** —
its hand-written markup is the drift risk your own memory notes flag (app CSS = verbatim mockup
copy: the CSS is synced, the specimens aren't). The screen mockups then retire one by one as each
screen gets stories/fixtures, leaving `docs/mockups/` only its future-screen exploration role.
`tokens.css`/`components.css` stay the shared source of truth exactly as today; nothing about any
path touches the CSS pipeline (deliberately — see [`tailwind-css.md`](tailwind-css.md)).

---

## 8. Verdict

Storybook the platform is safe (MIT, very alive) and it *is* the tool that matches what you said
you want — controls, a11y addon, stories. The adapter decides everything, and the staged answer
is:

1. **Now: spike Path B** — a ~1-day axum + `dioxus_ssr` story server behind `@storybook/server`,
   seeded from the existing `gallery()` in `tests/components.rs`. Success criteria: Controls
   round-trip on one component with non-trivial props (say `ConfidenceBadge` + `Chip`), **one
   full-screen story with a tab-state arg (§7's option 1)**, a11y addon flags a
   deliberately-broken specimen, `tokens.css` + `[data-theme]` theming works via
   `preview-head.html`. Stretch goals in the same spike: the JS re-fetch bridge (§7 option 2) and
   a liveview iframe (§7 option 4). If it holds, roll out story endpoints across components *and
   screens* and add Playwright `toHaveScreenshot()` baselines in CI.
2. **Alongside, not just as fallback: the preview crate** — the DioxusLabs pattern, a gallery
   crate of screens + fixture view-models under `dx serve --hotpatch` (web build for clickable
   walkthroughs, §7 option 3). If the server-framework spike stalls, this alone carries the whole
   plan (plus the same Playwright screenshot suite): less polish, zero new ecosystems, proven
   upstream. Extract the fixture builders from the SSR tests once; both consumers share them.
3. **When the web target lands** ([`web-frontend-strategy.md`](web-frontend-strategy.md) §7's
   spike): re-evaluate Path A (`dioxus-web-component` → `@storybook/web-components-vite`) for the
   handful of components where live interaction stories genuinely pay — by then the WASM bundling
   story exists anyway and the glue cost amortizes into the product.
4. **Either way**: retire `design-system.html` first, then the screen mockups one by one as each
   gains stories/fixtures (§7), and keep interaction/behavior testing in Playwright against the
   running app, not in the gallery.

No ADR needed yet — this is tooling, not architecture. If Path A ever graduates from gallery tool
to *shipping* web components, that's the moment it touches ADR territory (0008/0016).
