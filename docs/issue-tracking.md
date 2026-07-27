# Issue tracking on GitHub (proposal)

- **Status:** Proposed — **not applied.** No labels, milestones, or issues have been created.
- **Date:** 2026-07-27
- **Audience:** whoever decides to move the backlog onto GitHub
- **Companion:** [`issues.md`](issues.md) is the backlog this describes; [`roadmap.md`](roadmap.md)
  owns phase detail.

## Starting state

The repository has **only GitHub's default labels**, **no milestones**, and **no open issues** (the
one referenced issue, #38, is closed). So the taxonomy below is greenfield — nothing has to be
migrated or reconciled.

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

Define them in **`.github/labels.yml`** with a sync action, so the taxonomy is a version-controlled
guardrail rather than clicks in a web UI — consistent with how this repo already treats lints, i18n
completeness, and CSS tokens.

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
— which is exactly the 21 above. Separately, **`## Bugs`** has no H3s by design: a bug takes its
`area/*` from whichever area it affects, plus `type/bug`.

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
- **`manual-verify`** — this repo has a workflow state agents cannot discharge: "needs a manual
  webview pass (agents can't run libwebkit2gtk)". Making it a label matters because this is exactly
  how an unverified claim slipped through once already — the `## Bugs` section asserted test coverage
  for five fixes when two had none.

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

| Milestone | Contents |
| --- | --- |
| **`0.8 — UI parity`** | Every operation a user can reach from the GUI. The four *GUI ⇄ CLI parity* gaps (bulk export, bulk import + target selection, projection rebuild, Postgres workspace creation), the research-note UI **and** its missing mockup, and family child-removal. Plus the three items neither frontend can do today but the GUI must once it is the reference surface: place succession, tag restrictions, workspace-scope surety labels. |
| **`0.9 — UI stabilization`** | Bugfix and correctness before shipping. Highest first: **dirty saved-record edits are discarded silently** on close/quit — data loss in the primary interface. Then the `Modal` focus trap and click-away scrim (a keyboard user can tab into the inert background), the two shipped map fixes with no test coverage, the outstanding manual webview pass, the record-picker listener leak, the recent-list write racing a keyboard quit, `⌘S` outside the shortcut map, and the three *Shell, tabs & notifications* ease-of-use items (live list updates, toasts, remembered tab). |
| **`1.0`** | Release mechanics only: generate real release keys, verify `release.yml` end-to-end once billing is active, give `.deb` a default system plugin path (same fix as the duplicated/divergent embedded plugin-dir resolver), add the missing `[profile.release]`, and settle the cross-platform decision. |

**Do not create a fourth.** Everything else — DNA depth, the server/web work, the plugin-UI vocabulary
tail, the ADR 0014 plugin-trust out-of-scope list, round-trip gaps, performance work, and the
upstream-blocked dependencies — carries **no milestone**. "Someday" is honestly encoded as
`priority/low` with no milestone; an ungroomed `2.0` looks like a commitment nobody made.

## 4. Milestone contents

The two pre-1.0 gates, itemized from `issues.md` as it stands. ~20 issues total — small enough to
groom, which is the point of filing only what is being worked on.

### `0.8 — UI parity` (10)

| Item | Area |
| --- | --- |
| Bulk export is CLI-only | `frontend/gui-cli-parity` |
| Bulk import is CLI-only, and target selection has no GUI shape | `frontend/gui-cli-parity` |
| Projection rebuild is CLI-only | `frontend/gui-cli-parity` |
| Postgres workspaces can only be created from the CLI | `frontend/gui-cli-parity` |
| Research notes have no GUI at all (+ the missing mockup) | `records/notes` |
| A child cannot be removed from a family in the GUI | `records/person-family` |
| Place succession can be read but never written | `records/places` |
| Tag has no restrictions path | `records/tags` |
| Workspace-scope surety labels are read but unwritable | `records/cross-aggregate` |
| Interactive Set/Clear region on every owner (wired on Person only) | `records/media` |

The last four are *not* CLI-parity gaps — neither frontend can do them today. They belong here anyway:
once the GUI is the reference surface, "the CLI can't either" stops being a defence.

### `0.9 — UI stabilization` (10)

Ordered by severity, not area.

| Item | Why it gates a release |
| --- | --- |
| Dirty saved-record edits are not confirmed | **Silent data loss** in the primary interface — closing a tab or quitting discards an in-progress edit of a saved record with no prompt |
| `Modal`/`SidePanel` overlay follow-ups | No focus trap: a keyboard user tabs out of the confirm dialog into the inert background. Accessibility defect on the app's only modal |
| Two shipped map fixes have no test coverage | `type/test-gap` — both would regress undetected |
| Manual webview pass outstanding | `manual-verify` — the interactive map canvas has never been exercised |
| Record-picker scroll-listener cleanup | Leaks one inert JS listener per clear/re-search cycle |
| "Jump back in" recent-list write has no close/quit hook | A keyboard quit races the debounced write |
| `⌘S` lives outside the shortcut map | Save is neither listed by `?` nor rebindable — inconsistent with every other binding |
| Live list updates on create | A created record does not appear until manual refresh |
| Toast notifications | No feedback channel for completed actions |
| Remember the open record's tab | Tab resets on every navigation |

Optional twelfth: *Point tool has no confirm step in the Geography tool* — a known inconsistency with
the Place Map editor, cheap to close alongside the map work.

### Not in either milestone

DNA depth, round-trip gaps, performance/scale, the ADR 0014 plugin-trust out-of-scope list, the
plugin-UI vocabulary tail, upstream-blocked dependencies, assisted-import residuals, saved searches /
column chooser / list virtualization, geocoding, the `place_parent` index, and the
`geography_toolbar` argument cleanup. All `priority/low` or `priority/medium`, no milestone.

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

**Drift check.** A `cargo xtask issue-sync` lint fits this repo, which already gates `i18n-check`,
`css-check`, `input-guard`, and the framework-free boundary the same way:

- *Offline* (prek + CI, every commit): the doc's own invariants — every `— #N` well-formed, no
  duplicate numbers, no reference to a number below the lowest filed issue.
- *Online* (`--online`, weekly scheduled job with `GITHUB_TOKEN`): reconcile both directions and report
  drift — an open issue whose bullet is gone, a bullet referencing a closed issue, an open issue with no
  bullet at all.

Weekly rather than per-PR because drift is slow and the online path needs a token and network. If even
that is more machinery than wanted, the honest fallback is a line in the PR template — "if this closes
a `docs/issues.md` item, remove or move the bullet" — and accepting that the doc is only as current as
the discipline. Say which, rather than assuming the lint will get written.

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
