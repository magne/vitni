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

[`issues.md`](issues.md) currently holds **92 bullets: 79 actionable and 13 that are not tasks at
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

**Keep the two linked in both directions:** the issue body opens with a link to its `issues.md`
section, and the doc bullet gains its issue number (`— #142`). Without the back-link the doc silently
becomes the stale copy.

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

## 4. Triage

1. Issue opens → `status/needs-triage` (automated).
2. Once per cycle, for each new issue: assign **exactly one** `area/*`, one `type/*`, one
   `priority/*`; drop `needs-triage`; add `needs-adr` / `blocked/*` / `manual-verify` if they apply.
   Set a milestone **only** if it is committed to a release.
3. **Definition of ready** — an issue may be picked up when it has acceptance criteria or a repro, it
   names the crate and file, and any gating ADR is *accepted* (not merely proposed).
4. **Closing** — link the PR. By-design closes go `wontfix` **and** add a *Decided* entry in
   `issues.md`.

## 5. Migration mechanics

`gh issue create` per curated item. Bodies seed well from the doc: most bullets already carry the
file/symbol pointers and a suggested implementation, so the issue is mostly copy-paste.

Add issue **templates** (`.github/ISSUE_TEMPLATE/`) as forms: `bug`, `feature`,
`design-question` — each with an area dropdown mirroring `area/*`, so new issues arrive
pre-triaged rather than needing a first pass to classify.

Suggested automation, in rough order of value: auto-`needs-triage` on open; the `labels.yml` sync
action; a stale-bot **only** for `status/needs-triage` (never for `blocked/upstream` or
`priority/low`, which are correctly long-lived).
