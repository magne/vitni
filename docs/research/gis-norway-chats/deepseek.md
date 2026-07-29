### User

I'm writing a program that uses "Places". A place can be a point (building, farm, etc.), or an area/polygon/etc (municipality, county, country, parish, bailiwick, etc.). Based on available resources for Norway, how can I best download, import, and represent GIS data? For example, I want to import all Norwegian counties, and end up with "Place"s for each county, with GIS data so that I can draw a county's border on a map. Given the available resources, should a county "Place" reference a "meta" resource that contains all counties/borders, or should each county "Place" contain their own GIS area? How can I import and extract each county's GIS area given the freely available resources for Norway? The same goes for municipalities. Also, there is a temporal part of my program. Borders change over time. Are there resources available that will allow me to model/import GIS data back to, say 1600? If not, which resources, and how long back?

---

### DeepSeek

For Norwegian GIS data, you have excellent resources available, particularly through Kartverket (the Norwegian Mapping Authority) and other open data portals. Let me break down the solutions for your different needs.

## Data Sources for Norwegian Administrative Boundaries

### Modern Boundaries (Current)

**Kartverket's Administrative Units (Administrative enheter)**

- Counties (fylker) and municipalities (kommuner) available as GeoJSON, Shapefile, or GML
- Updated regularly
- Free and open data under Norwegian license (NLOD)
- Available at: [Kartverket's open data portal](https://kartkatalog.geonorge.no/)

### Historical Boundaries (Limited Availability)

This is more challenging. Norway's digital boundary data only goes back to approximately:

- **Municipalities**: Digital boundaries available from 1998 (when GIS became standardized)
- **Counties**: Similar timeframe, though some historical reconstructions exist
- **Before 1998**: Limited to paper maps and historical records

For historical boundaries before the digital era, you have these options:

1. **DigitaltMuseum** and **Arkivverket** (National Archives) - historical maps
2. **Norsk historisk leksikon** - textual descriptions of historical administrative divisions
3. **Historical GIS projects** (limited - Norway lacks comprehensive historical GIS like UK's "A Vision of Britain")

## Data Architecture Recommendation

Based on performance and maintainability, I recommend a **hybrid approach**:

```python
# Conceptual model
class GeoDataset:
    """Contains shared geometry for a type of boundary"""
    id: str  # e.g., "norway_counties_2024"
    geometry_type: str  # "MultiPolygon"
    source: str  # "Kartverket"
    valid_from: date
    valid_to: date | None
    raw_geojson: dict  # The complete GeoJSON FeatureCollection

class Place:
    """Individual place with reference to shared geometry"""
    id: str
    name: str
    place_type: str  # "county", "municipality", "farm"
    geo_dataset_id: str  # Reference to GeoDataset
    feature_index: int  # Index in the FeatureCollection
    temporal_validity: TemporalRange

    def get_geometry(self) -> dict:
        """Extract this place's geometry from the shared dataset"""
        dataset = GeoDataset.get(self.geo_dataset_id)
        return dataset.raw_geojson['features'][self.feature_index]
```

### Why This Approach?

1. **Storage Efficiency**: A county boundary file might be several MB, but individual counties share borders
2. **Performance**: Load the GeoJSON once, index features by property
3. **Consistency**: When rendering multiple counties, you work with one FeatureCollection
4. **Topology**: Adjacent features' boundaries match perfectly (no gaps/overlaps)

## Implementation Example

Here's how to import and structure the data:

```python
import requests
import json
from datetime import date
class NorwegianGeoImporter:
    """Import and manage Norwegian administrative boundaries"""

    KARTVERKET_API = "https://kartkatalog.geonorge.no/api"

    # Known dataset IDs from Kartverket
    DATASETS = {
        "counties_2024": "fylker-2024",
        "municipalities_2024": "kommuner-2024",
        # Add historical datasets as they become available
    }

    def download_boundary_dataset(self, dataset_id: str, format: str = "geojson") -> dict:
        """
        Download a complete boundary dataset from Kartverket.
        Returns the full GeoJSON FeatureCollection.
        """
        # Get actual download URL from Kartverket API
        metadata_url = f"{self.KARTVERKET_API}/datasets/{dataset_id}"
        # ... implementation to get actual file URL

        # Download and return the GeoJSON
        response = requests.get(download_url)
        return response.json()

    def extract_features_to_places(self, geo_dataset_id: str, place_type: str):
        """
        Create Place objects from a GeoJSON dataset.
        Each feature becomes a separate Place referencing the shared geometry.
        """
        dataset = self.load_dataset(geo_dataset_id)
        places = []

        for idx, feature in enumerate(dataset['features']):
            props = feature['properties']
            place = Place(
                name=props.get('navn', props.get('name')),
                place_type=place_type,
                geo_dataset_id=geo_dataset_id,
                feature_index=idx,
                # Extract administrative codes
                admin_code=props.get('fylkesnummer', props.get('kommunenummer')),
            )
            places.append(place)

        return places
# Usage
importer = NorwegianGeoImporter()
# Import all counties
county_dataset_id = "counties_2024"
geo_data = importer.download_boundary_dataset(county_dataset_id)
# Create GeoDataset record
dataset = GeoDataset(
    id=county_dataset_id,
    raw_geojson=geo_data,
    valid_from=date(2024, 1, 1),
    source="Kartverket"
)
# Create individual Place objects
counties = importer.extract_features_to_places(county_dataset_id, "county")
# For Oslo county specifically:
oslo = next(c for c in counties if c.name == "Oslo")
oslo_geometry = oslo.get_geometry()  # Extracts single feature from shared data
```

## Temporal Modeling

For historical boundaries, you'll need:

```python
class TemporalBoundary:
    """Track boundary changes over time"""
    place_id: str
    geometry: dict  # The actual boundary geometry
    valid_from: date
    valid_to: date | None  # None = currently valid
    change_type: str  # "created", "modified", "merged", "split"
    source_reference: str  # Citation for historical data

    def overlaps(self, other_date: date) -> bool:
        return self.valid_from <= other_date <= (self.valid_to or date.today())
```

### Available Temporal Resources

1. **SSB (Statistics Norway) Historical Classifications**
    - Municipality number changes tracked back to 1990s
    - Classification codes (kommunenummer) with validity dates
    - Not including geometry, just codes and names

2. **Lokalhistoriewiki**
    - Detailed historical municipality changes
    - Textual descriptions of boundary changes
    - You'd need to digitize geometries yourself for pre-1998

3. **For pre-1998 digital boundaries**, consider:
    - Digitizing historical maps yourself (labor intensive)
    - Partnering with historical museums or universities
    - Using modern boundaries as approximations (with caveats)

## Practical Recommendations

1. **Start with modern data** (post-1998) where digital boundaries are readily available
2. **Use GeoJSON as your primary format** - it's JSON, web-native, and easy to work with in Python
3. **Store the complete FeatureCollection** in your GeoDataset table as JSONB (PostgreSQL) or a JSON column
4. **Index features by their administrative code** for quick lookups:

    ```python
    def create_feature_index(geojson: dict, property_key: str) -> dict:
        """Create lookup index from property value to feature index"""
        return {
            feature['properties'][property_key]: idx
            for idx, feature in enumerate(geojson['features'])
        }
    ```

5. **For rendering**, use the shared GeoDataset directly with mapping libraries (Leaflet, OpenLayers)

6. **For temporal queries**, maintain a timeline of GeoDatasets:

    - Each GeoDataset represents a complete boundary set for a time period

    - Query: "Give me Oslo's boundaries in 2020" → Find GeoDataset valid for 2020 → Extract Oslo feature

Would you like me to elaborate on any specific aspect, such as the download process from Kartverket's specific APIs, or strategies for handling historical boundary data?
