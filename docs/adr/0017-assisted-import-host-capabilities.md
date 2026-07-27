# 17. Assisted-import host capabilities (net, media-store, ai, present)

- **Status:** Accepted
- **Date:** 2026-07-19

## Context

ADR 0011 §3 denied `files` and `net` by construction in the spike (an empty
`WasiCtx`, bytes-in/bytes-out worlds) and named real `net` and file access as
deferred to the import/export breadth phase. ADR 0013 built the bulk
import/export contract and explicitly scoped out "assisted single-record import —
network fetch (`net`), media-library file storage, AI interpretation, and
interactive present-and-confirm — a separate, larger host contract (the
Digitalarkivet work), gated by a later ADR." This is that ADR.

Roadmap Phase 8 is online, record-by-record assisted import from Digitalarkivet:
fetch a source page, resolve and download the scan into the workspace media
library, optionally AI-interpret a hard-to-read (gothic) page, present the
interpreted record **and the scan** for the user to confirm or edit, and import it
as low-confidence Software-agent assertions with an `ExternalId` back to the
record. The proven shape is the owner's prototype `~/Genealogi/scripts/sort-inbox.py`
(fetch → classify → resolve scan → download → AI-suggest fields → user confirms →
file into category folders → emit Source/Citation/Repository suggestions).

The research (`docs/research/digitalarkivet.md`, 2026-07-19) fixes one premise
this ADR depends on: **there is no anonymous public REST API** for the genealogy
records — `api.digitalarkivet.no` is 404, the documented Arkivverket APIs are
institutional deposit/preservation, and IIIF is an experimental photo-scoped
technology test. Search, page parsing, and scan-URL resolution all ship as
**HTML-first, unauthenticated, GET-only** access over `*.digitalarkivet.no`. So
`net` needs no token/auth surface, but it must follow redirects (the scan viewer
302-chains to the permanent-image host) and send an honest, non-crawler
`User-Agent` (`robots.txt` blocks 151 named crawlers and requests a 5 s
crawl-delay for the rest).

The GUI is the interactive frontend (Dioxus behind `genealogy-ui`, ADR 0008). This
ADR sits above the `genealogy-app` DTO boundary (ADR 0006), reuses the plugin host
(ADR 0011) and its deny-by-default grant model, the Software-agent provenance
(ADR 0001/0004/0007), and the client-scope config store (ADR 0015). It does not
restate them.

## Decision

1. **Four new deny-by-default capability interfaces — `net`, `media-store`, `ai`,
   `present` — under the existing `Grants` model.** Each is always linked and
   gated by the instance grant set in its host implementation (ADR 0011 §2),
   returning `capability-error::denied` when ungranted. No ambient WASI sockets or
   preopened directories are added; `files`/`net` stay denied by construction
   except through these host-mediated interfaces. The `Capability` enum and
   `Grants` gain `Net`, `MediaStore`, `Ai`, `Present`.

2. **`net`: GET-only, host-mediated fetch under a caller-supplied `NetPolicy`.**
   `fetch(url) -> http-response { status, final-url, headers, body }` returns the
   whole body (HTML/JSON) under a size cap; **large binaries never cross the guest
   boundary** (they go through `media-store`, §3). The policy is threaded per
   invocation like `BulkIo`: `NetPolicy { allowed_hosts: Vec<HostPattern>,
   max_response_bytes (default 8 MiB), timeout (default 30 s) }`, `HostPattern` =
   exact host or `*.suffix`. HTTPS-only; URLs with userinfo (`user@host`) or
   IP-literal hosts are denied (allowlist-bypass vectors). Redirects are followed
   (cap 10) with a custom `reqwest` redirect policy that **re-checks every hop**
   against the allowlist; `final-url` is reported. Timeout via host-side
   `tokio::time::timeout`; the size cap is enforced while streaming the body
   (guarding `Content-Length` lies). Client: **`reqwest`** (`default-features =
   false`; `rustls-tls`, `gzip`, `stream`). No authentication surface — access is
   anonymous per the research.

3. **`media-store`: host-owned writes strictly under the workspace media root.**
   `fetch-and-store(url, suggested-path)` (host downloads under the same
   allowlist/timeout, a separate larger **64 MiB** binary cap, streamed to a temp
   file then renamed) and `store(bytes, suggested-path)` (small guest-held
   payloads and tests), both returning `stored-media { relative-path, checksum,
   mime, size, existed }` with `checksum = "sha256:<hex>"` (**SHA-256 via `sha2`**).
   Path safety is host-enforced: reject absolute paths, `..` components, and
   backslashes; sanitize each component; the resolved target must remain under
   `<workspace>/media/` (the dir `workspace.rs` already creates at init). Dedup:
   same sanitized path **with the same checksum** → return `existed = true`
   (idempotent re-run); different bytes → uniquify (`-2`, `-3`). The plugin
   **proposes** names (`<category>/<date>_<place>_<event>_<name>.<ext>`), the user
   edits them, and the host enforces only safety — the store stays
   convention-free. The **Media aggregate stays metadata-only** (path + checksum +
   mime); bytes live on disk.

4. **`ai`: named, config-declared, multi-provider interpretation in client
   scope.** `interpret-media(provider: option<string>, media-path, prompt) ->
   string` returns the **raw model text**; the plugin owns JSON extraction/repair
   (the host stays schema-opaque, consistent with every other payload). `provider`
   names an `[ai.providers.<name>]` entry; `none` resolves `[ai].default`; an
   unknown name is `invalid-input`. `media-path` is validated under the media root.
   Config lives in **client/presentation scope** (ADR 0015 `ConfigStore`) — the
   provider inventory is a property of this machine and user (an installed CLI on
   `PATH`, an env-var credential), not of the dataset. Two provider kinds:
   - `kind = "command"` — `tokio::process::Command` with an **explicit argv vector,
     no shell, ever**; `{prompt}`/`{media}` substituted as whole argv elements so
     plugin-controlled prompt text cannot inject arguments; `cwd` = workspace;
     `tokio::time::timeout` + kill-on-drop; non-zero exit → `backend` with a stderr
     excerpt.
   - `kind = "vision-api"` — OpenAI-compatible `POST {url}/chat/completions` with a
     base64 `image_url`; the API key is read at call time from the env var named by
     `api-key-env` (**the key never lives in config or logs**); the provider URL is
     user-authored config (host policy, **not** the plugin allowlist), but
     HTTPS-only and the timeout still apply.

   A `kind = "plugin"` (a future `ai-provider` WASM world) is **reserved and named,
   not built**.

5. **`assisted-import` world + `present`: one long invocation carrying a typed,
   versioned presentation contract.** The world exports
   `run-assisted(request-json) -> result<summary-json, string>` and imports `log`,
   `query`, `commands`, `progress`, `net`, `media-store`, `ai`, `present`. One
   invocation runs the whole session; wizard state (parsed records, stored media
   paths, AI output, created ids) is plain **guest memory** for the life of the
   call — no state-blob protocol. `present.show(payload-json) -> result<response-json,
   capability-error>` is an **async host function that suspends** on a frontend
   `Presenter` (the async sibling of `progress`); the GUI runs the invocation as a
   background task and its `Presenter` forwards the payload to a Dioxus signal and
   awaits the user's answer. Response is
   `{"kind":"submit","action":…,"values":{…}} | {"kind":"cancel"}`.

   **The `present` payload is a typed, versioned assisted-import presentation
   contract — NOT the ADR 0022 UI vocabulary.** It is parsed in framework-free
   `genealogy-ui` (`import_payload.rs`, mirroring `vocabulary.rs::parse`) and
   rendered by a **first-party wizard Tool screen** (`Tool::Import`, like the Merge
   wizard). Stage payloads carry a records list, a per-record confirm block
   (`fields: [{key, label, value}]`, scan relative-path, suggested line-region,
   provenance preview, suggested save-path), and a summary — **the plugin never
   describes widgets**. The host stays opaque to the payload (consistent with
   ADR 0012/0022 and GEDCOM bytes). Cancellation has three layers: the `cancel`
   response, `progress.report`'s `proceed`/`cancel` return at long steps, and a
   dropped GUI channel surfacing as `capability-error::backend`.

6. **Source-neutrality: capabilities and the present contract are
   archive-agnostic.** Nothing Digitalarkivet-specific appears in any WIT
   interface or in the presentation contract. Any future assisted-import plugin
   (a different national archive, FamilySearch, etc.) compiles against the same
   `assisted-import` world and renders through the same first-party wizard. **Per-
   plugin specifics live only at the grant site**: the plugin's own `NetPolicy`
   host allowlist and its own grant set, hardcoded by the caller (consistent with
   ADR 0011/0013's "no manifest; caller hardcodes grants per role"). The wizard's
   Source stage **discovers** installed assisted-import-world plugins from the
   compiled components (as `discovery.rs` already reads worlds), with no hardcoded
   plugin. `fields` are generic key/label/value lists resolved through the plugin's
   own Fluent catalogue (ADR 0012 §5), not a census schema. Idempotent re-import
   and dedup work generically via `ExternalId.authority` per source (data-model §11).

7. **Provenance: assisted commands stamp `Confidence::Low`; operator stays
   `AgentKind::Software`.** An `Invocation.provenance_confidence: Option<Confidence>`
   template is threaded through the host; the assisted caller sets
   `Confidence::Low`, and the host's `commands` implementation applies it to every
   `Provenance` (closing today's `Provenance::default()` gap for this flow). The
   operator remains the plugin's `AgentKind::Software` session — every field was
   user-confirmed, so these are software-mediated, human-reviewed claims.
   `AgentKind::AiModel` is **reserved for unreviewed batch AI assertions** (out of
   scope).

8. **Resource bounding: fuel for guest compute, host-side timeouts for I/O; epoch
   interruption stays deferred.** Fuel does not tick during host awaits — correct
   here, since user think-time and downloads must not burn fuel; guest compute
   (HTML parsing) stays fuel-bounded and assisted invocations get a larger
   `ResourceBudget`. Every host I/O await (`net`, `media-store` download, `ai`) is
   bounded by a host-side `tokio::time::timeout`. The only deliberately unbounded
   await is the **cancellable** `present.show` (user think-time). **Epoch-based
   interruption** (ADR 0011 §4's named successor) is therefore still not needed and
   remains deferred; this ADR records that explicitly.

9. **Host-API version plan: one minor bump per WIT-touching PR, fleet rebuilt in
   lockstep** (the established no-back-compat policy; each bump gets its own
   changelog paragraph, matching 0.3.0 → 0.15.0):

   | Bump | Content |
   | --- | --- |
   | 0.15.0 → 0.16.0 | `net` + `media-store` interfaces; `host-imports` + `fixture` (`try-fetch`, `try-store`) grow |
   | 0.16.0 → 0.17.0 | `ai` interface; `fixture.try-interpret` |
   | 0.17.0 → 0.18.0 | `media-crop` record; `attach-*-media` gain `crop`/`caption` (breaking `commands`) |
   | 0.18.0 → 0.19.0 | `present` interface; `assisted-import` world; `fixture.try-present` |

   `run-assisted`'s grant set is Log + Query + Commands + Progress + Net +
   MediaStore + Ai + Present. Existing worlds' grants are unchanged.

## Rationale

- **Long invocation + suspending `present` vs. stateless round-trips (5).** A
  stateless ui-panel model (ADR 0022 `handle-action` re-instantiates per call)
  would force wizard state into an opaque state-blob echoed through every panel —
  a new protocol to design, version, and debug — and ADR 0022 explicitly scoped
  out long-running/streaming actions, yet every assisted step *is* long-running
  (fetch, download, AI). A single straight-line guest invocation with a suspending
  `present` maps 1:1 onto the linear prototype flow.
- **Typed present contract, vocabulary extensions rejected (5).** An earlier design
  considered extending the ADR 0022 UI vocabulary additively (a `Form.image`, field
  `value` prefill, `Table.select-action`) so the generic panel interpreter could
  render the confirm screen. That is **rejected as YAGNI**: the assisted confirm
  screen is a bespoke split view (scan + crop rectangle left, editable fields +
  provenance card right) that a first-party Tool screen renders far better than a
  generic widget vocabulary, and loading the vocabulary with import-only affordances
  couples two unrelated schemas. A dedicated, versioned assisted-import payload —
  parsed by `genealogy-ui`, rendered by `Tool::Import` — keeps the plugin describing
  *data* (not widgets), leaves the ui-panel vocabulary untouched, and keeps the host
  opaque to the payload exactly as it is to GEDCOM bytes and ui-panels.
- **Custom `net` interface vs. `wasi:http` (2).** `wasi:http` is standard but bypasses
  the uniform `Grants` + `capability-error::denied` model, would put the allowlist
  outside our policy seam, and pulls in `wasmtime-wasi-http`. A small GET-only host
  fn keeps the per-hop allowlist re-check and the deny-by-default shape. `ureq`
  (blocking) and raw `hyper` (hand-rolled redirects/TLS) were rejected against
  `reqwest`'s async + custom-redirect-policy fit.
- **SHA-256 for `media-store` (3).** Dedup-by-checksum must be collision-resistant
  (Gramps' md5 is legacy/broken); sha256 is verifiable with ubiquitous tooling.
  `blake3` is faster but non-standard for interchange with no measurable win at scan
  sizes, where download time dominates.
- **Client-scope `ai` config (4).** The provider inventory (a CLI on `PATH`, an
  env-var key) is machine/user-local, not dataset state; shipping command names and
  env-var names with the workspace would be wrong, and on the Phase 13 server the
  client still holds its own providers. Argv-vector execution with no shell makes
  hostile prompt text inert.
- **Host opacity of all payloads.** `net` bodies, `ai` text, and `present` payloads
  are all opaque to the host — the same boundary discipline as ADR 0012/0013.
- **Source-neutrality (6).** Keeping the archive out of the WIT and the contract
  means the second import source is a new pure crate + thin plugin + a grant-site
  allowlist, not a host change — the same "pure format crate + thin glue" shape
  ADR 0013 established for bulk formats.

## Consequences

### Positive

- The surface ADR 0011 §3 and ADR 0013 deferred lands behind the same
  deny-by-default grant model, with a per-hop-checked host allowlist and no ambient
  WASI network/filesystem access.
- Scans land in the workspace media library with verifiable SHA-256 checksums and
  idempotent re-import (path+checksum dedup, `ExternalId` by URN).
- The assisted flow is source-neutral: a second archive is additive (pure crate +
  thin plugin + grant-site allowlist), and the wizard discovers plugins rather than
  hardcoding one.
- Assisted assertions are auditable low-confidence Software-agent claims through the
  unchanged `decide()` path; no new mutation surface.

### Negative / costs

- Four new capabilities and four host-API minor bumps in one phase, each requiring a
  fleet `with:` sweep and changelog paragraph.
- `reqwest` + `rustls`/`aws-lc-rs`, `sha2`, and `mime_guess` add dependency weight to
  the host (each cleared through `cargo deny check`).
- `present` introduces the **first suspending host call** — a new liveness class the
  `Presenter` contract must respect (scripted-presenter and channel-drop tests cover
  submit/cancel/backend paths so it cannot deadlock).
- A second presentation schema (the assisted-import contract) to version alongside
  the ADR 0022 vocabulary, rather than one shared vocabulary.

## Out of scope

- **A CLI assisted frontend.** The roadmap's "CLI renders the image inline (kitty
  graphics / sixel)" is **dropped** (owner decision); the interactive present-and-
  confirm is GUI-only — the first-party `Tool::Import` wizard renders the `present`
  payload. `present` is frontend-neutral, so a CLI presenter could be added later,
  but none ships in Phase 8.
- **A config-file `net` allowlist override.** The allowlist is hardcoded at the grant
  site per plugin role; a user-editable override is deferred (third-party trust,
  ADR 0014 / Phase 11).
- **A content-addressed whole-library media index.** Dedup is path+checksum and
  upstream (image URL, `ExternalId`); a persistent content-addressed index is not
  built.
- **`kind = "plugin"` AI providers** (an `ai-provider` WASM world) — reserved, named,
  not built.
- **Epoch-based interruption / wall-clock guest interruption** — still ADR 0011 §4's
  named successor; unneeded here because every non-`present` await is timeout-bounded
  and `present` is cancellable.
- **Unreviewed `AgentKind::AiModel` batch assertions** — assisted import is
  human-reviewed; batch AI claiming is a later, separate contract.
- **Rate limiting / politeness delays beyond timeouts.** The interactive, low-volume
  flow stays well under `robots.txt`'s 5 s crawl-delay in practice; a modest
  inter-request delay is noted as a cheap future addition (research §5), not built
  now. Non-GET HTTP is also out — the flow is read-only.
- **DNS-rebinding pinning.** Mitigated by the hostname allowlist against known archive
  hosts and the userinfo/IP-literal denial; pinning resolved addresses is not done.

## References

- ADR 0007 — the plugin system: §6 capabilities, §7 Software provenance, §12
  coarse-grained boundary.
- ADR 0011 — the host this ADR extends: §2 deny-by-default grants, §3 deferred
  `files`/`net`, §4 resource limits (fuel, epoch successor).
- ADR 0012 — plugin-UI vocabulary (host opacity of UI payloads); the vocabulary this
  ADR deliberately does **not** extend.
- ADR 0013 — the bulk import/export contract whose out-of-scope this ADR fills; the
  pure-crate + thin-glue and `ExternalId` re-import strategy reused here.
- ADR 0015 — the config split and `ConfigStore`; `ai` config lives in client scope.
- ADR 0022 — plugin-UI panels, actions, and submission round-trip; the `present`
  suspend/response shape parallels its submission model.
- `docs/data-model.md` §11 (`ExternalId`, re-import idempotency), §13 (media,
  checksums).
- `docs/research/digitalarkivet.md` — the API-vs-scrape findings this ADR's `net`
  and source-neutrality decisions rest on.
- `docs/archive/plans/assisted-import.md` — the phase plan and PR sequence.
- `docs/roadmap.md` Phase 8.
