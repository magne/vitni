# 34. Licence split: AGPL application, permissive interchange crates

- **Status:** Accepted
- **Date:** 2026-08-14

## Context

The workspace has declared `license = "MIT OR Apache-2.0"` in `[workspace.package]` since the first
commit, and until this ADR the tree contained **no licence file at all** — no `LICENSE-MIT`, no
`LICENSE-APACHE`, no `NOTICE`. A manifest field is a claim about terms nobody could read. The
repository is still private, so nothing has been granted to anyone and the decision is free to make;
after publication it is not, because a grant already made cannot be withdrawn for the versions it
covered.

[`research/licensing-and-monetization.md`](../research/licensing-and-monetization.md) worked the
option space. Four things shape the answer:

- **The realistic free-rider is an embedder, not a reseller.** The plausible harm is an existing
  genealogy vendor taking `vitni-core` — the event-sourced, provenance-by-construction model that is
  the whole differentiator — into a closed product.
- **A permissive licence hands that away irreversibly**, in exchange for adoption obtainable under
  copyleft anyway (Gramps and webtrees, this project's neighbours, are both copyleft).
- **`genealogy` was a generic term and therefore unregistrable**, so trademark could not supply the
  protection copyleft leaves open. ADR-adjacent but load-bearing: issue #324 renamed the project to
  Vitni precisely so a mark exists.
- **The exclusive revenue line does not need the licence.** Paid plugins (research §6) work
  identically under AGPL, FSL or MIT, because the paid part is a separate work — but only if
  third-party components are unambiguously outside the copyleft boundary.

The rejected alternative worth naming is **FSL-1.1-Apache-2.0**, which matches the stated requirement
more literally: it blocks paid hosting and binary resale outright. It was rejected because its
protection lapses two years after each release, it is not OSI-approved and so forfeits the distro
repositories — a real discovery channel in this domain — and it trades a permanent partial defence
for a temporary fuller one. Research §3.6 notes the asymmetry: FSL → open is the cheap direction and
this decision is the expensive one to reverse, which is an argument for taking the reversible-looking
option only if it were otherwise equal. It is not.

## Decision

### 1. The workspace is licensed per crate, in three groups

| Side | Crates | Licence |
| --- | --- | --- |
| Commodity interop — the goodwill generators, nothing that would ever be charged for | `vitni-interchange`, `vitni-gedcom`, `vitni-gramps-xml`, `vitni-i18n`; `plugins/{plugin-api,gedcom-import,gedcom-export,gramps-import,gramps-export,ui-panel,_fixture}` | `MIT OR Apache-2.0` |
| The application | `vitni-core`, `vitni-db`, `vitni-app`, `vitni-plugin-host`, `vitni-ui`, `vitni-ui-dioxus`, `vitni-cli`, `xtask` | `AGPL-3.0-or-later` + the §7 permission (§2) |
| Chargeable-later feature code | `vitni-digitalarkivet`, `plugins/digitalarkivet-import` | `AGPL-3.0-or-later` |

`[workspace.package] license` stays `MIT OR Apache-2.0` and the permissive crates keep inheriting it
with `license.workspace = true`; the AGPL crates override it with an explicit `license =` line. The
`plugins/*` crates are excluded from the workspace (ADR 0011) and inherit nothing, so each carries its
own `license` field.

Keeping the **MIT arm** on the permissive side is deliberate: a GPLv2-only consumer can take MIT, and
Gramps is GPLv2-*or later*, so either arm reaches it.

`vitni-digitalarkivet` sits with the application rather than with the other format crates because a
Digitalarkivet importer is the first plausible paid feature; putting it on the permissive side would
give that away before the question is even asked.

**The split holds only while the dependency directions do.** `vitni-gedcom` and `vitni-gramps-xml`
depend on `vitni-interchange` alone; `vitni-i18n` on nothing internal; each permissive plugin on
`vitni-plugin-api` plus a format crate. One new edge from a permissive crate into an AGPL crate would
make the permissive declaration false — a licence claim the project cannot honour, not merely an
inconsistency. `cargo xtask licence-check` (§6) asserts the directions on every prek run and in CI so
the invariant is executable rather than remembered.

### 2. An additional permission under AGPLv3 section 7 for WIT-world plugins

Every AGPL **application** crate carries this in its module header, and `NOTICE` repeats it verbatim:

> Additional permission under GNU AGPL version 3 section 7: if you modify this Program, or any
> covered work, by combining it with a WebAssembly component that interacts with the Program solely
> through the versioned `vitni:host-api` WIT world (or any later version of that world), the licensor
> grants you additional permission to convey the resulting work. Such a component is not required to
> be licensed under the GNU AGPL.

**This is load-bearing, not decorative.** Without it, third-party plugins are legally doubtful and
paid ones unsellable — not because of anything the copyright holder needs, but because a *buyer* who
redistributes host plus plugin needs the permission, and cannot get it from the holder's silence.

It confirms the existing architecture rather than carving an exception into it: no `plugins/*`
component links host code, each depending only on `vitni-plugin-api` plus a format crate and talking
over the WIT world (ADR 0007, 0011). The wording says "or any later version of that world" so a
host-api version bump — routine, and deliberately not back-compatible per repo convention — never
silently revokes a permission someone already relied on.

The permission goes in **all eight application crates**, including the ones a component never touches
directly, because a redistributor conveys a binary that links all of them; a per-crate carve-out
would leave the conveyance uncovered.

`vitni-digitalarkivet` is AGPL but carries **no** §7 header. It is not part of the Program a
third-party component combines with: nothing in the application binary depends on it, and its only
consumer is `plugins/digitalarkivet-import`, which is AGPL itself.

### 3. Contributions: DCO plus a broad licence grant, stated in `CONTRIBUTING.md`

Every mechanism above — commercial exceptions, store builds, a future relicence, and unpublished
modifications in any hosted service — depends on being able to licence the whole work on terms other
than the public one. Sole ownership provides that today and the first un-granted outside patch ends
it. `CONTRIBUTING.md` therefore carries both a Developer Certificate of Origin sign-off requirement
and an explicit licence grant.

**The Norwegian drafting constraints are why the grant is worded the way it is.** Under
åndsverkloven (2018) §67 first paragraph economic rights are fully assignable, but §67 second
paragraph codifies the *spesialitetsprinsippet*: an unclear grant is construed **in the contributor's
favour**. Breadth by implication is precisely what the statute refuses to read in, so the grant
*names* reproduction, modification, distribution, **sublicensing**, **relicensing** and commercial
exploitation instead of gesturing at them. §5 makes moral rights largely unwaivable, so the document
**promises attribution** rather than purporting to acquire the right to drop it.

`CONTRIBUTING.md` also states plainly why the grant exists and what it permits, including the "you
may sell my volunteer work under closed terms" objection, which is legitimate and answered rather
than dodged.

### 4. Acceptance is a pull-request checkbox plus a sign-off, not a bot

`.github/pull_request_template.md` carries an explicit grant checkbox; every commit carries
`Signed-off-by`. Both are affirmative acts recorded in the PR body and in git history.

A CLA bot (cla-assistant, or CLA Assistant Lite as an Action) would run for free once the repository
is public — Actions on standard runners is not billed for public repos — and was still deferred: it
stores signatures behind a PAT or a gist, which means holding a secret, and there are no outside
contributors for it to serve. **The trigger to revisit is the first outside pull request**, or the
point at which checking assent by review stops being reliable.

The §67(2) explicitness requirement is met by the *wording* of the grant, not by the mechanism that
records assent, so this deferral costs nothing legally.

### 5. `deny.toml` gets per-crate exceptions, never a wider allow list

Each AGPL workspace crate gets a `[[licenses.exceptions]]` entry naming it and
`AGPL-3.0-or-later`. Adding `AGPL-3.0-or-later` to `licenses.allow` instead would make an AGPL
*dependency* pass silently, which is the exact thing the check exists to catch: the project can only
ship copyleft it owns.

MPL-2.0 dependencies (`cssparser`, `selectors`, `dtoa-short`, `option-ext`) carry Exhibit A only and
no Exhibit B, so they impose file-level obligations on their own files and nothing on this
distribution's `NOTICE`.

### 6. The licence directions are checked, not asserted

`cargo xtask licence-check` — a new arm beside `input-guard`, wired into `cargo xtask check` so prek
and CI both run it — resolves the workspace dependency graph via `cargo metadata` and fails if a
crate declaring `MIT OR Apache-2.0` reaches one declaring `AGPL-3.0-or-later`. Because `plugins/*`
are excluded from the workspace and invisible to a root `cargo metadata`, it parses those eight
manifests directly and applies the same rule across their path dependencies. It also fails on a crate
that declares no licence at all, which is what the `plugins/*` manifests did before this change.

### 7. No per-file SPDX headers, no REUSE compliance

The `license` field of each manifest plus the four root files are the declaration. Per-file headers
across ~500 source files would be churn with no consumer: `cargo deny` reads manifests, and
distribution packaging reads the root files.

## Consequences

### Positive

- The commercial-exception desk exists as an actual option: someone who needs `vitni-core` in a closed
  product has one place to go, permanently, rather than a permissive grant already made.
- Third-party and paid plugins are unambiguously outside the copyleft boundary, so the plugin
  boundary stays usable as the exclusive revenue line (research §6) without further legal work.
- The interchange crates stay reusable by anything, including GPLv2-only projects — the goodwill the
  permissive arm is for.
- The right to relicense survives the first outside contribution instead of ending at it.
- A dependency edge that would falsify the split now fails a check instead of being noticed later, or
  not at all.

### Negative / costs

- **Contributor friction.** A licence grant asks more than a DCO does, and some contributors decline
  on principle. That is the cost of the exception business existing at all; `CONTRIBUTING.md` states
  it rather than burying it.
- **AGPL does not block two things the requirement wanted blocked**: GPL §4 permits charging for
  copies, and verbatim paid hosting owes nothing under §13. Trademark (the Vitni mark, #324) is the
  only lever against rebranded resale. Accepted knowingly — both are the least likely to pay anyone.
- **The Apple App Store is closed to an AGPL build** by store terms. Not a channel this project has.
- **Reversal is expensive.** Going permissive later requires every contributor's agreement, which the
  CLA is designed to supply but does not make free.
- **Two more manifest fields to keep right.** A new crate now has to choose a side, and choosing wrong
  is silent until `licence-check` runs — which is why it runs in `cargo xtask check`.

## Out of scope

- **Paid-plugin distribution shape** — private repo, its own EULA, which ADR 0014 trust tier a paid
  first-party bundle receives, and how a purchase becomes a downloadable signed bundle. Backlog.
- **Trademark registration** for the Vitni mark; #324 made a registrable name possible, no more.
- **A CLA bot**, deferred with a named trigger (§4).
- **CRA obligations** — 11 September 2026 reporting duties precede 11 December 2027's main
  obligations. Nothing is monetised, so nothing is due; revisit the moment something is.
- **Relicensing anything already published** — nothing has been, which is what made this decision
  free to take.

## References

- [`research/licensing-and-monetization.md`](../research/licensing-and-monetization.md) — the full
  option space, the threat model, the FSL comparison scored against the requirement as worded, and
  the CLA/DCO analysis this ADR condenses.
- ADR 0007, 0011 — the WASM component plugin system and the versioned `vitni:host-api` WIT world the
  §7 permission is drawn around; the reason no component links host code.
- ADR 0014 — plugin signing and trust tiers, the mechanism a paid first-party bundle would use.
- Issue #324 — the rename to Vitni, which supplies the registrable mark that copyleft alone cannot.
- åndsverkloven (LOV-2018-06-15-40) §5 and §67 — the moral-rights and *spesialitetsprinsippet*
  constraints on the grant's wording.
