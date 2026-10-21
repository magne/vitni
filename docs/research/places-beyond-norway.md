# Research — Does the Norwegian place model carry over to other countries?

- **Status:** Research. No code, no ADR, no issue edits — recommendations only.
- **Date:** 2026-10-21
- **Question:** Does the place model that milestone 0.11 arrives at — [`gis-norway.md`](gis-norway.md)
  and ADR 0031 as scoped in #692 — carry over to the other Nordic countries, the rest of Europe
  (especially Scandinavia's neighbours), the US and elsewhere? If not, what would make it generalise?
- **Method:** Three web research passes (Nordic; Europe; US/Canada/Australia plus gazetteer
  standards), checked against the code (`crates/vitni-core/src/enums.rs`, `place_name.rs`,
  `place_ref.rs`, `date.rs`). A claim with no link is analysis, not a finding. **UNVERIFIED** marks what
  could not be confirmed.

## Summary

**The core of the model carries over everywhere. Its vocabularies do not.**

What holds in every country examined:
- the identity rule (a new legal entity is a new place, linked by succession);
- dated, language-tagged names;
- dated enclosure;
- dated geometry.

[GOV](https://genwiki39.genealogy.net/GOV/Datenmodell), the German genealogical gazetteer with over a
million objects across Central and Eastern Europe, has the same shape: dated, sourced names, types and
part-of relations. Wikidata, the Linked Places Format and Getty TGN put dates on the same things.
`PlaceName.language` is a BCP-47 tag, so scripts (`ru-Cyrl`) are already covered. Dates already carry a
`Calendar`.

Four parts of 0.11's plan are Norway-shaped and would need reworking country by country:

1. **`PlaceType` is a closed list built around one country's levels.** Its `County` variant is
   documented as the "first-level division (county, state, province)", which is wrong for the US, where
   the state is first-level. GOV needs about 270 types. Adding three Norwegian ecclesiastical variants
   (#692) is the first step of an endless list, and `Custom(String)` shows untranslated text.
2. **The planned `relation` (Civil | Ecclesiastical | Judicial) is too narrow.** Other countries need
   hierarchies for:
   - civil registration (Standesamt, English registration districts, Scottish registration districts,
     Polish registry offices);
   - manors (Prussian Gutsbezirk, Estonian manors);
   - fiscal and poor-law units (Swedish fögderi, Poor Law Unions);
   - military units (Swedish allotment);
   - land and cadastre (Danish ejerlav, the US land survey grid, Australian land parishes);
   - statistical units (census divisions).
3. **A parish is assumed to belong to one church.** In Germany, the Netherlands, Poland and the
   Baltics, several denominations' parishes cover the same village.
4. **Successions cover only identity changes.** The commonest change in the US and Finland is a
   partial territory transfer between units that both continue. There is also no kind for a new unit
   carved from a parent that continues, which is how most US counties were formed.

None of these changes the Norwegian import. All of them get harder once places, events and plugins
depend on the closed enums. **Recommendation:** widen ADR 0031 (#692) now with:
- an open, namespaced type vocabulary with a small closed core;
- a wider closed set of hierarchy kinds;
- denomination on church units;
- two more succession kinds;
- URI-typed external-id authorities.

Detail in [§ Recommendations](#recommendations).

## What carries over unchanged

| Model element | Evidence it generalises |
| --- | --- |
| Identity rule: new legal entity = new place + succession; continuity = same place | Same rule in GOV (Neumünster keeps its object when it goes from Flecken to Stadt in 1870 — [hbz PDF](https://s2w.hbz-nrw.de/download/pdf/9856973.pdf)). The Dutch *Amsterdamse code* survives boundary and name changes by design ([DANS](https://doi.org/10.17026/dans-xdr-cs36)). |
| Dated, language-tagged names | Finnish/Swedish official names change independently ([Statistics Finland](https://stat.fi/fi/luokitukset/kunta/kunta_1_20260101)). Estonian KNAB lists German, Estonian and Russian forms side by side ([KNAB](https://arhiiv.eki.ee/knab/valik/kbeehald.pdf)). Tschirnitz/Czernica is one GOV object with three dated names. Scripts come with the BCP-47 `LanguageTag`. |
| Country as a dated enclosure | Schleswig, Alsace, the Polish partitions, Karelia, Skåne and the Baltic provinces under Sweden and then Russia all fit as a dated change of parent. Historic states (Prussia, Congress Poland, Russian governorates) are places too. |
| Dated geometry, optional | Pleiades, GOV and Newberry all allow a place with no shape; GOV gives only settlements a position. A boundary adjustment is a new dated geometry, as `gis-norway.md` §4 already says. |
| `PositionalAccuracy` / `GeometryRole` (planned) | Pleiades rates every location's accuracy ([Pleiades](https://pleiades.stoa.org/help/pleiades-data-model)). Danish DigDag boundaries are computed from cadastral units, not surveyed ([DigDag](https://dataforsyningen.dk/data/3961)). |
| Calendars on dates | `Calendar::{Gregorian, Julian, …}` exists (`date.rs`). Switch dates differ by country *and* by denomination ([list](https://en.wikipedia.org/wiki/List_of_adoption_dates_of_the_Gregorian_calendar_by_country)). **Gap:** Sweden's own calendar 1700–1712, with its 30 February 1712, has no variant. That is a date-model issue, outside ADR 0031. |

## Per region

### Nordic countries

| Country | Verdict | What it needs beyond the Norway bundle |
| --- | --- | --- |
| Sweden | Small additions | Fiscal and military hierarchies (fögderi; TORA's "Militär indelning" type — [Riksarkivet](https://sok.riksarkivet.se/topografier/nad)); a härad (hundred) type; the landskap as a cultural province; parishes with **two civil parents on the same date** (12 parishes split by a municipal border in 2015, and since 2020 parishes need not follow municipal borders — [SCB](https://regina.scb.se/indelningar)); non-territorial parishes (court, admiralty and regiment parishes) with no geometry. |
| Denmark | Small additions | A cadastral hierarchy: ~9,000 ejerlav, where a cadastral number is unique only within its ejerlav ([GST](https://gst.dk/data-og-kort/matrikelkort-og-data/matrikelregistret)), and an ejerlav has civil, church *and* judicial parents. Herred as a judicial district. The South Jutland transfers of 1864 and 1920 change the country parent. |
| Finland | Small additions | **Partial transfers** (*osakuntaliitos*) move cadastral units between municipalities while both keep their codes ([crosswalk notes](https://github.com/miikapaal/finland-kunta-crosswalk)). Bilingual official names, so a name needs an "official in this language" mark. A merged municipality keeps the surviving one's number, so a code identifies a lineage, not one entity. |
| Iceland | Carries over | Sýsla, hreppur, sókn and prestakall fit dated types. Farm names repeat, so a farm is only identifiable through its sókn, hreppur and sýsla — the full chain must show ([Icelandic Roots](https://www.icelandicroots.com/place-names)). |
| Faroe, Greenland, Åland | Carries over | Not researched in depth. |

### Rest of Europe

| Country | Verdict | What it needs |
| --- | --- | --- |
| Germany | Additions | A **registration** hierarchy: Standesamt districts from 1874/76, made of one or more municipalities or estates. A **manorial** one: 11,894 independent Gutsbezirke before Prussia dissolved most of them in 1928–30 ([GenWiki](https://genwiki39.genealogy.net/Gutsbezirk)). Lutheran and Catholic parishes over the same village. |
| Netherlands | Minimal | The Amsterdamse code is a ready-made external id. Yearly municipal boundaries from 1812 exist ([IISG, CC BY-SA 4.0](https://datasets.iisg.amsterdam/dataset.xhtml?fileAccess=Public&persistentId=hdl%3A10622%2FURI8O2&version=1.0); [gemeentegeschiedenis.nl](https://www.gemeentegeschiedenis.nl/pagina/5/)). Several denominations' registers. |
| England & Wales | Additions | Ecclesiastical and civil parishes as separate places over the same land. **Registration** districts and Poor Law Unions, mostly but not always identical ([UKDS](https://datacatalogue.ukdataservice.ac.uk/studies/study/9032)). Hundreds. |
| Scotland | Additions | Registration districts from 1855 (1,027 at the start), each with first and last years and old and new numbers — effectively a dated succession table ([NRS](https://www.nrscotland.gov.uk/files//research/statutory-registers/registration-districts-of-scotland-guide.pdf)). The burgh and landward parts of one parish. |
| Ireland | Additions | The **townland** (61,119 mapped), the smallest unit in Irish records, under civil parish, barony and county ([townlands.ie](https://www.townlands.ie/en/page/download/)). Irish and English names ([Logainm, CC BY 4.0](https://www.logainm.ie/en/about/open-data)). No historical boundaries with dates. |
| Poland | Most work | Three registration regimes by partition: secular registration in the Prussian partition from 1874/75; religious registration elsewhere, mostly until 1939 ([Ciechanowski](https://doi.org/10.66847/jehl.v11i2.846)). Catholic, Lutheran and Jewish registers for one place. No open historical gazetteer found (UNVERIFIED — not searched in depth). |
| Estonia, Latvia, Lithuania | Additions | Manorial municipalities (*mõisavald*) tied to a church parish (*kihelkond*) ([KNAB](https://arhiiv.eki.ee/knab/valik/kbeehald.pdf)). Russian imperial units: gubernia, uyezd, volost. Up to eight denominations' records in one area ([Latvian archives](https://eresursi.arhivi.gov.lv/content.aspx?contentOnly=true&id=318)). German, Russian and local names. |
| France, Austria-Hungary, Czechia | Structure fits | Only the unit names differ: canton, arrondissement, Bezirkshauptmannschaft, Komitat, and Rotte and Einschicht for small places with no legal identity ([GOV types](https://genwiki39.genealogy.net/GOV/Objekttypen)). |

### US, Canada, Australia

| Country | Verdict | What it needs |
| --- | --- | --- |
| US | Additions | State is level 1 and county level 2; Louisiana parishes, Alaska boroughs and independent cities are county-equivalents ([Census](https://www.census.gov/programs-surveys/popest/guidance-geographies/terms-and-definitions.html)). A city can sit under a town, directly under a county, or across several counties, so the hierarchy is not a fixed ladder. Reservations cross state lines. The **land survey grid (PLSS)** of townships, ranges and sections is a land hierarchy, not a civil one ([BLM GLO](https://www.glorecords.blm.gov/LandPatentSearch/)). South Carolina had counties, church parishes (1706–1865) *and* judicial districts (1769–1868) at once ([Newberry](https://publications.newberry.org/ahcb/documents/US_HistCounties_Metadata2.htm)). Counties mostly change by **partial transfer**, and a new county is usually carved from parents that continue. |
| Canada | Additions | Census divisions are statistical units redrawn by census year ([StatCan 1971](https://archive.org/stream/1971927021973engfra/1971927021973engfra_djvu.txt)). In Quebec, counties moved parts of municipalities ([Érudit](https://doi.org/10.7202/1002575ar)). |
| Australia | Additions | NSW's 141 land counties and ~7,500 land parishes are a land-title hierarchy, not church parishes. South Australia and Western Australia use hundreds ([NSW LRS](https://nswlrs.com.au/assets/f/1129775276948026/c3ddc39674/searching_rg_maps_plans.pdf)). Here `Parish` means a land unit. |

## Model comparison

| Model | Types | Hierarchy | Time | Lesson for Vitni |
| --- | --- | --- | --- | --- |
| **GOV** | ~270 numbered types with multilingual labels — "unique type names were rejected" given regional and temporal differences ([types](https://genwiki39.genealogy.net/GOV/Objekttypen)) | One `part of` relation; whether it is a civil, church or court hierarchy follows from the parent's type. Plus `represents` (a church building represents a parish) and `stands in`. | Dated, sourced names, types and relations | An open type vocabulary is the proven answer. Denomination is a property. A church building can stand for its parish. |
| **Linked Places Format / World Historical Gazetteer** | URI into the Getty AAT vocabulary, plus the source's own label ([LPF](https://github.com/LinkedPasts/linked-places-format/blob/master/README.md)) | Open `relationType` URIs (`gvp:broaderPartitive`, member-of) | `when` on names, types, relations and geometries; certainty levels (certain, less certain, uncertain) | A natural **interchange format** for reference-data imports. Certainty on each assertion. |
| **Getty TGN** | Types carry abstract admin levels (first-level subdivision, second-level, inhabited place) ([rules](https://www.getty.edu/publications/vocabularies-editorial-guidelines/tgn-guidelines/3_editorial_rules/3.1/)) | Several parents allowed, one marked **preferred**; parent links dated and flagged current or historical | Dated | An abstract level on the type, separate from its local name. A preferred parent. |
| **GeoNames** | ADM1–ADM5 + historical twins, PPL…, PRSH ([codes](http://www.geonames.org/export/codes.html)) | Implied by level | Little | The level is abstract; the local name belongs to the country. |
| **Wikidata** | `instance of` | P131 (located in) with start/end qualifiers; **P1382 for partial containment** ([P131](https://www.wikidata.org/wiki/Property%3AP131)) | Qualifiers | Partial containment is a separate relation. CC0 — a free cross-walk between gazetteers. |
| **Pleiades** | — | Partitive connections | Attestations by period | Location certainty, from known to unlocated ([uncertainty](https://pleiades.stoa.org/help/uncertainty)). |
| **GEDCOM 7** | None: `PLAC` is a comma list with an optional `FORM` ([spec](https://gedcom.io/specifications/FamilySearchGEDCOMv7.html)) | Position in the list | None | Never infer a level from a list without a `FORM`. `EXID.TYPE` is a URI. |
| **GEDCOM X** | Type is a URI, with no defined set | `jurisdiction` reference | Each description is a snapshot | — |
| **Gramps v6** | 21 types + custom (State, Province, Region, Department, District, Borough, Hamlet, Locality, Street, …) | `PlaceRef(handle, date)`, several parents, the first primary | Dated refs | Vitni's planned `relation` is already richer. Round-tripping needs a mapping from the vocabulary to Gramps' 21 types. |

## Recommendations

Ordered by how much they cost if deferred past 0.11. Each one widens #692, ADR 0031.

1. **Replace the closed `PlaceType` with three fields.**
   - **`kind`**, a closed core enum: Administrative, Ecclesiastical, Judicial, Registration, Fiscal,
     Military, Manorial, Cadastral, Statistical, Settlement, Building, Natural.
   - **`level`**, optional, from the Getty and GeoNames idea: Country, Adm1…Adm5, Settlement,
     SubSettlement. So `County` stops meaning "first level".
   - **`type`**, a namespaced vocabulary key (`no:prestegjeld`, `se:harad`, `us:county`, `de:gutsbezirk`,
     `ie:townland`). Its labels ship as Fluent messages with the vocabulary, the way the surety scheme
     supplies labels (ADR 0027), so nothing renders as raw text. The type is dated, as in GOV and TGN.

   This replaces the Diocese, Deanery and District variants planned in #692. "District" means a
   prestegjeld in Norway, a US school or judicial district and a Swedish judicial district (domsaga), so
   that name was going to be wrong somewhere. Gramps and GEDCOM X types map in through the vocabulary.
   This is a breaking change to `PlaceType`. The repo keeps no backward compatibility before 1.0 except
   in backups (ADR 0041), so the backup upgrader has to map the old variants.
2. **Make the enclosure relation a closed set of hierarchy kinds**: the same list as `kind` minus
   Settlement, Building and Natural. Allow several parents of the same kind at one date, each with a
   `partial` flag (Wikidata P1382) and an optional `preferred` flag (TGN), for:
   - Swedish parishes split by a municipal border;
   - US places spanning counties;
   - reservations;
   - Danish ejerlav.

   GOV infers the hierarchy from the parent's type instead. Vitni keeps the relation explicit, because a
   Swedish socken is both a civil and a church unit.
3. **Denomination on church units.** An optional `denomination` (vocabulary key: Lutheran,
   Catholic, Reformed, Orthodox, Jewish, Mennonite, …) on places of the Ecclesiastical kind, so several
   congregations can cover one village. Norway's Church of Norway parishes set it once.
4. **Two succession kinds.**
   - `Formed`: a new place carved from parents that continue (most US counties).
   - `TerritoryTransferred`: a part moves from A to B and both continue (US counties, Finnish
     osakuntaliitos), with an optional geometry for the transferred area.

   Both are additive. Keep the boundary itself as dated geometry, as today.
5. **`ExternalId.authority` as a URI**, as GEDCOM 7 `EXID.TYPE` does, keyed with its validity date (as
   #693 already plans). Authorities worth naming up front:
   - **cross-country:** Wikidata, GeoNames, GOV, Getty TGN, WHG, FamilySearch Places;
   - **Norway:** SSB KLASS;
   - **Sweden:** SCB parish and municipality codes, Riksarkivet TORA;
   - **Denmark:** sognekode, ejerlavskode;
   - **Finland:** municipality numbers;
   - **Netherlands:** Amsterdamse code;
   - **Ireland:** Logainm;
   - **Scotland:** registration district numbers;
   - **US:** FIPS/GNIS.
6. **An "official in this language" mark on a name**, for bilingual units in Finland, Ireland and
   Wales, and for Sámi names.
7. **Certainty on geometry**: placement known, approximate, weak or unlocated (from Pleiades), next to
   the planned `PositionalAccuracy`. Name the method `derived` for boundaries computed from smaller
   units (DigDag, research P6).
8. **A reference-data import that is not Norway-specific.** The extended `staged-place` (#694) should
   express everything above, so each country is a plugin and no core change. Consider a
   Linked-Places-Format importer as the generic path: the World Historical Gazetteer publishes LPF, and a
   country plugin can produce it.
9. **Fix `PlaceType::County`'s doc comment** now. "first-level division (county, state, province)"
   is wrong for the US, and Gramps imports have separate State and Province types.

Out of scope here: Sweden's 1700–1712 calendar (a date-model gap); a church building standing for its
parish (GOV's `represents` — a later succession or relation addition, if needed); street- and
house-level places.

## Sources by country (open data)

| Country | Source | Coverage | Licence |
| --- | --- | --- | --- |
| Cross-country | [GOV](https://genwiki39.genealogy.net/GOV/Webservice), REST/SOAP, no auth | >1M objects, dated relations, Central and Eastern Europe | **CC BY-SA 4.0** — share-alike; an import would carry it, so the safe use is as an id authority |
| Cross-country | [Wikidata](https://www.wikidata.org/wiki/Wikidata:Licensing) | Ids, dated P131 | CC0 |
| Sweden | [swehist 1.1.1 (Zenodo)](https://doi.org/10.5281/zenodo.23396621) | Units 1600–1990 as GeoPackage, built from Riksarkivet NAD; only parishes and counties verified by the author ([histmaps](https://github.com/junkka/histmaps)) | CC BY 4.0 (checked 2026-10-21) |
| Sweden | [SCB REGINA](https://regina.scb.se/indelningar), parish codes and changes | 1952→ | UNVERIFIED |
| Sweden | [Rosenberg 1882–83](https://sok.riksarkivet.se/data-api/nedladdningsbara-datamangder/rosenbergs-geografiska-lexikon) | ~70,000 places with parish, härad and tingslag | Public domain mark |
| Denmark | [DigDag](https://dataforsyningen.dk/data/3961) | 1660–2007, 72 authority types, boundaries computed from ejerlav; frozen since 2012 | "Freely available"; download needs an FTP login (UNVERIFIED terms) |
| Denmark | [DAGI](https://www.eng.klimadatastyrelsen.dk/data/the-danish-administrative-geographical-division-dagi) | Current | CC BY 4.0 |
| Finland | [Statistics Finland classifications](https://stat.fi/fi/luokitukset/tupa) | Name changes 1923→, abolished municipalities 1922→ | UNVERIFIED |
| Finland | [Villages 1540–1940](https://zenodo.org/records/10408884) | Points | CC BY 4.0 |
| Iceland | [LMÍ municipality timeline](https://atlas.lmi.is/mapview/?application=markasja) | 1904→ | UNVERIFIED |
| Netherlands | [NLGis (IISG)](https://datasets.iisg.amsterdam/dataset.xhtml?fileAccess=Public&persistentId=hdl%3A10622%2FURI8O2&version=1.0), [gemeentegeschiedenis](https://www.gemeentegeschiedenis.nl/pagina/5/) | Boundaries for every year from 1812 | CC BY-SA 4.0 / CC0 (DataLegend copy) |
| UK | [GB Historical GIS via UKDS](https://datacatalogue.ukdataservice.ac.uk/studies/study/9032) | Registration districts 1851–1911 | CC BY-SA 4.0; parish boundaries commercially licensed ([Vision of Britain](https://visionofbritain.org.uk/data/)) |
| Ireland | [Logainm](https://www.logainm.ie/en/about/open-data) | Names, Irish and English | CC BY 4.0, API key |
| US | [Newberry Atlas of Historical County Boundaries](https://publications.newberry.org/ahcb/downloads/index.html) | Every county configuration, dated to the day, 1629–2000 | "Any lawful purpose, commercial or non-commercial" (checked 2026-10-21) |
| US | [NHGIS](https://www.nhgis.org/ipums-nhgis-terms-use) | County boundaries 1790→ | Registration required; no redistribution, so the user fetches it |

## Not verified

- The licences of SCB, Statistics Finland, LMÍ and Estonia's KNR.
- DigDag's exact terms.
- The licences of GNIS and FamilySearch Places.
- Polish and Baltic historical gazetteers.
- Sources for the Schleswig and Alsace sovereignty changes.
- Sámi official naming.
- Whether a Fluent-backed type vocabulary fits the `cargo xtask i18n-check` gate unchanged.
