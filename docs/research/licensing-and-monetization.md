# Research — licensing and monetization for the public release

- **Status:** Findings and a recommendation; **the decision is not made.** Nothing is applied: the
  workspace still declares `MIT OR Apache-2.0` and the repository is still private. The live fork in
  the road is [§3.2](#32-copyleft--sold-exceptions--recommended) (AGPL-3.0-or-later, recommended)
  versus [§3.3](#33-fair-source--delayed-open-source-fsl-busl) (FSL-1.1-Apache-2.0) — the
  requirement as stated is not expressible in an OSI-approved licence, so this is a choice between
  two partial answers, scored in [§8](#8-recommendation). A licence change needs an ADR (**0034** is
  the next free number) and the file work listed in [§9](#9-follow-ups).
- **Date:** 2026-08-13
- **Audience:** whoever decides the licence before the repository goes public — a decision that is
  cheap today and partly irreversible tomorrow.
- **Companion:** [`plugin-signing-and-trust.md`](plugin-signing-and-trust.md) (the signing/trust
  machinery the paid-plugin line in [§6](#6-paid-plugins--what-works-and-what-does-not) rests on),
  ADR 0007/0011/0014 (the plugin boundary), [`../release.md`](../release.md) (what is actually
  shipped today).
- **Verified when written:** dependency licences from `cargo deny --all-features list` against the
  current `Cargo.lock`; CRA dates from the Commission's own pages and Commission guidance
  C(2026) 5252 of 27 July 2026; licence texts fetched from `fsl.software` and
  `polyformproject.org`; Norwegian statute text from Lovdata. Ecosystem facts drift — re-verify
  before ADR 0034.

## Question

The stated goal is a permissive licence (MIT or BSD) **and** monetary payback from commercial use.
Those cannot both hold in one licence: a permissive licence *is* the promise that nobody owes you
anything, forever, for every version already published. So the real question is which of the
several things "no unpaid commercial use" can mean, and what each escape from permissiveness costs.

Two answers narrow the field before any analysis:

1. **Only free-riding gets charged.** Stated precisely: *anyone may use the app; nobody may make
   money reselling the binaries or offering it as a service; paid plugins stay possible later.*
   Explicitly **not** objected to: a professional genealogist billing clients for research done
   with the app, a distribution shipping it free, a consultancy charging to install and configure
   it. So the whole disputed territory is two scenarios — **paid resale of binaries** and **paid
   hosting** — and every other commercial use is welcome.
2. **It must stay OSI-approved open source.** That rules out FSL, BUSL, Elastic 2.0, PolyForm and
   the Sustainable Use family, and keeps distro packaging and crates.io reuse on the table.

**These two answers are not simultaneously satisfiable, and that is the central finding.** No
OSI-approved licence can forbid selling copies or offering a service: OSD #1 requires that the
licence not restrict any party from selling the software, and OSD #6 forbids discriminating against
a field of endeavour. GPL §4 says it out loud — "You may charge any price or no price for each copy
that you convey." So requirement (1) as worded describes a **Fair Source** licence
(FSL-1.1-Apache-2.0 matches it almost verbatim, §3.3), and requirement (2) forbids exactly that
family.

The resolution this report reaches is to keep (2) and accept that (1) is met only partly, because
the two scenarios (1) protects against turn out to be the two least likely to produce revenue for
anyone, including the licensor — see §3.6 for the reasoning and §4 for exactly what is given up.
Given (2), **copyleft is the only remaining mechanism**: it is the one OSI-approved way to put a
free-rider to a choice — publish your own source, or buy an exception from the copyright holder.

## 1. Where the project stands

These are the facts that make the decision cheap right now, and they will not all stay true.

- **Nothing has been granted to anyone.** `gh repo view magne/vitni` reports
  `visibility: PRIVATE`, `licenseInfo: null`. The workspace `Cargo.toml` declares
  `license = "MIT OR Apache-2.0"`, but **there is no `LICENSE` file in the tree at all** — no
  `LICENSE-MIT`, no `LICENSE-APACHE`, no `NOTICE`, no `CONTRIBUTING.md`. A licence field in a
  manifest of an unpublished private repository has granted nothing to nobody. Every option in
  §3 is open, including the ones that stop being available the moment the repo is public.
- **Sole authorship.** 872 commits: 866 from one author, 6 from dependabot (manifest/lockfile
  bumps). No CLA is needed to relicense *what exists*, because there is nothing to collect. This
  is the single most valuable and most perishable asset in this whole analysis — it expires the
  first time an outside pull request is merged without a contributor agreement.
- **Nothing in the docs commits to a position.** `docs/roadmap.md`, `docs/issues.md` and
  `docs/release.md` say nothing about licensing or monetisation; `CLAUDE.md` and `README.md` each
  assert only "keep it permissive", with no reasoning attached that this report needs to overturn.
- **What ships today is Linux-first** — tarball, `.deb`, AppImage (`docs/release.md`). No app
  store. That matters: the sharpest cost of copyleft is app-store distribution (§4), and the
  project is not paying it yet.
- **A web frontend is planned** (roadmap Phase 11, ADR 0016 unwritten). That is what makes AGPL
  rather than GPL the relevant question at all — see §3.2.

## 2. What the dependency tree permits — nothing is blocked

`cargo deny --all-features list` reports 19 distinct licences across the full graph and 0
unlicensed crates. Every entry that looks like a problem is a *choice* of licences, not an
obligation:

| Looks like | Actually |
| --- | --- |
| `GPL-2.0-only` (3 crates) | `ittapi`, `ittapi-sys` are `GPL-2.0-only OR BSD-3-Clause`; `self_cell` is `Apache-2.0 OR GPL-2.0-only`. Take the permissive option. |
| `LGPL-2.1-or-later` (2) | `r-efi` is `MIT OR Apache-2.0 OR LGPL-2.1-or-later`. |
| `MPL-2.0` (8) | Weak, file-scoped copyleft — see below. |
| `Unicode-3.0` (43), `CDLA-Permissive-2.0` (3), `NCSA`, `BSL-1.0`, `0BSD`, `CC0-1.0`, `Unlicense`, `ISC`, `Zlib`, BSD-2/3-Clause | All permissive, all in `deny.toml`'s allow-list. |
| `MIT-0` (2), `BSD-1-Clause` (1) | Permissive, and *not* in the allow-list — they pass because they are alternative arms that resolve elsewhere (`dunce` is `CC0-1.0 OR MIT-0 OR Apache-2.0`, `fiat-crypto` is `MIT OR Apache-2.0 OR BSD-1-Clause`). |

**There is no copyleft-only dependency on any path.** So the dependency tree constrains nothing:
permissive, copyleft, source-available and proprietary are all reachable from here. That is worth
stating plainly, because it is the first thing one fears when contemplating a licence change.

**The MPL crates, checked properly.** Eight crates are MPL-2.0: `cssparser`, `selectors`,
`dtoa-short` (pulled by `scraper`, a real — not dev — dependency of `vitni-digitalarkivet`)
and `option-ext` (via `directories` in `vitni-app`). A naive grep for
`Incompatible With Secondary Licenses` hits three of their `LICENSE` files and invites the
conclusion that they cannot be combined with (A)GPL. That conclusion is wrong: the phrase is the
heading of **Exhibit B inside the standard MPL-2.0 text itself**, present in every verbatim copy
of the licence. What decides the question is whether the notice is *applied*, and it is not — the
source headers carry the plain Exhibit A notice ("This Source Code Form is subject to the terms of
the Mozilla Public License, v. 2.0"). So these files are available as Secondary Licenses under
GPL/AGPL, and equally usable inside a proprietary or source-available larger work under MPL §3.3.
Either direction works. The rule to hold is narrow: **do not vendor or patch those crates
in-tree**, because MPL's disclosure duty follows modified files, and unmodified upstream copies
are satisfied by pointing at crates.io.

Two incidental notes: `option-ext` is removable if it ever matters (`directories` → `etcetera`,
already in the graph via sqlx), and `deny.toml` currently ignores 13 RUSTSEC *unmaintained*
advisories from the GTK3/Dioxus tree — unrelated to licensing, but the same file changes in §9.

## 3. The option space

### 3.1 Permissive + voluntary payback (MIT/Apache-2.0)

Keep the current declaration, publish, and earn from paid convenience builds, app stores,
sponsorship and hosting. This is the Krita and Ardour model, and it demonstrably funds real work:
Krita's store presence (Steam 2014, Microsoft Store 2017, Epic 2021) has been its main income line
with a €19.95 base price, and Ardour has run pay-what-you-want downloads plus subscriptions for a
decade, mostly on payments around US$1.

What it achieves: goal (1) — permissiveness — completely, plus maximum adoption, distro packaging,
zero legal surface, and no CLA ever needed. What it does not achieve: any *enforceable* payback.
Anyone may take `vitni-core`, close it, ship it, host it, rebrand it and undersell you, and
you will have consented in advance. Note that Krita and Ardour are **GPL**, not permissive; their
leverage over resale is copyleft plus a trademark, not generosity.

Verdict: rejected by answer (1), but keep its revenue mechanics — they survive under every other
option and are the only ones with evidence behind them at this scale.

### 3.2 Copyleft + sold exceptions — **recommended**

Publish under a strong copyleft licence and, as sole copyright holder, sell exceptions to whoever
cannot live with it. This is the MySQL and Qt structure ("selling exceptions"), and it is the only
OSI-approved way to make a free-rider pay.

**AGPL-3.0-or-later, not GPL-3.0.** For a desktop binary the two are identical in effect: AGPL §13
only bites on *network interaction*, and a GTK webview does not interact with a user remotely. The
difference is entirely about the planned web frontend. Once a hosted genealogy service exists, §13
is the only licence term in the OSI-approved set that reaches it. The neighbourhood agrees:
Gramps desktop is GPLv2-or-later, webtrees is GPL-3.0, and **Gramps Web — the closest thing to a
competitor for the planned web app — is AGPL-3.0**. Choosing AGPL puts the project in the licence
its own market already expects, not in an eccentric position needing explanation.

Who would actually buy an exception:

- a genealogy vendor embedding the event-sourced core or the projections engine in a closed
  product — the real value in `vitni-core`, and precisely what §3.1 gives away for free;
- an integrator building a closed system for an archive or a records office;
- **you**, for an iOS or macOS App Store build — the store terms are incompatible with (A)GPL
  (§4), and the escape is that a sole copyright holder can licence their own work to themselves
  on any terms. A third party wanting the same route must buy it.

Expected volume: near zero for years. Price this as **option value**, not revenue. The point is
that permissive publication destroys the option permanently, and copyleft preserves it at
approximately no cost while the project has no commercial suitors.

### 3.3 Fair Source / delayed open source (FSL, BUSL)

FSL-1.1-Apache-2.0 is the sharpest instrument available for the stated worry, and it is worth
recording accurately because it *would* be the answer under a different second answer. Its
Permitted Purpose is "any purpose other than a Competing Use", where a Competing Use means making
the software available to others in a commercial product or service that substitutes for it or
"offers the same or substantially similar functionality". It then explicitly permits use "for your
internal use and access", non-commercial education and research, and "in connection with
professional services that you provide to a licensee" — which happens to describe a professional
genealogist's business exactly. Each version converts irrevocably to Apache-2.0 (or MIT) on its
second anniversary. BUSL 1.1 is the same idea with a four-year default and a restriction on
production use itself, plus a per-adopter Additional Use Grant that makes every deployment a
bespoke licence.

**This is the licence that matches requirement (1) as worded.** Paid resale of binaries and paid
hosting are both Competing Uses; everything the requirement explicitly permits, FSL permits by
name. It also has two advantages AGPL cannot offer: it never forces a corporate user's own code
open, so it clears compliance departments that blanket-ban AGPL, and it is **compatible with the
Apple App Store**, which (A)GPL is not (§4).

Where it loses, stated precisely — the packaging cost is narrower than the usual telling:

- **Lost:** Debian, Ubuntu, Fedora, openSUSE and Arch's own repositories, which require an
  OSI/DFSG-free licence. For a genealogy application that is a genuine discovery channel — Gramps
  is one `apt install` away in every distribution, and a niche project dies of obscurity long
  before it dies of free-riding.
- **Kept:** Flathub (which hosts proprietary and source-available apps — `LicenseRef-proprietary`,
  with roughly 6% of apps shipping as `extra-data`), the Microsoft Store, Steam, AUR, Homebrew, and
  the project's own tarball/`.deb`/AppImage. Plus the Apple stores, which AGPL closes off.
- **Unaffected:** crates.io reuse of the format crates, which stay permissive under every option
  in §8.
- **Soft cost:** a contributor and packager chill that RedMonk's March 2026 licensing survey still
  measures as statistically invisible adoption for the whole source-available category. Hypothetical
  while the project has no contributors, but its realistic contributor pool — genealogy hobbyists
  next door to Gramps (GPLv2+), webtrees (GPL-3.0) and Gramps Web (AGPL-3.0) — is exactly the
  population that notices.

**And the protection expires, which matters more here than it would elsewhere.** FSL's conversion
is per version: every release becomes Apache-2.0 on its second anniversary. That is lethal to a
free-rider in the SaaS market FSL was designed for, where a two-year-old build is worthless. It is
close to harmless against a *desktop genealogy application*, where a two-year-old release is still
perfectly usable — a would-be paid host simply waits 24 months and then hosts a version that is
unambiguously Apache-2.0. So for this product FSL buys a **rolling two-year delay on the SaaS
scenario, not a prohibition**, while AGPL's reciprocity never lapses.

Answer (2) rules FSL out anyway. Recorded at this length because it is the option to trade the OSI
constraint for if that constraint is ever revisited — not PolyForm, and not a bespoke clause
bolted onto an OSI licence.

### 3.4 Noncommercial (PolyForm Noncommercial / Small Business, Prosperity)

The literal reading of "no commercial use without payment": free for individuals and, under
PolyForm NC's safe harbours, for charities, educational institutions, public research
organisations and government institutions "regardless of the source of funding" — which in this
domain covers the archives, libraries and historical societies that matter. Everyone else buys.

Rejected, and not only by answer (2). It taxes the professional genealogists who are the project's
best allies and most credible advocates; PolyForm's own drafters concede there is "no 'correct',
faultless definition of noncommercial", and that institutions widely refuse NC terms by policy
because they cannot police occasional commercial use; and it forecloses every packaging channel.
The one real-world adopter path visible in the wild is the reverse of what is wanted here —
projects starting at PolyForm NC and *loosening* to BUSL because NC "blocked legitimate small-team
adopters who would never have shown up on our commercial radar anyway".

### 3.5 Open core over the plugin boundary — **part of the recommendation, not an alternative**

Free core, paid proprietary components. This project has an unusually clean seam for it: the
plugin system is WASM components in separate packages excluded from the workspace, with a signing
trust root, per-plugin capability grants, and a three-layer loader (ADR 0007/0011/0014). Because
the paid parts are separate works, open core needs **no CLA for the open side** — community
contributions to the core are never relicensed, which is exactly the property that survives a
DCO-only decision in §5.

Open core is the answer to "I might want to charge for certain plugins", and it composes with
§3.2 rather than competing with it. §6 is about making it actually work.

### 3.6 Which way you can still move afterwards

Reversibility is asymmetric, and it is the strongest argument against the recommendation in §8, so
it belongs here rather than in a footnote.

- **Future versions are always yours to relicense, in either direction** — but only while you hold
  all rights. Sole authorship makes that true today; §5's CLA is what keeps it true.
- **Published versions are never revocable.** Whatever licence a release went out under, it stays
  available under that licence forever, and anyone may fork from the last such release.
- **Loosening is cheap.** FSL → AGPL, or FSL → permissive, is welcomed by everyone and happens
  automatically per release at the two-year mark anyway.
- **Tightening is expensive.** Open → source-available is the HashiCorp (2023), Redis (2024) and
  Elastic (2021) manoeuvre; each produced a funded fork of the last open release that is still
  alive — OpenTofu, Valkey, OpenSearch — and both Elastic (2024) and Redis (2025) partially walked
  it back. The licence change is legally trivial and the trust cost is not recoverable.
- **Permissive publication is the one irreversible act.** Anything released under MIT/Apache-2.0
  can be taken closed and commercialised by anyone, forever, which is why §8 keeps only the
  commodity interop crates there.

So starting restrictive preserves optionality and starting open spends it. The reason §8 still
recommends starting open: the optionality FSL preserves is optionality over **paid binaries and
SaaS** — a revenue line whose own premise is weak (nobody pays for binaries that are also free;
Krita's and Ardour's store income is donation-shaped, not sales-shaped) and which may never be
pursued. Meanwhile the adoption AGPL buys is needed now.

There is also a **cleaner escape hatch than relicensing**: paid work can arrive as *new private
plugin code* (§6), which is a separate work under its own terms. An AGPL core plus private paid
plugins never requires a relicensing event at all — the tightening conversation simply never
happens.

## 4. What the recommendation costs

Stated plainly, because two of these are places where copyleft does less than people expect.

- **AGPL does not stop paid hosting of an unmodified copy.** §13 obliges an operator to offer
  *their modified version's* Corresponding Source. Someone who hosts the project verbatim and
  charges for it modifies nothing and owes nothing beyond what is already public. What AGPL
  actually buys is that a hosting competitor can never accumulate *closed improvements* — they
  compete on operations against you, from a code base that stays yours to steer. That is a real
  and probably sufficient protection, but it is not "nobody may resell my service".
- **Copyleft does not stop rebranded resale either.** GPL §4 expressly permits charging for
  conveyance. The only lever is trademark, and Krita's licence page shows exactly how it is used:
  "Commercial redistribution is limited, though, because the Krita Foundation owns the trademark.
  If you want to sell Krita on eBay, change the icon and the application name and rebuild Krita
  yourself." **This project cannot make that argument today: "genealogy" is a generic term and
  therefore unregistrable as a mark for a genealogy program.** Choosing a distinctive product name
  before the repository goes public is not branding polish; it is the missing half of the
  protection the licence cannot provide. Filing a Norwegian/EU word mark once a name exists is
  cheap relative to what it defends.
- **App stores.** Apple's store terms impose usage rules on every product distributed through
  them, which GPL §6 / GPLv3 §10 forbid ("no further restrictions"). The FSF's 2010 GNU Go action
  and VLC's removal in January 2011 settled this in practice, and Microsoft's early Marketplace
  simply banned copyleft outright. Consequences here: Krita ships fine on Steam, Epic and the
  Microsoft Store, so those channels stay open; an iOS/iPadOS or Mac App Store build needs you to
  licence your own work to yourself under store-compatible terms, which requires that you still
  hold all rights (§5). Since `docs/release.md` is Linux-first, this cost is deferred, not paid.
- **Corporate AGPL bans.** Some organisations refuse AGPL by policy regardless of how it is used.
  For a genealogy application aimed at individuals and archives this is a small population, and
  the commercial-exception path is the answer for anyone who asks.
- **Publishing an AGPL library crate suppresses its reuse.** True, and intentional for the crates
  in the AGPL column. It is exactly why the commodity interop crates stay permissive.

## 5. Keeping the right to sell: CLA, DCO, or nothing

Every mechanism in §3.2, §3.5 and §6 depends on one thing: that you can still licence the whole
work on terms other than the public one. Today you can. The question is what happens at the first
outside pull request.

| Option | What it preserves | What it costs |
| --- | --- | --- |
| **Broad licence-grant CLA** (Apache ICLA shape, plus an explicit right to sublicense and to relicense) | Everything: commercial exceptions, store builds, a future relicence. | Contributor friction, an intake process to run, and the "you may sell my volunteer work under closed terms" objection, which is legitimate and should be answered in `CONTRIBUTING.md` rather than dodged. |
| **Copyright assignment (CAA)** | Same, plus simpler international enforcement. | Harder to get signed; unnecessary here, since a broad licence grant does the same work with less resistance. |
| **FLA-2.0** (FSFE Fiduciary Licence Agreement) | Same rights, but held in trust with obligations back to contributors, and drafted for civil-law jurisdictions where outright assignment is awkward. | Heavier document; aimed at organisations with governance, which this is not — yet. |
| **DCO only** | Community goodwill, lowest friction. | Kills exceptions the day an outside patch lands: no commercial licence covering the whole codebase, no App Store build, no relicence. Open core (§3.5) still works, because the paid parts are separate works. |

**The Norwegian angle matters for drafting.** Under åndsverkloven (2018) economic rights are fully
assignable — §67 first paragraph, "Opphaveren kan med den begrensning som følger av § 5 helt eller
delvis overdra sin rett til å råde over åndsverket" — but §67 second paragraph codifies the
*spesialitetsprinsippet*: "Ved overdragelse av opphavsrett skal opphaveren ikke anses for å ha
overdratt en mer omfattende rett enn det avtalen klart gir uttrykk for", i.e. an unclear grant is
construed restrictively **in the contributor's favour**. A CLA drafted for this project must
therefore name sublicensing, relicensing and commercial exploitation explicitly; breadth by
implication is precisely what the statute refuses to read in. Moral rights (§5 — attribution and
protection against derogatory use) cannot be waived except for uses "avgrenset etter art og
omfang", so any CLA should promise attribution rather than pretend to acquire the right to drop
it. None of this obstructs the model; it dictates the wording.

One further consequence of sole ownership worth knowing: **you cannot infringe your own
copyright**, so a hosted service you run may include unpublished modifications despite AGPL §13.
That freedom disappears with the first un-CLA'd outside contribution to the served code. If a
hosted service is ever a revenue line, this is a second, independent reason to run a CLA.

Recommendation: **DCO + a broad licence-grant CLA from day one**, with `CONTRIBUTING.md` saying
plainly why it exists and what it permits. The project has no contributors to alienate today and
everything to lose by deciding this after the fact.

## 6. Paid plugins — what works and what does not

The plugin boundary is the best monetisation seam the architecture has, and it is already built:
separate packages, a bundle format with an ed25519 signature over a sha2 digest against an
embedded trust root, trust tiers, deny-by-default capability grants, and a three-layer loader
(ADR 0011/0014, [`plugin-signing-and-trust.md`](plugin-signing-and-trust.md)).

**The §7 exception is a prerequisite, not a courtesy.** Verified against the current manifests: no
`plugins/*` component links host code at all — each depends only on `vitni-plugin-api` (which
depends on `vitni-interchange`) plus a format crate, and communicates over the WIT world. The
derivative-work argument is therefore already weak. Make it explicit anyway, as an additional
permission under AGPLv3 §7 on every AGPL crate:

> Additional permission under GNU AGPL version 3 section 7: if you modify this Program, or any
> covered work, by combining it with a WebAssembly component that interacts with the Program
> solely through the versioned `vitni:plugin` WIT world (or any later version of that world),
> the licensor grants you additional permission to convey the resulting work. Such a component is
> not required to be licensed under the GNU AGPL.

Without it, third-party plugins are legally doubtful and paid ones unsellable — a *buyer*
redistributing host plus plugin needs the permission even though you, as copyright holder, do not.
Krita made the opposite choice deliberately ("The extension API is an integral part of Krita and
is licensed under the GPL. This means that if you distribute a Krita plugin it also has to be
shared under the GNU GPL"), which is a coherent position and the wrong one for a project whose
import/export story is plugins.

Two mechanisms for charging, and only one holds:

- **Works — source stays private, the plugin ships as a signed bundle under its own EULA.** Open
  core over the plugin boundary. The host stays AGPL and fully functional without it; the paid
  component is a distinct product with its own terms; the CRA line stays clean (§7). ADR 0014's
  trust tiers already give first-party signing, and capability grants already give the user a
  legible consent surface for a component whose source they cannot read.
- **Does not work — publish the source under an OSI licence and sell licence keys or signed
  builds.** Anyone may rebuild and redistribute, and removing the key check is their right.
  Key-enforcement clauses are exactly what Elastic License 2.0 and the Fair Core License exist
  for, and both are non-OSI. Under AGPL you can sell the *convenience* of a build (Ardour's model,
  which works) but never exclusivity.

**Consequence for the crate split.** A paid plugin is worth nothing if its logic sits in a
permissive crate, so `vitni-digitalarkivet` moves to the AGPL column. Nothing internal depends
on it but `plugins/digitalarkivet-import`, so the move is free today, whereas MIT on it would
permanently forfeit charging for anything derived from it. Keeping the existing scraper in-repo
under AGPL preserves every option: a later *premium* importer (bulk fetch, OCR-assist,
subscription-backed sources) can be a private plugin without disturbing anything already
published. Whether the free importer eventually leaves the public repo is a later call, not part
of this decision.

One non-licensing flag for that plugin specifically: "Digitalarkivet" is the National Archives of
Norway's service name. Describing a plugin as an importer *for* Digitalarkivet is ordinary
nominative use, but shipping a paid product whose name leads with someone else's mark deserves a
look from counsel before money changes hands, and the fixtures under
`crates/vitni-digitalarkivet/tests/fixtures/` are third-party content whose redistribution
terms are a separate question from the code's licence.

## 7. Operating the money — CRA and VAT, as of 2026

**EU Cyber Resilience Act** (Regulation (EU) 2024/2847). In force since 10 December 2024;
Article 14 reporting obligations apply from **11 September 2026**; the main manufacturer
obligations from **11 December 2027**. Commission guidance C(2026) 5252 of 27 July 2026 settles
how this lands on a monetised open-source project:

- Charging a price for the software — explicitly including "charging a price for the pre-compiled
  binaries" — makes you a **manufacturer** of that product (¶51).
- A free community build of an almost identical codebase is a **different product**, not monetised
  and therefore not placed on the market, "also … where the paid version is an 'enhanced'
  commercial version … or incorporates that version into a broader product … (e.g. as in the case
  of the 'open-core' model)" (¶52). Manufacturer obligations attach to the paid artifact only.
- Monetising *other* things through the software also counts: a marketplace, paid extra servers,
  or requiring personal-data processing as a condition of use (¶54, examples 14–16). A paid plugin
  store reachable from the free app is worth thinking about here before it exists.
- An **open-source steward** must be a legal person (Art. 3(14)), so a solo natural person cannot
  be one; the unmonetised build simply falls outside the CRA. Stewards are also exempt from
  administrative fines (Art. 64(10)), and penalties on natural persons must account for economic
  situation and size (Recital 121).

The structural conclusion is the same one §6 reaches for other reasons: **keep the free build
genuinely unmonetised and put money on distinct artifacts or services.** That is the arrangement
the guidance describes most favourably, and it is also the one that keeps a hobby project from
acquiring conformity obligations by accident.

**VAT, from Norway.** Domestic MVA registration is required above NOK 50 000 turnover in a
12-month period. Norway sits outside the EU VAT area, so B2C digital sales into the EU require a
**non-Union OSS** registration in an EU member state (quarterly return, destination rates,
10-year records); EU B2B is reverse charge. For a one-person operation at plausible volumes, a
**merchant of record** (Polar, Lemon Squeezy) converts all of that into roughly 4–5% + $0.50 and
takes the audit risk, which is the right trade below mid-five-figure monthly revenue. Ardour's
experience is worth remembering when pricing: their revenue arrives as payments "less than US$10,
and most of them around US$1", and payment-processor micro-fees dominated the economics until they
found a schedule that fit.

## 8. Recommendation

**Split the workspace by crate: permissive where it is commodity interop, AGPL-3.0-or-later where
it is the product, an explicit WIT plugin exception, sole copyright preserved by a CLA, commercial
exceptions sold on request, and paid plugins kept possible by construction.**

| Side | Crates | Licence |
| --- | --- | --- |
| Commodity interop — the goodwill generators, nothing that would ever be charged for | `vitni-interchange`, `vitni-gedcom`, `vitni-gramps-xml`, `vitni-i18n`; `plugins/plugin-api`, `gedcom-import`, `gedcom-export`, `gramps-import`, `gramps-export`, `ui-panel`, `_fixture` | `MIT OR Apache-2.0` (unchanged) |
| The application | `vitni-core`, `vitni-db`, `vitni-app`, `vitni-plugin-host`, `vitni-ui`, `vitni-ui-dioxus`, `vitni-cli`, `xtask` | `AGPL-3.0-or-later` + the §7 plugin exception |
| Chargeable-later feature code | `vitni-digitalarkivet`, `plugins/digitalarkivet-import` | `AGPL-3.0-or-later` |

The split is structurally sound, and `cargo metadata --no-deps` is what proves it: the permissive
crates have no internal dependency on `vitni-core` (`vitni-gedcom` and
`vitni-gramps-xml` depend only on `vitni-interchange`; `vitni-i18n` on nothing
internal), and no plugin depends on anything in the AGPL column except `digitalarkivet-import` on
the crate that moves with it. `MIT OR Apache-2.0` on the leaves deliberately keeps the MIT option
so that even a GPLv2-only consumer can reuse them — Gramps is GPLv2-**or later**, so either arm
works there, and webtrees at GPL-3.0 can take Apache-2.0.

**Scored against the requirement as worded** (§Question), honestly:

| Requirement | This recommendation | FSL-1.1-Apache-2.0 |
| --- | --- | --- |
| Anyone may use the app | ✅ | ✅ |
| No money from reselling binaries | ❌ GPL §4 permits charging for copies; trademark is the only lever (§4) | ✅ Competing Use |
| No paid SaaS | ⚠️ partial — forces publication of *modifications*, but verbatim hosting owes nothing (§4) | ⚠️ for two years per release, then Apache-2.0 (§3.3) |
| Paid plugins possible later | ✅ (§6) | ✅ |

So neither option delivers all four permanently, and the choice is which partial failure to accept.
The argument for accepting this one rests on a threat model rather than on the OSI constraint:

- **The realistic free-rider is an embedder, not a reseller.** The plausible harm is an existing
  genealogy vendor taking `vitni-core` — the event-sourced, provenance-by-construction model
  that is the entire differentiator — into a closed product. AGPL blocks that permanently and routes
  them to the commercial-exception desk. FSL blocks it for two years.
- **The two scenarios AGPL fails to block are the two least likely to pay anyone.** A market for
  "buy this genealogy program" barely exists when free builds are published beside it, which is
  also why §3.1's paid-download line is donation-shaped rather than sales-shaped. A paid host of a
  verbatim copy competes with the upstream author on operations alone, from a code base they cannot
  privately improve.
- **Obscurity is the bigger risk than free-riding.** Distro repositories are a real discovery
  channel in this domain, and the open-source signal reads as ordinary to an audience whose
  neighbours are Gramps, webtrees and Gramps Web.
- **The exclusive revenue line does not need the licence.** Paid plugins (§6) work identically
  under AGPL, FSL, or even MIT, because the paid part is a separate work.

Against **§3.1 permissive everywhere**: it hands `vitni-core` to a closed competitor for free
and irreversibly, in exchange for adoption obtainable anyway under AGPL.

Against **§3.3 FSL**: it is the licence that matches the requirement as worded, and it keeps the
Apple stores open. It costs the distro channel and the open-source signal, and its protection of
the SaaS case lapses after two years per release (§3.3) — so on this product it trades a permanent
partial defence for a temporary fuller one. If the OSI constraint is ever revisited, §3.6 notes
that FSL → open is the cheap direction and this recommendation is the expensive one to reverse.
- Against **§3.4 noncommercial**: charges the wrong people, is definitionally fuzzy, and closes
  every distribution channel.
- Against **pure open core on a permissive base**: workable and CLA-free, but leaves the core
  itself unprotected; combining open core *with* AGPL (as recommended) costs nothing extra and
  protects both layers.

Revenue lines this preserves, in descending order of realism: paid convenience builds and store
presence (§3.1's evidence, and the licence permits it); a hosted sync/web subscription (§4 is
honest about what AGPL does and does not fence off); paid first-party plugins (§6 — the only
genuinely exclusive line); commercial exceptions (§3.2 — option value, not a forecast).

What this recommendation explicitly does **not** do: relicense anything already published
(impossible for granted versions, and the Elastic/Redis/HashiCorp record shows the trust cost of
trying); bolt a Commons Clause or a bespoke non-compete onto an OSI licence (that produces a
non-OSI licence with none of FSL's careful drafting); or accept outside contributions without a
contributor agreement while claiming a commercial-exception business exists.

## 9. Follow-ups

Filed as backlog bullets under *Docs & repo tooling* in [`issues.md`](../issues.md); none of it is
done in this change.

1. **ADR 0034** — record the split, the AGPLv3 §7 plugin exception, the CLA decision, and the fact
   that the MPL crates carry Exhibit A only. 0016 and 0031 are unwritten; the accepted set runs to
   0033.
2. **Licence files** — add `LICENSE-AGPL`; add the missing `LICENSE-MIT` and `LICENSE-APACHE`
   (absent today despite the manifest declaration); add `NOTICE`. Set per-crate
   `license = "AGPL-3.0-or-later"` where §8 says so instead of `license.workspace = true`, and put
   the §7 exception in each AGPL crate's `lib.rs` module header alongside the existing description.
3. **`deny.toml`** — do *not* widen `licenses.allow`; add per-crate `[[licenses.exceptions]]`
   entries for the AGPL workspace crates, so that an AGPL *dependency* still fails the check.
4. **`CONTRIBUTING.md` + CLA** — DCO plus a broad licence grant, drafted against
   åndsverkloven §67(2) (name sublicensing and relicensing explicitly) and §5 (promise
   attribution, do not purport to acquire a waiver).
5. **`COMMERCIAL.md`** — one page: what an exception covers, what it does not (support, trademark,
   sublicensing), and how to ask.
6. **A distinctive product name**, before the repository is public — the trademark half of §4's
   protection, which "genealogy" cannot supply.
7. **Update the docs that assert the current position** — `README.md`'s License section and
   `CLAUDE.md`'s "the workspace is `MIT OR Apache-2.0` (permissive). Keep it that way." The
   clean-room rule about not copying Gramps (GPLv2+) source stays exactly as it is; it is about
   provenance, not about which licence this project ships under.
8. **Paid-plugin distribution shape**, before the first one exists: private repo, its own EULA,
   which ADR 0014 trust tier a paid first-party bundle receives, and how a purchase becomes a
   downloadable signed bundle through a merchant of record.
9. **CRA calendar** — 11 September 2026 reporting duties arrive before 11 December 2027's main
   obligations. Nothing to do while nothing is monetised; revisit the moment something is.

## References

- `cargo deny --all-features list` over the current `Cargo.lock`; `cargo metadata --no-deps` for
  the internal dependency directions; `gh repo view magne/vitni`.
- MPL-2.0 Exhibit A/B, checked in the vendored sources of `cssparser`, `selectors`, `dtoa-short`,
  `option-ext`.
- FSL-1.1-MIT / FSL-1.1-ALv2 templates and FAQ, `fsl.software`; Fair Source definition, `fair.io`;
  PolyForm Noncommercial 1.0.0 and Small Business 1.0.0, `polyformproject.org`, plus the drafters'
  own discussion of "noncommercial" in `polyformproject/polyform-licenses#58`.
- Regulation (EU) 2024/2847 recitals 15, 17–20 and Articles 3(13)–(14), 24, 64; Commission
  guidance C(2026) 5252 of 27 July 2026 ¶¶42, 47, 51–54, 69–74; the ORC WG CRA FAQ on
  manufacturer vs steward; OpenSSF, "OSS and the CRA: am I a Manufacturer or a Steward?".
- Åndsverkloven (LOV-2018-06-15-40) §5 and §67; Prop. 104 L (2016–2017) and Innst. 258 L
  (2017–2018) on the codification of the *spesialitetsprinsippet*.
- FSF, "GPL Enforcement in Apple's App Store" (2010) and the follow-up; VLC's App Store removal,
  January 2011.
- Flathub app-author requirements and MetaInfo guidelines (`LicenseRef-proprietary`, `extra-data`
  sources, "Flathub for Proprietary Software" on the Flathub Discourse) — the basis for §3.3's
  packaging split; Debian DFSG and Fedora licensing policy for the repositories that are lost.
- OSD #1 (Free Redistribution) and #6 (No Discrimination Against Fields of Endeavour); GPLv3 §4
  (charging for copies), §10 (no further restrictions), AGPLv3 §7 (additional permissions) and §13
  (remote network interaction).
- Krita: licence page (GPL + trademark + GPL plugin API) and the 2021 store-pricing post;
  Ardour FAQ (download payments, subscriptions, no licence keys); Gramps project licence page
  (GPLv2-or-later, AGPL-3 for Web API and Gramps.js); webtrees (GPL-3.0).
- RedMonk, "The State of Open Source Licensing in 2026"; Sentry, "Sentry is now Fair Source"
  (2024); the 2021–2026 relicensing record (Elastic, HashiCorp/OpenTofu, Redis/Valkey, Elastic's
  2024 return to AGPL, NocoDB's January 2026 move to a Sustainable Use Licence).
- ADR 0007 §9, ADR 0011 §2/§6, ADR 0013, ADR 0014 §6/§7; `docs/release.md`;
  [`plugin-signing-and-trust.md`](plugin-signing-and-trust.md).
