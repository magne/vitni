# 31. The place model for administrative geography

- **Status:** Accepted
- **Date:** 2026-10-21

## Context

[`research/gis-norway.md`](../research/gis-norway.md) tested the place model against real Norwegian
administrative geography, 1838 to today, as the base for a reference-data import. The identity rule
held: a new legal unit is a new place, linked by succession (ADR 0026). Five things did not
(§ Recommendations 1, 2, 5, 6, 7):

- **Disconnected areas.** 280 of 357 municipalities have coastline, and Kvitsøy alone is 167 islands.
  `PlaceGeometry` is `Point` or `Polygon`, and ADR 0024 deferred the `Multi*` variants "when a concrete
  need appears". Several same-dated polygons do not substitute: `geometry_as_of` returns one while the
  spatial index holds every part (#569).
- **Hierarchies that do not nest.** In 1900 a farm sits in a *kommune*, a *sokn*, a *prestegjeld* and a
  *tinglag*. `PlaceRef { place_id, date }` has no relation, so every parent lands in one list,
  `enclosed_by_as_of` returns whichever import order put first, and the generated title can mix
  hierarchies. `PlaceSummary` exposes only the resolved chain, so a second parent cannot be seen or
  retracted (#570). The name and enclosure use-cases take no date (#568).
- **Geometry that says nothing about itself.** A surveyed boundary, a boundary reconstructed from a
  cadastre, and a pin standing for an area look the same, and two same-dated geometries at different
  generalisations are picked by assertion order. `Confidence` cannot carry metres: its ordinals are
  relabelled per workspace (ADR 0027 §3) (#571).
- **Codes that are reused.** SSB has given the same *kommunenummer* to different units: 1806 was
  Svolvær and is now Narvik. Place has no `ExternalId`, and `code` holds one undated value (#574, #693).
- **Missing types.** `PlaceType` has no diocese, deanery or *prestegjeld*, and `Custom(String)` renders
  raw text in a localized UI, against data-model §14 (#572).

[`research/places-beyond-norway.md`](../research/places-beyond-norway.md) then asked whether the answers
carry over to the other Nordic countries, Europe and the US. The core does: the identity rule, and dated,
language-tagged names, enclosure and geometry. The closed vocabularies do not. `PlaceType::County` is
documented as a first-level division, which is wrong for the US. GOV needs about 270 types, and three
Norwegian ecclesiastical variants would be the start of an endless list. Other countries need
registration, manorial, fiscal, military, cadastral and statistical hierarchies, parishes of several
denominations over one village, and successions where a unit is carved from parents that continue or a
part moves between units that both continue.

This ADR decides all of it now, before the reference import (#694) and its plugins depend on the closed
enums. It builds none of it. Two limits from the research stand: only the `Multi*` half of ADR 0024's
deferral reopens (`LineString` stays closed), and assertions get no `[from, until)` validity interval
([`decisions.md`](../decisions.md) § Model & interchange). A dissolved place resolving forever is #573,
which this ADR leaves as it is.

## Decision

1. **Geometry gains a multi-part shape, and each geometry assertion says what it is.**
   - `PlaceGeometry::MultiPolygon` holds one or more polygons, each an exterior ring plus holes, under
     the same ring rules as `Polygon` (`InvalidGeometry`). Its representative point is the largest
     part's, not an average across parts. `LineString` stays out of scope.
   - `PlaceGeometryAssertion` gains three optional fields, all `#[serde(default)]`:
     - `accuracy: PositionalAccuracy { metres, method }`, with `method` one of `Surveyed`, `Digitised`,
       `Derived` (computed from smaller units, as Danish DigDag boundaries are) or `Approximated`;
     - `role: GeometryRole`, `Extent` (the place's area or position) or `Representative` (a locator
       standing for an area whose shape is unknown);
     - `placement: Placement`, `Known`, `Approximate`, `Weak` or `Unlocated` (Pleiades). `Unlocated`
       means the place's position is not known and the geometry is the area it is known to lie in,
       drawn as that area and never as a marker.
   - None of the three is `Confidence`. They describe the measurement, and `Confidence` is the
     operator's surety in a claim, relabelled per workspace (ADR 0027).
   - The spatial index keeps one bounding box per geometry assertion. Containment follows the existing
     rule: the index finds candidates and one Rust predicate decides, on both engines
     ([`decisions.md`](../decisions.md), #608).

2. **An enclosure link names its hierarchy, and several parents may hold at once.**
   - `PlaceRef` gains `relation: Option<HierarchyKind>`, `partial: bool` and `preferred: bool`, all
     `#[serde(default)]`. `HierarchyKind` is its own closed enum: the place kinds of §3 that enclose as a
     hierarchy, i.e. all but `Settlement`, `Building` and `Natural`.
   - `partial` says the place lies only partly in the parent (Wikidata P1382): a Swedish parish split by
     a municipal border, a US city across two counties, a reservation across states. `preferred` marks
     the parent to show when several hold (Getty TGN).
   - Several parents of one relation at one date are allowed.
   - A stored link, and an imported one with no stated relation, decodes as `relation: None`, which
     means unstated. An unstated link takes part in every relation's walk, so data written before this
     ADR resolves as it does today.
   - Resolution:
     - `enclosed_by_as_of(target, relation)` reads the links of that relation plus the unstated ones,
       by the rule of §9.
     - The generated title and the hierarchy chain walk one relation. A place whose kind is a hierarchy
       kind walks its own relation: a *sokn* goes up through its *prosti* to its *bispedømme*. Any other
       place walks `Administrative`. The relation chosen at the first hop holds for the whole walk.
     - `primary_enclosed_by`, used where no date is known, returns a `preferred` link if there is one,
       and otherwise the first link asserted, dated or not, as today.
     - Matching keeps walking every enclosure link of every relation for a place's enclosing set and its
       country, so ADR 0038's Enclosure feature is unchanged. Being inside a parent is evidence whichever
       hierarchy says so.
   - `PlaceSummary` lists every live enclosure with its relation, flags, date and `AssertionId`, so each
     can be seen, edited and retracted (#570).
   - `add_place_name` and `assert_place_enclosed_by` take an optional date, read as effective from
     (ADR 0026 §1) (#568).

3. **A place type is a kind, a level and a vocabulary key.** `PlaceType` is replaced. A type assertion
   carries three optional fields:
   - `kind`, a closed enum: `Administrative`, `Ecclesiastical`, `Judicial`, `Registration`, `Fiscal`,
     `Military`, `Manorial`, `Cadastral`, `Statistical`, `Settlement`, `Building`, `Natural`;
   - `level`, a closed enum after Getty and GeoNames: `Country`, `Adm1` to `Adm5`, `Settlement`,
     `SubSettlement`. `County` stops meaning "first level": a Norwegian *fylke* is `Adm1`, a US county
     `Adm2`;
   - `type_key`, a vocabulary key such as `no:prestegjeld`, `se:harad`, `us:county` or `ie:townland`
     (§4). The field is `type_key` because `type` is a keyword in Rust and WIT.

   All three may be absent: a place imported from GEDCOM usually has no stated type. That replaces the
   `Custom("place")` sentinel and the `unstated_place_type` and `stated_place_type` helpers in
   `vitni-app`, which go away.

   Kind and level are stored as the operator stated them. A vocabulary entry suggests a kind and level
   for its key, and the app layer pre-fills them from it, but `decide` never consults a vocabulary
   (ADR 0004 §3), and a stated kind that disagrees with the entry is kept as stated. Rendering, the
   hierarchy walk and matching read the stored kind and level. The vocabulary supplies only the label.

   The type is dated and accumulates, like a name: `SetPlaceType` takes an optional date, and the type
   as of a date is resolved by ADR 0026 §1's rule. A *herred* that became a *by* has both types, each
   from its date.

   The country of a place becomes the nearest enclosing place whose stated `level` is `Country`.
   Matching's place-type feature compares kind, then level, then the key within one namespace, and an
   unstated side is neutral.

4. **Type vocabularies are data that plugins ship.**
   - A vocabulary entry is a key, the kind and level it suggests, an optional date span during which
     the type existed (shown, never enforced), and a Fluent message id for its label. An entry belongs
     to one of two sets: place types (§3) or denominations (§5).
   - **Key syntax.** `<namespace>:<name>`. A namespace is `[a-z0-9]+`: a country's ISO 3166-1 code in
     lower case (`no`, `se`, `us`) or a source's name (`gov`). A name is `[a-z0-9-]+`. A namespace
     belongs to one plugin id: a second plugin declaring keys in it is not loaded, and the error names
     both. The built-in `std` and `custom` namespaces below belong to no plugin, and a plugin may not
     declare keys in them. `custom` keys are the one exception to the name grammar.
   - **Who ships them.** A vocabulary plugin is a WASM component in a new `vocabulary` world of the
     host API, exporting its entries. Its `plugin.toml` names the new `vocabulary` role. It requests no
     capabilities. The bundle format, signing and discovery are those of ADR 0014 §2. The Norwegian
     vocabulary is a first-party plugin in the embedded fleet (ADR 0014 §7), so it is installed by
     default. A new country is a plugin, not a core change.
   - **Labels.** The entry's message id lives in the plugin's own `.ftl` catalogue, layered as ADR 0007
     §10 says. Host screens, not only plugin panels, render it: the Explorer, the record editor, the
     picker, the breadcrumb, the map label and the CLI resolve a stored key through the loaded
     vocabularies to its message id, then through the catalogues with the usual locale fallback. The
     plugin has no `fl!()` call to name an id, so `cargo xtask i18n-check` cannot see an entry's id and
     only warns about unused catalogue keys. The vocabulary loader checks instead: it does not load a
     vocabulary with an entry whose message id its own catalogue lacks in the fallback language, and the
     error names the plugin, the key and the id.
   - **A key with no loaded vocabulary** renders as the kind's label followed by the key itself
     (*Ecclesiastical · no:prestegjeld*). The key is a language-neutral code, not prose, so data-model
     §14 holds. It never renders as an empty cell.
   - **Two namespaces are built in**, because old data and Gramps files must read with no plugin
     installed:
     - `std` holds Gramps v6's standard place types (`std:country`, `std:state`, `std:county`,
       `std:city`, `std:parish`, `std:farm`, …). Their labels are in the `vitni-ui` and `vitni-cli`
       catalogues, as the `PlaceType` labels are today, and they are generic in every locale: a
       Norwegian *fylke* is `no:fylke` from the Norwegian vocabulary, not `std:county`.
     - `custom` holds a type the user or a file typed as free text: `custom:<text>`, where the text may
       be anything and renders as typed. It is data the user entered, like a name.
   - **Gramps round-trip.** Gramps standard types import as their `std` key and export back unchanged.
     Any other key exports as a Gramps custom type whose text is the key, so a round-trip restores it
     exactly. GEDCOM 7 has no place types, and its `FORM` is never read as levels.

5. **An ecclesiastical type may name its denomination.** A type assertion of kind `Ecclesiastical` may
   carry `denomination`, a key from a denomination vocabulary (`std:lutheran`, `std:catholic`,
   `std:orthodox`, `std:jewish`, …), labelled as §4 says. Several congregations' parishes can then cover
   one village (Germany, the Netherlands, Poland, the Baltics). It rides on the type assertion, so it is
   dated with it. `decide` rejects a denomination on any other kind.

6. **Two succession kinds for changes where every unit continues.** `SuccessionKind` gains:
   - **`Formed`**: a new place carved from one or more parents that continue (most US counties). `from`
     is the parents, at least one, and `to` is exactly the new place. The event lives on the new place:
     the anchor is the `to` place, where the other kinds anchor in `from`.
   - **`TerritoryTransferred`**: a part moves from one unit to another and both continue (US counties,
     Finnish *osakuntaliitos*). `from` is exactly one donor, `to` exactly one recipient, and the anchor
     is the donor. An optional `area` holds the transferred part: a `Polygon` or `MultiPolygon` under
     the ring rules, dated by the succession's date. It is not a geometry assertion on either place and
     is not in the spatial index. Each place's new boundary is still its own dated geometry assertion.

   The existing kinds keep their rules. A `from` place of `Formed` or `TerritoryTransferred` has not
   ceased, so neither kind counts as a cessation where one is derived (#573), and "what became of this
   place" does not follow them. Both still project both ways, so a county shows the counties it was
   formed from and the transfers it gave and received. New `PlaceError` variants name each broken rule:
   a `Formed` without exactly one new place or without a parent, a transfer without exactly one donor
   and one recipient, a transfer area that is not an area, and an anchor on the wrong side.

7. **A re-import reconciles a dated type per effective date.** ADR 0029's timestamp gate compares a
   live assertion with the file's claim for one field. For the place type, the field is the type at one
   effective date: `record_origins` derives a field key that carries the date (`place.PlaceTypeSet` for the
   undated type, the effective date appended for each dated one). A file's 1838 type supersedes only a
   live 1838 type, by the same gate, and leaves the 1964 one alone. Enclosure stays additive, as ADR
   0037 §4 says multi-valued fields do: a link the file no longer carries is retracted explicitly, never
   superseded. `code` stays single-valued and undated (§8), so it reconciles as today.

8. **Place gets external ids, keyed with a validity date.**
   - Place gains `ExternalId`s as Person and Family have them: `CreatePlace` carries them, `AddExternalId`
     adds one, `PlaceState` holds them, Place joins the store's external-id lookup, `staged-place`
     carries them, and the import planner links a place by them (`Disposition::Link` with
     `LinkBasis::ExternalId`, ADR 0040) (#693).
   - `ExternalId` gains `valid: Option<GenealogicalDate>`, `#[serde(default)]`, limited to the `None`,
     `From`, `To` and `Span` modifiers. A plain date covers the period its stated parts span: a year
     alone is that whole year (a code valid during 1964 only), a year and month that month, a full date
     that day. `From` and `To` leave one end open. Any other modifier or a
     text-only date is rejected, because it has no defined overlap. Person and Family share the type and
     gain the field unused.
   - **Identity is `(authority, value, valid)`.** Two ids with the same authority and value are the
     same identity when their validity periods overlap. A missing validity is unbounded and overlaps
     every period. An undated incoming 1806 therefore overlaps both Svolvær and Narvik.
   - **Ambiguity is never resolved silently.** The store returns every place carrying the authority and
     value, and a pure Rust helper keeps those whose validity overlaps. One match is a deterministic
     link. More than one is `Candidates` (ADR 0040), for the user to decide.
   - **Authorities.** A new authority is an absolute URI naming the register, as GEDCOM 7 `EXID.TYPE`
     does: SSB KLASS's classification URI for a *kommunenummer*, Wikidata for a QID. A stored bare
     authority (`gedcom-uid`, `digitalarkivet`) stays valid and matches only itself exactly. No
     upgrader rewrites it.
   - **A *kommunenummer* is a Place `ExternalId`** under SSB's authority, dated by its validity, and
     matching and re-import resolve places by it. Gramps' `code` stays a plain, undated text field that
     round-trips with Gramps. #574's dated, accumulating `code` is declined: the identifier history it
     wanted is the place's dated external ids. The place's profile lists them read-only, so the External
     ids rule in [`decisions.md`](../decisions.md) (no frontend entry point) applies to Person and
     Family, whose ids only an importer writes.

9. **A name can be official in its language, and one ranked tie rule serves every resolver.**
   - `PlaceName` gains `official: bool`, `#[serde(default)]`: the name is an official name in its
     `language` at its date. Bilingual Finnish municipalities, Irish and Welsh units and Sámi names have
     more than one. `decide` rejects an official mark on a name with no language.
   - `name_as_of_in(target, language)` resolves a name for a display language. The generated title
     passes the display locale. `name_as_of` stays as it is and ignores the mark.
   - `temporal::resolve_as_of` takes a rank, and its tie rule becomes:
     - Among dated items, the latest effective date not after the target wins, as today. On a tie, the
       higher rank wins, then the last asserted, as today.
     - When no dated item qualifies, the undated items are the fallback: the higher rank wins, then the
       first asserted, as today.
   - Ranks:
     - **name:** official in the requested language, then official in any language, then the rest;
     - **enclosure:** `preferred`, then not `partial`, then `partial`;
     - **geometry:** `Extent` before `Representative` before an unstated role, then the smaller
       `metres`, with an unstated accuracy last.
   - Every resolver uses the same rank, including the geometry-over-time view-model in `vitni-ui`, so
     the map and the record agree. With equal ranks the result is exactly today's.

10. **Replacing `PlaceType` is the one change that is not additive.**
    - `PlaceCreated` and `PlaceTypeSet` change shape, so their `event_version` changes (ADR 0004 §4). A
      merge or distinguish decision stores `MatchEvidence`, whose place-type feature carries the old
      value, so those events change too.
    - The backup format version bumps, and its upgrader step maps every old value by the table below, a
      pure function of the old value alone (ADR 0041 §3). The golden fixture is regenerated (ADR 0041
      §5). There are no upcasters (ADR 0018). A live workspace moves by a backup with the old version and a
      restore with the new.
    - Everything else here is additive under `#[serde(default)]`: the enclosure flags, the geometry
      fields, the official mark, external-id validity, the succession area and the new
      `SuccessionKind` variants.
    - A saved search or list column naming the place `type` column keeps the column, which becomes a
      text column over `type_key`. A saved clause that used a choice operator on it no longer has an
      operator its type takes, so ADR 0052 §10 already applies: the search does not run and the clause
      is marked for the user. Dropping a stale value, as §10 does for a deleted tag, would widen an
      *is none of* clause, which §10 forbids, so no extension of §10 is needed. `kind` and `level` are
      new choice columns. `workspace.toml` is not upgraded.

    | Old value | `kind` | `level` | `type_key` |
    | --- | --- | --- | --- |
    | `Country` | Administrative | Country | `std:country` |
    | `County` | Administrative | — | `std:county` |
    | `Municipality` | Administrative | — | `std:municipality` |
    | `Parish` | Ecclesiastical | — | `std:parish` |
    | `City` | Settlement | Settlement | `std:city` |
    | `Town` | Settlement | Settlement | `std:town` |
    | `Village` | Settlement | Settlement | `std:village` |
    | `Farm` | Settlement | — | `std:farm` |
    | `Building` | Building | — | `std:building` |
    | `Custom("place")` | — | — | — (no type stated) |
    | `Custom(text)` | — | — | `custom:<text>` |

    Only `Country` and the settlements get a level: a function of the old value alone cannot know
    whether a `County` was a *fylke* or a US county, and the Gramps import also folded State and Province
    into it. A municipality is `Adm2` in Norway and `Adm3` where counties sit under states, and a farm may
    be a settlement or part of one, so neither gets a level either.

This ADR amends earlier decisions in these sections:

- ADR 0013 §6: the identity key of an external id gains a validity (§8).
- ADR 0026 §1: the tie rule of `resolve_as_of` becomes ranked (§9). ADR 0026 §2: a territory transfer
  is a succession with an area (§6). ADR 0026 §3: `SuccessionKind` gains `Formed` and
  `TerritoryTransferred` (§6).
- ADR 0029 §1, the timestamp rule ADR 0049 left standing: a dated place type reconciles per effective
  date (§7).
- ADR 0037 §4: the place type's field key carries the effective date (§7). Enclosure stays additive as
  that section says.
- ADR 0038 §6: the same `ExternalId` needs overlapping validity (§8).

## Rationale

- **A key, not more variants (3, 4).** Every closed list tried so far was one country's list. GOV
  rejected unique type names for the same reason, and TGN and GeoNames split the abstract level from
  the local name. A small closed `kind` keeps the code's branches finite, an optional `level` answers
  "is this first-level", and the key carries the unit's real name without a core change per country.
- **Kind and level stored, not looked up (3).** A pure `decide` cannot read a registry, and a type
  whose meaning changes when a plugin is upgraded or removed would make old records say something new.
  Storing what the operator stated keeps the assertion self-contained (ADR 0002).
- **Vocabularies as components (4).** A data-only bundle would need a second bundle format, a second
  signing scope and a discovery path with no component to check the manifest against. A `vocabulary`
  world reuses all of ADR 0014 for the cost of one small export.
- **An explicit relation (2).** GOV infers the hierarchy from the parent's type, but a Swedish *socken*
  is both a civil and a church unit, and a *prestegjeld* and a *herred* were often coterminous and would
  be typed alike. The relation must be stated. Unstated links joining every walk keeps today's data
  resolving as it does.
- **Matching walks every link (2).** "Born in Fana" and "born in Fana prestegjeld" are the same evidence
  of place for matching. Restricting the walk to one resolved parent would drop real agreement and
  shift scores on Gramps data, which already carries several parents.
- **Validity in the identity key (8).** SSB's own key for a municipality is the number together with
  its validity. Keying on the number alone merges Svolvær into Narvik on the first import. Returning
  `Candidates` on an overlap keeps the deterministic band honest (ADR 0038 §6).
- **A *kommunenummer* as an external id, not a dated `code` (8).** It is an identifier from a register,
  which is what `ExternalId` is for (ADR 0037 §3). Making `code` dated as well would hold the same
  number twice, free to drift.
- **Per-date reconciliation for the type only (7).** A dated type is several values that hold one after
  another, so one file's view of 1838 must not overwrite another date. Enclosure, by contrast, holds
  several values at once (§2), so superseding one link with another would delete a true parent.
- **One rank in one function (9).** Three resolvers and a view-model share `resolve_as_of`. Ranking in
  each wrapper would let the map and the record disagree on the same data.
- **Stale handling over a `workspace.toml` upgrade (10).** ADR 0052 §10 already refuses to run a search
  it cannot evaluate, and the column keeps its id. An upgrader over `workspace.toml` would extend the
  backup format for a rewrite the user would want to check anyway.

## Consequences

### Positive

- The Norwegian civil and ecclesiastical import can be written with no workaround, and a new country is
  a vocabulary plugin.
- Islands, parallel hierarchies, partial containment, reused codes and bilingual names are first-class,
  and each is correctable by `AssertionId`.
- No type renders as raw prose. A missing vocabulary degrades to kind plus key.
- Re-importing a dataset vintage is idempotent per field and date, and an ambiguous code reaches the
  user instead of merging two municipalities.
- The Gramps round-trip keeps every type.

### Negative / costs

- Replacing `PlaceType` touches every layer: core (`enums.rs`, the place aggregate, matching), the
  store (`record_origins`, the list table, the external-id lookup), `vitni-app` (staging, profiles,
  history values, the backup upgrader and its golden fixture), `vitni-ui` and `vitni-ui-dioxus` (place
  screen, picker, list filters, labels), `vitni-cli` (`PlaceTypeArg`, help, catalogues), the host API
  (`place-type`, `place-dto`, `set-place-type`, `staged-place`, the `vocabulary` world, a minor version
  bump), every interchange crate and plugin that reads or writes a type, the match-eval corpus, and the
  tests that build `PlaceType` literals.
- `MultiPolygon` needs an arm in every geometry match: validation, the representative point, WKB and
  GeoJSON, the spatial index on both engines, and the map.
- A succession's `from` no longer always means "ceased", so its UI label and docs become per kind.
- Host screens now depend on plugin catalogues for some labels, and a removed vocabulary plugin changes
  how its places render.
- Kind and level stated by hand can disagree with the vocabulary's suggestion, and nothing reconciles
  them.
- One more host-API world and plugin role to keep, sign and ship.

## Out of scope

- **Cessation** — `existed_as_of` and a dissolution with no successor (#573). §6 only says which kinds
  do not cease.
- **Validity intervals on assertions** — still closed ([`decisions.md`](../decisions.md)).
- **A batched Place command and idempotent recurring import** (gis-norway Recs 5b, 5c) — they need
  their own ADR 0004 §3 decision on minting many assertion ids per command.
- **Separating reference data from the user's places** (#696) and the data licences.
- **The Linked Places Format importer** (places-beyond-norway Rec 8).
- **Sweden's 1700–1712 calendar** — a date-model gap.
- **A church building standing for its parish** (GOV's `represents`), and street- and house-level places.
- **The Postgres mirror of the new indexes** — follows the existing split per engine.

## References

- ADR 0002 / 0004 — self-contained events, the pure `decide`, `event_version`.
- ADR 0003 §3, ADR 0022 §5 — frontends own their catalogues, and plugin panels resolve against the
  plugin's. §4 widens the plugin catalogue to host screens through ADR 0007 §10.
- ADR 0007 §10, §11 — plugin catalogues and base plugins. ADR 0011 §1 — versioned worlds. ADR 0014 §2,
  §7 — the bundle, its roles, and the embedded fleet.
- ADR 0013 §6, ADR 0037 §3–§4, ADR 0038 §6, ADR 0040 — external ids, record origins, deterministic
  identity, the import plan.
- ADR 0018, ADR 0041 §3, §5 — no upcasters, backup upgraders and the golden fixture.
- ADR 0024 — place geometry. ADR 0026 — temporal resolution and succession. ADR 0027 — why `Confidence`
  carries no metres. ADR 0029, ADR 0049 — the timestamp gate, and what of 0029 §1 still stands.
- ADR 0052 §2, §10 — operators per column type and stale saved searches.
- [`research/gis-norway.md`](../research/gis-norway.md) § Model fitness, § Recommendations;
  [`research/places-beyond-norway.md`](../research/places-beyond-norway.md) § Recommendations.
- Issues: #568 (§2, dated writes), #569 (§1, `MultiPolygon`), #570 (§2, relation and the enclosure
  set), #571 (§1, accuracy, role, placement), #572 (§3, §4, the types it asked for), #574 (§8,
  declined), #693 (§8, place external ids), #694 (§1–§6, §8, what `staged-place` carries).
