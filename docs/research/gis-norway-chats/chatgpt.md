### User

I'm writing a program that uses "Places". A place can be a point (building, farm, etc.), or an area/polygon/etc (municipality, county, country, etc.). Based on available resources for Norway, how can I best download, import, and represent GIS data? For example, I want to import all Norwegian counties, and end up with "Place"s for each county, with GIS data so that I can draw a county's border on a map. Given the available resources, should a county "Place" reference a "meta" resource that contains all counties/borders, or should each county "Place" contain their own GIS area? How can I import and extract each county's GIS area given the freely available resources for Norway? The same goes for municipalities. Also, there is a temporal part of my program. Borders change over time. Are there resources available that will allow me to model/import GIS data back to, say 1600? If not, which resources, and how long back?

---

### ChatGPT

If you're designing a general-purpose **Place** model rather than a GIS application, I would separate **identity** from **geometry**. In other words:

```plain
Place
 ├── id
 ├── type (county, municipality, farm, church, mountain...)
 ├── names
 ├── validity interval
 ├── parent place(s)
 └── geometries[]  <-- references

Geometry
 ├── id
 ├── place_id
 ├── valid_from
 ├── valid_to
 ├── source
 ├── CRS
 ├── geometry (MultiPolygon, Polygon, Point...)
 └── simplification_level
```

rather than embedding geometry directly inside the Place.

This turns out to match how almost every national mapping agency models their data.

---

# Norwegian sources

For Norway, there are essentially four authoritative sources.

| Dataset | Geometry | Temporal | Authority | Best use |
| --- | --- | --- | --- | --- |
| Kartverket Administrative boundaries | ✓ | current | Kartverket | counties, municipalities |
| Matrikkelen | points + polygons | limited | Kartverket | farms, properties, addresses |
| SSR (Central Place Name Register) | mostly points | historical names | Kartverket | names |
| SSB historical municipality tables | no geometry | historical | Statistics Norway | temporal relationships |

The important observation is:

**Current geometry is excellent. Historical geometry is much weaker.**

---

# Current county and municipality borders

Kartverket publishes administrative boundaries as open data.

You can download

- GeoPackage
- Shapefile
- GML

GeoPackage is by far the nicest format.

Inside you'll typically get something like

```text
AdministrativeBoundary.gpkg

kommuner
fylker
land
territorial waters
...
```

Each row contains

```text
kommunenummer
kommunenavn
geometry (MultiPolygon)
```

or

```text
fylkesnummer
fylkesnavn
geometry
```

This is exactly what you want.

---

# Should each Place contain geometry?

I would **not**.

Instead:

```text
Place
-------
id = kommune:5501
name = Tromsø
kind = municipality

Geometry
--------
place = kommune:5501
source = Kartverket
valid_from = 2024-01-01
geometry = MULTIPOLYGON(...)
```

Advantages

- geometry can change
- multiple resolutions
- multiple CRS
- multiple sources
- easier caching

For example:

```text
Geometry
---------
id=1
Place=Tromsø
resolution=high

Geometry
---------
id=2
Place=Tromsø
resolution=medium

Geometry
---------
id=3
Place=Tromsø
resolution=low
```

Leaflet/OpenLayers can use the low-res version while editing.

---

# Should all counties reference one shared geometry resource?

I would not model it that way.

Instead, import the dataset once into your own Geometry table.

Think of the downloaded GeoPackage as merely an **import source**, not part of your runtime model.

During import:

```text
GeoPackage

row 1
row 2
row 3

↓

Geometry table

geometry_id=...
place_id=...
polygon...
```

After import you never need the GeoPackage again.

---

# Import process

GeoPackage is SQLite.

That means you can simply use SQLite libraries.

```sql
SELECT
    kommunenummer,
    kommunenavn,
    geom
FROM kommune;
```

The geometry column is binary (GeoPackage geometry blob).

Libraries exist for

- Rust (geo, gdal, geozero)
- Go (orb, go-geom, GDAL bindings)
- Python (geopandas)
- Java
- C#

If you're writing Rust, I'd recommend

- GDAL crate for import
- geo-types internally

---

# Internal representation

I'd avoid inventing your own GIS format.

Internally use

```rust
geo_types::Geometry<f64>

or

geo_types::MultiPolygon<f64>
```

You can serialize as

- WKB
- EWKB
- PostGIS
- FlatGeobuf

WKB is an excellent storage format.

---

# CRS

Norwegian datasets are often supplied as

```text
EPSG:25833
```

(ETRS89 / UTM 33N)

I'd convert immediately to

```text
EPSG:4326
```

(latitude/longitude)

unless you need centimeter accuracy.

Most web maps expect 4326 or 3857.

---

# Temporal data

This is the difficult part.

Norway has changed municipalities many times.

Example:

```text
Troms county

1866
↓

1920

↓

1972

↓

2020 merger

↓

2024 split
```

The geometry changed each time.

Kartverket generally publishes only **current** boundaries.

---

# Historical municipalities

Statistics Norway (SSB) publishes

- municipality history
- municipality mergers
- municipality splits
- validity dates

For example

```text
1902

Municipality A

↓

1964

merged into B

↓

2020

merged into C
```

But not always geometry.

---

# Historical geometry

Historical geometry exists, but not as a single official open dataset back to 1600.

Available sources include:

- Historical administrative maps from Kartverket (selected periods)
- Historical municipality datasets from SSB (mostly tables)
- National Library scanned maps
- OpenStreetMap history (recent decades only)
- Cadastre history (limited)
- Norgeskart historical map layers (raster)

---

# How far back?

Approximately

| Time | Geometry availability |
| --- | --- |
| Today | Excellent |
| 2024 | Excellent |
| 2020 | Excellent |
| 2010 | Very good |
| 2000 | Good |
| 1980 | Partial |
| 1960 | Sparse |
| Before 1900 | Mostly scanned maps |
| 1600 | No complete GIS polygons |

So a fully GIS-based temporal model to 1600 is unfortunately not realistic from freely available authoritative datasets.

---

# What I'd build

I'd model borders as temporal geometry.

```text
Place
------
id=municipality:5501

Geometry
--------
valid_from=2024
valid_to=NULL

Geometry
--------
valid_from=2020
valid_to=2023

Geometry
--------
valid_from=1964
valid_to=2019
```

The Place identity remains constant within whatever identity scheme you define, while the geometry changes over time. Note that official municipality codes themselves may also change due to mergers and splits, so if you're modeling historical entities faithfully, you may prefer to treat each legally distinct municipality as its own Place and link them with predecessor/successor relationships rather than assuming a single enduring identity.

---

# For genealogy software

Since you mentioned you're building genealogy software, I'd recommend going one step further.

Represent administrative entities as a graph:

```text
Place
 ├── Tromsø Municipality (2024-)
 ├── Tromsø Municipality (2020–2023)
 ├── Tromsø Municipality (1861–2019)
```

linked by

```text
replaced_by
split_into
merged_into
parent_county
parent_country
```

Each node then has:

- names
- aliases
- language variants
- geometry
- validity interval
- identifiers (municipality number, Wikidata, GeoNames, etc.)

This approach naturally handles border changes, renamings, and administrative reorganizations without forcing a single identity through fundamentally different historical entities. It's also extensible to parishes, judicial districts, farms, and other place types that are important in genealogical records.

For a genealogy-focused application, I would aim for this pipeline:

1. **Current administrative boundaries:** import authoritative GeoPackage data from Kartverket into your own `Geometry` table.
2. **Historical administrative relationships:** import SSB's municipality history tables to build predecessor/successor and validity relationships.
3. **Historical geometries:** add snapshots when authoritative polygon datasets are available (recent decades first), leaving older periods without polygons where necessary.
4. **Storage:** keep geometries as `MultiPolygon` in WKB (or PostGIS if using a spatial database), referenced from `Place` records rather than embedded directly.

This architecture scales well from modern municipalities and counties to farms, parishes, and historical administrative units while accommodating the incomplete nature of historical GIS data.
