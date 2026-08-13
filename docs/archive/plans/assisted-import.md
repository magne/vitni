# Plan — Assisted import & external search (Digitalarkivet) (full phase)

- **Status:** Proposed
- **Roadmap home:** Phase 8 (assisted import; the `net` capability Phase 9 depends on)
- **Mockups:** [`../../mockups/import.html`](../../mockups/import.html) (the wizard),
  [`../../mockups/media.html`](../../mockups/media.html) (preview, media viewer, crop tool)
- **Gating ADR:** [0017](../../adr/0017-assisted-import-host-capabilities.md) (assisted-import host
  capabilities: `net`, `media-store`, `ai`, `present`)
- **Research:** [`../../research/digitalarkivet.md`](../../research/digitalarkivet.md) (API vs. scrape,
  per feature)

## Context

Online, record-by-record assisted import from Digitalarkivet: fetch a source page, resolve and
download the scan into the workspace media library, optionally AI-interpret a hard-to-read (gothic)
page, present the interpreted record **and the scan** for the user to confirm or edit, and import it
as low-confidence Software-agent assertions with an `ExternalId` back to the record. The proven UX is
the owner's prototype `~/Genealogi/scripts/sort-inbox.py`; the research doc fixes that **there is no
anonymous public API** — search, page parsing, and scan resolution all ship HTML-first over
`*.digitalarkivet.no`, unauthenticated and GET-only.

**Deviation from the roadmap (owner-approved):** the roadmap's "CLI renders the image inline (kitty
graphics / sixel)" is dropped. Present-and-confirm is **GUI-only** — a first-party `Tool::Import`
wizard renders the `present` payload (ADR 0017, Out of scope). This plan and PR1 record the deviation.

## What the model already supports (no change)

Confirmed in `vitni-core` / `vitni-app`:

- **Crop rectangle** — `Rect` and `MediaRef.crop` (`vitni-core/src/text.rs`) already model a
  percent crop region on a media reference. Only the app/DTO/WIT plumbing is missing: six
  `crop: None` sites in the projection mappers and `MediaRefSummary` drops `crop`.
- **Media directory** — `<workspace>/media/` is created at init (`workspace.rs`); `media-store`
  writes under it, the Media aggregate stays metadata-only (path + checksum + mime).
- **External-id dedup** — `ExternalId { authority, value }` and resolve-or-create re-import by
  `(authority, value)` against projections (data-model §11) are already wired on the aggregates from
  the Phase 4 breadth work.
- **Software-agent sessions** — the plugin host drives use-cases under an `AgentKind::Software`
  session (ADR 0011 §5); assisted commands reuse it, adding only a `Confidence::Low` template.

## What this phase adds

### 1. `net` + `media-store` host capabilities (ADR 0017 §2–3)

GET-only `net.fetch(url) -> http-response` under a caller-supplied `NetPolicy` (HTTPS-only host
allowlist checked per redirect hop, host-side timeout, response-size cap; userinfo/IP-literal URLs
denied); large binaries never enter the guest. `media-store.fetch-and-store` / `store` write under the
media root with host-enforced path safety, SHA-256 checksums (`"sha256:<hex>"`), and path+checksum
dedup. Client: `reqwest` (rustls-tls, gzip, stream); hashing: `sha2`; mime: `mime_guess`.

### 2. `vitni-digitalarkivet` pure crate (ADR 0017 §6; research §3)

HTML in, typed records out, zero I/O — unit-tested via `--workspace` against verbatim fixtures.
`classify_url`, `parse_person_page`, `parse_residence_page`, `extract_viewer_image_url` (the
`scannedImageLink` → `permanent_image_link` → `urn.digitalarkivet.no/URN:NBN:…jpg` chain),
church-book parsing; URN → `ExternalId { authority: "digitalarkivet" }`; Source/Citation/Repository
suggestion metadata (Norwegian archival constants are crate data, not UI strings). HTML parser:
`scraper` (must build for `wasm32-wasip2` — verified in its PR; fallback `tl`). An empty `api` module
seam is kept for a future documented endpoint/IIIF. Fixtures copied verbatim from
`~/Genealogi/fixtures/{census,churchbook}/`; `prek.toml` whitespace/EOF exclusions extended to the new
path.

### 3. `ai` capability + config (ADR 0017 §4)

`ai.interpret-media(provider, media-path, prompt) -> string` (raw model text; plugin parses/repairs
JSON). `[ai]`/`[ai.providers.<name>]` config in **client scope** (ADR 0015 `ConfigStore`):
`kind = "command"` (argv vector, no shell, `{prompt}`/`{media}` as whole args, cwd = workspace,
timeout + kill-on-drop) or `kind = "vision-api"` (OpenAI-compatible chat-completions, base64 image,
`api-key-env` indirection). `Invocation.provenance_confidence` template; assisted caller sets
`Confidence::Low`.

### 4. Media crop plumbing (ADR 0017 §9, 0.18.0)

`MediaRefInput { crop: Option<CropRect>, caption: Option<String> }` on the six `attach_*_media`
use-cases; `MediaRefSummary` gains `crop` (+ `path`/`mime` joined from the media projection); the six
`crop: None` projection sites stop dropping it. New per-owner `update_*_media_ref(owner, assertion_id,
MediaRefInput)` = `AssertionSuperseded(old)` + a new `MediaAttached` (the row-Edit correction
pattern, never mutation). WIT `attach-*-media` gain `crop`/`caption` (breaking `commands` → the
0.18.0 bump); the gramps-xml plugin maps `<region>` end-to-end (GEDCOM passes none). All new
use-cases/DTOs re-exported from `vitni-app/src/lib.rs` **first** (export-before-consume, incl.
`Rect`/`CropRect`).

### 5. GUI media display + crop tool + save dialog (ADR 0017 §5; ADR 0008)

- **Image serving:** `dioxus::desktop::use_asset_handler("media", …)` at shell mount; a pure
  `resolve_media_path(root, req) -> Option<PathBuf>` rejects traversal (unit-testable); `img
  src="/media/<rel>"` (no data URIs for multi-MB scans).
- **Crop tool:** framework-free math in `vitni-ui/src/view_model/crop.rs` (`rect_from_drag`,
  `rect_css`, + proptest); a pure-RSX pointer-event overlay (drag-to-draw, Clear, live percent
  readout); existing crops render as `.crop-outline`. Appears in the media viewer overlay and the
  wizard confirm stage.
- **Media save dialog:** modal; Category = SelectInput over the fixed convention list ∪ existing
  `media/` dirs + free text; Subfolder suggested from metadata; Filename from pure
  `suggest_filename`/`slugify` (keeps æøå, census year shortening) in
  `vitni-ui/src/view_model/media_save.rs`; live relative-path preview. Outputs the relative target
  path; `media-store` writes/checksums/uniquifies.
- **Existing screens:** Media detail Overview preview card; `media_gallery`/`family_media_gallery`
  (`screens/shared.rs`) get real thumbnails + captions + crop outlines.

### 6. `present` capability + `assisted-import` world (ADR 0017 §5, 0.19.0)

Host: a `Presenter` trait (`async fn present(&mut self, payload_json) -> Result<String, PresentError>`)
and `run_assisted_import(component, invocation, request, presenter, progress)`. `vitni-ui`:
`import_payload.rs` (typed, versioned assisted-import presentation contract — parsed like
`vocabulary.rs`, **not** ADR 0022 vocabulary) + an `ImportSession` state machine
(`view_model/import.rs`) in a context-level signal. GUI: the background-task + mpsc/oneshot `Presenter`
wired in `vitni-ui-dioxus/src/services.rs`; the `Tool::Import` wizard screen. Cancellation:
`cancel` response, `progress` cancel points, dropped channel → `capability-error::backend`.

### 7. `digitalarkivet-import` plugin (ADR 0017 §6)

`plugins/digitalarkivet-import` — assisted-import world glue over the pure crate; its Fluent
catalogue (en+no); the wizard flow per the prototype (request → fetch/classify → residence person
loop → resolve scan → `fetch-and-store` with a plugin-proposed editable path → transcribed fields or
`interpret-media` → `present` prefilled confirm → `commands` resolve-or-create Person by `ExternalId`
+ Source/Citation/Repository with URN + retrieval date + Media attach → summary). Grant site: grants +
`NetPolicy` host list (`*.digitalarkivet.no`). The wizard's Source stage discovers installed
assisted-import-world plugins — Digitalarkivet is the first, not a hardcode.

## Files (indicative — detailed per PR)

- `vitni-plugin-host`: `wit/host.wit` (`net`, `media-store`, `ai`, `present`, `assisted-import`
  world, `media-crop`, `fixture` `try-*`); `src/capability.rs` (+`Net`/`MediaStore`/`Ai`/`Present`);
  `src/state.rs` (impls: reqwest client, `NetPolicy`, media root, argv/vision-api runners, `Presenter`
  dispatch); `src/lib.rs` (`Invocation.net_policy`, `provenance_confidence`, `run_assisted_import`);
  `src/bindings.rs`, `src/discovery.rs`.
- `vitni-digitalarkivet` (new crate): `html` parsers + `api` seam; `tests/fixtures/` (verbatim).
- `vitni-app`: `[ai]` config types in the client scope of `ConfigStore`; `MediaRefInput`; six
  `attach_*_media` + `update_*_media_ref` use-cases; `MediaRefSummary` (+`crop`/`path`/`mime`); the six
  projection mapper sites; `lib.rs` re-exports (incl. `Rect`/`CropRect`).
- `vitni-ui`: `import_payload.rs`; `view_model/{import,crop,media_save}.rs`; ~45 Fluent keys
  (en+no) `import-*`, `media-viewer-*`, `media-save-*`, `nav-import`.
- `vitni-ui-dioxus`: asset handler + `resolve_media_path`; `screens/import.rs`; media viewer
  overlay + crop tool + save dialog; gallery thumbnails; `services.rs` Presenter.
- `vitni-gramps-xml`: `<region>` ↔ `crop` mapping.
- `plugins/digitalarkivet-import` (new) + `plugins/plugin-api` re-exports/helpers + fleet `with:`
  sweeps + `plugins/fixture` `try-*`; `xtask` build-plugins entry.
- Workspace `Cargo.toml`: `reqwest`, `sha2`, `mime_guess`, `scraper` (+ dev `wiremock`).
- `prek.toml`: fixture-fixer exclusion for `crates/vitni-digitalarkivet/tests/fixtures/`.

## PR sequence (each green/mergeable; TDD; one opus subagent per PR, sequential, main worktree)

| # | Branch | Content | Key tests |
|---|---|---|---|
| 1 | `docs/phase8-adr-0017` | Research doc, ADR 0017, this plan, mockups (`import.html` new; `media.html` viewer/crop/preview; `index.html` link), roadmap.md/html Phase 8 GUI-only deviation | prek/markdown/link/css hygiene |
| 2 | `feat/host-net-media-store` | WIT 0.16.0 `net`+`media-store`; capability enum; `state.rs` impls; `Invocation.net_policy`; fleet sweep; fixture `try-fetch`/`try-store`; deps | wiremock: grant denial; allowlist deny (host/`http://`/IP/userinfo/redirect hop); final-url; size-cap; timeout; checksum vector; traversal reject; dedup existed/uniquify; `cargo deny` |
| 3 | `feat/digitalarkivet-crate` | `vitni-digitalarkivet` (+`scraper`); verbatim fixtures; prek exclusion | per-fixture parsers; viewer→image chain; URN/ExternalId; classification; malformed-HTML typed errors; wasm32-wasip2 build |
| 4 | `feat/host-ai-capability` | WIT 0.17.0 `ai`; `[ai]` client-scope config; argv + vision-api runners; `Invocation.provenance_confidence`; fixture `try-interpret` | config round-trip; unknown provider → invalid-input; argv-injection safety; timeout kill; vision-api wiremock + env key; denial; `Confidence::Low` on events |
| 5 | `feat/media-crop-plumbing` | WIT 0.18.0 attach-media crop/caption; `MediaRefInput`; six use-case sites; `MediaRefSummary`; `update_*_media_ref`; lib.rs re-exports; gramps-xml region; fleet sweep | attach-with-crop DTO round-trip per owner; supersede + history; gramps region round-trip; WIT plumb-through |
| 6 | `feat/gui-media-crop` | asset handler + `resolve_media_path`; media preview; gallery thumbnails/crop outlines; viewer overlay + crop tool (proptest); save dialog + pure naming fns; i18n; mockup-asset CSS copied | traversal units; SSR (img src, outline, fallback, viewer, dialog, path preview); crop math props; slug/filename (æøå, year) |
| 7 | `feat/host-present-assisted` | WIT 0.19.0 `present` + `assisted-import` world; `Presenter` + `run_assisted_import`; `vitni-ui` `import_payload.rs` + `ImportSession`; GUI Presenter in `services.rs`; fixture `try-present` | scripted-Presenter (submit/cancel/channel-drop); payload parse incl. malformed; session-machine units |
| 8 | `feat/digitalarkivet-plugin` | `plugins/digitalarkivet-import` (world glue; flow per prototype; Source/Citation/Repository + URN citation + retrieval date; ExternalId resolve-or-create); plugin Fluent en+no; `Tool::Import` wizard stages; grant site + NetPolicy; e2e | plugin-host e2e: wiremock serves PR3 fixtures + scripted Presenter → created aggregates, re-run idempotence (created=false, existed=true), cancel mid-flow, denial; SSR per stage |
| 9 | `docs/phase8-closure` | roadmap.md Phase 8 → delivered; roadmap.html sync; issues.md Completed; memory notes | link/i18n/css checks; full gate suite |

Dependencies: 2←4; 2,3,4,5,7←8; 5←6; 1 first; 9 last. PR5/6 are independent of 2–4 and can interleave.

## Verification

Gates before every commit (the full suite runs at PR8 close and PR9):

```
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo xtask build-plugins
cargo nextest run --workspace --all-features --all-targets
cargo xtask i18n-check
cargo xtask css-check
cargo xtask input-guard
cargo deny check
prek run
```

- **Machine-checkable proof (PR8 e2e):** a fixture-served assisted session (wiremock serves the PR3
  fixtures) + a scripted `Presenter` → confirm → import → assert created aggregates; a re-run is
  idempotent (`created = false`, media `existed = true`); cancel mid-flow leaves a partial summary.
- **Manual GUI run** (`cargo run -p vitni-ui-dioxus`): Tools → Import; paste a real Digitalarkivet
  person URL; the scan downloads under `media/<category>/…` with a sha256 checksum; drag the census-line
  highlight; import; verify Person/Source/Citation/Media + `Confidence::Low` + `ExternalId` in the
  record screens; re-import the same URL → no duplicates. Crop tool: Media tab → viewer → drag a face
  region on a photo → supersede visible in History.
- **Docs (PR9):** roadmap/issues/mockups synced; ADR 0017 accepted; research doc committed.

## Notes / risks

- `scraper` on `wasm32-wasip2` is unverified until PR3 (fallback `tl`).
- `present` is the first suspending host call — the `Presenter` contract must not deadlock
  (scripted-presenter + channel-drop tests cover it).
- Worktree nesting breaks `build-plugins` (memory) — subagents stay in the main worktree, sequential.
- `prek.toml` currently names the planned `vitni-import` fixtures path; PR3 updates it to
  `vitni-digitalarkivet`.
