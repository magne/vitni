# Plan — Phase 10: research rigor & import sync

- **Status:** Gate 2 (implementation) approved and underway — PR1 (ADR 0027) done; PR2–PR4 remain
- **Roadmap home:** [Phase 10](../roadmap.md#phase-10--research-rigor--import-sync)
- **Gating ADRs:** [0027](../adr/0027-configurable-surety-scheme-labels.md) (surety-scheme labels),
  [0028](../adr/0028-research-note-argument-aggregate.md) (`ResearchNote`/`Argument` aggregate),
  [0029](../adr/0029-import-merge-sync-reconciliation.md) (import merge/sync). The fourth workstream
  (round-trip gaps) needs **no new ADR** — see below.
- **Research:** [`surety-schemes.md`](../research/surety-schemes.md),
  [`proof-argument-modelling.md`](../research/proof-argument-modelling.md),
  [`merge-sync-conflict-resolution.md`](../research/merge-sync-conflict-resolution.md)

## Context

Phase 10 is the evidence/conclusion model's research-quality layer (data-model §17): four
independent workstreams parked there because each needed either a gating ADR or enough research to
scope correctly before code. This phase is delivered in **two gates**:

- **Gate 1 (this delivery):** research docs + the three gating ADRs (Proposed) + this plan. No code.
- **Gate 2:** one PR per workstream, each ADR-gated, built only after the ADRs above are reviewed and
  accepted. This document is Gate 2's map.

## Why no fifth ADR for round-trip gaps

The roadmap's fourth Phase 10 bullet — GEDCOM `REPO` records/pointer, `FAM`-level `SOUR`/`OBJE`/`NOTE`,
multi-`NAME`, `FAMS`/`FAMC` back-refs, `SUBM`, media `FORM`, citation `CALN`, Gramps `<tagref>`, the
`RichText` translator, `Address.original_text`, and Gramps `<region>` export — extends the *mapping
strategy* ADR 0013 already fixed (map external records to persona-level aggregates, carry
`ExternalId`, resolve-or-create) and the *owner-link projection* ADR 0018 already fixed (project
attachments so an exporter can read them back). Each gap is a new field or record type reaching an
already-decided contract, mechanical in the same sense ADR 0018's consequences call its own verb
growth "roughly doubles the surface... each verb a thin delegation" — not a new decision. No ADR
gates this workstream.

**A staleness note for whoever picks up this PR:** two items in the roadmap/issues Phase 10 bullet —
place `MAP`/coordinates and event-level witnesses — read as already closed by work that landed *after*
that bullet was written. Place `MAP`/coordinates round-trips via GeoJSON since ADR 0024 (Phase 9,
`docs/data-model.md` §17's own opening line: "closes the deferred `PLAC.MAP` round-trip gap"), and
event-level witnesses round-trip since ADR 0019 (data-model §17: "closing the §17 event-level-witness
gap"). data-model §17's own "Model gaps... (none modelled yet)" bullet still lists `PLAC.MAP` /
place coordinates, which now reads as stale prose left over from before Phase 9 — not a code gap.
Verify current state against `genealogy-gedcom`/`genealogy-gramps-xml` before scoping the PR rather
than trusting the bullet list verbatim; drop closed items, keep the rest.

## PR stack

The four workstreams touch disjoint code (a config field; a new, isolated aggregate; the import
use-case + host-api; the two format crates) and have **no dependencies on each other** — they can be
built and reviewed in any order, or in parallel across worktrees. Ordered here smallest-blast-radius
first, matching how Phase 9's stack sequenced its three ADR-gated slices:

### PR1 — Configurable surety-scheme labels (ADR 0027) ✅ done

- **Branch:** `feat/surety-scheme-labels`
- **Minimal slice:** a `SuretyScheme` (5 ordered `SuretyLevel { label, description }`) added to
  workspace-functionality config (`workspace.toml`, following the `id_formats` shape exactly — a
  per-workspace override plus a live `[workspace-defaults.surety]` fallback, ADR 0005/0015); every
  Fluent-resolved confidence label (person/family/event/place/source/citation/DNA screens, the CLI)
  reads through it instead of a hardcoded per-variant message id. `Confidence`, `EventContext`, GEDCOM
  `QUAY`, and Gramps confidence mapping are **untouched**.
- **Files (indicative):** `genealogy-app` config types + `ConfigStore` scope; `genealogy-core`
  unaffected; `genealogy-cli`/`genealogy-ui-dioxus` label resolution; new Fluent fragment(s) for the
  five default labels (`en`, `no`); a **surety-scheme preferences panel** mockup addition (workspace
  settings screen).
- **Delivered as:** `SuretyLabelOverride`/`SuretyLabelOverrides` (`genealogy-app::config`), the
  `[workspace-defaults.surety]` / per-workspace `[surety]` manifest layer (`workspace.rs`, mirroring
  `id_formats`/`locale`), a `ConfigStore::store_workspace_default_surety` seam method, and a
  `Localizer::with_surety_overrides` builder consumed by both `genealogy-cli` and `genealogy-ui`'s
  `confidence_label`/`confidence` — no new Fluent keys needed there (the existing `confidence-*` keys
  are the default wording ADR 0027 §3 already names). `genealogy-ui-dioxus` gained its own
  `prefs-surety-*` chrome keys (`en`/`no`) for the new "Surety scheme" Preferences card
  (`docs/mockups/preferences.html`, `screens/preferences.rs`), which edits the live global default —
  matching the existing locale/id-format cards, not a per-workspace override UI (none of those cards
  expose per-workspace overrides either; editing one is a manifest-file-only affordance today).
- **Verification:** default labels render unchanged when no override is set; an overridden label
  renders everywhere a `Confidence` appears; GEDCOM/Gramps round-trip tests untouched and green.

### PR2 — `ResearchNote`/`Argument` aggregate (ADR 0028)

- **Branch:** `feat/research-note-aggregate`
- **Minimal slice:** the 13th aggregate, template-following (command/event/state/view/decide/error),
  `SubjectRef` value object, `ResearchNoteCreated`/`RichTextSet`/`Tagged`/`Untagged`/
  `RestrictionsChanged`/retract-supersede, the `UnknownSubject` aggregate-tax check, wired through the
  three x-macro registries (`genealogy-app::aggregates`, `genealogy-db::registry`, CLI `.ftl` +
  `for_each_cli_command!`), re-exported from `genealogy-app/src/lib.rs`. CLI-only in this PR
  (`genealogy research-note create/show/list`); the GUI screen is its own follow-up (below) so this PR
  stays reviewable as "one new aggregate," matching the Person/Family template precedent.
- **Follow-up in the same PR or immediately after:** a minimal **ResearchNote/Argument UI screen**
  mockup + `genealogy-ui`/`genealogy-ui-dioxus` screen (list + detail, citations, subject link) —
  named explicitly so it is not silently dropped; split into PR2b if PR2 alone is large enough to want
  independent review.
- **Verification:** TDD per aggregate convention (given/when/then over `decide`); `UnknownSubject`
  rejected against a lagging projection like `UnknownPlace`; `cargo xtask i18n-check` passes with the
  new fragment; a `ResearchNote` citing a `Citation` and a `DnaMatch` (ADR 0023's `EvidenceRef` union)
  both work end-to-end.

### PR3 — Import merge/sync reconciliation (ADR 0029)

- **Branch:** `feat/import-merge-sync`
- **Minimal slice:** `genealogy-gedcom` (and the Gramps XML equivalent) resolve `file_asserted_at`
  from `HEAD.1 DATE` / `<header created=…>`; a new host-api `commands` parameter threads it once per
  import session (`host-api@0.19.0` → `0.20.0`, a label bump per ADR 0018 §3, both first-party plugins
  updated in lockstep); the `genealogy-app` resolve-or-create import use-case gains the timestamp-gated
  policy (supersede vs. leave-untouched) for exactly two field groups: `Person.sex` and
  `Source.title`/`author`/`pub_info`/`abbrev`.
- **Files (indicative):** `genealogy-gedcom`/`genealogy-gramps-xml` header-date parsing;
  `crates/genealogy-plugin-host/wit/host.wit` + `genealogy-plugin-api`; `genealogy-app` import
  use-cases (`import_person`/`import_source`); a **merge/conflict view** addition to
  `docs/mockups/import.html` showing a reconciled field's audit trail (who/when/why — the generated
  rationale), not an interactive picker (out of scope per the ADR).
- **Verification:** a host integration test re-importing a file with (a) an intentionally stale
  in-file value against a workspace value asserted after the file's export date (no change expected)
  and (b) a workspace value asserted before the file's export date (supersede expected, audited);
  missing/unparseable `HEAD.DATE` falls back to today's additive-only behaviour (regression-tested).

### PR4 — Round-trip gaps breadth (no new ADR)

- **Branch:** `feat/round-trip-gaps`
- **Minimal slice:** verify current state first (staleness note above), then close the genuinely open
  items against ADR 0013/0018's existing contract: GEDCOM `REPO` record/pointer, `FAM`-level
  `SOUR`/`OBJE`/`NOTE`, multi-`NAME` records per person, `FAMS`/`FAMC` back-refs, `SUBM`/`HEAD`
  metadata, media `FORM`/type/`CAPT`, citation `CALN`, Gramps `<tagref>` on person/family,
  `RichText` translator round-trip, `Address.original_text` fallback, and Gramps `<region>` export
  (needs the query-side media-crop DTO from PR #157 first — data-model §17 already names this
  dependency). Can be split into several commit-sized groups (mirroring Phase 4's group lettering)
  rather than one PR if the combined diff is large.
- **Files (indicative):** `genealogy-gedcom`, `genealogy-gramps-xml`, `genealogy-interchange`, the
  `commands`/`query` host-api surface (additive verbs only, reusing existing capabilities per the ADR
  0018 §2 precedent — likely no version bump needed unless a genuinely new verb is required).
- **Verification:** the existing `gramps_round_trip.rs` / GEDCOM round-trip integration tests extended
  per closed gap; a fixture-backed import → export → re-import cycle shows no event-log growth on
  re-import (idempotency preserved) for every newly-closed field.

## Open questions for the owner before Gate 2

1. **ADR 0027 scope check** — relabeling only (this ADR) versus deferring the whole surety-scheme item
   further if even relabeling isn't wanted yet.
2. **ADR 0028 naming** — confirm `ResearchNote` (not `Argument`, not both as separate types) as the
   aggregate name before it is wired through three registries and a `HumanId` letter (`A`) is spent.
3. **ADR 0029 field scope** — confirm `Person.sex` + `Source` bibliographic fields as the right first
   two, or substitute/add fields the owner cares about more for their actual re-import workflow.
4. **PR4 staleness** — confirm the place-`MAP` and event-witness items are indeed already closed (drop
   from scope) before the implementer spends time re-verifying what Phase 9/ADR 0019 already shipped.

## Gate 2 exit criteria (once implemented)

- All four PRs merged (`--no-ff`, feature branches, never direct to `main`).
- `cargo build --workspace`, `cargo nextest run --workspace --all-features --all-targets`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all`,
  `cargo xtask build-plugins`, `cargo xtask i18n-check`, `cargo deny check`, `prek run` all clean.
- ADRs 0027–0029 flipped from Proposed to **Accepted** (ADRs are immutable once accepted — only the
  Status/Date lines and this plan change, never the Decision text, unless Gate 2 implementation reveals
  the ADR itself needs a correction, in which case supersede with a new ADR rather than edit).
- `docs/roadmap.md`/`roadmap.html`: Phase 10 marked ✅ with a summary paragraph (the Phase 9 precedent);
  the "New ADRs required" table rows for 0027–0029 flipped to "— accepted".
- `docs/issues.md`: the Phase 10 bullets moved to *Completed* with branch/PR references, mirroring how
  Phase 7/8/9 completions are recorded there today.
- Mockups updated: the ResearchNote/Argument screen, the surety-scheme preferences panel, and the
  import merge/conflict view (named above per-PR) all present in their respective `docs/mockups/*.html`
  files, not just described in prose.
