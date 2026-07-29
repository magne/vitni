### User

I'm writing a program that uses "Places". A place can be a point (building, farm, etc.), or an area/polygon/etc (municipality, county, country, parish, bailiwick, etc.). Based on available resources for Norway, how can I best download, import, and represent GIS data? For example, I want to import all Norwegian counties, and end up with "Place"s for each county, with GIS data so that I can draw a county's border on a map. Given the available resources, should a county "Place" reference a "meta" resource that contains all counties/borders, or should each county "Place" contain their own GIS area? How can I import and extract each county's GIS area given the freely available resources for Norway? The same goes for municipalities. Also, there is a temporal part of my program. Borders change over time. Are there resources available that will allow me to model/import GIS data back to, say 1600? If not, which resources, and how long back?

---

### Claude

### Resources (all free, NLOD/CC-BY)

**Kartverket via Geonorge** is your primary source for everything current.

- *Administrative enheter – fylker* and *Administrative enheter – kommuner* — the authoritative polygons. Published as a new version each 1 January (and after each reform). Available as SOSI, GML, FGDB, GeoPackage, and sometimes GeoJSON/PostGIS dump; SRS EPSG:25833 (EUREF89 UTM33) or EPSG:4258.
- *Administrative enheter – historiske versjoner* — Kartverket keeps prior yearly snapshots. Verify how far back the catalog actually goes; in my experience it's roughly the reform era (~2017/2018 onward) with decent coverage, not the 19th century.
- **N-series generalisations** (N50, N250, N500, N1000, N2000, N5000 Kartdata) each carry admin boundaries at the matching scale. Use these — full-resolution Norwegian coastline is brutal (a county is a MultiPolygon with thousands of rings, and Norway has ~240k islands).
- *Sentralt stedsnavnregister (SSR)* — point geometry for farms, settlements, natural features, including historical and alternate name forms. REST API: `ws.geonorge.no/stedsnavn/v1/`. This is your farm/building layer.
- *Matrikkelen – Eiendomskart Teig* — cadastral parcel polygons keyed by gnr/bnr. Because gårdsnummer are largely continuous back to the 1886 (and traceable to the 1838) matrikkel, this is the best bridge between modern GIS and 19th-century sources.
- *Historiske kart* — georeferenced scans: amtskart (1826–1917), rektangelmålinger (1770s–1890s), gradteigskart. Raster, not vector. These are what you digitise from.

**SSB KLASS** (`data.ssb.no/api/klass/v1/`) is the authority for codes, names, and *validity dates*: `131` kommuneinndeling, `104` fylkesinndeling, plus **correspondence tables** between versions telling you which unit became which. No geometry, but it's the temporal skeleton you hang geometry on. Coverage is roughly mid-20th century forward; for 1837–1974 the canonical source is SSB's *Historisk oversikt over endringer i kommune- og fylkesinndelingen* (text, not data).

**Secondary/fallback:** OSM (admin_level 4 = fylke, 7 = kommune; current only), Wikidata (good for QIDs and start/end dates, weak polygons), Eurostat NUTS (fylke ≈ NUTS-3, versioned 2003/2006/…/2024, heavily generalised), GADM (check the licence — not open for commercial use).

### Representation: geometry belongs to the Place

Don't have the county Place point at a shared "all counties" resource for its geometry. Copy the geometry into your own store, normalised per place, and keep provenance separately. Reasons: you need per-place temporal validity (counties change independently), you need offline rendering of one county without loading a national file, and external datasets mutate under you so reproducibility requires a snapshot.

Keep a `dataset` table for the "meta" part — that's where the shared-resource identity lives, as provenance rather than as storage.

sql

```sql
place(id, kind, canonical_name)                    -- country|county|municipality|parish|farm|building
place_name(place_id, name, name_type, lang, valid_from, valid_to, dataset_id)
place_geom(id, place_id, geom, generalization, valid_from, valid_to,
           method, accuracy_m, dataset_id, source_feature_id)
place_link(place_id, scheme, value, valid_from, valid_to)  -- ssb_kommunenr | kartverket_lokalid | wikidata | ssr_stedsnummer
place_parent(child_id, parent_id, relation, valid_from, valid_to)
place_succession(pred_id, succ_id, kind, at_date)  -- merge|split|rename|transfer
dataset(id, title, publisher, version_date, licence, url, checksum, downloaded_at)
```

Four things that will bite you if you don't design for them now:

1. **`place_geom` is 0..n per place**, keyed on both a validity interval *and* a generalisation level. Same county, five geometries: N50 for 2024, N50 for 1972, N5000 for both, etc. Use `daterange` with `[)` semantics; date granularity suffices since Norwegian changes land on 1 January.
2. **Parallel hierarchies.** A farm sits in a kommune *and* a sogn *and* a prestegjeld *and* a tinglag/fogderi — and these do not nest consistently. Hence `relation` on `place_parent` (civil / ecclesiastical / judicial). For genealogy the ecclesiastical hierarchy is the one your sources actually use.
3. **Never derive hierarchy from geometry.** Administrative parentage is a legal fact recorded in sources; containment tests will disagree with it, and most historical places will have no polygon at all.
4. **Identity vs. succession.** Is 1838 Kristiansand the same Place as post-2020 Kristiansand? Pick a rule (same Place + new name/geom version when legal continuity holds; new Place when a new legal entity is constituted) and record the rest in `place_succession`. SSB's correspondence tables map directly onto this.

Store canonical geometry in EPSG:4326 for portability and add a generated 25833 column for metric operations, or the reverse — just pick one canonical and be strict.

### Import

Request **GML or GeoPackage**, not SOSI. GDAL's SOSI driver needs FYBA and is usually not compiled in.

bash

```bash
ogr2ogr -f PostgreSQL PG:"dbname=places" fylker.gml \
  -nln src_fylke -nlt PROMOTE_TO_MULTI \
  -t_srs EPSG:4326 -lco GEOMETRY_NAME=geom -lco FID=fid \
  --config PG_USE_COPY YES
```

Then a SQL/Rust pass maps `src_fylke` → `place` + `place_geom` + `place_link`, stamping `valid_from` from the dataset's version date and closing the previous row's `valid_to`. Keep the raw `src_*` tables; re-derivation is cheap, re-downloading a withdrawn version isn't.

Geonorge has a download REST API (`nedlasting.geonorge.no/api/`) so the whole fetch can be scripted and checksummed into `dataset`.

For a desktop app, **GeoPackage/SpatiaLite** is the pragmatic target — `rusqlite` + `mod_spatialite`, or the `gdal` crate for I/O plus `geo`/`geozero`/`geojson` for in-process work and `wkb` for storage. If you want PostGIS, `sqlx` with `geozero`'s postgis feature.

### How far back you can actually go

- **~2018 → present:** real vector snapshots from Kartverket, per year. Straightforward import.
- **1838 → ~2018:** 1838 (formannskapsdistriktene) is the floor for municipalities as entities. Names, codes and change events are well documented via SSB/KLASS and the historical overview. Polygons are *not* systematically published as open vector data. You have three options: check Sikt/NSD's Kommunedatabase-derived boundary files, look for community GitHub datasets of historical kommunegrenser, or digitise from the amtskart series. Expect to do real work here.
- **1662–1919 (amt):** amt largely prefigure the 1919 fylker, so you can approximate by unioning constituent herreder/prestegjeld. Fogderi likewise.
- **Pre-1662 (len):** for 1600 you're dealing with a handful of hovedlen — Akershus, Bergenhus, Trondheim, Stavanger, Båhus — plus fogderi beneath them. No vector data exists. And genuinely: some borders *had no defined line*. The Norway–Sweden border wasn't fixed until 1751, the Russian border not until 1826 (Pasvik/fellesdistriktet), and Båhuslen, Jämtland and Härjedalen left Norway in 1645/1658.

That last point is the design consequence: for the 17th century a polygon is often factually wrong, not merely imprecise. Support geometry that is a point, a centroid with radius, a convex hull, or absent — with `method` ∈ {surveyed, derived, digitised, approximated} and `accuracy_m` — and let your renderer degrade gracefully.

The technique that actually works for pre-1838 polygons: build **bottom-up from farms**. SSR gives you farm points; the 1838/1886 matrikkel and church books tell you which sogn, prestegjeld, skipreide and tinglag each farm belonged to; take the union of buffered points or a Voronoi partition clipped to coastline. The result is honest-looking fuzzy extents rather than false-precision boundaries, and it doubles as your source-to-place linkage for the genealogical data itself.

One caveat worth stating plainly: exact Geonorge dataset titles and the historical coverage of the "historiske versjoner" product shift over time, and my information there may be stale. Check Kartkatalogen before you commit to a specific product name in code.
