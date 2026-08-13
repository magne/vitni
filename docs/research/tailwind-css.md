# Tailwind CSS for Vitni — what utility-first would buy a hand-rolled token system

> Researched 2026-07-15 from fetched primary sources (tailwindcss.com docs/blog, GitHub releases,
> npm, dioxuslabs.com). Version claims accurate to that date: **Tailwind CSS 4.3.2** (2026-06-29,
> MIT), Dioxus 0.7 (built-in Tailwind support). Repo claims were spot-checked against the actual
> files (`crates/vitni-ui-dioxus/src/app.rs`, `src/components/button.rs`,
> `tests/components.rs`, `components.css`). Links were live when fetched.
>
> Companion reports: [`web-frontend-strategy.md`](web-frontend-strategy.md) and
> [`design-system-tooling.md`](design-system-tooling.md).

---

## 1. What Tailwind v4 actually is now

Worth stating precisely, because v4 (2025-01) changed the architecture and retired several classic
objections:

- **CSS-first configuration**: no `tailwind.config.js`; tokens live in an `@theme { … }` block in
  CSS. **Every `@theme` token is emitted as a real CSS custom property**, and generated utilities
  reference it via `var()` — `.bg-accent { background-color: var(--color-accent); }` — unless you
  opt into `@theme inline`, which bakes the literal value in and kills runtime overridability.
- **Standalone CLI**: a genuine single binary, no Node/npm anywhere. cargo-leptos auto-downloads
  and pins it — the no-Node path is proven in the Rust web ecosystem, not theoretical.
- **Dioxus 0.7 has built-in support**: `dx serve` auto-detects a `tailwind.css` next to
  `Cargo.toml` and runs the CLI itself; configurable via `Dioxus.toml`. And v4's `@source
  "./src/**/*.rs";` makes the scanner extract class names from Rust source — confirmed working in
  Leptos production writeups.
- `@apply` and `@layer components` still exist: Tailwind's own documented pattern permits
  hand-authored semantic classes that consume theme tokens. MIT license; permissive-policy clean.

So in 2026 the tooling objections — Node dependency, no Rust-source scanning, no Dioxus
integration — are all gone. The question is purely whether the *design* trade is worth it here.

- One real DX gap remains: **Tailwind's IntelliSense/LSP does not support Rust/RSX** — only a
  fragile `classRegex` workaround in a years-old open discussion (tailwindlabs #7073). Your
  current semantic class names need no editor tooling at all.

---

## 2. What Tailwind buys — and how much of it you already banked

Tailwind's genuine, consistently-reported wins, checked against this repo:

| Claimed win | Status here |
| --- | --- |
| Enforced token scale (no magic values) | **Already banked** — `tokens.css` semantic custom properties; "components never hardcode a hex" is the standing rule; `components.css` references `var(--…)` 359 times |
| No naming bikeshedding, no dead CSS | **Already paid** — ~50 components with a settled semantic vocabulary (`btn`, `chip`, `ev source`…); the naming decisions Tailwind saves you from are behind you |
| Colocation of style with markup | Real, but the cost side (§3) — and Tailwind's own docs recommend extracting a component once a pattern repeats, which is… the system you already have |
| Tiny purged output | Your CSS is 37 KB hand-authored and `include_str!`-embedded; there is no bloat to purge |
| Responsive variants across unknown viewports; papering over browser differences | **Weakly applicable**: primary target is a single desktop WebView (WebKitGTK). This is inference from verified facts, but the classic Tailwind rationale assumes a public multi-browser web surface you don't have (yet — and even the Phase-10 web app is one evergreen-browser client) |
| Component ecosystem (Tailwind UI, shadcn/ui, daisyUI) | Tailwind UI is commercial; shadcn/ui is React-bound; **only daisyUI transfers** (plain CSS classes, npm-free `@plugin` bundle, proven from Leptos) — and it is conceptually a competitor to your `components.css`, not an addition |

The pattern is clear: **Tailwind's headline value is a constraint system and a settled vocabulary
— you already own both.** v4's token architecture is, if anything, *validation* of your design:
utilities-over-CSS-variables is exactly the tokens.css approach, with a compiler attached.

---

## 3. What it would cost in this repo specifically

1. **The `skin.css` layer is the sharpest casualty.** `~/.config/vitni/skin.css` is injected
   last so an installation can restyle the app without a rebuild (`app.rs:31-51`). Under v4:
   - *Token-level* overrides **survive** — non-inline `@theme` utilities reference
     `var(--color-accent)`, so a skin redefining the variable still repaints everything.
   - *Structural* overrides **break**. Today "make every button pill-shaped" is one `.btn` rule.
     With per-element utility stacks there is no `.btn` to target — a skin would have to fight
     individual flat-specificity utilities, order-dependent against Tailwind's `@layer` output.
     Keeping this working means re-adding semantic wrapper classes — i.e. re-inventing
     `components.css` on top of Tailwind.
2. **The mockup↔app pipeline gains a build step.** Today: copy two files verbatim, zero CSS
   tooling. With Tailwind: a compiler run and a generated artifact, and the 26 static mockup pages
   need their own scan/build outside `dx serve`. A new moving part with no offsetting gain for
   static mockups.
3. **SSR tests get more brittle.** ~55 tests assert semantic markers (`"ev source"`,
   `tests/components.rs:157`). Utility stacks turn class assertions into long strings that churn
   on every visual tweak. Mechanical, not fatal — but friction.
4. **RSX verbosity.** `class: "btn primary"` becomes 10–20 utility tokens per element. JS
   ecosystems absorb this with `clsx`/`tailwind-merge`; Rust/RSX has no equivalent, so you'd
   absorb it with component extraction — again, the system you already have.

A directly analogous case study (dev.to, 2026-05): a team layered atomic CSS over an existing
token system, hit the parallel-mapping maintenance problem, and rolled back to "utilities for
value-less layout only; colors/spacing/radius stay in scoped CSS + `var(--ui-*)`" — concluding the
trade is "maintenance cost for syntactic sugar." The design-token critique (Crucible, 2026-07)
lands the same way: Tailwind deliberately omits the semantic middle tier; you built that tier.

---

## 4. Middle paths, priced

- **Tailwind as token engine only** (`@theme` emitting variables, all styling stays in semantic
  classes): produces the same CSS you hand-write today, via a compiler. Zero benefit; skip.
- **Layout-utilities-only hybrid** (utilities for value-less primitives — `flex`, `grid`,
  `items-center` — mapped to your `--sp-*` tokens; color/radius/shadow stay semantic): the one
  configuration with a defensible story, per the case study above. Adopt only if layout-class
  verbosity becomes a *felt* pain in RSX; nothing in the repo's history suggests it is.
- **daisyUI as inspiration, not dependency**: its plain-CSS component classes are the closest
  ecosystem analogue to your `components.css`; worth studying for patterns, not installing.
- **Open Props**: a token library — you already have one; redundant.
- **UnoCSS**: same trade-offs as Tailwind, smaller ecosystem, no `dx serve` integration; its
  attributify mode would add new RSX attribute surface. No advantage here.
- **CUBE-CSS-style discipline**: your base < skin layering + semantic classes + tokens effectively
  already *is* this, unnamed. Nothing to adopt.

---

## 5. Verdict

**Don't adopt Tailwind — not wholesale, and not now.** This is a "no action needed" conclusion,
not "adopt with caveats": the two problems Tailwind is famous for solving (an enforced token
scale, an end to naming/dead-CSS churn) are already solved here, its classic web-facing rationale
barely applies to a desktop WebView app, and its one sharp interaction with this codebase — the
`skin.css` end-user restyling layer — is strictly negative for structural overrides. The 2026
tooling story (standalone binary, `dx serve` integration, `.rs` scanning) removes every friction
objection and still leaves no positive case.

### What to take from Tailwind anyway

1. **v4 validates your architecture** — tokens as CSS custom properties consumed by everything
   above them is precisely what `tokens.css` does. No compiler required to keep being right.
2. If the **Phase-10 web app** ever creates real multi-viewport pressure (mobile browsers),
   revisit the **layout-utilities-only hybrid** (§4) — never utility colors/spacing; the skin
   layer depends on the semantic tier staying authoritative. Note this in ADR 0016's context if
   relevant (see [`web-frontend-strategy.md`](web-frontend-strategy.md)).
3. **Guard the constraint mechanically instead**: the one thing Tailwind enforces that convention
   doesn't is "no magic values." A trivial lint (grep for hex colors / raw `px` outside
   `tokens.css` in a prek hook or xtask) buys that enforcement with zero migration.
4. Skim **daisyUI**'s class taxonomy when extending `components.css` — free design review from a
   system with the same shape as yours.
