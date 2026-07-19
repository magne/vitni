# Integrating with Digitalarkivet — API vs. scrape, per feature

> Researched 2026-07-19 from fetched primary sources (digitalarkivet.no, its
> `robots.txt` and privacy policy, the Arkivverket GitHub org, and the owner's working
> prototype `~/Genealogi/scripts/sort-inbox.py` verified against live markup). Findings
> that will drift — the absence of a public search API, `robots.txt` contents, whether
> IIIF is production — are flagged inline; **re-verify before implementing ADR 0017's
> `net` allowlist and the `genealogy-digitalarkivet` parsers**. Every URL below was live
> when fetched.
>
> Companion: the assisted-import architecture is [ADR 0017](../adr/0017-assisted-import-host-capabilities.md)
> and the phase plan is [`../plans/assisted-import.md`](../plans/assisted-import.md).

---

## 1. What "the Digitalarkivet API" turns out to be

Searching for an official REST API surfaces real Arkivverket/Nasjonalarkivet APIs — but
they are the **wrong ones** for assisted import:

- **Bevaringstjeneste / arkivpakke APIs** (`digitalarkivet.no/content/1243`, `/1279`) —
  system-to-system *deposit and preservation* endpoints for archive institutions to
  ingest datasets and archive packages. Audience: "Arkivverket, kommunale og private
  arkivinstitusjoner". Not record retrieval.
- **Noark 5 tjenestegrensesnitt** (`github.com/arkivverket/noark5-tjenestegrensesnitt-standard`)
  — a records-management interchange standard for case/archive systems, authenticated with
  OpenID Connect bearer tokens. Not genealogy records.
- **NB Digital bevaring** (`digitalpreservation.no`, National Library) — OAuth2
  client-credentials submission/dissemination for digital preservation. A different
  institution entirely, listed only because it ranks for "Arkivverket API".

Probing the obvious host directly: **`https://api.digitalarkivet.no/` returns HTTP 404** —
there is no documented public API root there, no Swagger/OpenAPI, no genealogy search
endpoint. The genealogy record surface of Digitalarkivet is delivered as **server-rendered
HTML** through the search pages below, not a JSON API.

**Conclusion:** there is **no anonymous public REST API for census/person/church records**.
Every feature this phase needs is reached through the HTML site, exactly as the prototype
already does.

## 2. The search surface (HTML, form-driven)

The public entry points are the "doors" on the front page
([`digitalarkivet.no/en/`](https://www.digitalarkivet.no/en/)): **Search for individuals**,
**Find source**, **Scanned material**, **Censuses**, **Parish registry**, **Property**,
**Law and justice** (Martin Roe Eidhammer's 2026-01 guide, martinroe.com, is a good current
map of which door does what). The one that matters for assisted import is **Detailed person
search** — [`/en/search/persons/advanced`](https://www.digitalarkivet.no/en/search/persons/advanced) —
a GET form over criteria (first/last name with `*` wildcards, gender, role, event/birth
date and place, municipality/county, and a long census/source-type facet list: 1801, 1815,
…, 1875, 1885, 1891, 1900, 1910, 1920, 1960 censuses, church books, probate, emigration,
etc.). Results are an HTML list; each hit that has been imaged carries a **"See scanned
version"** link into the scan viewer.

Third-party tooling confirms the shape: the R package **`cstubben/aRkivet`**
(`rdrr.io/github/cstubben/aRkivet`) implements `advanced_search(...)` purely by **formatting
the advanced-person-search URL and scraping the returned HTML table** into a data frame
(columns: name, residence, year, type, role, event, birth, place, family position, source,
url) — no API involved. That is the only known programmatic path to search results, and it
is a scrape.

> **Uncertain:** there may be an internal JSON/XML endpoint behind the search form (the site
> is a modern SPA-ish app in places). None is documented or advertised, and relying on an
> undocumented internal endpoint is fragile. Treat search as **HTML scrape** unless a stable
> documented endpoint is found at implementation time.

## 3. The page / scan-URL chain (HTML, prototype-proven)

The owner's prototype (`sort-inbox.py`, the `--- Digitalarkivet ---` section) already resolves
a person record to its permanent scan image, with selectors **verified against live markup**.
This is the reliable, shipping path:

1. **Classify the URL** by path (the prototype's `da_page_type`):
   `/census/person/…` → person page; `/census/rural-residence/…` or
   `/census/urban-residence/…` → residence page.
2. **Residence → person links.** A residence page lists its household; collect every
   `/census/person/…` anchor (absolute, de-duplicated).
3. **Person → scan viewer.** On a person page the scan link is the anchor
   `id="scannedImageLink"` (also carrying `data-scans`), whose `href` is the media viewer
   on `media.digitalarkivet.no` (e.g. `…/fs10771822220997`). Fallbacks in priority order:
   any `data-scans` anchor, an anchor whose text matches `skannet|scanned`, then any
   `media.digitalarkivet.no` link/image. **`og:image` is the site logo on person pages and
   must not be used.**
4. **Viewer → permanent image.** On the viewer page the `<input id="permanent_image_link">`
   value is the permanent URL, e.g.
   `https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs10771822220997.jpg`. Here the viewer's
   `og:image` *is* that same image (usable as a fallback). The viewer 302-redirects to the
   URN host, so the fetch chain must **follow redirects and report the final URL**.

Hosts touched: `www.digitalarkivet.no` (pages), `media.digitalarkivet.no` (viewer),
`urn.digitalarkivet.no` (permanent images). A `*.digitalarkivet.no` allowlist covers all
three; an exact-host list (`www.`/`media.`/`urn.`) is tighter. Church-book pages follow the
analogous pattern and are parsed from the fixtures at implementation time.

The permanent `URN:NBN:…` string is a stable external identifier — it becomes the
`ExternalId { authority: "digitalarkivet", value: <urn> }` that makes re-import idempotent
(data-model §11), and the citation's archival reference (the prototype's
`extract_source_ref` pulls the `URN:NBN` or the `fs`-scan id out of the filename).

## 4. IIIF — exists, but not the shipping path

Arkivverket has **IIIF work**, so this was checked directly:

- [`github.com/arkivverket/digitalarkivet-iiif-presentation`](https://github.com/arkivverket/digitalarkivet-iiif-presentation)
  — a PHP library that *generates* IIIF **presentation** manifests (BSD-3-Clause, created
  2024-11, 2 stars, topic `oslo-1`). A manifest generator, not a documented public image
  endpoint over the census corpus.
- [`digitalarkivet.no/content/1249`](https://www.digitalarkivet.no/content/1249/integrering-av-iiif-for-foto)
  — "Integrering av IIIF for foto" describes IIIF explicitly as a **"Test av teknologi"**
  (technology test) for cross-format photo search.

So IIIF is **experimental and photo-scoped**, not a stable, documented Image API 3.0 surface
over the census/church scans this phase needs. The permanent `urn.digitalarkivet.no/URN:NBN:…jpg`
route is the proven one. `genealogy-digitalarkivet` should keep the scan-resolution logic
behind a function boundary so an IIIF `info.json`/image path can be added later without
touching the plugin flow — but **ship HTML-first**.

## 5. Access, licensing, and politeness constraints

**Anonymous access is expressly permitted.** The privacy policy
([`/en/content/privacy`](https://www.digitalarkivet.no/en/content/privacy), and the Norwegian
[`/content/privacy`](https://www.digitalarkivet.no/content/privacy)) states you do **not** need
to register to search or browse digitised archives; a user account is only required for
forums, saved settings, restricted ("klausulert") material, PDF-booklet creation, and the
institutional publishing tools. Registration is gated by a "not a robot" CAPTCHA whose stated
purpose is "å unngå at programmer logger seg inn hos oss" (to stop programs logging in). So
the plugin should stay **unauthenticated and GET-only**; it must never attempt login.

**Reuse of downloaded scans** ([`/content/1570`](https://www.digitalarkivet.no/content/1570/videreformidling-av-skannede-dokumenter)):
non-restricted scanned documents "kan fritt brukes videre både i trykksaker og på internett"
(may be freely reused in print and online); it is **requested that you name the archive
institution** that manages the source. Restricted documents are governed by the confidentiality
declaration the accessing user signed and are **not** freely reusable. Practical consequence:
the importer targets **non-restricted** material, and the generated citation/source should
name the managing repository — the prototype's `REPOSITORY_BY_CATEGORY` already does this
("Digitalarkivet (Arkivverket)").

**`robots.txt` is strict** (fetched 2026-07-19). It **`Disallow: /` for 151 explicitly named
crawlers**, including AI crawlers — `GPTBot`, `ClaudeBot`, `Googlebot-Extended`,
`PerplexityBot`, and many more. For every **other** user-agent it allows crawling with
**`Crawl-delay: 5`** (five seconds between requests). No `Allow`, no `Sitemap`. Implications
for ADR 0017's `net` capability:

- The host's HTTP client **must send a real, honest, non-crawler `User-Agent`** (an
  identifying product string such as `genealogy/<version> (+contact)`), and must **not**
  impersonate a browser or use any of the blocked names.
- `robots.txt` **requests a 5-second crawl-delay**. Assisted import is low-volume and
  user-driven — a session fetches a handful of pages plus one scan per record, at human
  pace, not a crawl. That is materially gentler than crawling, but it is **honest to note**
  that the plan deliberately puts "rate limiting / politeness delays beyond timeouts" out of
  scope (ADR 0017). Recommendation: treat the 5 s crawl-delay as the ceiling this flow is
  already under in practice, and **flag a modest inter-request delay as a cheap future
  politeness measure** rather than pretending the constraint does not exist. The interactive
  nature (the user reviews each record) provides most of the spacing for free.
- IP addresses are logged server-side (standard practice, stated in the privacy policy);
  there is no rate-limit *number* published. Undocumented — do not assume a budget; rely on
  per-request timeouts and the naturally low volume.

## 6. Per-feature decision

| Feature | API available? | Decision | Evidence |
| --- | --- | --- | --- |
| **Search** (person/record lookup) | No anonymous public API (`api.` = 404; only deposit/Noark5 APIs exist) | **HTML-first scrape** of `/search/persons/advanced` results; keep an `api` module seam | §1, §2; `cstubben/aRkivet` scrapes it |
| **Page parsing** (person / residence / church-book) | No | **HTML scrape**, prototype-proven selectors | §3; `sort-inbox.py` verified live |
| **Scan-URL resolution** (viewer → permanent `URN:NBN:…jpg`) | No (IIIF experimental, photo-scoped) | **HTML scrape** the `scannedImageLink` → `permanent_image_link` chain, follow redirects, report final URL | §3, §4 |
| **Image download** | Permanent `urn.` URL is a direct file | Host `media-store.fetch-and-store` (bytes never enter the guest) | §3; ADR 0017 §C |

This matches the plan's default: **search prefers the API if anonymous access suffices —
which it does not, so search also ships HTML-first**; the page/scan chain ships HTML-first
regardless. Nothing in the flow requires authentication or a token.

## Verdict

Build the Digitalarkivet integration as an **HTML scraper over `*.digitalarkivet.no`**, not
an API client. Concretely, for ADR 0017 and `genealogy-digitalarkivet`:

1. **No auth, GET-only, honest `User-Agent`.** The `net` capability fetches public pages
   anonymously; it must not log in and must not use a `robots.txt`-blocked crawler UA.
2. **`net` allowlist = `*.digitalarkivet.no`** (or exact `www.`/`media.`/`urn.`), HTTPS-only,
   **redirect-following with a final-URL report** (the viewer 302-chain needs it).
3. **Search is a scrape too.** Design `genealogy-digitalarkivet` with `html` parsers as the
   shipping path and a thin, empty `api` module seam so a documented endpoint (or IIIF) can
   be adopted later without reworking the plugin flow.
4. **Idempotency via `URN:NBN`.** The permanent image URN is the `ExternalId`
   (`authority = "digitalarkivet"`); re-import resolves by it (data-model §11).
5. **Citation attribution.** Generated Source/Citation names the managing repository
   ("Digitalarkivet (Arkivverket)") per the free-reuse terms; target non-restricted material
   only.
6. **Politeness, honestly.** ADR 0017 scopes rate-limiting out, but `robots.txt` requests a
   5 s crawl-delay; the interactive, low-volume flow already stays well under a crawl, and a
   small inter-request delay is a cheap future addition to record as a follow-up rather than
   ignore.

Open items to re-verify at implementation time: whether an undocumented internal
search-results JSON endpoint exists and is stable (§2); whether IIIF has graduated from
"technology test" to a documented image API over the census corpus (§4); the current
`robots.txt` UA list and crawl-delay (§5).
