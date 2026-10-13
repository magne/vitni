# Issue tracking on GitHub

- **Status:** Applied
- **Date:** 2026-07-27 (revised 2026-10-13: GitHub Issues became the canonical home of open work)
- **Audience:** anyone filing, triaging, or closing an issue
- **Companion:** [`issues.md`](issues.md), the legacy backlog and the record of decisions;
  [`roadmap.md`](roadmap.md) owns phase detail.

This document describes **how** work is tracked — where it lives, how it is labelled, scheduled and
closed. It does not list the work. What is open, and what a milestone holds, is on GitHub: the
[issues](https://github.com/magne/vitni/issues) and the
[milestones](https://github.com/magne/vitni/milestones). A change to one issue never needs an edit here.

## 1. Where work lives

| | Owns |
| --- | --- |
| GitHub Issues | **Every open item** — bugs, features, design questions, chores — whoever reports it. The issue body is the canonical statement of the work; discussion accretes on it. |
| GitHub milestones | Which issues a release is committed to (§3). |
| `docs/issues.md` | **Decided — no action needed**: deliberate non-tasks, recorded so they are not re-raised. Until the migration (#615) lands, also the legacy backlog: bullets written before issues became canonical. |

New work is filed as an issue, through one of the templates in `.github/ISSUE_TEMPLATE/` (bug, feature,
design question), and gets no `issues.md` bullet. That is what lets a community report and a
maintainer's note take the same path.

**Never let a closed issue be the only record of a decision.** If an issue closes as by-design,
impossible or deferred indefinitely, the reasoning goes into `issues.md`'s *Decided* section in the same
change. Issue comments are not discoverable six months later; the doc is.

### The legacy backlog, until #615

`issues.md` still holds bullets from before this rule. A filed bullet ends with its issue number
(`— #142`), and its issue body opens with a link to the bullet's H3 anchor. While both exist the issue
is canonical: a change of shape is made on the issue, and the bullet is updated or deleted in the next
change that touches it. A bullet with no number is not yet filed; #615 files or retires the rest.
`cargo xtask issue-sync` (§4) keeps the two ends honest meanwhile.

## 2. Labels

Declared in **[`.github/labels.toml`](../.github/labels.toml)**, so the taxonomy is a
version-controlled guardrail rather than clicks in a web UI. TOML because `xtask` already depends on
`toml`.

    cargo xtask labels            # show the plan (create / update / extra), change nothing
    cargo xtask labels --apply    # create and update to match the file

`.github/workflows/labels.yml` applies the file on a push that touches it. Neither path **deletes**: a
label on GitHub but absent from the file is reported as `extra` and left alone, because deleting a label
silently strips it from every issue carrying it. GitHub's defaults are among those extras.

Every issue carries **exactly one** `area/*`, one `type/*` and one `priority/*`.

### `area/*` — which surface of the product

One per backlog `###` heading in `issues.md` under its four backlog H2s (*Records & data model*,
*Frontend & interaction*, *Import, export & plugins*, *Platform & operations*), plus `docs` and `i18n`.
The headings under *Decided* group decisions and are not areas. An area whose last item closes keeps its
heading with a "No open items." note — the taxonomy maps the product's surfaces, not a work queue —
and is retired only when the surface itself goes, together with its label.

### `type/*` — what kind of work

`bug`, `feature`, `chore`, `refactor`, `test-gap`, `design-question`, `research`.

- **`test-gap`** — behaviour that exists but nothing asserts, a class this repo produces because some
  behaviour lives only in `format!`-built JavaScript in a live webview.
- **`design-question`** — needs a design or product call before it needs code.

### `priority/*` — when

`high`, `medium`, `low`. Three levels; five-level schemes do not survive a backlog nobody grooms weekly.
`low` with no milestone is the honest encoding of "someday".

### `status/*` — workflow state

`needs-triage` (applied on open), `blocked`, `needs-design`.

### Repo-specific labels

- **`needs-adr`** — a gating ADR lands in the same cycle as the work it unblocks, and ADRs are
  immutable. Without the label an ADR-gated item reads as actionable when it is blocked on a decision
  nobody has made.
- **`blocked/upstream`** — waiting on a third-party release. Distinguishing it from `blocked` stops
  re-investigation of a conclusion already reached and written down.
- **`at-scale`** — pays off only at high data volume (an index, pagination, viewport loading). Real work,
  but premature until a measured workspace is slow; the label says what will trigger it, so the item is
  neither closed nor mistaken for current work.
- **`manual-verify`** — behaviour only a human can confirm. Most webview behaviour is automatable with
  `cargo xtask gui-pass` (see [`CLAUDE.md`](../CLAUDE.md#testing-the-gui)): it drives the real GUI on
  Xvfb and asserts over screenshots and an element probe, including the window-manager close. What is
  left is **feel** — pan/zoom smoothness, click latency, motion — because software GL is not a GPU and a
  still image has no frame rate. The label means that residual.

GitHub's built-ins `bug`, `documentation`, `good first issue`, `help wanted`, `wontfix` and
`dependencies` (applied by Dependabot) stay in use.

## 3. Milestones

Milestones are **shipping vehicles, not phases**. A milestone holds groomed, committed scope; its
description says what it delivers, and its issues are its contents. Neither is copied here.

- **Committed scope only.** Set a milestone only when the issue is committed to that release. Everything
  else carries none — an ungroomed `2.0` looks like a commitment nobody made.
- **Order by what the release needs.** The GUI is the main interface at 1.0, so UI correctness and
  import integrity are pre-1.0 gates and the packaging work sequences last: there is no point packaging
  an interface that is not finished.
- **Numbers are versions, not strings.** `0.10` follows `0.9`, even where GitHub's title sort shows it
  first.
- **A landed milestone** is closed on GitHub and gets one narrative entry in
  [`archive/completed-work.md`](archive/completed-work.md): what changed, which PRs, honest residuals.

## 4. Keeping `issues.md` and the tracker in sync

`cargo xtask issue-sync` guards the legacy linkage, and goes when the legacy backlog does (#615):

- *Offline* (in `cargo xtask check`, so prek and CI run it): every `— #N` well-formed, no number claimed
  twice, every backlog bullet under an `###` area, none directly under `## Bugs`.
- *Online* (`--online`, by hand with your own `gh` auth): a bullet pointing at a closed issue, an open
  issue whose bullet is gone. An open issue with no bullet is normal now — new work has none.

The reference sits at the **end** of a bullet's block, after its prose: the parser closes each block at
the next bullet, heading or blank line and reads the reference off the last line.

## 5. When work completes

| Closure | What happens |
| --- | --- |
| **Done as specified** | Close the issue from the PR. If a legacy bullet exists, **delete it** in the same PR — the PR and commit are the record, and `git log -S'<title>' -- docs/issues.md` recovers it. |
| **Closed with a decision** (wontfix, impossible, deferred indefinitely) | Close `wontfix` **and** add the decision to `issues.md`'s *Decided* section, rewritten as the decision rather than the task. |
| **A whole milestone lands** | Close the milestone and add the narrative entry (§3). |

So `issues.md` shrinks as work lands, except *Decided*, which is reference material and should grow.
Closed issues need no management: they drop out of the default view and stay searchable.

## 6. Triage

1. Issue opens → `status/needs-triage` (automated).
2. Once per cycle, for each new issue: assign one `area/*`, one `type/*`, one `priority/*`; drop
   `needs-triage`; add `needs-adr` / `blocked/*` / `manual-verify` if they apply. Set a milestone
   **only** if it is committed to a release (§3).
3. **Definition of ready** — an issue may be picked up when it has acceptance criteria or a repro, it
   names the crate and file, and any gating ADR is *accepted* (not merely proposed).
4. **Closing** — link the PR. By-design closes follow §5.

## 7. Templates and automation

The issue templates are forms with an area dropdown mirroring `area/*`, so issues arrive pre-sorted.
In order of value: auto-`needs-triage` on open; the `labels.yml` sync; a stale-bot **only** for
`status/needs-triage` — never for `blocked/upstream` or `priority/low`, which are correctly long-lived.
