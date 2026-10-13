# Issue tracking on GitHub

- **Status:** Applied
- **Date:** 2026-07-27 (revised 2026-10-13: GitHub Issues became the canonical home of open work)
- **Audience:** anyone filing, triaging, or closing an issue
- **Companion:** [`decisions.md`](decisions.md), the record of deliberate non-tasks;
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
| `docs/decisions.md` | **Decided — no action needed**: deliberate non-tasks, recorded so they are not re-raised. |

New work is filed as an issue, through one of the templates in `.github/ISSUE_TEMPLATE/` (bug, feature,
design question). That is what lets a community report and a maintainer's note take the same path.

**Never let a closed issue be the only record of a decision.** If an issue closes as by-design,
impossible or deferred indefinitely, the reasoning goes into [`decisions.md`](decisions.md) in the same
change. Issue comments are not discoverable six months later; the doc is.

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

One per surface of the product, grouped `area/records/*`, `area/frontend/*`, `area/import/*`,
`area/plugins/*` and `area/platform/*`, plus `docs` and `i18n`; `.github/labels.toml` is the list, and the
issue templates' area dropdown mirrors it. An area with no open issue keeps its label — the taxonomy
maps the product, not a work queue — and is retired only when the surface itself goes.

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

## 4. When work completes

| Closure | What happens |
| --- | --- |
| **Done as specified** | Close the issue from the PR. The PR and its commits are the record. |
| **Closed with a decision** (wontfix, impossible, deferred indefinitely) | Close as not planned with the `wontfix` label **and** add the decision to [`decisions.md`](decisions.md), written as the decision rather than the task. |
| **A whole milestone lands** | Close the milestone and add the narrative entry (§3). |

`decisions.md` is reference material and should grow. Closed issues need no management: they drop out
of the default view and stay searchable.

## 5. Triage

1. Issue opens → `status/needs-triage` (automated).
2. Once per cycle, for each new issue: assign one `area/*`, one `type/*`, one `priority/*`; drop
   `needs-triage`; add `needs-adr` / `blocked/*` / `manual-verify` if they apply. Set a milestone
   **only** if it is committed to a release (§3).
3. **Definition of ready** — an issue may be picked up when it has acceptance criteria or a repro, it
   names the crate and file, and any gating ADR is *accepted* (not merely proposed).
4. **Closing** — link the PR. By-design closes follow §4.

## 6. Templates and automation

The issue templates are forms with an area dropdown mirroring `area/*`, so issues arrive pre-sorted.
In order of value: auto-`needs-triage` on open; the `labels.yml` sync; a stale-bot **only** for
`status/needs-triage` — never for `blocked/upstream` or `priority/low`, which are correctly long-lived.
