# Genealogy data model

- **Status:** Draft
- **Date:** 2026-06-17
- **Audience:** anyone implementing `genealogy-core` (aggregates, events, projections)

This document defines the domain data model for the genealogy workspace: the entities and value
objects we store, the weaknesses we inherited from our reference model and what we decided to do
about them, and the mapping of all of it onto the event-sourcing architecture committed in
[ADR 0001](adr/0001-use-event-sourcing-for-the-domain-core.md) and
[ADR 0002](adr/0002-cqrs-es-framework-and-per-workspace-database.md).

It is reference plus design. No code is written here; the illustrative Rust in §15 is a sketch,
not the final API. Class diagrams generated from the implemented aggregates live in
[data-model-diagram.md](data-model-diagram.md); a design review of the implemented model (with
recommended follow-up ADRs) lives in [data-model-review.md](data-model-review.md). The cross-cutting
event-sourcing *mechanics* — where the provenance envelope physically lives, how an assertion is
identified, the determinism boundary, and the event encoding/versioning convention — are decided in
[ADR 0004](adr/0004-event-sourcing-implementation-contract.md); this document holds the domain
vocabulary those mechanics operate on. Concrete projection schemas and event-version upcasting
are deferred to follow-up ADRs (ADR 0002, *Out of scope*).

## 1. Purpose and scope

We build on the [Gramps](https://github.com/gramps-project/gramps) v6 data model as our **entity
reference** — the vocabulary of *what entities exist* (people, families, events, places, sources,
citations, and so on). We do **not** adopt Gramps' persistence approach: Gramps mutates records in
place and keeps no history, whereas our domain core derives state by replaying an append-only event
log (ADR 0001).

The deliverable of this document is the conceptual model, the issues analysis, and the event-source
mapping. In scope:

- the entities (conclusion-layer projections) and the value objects they are built from;
- the known weaknesses in the Gramps model and our decisions about each;
- lessons folded in from GEDCOM 7, GEDCOM X, the GENTECH GDM, and several shipping products;
- the aggregates, the per-aggregate command/event catalog (§10), the per-event provenance
  envelope, and the per-aggregate domain-error taxonomy (§10.1);
- how external search sites/APIs and imports fit (§11), how DNA is modelled as evidence (§12),
  why AI does not change the model (§13), and how internationalization is handled (§14).

Out of scope (deferred to later ADRs): concrete read-model/projection SQL, event-version upcasting
tooling, the GEDCOM import/export round-trip strategy, and configurable surety schemes.

## 2. Standards surveyed

### 2.1 Gramps 6.0

Gramps has **ten primary (first-class) objects**: `Person`, `Family`, `Event`, `Place`, `Source`,
`Citation`, `Repository`, `Media`, `Note`, `Tag`. Everything else is a secondary/value object
embedded in a primary.

Relevant 6.0 facts:

- Serialization changed from pickled binary BLOBs to **human-readable JSON** (database schema
  version 21); the store is directly queryable as SQL/JSON without a Python round-trip.
- **SQLite is the default backend** (BSDDB is now legacy; Postgres/MongoDB are experimental addons).
- Objects link by an internal `handle` (string primary key) plus a user-facing `gramps_id`.
- `Place` has a **dated, hierarchical** model: `PlaceRef` (enclosed-by) plus `PlaceName` entries
  that carry a date and language; alternate names are supported.
- The **Source/Citation split** (since Gramps 3.4): `Source` is the work; `Citation` is a specific
  reference into it (page, confidence, date), pointing at one `Source`.
- A `private` flag, a `change` timestamp, and a `tag_list` exist on the primary objects.

Reference: <https://gramps-project.org/wiki/index.php/Database_Formats>,
<https://gramps-project.org/wiki/index.php/SQL_Schema>,
<https://github.com/gramps-project/gramps/blob/master/gramps/gen/lib/date.py>.

### 2.2 GEDCOM 7.0 (2021)

The first major revision since 5.5.1. Records: `INDI`, `FAM`, `SOUR`, `REPO`, `OBJE`, `SNOTE`
(shared note), `SUBM`. Lessons relevant to us:

- **Date model** is three types: `DateValue` (a date, `DatePeriod`, `dateRange`, or `dateApprox`),
  `DateExact`, and `DatePeriod`. Ranges use `BET x AND y`; open-ended `BEF`/`AFT`; approximations
  `ABT`/`CAL`/`EST`; periods `FROM`/`TO`. Calendars include `GREGORIAN`, `JULIAN`, `HEBREW`,
  `FRENCH_R`, with `BCE`/epoch. Every date can now carry a free-text **phrase** and a **time**.
- Confusing **dual-year** semantics were dropped in favour of date phrases.
- `ASSO` (associations) gained an **enumerated `ROLE`** (replacing free-text `RELA`):
  `CLERGY`, `FRIEND`, `GODP`, `NGHBR`, `OFFICIATOR`, `PARENT`, `WITN`, … — and `ASSO` is **now
  allowed on events**, i.e. witnesses/officiators are first-class event participants.
- `UID` (stable record id) and `SDATE` (sort date) are now standard.

Reference: <https://gedcom.io/specifications/FamilySearchGEDCOMv7.html>,
<https://github.com/FamilySearch/GEDCOM/blob/main/changelog.md>.

### 2.3 GEDCOM X (FamilySearch conceptual model)

GEDCOM X separates **evidence** from **conclusions**:

- `Conclusion` is the base type (`confidence`, `attribution`, `source`, `note`).
- `Subject` extends `Conclusion` and adds identity, media, and an `evidence` list of references.
  `Person` and `Relationship` are Subjects; `Fact`, `Name`, `Gender` are Conclusions.
- A **persona** is a `Person` extracted from a *single source*; a conclusion `Person` is synthesised
  by pointing its `evidence` references at the personas.
- `Attribution` records **who** (contributor/creator `Agent`), **when** (created/modified), and
  **why** (change message) — provenance as a first-class object.
- `SourceDescription`, `SourceReference`, `EvidenceReference`, and `Document` (type `Analysis`, for
  proof arguments) complete the model. `ConfidenceLevel` is an enumerated scale.

Reference: <https://github.com/FamilySearch/gedcomx> (`specifications/`),
<http://gedcomx.org/v1/Conclusion.html>.

### 2.4 GENTECH Genealogical Data Model (GDM 1.1)

The GDM makes the **ASSERTION** the atomic unit. An assertion links a `Source` and a `Researcher`
to a subject (or two subjects), carrying a `Value`, a `Rationale`, a `Surety` (from a
project-defined **surety scheme**), and a `Disproved` flag. Assertions can be built on other
assertions, forming an auditable chain of deductions. Crucially, surety is an attribute *of the
assertion*, not of the source — the same source can support claims of different reliability. A
disproved conclusion is **marked, not erased**, so future researchers see the dead end.

Reference: <https://genealogy.sourceforge.net/GENTECH_Primer.html> (primer; the GDM 1.1 diagram is
published by NGS).

## 3. Successful products surveyed, and the lessons taken

- **The Master Genealogist (TMG).** Per-participant **Roles** are first-class (Bride, Groom,
  Minister, Heir, Executor, Witness), and each role drives narrative output. TMG records
  **conflicting evidence** with a per-evidence **surety**. Its GenBridge importer exists precisely
  because GEDCOM export *loses source structure*. **Lessons:** roles are per-participant and
  first-class; surety is per-claim; a rich native model beats treating GEDCOM as the native shape.
- **Big Three (Family Tree Maker, Legacy Family Tree, RootsMagic).** All three blur the
  source-vs-citation-detail distinction (RootsMagic is clearest). Best practice is a citation that
  is **reusable and attached per fact** — Legacy's per-detail sourcing beats RootsMagic's
  per-person sourcing for later evaluation. All expose **GPS source-quality** analysis along
  *Evidence Explained*'s three axes (original/derivative source, primary/secondary information,
  direct/indirect/negative evidence). Family Tree Maker recognises only present-day places (no
  dated jurisdictions) — a flagged weakness. **Lessons:** model evidence quality as several
  dimensions, not a single confidence enum; attach citations per assertion; date place
  jurisdictions.
- **FamilySearch Family Tree / GEDCOM X.** A single collaborative *conclusion* tree with exactly
  **two relationship types — couple and child-and-parents** (no `Family` object). Merge is central,
  **non-destructive, carries a reason, and retains contributors**; conflicts are surfaced for humans
  to resolve, not auto-merged. **Lessons:** neutral partner roles and child-and-parents-as-a-
  relationship are validated by a system at planetary scale; merge-with-provenance is exactly what
  event sourcing gives for free.
- **MyHeritage.** SmartMatch (tree↔tree, fuzzy on names/dates/relatives), Record Match
  (tree↔historical record), and the **Theory of Family Relativity** — a big-data graph that computes
  relationship paths between DNA matches across trees and records and attaches a **confidence
  score**; plus AutoClusters. Family Tree Builder is GEDCOM-based and syncs to the online tree.
  Confirming a match "saves new facts, citations, and relatives into your tree". **Lesson:** a
  planet-scale system already treats every match/theory as a *machine-generated, confidence-scored
  suggestion the human confirms or rejects* — an assertion by a non-human agent with provenance.
  This is exactly the evidence-layer assertion our event log already models (see §11, §13).
- **webtrees** is GEDCOM-native and inherits GEDCOM's ceiling. **Geni / Ancestry** contrast a
  single merged tree against piles of duplicate uploaded trees. **Lesson:** a GEDCOM-shaped core
  inherits GEDCOM's limits, and collaboration demands first-class merge plus provenance.

## 4. The core decision: evidence/conclusion, not conclusion-only

Gramps — like most desktop software — is **conclusion-only**: it stores the researcher's
synthesised answer ("born 1847 in Bergen") and bolts sources on as citations. It cannot natively
represent *"the 1865 census says 1846, the parish register says 1847, and I conclude 1847 because
the register is the original"*. The evidence is flattened into the conclusion.

The **evidence/conclusion (assertion) model** — GENTECH's assertion and GEDCOM X's
persona/conclusion/attribution split — treats every claim as an assertion that links evidence to a
conclusion with a confidence and a provenance record.

That maps **directly** onto our event log. Per ADR 0001 every event already carries an **event
context** recording operator, time, and rationale. Therefore:

- **The event log is the assertion / evidence layer.** Each event is a claim made by an operator,
  optionally backed by citations, carrying a surety and an evidence analysis.
- **Projections are the conclusion layer**, shaped as the familiar Gramps ten entities — the
  current best synthesis, rebuildable from the log.
- **Corrections are non-destructive.** A wrong claim is superseded or retracted by a *new* event;
  nothing is overwritten. This is GENTECH's `disproved` expressed as events — the audit trail of
  deductions is preserved by construction.

The practical payoff: the audit trail, conflict retention, time-travel, and attributed merge that
the surveyed products bolt on (or lack) all fall out of the architecture we already chose. The cost
is that "the current truth" is a derived projection, not a row you edit — which is the whole point.

## 5. Layered architecture

```text
            commands (operator intent)
                     │
                     ▼
        ┌─────────────────────────┐
        │  decision core (pure)   │   state + command -> events | error
        │  framework-agnostic     │   (ADR 0002 portability habit)
        └─────────────────────────┘
                     │ emits
                     ▼
   ┌───────────────────────────────────────┐
   │  EVENT LOG  =  assertion / evidence   │  append-only, source of truth
   │  each event: payload + event context  │  (operator, when, why, surety)
   └───────────────────────────────────────┘
                     │ replay / fold
                     ▼
   ┌───────────────────────────────────────┐
   │  PROJECTIONS  =  conclusion layer     │  disposable, rebuildable
   │  shaped as the Gramps 10 entities     │  (Person, Family, Event, …)
   └───────────────────────────────────────┘
                     │ query
                     ▼
              read views / UI / export
```

The decision core is a pure `state + command -> events | error` function; the `cqrs-es`
`Aggregate` impl is a thin adapter over it (ADR 0002). Events are self-contained — every identifier
we might query by lives in the payload, never implicitly in a stream key.

## 6. Entity catalog (conclusion-layer projections)

The ten Gramps primaries, described as the shape of the conclusion projections. Each is the *current
synthesis* derived from the log; none is edited directly.

| Entity         | Purpose                                                          | Key projected fields                                                                                                                                                                                                                                                                                |
| -------------- | ---------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Person**     | An individual (conclusion or persona).                           | `id`, `human_id`, names (`PersonName` list), `sex`, facts (`Fact` list: birth/death/…), event participations, associations, citations, media, notes, tags, `external_ids`, merged personas (`PersonsMerged` links), `evidence_level` (conclusion vs persona), `restrictions` (a `Restriction` set). |
| **Family**     | A union and its children.                                        | `id`, `human_id`, partner participations (neutral roles), child list (`ChildParentRelationship` per partner per child), family-level events (marriage/divorce), citations, media, notes, tags, `external_ids`, `restrictions` (a `Restriction` set).                                                |
| **Event**      | Something that happened at a date/place, shared by participants. | `id`, `human_id`, `event_type`, `date` (`GenealogicalDate`), `place_id`, `description`, participants (a projection of the person-side `ParticipationAsserted` rows that reference this event — the Person aggregate owns participation), addresses, citations, media, notes, tags, `restrictions` (a `Restriction` set). |
| **Place**      | A location, hierarchical and dated.                              | `id`, `human_id`, `place_type`, names (`PlaceName` list, dated), enclosed-by (`PlaceRef`, dated), `coordinates`, `code`, citations, media, notes, tags, `restrictions` (a `Restriction` set).                                                                                                       |
| **Source**     | A work / document.                                               | `id`, `human_id`, `title`, `author`, `pub_info`, `abbrev`, repository links (`RepoRef` with call number + media type), attributes, media, notes, tags, `restrictions` (a `Restriction` set).                                                                                                        |
| **Citation**   | A specific reference within a Source.                            | `id`, `human_id`, `source_id`, `page`, `date`, `confidence`, `evidence_analysis`, attributes, media, notes, tags, a `created_by`/`created_at` creation stamp, `restrictions` (a `Restriction` set).                                                                                                 |
| **Repository** | A place that holds sources.                                      | `id`, `human_id`, `repository_type`, `name`, addresses, urls, notes, tags, `restrictions` (a `Restriction` set).                                                                                                                                                                                    |
| **Media**      | A digital artifact.                                              | `id`, `human_id`, `path`/web reference, `mime`, `checksum`, `date`, attributes, citations, notes, tags, `restrictions` (a `Restriction` set).                                                                                                                                                       |
| **Note**       | Free or rich text.                                               | `id`, `human_id`, `note_type`, `RichText` (Markdown + language), tags, `restrictions` (a `Restriction` set).                                                                                                                                                                                        |
| **Tag**        | A user-defined label (definition).                               | `id`, `name`, `color`, `priority`, `restrictions` (a `Restriction` set).                                                                                                                                                                                                                            |

## 7. Value-object catalog

Value objects have no independent identity; they are immutable and embedded in event payloads (and
hence in projections). Newtypes are used over bare primitives (Rust standards).

- **`HumanId`** — the user-facing identifier (the `gramps_id` analog). Distinct from the aggregate
  `id` (a UUID v7 — [ADR 0004](adr/0004-event-sourcing-implementation-contract.md) §5).
- **`AssertionId`** — a UUID v7 identifying a single assertion (one event). Carried in the event
  payload so that a correction (`AssertionRetracted` / `AssertionSuperseded`, §10) names the exact
  claim it revises, portably and queryably — never the implicit `(aggregate, sequence)` stream key.
  See ADR 0004 §2.
- **`ExternalId`** — `{ authority, value, kind, url }` — a stable identifier in an external system
  (FamilySearch FSID, MyHeritage/Geni id, a Digitalarkivet record URL, an Ancestry record). Held as
  a list on any aggregate sourced or matched externally (Person/Source/Citation/Place/Media). It is
  the GEDCOM 7 `EXID`/`UID` and GEDCOM X `identifiers`, and it is what makes re-import, sync,
  deduplication, and a provenance back-link to the origin record possible. Wired on Person and
  Family today; widening to the remaining externally-sourced aggregates (Source, Citation, Place,
  Media) is deferred (§17). See §11.
- **`Agent`** — *who* made an assertion: `{ kind, id, display }` where
  `AgentKind = Human | Software { name, version } | AiModel { name, version }`. A claim from an
  import script, a match engine (MyHeritage/FamilySearch), or an AI model is therefore attributable
  and distinguishable from a human's claim. Used by `EventContext.operator` (§8); rationale in §13.
- **`PersonName`** — `name_type`, given, a **list of `Surname`** (each with prefix, the surname
  text, a `primary` flag, and a connector), suffix, title, nickname, call name, optional `date`,
  optional `language` (`LanguageTag`), and `transliterations` — a list of alternate-script/romanised
  forms of this same name (GEDCOM 7 `NAME`.`TRAN`; e.g. a Han name plus its Pinyin). The surname
  *list* (kept from Gramps) handles patronymics and multi-part surnames better than GEDCOM's single
  field.
- **`LanguageTag`** — a BCP-47 language tag (e.g. `nb`, `en-US`, `zh-pinyin`, `sr-Cyrl`). Reused by
  `PlaceName`, `RichText`/Note, and `PersonName`. The script is a BCP-47 subtag, so no separate
  script field is needed. See §14.
- **`GenealogicalDate`** — see §7.1.
- **`Sex`** — `Male` / `Female` / `Unknown` / `Intersex` / `Other(String)`. `Intersex` is the
  first-class GEDCOM 7 `X` value (does not fit a binary male/female classification); `Other(String)`
  remains for any other recorded value.
- **`PlaceName`** — `{ value, date, language: LanguageTag }` (dated, language-tagged names per
  Gramps 6 — e.g. *Kristiania* vs *Oslo*, or German exonyms of Norwegian places).
- **`GeoCoordinates`** — `{ latitude, longitude }`.
- **`Confidence`** — the surety scale on an assertion: `VeryLow`/`Low`/`Normal`/`High`/`VeryHigh`
  (Gramps' five levels; aligns with GEDCOM `QUAY 0-3` and GEDCOM X `ConfidenceLevel`).
- **`Restriction`** — a privacy restriction on a record (GEDCOM v7 `RESN`):
  `Confidential` / `Locked` / `Privacy`. A record carries a **set** (`BTreeSet<Restriction>`); the
  empty set means unrestricted. Closed enum (no custom escape — `RESN` has exactly these three).
  Replaces the former single `private` boolean; present on every aggregate (§6). See §16.
- **`EvidenceAnalysis`** — *Evidence Explained*'s three axes, carried alongside `Confidence`:
  source = `Original`/`Derivative`, information = `Primary`/`Secondary`, evidence =
  `Direct`/`Indirect`/`Negative`.
- **`ParticipantRole`** — rich, TMG-style, per participant: `Primary`, the GEDCOM 7 `ROLE` values
  (`Witness`, `Officiator`, `Clergy`, `Father`, `Mother`, `Parent`, `Child`, `Husband`, `Wife`,
  `Spouse`, `Godparent`, `Friend`, `Neighbour`, `Multiple`), the `Bride`/`Groom` extensions, plus a
  `Custom(String)` escape.
- **`ChildParentRelationship`** — per parent: `Birth`/`Adopted`/`Foster`/`Step`/`Sealed`/`Unknown`.
  (FamilySearch models child-and-parents itself as a relationship; we keep it as a value within the
  Family child list.)
- **`Fact`** — a claimed characteristic or event-like attribute of a Person:
  `{ fact_type: FactType, date: Option<GenealogicalDate>, place_id: Option<PlaceId>, value:
  Option<String> }`. Carries the payload of `FactAsserted` (§10) and shapes the Person `facts`
  list (§6) — birth, death, occupation, residence, religion, and the like (`FactType` is one of the
  enumerated sets below). Distinct from a full `Event` aggregate: a `Fact` is a single-person
  attribute, an `Event` is shared between participants. The citations backing the claim live on the
  assertion envelope (`EventContext.citations`, §8), the sole evidence channel (ADR 0020).
- **`Attribute`** — `{ attribute_type, value }`. Its backing citations live on the assertion
  envelope (§8, ADR 0020), not on the attribute.
- **`Age`** — a participant's age at an event, as a *duration* (not a calendar point):
  `{ bound: Option<AgeBound>, years: Option<u16>, months: Option<u16>, days: Option<u16>, phrase:
  Option<String> }`. Every part is optional (a partially-recorded age is common); an all-absent age
  is normalized to `None` at the boundary. `AgeBound` is the GEDCOM `AGE` `<`/`>` qualifier
  (`LessThan`/`GreaterThan`); `phrase` carries an age that does not decompose. Weeks are normalized to
  days at import. Carried by `ParticipationAsserted` (§10, ADR 0019).
- **`Url`** — `{ url_type, href, description }`.
- **`Address`** — a postal address (GEDCOM `ADDR`): `{ lines: Vec<String>, locality, region,
  postal_code, country, phone, email, fax, www, original_text }`. `lines` is the ordered street
  lines (`ADR1`/`ADR2`/`ADR3` in 5.5.1, the multi-line `ADDR` payload in 7.0) — never collapsed to
  one. The contact subtags (`PHON`/`EMAIL`/`FAX`/`WWW`) and a verbatim `original_text` fallback (so
  an address that cannot be split into fields is never lost) complete it. Wired on `Repository`
  today; widening to other aggregates is in §17.
- **`RichText`** — `{ text, media_type, language, translations }`. `media_type` defaults to `text/markdown`
  (CommonMark); `text/plain` and `text/html` are accepted so imports round-trip losslessly. Replaces
  Gramps' offset-range `StyledText`: Markdown-as-text is more expressive, diffs cleanly, is
  human- and AI-readable, and needs no fragile span bookkeeping (YAGNI). Typed links to aggregates
  use a documented URI scheme (`x-genealogy:person/<uuid>`) — the one capability Gramps' styled
  links had. Mirrors GEDCOM 7 `NOTE`(text + `MIME` + `LANG`), and carries a `translations` list —
  the same content in other languages (GEDCOM `NOTE`.`TRAN`), parallel to
  `PersonName.transliterations`. See §14.
- **`MediaRef`** — `{ media_id, crop: Option<Rect>, caption, citations }`. This is the *use* of a
  shared `Media` aggregate at one attachment point, not the file itself. Many objects reference the
  same `Media`; each `MediaRef` adds context specific to *this* use — a crop/region (e.g. one face
  in a group photo), a caption, and citations for using it here. Same indirection as `CitationRef`;
  it keeps per-use detail off the shared file.
- **`CitationRef`** — `{ citation_id }` (citations are aggregates; this is the link).
- **Enumerated type sets** — `EventType`, `FactType`, `NameType`, `PlaceType`, `AttributeType`,
  `RepositoryType`, `SourceMediaType`, `ParticipantRole`, `AssociationRole`. Each is a closed enum
  **plus a `Custom(String)` escape hatch**, mirroring Gramps' "custom type" pattern. The standard
  GEDCOM 5.5.1/7.0 enumerated values are **first-class variants**, not `Custom` strings, so they
  round-trip, deduplicate, and localize as codes; `Custom` is reserved for genuinely non-standard
  values. `EventType` carries the civil/common GEDCOM events (birth/death/marriage, the
  baptism/christening pair, cremation, adoption, confirmation, naturalization, ordination, probate,
  retirement, will, engagement, annulment, divorce/divorce-filed, and the marriage banns/contract/
  licence/settlement set — LDS ordinances excluded); `FactType` carries the GEDCOM attributes
  (caste, physical description, education, ethnicity, national id, nationality, number of children/
  marriages, property, SSN, nobility title); `ParticipantRole` and `AssociationRole` each carry the
  full GEDCOM 7 `ROLE` set so neither degrades — they stay **separate** (event participation vs
  person↔person), disambiguated on import by GEDCOM's structural context (`ASSO`-in-event vs
  `ASSO`-on-`INDI`).
- **`EventContext`** — the provenance envelope on *every* event; see §8.

### 7.1 `GenealogicalDate` — the hard one

Dates are the most error-prone part of every genealogy model. We adopt a **structured** date (not a
string), keeping the Gramps richness and folding in GEDCOM 7's lessons:

- **Calendar** — `Gregorian`, `Julian`, `Hebrew`, `FrenchRepublican`, `Islamic`, `Swedish`
  (Gramps' set).
- **Modifier** — `None`, `Before`, `After`, `About`, `Range(a, b)`, `Span(a, b)`, `From`, `To`,
  `Interpreted { date, phrase }` (Gramps `MOD_*`; `Interpreted` is GEDCOM `INT` — a structured
  reading plus the verbatim phrase it came from). `Range`/`Span` carry two sub-dates. A date that
  cannot be parsed at all is a `TextOnly { text }` body *beside* the structured modifiers (an
  untagged `Structured`/`TextOnly` body enum in code), so it is honest about carrying no
  calendar/point structure.
- **Quality** — `Normal`, `Estimated`, `Calculated` (Gramps `QUAL_*`; note `QUAL_INTERPRETED` is
  defined-but-unused in Gramps — we omit it rather than carry a dead variant).
- **Components** — partial dates allowed: optional year/month/day; supports BCE via signed year.
- **Dual dating** — an explicit `new_year_begins` field (Gramps `newyear`) so 23 Jan 1735/6 is
  represented honestly, not mistaken for uncertainty.
- **Sort value** — a precomputed integer (`sortval`) for cheap ordering.
- **Original text** — the verbatim source string is always retained (GEDCOM 7 date *phrase*), so a
  date we cannot parse is never lost.

GEDCOM 7 also allows a **time** on any date; we include an optional `time` (`{ hour, minute,
second? }`) for exact timestamps but do not require it for genealogical dates. It is `#[serde(default)]`
so events recorded before the field existed still decode (ADR 0004 §4).

## 8. Event context (provenance, by construction)

Every event carries an `EventContext`. This is the GEDCOM X `Attribution` and the GENTECH
researcher/rationale/surety, made mandatory by the architecture rather than optional:

- `operator` — **who** caused the change: an **`Agent`** (human, software, or AI model — §7, §13),
  not merely a user id, so imported and machine-generated claims stay attributable.
- `occurred_at` — **when** (the assertion time; distinct from any subject date in the payload).
- `rationale` — **why** (free text; GENTECH `Rationale`, GEDCOM X change message).
- `confidence` — the operator's surety in *this* claim (`Confidence`).
- `citations` — zero or more `CitationRef` backing this claim (the evidence link). This is the
  **sole evidence channel** for a claim (ADR 0020): payload value objects (`Fact`, `Attribute`)
  carry no citation lists. (`MediaRef.citations` is unaffected — it is per-use context for a media
  attachment, not evidence for a claim.)
- optional `evidence_analysis` — the *Evidence Explained* axes for this claim.

Because the context lives on the event, surety and provenance are **per assertion**, fixing the
Gramps limitation where confidence lives only on the citation and quality is a single value.

The `EventContext` is carried **inside each event payload**, not in the `cqrs-es` metadata map
(which is reserved for non-domain ops/tracing — correlation/trace/request/host). Provenance is
domain data and is structured, so it belongs in the payload; see
[ADR 0004](adr/0004-event-sourcing-implementation-contract.md) §1.

## 9. Aggregates (event-sourcing boundaries)

We use `cqrs-es` fixed aggregates with per-stream `(aggregate, sequence)` optimistic concurrency
(ADR 0002). An aggregate is a thing with an independent lifecycle, identity, and within-stream
invariants. There are **twelve** — the ten Gramps primaries plus two for DNA (§12):

**Person, Family, Event, Place, Source, Citation, Repository, Media, Note, Tag, DnaTest, DnaMatch.**

Boundary notes:

- **Citation is its own aggregate**, not a value embedded in the citing object. It is reusable
  across many assertions and links to a `Source` aggregate, carrying page, date, confidence, and
  evidence analysis. This matches the Big Three best practice (reusable citation, attached per
  fact) and TMG's database-style sources.
- **Tag** is a lightweight aggregate for the *definition* (name/colour/priority). Tag *application*
  is an event on the tagged aggregate that references the tag id — tagging does not mutate the Tag.
- **Cross-aggregate links live in event payloads as ids** — event participation, family membership,
  place enclosure, citation→source. Never implicit in a stream key (ADR 0002 self-contained-events
  rule; the one thing that cannot be retrofitted).
- **Participation is owned by the Person aggregate** (ADR 0019): the `ParticipationAsserted` events
  live on Person, and an Event's participant list is a **projection** over the person-side rows that
  reference it — the Event aggregate holds no participation state. This makes retraction unambiguous
  (the person-side `AssertionId` is the only handle) and keeps person history self-contained for
  merge/persona flows and GEDCOM INDI-centric export.
- **Persona vs conclusion person.** A person extracted from a single source is a `Person` aggregate
  with `evidence_level = Persona`. When the researcher concludes two records are the same
  individual, a `PersonsMerged` event records the join (operator + rationale + confidence) and the
  conclusion person references the personas as evidence. **Both streams are retained** — the merge
  is non-destructive, exactly as FamilySearch/Geni require but as audit-by-construction.
- **`DnaMatch` is owned by neither person.** It is a pairwise observation between two `DnaTest`s
  (referenced by id, self-contained) that genealogists research over time — so it is its own
  aggregate, not a value on a Person. `DnaTest` is anchored to one Person. See §12.
- **Cross-aggregate invariants** (e.g. "an Event's `place_id` must exist") are checked against
  possibly-lagging projections, not transactionally — the accepted `cqrs-es` "aggregate tax"
  (ADR 0002). Genealogy invariants are largely within-aggregate.

## 10. Commands and events per aggregate

**Commands** are imperative operator intent (`CreatePerson`, `AssertName`); **events** are the
past-tense assertions a command produces (`PersonCreated`, `NameAsserted`). One command may emit
several events, and a command may be **rejected** with a domain error (§10.1) instead — the
decision core is `decide(state, command) -> Result<Vec<Event>, Error>`
([ADR 0004](adr/0004-event-sourcing-implementation-contract.md) §3). The application layer fills
the `EventContext` (operator `Agent`, clock, generated `AssertionId`) onto the command before
`decide` runs, keeping `decide` pure (ADR 0004 §3).

Representative **commands** (not exhaustive):

- **Person:** `CreatePerson`, `AssertName`, `AssertSex`, `AssertFact`, `AssertParticipation`,
  `AssertAssociation`, `AttachMedia`, `AttachNote`, `Tag` / `Untag`, `SetRestrictions`,
  `RetractAssertion`, `SupersedeAssertion`, `MergePersons`.
- **Family:** `CreateFamily`, `AddPartner` / `RemovePartner`, `AddChild` / `RemoveChild`,
  `LinkFamilyEvent`, `Tag`, `SetRestrictions`, plus the retract/supersede pair.
- **Event:** `CreateEvent`, `SetEventType`, `AssertDate`, `LinkPlace`, `SetDescription`,
  `AddCitation`, `AttachMedia`, `AttachNote`, `Tag`. (Participation is asserted on the Person
  aggregate via `AssertParticipation`; the event's participant list is a projection of those rows.)
- **Place:** `CreatePlace`, `SetPlaceType`, `AssertName`, `AssertEnclosedBy`, `AssertCoordinates`,
  `SetCode`, `AddCitation`, `Tag`.
- **Source / Citation / Repository / Media / Note / Tag / DnaTest / DnaMatch:** the imperative form
  of each aggregate's events below (`CreateSource`/`SetTitle`/…, `CreateCitation`/`SetPage`/…,
  `CreateDnaMatch`/`ObserveMatch`/`ConfirmMatch`/`RejectMatch`/…).

The matching **events** are **assertions** — verbs naming a claim, each self-contained, carrying its
own `AssertionId` and `EventContext`, and explicitly versioned (ADR 0004 §2, §4). Representative
verbs (not exhaustive):

- **Person:** `PersonCreated`, `NameAsserted`, `SexAsserted`, `FactAsserted` (birth, death,
  occupation, residence, …), `ParticipationAsserted` (links to an Event with a `ParticipantRole`,
  plus the participant-scoped detail a source records: an optional `Age`, participant-scoped
  `Attribute`s, and `NoteId`s — ADR 0019), `AssociationAsserted` (person↔person with an
  `AssociationRole`), `MediaAttached`, `NoteAttached`,
  `Tagged` / `Untagged`, `RestrictionsChanged`, `AssertionRetracted`, `AssertionSuperseded`,
  `PersonsMerged`.
- **Family:** `FamilyCreated`, `PartnerAdded` (neutral role) / `PartnerRemoved`, `ChildAdded`
  (with `ChildParentRelationship` per parent) / `ChildRemoved`, `FamilyEventLinked`, `Tagged`,
  `RestrictionsChanged`, retraction/supersede verbs.
- **Event:** `EventCreated`, `EventTypeSet`, `DateAsserted`, `PlaceLinked`, `DescriptionSet`,
  `CitationAdded`, `MediaAttached`, `NoteAttached`, `Tagged`. (Participants come from the person-side
  `ParticipationAsserted` rows — the Event aggregate holds no participation events.)
- **Place:** `PlaceCreated`, `PlaceTypeSet`, `NameAsserted` (dated, language), `EnclosedByAsserted`
  (dated `PlaceRef`), `CoordinatesAsserted`, `CodeSet`, `CitationAdded`, `Tagged`.
- **Source:** `SourceCreated`, bibliographic setters (`TitleSet`, `AuthorSet`, `PubInfoSet`,
  `AbbrevSet`), `RepositoryLinked` (call number + media type), `AttributeAdded`, `MediaAttached`,
  `NoteAttached`, `Tagged`.
- **Citation:** `CitationCreated` (→ source), `PageSet`, `DateAsserted`, `ConfidenceSet`,
  `EvidenceAnalysisSet`, `AttributeAdded`, `MediaAttached`, `NoteAttached`, `Tagged`.
- **Repository:** `RepositoryCreated`, `RepositoryTypeSet`, `NameSet`, `AddressAdded`, `UrlAdded`,
  `NoteAttached`, `Tagged`.
- **Media:** `MediaCreated`, `PathSet` / web reference, `ChecksumSet`, `DateAsserted`,
  `AttributeAdded`, `CitationAdded`, `NoteAttached`, `Tagged`.
- **Note:** `NoteCreated`, `NoteTypeSet`, `RichTextSet`, `Tagged`.
- **Tag:** `TagCreated`, `TagRenamed`, `TagColorSet`, `TagPrioritySet`.
- **DnaTest:** `DnaTestCreated` (→ person), `ProviderSet`, `KitIdSet`, `TestTypeSet`,
  `GenomeBuildSet`, `HaplogroupAsserted`, `NoteAttached`, `Tagged`.
- **DnaMatch:** `DnaMatchObserved` (→ two `DnaTest`s, shared cM/percent/segments/predicted
  relationship), `SegmentAdded`, `SharedAncestorAsserted`, `MatchConfirmed` / `MatchRejected`,
  `NoteAttached`, `Tagged`. The relationship *inference* drawn from a match is a separate
  `FactAsserted`/`AssociationAsserted` on Person/Family that cites the `DnaMatch` (§12).

`SetRestrictions` / `RestrictionsChanged` (the privacy `Restriction` set, §7) are **universal** —
present on every aggregate above, last-writer-wins (the events lists show them only on Person/Family
for brevity).

`AssertionRetracted` / `AssertionSuperseded` are the universal non-destructive correction verbs
(GENTECH `disproved` as events): they reference the prior claim **by its `AssertionId`** (§7,
ADR 0004 §2) and never delete it.

### 10.1 Domain error taxonomy

The rejection half of `decide -> Result<Vec<Event>, Error>`. Each aggregate has a `thiserror` error
enum (its `cqrs-es` `Aggregate::Error`); these are domain rejections, not infrastructure failures.
Representative variants (not exhaustive):

- **Shared (all aggregates):** `NotFound` (command targets a non-existent aggregate),
  `AlreadyExists` (re-create), `RetractsMissingAssertion` / `SupersedesMissingAssertion` (the
  referenced `AssertionId` is unknown or already retracted), `InvalidDate` (a `GenealogicalDate`
  that cannot be ordered or is internally inconsistent), `EmptyRequiredField`.
- **Person:** `EmptyName` (a `PersonName` with neither given nor surname), `MergeConflict` (the two
  persons cannot be merged — e.g. contradicting irreversible facts), `SelfAssociation`.
- **Family:** `DuplicatePartner`, `DuplicateChild`, `ChildIsOwnAncestor` (cycle in the
  child/partner graph).
- **Event:** `UnknownPlace` (a `LinkPlace` to a place id the projection does not know — the §9
  aggregate-tax check).
- **Citation:** `UnknownSource` (citation created against a missing `Source`).
- **DnaMatch:** `SameTestBothSides` (a match between a test and itself), `NegativeSharedCm`.

Cross-aggregate checks (`UnknownPlace`, `UnknownSource`) are validated against possibly-lagging
projections, the accepted `cqrs-es` aggregate tax (§9); within-aggregate checks are strongly
consistent.

## 11. External sources, APIs, and import

Several services expose genealogical data, some with APIs: FamilySearch (a GEDCOM X REST API),
MyHeritage, Geni, Ancestry (largely GEDCOM export plus record hints), and Digitalarkivet (the
Norwegian National Archives — the target of `genealogy-import`). They affect the model in three
small, consistent ways — none of which disturbs the layering, because the model was already built
around evidence and provenance.

1. **Import populates the evidence layer, not conclusions.** An external record (a census page, a
   parish-register entry, a FamilySearch *record persona*) becomes a `Source` + `Citation` + one or
   more **personas** (`Person` aggregates with `evidence_level = Persona`). Conclusions are then
   formed by assertion events — by a human, or by an accepted machine suggestion — exactly the
   persona→conclusion path in §9. FamilySearch's own model works this way: a record yields a
   `SourceDescription` (collection) plus a digital artifact plus extracted personas.

2. **External identifiers are first-class (`ExternalId`, §7).** Every aggregate that came from, or
   was matched to, an external system carries its `ExternalId`s (FSID, MyHeritage/Geni id,
   Digitalarkivet record URL). This is what makes **re-import idempotent**, enables **sync** and
   **deduplication**, and keeps a **provenance back-link** to the origin record. It maps to GEDCOM 7
   `EXID`/`UID` and GEDCOM X `identifiers`. Cross-system identity is recorded **non-destructively**:
   like FamilySearch's *Tree Person Reference* (a directional "same individual" link that does *not*
   merge data), we record a same-as assertion / `ExternalId` link rather than forcing a merge.

3. **Matches and hints are suggested assertions by a software `Agent`.** A SmartMatch, a Record
   Match, a FamilySearch person match, or a Theory of Family Relativity is a *claim made by an
   engine*, with a confidence. It enters as a low-confidence assertion in the evidence layer
   attributed to a `Software` agent (§7, §8); the user's **confirm** or **reject** is itself an
   audited event. Nothing is silently merged into the conclusion layer.

The upshot: external APIs add the `ExternalId` value object and exercise the `Agent` generalisation,
but the evidence/conclusion architecture absorbs imports and machine matches without new structure.

References: <https://developers.familysearch.org/main/docs/the-family-tree-data-model>,
<https://developers.familysearch.org/main/docs/tree-to-tree-linking>,
<https://www.myheritage.com/wiki/Theory_of_Family_Relativity>.

## 12. DNA as evidence

Genetic genealogy is now central, and it fits the evidence/conclusion model cleanly: a DNA match is
*observed data* (high data-surety), while the *relationship it implies* is an uncertain inference
(a given shared-cM total is consistent with many relationships — cf. the Shared cM Project and
WATO). We therefore model the observation and the inference separately.

Gramps had **no** native DNA model (a workaround using Associations + a Note of segment text +
a per-provider Citation); the in-progress native model (PR #2295) adds first-class `DNATest` and
`DNAMatch` objects. We adopt that shape as **two aggregates** (§9):

- **`DnaTest`** (aggregate, anchored to one Person) — `{ person_id, provider, kit_id, account,
  test_type, genome_build, haplogroups, external_ids }`, where `DnaTestType =
  Autosomal | YDna | MtDna | XDna` and `DnaGenomeBuild = GRCh37 | GRCh38`. It anchors many matches
  and carries the raw result metadata.
- **`DnaMatch`** (aggregate, owned by neither person) — a pairwise observation between two
  `DnaTest`s: `{ test_a, test_b, provider, shared_cm, percent_shared, segment_count,
  largest_segment_cm, predicted_relationship, segments: Vec<DnaSegment>,
  shared_ancestors: Vec<SharedAncestor>, status }` (`status` is the folded
  `MatchConfirmed`/`MatchRejected` outcome). Because providers use different thresholds and
  builds, the provider/build live **on the match**, not globally. Centimorgan and percent values
  are fixed-decimal integer newtypes (`Centimorgans`, `PercentShared`), not floats, so
  observations compare exactly and round-trip losslessly.

Value objects: **`DnaSegment`** `{ chromosome, start, end, centimorgans, snps, side }` (side =
`Maternal | Paternal | Unknown`) and **`SharedAncestor`** (a reference to the inferred common
ancestor[s]).

The **relationship inference** is *not* a field on the match. It is a normal assertion event on a
Person or Family — a `FactAsserted` / `AssociationAsserted` that **cites the `DnaMatch`** via the
`EventContext.citations` link — carrying its own (lower) `Confidence`. The raw match can be
high-surety while the inferred relationship is tentative, and a revised inference is just a new
superseding event. A match supplied by an engine is attributed to a `Software`/`AiModel` agent; the
human's confirmation is an audited event, as in §11.

GEDCOM portability is weak here: GEDCOM 7 has only a *proposed* `DNA_MATCH` (discussion #464),
Family Tree Maker exports a non-standard `_DNA`, and RootsMagic 10 stores matches but does not
export them — so DNA round-trip is a flagged open question (§16).

References: <https://github.com/gramps-project/gramps/discussions/2292>,
<https://github.com/FamilySearch/GEDCOM/discussions/464>,
<https://www.grampsweb.org/user-guide/dna-matches/>.

## 13. AI and the model

AI does **not** change the data model. Its impact is fully covered by one decision already taken for
imports and match engines: the operator of an event is an **`Agent`**, and `AgentKind` includes
`AiModel { name, version }` (§7, §8).

- AI transcription, extraction, and suggestion (e.g. reading a parish register, proposing a match,
  drafting a name) are **assertions in the evidence layer**, attributed to an AI agent with a
  `Confidence` — identical in shape to a human claim or an engine match.
- They are **promoted to the conclusion layer only by a human** (or an explicit, recorded policy)
  via a confirming event. The evidence/conclusion split is precisely what makes AI safe: a machine
  claim can never silently overwrite a conclusion; it sits as a reviewable, retractable assertion.
- Recording the **model name and version** in the `Agent` gives reproducibility and audit — you can
  always see which model asserted what, and when.

So the only model-level requirement AI imposes is the `Agent`/`AgentKind` generalisation, which we
were already making. No AI-specific entity, event, or field is needed.

## 14. Internationalization and localization

Genealogy is inherently multilingual — sources, place names, and personal names span languages and
scripts. Most of i18n is **presentation**, which the data model does not touch; a few concerns are
genuinely **data**, and those are already covered by small value objects.

**Presentation — no model impact (UI and per-workspace/user preferences):**

- Display/UI language, date and number formatting, and right-to-left layout.
- **Localized labels for coded enums.** `EventType`, `NameType`, `PlaceType`, etc. are stored as
  language-neutral codes (with a `Custom(String)` escape); their human labels are translated in the
  UI, never stored. The retained `original_text` on `GenealogicalDate` and the coded calendars keep
  dates language-neutral at the core too.
- **Name collation / sort order** is locale-sensitive (Norwegian *å* sorts after *z*; surname-prefix
  rules differ). Sort keys are **derived in projections** for the active locale, not stored on the
  conclusion as truth.

**Data — the model touchpoints (all via `LanguageTag`, §7):**

- `RichText` / Note carry a `language`, and a note may also hold **translations** (the same content
  in another language) — GEDCOM 7 `NOTE`.`LANG` + `TRAN`.
- `PlaceName` is already dated **and** language-tagged.
- `PersonName` carries an optional `language` and a list of **transliterations** (alternate
  scripts/romanisations of the same name) — GEDCOM 7 `NAME`.`TRAN`.
- Free-text fields and `Custom(String)` enum values are in the author's language; they may be
  language-tagged where it matters but are not required to be.

Net: i18n adds the `LanguageTag` value object and the name/note language+translation fields; the
heavy lifting (formatting, collation, RTL, label catalogs) is UI and workspace configuration, out
of the domain model. Deferred items are in §17.

## 15. Illustrative Rust sketches

Illustrative only — not the final API. They honour the workspace lints (newtypes over primitives,
enums over boolean flags, no panics, `thiserror` for domain errors).

```rust
/// User-facing identifier (the gramps_id analog); the aggregate id is a separate UUID v7.
pub struct HumanId(String);

/// Identity of a single assertion (one event), carried in the payload so corrections
/// can target it portably (ADR 0004 §2). UUID v7 — time-sortable.
pub struct AssertionId(Uuid);

/// A claimed single-person characteristic; payload of `FactAsserted`.
/// Backing citations live on the assertion envelope (`EventContext.citations`), not here (ADR 0020).
pub struct Fact {
    pub fact_type: FactType,
    pub date: Option<GenealogicalDate>,
    pub place_id: Option<PlaceId>,
    pub value: Option<String>,
}

/// Operator's surety in a single assertion (Gramps' five levels).
pub enum Confidence {
    VeryLow,
    Low,
    Normal,
    High,
    VeryHigh,
}

/// A privacy restriction on a record (GEDCOM v7 `RESN`). A record carries a `BTreeSet` of these;
/// the empty set means unrestricted. Replaces the former `private: bool`.
pub enum Restriction {
    Confidential,
    Locked,
    Privacy,
}

/// Evidence Explained's three analysis axes, carried alongside `Confidence`.
pub struct EvidenceAnalysis {
    pub source: SourceQuality,       // Original | Derivative
    pub information: InformationKind, // Primary | Secondary
    pub evidence: EvidenceKind,       // Direct | Indirect | Negative
}

/// Who made an assertion — human, software, or AI model (§13).
pub enum AgentKind {
    Human,
    Software { name: String, version: String },
    AiModel { name: String, version: String },
}

pub struct Agent {
    pub kind: AgentKind,
    pub id: AgentId,
    pub display: Option<String>,
}

/// A stable identifier in an external system (FamilySearch, MyHeritage, Digitalarkivet, …).
pub struct ExternalId {
    pub authority: String,
    pub value: String,
    pub kind: Option<String>,
    pub url: Option<String>,
}

/// Provenance envelope on every event: who / when / why / how sure / on what evidence.
pub struct EventContext {
    pub operator: Agent,
    pub occurred_at: Timestamp,
    pub rationale: Option<String>,
    pub confidence: Confidence,
    pub citations: Vec<CitationRef>,
    pub evidence_analysis: Option<EvidenceAnalysis>,
}

pub enum Calendar {
    Gregorian,
    Julian,
    Hebrew,
    FrenchRepublican,
    Islamic,
    Swedish,
}

pub enum DateQuality {
    Normal,
    Estimated,
    Calculated,
}

/// A single (possibly partial) point on a calendar; year may be negative for BCE.
pub struct DatePoint {
    pub year: Option<i32>,
    pub month: Option<u8>,
    pub day: Option<u8>,
}

pub enum DateModifier {
    None(DatePoint),
    Before(DatePoint),
    After(DatePoint),
    About(DatePoint),
    Range { start: DatePoint, end: DatePoint },
    Span { start: DatePoint, end: DatePoint },
    From(DatePoint),
    To(DatePoint),
    /// GEDCOM `INT`: a structured reading plus the verbatim phrase it was interpreted from.
    Interpreted { date: DatePoint, phrase: String },
    TextOnly(String),
}

pub struct GenealogicalDate {
    pub calendar: Calendar,
    pub quality: DateQuality,
    pub modifier: DateModifier,
    /// Optional time of day on an exact date (GEDCOM 7 `TIME`).
    pub time: Option<TimeOfDay>,
    /// Month in which the year begins, for dual/old-style dating (e.g. 1735/6).
    pub new_year_begins: Option<u8>,
    /// Precomputed ordering key.
    pub sort_value: i64,
    /// Verbatim source text, always retained even when unparseable.
    pub original_text: Option<String>,
}

pub struct Surname {
    pub prefix: Option<String>,
    pub surname: String,
    pub primary: bool,
    pub connector: Option<String>,
}

/// A BCP-47 language tag; the script is a subtag (e.g. "zh-pinyin", "sr-Cyrl").
pub struct LanguageTag(String);

/// Note / free-text content: Markdown by default, language-tagged.
pub struct RichText {
    pub text: String,
    pub media_type: MediaType, // default text/markdown; text/plain, text/html allowed
    pub language: Option<LanguageTag>,
}

pub struct PersonName {
    pub name_type: NameType,
    pub given: Option<String>,
    pub surnames: Vec<Surname>,
    pub suffix: Option<String>,
    pub title: Option<String>,
    pub nickname: Option<String>,
    pub call_name: Option<String>,
    pub date: Option<GenealogicalDate>,
    pub language: Option<LanguageTag>,
    /// Alternate-script / romanised forms of this same name (GEDCOM 7 NAME.TRAN).
    pub transliterations: Vec<PersonName>,
}

/// One aggregate's events. Each variant is a self-contained, versioned assertion;
/// each carries its own `AssertionId` and an embedded `EventContext` in the payload
/// (ADR 0004 §1–§2) — not in cqrs-es metadata.
pub enum PersonEvent {
    PersonCreated { person_id: PersonId, human_id: HumanId, evidence_level: EvidenceLevel },
    NameAsserted { person_id: PersonId, name: PersonName },
    SexAsserted { person_id: PersonId, sex: Sex },
    FactAsserted { person_id: PersonId, fact: Fact },
    ParticipationAsserted { person_id: PersonId, event_id: EventId, role: ParticipantRole },
    AssociationAsserted { person_id: PersonId, other: PersonId, role: AssociationRole },
    AssertionSuperseded { person_id: PersonId, supersedes: AssertionId },
    PersonsMerged { surviving: PersonId, merged: PersonId },
    // … Tagged, MediaAttached, NoteAttached, RestrictionsChanged, AssertionRetracted
}

/// Imperative operator intent (§10). The application layer attaches the `EventContext`
/// (operator, clock, generated `AssertionId`) before `decide` runs (ADR 0004 §3).
pub enum PersonCommand {
    CreatePerson { human_id: HumanId, evidence_level: EvidenceLevel },
    AssertName { person_id: PersonId, name: PersonName },
    AssertFact { person_id: PersonId, fact: Fact },
    RetractAssertion { person_id: PersonId, target: AssertionId },
    SupersedeAssertion { person_id: PersonId, target: AssertionId, replacement: Box<PersonCommand> },
    MergePersons { surviving: PersonId, merged: PersonId },
    // … AssertSex, AssertParticipation, AssertAssociation, AttachMedia/Note, Tag/Untag, SetRestrictions
}

/// The rejection half of `decide -> Result<Vec<PersonEvent>, PersonError>` (§10.1).
#[derive(thiserror::Error, Debug)]
pub enum PersonError {
    #[error("person {0} does not exist")]
    NotFound(PersonId),
    #[error("name must have a given name or a surname")]
    EmptyName,
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    #[error("persons {surviving} and {merged} cannot be merged: {reason}")]
    MergeConflict { surviving: PersonId, merged: PersonId, reason: String },
}

// The pure decision core (ADR 0004 §3): no clock, no id generation, no I/O.
// It matches the command, checks within-aggregate invariants against `state`, and
// returns events or a domain error:
//   fn decide(state: &PersonState, command: PersonCommand)
//       -> Result<Vec<PersonEvent>, PersonError>
// The cqrs-es `Aggregate::handle` impl is a thin adapter that calls it.

pub enum ChromosomeSide {
    Maternal,
    Paternal,
    Unknown,
}

pub struct DnaSegment {
    pub chromosome: String, // 1..=22 or "X"
    pub start: u64,
    pub end: u64,
    pub centimorgans: f64,
    pub snps: Option<u32>,
    pub side: ChromosomeSide,
}

/// A pairwise observation between two DNA tests; owned by neither person (§12).
/// The relationship it implies is a separate, citing assertion — not a field here.
pub struct DnaMatch {
    pub id: DnaMatchId,
    pub test_a: DnaTestId,
    pub test_b: DnaTestId,
    pub provider: DnaProvider,
    pub shared_cm: f64,
    pub percent_shared: Option<f64>,
    pub segment_count: u32,
    pub largest_segment_cm: f64,
    pub predicted_relationship: Option<String>,
    pub segments: Vec<DnaSegment>,
}
```

## 16. Cross-reference mapping

For import/export fidelity. "—" means no direct equivalent.

| This model                                             | Gramps 6                          | GEDCOM 7                                                                   | GEDCOM X                                 |
| ------------------------------------------------------ | --------------------------------- | -------------------------------------------------------------------------- | ---------------------------------------- |
| Person (conclusion)                                    | Person                            | `INDI`                                                                     | Person (Subject)                         |
| Person (persona)                                       | —                                 | —                                                                          | Person (extracted)                       |
| Family                                                 | Family                            | `FAM`                                                                      | Couple + Child-and-Parents relationships |
| Event                                                  | Event (+ `EventRef`)              | event structures + `ASSO`/`ROLE`                                           | Fact / Event + roles                     |
| Place                                                  | Place (`PlaceRef`, `PlaceName`)   | `PLAC` + `_LOC`                                                            | PlaceDescription                         |
| Source                                                 | Source                            | `SOUR`                                                                     | SourceDescription                        |
| Citation                                               | Citation                          | `SOUR` pointer + `PAGE` + `QUAY`                                           | SourceReference                          |
| Repository                                             | Repository                        | `REPO`                                                                     | Agent / SourceDescription                |
| Media                                                  | Media                             | `OBJE`                                                                     | SourceDescription (digital artifact)     |
| Note (`RichText`)                                      | Note (`StyledText`)               | `SNOTE`/`NOTE` (text + `MIME` + `LANG`)                                    | Note (`textType`)                        |
| Tag                                                    | Tag                               | — (`_UID`/extensions)                                                      | —                                        |
| `EventContext`                                         | `change` timestamp (partial)      | `CHAN`, `_UID`                                                             | Attribution                              |
| `Confidence`                                           | Citation confidence               | `QUAY`                                                                     | ConfidenceLevel                          |
| `EvidenceAnalysis`                                     | —                                 | —                                                                          | — (research-process guidance)            |
| `GenealogicalDate` (+ `time`)                          | Date (`MOD_*`,`QUAL_*`,`newyear`) | `DATE` value/period/range + `TIME` + `INT`                                 | Date                                     |
| `Address`                                              | Address                           | `ADDR` (`ADR1-3`/`CITY`/`STAE`/`POST`/`CTRY` + `PHON`/`EMAIL`/`FAX`/`WWW`) | Address                                  |
| `ParticipantRole` / `AssociationRole`                  | `EventRef` role                   | `ASSO`.`ROLE` (full set, both contexts)                                    | role in Fact                             |
| `restrictions` (`Restriction` set)                     | `private` flag (boolean, lossy)   | `RESN` (`CONFIDENTIAL`/`LOCKED`/`PRIVACY`)                                 | —                                        |
| `ExternalId`                                           | `gramps_id` / handle              | `EXID` / `UID`                                                             | `identifiers` (Primary/Persistent)       |
| `Agent`                                                | — (change author only)            | `SUBM` / `_UID` author                                                     | `Agent` / `Attribution.contributor`      |
| `DnaTest`                                              | DNATest (native DNA model)        | —                                                                          | —                                        |
| `DnaMatch` (+ `DnaSegment`)                            | DNAMatch + DNASegment             | proposed `DNA_MATCH`; FTM `_DNA`                                           | —                                        |
| `LanguageTag`                                          | name/place language               | `LANG` (BCP-47)                                                            | `lang`                                   |
| `PersonName` transliteration / `RichText` translations | —                                 | `NAME`.`TRAN` / `NOTE`.`TRAN`                                              | Name / Note (alternate `lang`)           |

## 17. Open questions and deferred work

- **Configurable surety scheme.** GENTECH allows per-project surety scales. We start with a fixed
  five-level `Confidence`; making the scheme configurable is deferred.
- **Proof-argument / `Analysis` document.** GEDCOM X has a first-class `Document(Analysis)` for
  written proof arguments; the GENTECH/GPS process wants research questions and tasks. Out of v1
  scope — likely a future `ResearchNote`/`Argument` aggregate.
- **GEDCOM round-trip strategy.** Lossy by nature (TMG's GenBridge exists for this reason);
  import/export mapping (using §16) is its own design task. The *model* carries the standard GEDCOM
  enumerated values as first-class variants, and the GEDCOM **import** parser fills them — structured
  `NAME` sub-records, the full `DATE` grammar (calendars/modifiers/dual dates), `ADDR`, INDI-attribute
  facts, and `ASSO` associations (`docs/phase-4-followups.md` group F′). The matching **export** now
  round-trips them back out: persons (structured name, sex, INDI-attribute facts, `ASSO`
  associations), their individual/family events (type, `DATE` grammar, `PLAC`, `ADDR`), and
  top-level `SOUR` records. **Owner-linked citations, media, and notes now round-trip too**
  ([ADR 0018](adr/0018-round-trip-owner-links-and-host-api-0.8.md)): Person/Family/Event project
  their attached citations/media/notes/tags, the importer attaches each owned record to its owner,
  and the read DTOs expose them — so an import → export → import cycle preserves that data
  (`host-api@0.8.0`). Repositories, place hierarchy (`enclosed-by`), place type, citation
  confidence, and source author/`PUBL` round-trip too. Both the **Gramps XML** and **GEDCOM**
  plugins emit the owner-linked citations/media/notes (GEDCOM `INDI.SOUR`/`OBJE`/`NOTE` + `SOUR`
  `AUTH`/`PUBL`); Gramps additionally carries repositories, place type/hierarchy, and citation
  confidence. **The participation payload now round-trips too**
  ([ADR 0019](adr/0019-participation-ownership-and-payload.md), `host-api@0.13.0`): a participant's
  **age** at an event (GEDCOM `INDI.*.AGE` and `HUSB`/`WIFE` `AGE`; Gramps eventref `Age` attribute)
  and **event-level witnesses** (GEDCOM `2 ASSO` under an event with `ROLE`/`SOUR`/`NOTE`; Gramps a
  person-side `<eventref role=…>` with `<attribute>`/`<noteref>`) survive the cycle — closing the
  §17 event-level-witness gap. A witness's role and notes round-trip both ways; Gramps additionally
  round-trips participant-scoped **attributes**.
  - **Still does not round-trip out — and why:**
    - **Custom-role/custom-type events.** A person's participation is recorded on the *person*
      (`ParticipationAsserted`); export reconstructs an INDI/FAM event from each person's
      `participations`. A participant with a `Custom` role, or an event whose `EventType` is `Custom`,
      has no GEDCOM/Gramps enum slot and is dropped on export.
    - **Participation attributes and primary-participation notes over GEDCOM.** GEDCOM has no slot for
      a participant-scoped attribute, so participation `attributes` are GEDCOM-lossy (Gramps carries
      them on the `<eventref>`). Notes on a *primary* participation are also dropped on GEDCOM export —
      an event-scoped `2 NOTE` would re-import as an event note, not a participation note; only a
      *witness*'s notes ride the `2 ASSO`/`3 NOTE` under their association.
    - **Event-`ASSO` witness citations are import-only.** A witness `SOUR` (or Gramps `<citationref>`
      on the eventref) imports to the participation's assertion envelope (`EventContext.citations`, the
      sole evidence channel — [ADR 0020](adr/0020-evidence-citations-live-in-the-envelope.md)), so it
      raises the participation's source count, but export does not re-emit it (matching the
      fact-citation precedent — the WIT `participation` read record carries no citations). Full witness
      citation export is a follow-up.
    - **Model gaps a real-world file carries** (none modelled yet): multiple `NAME` records per
      person; INDI↔FAM `FAMS`/`FAMC` back-refs;
      `PLAC.MAP` / Gramps place coordinates; submitter (`SUBM`) and `HEAD` metadata; media
      `FORM`/type/`CAPT`; citation `CALN`; Gramps `<tagref>` on the person/family record (tags are
      created but not yet attached to their owner on import); the adoption-to-family link
      (`ADOP.FAMC`); the verbatim `Address.original_text` fallback for an unsplittable `ADDR`.
- **Restriction (`RESN`).** Privacy is a `Restriction` set (`Confidential`/`Locked`/`Privacy`) on
  every aggregate (§6, §7), set by the universal `SetRestrictions` command. The GEDCOM round-trip
  carries `RESN` on **Person and Family** (the living-person records it matters for); Gramps stores
  a single boolean `priv`, so its mapping is lossy (import `priv=1` → `{Privacy}`, export
  non-empty → `priv=1`). Per-record `RESN`/`priv` round-trip for the remaining records (events, sources,
  citations, media, notes, repositories, places) is **deferred** — the field exists everywhere and
  crosses the plugin boundary (host-api 0.9.0), but the format plugins only emit/parse it for
  person/family today.
- **Address reach and verbatim parsing.** `Address` is wired on `Repository` and `Event` (a
  residence/census `ADDR`, group F′); widening it to other aggregates and a verbatim
  `original_text` fallback on import is deferred.
- **Child-link proof status and sort date.** GEDCOM `FAMC`.`STAT` (`CHALLENGED`/`DISPROVEN`/
  `PROVEN`) and a user-supplied `SDATE` (distinct from `GenealogicalDate.sort_value`, which we
  compute) are not modelled yet; the proof status overlaps the evidence/confidence layer.
- **LDS ordinances.** LDS-specific events (`BAPL`/`CONL`/`ENDL`/`SLGC`/`SLGS`), blessing, and
  mission are intentionally left to `EventType::Custom` rather than first-class variants.
- **Internationalization tail.** Stored locale **collation keys** (vs deriving them per query),
  localized **enum-label catalogs**, the GEDCOM 7 `PHRASE`-has-no-language limitation, and how much
  script-variant editing UI to build are deferred — none changes the core model.
- **DNA depth and round-trip.** Y-DNA/mtDNA STR markers and haplogroup detail, triangulation
  groups, and a DNA GEDCOM round-trip (no stable standard — only a proposed `DNA_MATCH`) are
  deferred. So is a match-confidence scheme and how far to auto-infer relationships
  (Theory-of-Relativity-style) versus leaving inference to the researcher.
- **Projection / read-model schema** and **event-version upcasting** — explicitly deferred by
  ADR 0002. ADR 0004 §4 fixes the event *encoding* so versioning is possible from day one, but the
  upcasting *tooling* remains future work.
- **Snapshotting** — `cqrs-es` supports aggregate snapshots; whether replay cost warrants them is
  deferred until measured (ADR 0004, *Out of scope*).

## References

- [ADR 0001](adr/0001-use-event-sourcing-for-the-domain-core.md) — event-sourced domain core.
- [ADR 0002](adr/0002-cqrs-es-framework-and-per-workspace-database.md) — `cqrs-es`, SQLite default.
- [ADR 0004](adr/0004-event-sourcing-implementation-contract.md) — provenance-in-payload,
  `AssertionId`, the pure-`decide` determinism boundary, event encoding/versioning.
- [`docs/research/event-sourcing-rust.md`](research/event-sourcing-rust.md) — event-store patterns.
- Gramps data model — <https://gramps-project.org/wiki/index.php/Database_Formats>.
- GEDCOM 7 — <https://gedcom.io/specifications/FamilySearchGEDCOMv7.html>.
- GEDCOM X conceptual model — <https://github.com/FamilySearch/gedcomx> (`specifications/`).
- GENTECH GDM primer — <https://genealogy.sourceforge.net/GENTECH_Primer.html>.
