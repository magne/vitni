# Phase 4 follow-ups — import/export breadth and assisted import

- **Status:** Living checklist
- **Date:** 2026-06-22
- **Audience:** anyone continuing Phase 4 after the bulk import/export foundation

Phase 4 of [`docs/roadmap.md`](roadmap.md) is large and is being delivered as a
sequence of PRs, each landing with its gating ADR. The **foundation** — the
format-neutral `bulk-import`/`bulk-export` worlds, streaming source/sink, the
`progress` capability, the `genealogy-plugin-api` crate, the migrated GEDCOM
plugins, and the `genealogy import`/`export` CLI commands — is **done**
([ADR 0013](adr/0013-import-export-contract.md)). This document captures the work
that remains so it is not lost.

## PR 2 — bulk-format breadth

Delivered as a sequence of commit-sized groups. Groups A–E (the re-import
idempotency mechanism and the new-workspace-default CLI) are **done** on branch
`feat/phase-4-pr2-import-idempotency`; F–G (format breadth) remain.

### Done

- **A — `ExternalId` assertion.** `ExternalId` (`genealogy-core` `text.rs`,
  data-model §7/§11) wired into **Person and Family**: `AddExternalId` command +
  `ExternalIdAdded` event (accumulate-in-state), idempotent in `decide` (re-adding
  the same `(authority, value)` emits no event). *(Source/Citation/Media join in
  group F, when the importer first creates them.)*
- **B — projection lookup by external id.** `find_person_by_external_id` /
  `find_family_by_external_id` on the store and both backends (SQLite `json_each`,
  Postgres `json_array_elements`).
- **C — resolve-or-create import use-cases.** `genealogy-app` `import_person` /
  `import_family` resolve an incoming record by `(authority, value)` and update the
  existing aggregate instead of duplicating it (data-model §11); names are added
  additively; `import_add_partner` / `import_add_child` treat an already-present
  member as a no-op. Host `commands` capability bumped to WIT `host-api@0.4.0`
  (`external-id` record; `create-person` / `create-family` take an optional
  external id and upsert). `genealogy-gedcom` captures/emits `_UID`; the GEDCOM
  import plugin keys on `_UID` (authority `gedcom-uid`), falling back to the file
  xref (`gedcom-xref`). This is the mechanism behind re-importing the same
  Digitalarkivet URL or re-syncing a Gramps export.
- **D — idempotency verification.** Re-importing an identical file produces no new
  events (host integration test; manual run on a 1513-person MyHeritage export).
- **E — new-workspace-default CLI.** `genealogy import` imports into a fresh
  workspace by default (`--new NAME PATH`); `--into NAME` targets an existing one
  and prompts for confirmation when it already holds data (skipped with `--yes`).
- **F — GEDCOM 7 round-trip.** `genealogy-gedcom` now parses and emits `SEX`,
  events (`BIRT`/`DEAT`/`MARR`/`CHR`/`BURI`/`CENS`/`RESI`/`IMMI`/`EMIG`) with
  `DATE`/`PLAC`, top-level `SOUR` records + `SOUR`/`PAGE` citations, inline `OBJE`
  media (`FILE`/`TITL`), and `NOTE`. Each maps to its aggregate (Event, Place,
  Source, Citation, Media, Note) through new `commands` verbs (WIT
  `host-api@0.5.0`). A person's or family's owned records (events, citations,
  media, notes — and the sex/places/sources they pull in, deduped within an
  import) are created **only when that owner is newly created**, so re-import
  stays idempotent without an `ExternalId` on every aggregate. Verified on a
  1513-person MyHeritage export: 2902 events, 352 places, 60 sources, 431
  citations, 69 media, 21 notes — all unchanged on re-import. Structured name
  parts and `ADDR` are not yet mapped. (Note: the simpler owner-gating made the
  originally-planned `ExternalId` on Source/Citation/Media unnecessary.)

### Remaining

- **F′ — GEDCOM 7 round-trip, finishing touches.** The parts group F left unmapped:
  - **Structured name parts.** Parse/emit the `NAME` sub-records (`GIVN`, `SURN`,
    `NICK`, `NPFX`, `NSFX`, `SPFX`) and the `TYPE` instead of only splitting the
    `Given /Surname/` slash form, and map them onto `PersonName`'s structured fields
    (given, surnames, nickname, prefix, suffix, name type — data-model §14) rather
    than the current given+primary-surname approximation. Needs a richer
    `assert-name` host verb (or extending `create-person`) carrying the parts.
  - **`ADDR`.** Parse/emit the `ADDR` structure (`ADR1`/`ADR2`/`CITY`/`STAE`/
    `POST`/`CTRY`, plus `PHON`/`EMAIL`/`WWW`) and map it onto the `Address` value
    object — wired today only on Repository (`add_repository_address`); decide the
    target aggregate(s) for an event/individual residence address.
  - **Full GEDCOM date grammar.** Replace the best-effort year/month/day parser
    (which drops modifiers) with the real grammar: `ABT`/`EST`/`CAL` (→
    `DateQuality`), `BEF`/`AFT`/`BET…AND`/`FROM…TO` (→ the `DateModifier`
    before/after/range/span variants), dual dates, and non-Gregorian calendars
    (`@#DJULIAN@` etc.), round-tripping through `GenealogicalDate` instead of a
    plain `(year, month, day)`.
- **G — Gramps XML.** A new pure `genealogy-gramps-xml` crate (parse/emit over an
  intermediate model, mirroring `genealogy-gedcom`), plus `plugins/gramps-import`
  and `plugins/gramps-export` glue on the `bulk-import`/`bulk-export` worlds.
- **Future — merge / sync.** Re-import is **additive-only** today: an identical
  value is a no-op, a genuinely new value is added, but a *conflicting*
  single-valued fact (the file disagrees with what is stored) is left untouched.
  True merge — reconciling divergent values, never overriding a fact asserted
  *after* the file's HEAD `1 DATE` (export date) — is deferred to its own PR.

## PR 3 — Digitalarkivet assisted importer (new ADR)

Gated by a new **ADR 0017 — assisted-import host capabilities**, which fixes the
host contract ADR 0011 §3 deferred and ADR 0013 left out of scope:

- **`net` capability.** Outbound HTTP with a host allowlist (e.g.
  `*.digitalarkivet.no`), deny-by-default. Used to fetch the source record page
  and resolve the scan image URL chain.
- **`media-store` capability.** The host writes downloaded bytes under the
  workspace `media/` directory, computes a checksum, and returns a relative path.
  The Media aggregate stays **metadata-only** (path + checksum in the event log);
  the host owns the bytes on disk. The plugin then creates the Media aggregate via
  `commands` with that path + checksum + an `ExternalId`.
- **`ai` capability — pluggable, named, multi-provider.** Config (app config and
  `workspace.toml`, layered per ADR 0005) declares `[ai.providers.<name>]`
  entries, each with `kind = "command"` (shell out to a local CLI, e.g. `gemini`)
  or `kind = "vision-api"`, plus `[ai].default = "<name>"`. The host `ai`
  capability resolves a provider **by name** (caller may pass a name; absent →
  default) and dispatches on `kind`. No single hardcoded provider.
- **`plugins/digitalarkivet-import`** glue + a pure `genealogy-digitalarkivet`
  crate that parses census (`/census/person`, `/census/rural-residence`) and
  churchbook (`/view/{src}/{rec}`) pages and resolves the scan URL chain
  (`scannedImageLink` → media viewer → `permanent_image_link` / `og:image` →
  `urn.digitalarkivet.no/...jpg`), modelled on the reference
  `~/Genealogi/scripts/sort-inbox.py`.
- **Flow.** Fetch the source page → store the scan via `media-store` → if the
  record is transcribed, parse the fields; otherwise AI-interpret the scan →
  import as **low-confidence** Software-agent Person/Source/Citation/Media with an
  `ExternalId` back to the Digitalarkivet record URL. The user reviews and
  corrects with the existing CLI commands (the interactive flow is PR 4).
- **Notes for the implementer.** Census records are household-centric (parse the
  residence page to get all members); churchbooks are person-centric with related
  persons. Transcribed fields show as `-` when not indexed. Scans are JPEGs via
  the `urn.digitalarkivet.no` URN; high-res/IIIF retrieval is a later refinement.

## PR 4 — interactive present-and-confirm (extends ADR 0017)

- A host `present` capability: show the AI-interpreted record **and the scan**, let
  the user confirm or edit before import. The CLI renders the image inline via the
  kitty graphics protocol (or sixel) when the terminal supports it, else prints a
  path; the same capability backs a future GUI with a different renderer. This is
  the deferred half of the assisted-import experience.

## PR 5 — distribution (ADR 0014)

- **ADR 0014 — plugin signing, trust tiers, distribution.** Plugin signing and
  trust tiers (ADR 0007 §9), the three-layer loading override (workspace >
  app-dir > embedded, ADR 0007 §4), a per-plugin declared-capability manifest, and
  the capability-grant UX. Replaces the foundation's minimal
  `PluginHost::load_by_id` directory loader and the CLI's `$GENEALOGY_PLUGIN_DIR`
  / `target/plugins` default.

## Smaller deferred items

- **Software-agent identity/version.** The CLI currently builds the plugin's
  Software session with the plugin id as the name and the CLI's own crate version
  (`Session::software`). A real plugin version (and a stable agent id) should come
  from the plugin manifest introduced in ADR 0014.
- **Localized progress steps.** The `progress` `step` string is shown verbatim;
  treat it as a Fluent message ID resolved by the frontend (as the ui-panel does,
  ADR 0012) once steps stabilize.
- **Trigger progress cancellation.** The `progress` capability returns
  `proceed`/`cancel` and the plugins honor `cancel`, but the CLI sink always
  returns `proceed`. Wire a Ctrl-C / interrupt handler (and the GUI's cancel
  button) to return `cancel`.
- **Epoch-based wall-clock timeout.** ADR 0011 §4 named epoch interruption as the
  production successor to fuel; bulk operations that block on I/O are a good reason
  to wire it.
