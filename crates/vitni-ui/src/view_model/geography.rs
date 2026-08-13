//! The Geography tool's view-model (ADR 0025 §1): framework-free place markers, event pins, the
//! resolved time-slider year, and a provider descriptor for the map component. No framework types —
//! `vitni-ui-dioxus`'s `MapLibre` component binds directly to these (`framework_free.rs` guard).
//!
//! Coordinates are decimal degrees (`f64`), mirroring the Phase-6 [`crate::MapPointVm`] convention —
//! the same reason [`GeographyVm`] is `PartialEq` but not `Eq`.

use vitni_app::{GeographySummary, PlaceGeometry, UnplottedReason};

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

/// The map camera's allowed zoom range. The ceiling is the last zoom the raster tile source actually
/// serves (`tile.openstreetmap.org` stops at z19); `MapLibre`'s own default ceiling is 22, so three
/// levels past the last existing tile the map went blank. The floor keeps the whole globe in view
/// without letting a gesture zoom out into repeated world copies.
pub const ZOOM_RANGE: (f64, f64) = (1.0, 19.0);

/// Clamps a zoom level into [`ZOOM_RANGE`] — the map's own `minZoom`/`maxZoom` bound the camera, so
/// this is what keeps a *readout* (or any zoom a caller computes itself) inside the same range.
/// Non-finite input falls back to the floor: [`f64::clamp`] panics on a non-finite bound and
/// propagates `NaN` otherwise, and `panic` is deny-level in this workspace.
#[must_use]
pub fn clamp_zoom(zoom: f64) -> f64 {
    if !zoom.is_finite() {
        return ZOOM_RANGE.0;
    }
    zoom.clamp(ZOOM_RANGE.0, ZOOM_RANGE.1)
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

/// Whether a [`PlaceRowVm`] is currently plotted on the map, holds geometry that just does not
/// resolve as of the feed's year, or has never been located at all (#256). Carries no localized
/// text itself — the `geography-row-*`/`geography-rail-note` chrome keys live in the Dioxus bundle,
/// which is the only layer that owns Fluent strings (ADR 0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceRowStatus {
    /// The place resolved a geometry as of this feed's year and is plotted on the map.
    Plotted,
    /// The place holds geometry, but none of it resolves as of this feed's year.
    NoGeometryAsOf,
    /// The place has never been located: no geometry assertion, no scalar coordinate either.
    NoGeometry,
}

/// One row of the Geography rail (#256): every place in the workspace, not just the plotted ones —
/// so a place with no geometry is still a selectable draw target, and the list no longer silently
/// shrinks as the time slider moves.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceRowVm {
    /// The place's user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The place's stable id (a UUID string) — the selection/navigation key.
    pub id: String,
    /// The place's display name.
    pub name: String,
    /// The localized place-type label, if set.
    pub type_label: Option<String>,
    /// Whether this place is currently plotted, and if not, why.
    pub status: PlaceRowStatus,
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

/// The Geography tool's full view: every resolved place marker and event pin, every place as a rail
/// row (plotted or not, #256), the year they were resolved as of (`None` for the current/primary
/// resolution), and the map's provider descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct GeographyVm {
    /// Every place with a resolved geometry, ready to plot. The map, the pushed `GeoJSON`, and the
    /// "⤢ Fit" toolbar button all still work from this list alone — [`Self::places`] is for the rail.
    pub markers: Vec<PlaceMarkerVm>,
    /// Every event whose place resolved a geometry, ready to pin.
    pub events: Vec<EventPinVm>,
    /// Every place in the workspace, sorted by `human_id` (stable regardless of the slider year),
    /// merged from [`Self::markers`] and the app's `GeographySummary::unplotted` — the rail's rows.
    pub places: Vec<PlaceRowVm>,
    /// The time-slider year this view is resolved as of; `None` for the current/primary resolution.
    pub resolved_year: Option<i32>,
}

impl GeographyVm {
    /// Builds the view from the app's [`GeographySummary`] feed, localizing every marker/pin/row
    /// label.
    #[must_use]
    pub fn from_summary(summary: &GeographySummary, loc: &Localizer) -> Self {
        let markers = summary
            .markers
            .iter()
            .map(|marker| place_marker_vm(marker, loc))
            .collect();
        let events = summary.events.iter().map(|pin| event_pin_vm(pin, loc)).collect();
        let mut places: Vec<PlaceRowVm> = summary
            .markers
            .iter()
            .map(|marker| PlaceRowVm {
                human_id: marker.human_id.clone(),
                id: marker.id.clone(),
                name: marker.name.clone(),
                type_label: marker.place_type.as_ref().map(|t| loc.place_type_label(t)),
                status: PlaceRowStatus::Plotted,
            })
            .chain(summary.unplotted.iter().map(|place| PlaceRowVm {
                human_id: place.human_id.clone(),
                id: place.id.clone(),
                name: place.name.clone(),
                type_label: place.place_type.as_ref().map(|t| loc.place_type_label(t)),
                status: match place.reason {
                    UnplottedReason::DatedLater => PlaceRowStatus::NoGeometryAsOf,
                    UnplottedReason::NoGeometry => PlaceRowStatus::NoGeometry,
                },
            }))
            .collect();
        places.sort_by(|left, right| left.human_id.cmp(&right.human_id));
        let resolved_year = summary.resolved_as_of.as_ref().map(year_of);
        Self {
            markers,
            events,
            places,
            resolved_year,
        }
    }

    /// How many rail rows are not plotted — the screen's note renders this count so those places are
    /// reported, not silently absent.
    #[must_use]
    pub fn unplotted_count(&self) -> usize {
        self.places
            .iter()
            .filter(|row| row.status != PlaceRowStatus::Plotted)
            .count()
    }

    /// Every rail row matching `query` ([`name_matches`]) — plotted and unplotted alike, so the
    /// toolbar search filters the whole rail, not just plotted markers.
    #[must_use]
    pub fn filtered_places(&self, query: &str) -> Vec<&PlaceRowVm> {
        self.places
            .iter()
            .filter(|row| name_matches(&row.name, query))
            .collect()
    }
}

/// Case-insensitive substring match on a trimmed `query`; a blank query matches everything — the
/// exact rule the Geography rail/map filter has always used, now shared rather than duplicated.
#[must_use]
pub fn name_matches(name: &str, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty() || name.to_lowercase().contains(&query)
}

/// The representative year of a resolved date's sort key (mirrors `vitni_app::dto`'s private
/// `year_of`, which this crate cannot reach — the view-model needs only the year for the caption).
/// `pub(crate)` so the Place VM's geometry-over-time list (Phase 9) can sort/resolve its dated
/// assertions by year without duplicating this conversion.
pub(crate) fn year_of(date: &vitni_app::GenealogicalDate) -> i32 {
    i32::try_from(date.sort_value / 10_000).unwrap_or_default()
}

fn place_marker_vm(marker: &vitni_app::PlaceMarker, loc: &Localizer) -> PlaceMarkerVm {
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
/// this stays in `(lat, lon)` order matching every other view-model coordinate. `pub(crate)` so the
/// Place VM's [`crate::view_model::place::PlaceGeometryVm`] (Phase 9's per-place geometry-over-time
/// list) reuses this conversion rather than duplicating it.
pub(crate) fn marker_shape(geometry: &PlaceGeometry) -> MarkerShapeVm {
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

fn ring(points: &[vitni_app::GeoCoordinates]) -> Vec<(f64, f64)> {
    points
        .iter()
        .map(|point| (point.latitude.to_degrees(), point.longitude.to_degrees()))
        .collect()
}

/// `pub(crate)` so the Place VM's own single-place event pins (Phase 9 follow-up) reuse this
/// conversion rather than duplicating it — mirrors [`marker_shape`]'s sharing.
pub(crate) fn event_pin_vm(pin: &vitni_app::EventPin, loc: &Localizer) -> EventPinVm {
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
    use super::{GeographyVm, MarkerShapeVm, PlaceRowStatus, ZOOM_RANGE, clamp_slider_year, clamp_zoom, name_matches};
    use crate::i18n::Localizer;
    use std::str::FromStr;
    use vitni_app::{
        Calendar, DateModifier, DatePoint, DateQuality, EventPin, EventType, GenealogicalDate, GenealogicalDateBody,
        GeoCoordinates, Microdegrees, PlaceGeometry, PlaceMarker, PlaceType, UnplottedPlace, UnplottedReason,
    };

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
        let summary = vitni_app::GeographySummary {
            markers: vec![PlaceMarker {
                human_id: "P0001".to_owned(),
                id: "place-1".to_owned(),
                name: "Oslo".to_owned(),
                place_type: Some(PlaceType::City),
                geometry: PlaceGeometry::Point(coord("59.9139", "10.7522")),
            }],
            events: Vec::new(),
            unplotted: Vec::new(),
            resolved_as_of: None,
        };
        let vm = GeographyVm::from_summary(&summary, &loc());
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
        let summary = vitni_app::GeographySummary {
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
            unplotted: Vec::new(),
            resolved_as_of: None,
        };
        let vm = GeographyVm::from_summary(&summary, &loc());
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
        let summary = vitni_app::GeographySummary {
            markers: Vec::new(),
            events: vec![EventPin {
                human_id: "E0001".to_owned(),
                id: "event-1".to_owned(),
                event_type: Some(EventType::Birth),
                date: None,
                place_human_id: "P0001".to_owned(),
                point: coord("59.9", "10.7"),
            }],
            unplotted: Vec::new(),
            resolved_as_of: None,
        };
        let vm = GeographyVm::from_summary(&summary, &loc());
        assert_eq!(vm.events.len(), 1);
        assert_eq!(vm.events[0].place_human_id, "P0001");
        assert!((vm.events[0].lat - 59.9).abs() < 1e-6);
    }

    #[test]
    fn the_resolved_year_threads_through_from_the_as_of_date() {
        let summary = vitni_app::GeographySummary {
            markers: Vec::new(),
            events: Vec::new(),
            unplotted: Vec::new(),
            resolved_as_of: Some(year_date(1900)),
        };
        let vm = GeographyVm::from_summary(&summary, &loc());
        assert_eq!(vm.resolved_year, Some(1900));
    }

    #[test]
    fn no_as_of_date_leaves_the_resolved_year_unset() {
        let summary = vitni_app::GeographySummary::default();
        let vm = GeographyVm::from_summary(&summary, &loc());
        assert_eq!(vm.resolved_year, None);
    }

    fn unplotted(human_id: &str, name: &str, reason: UnplottedReason) -> UnplottedPlace {
        UnplottedPlace {
            human_id: human_id.to_owned(),
            id: format!("place-{human_id}"),
            name: name.to_owned(),
            place_type: Some(PlaceType::Parish),
            reason,
        }
    }

    #[test]
    fn the_places_that_did_not_resolve_are_counted_for_the_note() {
        let summary = vitni_app::GeographySummary {
            markers: Vec::new(),
            events: Vec::new(),
            unplotted: vec![
                unplotted("P0001", "Vågå", UnplottedReason::DatedLater),
                unplotted("P0002", "Lom", UnplottedReason::NoGeometry),
            ],
            resolved_as_of: Some(year_date(1850)),
        };
        let vm = GeographyVm::from_summary(&summary, &loc());
        assert_eq!(vm.unplotted_count(), 2);
    }

    #[test]
    fn nothing_unplotted_counts_zero_so_the_note_stays_hidden() {
        let vm = GeographyVm::from_summary(&vitni_app::GeographySummary::default(), &loc());
        assert_eq!(vm.unplotted_count(), 0);
    }

    #[test]
    fn places_are_merged_from_markers_and_unplotted_sorted_by_human_id() {
        let summary = vitni_app::GeographySummary {
            markers: vec![PlaceMarker {
                human_id: "P0002".to_owned(),
                id: "place-2".to_owned(),
                name: "Lom".to_owned(),
                place_type: Some(PlaceType::Parish),
                geometry: PlaceGeometry::Point(coord("61.8", "8.5")),
            }],
            events: Vec::new(),
            unplotted: vec![
                unplotted("P0003", "Nordland", UnplottedReason::NoGeometry),
                unplotted("P0001", "Vågå", UnplottedReason::DatedLater),
            ],
            resolved_as_of: None,
        };
        let vm = GeographyVm::from_summary(&summary, &loc());
        let ids: Vec<&str> = vm.places.iter().map(|row| row.human_id.as_str()).collect();
        assert_eq!(ids, ["P0001", "P0002", "P0003"]);
    }

    #[test]
    fn a_marker_row_is_plotted() {
        let summary = vitni_app::GeographySummary {
            markers: vec![PlaceMarker {
                human_id: "P0001".to_owned(),
                id: "place-1".to_owned(),
                name: "Oslo".to_owned(),
                place_type: Some(PlaceType::City),
                geometry: PlaceGeometry::Point(coord("59.9", "10.7")),
            }],
            events: Vec::new(),
            unplotted: Vec::new(),
            resolved_as_of: None,
        };
        let vm = GeographyVm::from_summary(&summary, &loc());
        assert_eq!(vm.places[0].status, PlaceRowStatus::Plotted);
    }

    #[test]
    fn a_dated_later_row_carries_its_status_and_type_label() {
        let summary = vitni_app::GeographySummary {
            markers: Vec::new(),
            events: Vec::new(),
            unplotted: vec![unplotted("P0001", "Vågå", UnplottedReason::DatedLater)],
            resolved_as_of: None,
        };
        let vm = GeographyVm::from_summary(&summary, &loc());
        assert_eq!(vm.places[0].status, PlaceRowStatus::NoGeometryAsOf);
        assert!(vm.places[0].type_label.is_some());
    }

    #[test]
    fn a_no_geometry_row_carries_its_status_and_type_label() {
        let summary = vitni_app::GeographySummary {
            markers: Vec::new(),
            events: Vec::new(),
            unplotted: vec![unplotted("P0001", "Nordland", UnplottedReason::NoGeometry)],
            resolved_as_of: None,
        };
        let vm = GeographyVm::from_summary(&summary, &loc());
        assert_eq!(vm.places[0].status, PlaceRowStatus::NoGeometry);
        assert!(vm.places[0].type_label.is_some());
    }

    #[test]
    fn filtered_places_matches_both_plotted_and_unplotted_rows() {
        let summary = vitni_app::GeographySummary {
            markers: vec![PlaceMarker {
                human_id: "P0001".to_owned(),
                id: "place-1".to_owned(),
                name: "Oslo".to_owned(),
                place_type: Some(PlaceType::City),
                geometry: PlaceGeometry::Point(coord("59.9", "10.7")),
            }],
            events: Vec::new(),
            unplotted: vec![unplotted("P0002", "Oslofjord", UnplottedReason::NoGeometry)],
            resolved_as_of: None,
        };
        let vm = GeographyVm::from_summary(&summary, &loc());
        let matches = vm.filtered_places("osl");
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn a_blank_query_matches_everything() {
        assert!(name_matches("Oslo", ""));
        assert!(name_matches("Oslo", "   "));
    }

    #[test]
    fn a_query_matches_a_case_insensitive_substring() {
        assert!(name_matches("Oslo", "osl"));
        assert!(name_matches("Oslo", "OSLO"));
    }

    #[test]
    fn a_non_matching_query_does_not_match() {
        assert!(!name_matches("Oslo", "Bergen"));
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

    #[test]
    fn the_zoom_range_is_ordered_low_to_high() {
        assert!(ZOOM_RANGE.0 < ZOOM_RANGE.1, "a floor above the ceiling clamps nothing");
    }

    #[test]
    fn a_zoom_within_range_is_unchanged() {
        assert!((clamp_zoom(14.2) - 14.2).abs() < 1e-9);
    }

    /// The defect itself: `MapLibre`'s own default ceiling is 22, three levels past the last raster
    /// tile the OSM source serves, and a wheel gesture past it blanked the map.
    #[test]
    fn a_zoom_above_range_clamps_to_the_last_zoom_the_tiles_exist_at() {
        assert!((clamp_zoom(22.0) - ZOOM_RANGE.1).abs() < 1e-9);
    }

    #[test]
    fn a_zoom_below_range_clamps_up_to_the_floor() {
        assert!((clamp_zoom(-3.0) - ZOOM_RANGE.0).abs() < 1e-9);
    }

    /// `f64::clamp` panics on a non-finite bound *and* propagates `NaN` — and `panic` is deny-level
    /// here, so the guard runs first and a nonsense reading falls back to the floor.
    #[test]
    fn a_non_finite_zoom_falls_back_to_the_floor_instead_of_propagating() {
        for nonsense in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                (clamp_zoom(nonsense) - ZOOM_RANGE.0).abs() < 1e-9,
                "{nonsense} must not reach the map as a camera bound"
            );
        }
    }
}
