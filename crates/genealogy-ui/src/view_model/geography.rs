//! The Geography tool's view-model (ADR 0025 §1): framework-free place markers, event pins, the
//! resolved time-slider year, and a provider descriptor for the map component. No framework types —
//! `genealogy-ui-dioxus`'s `MapLibre` component binds directly to these (`framework_free.rs` guard).
//!
//! Coordinates are decimal degrees (`f64`), mirroring the Phase-6 [`crate::MapPointVm`] convention —
//! the same reason [`GeographyVm`] is `PartialEq` but not `Eq`.

use genealogy_app::{GeographySummary, MapProvider, PlaceGeometry};

use crate::i18n::Localizer;

/// The slider's allowed year range: wide enough for any genealogical record (parish registers run
/// back centuries; a birth can be recorded decades into the future in a draft/planned record) without
/// being unbounded, which an `<input type=range>` needs concretely.
pub const TIME_SLIDER_RANGE: (i32, i32) = (1000, 2200);

/// Clamps a requested time-slider year into [`TIME_SLIDER_RANGE`] — the slider input's own bounds
/// make an out-of-range value unlikely, but a caller constructing a year directly (a "jump to year"
/// text entry, a saved link) is not guaranteed to respect them.
#[must_use]
pub fn clamp_slider_year(year: i32) -> i32 {
    year.clamp(TIME_SLIDER_RANGE.0, TIME_SLIDER_RANGE.1)
}

/// One place's shape on the map, in decimal degrees — a point, or a polygon boundary (exterior ring
/// plus any holes).
#[derive(Debug, Clone, PartialEq)]
pub enum MarkerShapeVm {
    /// A single point.
    Point {
        /// Latitude in decimal degrees.
        lat: f64,
        /// Longitude in decimal degrees.
        lon: f64,
    },
    /// A polygon boundary.
    Polygon {
        /// The outer boundary, as `(lat, lon)` pairs.
        exterior: Vec<(f64, f64)>,
        /// Interior holes cut out of the exterior, if any.
        holes: Vec<Vec<(f64, f64)>>,
    },
}

/// One place plotted on the map (ADR 0025 §1): its shape plus enough identity to label, select, and
/// navigate to it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceMarkerVm {
    /// The place's user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The place's stable id (a UUID string) — the selection/navigation key.
    pub id: String,
    /// The place's display name.
    pub name: String,
    /// The localized place-type label, if set.
    pub type_label: Option<String>,
    /// The shape to plot.
    pub shape: MarkerShapeVm,
}

/// One event pinned at its place (ADR 0025 §1 "event-at-place pins", Gramps `GeoView` parity).
#[derive(Debug, Clone, PartialEq)]
pub struct EventPinVm {
    /// The event's user-facing id (e.g. `E0001`).
    pub human_id: String,
    /// The event's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The localized event-type label, or a generic fallback when unset.
    pub label: String,
    /// The localized date, if known.
    pub date: Option<String>,
    /// The place's user-facing id the pin sits at (the pin's navigation target).
    pub place_human_id: String,
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

/// The map component's provider descriptor (ADR 0025 §3) — never carries key *material*, only the
/// **name** of the environment variable holding it (mirrors [`MapProvider`]); the renderer resolves
/// the value at the point it builds the tile/style request, keeping it out of the view-model and any
/// log built from one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapProviderVm {
    /// A raster XYZ tile source.
    OsmRaster {
        /// The `{z}/{x}/{y}` tile URL template.
        tile_url: String,
        /// The attribution string to display.
        attribution: String,
    },
    /// A `MapLibre` GL JS vector style.
    MaplibreStyle {
        /// The style URL.
        style_url: String,
        /// The attribution string to display.
        attribution: String,
        /// The environment variable name holding the style's API key, if it needs one.
        api_key_env: Option<String>,
    },
    /// A paid Google Maps style. Declared for configuration completeness (ADR 0025 §3); the v1 map
    /// component has no Google tile adapter yet (no plugin/geocoding work in this phase, ADR 0025
    /// §4) — selecting it renders the empty-provider state until that adapter lands, mirroring how
    /// `AiProvider::Plugin` is "named but not yet supported".
    Google {
        /// The environment variable name holding the API key.
        api_key_env: String,
        /// The attribution string to display.
        attribution: String,
    },
}

impl From<MapProvider> for MapProviderVm {
    fn from(provider: MapProvider) -> Self {
        match provider {
            MapProvider::OsmRaster { tile_url, attribution } => Self::OsmRaster { tile_url, attribution },
            MapProvider::MaplibreStyle {
                style_url,
                attribution,
                api_key_env,
            } => Self::MaplibreStyle {
                style_url,
                attribution,
                api_key_env,
            },
            MapProvider::Google {
                api_key_env,
                attribution,
            } => Self::Google {
                api_key_env,
                attribution,
            },
        }
    }
}

/// The Geography tool's full view: every resolved place marker and event pin, the year they were
/// resolved as of (`None` for the current/primary resolution), and the map's provider descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct GeographyVm {
    /// Every place with a resolved geometry, ready to plot.
    pub markers: Vec<PlaceMarkerVm>,
    /// Every event whose place resolved a geometry, ready to pin.
    pub events: Vec<EventPinVm>,
    /// The time-slider year this view is resolved as of; `None` for the current/primary resolution.
    pub resolved_year: Option<i32>,
    /// The map's provider descriptor.
    pub provider: MapProviderVm,
}

impl GeographyVm {
    /// Builds the view from the app's [`GeographySummary`] feed and the configured provider,
    /// localizing every marker/pin label.
    #[must_use]
    pub fn from_summary(summary: &GeographySummary, provider: MapProvider, loc: &Localizer) -> Self {
        let markers = summary
            .markers
            .iter()
            .map(|marker| place_marker_vm(marker, loc))
            .collect();
        let events = summary.events.iter().map(|pin| event_pin_vm(pin, loc)).collect();
        let resolved_year = summary.resolved_as_of.as_ref().map(year_of);
        Self {
            markers,
            events,
            resolved_year,
            provider: provider.into(),
        }
    }
}

/// The representative year of a resolved date's sort key (mirrors `genealogy_app::dto`'s private
/// `year_of`, which this crate cannot reach — the view-model needs only the year for the caption).
fn year_of(date: &genealogy_app::GenealogicalDate) -> i32 {
    i32::try_from(date.sort_value / 10_000).unwrap_or_default()
}

fn place_marker_vm(marker: &genealogy_app::PlaceMarker, loc: &Localizer) -> PlaceMarkerVm {
    PlaceMarkerVm {
        human_id: marker.human_id.clone(),
        id: marker.id.clone(),
        name: marker.name.clone(),
        type_label: marker.place_type.as_ref().map(|t| loc.place_type_label(t)),
        shape: marker_shape(&marker.geometry),
    }
}

/// Converts a domain [`PlaceGeometry`] (integer microdegrees) to the view-model's decimal-degree
/// shape — the boundary a map library plots in `[lon, lat]` `GeoJSON` order lives at the render layer;
/// this stays in `(lat, lon)` order matching every other view-model coordinate.
fn marker_shape(geometry: &PlaceGeometry) -> MarkerShapeVm {
    match geometry {
        PlaceGeometry::Point(point) => MarkerShapeVm::Point {
            lat: point.latitude.to_degrees(),
            lon: point.longitude.to_degrees(),
        },
        PlaceGeometry::Polygon { exterior, holes } => MarkerShapeVm::Polygon {
            exterior: ring(exterior),
            holes: holes.iter().map(|hole| ring(hole)).collect(),
        },
    }
}

fn ring(points: &[genealogy_app::GeoCoordinates]) -> Vec<(f64, f64)> {
    points
        .iter()
        .map(|point| (point.latitude.to_degrees(), point.longitude.to_degrees()))
        .collect()
}

fn event_pin_vm(pin: &genealogy_app::EventPin, loc: &Localizer) -> EventPinVm {
    EventPinVm {
        human_id: pin.human_id.clone(),
        id: pin.id.clone(),
        label: pin
            .event_type
            .as_ref()
            .map_or_else(|| loc.field_label("event"), |kind| loc.event_type_label(kind)),
        date: pin.date.as_ref().map(|date| loc.date(date)),
        place_human_id: pin.place_human_id.clone(),
        lat: pin.point.latitude.to_degrees(),
        lon: pin.point.longitude.to_degrees(),
    }
}

#[cfg(test)]
mod tests {
    use super::{GeographyVm, MapProviderVm, MarkerShapeVm, clamp_slider_year};
    use crate::i18n::Localizer;
    use genealogy_app::{
        Calendar, DateModifier, DatePoint, DateQuality, EventPin, EventType, GenealogicalDate, GenealogicalDateBody,
        GeoCoordinates, MapProvider, Microdegrees, PlaceGeometry, PlaceMarker, PlaceType,
    };
    use std::str::FromStr;

    fn coord(lat: &str, lon: &str) -> GeoCoordinates {
        GeoCoordinates {
            latitude: Microdegrees::from_str(lat).expect("lat"),
            longitude: Microdegrees::from_str(lon).expect("lon"),
        }
    }

    fn year_date(year: i32) -> GenealogicalDate {
        GenealogicalDate {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
                year: Some(year),
                month: None,
                day: None,
            })),
            time: None,
            new_year_begins: None,
            sort_value: i64::from(year) * 10_000,
            original_text: None,
        }
    }

    fn loc() -> Localizer {
        Localizer::for_test("en")
    }

    #[test]
    fn a_point_marker_converts_to_decimal_degrees() {
        let summary = genealogy_app::GeographySummary {
            markers: vec![PlaceMarker {
                human_id: "P0001".to_owned(),
                id: "place-1".to_owned(),
                name: "Oslo".to_owned(),
                place_type: Some(PlaceType::City),
                geometry: PlaceGeometry::Point(coord("59.9139", "10.7522")),
            }],
            events: Vec::new(),
            resolved_as_of: None,
        };
        let vm = GeographyVm::from_summary(&summary, MapProvider::default_osm(), &loc());
        assert_eq!(vm.markers.len(), 1);
        let marker = &vm.markers[0];
        assert_eq!(marker.human_id, "P0001");
        match marker.shape {
            MarkerShapeVm::Point { lat, lon } => {
                assert!((lat - 59.9139).abs() < 1e-6);
                assert!((lon - 10.7522).abs() < 1e-6);
            }
            MarkerShapeVm::Polygon { .. } => panic!("expected a point"),
        }
    }

    #[test]
    fn a_polygon_marker_carries_its_exterior_and_holes() {
        let summary = genealogy_app::GeographySummary {
            markers: vec![PlaceMarker {
                human_id: "P0002".to_owned(),
                id: "place-2".to_owned(),
                name: "Old County".to_owned(),
                place_type: Some(PlaceType::County),
                geometry: PlaceGeometry::Polygon {
                    exterior: vec![coord("60.0", "5.0"), coord("61.0", "5.0"), coord("61.0", "6.0")],
                    holes: vec![vec![coord("60.3", "5.3"), coord("60.4", "5.3"), coord("60.4", "5.4")]],
                },
            }],
            events: Vec::new(),
            resolved_as_of: None,
        };
        let vm = GeographyVm::from_summary(&summary, MapProvider::default_osm(), &loc());
        match &vm.markers[0].shape {
            MarkerShapeVm::Polygon { exterior, holes } => {
                assert_eq!(exterior.len(), 3);
                assert_eq!(holes.len(), 1);
                assert_eq!(holes[0].len(), 3);
            }
            MarkerShapeVm::Point { .. } => panic!("expected a polygon"),
        }
    }

    #[test]
    fn an_event_pin_resolves_its_places_point() {
        let summary = genealogy_app::GeographySummary {
            markers: Vec::new(),
            events: vec![EventPin {
                human_id: "E0001".to_owned(),
                id: "event-1".to_owned(),
                event_type: Some(EventType::Birth),
                date: None,
                place_human_id: "P0001".to_owned(),
                point: coord("59.9", "10.7"),
            }],
            resolved_as_of: None,
        };
        let vm = GeographyVm::from_summary(&summary, MapProvider::default_osm(), &loc());
        assert_eq!(vm.events.len(), 1);
        assert_eq!(vm.events[0].place_human_id, "P0001");
        assert!((vm.events[0].lat - 59.9).abs() < 1e-6);
    }

    #[test]
    fn the_resolved_year_threads_through_from_the_as_of_date() {
        let summary = genealogy_app::GeographySummary {
            markers: Vec::new(),
            events: Vec::new(),
            resolved_as_of: Some(year_date(1900)),
        };
        let vm = GeographyVm::from_summary(&summary, MapProvider::default_osm(), &loc());
        assert_eq!(vm.resolved_year, Some(1900));
    }

    #[test]
    fn no_as_of_date_leaves_the_resolved_year_unset() {
        let summary = genealogy_app::GeographySummary::default();
        let vm = GeographyVm::from_summary(&summary, MapProvider::default_osm(), &loc());
        assert_eq!(vm.resolved_year, None);
    }

    #[test]
    fn the_osm_default_provider_maps_to_its_vm_variant() {
        let vm = GeographyVm::from_summary(
            &genealogy_app::GeographySummary::default(),
            MapProvider::default_osm(),
            &loc(),
        );
        assert!(matches!(vm.provider, MapProviderVm::OsmRaster { .. }));
    }

    #[test]
    fn a_year_within_range_is_unchanged() {
        assert_eq!(clamp_slider_year(1900), 1900);
    }

    #[test]
    fn a_year_below_range_clamps_to_the_minimum() {
        assert_eq!(clamp_slider_year(-500), 1000);
    }

    #[test]
    fn a_year_above_range_clamps_to_the_maximum() {
        assert_eq!(clamp_slider_year(9999), 2200);
    }
}
