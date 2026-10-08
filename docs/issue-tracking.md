# Issue tracking on GitHub

- **Status:** **Applied 2026-07-27; updated 2026-10-08**: a second `0.9` and `0.10` grooming pass
  (2026-10-04: the unfiled `0.9` and `0.10` bullets filed; 2026-10-01: `0.9.1 — Fixture provenance` landed; 2026-09-30: that milestone; 2026-09-27: the `0.10` milestone and two area labels). 42 labels and the
  issue-template forms exist, alongside `.github/labels.toml` and `cargo xtask issue-sync`. `0.8 — UI
  parity` and `0.9.1 — Fixture provenance` shipped; the three remaining gates are `0.9`, `0.10` and
  `1.0`.
- **Date:** 2026-07-27 (updated 2026-10-08)
- **Audience:** anyone filing, triaging, or closing an issue
- **Companion:** [`issues.md`](issues.md) is the backlog this describes; [`roadmap.md`](roadmap.md)
  owns phase detail.

## Starting state

Before this was applied the repository had **only GitHub's default labels**, **no milestones**, and
**no open issues** (the one referenced issue, #38, was closed) — so the taxonomy below was designed
greenfield, with nothing to migrate or reconcile.

**One note about the automation.** `.github/workflows/labels.yml` reconciles the labels from
`.github/labels.toml` on a push that touches it, or on `workflow_dispatch`; `cargo xtask labels
--apply` does the same thing locally and stays the quicker path while iterating. The doc↔tracker
drift check is an **xtask** rather than a workflow so the same command gates a commit through prek
and runs in CI; only its online half (§5) is deliberately left to a person.

[`issues.md`](issues.md) currently holds **111 bullets: 92 actionable and 19 that are not tasks at
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
`records/dna`, `records/cross-aggregate`, `frontend/shell`, `frontend/records`, `frontend/lists`,
`frontend/keyboard`, `frontend/pedigree`, `frontend/geography`, `frontend/gui-cli-parity`,
`import/bulk`, `import/assisted`, `import/matching`, `import/round-trip`, `plugins/ui-vocabulary`,
`plugins/trust`, `platform/perf`, `platform/packaging`, `platform/backup`, `platform/deps`.

`frontend/records` and its H3 (*Record detail & shared tabs*) were added on 2026-08-12: the 13 detail
screens share their tab bodies, so a defect in one lands on every aggregate at once, and folding those
into `frontend/shell` would have put the app tabstrip and the in-record tabs — two unrelated surfaces —
behind one label. `records/tags` gained its first H3 in the same pass, having been declared with no
home since the taxonomy was applied.

`import/matching` (*Record matching & identity*) and `platform/backup` (*Backup & restore*) were added
on 2026-09-27 with the `0.10 — Record matching` milestone (§4). Matching spans the importers, the core
and the UI, and has its own ADRs (0037–0040), so filing it under `import/bulk` would hide it inside
reconciliation. Backup is the project's one compatibility surface (ADR 0041), not a packaging concern.

The area reorganization is what makes this mapping 1:1 and mechanical — an item's label follows from
where it already sits in the doc. Add `docs` and `i18n` as cross-cutting extras.

**Two exclusions the mapping depends on.** The H3s under **`## Decided — no action needed`**
(*Keyboard & shortcuts (ADR 0030 …)*, *Model & interchange*, *Architecture*) are **not** areas — they
group decisions, and nothing there is ever filed. Worth stating explicitly because those headings only
became H3s to satisfy markdownlint MD036, not because they name areas: a naive "one label per H3" sweep
would invent `area/model-interchange` and `area/architecture` for two headings with no work behind them,
and a second near-duplicate keyboard label. The rule is **one label per H3 under the four backlog H2s**
(*Records & data model*, *Frontend & interaction*, *Import, export & plugins*, *Platform & operations*)
— which is exactly the 24 above. Separately, **`## Bugs`** is a pointer section, not a place bullets
live: an open bug sits under the `###` area it affects and takes that `area/*` label plus `type/bug`.
`cargo xtask issue-sync` enforces that — a bullet directly under `## Bugs` is reported as misplaced,
with the area H3 named as the fix.

### `type/*` — what kind of work

`bug`, `feature`, `chore`, `refactor`, `test-gap`, `design-question`, `research`.

Two earn their place from the current backlog: **`test-gap`** (the zoom-interpolated marker radius has
no witness in any test — a real class this repo produces, since the behaviour exists only inside
`format!`-built JavaScript in a live webview) and **`design-question`** for the items the doc marks as
needing a design or product call (the Attach-versus-Add model #314, saved searches, Repository media
refs, column chooser).

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

The workspace is at `0.1.0` with no tags yet, so the gates are numbered to make their order
unambiguous. The numbers are versions, not strings: `0.10` follows `0.9`, even where a title sort in
GitHub's milestone list shows it first. The order is the one in the table below.

**`0.10 — Record matching` is a pre-1.0 gate because import currently corrupts data.** A second,
unrelated GEDCOM file silently resolves its people onto the first file's people, and a re-import
duplicates every place and source. A user of the Digitalarkivet flow reports duplicated persons as
well. A 1.0 whose import can merge strangers is worse than a later 1.0. The milestone also delivers the
backup format that 1.0 freezes (ADR 0041), so it must land before the `1.0` release mechanics.

`0.8 — UI parity` and `0.9.1 — Fixture provenance` shipped; their narratives live in
[`archive/completed-work.md`](archive/completed-work.md). A closed gate is dropped from this table
rather than kept as history — the archive is the record.

| Milestone | Contents |
| --- | --- |
| **`0.9 — UI stabilization`** | Bugfix and correctness before shipping. **Expected to grow substantially** — the list below is a floor, not a scope: most of what belongs here has not been found yet, because it takes real GUI use to surface. The 2026-08-12 walkthrough proved that twice over: the milestone had reached zero open issues, and one pass through the GUI refilled it with 15. Highest first: attaching a note or media on seven record kinds discarding the attach form's provenance (#424). |
| **`0.10 — Record matching`** | Import that never duplicates and never guesses: record origins and import runs, the matching engine, identity decisions, staged import with side-by-side review, and versioned workspace backup (ADRs 0037–0041). Groomed in full in §4, in dependency order. |
| **`1.0`** | Release mechanics only (#210–#215): generate real release keys, verify `release.yml` end-to-end on the first real tag, give `.deb` a default system plugin path (same fix as the duplicated/divergent embedded plugin-dir resolver), add the missing `[profile.release]`, and settle the cross-platform decision. Also the deletion of the pre-1.0 backup upgraders, which freezes backup format v1 (#392, ADR 0041 §4). |

**A milestone requires groomed, committed scope — not a theme.** Everything else — DNA depth, the
server/web work, the plugin-UI vocabulary tail, the ADR 0014 plugin-trust out-of-scope list, round-trip
gaps, performance work, the Norwegian-geography import, and the upstream-blocked dependencies — carries
**no milestone**. "Someday" is honestly encoded as `priority/low` with no milestone; an ungroomed `2.0`
looks like a commitment nobody made. This is deliberately a principle rather than a count: a count has
to be re-argued every time a gate ships, and the thing worth guarding is the commitment, not the
arithmetic.

## 4. Milestone contents

The remaining pre-1.0 gates, itemized from `issues.md` as they stand. Each is small enough to groom,
which is the point of filing only what is being worked on.

### `0.9 — UI stabilization` (8 open)

Ordered by severity, not area. **This milestone is deliberately open-ended, and the 2026-08-12
walkthrough is the proof.** Every issue in the previous round closed, leaving the milestone empty — and
one pass through the real GUI, looking at nothing but what a user sees, refilled it with 15. Ten of
those are defects no SSR test could have caught, because they are about what reaches the DOM, the
stylesheet or the webview rather than what the view logic decided. Treat the count as a floor again.

**Open now (2026-10-04).** #424 and #382 are what is left of that round. The other four were filed on
2026-10-04 from bullets already in `issues.md`: one shipped screen that contradicts its mockup, two
mockup/sheet drifts against the rule that the mockups describe shipped behaviour, and the `gui-pass`
flakes that make every scenario's first assertion unreliable.

| Item | Why it gates a release |
| --- | --- |
| [Attaching a note or media on seven record kinds discards the attach form's provenance](https://github.com/magne/vitni/issues/424) | The reason, surety and citations a user enters are silently lost from History, so the audit trail records less than the user asserted |
| [A date row's four controls stack instead of sitting on one line](https://github.com/magne/vitni/issues/500) | Every date field on all 13 record screens draws a 5-row block where the mockup draws one row |
| [Ten mockups quote a History note no shipped string says](https://github.com/magne/vitni/issues/501) | The mockups advertise a competitor claim the product does not make |
| [The mockup sheet is only partly the superset it is documented to be](https://github.com/magne/vitni/issues/502) | 24 app rules have no mockup counterpart and nothing gates the sheet as a whole; needs a decided rule first |
| [Fast typing can lose characters in record text fields](https://github.com/magne/vitni/issues/382) | Typed text silently differs from what was entered; unconfirmed outside Xvfb |
| [`gui-pass` occasionally grabs a blank first shot](https://github.com/magne/vitni/issues/503) | The harness that tests this milestone's class of defect is flaky at its first assertion |
| [`gui-pass` steps address elements by window pixels, not by id](https://github.com/magne/vitni/issues/533) | Every layout change this milestone makes breaks unrelated scenarios until their pixels are re-read |
| [No merge/conflict mockup for reconciled fields](https://github.com/magne/vitni/issues/534) | No agreed design shows a user that an import overwrote one of their values; needs a design first |
| [History does not show the value an entry recorded or replaced](https://github.com/magne/vitni/issues/545) | Seventeen mockup History items name a value the app never shows; split from #534 |

**Added 2026-10-08.** #533 and #534 come from the second grooming pass. #533 is tooling, here on the
vehicle rule like #322: the walkthroughs that refill this milestone move the layout, and each move
re-calibrates scenarios that test something else.

The 2026-08-12 round, closed except #424:

| Item | Why it gated a release |
| --- | --- |
| [A save run whose target leaves the strip hangs](https://github.com/magne/vitni/issues/302) | Save all can wedge the quit/close path with no way out and no message |
| [Attaching a note or media on seven record kinds discards the attach form's provenance](https://github.com/magne/vitni/issues/424) | The reason, surety and citations a user enters are silently lost from History, so the audit trail records less than the user asserted |
| [The shared record tabs have no common layout contract](https://github.com/magne/vitni/issues/303) | Explanations below buttons, gone entirely when empty, and an add bar with no CSS rule at all — on all 13 screens |
| [Attached records have four different presentations](https://github.com/magne/vitni/issues/304) | An attached note can be neither read nor opened from the record that references it |
| [A ghost row action disappears on the hovered row](https://github.com/magne/vitni/issues/305) | Detach/Remove lose every visual affordance at the moment they are being aimed at |
| [A record's History tab describes an import in the Dashboard's words](https://github.com/magne/vitni/issues/306) | One person's history claims several *records* were imported; the audit trail must not misreport its own scope |
| [Closing a pristine draft tab still raises the unsaved-work confirm](https://github.com/magne/vitni/issues/307) | Discarding nothing needs a decision, and an untouched draft makes `⌘Q` claim unsaved work |
| [A draft tab and the `+` menu read "New People"](https://github.com/magne/vitni/issues/308) | Plural rail labels in a singular slot; ungrammatical in `no` for every category |
| [The Media record diverges from its mockup](https://github.com/magne/vitni/issues/309) | The mockups are the design source of truth, so a shipped screen that contradicts them is unfinished |
| [The Tag screen diverges from its mockup](https://github.com/magne/vitni/issues/310) | Same rule; no colour swatch anywhere in the read-only view of a record whose content *is* a colour |
| [The mockups' record-picker specimen pins itself to the viewport's top-left](https://github.com/magne/vitni/issues/311) | Three mockup pages render a stray floating dropdown, and the specimen's prose contradicts it |
| [A `SidePanel`'s background is not `inert`](https://github.com/magne/vitni/issues/312) | Assistive tech still reaches the shell behind an open panel |
| [Back/forward cannot return to a draft tab](https://github.com/magne/vitni/issues/313) | `⌘←`/`⌘→` step past an open draft to the last saved record |
| [The shared record-tab arms are re-implemented on 13 screens](https://github.com/magne/vitni/issues/322) | The vehicle for #303 and #304: changing the shared frame's shape is 13 edits, so a per-tab explanation lands on ~45 call sites |

Fourteen of these came out of that walkthrough: three — #302, #312, #313 — were already in `issues.md`
from earlier code reading and were promoted here by the same triage, and the other eleven are new.
The fifteenth, #322, is the one **refactor** in the gate, and it is here on the vehicle rule rather
than on its own user-visible consequence: #303 and #304 both change the shared tab frame's shape, and
with the arms written out on all 13 screens each of those lands as ~45 call-site edits instead of one.
The other simplification findings from the same 2026-08-13 code read carry no milestone — they are
cleanups whose absence changes nothing a user sees. The **design questions** that came out of the
walkthrough are filed *without* a milestone, because each needs a call before it needs code — the
Attach-versus-Add model (#314) among them.

#424 came later, from code reading while closing #421 rather than from the walkthrough. It sits second
because it silently drops data the user entered, which only the hang outranks.

**The previous round closed in full** — #200, #201, #203–#209, #231–#233, #239, #240, #244, #247,
#252–#261, #266, #279, #281–#285 — and their bullets left `issues.md` per §6. What those closures
*taught*, as opposed to what they changed, is in `issues.md`'s *Decided* section: duplicate element ids
cannot make a Dioxus handler inert (#279), a dead click on a scrolling strip is a scrollbar before it is
a hit-test bug (#285), the map needs a repaint rather than a resize (#252), and only a layout change
ever blanked the canvas. The code changes themselves are in the PRs and the commit log, which is where
§6 says they belong.

### `0.10 — Record matching` (6 open)

Ordered by dependency, not severity. Backup goes first, so every later event change passes its
fixture guard. Then come the origins, and the xref-collision bug with them, because its fix *is* the
origin model: it needs datasets and resolve-by-origin. The engine needs no origin work except the person profile, so it
can start alongside Backup and Origin. The identity decisions, the import rewrite and the UI build on
all of these. The design is ADRs 0037–0041. Every bullet is in `issues.md`: the bug under *Bulk
import, export & sync*, backup under *Backup & restore*, and the rest under
[*Record matching & identity*](issues.md#record-matching--identity), whose *Needs:* lines are the
per-issue prerequisites.

| Stage | Items, in order | Needs (other stages) | Why it gates |
| --- | --- | --- | --- |
| Backup (ADR 0041, #391) | Versioned backup and restore, with golden fixtures and the pre-1.0 window | — | Internal ids must survive; the format 1.0 freezes has to exist |
| Origin (ADR 0037, #393, #394, #389, #395) | `RecordOrigin` + `ImportRun`; the `record_origins` index and resolve-by-origin; GEDCOM/Gramps external ids collide across files; tombstones | Backup | A re-import is a no-op for every kind; two unrelated files no longer merge their people; retractions survive re-import |
| Engine (ADR 0038, #396–#400, #402) | Matching core with culture packs; person profile; Family/Event profiles; the other kinds' profiles; `match_keys` + `find_similar`; evaluation corpus + `match-eval` | Only the person profile (#397) needs Origin (#393) | One explainable answer to "are these the same?" |
| Identity (ADR 0039, #403, #401, #404–#406) | `PersonsDistinguished` + assessment; duplicate check through the engine; persona clusters; Event/Family merge; the other kinds' merge | Engine | A confirmed match behaves as one record; a rejected one never returns |
| Staged import (ADR 0040, #407–#410) | WIT record graph + `ImportPlan` (with the importers' port); GEDCOM/Gramps dataset proposal; one-graph Digitalarkivet import; Digitalarkivet events, places, households | Origin (#394), Engine (#400) | The host resolves identically for every importer, before writing |
| UI & CLI (#411–#416) | Shared compare view; assisted match stage; bulk Plan/Review + `--plan`; review queue; similar-record hint; linked records | Engine, Identity, Staged import | The user decides side by side; with nothing to ask, nothing is shown |
| Plugins (#417) | `find-similar` WIT query | Engine (#400) | Plugins reuse the engine instead of re-implementing it |

The count is 28 because the *More name-culture packs* bullet is unfiled by design, and the backup
freeze (v1, #392) is filed in `1.0`. Issues were numbered before the order was checked, so two sit
out of sequence. #389 follows #394, because once xrefs and Gramps ids stop being `ExternalId`s only
resolve-by-origin can find a re-imported person. #401 follows #403, because the duplicate check
excludes decided pairs. Every other stage is in number order. All 28, and the defects found while
landing them, are closed.

**Residuals (filed 2026-10-04).** What the first round left open: the bullets under *Record matching &
identity* are failures of the milestone's own promise — a crash or an interrupted open that lets a
re-import duplicate, a *Same* that drops data, and lookups that do not scale to the 100k persons the
engine targets. The three *Bulk import, export & sync* items are ADR 0029 reconciliation, which a
re-import leans on once it no longer duplicates. #486 is here because a failed projection write lets
the ADR 0044 replace commit a workspace with records missing.

| Item | Area | Why it gates |
| --- | --- | --- |
| [An interrupted `record_origins` backfill is never resumed](https://github.com/magne/vitni/issues/489) | matching | Re-import misses earlier imports' origins and duplicates until `vitni rebuild` |
| [The origin index is written after the event commit](https://github.com/magne/vitni/issues/490) | matching | A kill between the two makes the next import create the record again |
| [A record decided *Same* as a stored place, source or repository adds nothing to it](https://github.com/magne/vitni/issues/491) | matching | The incoming record's fields are dropped without notice |
| [The plan does not show what reusing a record adds to it](https://github.com/magne/vitni/issues/509) | matching | The review cannot show what a *Same* writes |
| [The import plan shows no record's fields](https://github.com/magne/vitni/issues/510) | matching | The user cannot see what a re-import or a *Same* changes before committing |
| [An assisted import shows nothing of what it adds before it commits](https://github.com/magne/vitni/issues/512) | matching | A one-record import writes what a *Same* adds unseen |
| [Projection write errors are dropped](https://github.com/magne/vitni/issues/486) | cross-aggregate | A rebuild or replace finishes `Ok` with records missing |
| [`find_similar` reads every profile view](https://github.com/magne/vitni/issues/492) | matching | One lookup costs ~0.9 s at 100k persons |
| [The all-pairs duplicate scan does not scale to 100k persons](https://github.com/magne/vitni/issues/493) | matching | The Dashboard runs it on every show |
| [The Dashboard reads every person summary on each show](https://github.com/magne/vitni/issues/519) | matching | 1.8 s at 100k persons on each Dashboard load |
| [Blocking loses a given name that is only similar](https://github.com/magne/vitni/issues/494) | matching | The engine shows a pair blocking can never find |
| [Planning an import costs a workspace-wide load](https://github.com/magne/vitni/issues/495) | matching | Once per record in an assisted session |
| [An abandoned import run cannot be resumed from History](https://github.com/magne/vitni/issues/496) | matching | ADR 0040 §5's resume has no GUI entry point |
| [Gramps `<header created>` is never threaded to `begin-run`](https://github.com/magne/vitni/issues/497) | bulk | A Gramps re-import gets no timestamp gating |
| [Source merge/sync reconciliation prerequisite](https://github.com/magne/vitni/issues/498) | bulk | A re-import cannot reconcile a source's bibliographic fields |
| [Place merge/sync reconciliation prerequisite](https://github.com/magne/vitni/issues/499) | bulk | A re-import cannot reconcile a place's fields |

All 16 are closed.

**Residuals (filed 2026-10-08).** The second pass. #528 and #529 are ADR 0029 reconciliation again: a
re-import that silently updates nothing, and a boundary no test pins. #537 is #499's follow-up — round-trip
gaps carry no milestone (§3), but this one decides whether a Gramps re-import can reconcile a place's code.
#535 is a regression of #486's fix, and #536 is #490's window on every projection. #525 makes the
`matches-*` scenarios unreliable, and may be pairs lost from the projection.

| Item | Area | Why it gates |
| --- | --- | --- |
| [An unreadable event stops the workspace opening](https://github.com/magne/vitni/issues/535) | cross-aggregate | One bad payload locks the user out, with no export or restore and an error naming no event |
| [A projection is written after the event commit](https://github.com/magne/vitni/issues/536) | cross-aggregate | A kill between the two leaves records stale until `vitni rebuild` |
| [The Matches table sometimes lacks the fixture's media pair in `gui-pass`](https://github.com/magne/vitni/issues/525) | matching | Either the projection can lose pairs for good, or the matching scenarios are flaky |
| [An unreadable file export date is ignored without notice](https://github.com/magne/vitni/issues/528) | bulk | A re-import of a newer file updates nothing, unseen |
| [No test covers an export date equal to the live value's time](https://github.com/magne/vitni/issues/529) | bulk | Flipping the "at least as recent" boundary stays green |
| [Gramps `<placeobj><code>` is not read](https://github.com/magne/vitni/issues/537) | round-trip | A Gramps re-import cannot reconcile a place's code |

### Not in any milestone

DNA depth, round-trip gaps, performance/scale, the ADR 0014 plugin-trust out-of-scope list, the
plugin-UI vocabulary tail, upstream-blocked dependencies, assisted-import residuals, saved searches /
column chooser / list virtualization, geocoding, and the `place_parent` index. All `priority/low` or
`priority/medium`, no milestone.

Three items from the 2026-08-12 walkthrough stay here too, and are not filed at all: expandable
collection nodes with counts on the Dashboard and the History tab (a feature needing a disclosure
primitive that does not exist yet), media edit-mode file handling (existence flagging, download from a
changed web path, MIME inference), and the unasserted zoom-interpolated marker radius. Each is real and
each is depth, not correctness — none of them makes the GUI report something false, which is the line
this milestone draws.

The **Norwegian-geography import** belongs here too, and deliberately so. Its model changes are gated by
an unwritten ADR 0031 and it is scoped in `roadmap.md` as closing Phase 9's declared residuals, not as a
shipping gate — putting a feature body with an unaccepted gating ADR ahead of UI correctness would
invert the ordering rule in §3.

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
- *Online* (`--online`, run by hand against your own `gh` auth): reconcile both directions and report
  drift — an open issue whose bullet is gone, a bullet referencing a closed issue, an open issue with no
  bullet at all.

The offline half gates every commit; the online half is run by hand because it needs a token and
network and drift is slow.

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
