# Issue tracking on GitHub

- **Status:** **Applied 2026-07-27.** 39 labels and the issue-template forms exist, alongside
  `.github/labels.toml` and `cargo xtask issue-sync`. `0.8 — UI parity` shipped and is closed; the two
  remaining gates are `0.9` and `1.0`.
- **Date:** 2026-07-27
- **Audience:** anyone filing, triaging, or closing an issue
- **Companion:** [`issues.md`](issues.md) is the backlog this describes; [`roadmap.md`](roadmap.md)
  owns phase detail.

## Starting state

Before this was applied the repository had **only GitHub's default labels**, **no milestones**, and
**no open issues** (the one referenced issue, #38, was closed) — so the taxonomy below was designed
greenfield, with nothing to migrate or reconcile.

**One caveat about the automation.** Actions billing is blocked, so every workflow run currently fails
before starting. `.github/workflows/labels.yml` is therefore committed but **never executed** — it is
zizmor-clean and YAML-valid, no more. Until billing clears, reconcile labels locally with
`cargo xtask labels --apply`. This is also why the doc↔tracker drift check is an **xtask** rather than
a scheduled workflow: it has to work without Actions.

[`issues.md`](issues.md) currently holds **91 bullets: 78 actionable and 13 that are not tasks at
all** — the latter live under *Decided — no action needed* and record deliberate choices ("by design",
"a deliberate simplification", "permanently non-round-trippable", "recorded so it is not re-raised").
Filing those as issues would create a tracker that can never reach zero, so the split below matters
more than the label names.

## 1. What lives where

| | Owns |
| --- | --- |
| `docs/issues.md` | The **backlog of record.** Every outstanding item, grouped by area, plus the whole *Decided* section. Reviewable in one diff, greppable, and versioned with the code it describes. |
| GitHub Issues | The **work queue.** Only items someone intends to work on this cycle or next. |

An item graduates doc → issue when it is picked up, not when it is discovered. Keeps the tracker at
~20–30 live items instead of ~80 stale ones.

**Cross-link both ways, but authority runs one way** (§5): the issue body opens with a link to its
`issues.md` section, and the doc bullet gains its issue number (`— #142`). The links are mutual so
either end is discoverable from the other; the *canonical statement* stays in the doc. Without the
back-link the doc silently becomes the stale copy.

**Never let a closed issue be the only record of a decision.** If an issue closes `wontfix`, the
reasoning goes into the doc's *Decided* section in the same change. Issue comments are not
discoverable six months later; the doc is.

## 2. Labels

Declared in **[`.github/labels.toml`](../.github/labels.toml)**, so the taxonomy is a
version-controlled guardrail rather than clicks in a web UI — consistent with how this repo already
treats lints, i18n completeness, and CSS tokens. TOML rather than YAML because `xtask` already has a
`toml` dependency and no YAML one; adding a parser for a label list is not worth a new dependency.

    cargo xtask labels            # show the plan (create / update / extra), change nothing
    cargo xtask labels --apply    # create and update to match the file

It **never deletes**: a label on GitHub but absent from the file is reported as `extra` and left alone,
because deleting a label silently strips it from every issue carrying it. GitHub's defaults are
deliberately among those extras.

### `area/*` — one per backlog H3 in `issues.md`

`records/person-family`, `records/places`, `records/notes`, `records/tags`, `records/media`,
`records/dna`, `records/cross-aggregate`, `frontend/shell`, `frontend/lists`, `frontend/keyboard`,
`frontend/pedigree`, `frontend/geography`, `frontend/gui-cli-parity`, `import/bulk`,
`import/assisted`, `import/round-trip`, `plugins/ui-vocabulary`, `plugins/trust`, `platform/perf`,
`platform/packaging`, `platform/deps`.

The area reorganization is what makes this mapping 1:1 and mechanical — an item's label follows from
where it already sits in the doc. Add `docs` and `i18n` as cross-cutting extras.

**Two exclusions the mapping depends on.** The H3s under **`## Decided — no action needed`**
(*Keyboard & shortcuts (ADR 0030 …)*, *Model & interchange*, *Architecture*) are **not** areas — they
group decisions, and nothing there is ever filed. Worth stating explicitly because those headings only
became H3s to satisfy markdownlint MD036, not because they name areas: a naive "one label per H3" sweep
would invent `area/model-interchange` and `area/architecture` for two headings with no work behind them,
and a second near-duplicate keyboard label. The rule is **one label per H3 under the four backlog H2s**
(*Records & data model*, *Frontend & interaction*, *Import, export & plugins*, *Platform & operations*)
— which is exactly the 21 above. Separately, **`## Bugs`** is a pointer section, not a place bullets
live: an open bug sits under the `###` area it affects and takes that `area/*` label plus `type/bug`.
`cargo xtask issue-sync` enforces that — a bullet directly under `## Bugs` is reported as misplaced,
with the area H3 named as the fix.

### `type/*` — what kind of work

`bug`, `feature`, `chore`, `refactor`, `test-gap`, `design-question`, `research`.

Two earn their place from the current backlog: **`test-gap`** (two shipped map fixes have no test
coverage — a real class this repo produces, since agents cannot drive the webview) and
**`design-question`** for the items the doc already marks as needing a design or product call (saved
searches, Repository media refs, column chooser).

### `priority/*` — when

`high`, `medium`, `low`. Three levels. Five-level schemes do not survive contact with a backlog
nobody grooms weekly.

### `status/*` — workflow state

`needs-triage` (auto-applied on open), `blocked`, `needs-design`.

### Three repo-specific labels that pay for themselves

- **`needs-adr`** — this repo's rule is that a gating ADR lands in the same cycle as the work it
  unblocks, and ADRs are immutable. Several backlog items are explicitly ADR-gated: surety-scheme
  cardinality, `SUBM`/`HEAD` metadata, the server/web work. Without this label they read as
  actionable when they are in fact blocked on a decision nobody has made.
- **`blocked/upstream`** — for `sqlx` 0.9 and `ed25519-dalek` 3.0.0. Both are waiting on third-party
  releases. Distinguishing them from ordinary `blocked` stops recurring re-investigation of a
  conclusion already reached and written down.
- **`manual-verify`** — this repo has a workflow state SSR tests cannot discharge: behaviour that only
  exists in a running webview (`document::eval`, CSS, the MapLibre canvas). Making it a label matters
  because this is exactly how an unverified claim slipped through once already — the `## Bugs` section
  asserted test coverage for five fixes when two had none.

  **Most of it is automatable, so scope the label to what actually needs a human.** `cargo xtask
  gui-pass` (see [`CLAUDE.md`](../CLAUDE.md#testing-the-gui)) runs the real GUI on an Xvfb display,
  drives it with `xdotool`, and asserts over screenshots — pan, zoom, click-to-place, overlay dismissal
  and static appearance are all reachable there, and the scenarios are TOML files, not Rust. What is
  left for a human is **feel**: pan/zoom smoothness, click latency, whether a save looks instant,
  slide-in motion. Software GL is not a GPU and a still image has no frame rate. The harness needs a
  graphical-capable machine, so it does not run in CI today. `manual-verify` means *that* residual, not
  "agents can't run the webview" — the reason this label used to give.

  Even the **window-manager close** is scriptable without a window manager: the `wm-close` step sends the
  toplevel a `WM_DELETE_WINDOW` ClientMessage, which GDK dispatches on its own, so the titlebar-✕ path
  (#281) is asserted by `wm-close-confirm.toml` rather than left to a human. What is left there is again
  feel — whether the hide-and-re-show flashes, and where focus lands after it.

`docs` now has an H3 home of its own — *Docs & repo tooling* under *Platform & operations* — because
`issue-sync` requires every bullet to sit under some `###` area to inherit a label from. `i18n` remains
purely cross-cutting.

Reuse the built-ins `bug`, `documentation`, `good first issue`, `help wanted`, `wontfix`,
`dependencies` (Dependabot already applies the last one).

## 3. Milestones

Milestones are **shipping vehicles, not phases.** The phases are done, and recreating them as
milestones would re-import the framing the doc reorganization just removed.

**The GUI is the main interface at 1.0.** That single constraint sets the order: parity and UI
correctness are *pre*-1.0 gates, not a follow-up cycle. Shipping a 1.0 whose primary interface cannot
export, cannot import a file, and silently discards an in-progress edit would be a worse outcome than
shipping later. So the packaging work sequences **last** — it is the least valuable milestone to reach
early, because there is no point packaging an interface that isn't finished.

The workspace is at `0.1.0` with no tags yet, so the two gates are numbered to make their order
unambiguous in GitHub's milestone list.

`0.8 — UI parity` shipped and is closed on GitHub; its narrative lives in
[`archive/completed-work.md`](archive/completed-work.md). A closed gate is dropped from this table
rather than kept as history — the archive is the record.

| Milestone | Contents |
| --- | --- |
| **`0.9 — UI stabilization`** | Bugfix and correctness before shipping. **Expected to grow substantially** — the list below is a floor, not a scope: most of what belongs here has not been found yet, because it takes real GUI use to surface. Highest first: **the close/quit confirm cannot save** — the confirm landed (#200) and an in-progress edit now survives leaving its tab (#239), but keeping the edit still means cancelling, finding the record, saving, and closing again. Then the outstanding manual webview pass, the record-picker listener leak, and the recent-list write racing a keyboard quit. |
| **`1.0`** | Release mechanics only (#210–#215): generate real release keys, verify `release.yml` end-to-end once billing is active, give `.deb` a default system plugin path (same fix as the duplicated/divergent embedded plugin-dir resolver), add the missing `[profile.release]`, and settle the cross-platform decision. |

**A milestone requires groomed, committed scope — not a theme.** Everything else — DNA depth, the
server/web work, the plugin-UI vocabulary tail, the ADR 0014 plugin-trust out-of-scope list, round-trip
gaps, performance work, the Norwegian-geography import, and the upstream-blocked dependencies — carries
**no milestone**. "Someday" is honestly encoded as `priority/low` with no milestone; an ungroomed `2.0`
looks like a commitment nobody made. This is deliberately a principle rather than a count: a count has
to be re-argued every time a gate ships, and the thing worth guarding is the commitment, not the
arithmetic.

## 4. Milestone contents

The remaining pre-1.0 gate, itemized from `issues.md` as it stands. Small enough to groom, which is the
point of filing only what is being worked on.

### `0.9 — UI stabilization` (17 so far)

Ordered by severity, not area. **This milestone is deliberately open-ended.** Ten issues is what the
audit could find by reading code; the rest came from using the GUI in earnest, which is exactly how it
was supposed to go — the 2026-07-31 manual pass on the map and the unsaved-work confirm nearly doubled
the list on its own. Treat the count as a floor.

| Item | Why it gates a release |
| --- | --- |
| [The Geography place list is undocumented and geometry-only](https://github.com/magne/genealogy/issues/256) | A place without geometry can never be selected, so it cannot be a draw target, and the list shrinks with the year with no label saying why |
| [Only one unsaved new record per category](https://github.com/magne/genealogy/issues/260) | Two new people cannot be sketched side by side; the draft's identity is its category, all the way down to the stash key |
| [A docked split mounts two detail panes, breaking `NavState`'s single-mount assumptions](https://github.com/magne/genealogy/issues/284) | One `⌘Z` retracts an assertion in **both** panes' records; fixed with #279, filed so the invariant has a home |

#256 and #260 came out of the 2026-07-31 manual pass — the map and the shell respectively; #281–#284
came out of the 2026-08-04 sweep alongside the #279 trace; the rest came from reading the code.

**#283 is closed and dropped from this table:** `[map]` config is now named providers
(`[map.providers.*]`, mirroring `[ai.providers]`, ADR 0033) resolved to a `MapSource` in
`genealogy-app`; the toolbar select offers exactly the built-in default plus what is configured, and
picking one persists it, resolves it, and calls `MapLibre`'s `setStyle` on the running map — no
remount — instead of writing config nothing ever read back. A second, unreported defect surfaced
while fixing this: both screens read/wrote `[map]` through `FileConfigStore::for_workspace`, whose
`config_path` is `None`, so the write had always landed nowhere; `map_config` now reads the
already-loaded global config directly.

**#282 is closed and dropped from this table:** the Point tool now commits through the same
`draft_geometry` helper Finish polygon uses, reaching the previously-dead `GeoPanel::CreateHere` path;
Clear was never a state bug (`MapDraft` and its `geo-draft` push were both correct) but a #252-shaped
compositor gap — a draft push reaching the map from outside any canvas gesture never got its scheduled
frame composited under `WebKitGTK`'s software-GL path, fixed with the same forced `redraw()` the #252
resize observer already uses; and a successful save now clears the draft in both screens, unifying
Geography (which never cleared it) and Place (which cleared it too early, at confirm, discarding the
drawing if Cancel followed) on one rule.

**#281 is closed and dropped from this table:** a `CloseRequested` handler in `QuitManager` now turns a
window-manager close — the titlebar `✕`, a session logout, `wmctrl -c` — into the same quit confirm
`⌘Q` raises, holding the native close back with `WindowCloseBehaviour::WindowHides` and re-showing the
window in the same event-loop iteration; with nothing unsaved the close is let through untouched.

**#206 is closed and dropped from this table:** `⌘S` is now `ShortcutAction::SaveRecord`, a `Global`,
rebindable chord dispatched by the shell and routed to the mounted pane through the existing
save-request machinery — listed by `?`, editable in Preferences, and working uniformly across every
aggregate screen and create draft instead of only the 5 that wired it by hand.

**#208 is closed and dropped from this table:** the 19 screen-local toasts (each with its own
`use_signal`, disappearing silently on a tab switch and shifting the layout it reported on) are unified
into one shell-owned notice channel, rendered by one positioned `.toast-layer` pinned to the bottom of
the work area — confirmations auto-dismiss after 6 s, errors stay until dismissed.

**#279 and #209 are closed by the docking PR** and dropped from this table: the detail-tab clicks and
the remembered tab were one missing piece of state between them. Worth recording what the trace
established, because #279's own diagnosis was wrong — duplicate element ids cannot make a Dioxus
handler inert (events route by `data-dioxus-id`); the cause was `.master-detail.split-2` collapsing at
exactly the app's default window width, plus a tab strip that moves when a pane halves, which is what
made the fixed-coordinate repro look like a dead click. The ruled-out hypothesis is now a *Decided*
entry in [`issues.md`](issues.md) so it is not reached for again.

**#285, #279's named residual, is closed and dropped from this table.** Writing the #279 scenarios
turned up a second mechanism that also made a click on the visible tab label do nothing, and it was
filed as a hit region sitting ~9–14px above the painted row. That diagnosis was wrong: column-scanning
the shots (`convert <shot> -crop 1xH+X+Y +repage txt:-`, not reading them by eye) showed both the
"works" and the "does nothing" coordinates were inside the *same* painted button, and a click ladder
found the boundary at y=245/247 with the lower band visibly scrolling the strip sideways. The cause was
`.tabs`' own horizontal scrollbar, whose `WebKitGTK` hit rectangle is ~20px of a 36px row; it is there
whenever the strip overflows, so the minimal reproduction needs no dock at all — a single pane at the
app's default 1280px window already does it, which `tab-strip-overflow.toml` now locks in. The
corrected measurement is a *Decided* entry in [`issues.md`](issues.md).

### Not in either milestone

DNA depth, round-trip gaps, performance/scale, the ADR 0014 plugin-trust out-of-scope list, the
plugin-UI vocabulary tail, upstream-blocked dependencies, assisted-import residuals, saved searches /
column chooser / list virtualization, geocoding, the `place_parent` index, and the
`geography_toolbar` argument cleanup. All `priority/low` or `priority/medium`, no milestone.

The **Norwegian-geography import** belongs here too, and deliberately so. Its model changes are gated by
an unwritten ADR 0031 and it is scoped in `roadmap.md` as closing Phase 9's declared residuals, not as a
shipping gate — putting a feature body with an unaccepted gating ADR ahead of UI correctness would
invert the ordering rule in §3. Only its three by-products above are milestoned.

**One item to close, not file:** *DNA match views in the UI* is **stale**. The screens exist
(`screens/dna_match.rs` has Segments and Ancestors tabs with per-row edit/retract, plus
`tests/dna_match_detail.rs`). Delete the bullet rather than filing it. The other five DNA bullets are
accurate and about *depth*, not views — `DnaTestState` really does lack `account`/`date_tested`/
`snp_count`, and `citations: Vec::new()` really is hardcoded in three places.

## 5. Keeping the doc and the tracker in sync

**Sync is one-way: doc → GitHub.** The doc is the source; an issue is a working copy of one bullet.
Two-way sync between prose and a tracker is what produces two stale records instead of one good one.

- Filing appends the number to the bullet: `— #142`. That is the whole linkage.
- The issue body opens with a link to the bullet's **H3 anchor** (`docs/issues.md#places`), not a line
  number — line numbers move on every edit.
- **Discussion accretes on the issue; the canonical statement stays in the doc.** If discussion changes
  the shape of the work, update the bullet in the same PR that acts on it. Do not maintain both.

**Drift check.** `cargo xtask issue-sync` — wired into `cargo xtask check`, so prek and CI cover it
alongside `i18n-check`, `css-check`, and `input-guard`:

- *Offline* (prek + CI, every commit): the doc's own invariants — every `— #N` well-formed, no
  duplicate numbers, no reference to a number below the lowest filed issue.
- *Online* (`--online`, weekly scheduled job with `GITHUB_TOKEN`): reconcile both directions and report
  drift — an open issue whose bullet is gone, a bullet referencing a closed issue, an open issue with no
  bullet at all.

The offline half gates every commit; the online half is run by hand (or scheduled, once billing
allows) because it needs a token and network and drift is slow.

A reference sits at the **end** of a bullet's block, after however many lines of prose — that is where
it reads naturally, so the parser closes each block at the next bullet, heading, or blank line and
reads the reference off the last line. Getting this wrong is not hypothetical: the first version read
only the title line and silently found nothing, and the fix after that closed each block twice, so an
area's *last* bullet lost its reference. Both are now regression-tested.

## 6. What happens when work completes

This is the question that decides whether the doc stays reasonable, and the answer is **not** "archive
everything". Copying every closed item into `archive/completed-work.md` just relocates the bloat: that
file becomes the unreadable one instead. **The archive is for milestone-scale completions, not for
items.**

Three outcomes, by what the closure actually taught us:

| Closure | What happens to the bullet |
| --- | --- |
| **Done as specified** | **Delete it.** The PR and the commit are the record. `git log -S'<bullet title>' -- docs/issues.md` recovers the full history whenever anyone asks "why was this ever a concern". No archive entry — the repo's standard is not to duplicate what git already records. |
| **Closed with a decision** (wontfix, turned out impossible, deferred indefinitely) | **Move it to `Decided — no action needed`**, rewritten as the decision rather than the task. This is the one section that *should* grow: it is what stops the item being re-raised in a year. |
| **A whole milestone lands** | One narrative entry in [`archive/completed-work.md`](archive/completed-work.md), matching how the phases are recorded there — what changed, which PRs, and any honest residuals. |

Deleting the last bullet of an area **does not delete the `###` heading**. The heading keeps a
one-line "No open items." note instead, the way `## Bugs` already does: the area taxonomy is a map of
the product's surfaces, not a work queue, so an empty area says "nothing open here" and is where the
next item lands. Three concrete reasons — the `area/*` label lives in
[`.github/labels.toml`](../.github/labels.toml) and is unaffected by the heading (and
`cargo xtask labels` never deletes), every filed issue body deep-links the H3 anchor
(`docs/issues.md#places`) including the closed ones, and `cargo xtask issue-sync` tracks areas per
bullet so an area with no bullets reports no drift. Drop the heading only when the *area itself* stops
existing, and retire its label in the same commit.

So `issues.md` **shrinks monotonically** as work lands, except for `Decided`, which is reference
material rather than backlog. That is the property worth protecting: the file stays reasonable because
finished work leaves it entirely, not because it is filed somewhere else in the same repo.

On the GitHub side there is nothing to manage. Closed issues drop out of the default view and stay
searchable; a tracker of closed issues is not a maintenance burden the way a growing document is. The
bloat risk was only ever the doc.

## 7. Triage

1. Issue opens → `status/needs-triage` (automated).
2. Once per cycle, for each new issue: assign **exactly one** `area/*`, one `type/*`, one
   `priority/*`; drop `needs-triage`; add `needs-adr` / `blocked/*` / `manual-verify` if they apply.
   Set a milestone **only** if it is committed to a release.
3. **Definition of ready** — an issue may be picked up when it has acceptance criteria or a repro, it
   names the crate and file, and any gating ADR is *accepted* (not merely proposed).
4. **Closing** — link the PR. By-design closes go `wontfix` **and** add a *Decided* entry in
   `issues.md`.

## 8. Migration mechanics

`gh issue create` per curated item. Bodies seed well from the doc: most bullets already carry the
file/symbol pointers and a suggested implementation, so the issue is mostly copy-paste.

Add issue **templates** (`.github/ISSUE_TEMPLATE/`) as forms: `bug`, `feature`,
`design-question` — each with an area dropdown mirroring `area/*`, so new issues arrive
pre-triaged rather than needing a first pass to classify.

Suggested automation, in rough order of value: auto-`needs-triage` on open; the `labels.yml` sync
action; a stale-bot **only** for `status/needs-triage` (never for `blocked/upstream` or
`priority/low`, which are correctly long-lived).
