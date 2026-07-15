# Exposing Genealogy on the web — Dioxus web target vs. a web-native JS frontend

> Researched 2026-07-15 from fetched primary sources (dioxuslabs.com docs, docs.rs, GitHub
> issues/PRs, npm). Version claims are accurate to that date: **Dioxus 0.7.4** (0.7 line at 0.7.8,
> 0.8 in alpha), React 19, Vue 3.6, Svelte 5, **@fluent/bundle 0.19.1**. Links were live when
> fetched; ecosystem facts (bundle sizes, bug status) will drift — re-verify before ADR 0016.
>
> Companion reports: [`design-system-tooling.md`](design-system-tooling.md) (the web-component
> angle reappears there) and [`tailwind-css.md`](tailwind-css.md).

---

## 1. Where this decision sits

You already made most of it. ADR 0008 chose Dioxus partly *because* "one Rust codebase targets
desktop now and mobile/web later" and explicitly "avoids a JS frontend"; it rejected Tauri for
moving the UI onto web tech. Roadmap Phase 10 commits the web frontend to "reusing `genealogy-ui`
view-models and intents unchanged" and defers exactly one thing: whether the web renderer is *"a
web target of the same renderer or a sibling [crate], decided when built"* — gated by the proposed
ADR 0016.

So the honest framing of "Dioxus vs. React/Vue/Svelte" is not a green-field choice. It is:
**does anything about the web target justify reopening ADR 0008?** The answer below is no for the
app itself, with one carve-out (the pedigree/tree visualization, §5) that is a problem on *every*
path and should be solved in a path-independent way.

A useful clarification that fell out of the research: a Dioxus web build is arguably **not** the
"second renderer" ADR 0008 hedged about. Dioxus's platforms are separate renderer crates
(`dioxus-web`, `dioxus-desktop`) selected by cargo features on the umbrella `dioxus` crate — the
same RSX components run on both. A JS frontend is the thing the ADR's "additive second renderer"
promise was insurance *against*, and the ADR deliberately did not commit to ever paying for one.

---

## 2. Dioxus on the web today (0.7.x, verified 2026-07-15)

**The good — it's real, not aspirational:**

- **Current stable 0.7.4** (2026-03-27); the 0.7 line is on 0.7.8 (2026-05-07) with 0.8 in alpha.
  Releases are frequent and the team is active.
- **SSR + hydration + fullstack are shipped and documented**: `dioxus-ssr` renders a `VirtualDom`
  to HTML (this is what your ~55 SSR tests already exercise); `dioxus-fullstack` does streaming
  SSR with suspense-gated hydration over axum; server functions exist.
  (dioxuslabs.com/learn/0.7/essentials/fullstack/ssr)
- **Bundle size** is acceptable if you tune it: official hello-world claim ≈ 70 KB gzip; the most
  realistic real-app data point found is `wrench-dioxus-lab` (a size-tuned fullstack reference app
  with Playwright e2e across four browser engines): **941 KiB raw / 386 KiB gzip / 303 KiB
  brotli**. Naive builds are multi-MB — the release profile and `wasm-opt` matter. WASM also
  streams-compiles during download, so raw KB is not directly comparable to JS KB.
- **Accessibility is being actively hardened**, not just claimed: `dioxus-primitives` follows the
  WAI-ARIA authoring practices, and 2026-05 commits in `DioxusLabs/dioxus-components` fix real
  axe-core violations. Third-party ARIA-focused libraries exist (`stratum-primitives`).
- **Router** (`dioxus-router`, typed `#[derive(Routable)]` enum) and **hot reload** (`dx serve`,
  plus the `subsecond` Rust hot-patching shipped with 0.7) are solid.
- `document::eval` — the JS interop escape hatch — works identically on **web, desktop, mobile,
  and liveview**. Whatever interop you build for a web pedigree also runs in today's desktop
  WebView (§5).

**The rough edges — documented, recent, and worth respecting:**

- **SEO shipped broken by default until 2025-03**: fullstack returned HTTP 406 to Googlebot
  (fixed in PR #3851), and streamed content rendered inside a `hidden` div until WASM executed
  (issue #3276, remedied by making streaming optional). Head elements (`Title`/`Meta`) only reach
  the crawlable initial chunk if resolved before any `Suspense` boundary. Workable, but the happy
  path in the docs is younger than it looks. For an authenticated genealogy workspace app SEO is
  nearly irrelevant — this matters only if you ever publish shareable public pages.
- **"Death by 1000 papercuts"** (issue #4011, 2025-04) is a fair summary of DX complaints:
  workspace/feature-flag friction in multi-crate projects (you are one), `ErrorBoundary` bugs,
  `dx serve` stalls. `dx bundle --ssg` was nondeterministic as of issue #5050 (2026-04).
- **Large-DOM performance is an observed risk zone**: issue #3076 reports ~2 000 inserted nodes
  freezing *Safari* for 20–30 s (style-recalc cost, not clearly a Dioxus bug — but real, and at
  exactly pedigree scale). First-party virtualization exists only as `RecycleList`
  (list-shaped, in `dioxus-components`); there is **no first-party tree/graph component**.
- **No verified production Dioxus-web track record.** A vendor listicle claims Airbus/ESA use
  Dioxus — no primary source found; treat as marketing. The evidence base is reference repos and
  personal projects. You would be an early adopter on web, though not on Dioxus itself.

---

## 3. What each path reuses

This is the decisive section, because your architecture was explicitly built to make it decisive.

| Layer | Dioxus web target | JS frontend (React/Vue/Svelte) |
| --- | --- | --- |
| `genealogy-core` / `genealogy-app` | unchanged (behind the Phase-10 server) | unchanged (behind the same server) |
| Server API / DTO wire contract | shared | shared — this is the baseline, not an advantage |
| `genealogy-ui`: view-models, intents, navigation, shortcuts, palette | **reused unchanged** (ADR 0008's promise, enforced by `framework_free.rs`) | **hand-ported to TypeScript and maintained forever in parallel** |
| ~50 RSX components in `genealogy-ui-dioxus` | **reused** — same components compile for `web` and `desktop` features; platform-conditional code is minimal because `document::eval` and most hooks are renderer-agnostic | rebuilt on a JS component stack |
| ~55 SSR tests (`dioxus_ssr::render`) | reused as-is (renderer-agnostic) | replaced by Testing Library / Vitest equivalents |
| Fluent i18n (ADR 0003) | `fl!()` unchanged | **`.ftl` files reusable as data** via `@fluent/bundle` (Mozilla, Apache-2.0, 0.19.1 2025-04 — stable but slow-moving); the resolution *code* is rewritten |
| Design-system CSS (`tokens.css` / `components.css`) | shared verbatim, as today | shareable in principle (it's plain CSS) but the component markup that targets it is rewritten |
| Toolchain / CI | existing cargo workspace + `dx` | **new**: npm, bundler, JS lint/format/test, second CI pipeline — the repo currently has zero JS infrastructure |

The JS column's honest benefits: a much larger ecosystem exactly where you're weakest (§5's tree
libraries, virtualized tables, mature axe/Testing Library a11y tooling), and a larger hiring pool.
If going JS in 2026: React 19 wins ecosystem/hiring, Svelte 5 wins bundle/perf, Vue 3.6 (Vapor)
is the balanced middle — for a solo/small project, Svelte or Vue on merit, React only if hiring is
the real constraint. None of that outweighs duplicating the presentation layer for a project this
size: it is a second product to keep in sync, with drift as the steady state. That is precisely
the cost ADR 0008 spent a crate boundary to avoid.

---

## 4. The web-native path, costed honestly

What you would actually sign up for:

1. **Re-implement `genealogy-ui` in TypeScript** — view-model shaping, intent dispatch, navigation
   state, keyboard shortcuts, the palette, plus the plugin-UI vocabulary interpreter (ADR 0012) a
   second time. Every future screen is then built twice or drifts.
2. **Second i18n runtime** — same `.ftl` catalogues via fluent.js (genuine content reuse; ADR 0003
   discipline survives), but lookup helpers, fallback chains (your `no`/`nb-NO`/`nn-NO` logic),
   and the i18n-check tooling get a JS twin.
3. **Second toolchain** — package.json, lockfile, bundler, formatter, linter, test runner,
   Dependabot surface, CI jobs. Permanent fixed cost.
4. **A second design-system implementation** — the CSS transfers, the ~50 components don't.

And what you'd get: the JS ecosystem's genuinely superior off-the-shelf answers for data-heavy UI
(TanStack Virtual et al.) and for the pedigree view (§5), better-trodden SSR/SEO, and no bet on
Dioxus-web maturing. For a genealogy *workspace* app (authenticated, data-dense, not
SEO-driven), those advantages concentrate almost entirely in one screen family: the tree
visualizations. Which is why the recommendation isolates that problem instead of letting it drive
the whole frontend choice.

---

## 5. The pedigree view is a separate problem — on every path

The research's strongest finding: a MyHeritage-style interactive pedigree (pan/zoom, thousands of
nodes, expand/collapse) is **not safely buildable as naive DOM/SVG in any framework**, and real
genealogical data adds a failure mode frameworks don't see coming.

- **Rendering ceiling**: SVG and plain Canvas2D degrade from roughly 1 000–10 000 elements;
  beyond that WebGL (PixiJS) wins (Horak et al.; Neo4j engineering series). Dioxus's own issue
  #3076 (2 000 nodes freezing Safari) lands inside that band.
- **The endogamy bomb**: topola-viewer — a deployed genealogy viewer integrated into Gramps,
  Webtrees, and WikiTree — froze browsers for 4+ minutes on a real ~24 000-person file because
  cousin marriages made naive ancestor/descendant traversal explode: **445 895 traversal
  iterations**, a 445 K-node D3 tree, 1.3 M `getComputedTextLength` calls. The fix
  (topola PR #110, topola-viewer PRs #295/#296) was visited-family tracking → 3 579 iterations.
  Norwegian parish data will do this to you too. Cycle-aware layout is a *domain* requirement,
  not a rendering detail.
- **Off-the-shelf JS libraries** (relevant if a JS island or JS frontend is ever in play):
  - **family-chart** (donatso) — D3-based, MIT core, framework-agnostic, actively maintained; but
    "performance optimizations" sit in a paid Premium tier.
  - **topola** (PeWu) — TypeScript, genealogy-specific (ancestor/descendant/hourglass), the one
    with the endogamy fix and real integrations into Gramps/Webtrees/WikiTree.
  - **relatives-tree** — layout-only (positions in, JSON out), renderer-agnostic, tiny; stale
    since 2022 but stable; architecturally the cleanest separation to imitate.
  - **Balkan FamilyTreeJS** — capable but commercial with a restrictive EULA; **ruled out** by the
    workspace's permissive-license discipline.
- **In Dioxus**: SVG is first-class in RSX; `canvas` is an element but *drawing* goes through
  `web-sys`/`wasm-bindgen` or `document::eval` — i.e. you write the renderer. There is no
  first-party island abstraction for hosting a JS library; the documented pattern is
  `onmounted`/element-id → hand the node to JS via `eval` (maintainer's own words in discussion
  #3008: "you will deal with evals"). The `dioxus-use-js` crate reduces the stringly-typed pain.
  Crucially this works on the **desktop WebView today**, not just a future web target.
- **The path-independent building block**: keep layout math out of the renderer. The precedent is
  Dioxus's own `taffy` shipping wasm-bindgen JS bindings, and a documented Webcola case where
  moving layout (not rendering) to WASM took a force-directed graph from ~8 fps to 60 fps. A small
  Rust pedigree-layout crate (generations, spouses, **cycle handling**) can feed a Dioxus canvas,
  a JS canvas, or an SSR snapshot equally well — and it fits your risk-first spike culture.

Bottom line: build the pedigree as **canvas/WebGL rendering + culling/level-of-detail + a
cycle-aware Rust layout crate**, mounted inside a Dioxus-owned element. The existing simple
DOM/CSS pedigree screen is fine as the small-tree default; the "big tree" view is a renderer, not
a form, and should be engineered as one.

---

## 6. Comparable decisions in the wild

- **Arboretum** (shipping product, 2025-02 writeup) evaluated Dioxus, Tauri+Leptos, Tauri+Yew and
  chose **Tauri+React** — because they needed a mature JS text-editor component and judged Rust-UI
  interop friction too high. This is the closest real precedent to your pedigree question, and the
  cautionary tale: one killer JS component can sink a Rust-UI commitment *if you insist on solving
  it inside the framework*. The mitigation is §5's isolation strategy.
- Individual developers picking Leptos over Dioxus did so for bundle size/reactivity on simple
  sites; the one documented pick of Dioxus over Leptos cited cross-platform reach — ADR 0008's own
  rationale.
- No verified consumer-scale Dioxus-web production app was found (2026-07-15). Weigh that against
  the fact that your web frontend is Phase 10 — post-1.0 — and Dioxus's trajectory (0.8 in alpha,
  active a11y/fullstack investment) has months-to-years to mature before you build.

---

## 7. Verdict

**Stay on the Dioxus path for the web app; do not build a JS frontend.** The architecture you
already paid for (framework-free `genealogy-ui`, renderer-agnostic SSR tests, shared CSS) makes
the Dioxus web target nearly free where a JS frontend is a second product. Nothing found in
Dioxus-web's current state is disqualifying for a post-1.0, authenticated, non-SEO app — and the
decision point is still phases away.

**But treat the pedigree/tree view as its own engineering problem on every path**, solved with
canvas/WebGL + culling/LOD + a cycle-aware layout algorithm, isolated behind a Dioxus-owned mount
point so it never becomes an argument about the app framework.

### What to do now (cheap, order-independent)

1. **Spike a `web` build of `genealogy-ui-dioxus`** (a day or two): add the feature/target, run
   the existing screens in a browser, note what breaks (window/geometry/plugin-host coupling,
   `include_str!` CSS is already portable). This empirically answers Phase 10's "same crate or
   sibling" question long before ADR 0016 — and it doubles as the second-renderer readiness check
   Phase 5 already lists.
2. **Spike the pedigree-layout crate** (pure function: people/families in → positioned nodes +
   edges out, endogamy fixture from real-ish data). Test it against a deliberately endogamous
   fixture. This de-risks the hardest screen independent of any renderer decision.
3. **Prototype canvas rendering inside Dioxus desktop** via `onmounted` + `document::eval` or
   `web-sys` — the interop is identical on the desktop WebView you ship today, so nothing waits
   for Phase 10.
4. **Record in ADR 0016 (when written)**: web = Dioxus web target; JS frontend rejected for
   presentation-layer duplication; pedigree isolated as a renderer component; Balkan FamilyTreeJS
   excluded on license grounds.
