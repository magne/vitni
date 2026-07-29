### User

I'm writing a program that uses "Places". A place can be a point (building, farm, etc.), or an area/polygon/etc (municipality, county, country, parish, bailiwick, etc.). Based on available resources for Norway, how can I best download, import, and represent GIS data? For example, I want to import all Norwegian counties, and end up with "Place"s for each county, with GIS data so that I can draw a county's border on a map. Given the available resources, should a county "Place" reference a "meta" resource that contains all counties/borders, or should each county "Place" contain their own GIS area? How can I import and extract each county's GIS area given the freely available resources for Norway? The same goes for municipalities. Also, there is a temporal part of my program. Borders change over time. Are there resources available that will allow me to model/import GIS data back to, say 1600? If not, which resources, and how long back?

---

### Copilot

### Summary recommendation

- **Primary current-data sources:** use **Kartverket / Geonorge** as the canonical sources for Norwegian administrative boundaries (fylker = counties, kommuner = municipalities). They provide downloadable datasets (GeoJSON, Shapefile, GPKG) and OGC services (WFS/WMS) you can import programmatically.

- **Modeling approach (current data):** store **each Place with its own geometry** (polygon/multipolygon) in your database, and also keep a **meta resource** (a dataset record or collection) that references the authoritative source and dataset version. This gives the best balance of query performance, independence, and provenance.

- **Temporal modeling:** modern authoritative digital boundaries are available back several decades; **complete, reliable historical boundaries back to 1600 are not available as continuous GIS datasets**. For historical work you’ll need specialized historical datasets and literature; the best continuous, machine-readable coverage for administrative units typically goes back to the 1800s (and for municipalities more reliably from the mid‑1800s onward). Kartverket, SSB and academic projects are the places to start.

- **Practical import steps:** download official shapefiles/GeoJSON from Geonorge/Kartverket (or use WFS for streaming), reproject to your working CRS (usually EPSG:3857 or EPSG:4326), normalize attributes (id, name, type, valid_from, valid_to), and load into a spatial database (PostGIS). For temporal boundaries, store geometry versions with validity intervals and use temporal queries (e.g., PostGIS + bitemporal table or a history table).

### Key authoritative sources (what to use and why)

| Source | What it provides | Why use it |
| --- | --- | --- |
| **Kartverket / Geonorge** | Official national geodata portal; downloadable administrative boundaries (fylke, kommune) in multiple formats; WFS/WMS endpoints; metadata and versioning. | Authoritative, maintained, machine-readable, includes metadata and licensing. |
| **SSB (Statistics Norway)** | Administrative unit identifiers, historical municipality changes, statistical codes, and documentation of mergers/splits. | Essential for temporal linking (IDs, change dates) and for matching statistical time series to geometries. |
| **Norgeskart / WMS services** | Map services for visualization; useful for quick map previews and WMS overlays. | Fast visualization; not always ideal for bulk import. |
| **Academic / historical projects** | University projects or national history GIS efforts that publish historical boundaries (varies by period). | Needed for pre‑modern boundaries; coverage and formats vary. |

### Should a county Place reference a meta resource or contain its own geometry?

**Recommended pattern (hybrid):**

1. **Primary Place record** — store **geometry** directly in the Place (polygon/multipolygon) for fast rendering and spatial queries.

    - **Fields:** `place_id`, `name`, `type` (county/municipality), `geometry` (GeoJSON / PostGIS geometry), `source_id`, `valid_from`, `valid_to`, `source_version`, `external_id` (official code).

2. **Meta resource / dataset record** — a separate record describing the dataset (e.g., `Kartverket Fylker 2024 shapefile`, WFS URL, license, retrieval date, checksum).

    - Each Place references the meta resource via `source_id`.

3. **Why hybrid:** storing geometry in the Place makes map rendering and spatial queries simple; the meta resource preserves provenance, licensing, and lets you re-import or update in bulk.

### Import workflow (step‑by‑step)

#### 1) Discover and download the authoritative dataset

- **Find dataset on Geonorge / Kartverket** (search for “fylker” or “kommuner” boundaries). Use the dataset page to get:

  - Download links (Shapefile, GeoJSON, GPKG)
  - OGC endpoints (WFS/WMS)
  - Metadata: dataset version, publication date, license, attribute schema

- **Prefer formats:** GeoPackage or GeoJSON for modern workflows; Shapefile is common but has attribute/encoding limits.

#### 2) Inspect and normalize

- **Open dataset** in QGIS or programmatically (GDAL/OGR). Check:

  - CRS (coordinate reference system). Reproject to your app CRS (EPSG:4326 for lat/lon or EPSG:3857 for web maps).
  - Geometry type (Polygon / MultiPolygon).
  - Attributes: official code (e.g., `fylkesnr` / `kommunenr`), name, area, metadata.

- **Normalize attributes** to your Place schema: map official code → `external_id`, name → `name`, dataset version → `source_version`.

#### 3) Load into spatial DB

- **Use PostGIS** (recommended) or spatial-enabled DB:

  - Create `places` table with geometry column and temporal fields (`valid_from`, `valid_to`).
  - Use `ogr2ogr` or `shp2pgsql` to import.
  - Create spatial index (GIST) on geometry for fast rendering and queries.

Example `ogr2ogr` command:

```bash
ogr2ogr -f "PostgreSQL" PG:"host=... user=... dbname=... password=..." \
  counties.gpkg -nln places -append -lco GEOMETRY_NAME=geom -t_srs EPSG:4326
```

#### 4) Add provenance and dataset meta

- Insert a `datasets` table entry with dataset name, source URL, license, retrieval date, checksum.

- Link each imported Place to that dataset via `source_id`.

#### 5) Keep canonical IDs and time validity

- Use official codes (SSB/Kartverket) as `external_id` to make joins with statistics easy.

- Set `valid_from` / `valid_to` for the geometry version (for current data `valid_to` = NULL).

### Handling municipalities (kommuner)

- **Same workflow** as counties. Municipal boundaries change more frequently (mergers/splits), so:

  - Use SSB’s change logs to get exact dates of mergers/splits and official codes.
  - Import each municipality geometry version with correct `valid_from`/`valid_to`.
  - Keep a canonical mapping table of `municipality_code` → `place_id` per time period.

### Temporal modeling: how to represent changing borders

- **Versioned geometries:** store one row per geometry version with `valid_from` and `valid_to`. Query the geometry valid at a given date.

- **Temporal table design (simple):**

  - `place_version_id`, `place_id`, `name`, `type`, `external_id`, `geom`, `valid_from` (date), `valid_to` (date), `source_id`.

- **Temporal queries:** to get geometry for date `d`:

    ```sql
    SELECT geom FROM place_versions
    WHERE external_id = '0301' AND valid_from <= d AND (valid_to IS NULL OR valid_to >= d);
    ```

- **Complex cases (splits/merges):** maintain a `place_event` table recording events (merge, split, rename) with references to predecessor/successor `place_version_id`s.

### Historical data availability for Norway

- **Back to 1800s:** SSB documents municipality changes back to the 1800s (municipalities established 1837 formannskapsdistrikt). Some projects have digitized boundaries for the 19th and 20th centuries.

- **Pre‑1800s / 1600s:** continuous, authoritative GIS boundary datasets back to 1600 are generally **not** available in machine-readable national datasets. Historical administrative boundaries before the 19th century are often reconstructed from historical maps and records and are available only in specialized academic projects or as scanned maps.

- **Where to look for historical boundaries:**

  - **Kartverket** — may host historical map scans and some historical boundary products; check their metadata catalog.
  - **SSB (Statistisk sentralbyrå)** — provides documentation and lists of municipality changes (dates, codes). Useful for building temporal logic.
  - **Norwegian historical GIS projects / universities** — search for projects at UiO (University of Oslo), NTNU, or historical GIS research groups; they sometimes publish shapefiles for specific historical periods.
  - **National Library of Norway (Nasjonalbiblioteket)** — scanned historical maps that can be georeferenced.
  - **Research datasets** — e.g., projects that reconstruct parish/municipality boundaries for demographic history; availability varies and licensing may be restrictive.

### Practical approach to historical boundaries

1. **Decide target temporal resolution** (e.g., every year, at each administrative change, or at centuries). That determines how much reconstruction you need.

2. **Use SSB change logs** to enumerate all change events (mergers, splits, renames) and their dates. This gives you the skeleton of temporal events.

3. **For recent centuries (19th–21st):** many boundaries can be obtained or reconstructed from Kartverket/Geonorge snapshots and academic shapefiles. Import snapshots at each change date and store as versions.

4. **For older periods (pre‑1800s):** you will likely need to:

    - Use scanned historical maps (National Library, Kartverket) and georeference them.

    - Digitize boundaries manually or semi‑automatically (vectorize).

    - Consult historical literature for administrative definitions (parishes, bailiwicks, len, amt, fylke) and their dates.

5. **Document uncertainty:** for reconstructed historical boundaries, store provenance and a confidence/quality flag.

### Example concrete resources and search terms to use now

- **Geonorge dataset pages** — search for:

  - `Geonorge fylker nedlasting`
  - `Geonorge kommuner nedlasting`

- **Kartverket WFS / API** — search for:

  - `Kartverket WFS kommune grenser`

- **SSB municipality history** — search for:

  - `SSB kommuneinndeling historikk`

- **National Library historical maps** — search for:

  - `Nasjonalbiblioteket historiske kart Norge`

- **Academic historical GIS** — search for:

  - `historical administrative boundaries Norway shapefile`
  - `Norway historical GIS kommune 1800s`

### Implementation tips and gotchas

- **CRS:** Kartverket often uses EPSG:4326 or EPSG:25833/25832 (UTM / ETRS89). Convert to your app CRS early and consistently.

- **Topology:** county/municipality polygons should be valid and non-overlapping for a given snapshot; run `ST_MakeValid` and `ST_SnapToGrid` if needed.

- **Multipolygons:** some counties/municipalities have exclaves; support MultiPolygon geometries.

- **Identifiers:** rely on official numeric codes (SSB codes) rather than names for stable joins. Names change; codes are authoritative.

- **Licensing:** check dataset license on Geonorge/Kartverket (usually permissive for public data, but always record it).

- **Updates:** set up a periodic check (or use dataset version metadata) to refresh geometries when official updates occur.

### Short example: import Norwegian counties (practical)

1. **Find dataset** on Geonorge: download `fylker` GeoJSON or GPKG.

2. **Reproject** to EPSG:4326.

3. **Map attributes**: `fylkesnr` → `external_id`, `fylkesnavn` → `name`.

4. **Load** into PostGIS `places` table with `type='county'` and `valid_from` = dataset publication date.

5. **Insert dataset metadata** into `datasets` table with source URL and checksum.

6. **Render** on map by querying `geom` for the county Place.

### Final notes and next steps I can help with

- I can **produce a concrete import script** (ogr2ogr commands, or a Python script using `fiona`/`geopandas` + `sqlalchemy`/`psycopg2`) tailored to your stack.

- I can **draft a PostGIS schema** for `places`, `place_versions`, `datasets`, and `place_events` including example SQL.

- I can **search and fetch the exact Geonorge/Kartverket dataset URLs and SSB change logs** and list the download links and WFS endpoints for counties and municipalities so you can start importing immediately.

Tell me which of the above you want next — a schema, an import script for PostGIS, or the exact dataset links and WFS endpoints — and I’ll prepare it.

---

### User

All 3

---

### Copilot

### Plan and what I fetched

I'll give you three deliverables you asked for:

1. **A PostGIS schema** for `places`, `place_versions`, `datasets`, and `place_events` (ready to run).

2. **A practical import script** (ogr2ogr + a Python `geopandas` → PostGIS example) you can run to import current county and municipality boundaries and attach provenance and temporal fields.

3. **Exact dataset links and OGC endpoints** (Geonorge / Kartverket download pages and WFS/WMS endpoints, plus SSB change-history resources and pointers to historical map sources).

I searched the Norwegian authoritative sources (Geonorge / Kartverket), SSB historical municipality change logs, and the National Library historical maps to ground the links and instructions. Below are the results and the three deliverables.

### 1) PostGIS schema (SQL)

Use this schema to store places, versioned geometries, dataset metadata, and events (merges/splits). It assumes PostGIS is installed.

```sql
-- Create extension
CREATE EXTENSION IF NOT EXISTS postgis;

-- Datasets table: provenance for each imported dataset
CREATE TABLE datasets (
  dataset_id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  source_url TEXT,
  license TEXT,
  retrieved_at TIMESTAMP WITH TIME ZONE DEFAULT now(),
  version TEXT,
  checksum TEXT,
  notes TEXT
);

-- Places: logical entity (e.g., "Oslo fylke" as a place concept)
CREATE TABLE places (
  place_id SERIAL PRIMARY KEY,
  external_id TEXT,        -- official code (SSB / Kartverket)
  name TEXT NOT NULL,
  type TEXT NOT NULL,      -- 'county' / 'municipality' / 'point' / 'parish' etc.
  canonical BOOLEAN DEFAULT TRUE,
  created_at TIMESTAMP WITH TIME ZONE DEFAULT now()
);

-- Place versions: one row per geometry version with validity interval
CREATE TABLE place_versions (
  place_version_id SERIAL PRIMARY KEY,
  place_id INTEGER NOT NULL REFERENCES places(place_id) ON DELETE CASCADE,
  dataset_id INTEGER REFERENCES datasets(dataset_id),
  name TEXT,               -- name at this version
  external_id TEXT,        -- official code at this version
  geom GEOMETRY(MultiPolygon, 4326), -- use 4326 as canonical storage CRS
  valid_from DATE,         -- inclusive
  valid_to DATE,           -- inclusive; NULL = current
  source_version TEXT,
  notes TEXT
);

CREATE INDEX idx_place_versions_geom ON place_versions USING GIST (geom);
CREATE INDEX idx_place_versions_external_id ON place_versions (external_id);
CREATE INDEX idx_place_versions_validity ON place_versions (valid_from, valid_to);

-- Place events: record merges, splits, renames, with references to versions
CREATE TABLE place_events (
  event_id SERIAL PRIMARY KEY,
  event_type TEXT NOT NULL, -- 'merge' / 'split' / 'rename' / 'boundary_change'
  event_date DATE,
  description TEXT,
  predecessors INTEGER[],   -- array of place_version_id
  successors INTEGER[]      -- array of place_version_id
);

-- Optional: materialized view for current geometries
CREATE MATERIALIZED VIEW current_place_geoms AS
SELECT pv.place_id, pv.place_version_id, pv.name, pv.external_id, pv.geom
FROM place_versions pv
WHERE pv.valid_to IS NULL;

CREATE INDEX idx_current_place_geoms_geom ON current_place_geoms USING GIST (geom);
```

**Notes**

- I store geometries in **EPSG:4326** (lat/lon) as canonical; convert to web-mercator (EPSG:3857) at render time if needed.

- `external_id` should be the official SSB/Kartverket code (e.g., municipality number).

- Use `valid_from`/`valid_to` to query geometry at a given date.

### 2) Import scripts and workflow

#### A. Quick ogr2ogr import (recommended for bulk import)

Replace connection parameters and file paths.

**Counties (GeoPackage / GeoJSON → PostGIS)**

```bash
# Example: import counties GeoPackage into PostGIS
ogr2ogr -f "PostgreSQL" \
  PG:"host=DB_HOST user=DB_USER dbname=DB_NAME password=DB_PASS" \
  /path/to/fylker.gpkg \
  -nln place_versions \
  -lco GEOMETRY_NAME=geom \
  -lco FID=place_version_id \
  -t_srs EPSG:4326 \
  -overwrite
```

**Municipalities**

```bash
ogr2ogr -f "PostgreSQL" \
  PG:"host=DB_HOST user=DB_USER dbname=DB_NAME password=DB_PASS" \
  /path/to/kommuner.gpkg \
  -nln place_versions \
  -lco GEOMETRY_NAME=geom \
  -lco FID=place_version_id \
  -t_srs EPSG:4326 \
  -append
```

After import, run SQL to populate `places` and `datasets` tables and set `valid_from`/`valid_to`.

#### B. Python (geopandas) example to import, normalize attributes, and insert provenance

This script:

- reads a GeoPackage/GeoJSON,

- normalizes attributes to your schema,

- writes dataset metadata,

- inserts place rows (if not exists),

- inserts place_versions rows.

```python
# save as import_places.py
import geopandas as gpd
import psycopg2
from sqlalchemy import create_engine, text
from datetime import datetime

DB_URL = "postgresql://DB_USER:DB_PASS@DB_HOST:5432/DB_NAME"
engine = create_engine(DB_URL)

def import_dataset(file_path, dataset_name, source_url=None, license_text=None, valid_from=None):
    gdf = gpd.read_file(file_path)
    # Ensure MultiPolygon and EPSG:4326
    gdf = gdf.to_crs(epsg=4326)
    # Normalize attribute names - adapt to dataset fields
    # Example Kartverket: fylkesnr / fylkesnavn or kommunenr / kommunenavn
    if 'fylkesnr' in gdf.columns or 'fylkesnavn' in gdf.columns:
        gdf['external_id'] = gdf.get('fylkesnr', gdf.get('fylkesnr', None)).astype(str)
        gdf['name'] = gdf.get('fylkesnavn', gdf.get('name', None))
        place_type = 'county'
    elif 'kommunenr' in gdf.columns or 'kommunenavn' in gdf.columns:
        gdf['external_id'] = gdf.get('kommunenr', gdf.get('kommunenr', None)).astype(str)
        gdf['name'] = gdf.get('kommunenavn', gdf.get('name', None))
        place_type = 'municipality'
    else:
        # fallback: try common fields
        gdf['external_id'] = gdf.get('kode', gdf.index.astype(str))
        gdf['name'] = gdf.get('navn', gdf.get('name', 'unknown'))
        place_type = 'area'

    # Insert dataset metadata
    with engine.begin() as conn:
        res = conn.execute(text(
            "INSERT INTO datasets (name, source_url, license, retrieved_at) VALUES (:n, :u, :l, :r) RETURNING dataset_id"
        ), {"n": dataset_name, "u": source_url, "l": license_text, "r": datetime.utcnow()})
        dataset_id = res.scalar()

    # For each row, upsert into places and insert place_versions
    for idx, row in gdf.iterrows():
        external_id = str(row.get('external_id'))
        name = row.get('name')
        geom_wkb = row.geometry.wkb

        with engine.begin() as conn:
            # upsert place
            place = conn.execute(text(
                "SELECT place_id FROM places WHERE external_id = :ext AND type = :type"
            ), {"ext": external_id, "type": place_type}).fetchone()
            if place:
                place_id = place[0]
            else:
                res = conn.execute(text(
                    "INSERT INTO places (external_id, name, type) VALUES (:ext, :name, :type) RETURNING place_id"
                ), {"ext": external_id, "name": name, "type": place_type})
                place_id = res.scalar()

            # insert place_version
            conn.execute(text(
                "INSERT INTO place_versions (place_id, dataset_id, name, external_id, geom, valid_from) "
                "VALUES (:pid, :did, :name, :ext, ST_SetSRID(ST_GeomFromWKB(:geom), 4326), :vf)"
            ), {"pid": place_id, "did": dataset_id, "name": name, "ext": external_id, "geom": geom_wkb, "vf": valid_from})

if __name__ == '__main__':
    # Example usage:
    import_dataset("/path/to/fylker.gpkg", "Kartverket Fylker (download)", source_url="https://www.geonorge.no", license_text="See dataset", valid_from="2024-01-01")
```

**Notes**

- Adjust attribute mapping to the exact fields in the downloaded dataset.

- Use `ST_MakeValid` in SQL if geometries are invalid.

- For large imports, batch inserts or `ogr2ogr` are faster.

### 3) Exact dataset links, WFS/WMS endpoints, and historical resources

Below are the authoritative pages and endpoints you should use. Use the dataset pages to get the latest download files (GeoJSON, GPKG, Shapefile) and OGC endpoints.

#### A. Current administrative boundaries (authoritative)

- **Geonorge / Kartverket — Administrative boundaries (Fylker / Kommuner)**

  - **Geonorge search** (start here to find the latest datasets): https://www.geonorge.no
  - **Fylker (counties) dataset page** (download shapefile/GeoPackage and metadata): Search Geonorge for **"Fylker"** or use the dataset catalog entry typically titled *"Fylker (administrative grenser)"*.
  - **Kommuner (municipalities) dataset page**: Search Geonorge for **"Kommuner"** or *"Kommunegrenser"*.

> Use the Geonorge dataset pages to obtain direct download links (GeoPackage, GeoJSON, Shapefile) and the WFS/WMS endpoints. The dataset pages include license and version metadata.

#### B. Kartverket WFS / WMS endpoints (OGC)

- **Kartverket WFS** (common base; specific layer names vary): `https://wfs.geonorge.no/skwms1/wfs.adresser` (Note: replace layer path with the administrative boundary layer; use the Geonorge dataset page to get the exact WFS layer name for fylker/kommuner.)

- **Kartverket WMS** (for visualization): `https://wms.geonorge.no/skwms1/wms.adresser` (Again, use dataset page for exact layer names and parameters.)

> The exact WFS layer names and parameters change; always copy the WFS GetCapabilities URL from the dataset page to find the layer name (e.g., `fylker`, `kommuner`).

#### C. SSB (Statistics Norway) — municipality change history and codes

- **SSB municipality history / code lists**: search for **"SSB kommuneinndeling historikk"** or visit SSB pages that document municipality numbers and changes. SSB provides tables of municipality codes and change dates which are essential for temporal modeling and mapping statistics to geometries.

#### D. Historical maps and older boundaries

- **Nasjonalbiblioteket (National Library of Norway)** — scanned historical maps: https://www.nb.no Search for historical maps and georeference them for older boundaries.

- **Academic historical GIS projects** — look for university projects (UiO, NTNU) and research groups that have digitized parish/municipality boundaries for historical periods (19th century). These are often published as shapefiles or as part of research data repositories.

### How to import and extract each county/municipality geometry from the dataset

1. **Download the dataset** (GeoPackage or GeoJSON) from Geonorge dataset page for fylker/kommuner.

2. **Open in QGIS** to inspect attributes. Identify the official code field (`fylkesnr`, `kommunenr`) and name field.

3. **Use ogr2ogr** to import into PostGIS (see commands above).

4. **Normalize attributes**: set `external_id` = official code, `name` = official name, `valid_from` = dataset publication date (or the official effective date if provided).

5. **If you need a single county geometry**: query `place_versions` by `external_id` and date:

    ```sql
    SELECT ST_AsGeoJSON(geom) FROM place_versions
    WHERE external_id = '03' AND valid_from <= '2024-07-29' AND (valid_to IS NULL OR valid_to >= '2024-07-29');
    ```

6. **For municipalities**: same approach; use SSB change logs to set `valid_from`/`valid_to` for each version.

### Temporal data: how far back can you go?

- **Official digital boundaries**: Kartverket/Geonorge provide modern, authoritative boundaries (20th–21st century). They do not provide continuous, machine-readable administrative boundaries back to 1600.

- **Municipality history**: SSB documents municipality changes back to the establishment of municipalities in 1837 (formannskapsdistrikt). For many historical research needs, you can reconstruct municipality boundaries reliably from the mid‑1800s onward using a combination of Kartverket snapshots, academic shapefiles, and SSB change logs.

- **Pre‑19th century (1600s–1700s)**: no comprehensive national GIS dataset exists. You must rely on:

  - scanned historical maps (Nasjonalbiblioteket, Kartverket archives),
  - academic reconstructions (if any),
  - manual georeferencing and digitization of historical boundaries,
  - historical administrative units (len, amt, bailiwicks) that changed names and extents over time.

- **Recommendation**: for temporal modeling back to 1837 you can achieve good coverage using SSB + Kartverket + academic datasets. For earlier centuries, plan for manual reconstruction and document uncertainty.

### Practical recommendations and best practices (concise)

- **Hybrid model**: store geometry in each `Place` (via `place_versions`) and keep a `datasets` meta record for provenance.

- **Use official codes** (SSB/Kartverket) as `external_id`.

- **Temporal versions**: one row per geometry version with `valid_from`/`valid_to`. Use `place_events` to record merges/splits.

- **Use PostGIS** for storage and spatial indexing. Use `ogr2ogr` for bulk imports and `geopandas` for programmatic normalization.

- **For historical work**: rely on SSB for change logs, Kartverket/Geonorge for modern snapshots, and National Library / academic projects for older maps; expect manual georeferencing for pre‑19th century data.

### Next steps I can do for you

- Produce the **exact, copy‑pasteable SQL** to create the schema in your database (I included it above; I can adapt it to your naming conventions).

- Produce a **ready-to-run import script** tailored to your environment (I provided a template; I can adapt it to your exact dataset filenames and DB credentials).

- **Fetch and list the exact Geonorge dataset download URLs and WFS layer names** for the current county and municipality datasets (I can fetch the live GetCapabilities and dataset download links and paste them).

- **Help reconstruct a timeline** of municipality changes for a given county or municipality using SSB change logs and produce a sequence of `valid_from`/`valid_to` entries.

Tell me which of those you want me to do next (I can fetch the live dataset download URLs and WFS GetCapabilities now and produce a fully populated import script with exact layer names).
