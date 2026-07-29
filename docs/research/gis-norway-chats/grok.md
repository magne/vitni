### User

I'm writing a program that uses "Places". A place can be a point (building, farm, etc.), or an area/polygon/etc (municipality, county, country, parish, bailiwick, etc.). Based on available resources for Norway, how can I best download, import, and represent GIS data? For example, I want to import all Norwegian counties, and end up with "Place"s for each county, with GIS data so that I can draw a county's border on a map. Given the available resources, should a county "Place" reference a "meta" resource that contains all counties/borders, or should each county "Place" contain their own GIS area? How can I import and extract each county's GIS area given the freely available resources for Norway? The same goes for municipalities. Also, there is a temporal part of my program. Borders change over time. Are there resources available that will allow me to model/import GIS data back to, say 1600? If not, which resources, and how long back?

I'm writing a program that uses "Places". A place can be a point (building, farm, etc.), or an area/polygon/etc (municipality, county, country, parish, bailiwick, etc.). Based on available resources for Norway, how can I best download, import, and represent GIS data? For example, I want to import all Norwegian counties, and end up with "Place"s for each county, with GIS data so that I can draw a county's border on a map. Given the available resources, should a county "Place" reference a "meta" resource that contains all counties/borders, or should each county "Place" contain their own GIS area? How can I import and extract each county's GIS area given the freely available resources for Norway? The same goes for municipalities. Also, there is a temporal part of my program. Borders change over time. Are there resources available that will allow me to model/import GIS data back to, say 1600? If not, which resources, and how long back?

---

### Grok

**Best primary source: Kartverket (Norwegian Mapping Authority) via Geonorge.** These are the official, most accurate administrative boundaries (derived from the matrikkel/property register + official national/territorial borders). They are free under CC BY 4.0, updated regularly (reference date typically 1 January of the current year), and available for both current and historical versions.

### Download and access

- **Portal**: [geonorge.no](https://www.geonorge.no) / [kartkatalog.geonorge.no](https://kartkatalog.geonorge.no). Search for “Administrative enheter kommuner”, “Administrative enheter fylker”, or the series “Administrative enheter – historiske versjoner”.
- **Formats**: FGDB (File Geodatabase), GeoJSON, GML, PostGIS, SOSI. Also available as national, county-level, or municipality-level files.
- **APIs/services**:
  - WFS: https://wfs.geonorge.no/skwms1/wfs.administrative_enheter
  - WMS (current and historical)
  - REST API for administrative units (lookups by code/name): e.g. via ws.geonorge.no/kommuneinfo
  - Atom feeds and the Geonorge download API for programmatic bulk access.
- **Other options**: OpenStreetMap-derived boundaries (e.g. via geoBoundaries or commercial packs) exist but are less authoritative and accurate for official borders. SSB provides complementary statistical/region data and some improved historical municipality boundaries (1986–2019).

Download the national (landsdekkende) GeoJSON/FGDB/GML for municipalities or counties, then process with tools such as GDAL/OGR, GeoPandas (Python), QGIS, PostGIS, or your preferred GIS library.

### Representing “Place”s in your program

**Prefer each Place (county or municipality) owning/storing its own geometry**, with attributes (official number/code, Norwegian/Sámi/Kven names, valid-from/to dates, etc.).

Reasons:

- Independent querying, drawing, spatial joins, and versioning per entity.
- Natural fit for temporal modelling (one Place can have multiple geometry versions over time).
- Avoids loading an entire national dataset just to draw one border.
- You can still keep a shared “meta” catalog or index of all Places (codes, names, hierarchy, parent links) for discovery and consistency.

A hybrid is also fine: a central geometry store/table keyed by official ID + validity period, referenced by your Place objects. Store geometries in a spatial database (PostGIS recommended) or as GeoJSON/WKT blobs with spatial indexes. Use the official municipality/county numbers (kommunenummer/fylkesnummer) as stable keys where possible; note that these change with reforms.

For points (buildings, farms, etc.) use other Kartverket data (addresses, matrikkel points, SSR place names) or OSM.

### Import / extraction workflow (counties & municipalities)

1. Download the current national municipalities (or counties) file in GeoJSON/FGDB/GML.
2. Parse features. Each feature has attributes such as official number, names, and a polygon/multipolygon geometry (often including maritime parts out to territorial waters; clip if you only want land).
3. Create one Place per feature, attach the geometry (and validity period if historical), and store hierarchy (municipality → county → country).
4. For updates, re-download and diff by official code + geometry hash or validity dates.
5. Same process for historical snapshots (see below).

Libraries: GeoPandas + Fiona/shapely (Python), GDAL, or PostGIS ogr2ogr import. Reproject as needed (source is typically EUREF89 / UTM zones or geographic).

### Temporal / historical data

Official digital boundary datasets from Kartverket go back to the late 1990s (individual yearly files from ~1997 onward are listed under the historical series; earlier digitization started in the 1980s from paper maps).

- **Kartverket historical series** (“Administrative enheter – historiske versjoner” and yearly snapshots): multiple years of fylke/kommune polygons. Available in the same formats. Use for any date from the late 1990s to present. There is also a historical WMS.
- **SSB improved historical municipality boundaries**: 1986–2019 (higher accuracy reconstruction based on later measured borders + change registers). Available via Geonorge/SSB (contact or download links on SSB geodata pages).
- **Pre-1980s / back to ~1600**: No complete, ready-to-use vector GIS polygons of administrative borders exist at national scale.
  - Municipalities (kommuner) were introduced with the Formannskapslovene of 1837 (initially based on prestegjeld/parishes). Number and borders changed extensively (peak ~747 in 1930, major consolidations later).
  - Counties/amts/fylker have longer history with many revisions (e.g. 1671 stiftamt structure, 1918/1919 renaming to fylker, major 2018–2020 and later reforms).
  - For earlier periods you must reconstruct from:
    - Scanned historical maps (Kartverket’s free historical map archive – amtskart, rektangelkart, etc., from the 1600s/1700s onward; many downloadable as images/GeoTIFFs). Digitize borders yourself.
    - Textual change registers (SSB’s “Alle endringer i de regionale inndelingene”, Klass classifications, historical laws).
    - Research literature, parish records, or specialized historical GIS projects (none provide nationwide seamless vectors to 1600).
  - Practical limit for reasonably accurate nationwide vectors is roughly the 1980s–1990s onward; earlier requires significant manual work or accepting schematic/generalized representations.

For your temporal model: give each Place geometry one or more validity intervals (valid_from, valid_to). Store successive geometries for the same logical Place (or treat major mergers/splits as new Places linked by succession relations). Official codes change, so maintain a mapping/history table. SSB and Kartverket change lists help track merges, splits, and renamings.

### Practical recommendations

- Start with current national GeoJSON/FGDB from Geonorge for municipalities and counties.
- Import into PostGIS (or equivalent) for spatial queries, simplification (for map drawing), and temporal versioning.
- Keep geometries at full resolution for analysis; simplify (Douglas-Peucker or similar) for web map display.
- Attribute everything with official codes, multilingual names, and source/version date.
- For points vs areas: use a type discriminator on Place and optional geometry (Point or Polygon/MultiPolygon).
- Monitor Geonorge for updates (Atom feeds or the download API) and reform years (e.g. 2020, 2024 adjustments).

This gives you authoritative, free, high-quality data for modern and recent historical borders, with a clear path for older periods via map digitization if needed. Check the exact metadata pages on Geonorge for the latest download links, CRS details, and product specifications.
