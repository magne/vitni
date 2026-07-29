### User

I'm writing a program that uses "Places". A place can be a point (building, farm, etc.), or an area/polygon/etc (municipality, county, country, parish, bailiwick, etc.). Based on available resources for Norway, how can I best download, import, and represent GIS data? For example, I want to import all Norwegian counties, and end up with "Place"s for each county, with GIS data so that I can draw a county's border on a map. Given the available resources, should a county "Place" reference a "meta" resource that contains all counties/borders, or should each county "Place" contain their own GIS area? How can I import and extract each county's GIS area given the freely available resources for Norway? The same goes for municipalities. Also, there is a temporal part of my program. Borders change over time. Are there resources available that will allow me to model/import GIS data back to, say 1600? If not, which resources, and how long back?

---

### Qwen

Building a GIS-aware application with a temporal component for Norway is a fantastic project, but it comes with specific challenges, especially regarding historical data.
Here is a comprehensive guide on how to architect your data, where to get it, and how to handle the historical timeline.

#### 1. Architecture: "Meta" Resource vs. Individual Place Geometries

**Recommendation: Use a centralized spatial database (the "meta" resource) and link your "Place" objects to it.**

Do not store the actual GIS geometry (the thousands of coordinate pairs) inside your individual `Place` objects in your application code. Instead, use a relational spatial database (like **PostgreSQL with PostGIS**).

**Why?**

- **Performance:cc Spatial databases use R-Tree indexes. If you want to find "all places within this bounding box," the database can do it in milliseconds. Doing this in application memory is incredibly slow.
- **Topology & Shared Borders:** Counties share borders. If you store geometries individually, you will have overlapping or mismatched coordinates. A centralized spatial table ensures topological consistency.
- **Storage:** Storing geometry in a database is vastly more efficient than storing thousands of individual GeoJSON files.

**Suggested Data Model:**

- **Table 1:** `Places` (Your core object)
  - id (UUID)
  - name (String)
  - type (Enum: County, Municipality, Farm, Building)
  - parent_id (Foreign key to another Place, e.g., a Municipality's parent is a County)

- **Table 2:** `Place_Geometries` (The "meta" spatial resource)
  - place_id (Foreign key to Places)
  - geometry (PostGIS GEOMETRY or GEOGRAPHY type)
  - valid_from (Date)
  - valid_to (Date, nullable if currently active)

#### 2. Sourcing and Importing Current GIS Data

For current, official Norwegian data, there is one absolute source of truth: **Kartverket (The Norwegian Mapping Authority)**, accessed via the national geoportal, **Geonorge**.

##### For Polygons (Counties, Municipalities)

1. Go to **Geonorge.no**.
2. Search for the dataset: **Administrative grenser** (Administrative boundaries).
3. Download the dataset.
    - _Format_: Choose **GeoPackage** (`.gpkg`) or **GeoJSON**. (Avoid Shapefile if possible, as it has legacy file-size and character-encoding limits. Avoid SOSI unless you have specialized Norwegian legacy software).
    - _Projection_: Ensure you download it in **EPSG:25833** (UTM Zone 33N) or **EPSG:4258** (ETRS89), which are the official Norwegian/European standards, or **EPSG:4326** (WGS84) if you are just using it for web maps.

##### For Points (Farms, Buildings)

1. On Geonorge, look for **Matrikkelen** (The Cadastre). This contains property boundaries, farm numbers (gårdsnummer), and building points.
2. Alternatively, for simple building points, look at the **Bygningspunkter** (Building points) dataset.

##### How to Import

Do not write your own parser for GIS files. Use **GDAL/OGR** or **GeoPandas** (Python).

- **To PostGIS:** Use the command line tool `ogr2ogr` (part of GDAL) to push the GeoPackage directly into PostGIS:

    ```bash
    ogr2ogr -f "PostgreSQL" PG:"dbname=mydb user=myuser" administrative_grenser.gpkg
    ```

- **In Python:** Use `geopandas` to read the file and `SQLAlchemy/GeoAlchemy2` to write it to your database.

#### 3. The Temporal / Historical Challenge

This is the hardest part of your project. **There is no authoritative, downloadable vector GIS dataset for Norwegian borders going back to 1600.**

In 1600, borders (like the _Len_ or _fiefs_) were not defined by precise GPS coordinates; they were defined by watersheds, local customs, and feudal rights.

##### What is available, and how far back?

1. **Modern Digital Polygons (1980s–Present):** Kartverket has digitized current borders.
2. **Reconstructed Historical Polygons (1837–Present):** In 1837, the Formannskapslovene established the modern municipality system. Academic projects (often tied to NTNU or the University of Tromsø) and SSB (Statistics Norway) have reconstructed municipality borders back to 1837.
3. **Metadata of Changes (1837–Present):** **SSB (ssb.no)** maintains a complete list of every municipality split, merger, and border adjustment since 1837. They provide the metadata (e.g., "Municipality A was merged into B in 1964"), but they do not always provide the exact historical polygon shapes for every single year.
4. **Raster Maps (1600–1800s):** The **National Archives of Norway (Riksarkivet)** and **Kartverket's historical map archive** have high-resolution scanned historical maps. You can use these as a background to manually digitize borders if you are doing historical research, but it is not "downloadable data."

##### How to model the temporal part

Since you cannot get 1600 vector data, you must design your system to handle **Temporal GIS (Valid Time)**.

When a municipality changes (e.g., a merger in 1964):

1. The old municipality's geometry in `Place_Geometries` gets a `valid_to` date of `1964-01-01`.
2. A new geometry is created for the new municipality with a `valid_from` date of `1964-01-01`.
3. Your `Places` table uses a temporal query to find the correct geometry for a specific year:

    ```sql
    SELECT p.name, g.geometry
    FROM Places p
    JOIN Place_Geometries g ON p.id = g.place_id
    WHERE p.type = 'MUNICIPALITY'
        AND g.valid_from <= '1850-01-01'
        AND (g.valid_to IS NULL OR g.valid_to > '1850-01-01');
    ```

##### Summary / Recommended Tech Stack

1. **Database:** PostgreSQL with the **PostGIS** extension. This is non-negotiable for serious GIS work.
2. **Backend:** Python (FastAPI/Django) or Node.js/TypeScript. Use **GeoPandas** (Python) or **Turf.js** / **PostGIS functions** for spatial calculations.
3. **Data Source:** **Geonorge.no** for current boundaries (Administrative grenser). **SSB** for the historical metadata of municipality changes.
4. **Historical Strategy:** Accept that 1600 is impossible in vector format. Build your system to support `valid_from`/`valid_to`, populate it with 1837+ data using SSB's historical codes, and for pre-1837, either limit your app to showing historical raster maps, or manually digitize the few major historical regions (like Syssel or Len) based on historical literature.
5. **Frontend Mapping:** **Leaflet.js**, **Mapbox GL JS**, or **OpenLayers** to render the GeoJSON/Vector tiles from your backend.
