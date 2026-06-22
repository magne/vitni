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

- **GEDCOM 7 round-trip.** Expand `genealogy-gedcom` from the current minimal
  subset (INDI + NAME, FAM + HUSB/WIFE/CHIL) toward a GEDCOM 7 round-trip:
  structured name parts, dates, places, `OBJE`, `NOTE`, `SOUR`/citations, `ADDR`.
  Keep the parse → emit → parse round-trip property test.
- **Gramps XML.** A new pure `genealogy-gramps-xml` crate (parse/emit over an
  intermediate model, mirroring `genealogy-gedcom`), plus `plugins/gramps-import`
  and `plugins/gramps-export` glue on the `bulk-import`/`bulk-export` worlds.
- **`ExternalId` wiring.** `ExternalId` is defined (`genealogy-core` `text.rs`,
  data-model §7/§11) but wired into **zero** aggregates. Add `AddExternalId`
  command + `ExternalIdAdded` event (accumulate-in-state) to Person, Source,
  Citation, and Media; surface it in the app use-cases, DTOs, and the host
  `commands` capability.
- **Re-import idempotency / dedup / sync.** Resolve an incoming record by
  `(authority, value)` against the projections; update the existing aggregate
  instead of creating a duplicate (data-model §11). This is the mechanism behind
  re-importing the same Digitalarkivet URL or re-syncing a Gramps export.

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
- **Epoch-based wall-clock timeout.** ADR 0011 §4 named epoch interruption as the
  production successor to fuel; bulk operations that block on I/O are a good reason
  to wire it.
