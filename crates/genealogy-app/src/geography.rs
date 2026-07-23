//! Geography use-cases (ADR 0025 §1): the read-only feed the framework-free map view-model
//! (`genealogy-ui`) renders — every place with a resolved geometry (an ADR 0024 assertion, or the
//! scalar `AssertCoordinates` point when no dedicated geometry has been drawn), plus event pins at
//! those places — resolved **as of** an optional year (ADR 0026 §1) for the time slider.
//!
//! v1 loads every place with [`list_places`]/[`list_places_as_of`], mirroring the existing Places
//! list screen. Wiring the viewport through [`genealogy_db::Store::places_in_bbox`] (ADR 0024 §3) to
//! skip summarizing places outside the current map view is a deferred follow-up once dataset scale
//! needs it — the Postgres mirror has no spatial index yet either (ADR 0024 §3's own deferral), so a
//! viewport-aware `show_geography` would need to degrade gracefully on that backend regardless.

use genealogy_core::date::GenealogicalDate;
use genealogy_core::enums::{EventType, PlaceType};
use genealogy_core::geo::{GeoCoordinates, PlaceGeometry};

use crate::error::AppError;
use crate::event::{DateParts, gregorian_date, list_events};
use crate::place::{list_places, list_places_as_of};
use crate::workspace::Workspace;

/// A place ready to render as a map marker: its resolved-as-of geometry (ADR 0026 §1) plus enough
/// identity to label and navigate to it. Only places with a resolved geometry (a drawn ADR 0024
/// assertion, or the scalar coordinate fallback) ever become a marker.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceMarker {
    /// The place's user-facing identifier (e.g. `P0001`).
    pub human_id: String,
    /// The place's stable `PlaceId` (a UUID string) — the join/navigation/selection key.
    pub id: String,
    /// The place's display name (falls back to the `human_id`).
    pub name: String,
    /// The place's type, if set. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub place_type: Option<PlaceType>,
    /// The resolved shape (a point or a polygon) to plot.
    pub geometry: PlaceGeometry,
}

/// An event pinned at its place's resolved point (ADR 0025 §1 "event-at-place pins", Gramps `GeoView`
/// parity). Only events whose linked place resolved a geometry are included; the pin sits at the
/// place's [`PlaceGeometry::representative_point`] (its own point, or an area's approximate centre).
#[derive(Debug, Clone, PartialEq)]
pub struct EventPin {
    /// The event's user-facing identifier (e.g. `E0001`).
    pub human_id: String,
    /// The event's stable `EventId` (a UUID string) — the navigation key.
    pub id: String,
    /// The kind of event, if set. Structured so the frontend localizes it (ADR 0003).
    pub event_type: Option<EventType>,
    /// When the event occurred, if known. Structured so the frontend localizes it (ADR 0003).
    pub date: Option<GenealogicalDate>,
    /// The pinned place's user-facing identifier — the pin's label/navigation target.
    pub place_human_id: String,
    /// The point the pin is plotted at.
    pub point: GeoCoordinates,
}

/// The geography view's data feed (ADR 0025 §1): every resolved place marker and event pin, plus the
/// date they were resolved **as of** (echoed for the time-slider caption, mirroring
/// [`crate::place::PlaceSummary::resolved_as_of`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeographySummary {
    /// Every place with a resolved geometry, ready to plot.
    pub markers: Vec<PlaceMarker>,
    /// Every event whose place resolved a geometry, ready to pin.
    pub events: Vec<EventPin>,
    /// The date this feed is resolved **as of**; `None` for the current/primary resolution.
    pub resolved_as_of: Option<GenealogicalDate>,
}

/// A bare-year [`GenealogicalDate`] for the geography time slider and in-map dated edits (ADR 0025
/// §2, ADR 0026 §1): reuses the exact-date sort-key math [`gregorian_date`] already computes for
/// events, so "as of 1900" sorts and resolves identically everywhere a date drives ADR 0026 §1's rule.
#[must_use]
pub fn year_only_date(year: i32) -> GenealogicalDate {
    gregorian_date(DateParts {
        year,
        month: None,
        day: None,
    })
}

/// Loads the geography view's markers and event pins, resolved **as of** `year` (ADR 0026 §1) — the
/// current/primary resolution when `None`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_geography(workspace: &Workspace, year: Option<i32>) -> Result<GeographySummary, AppError> {
    let as_of = year.map(year_only_date);
    let places = match as_of.clone() {
        Some(date) => list_places_as_of(workspace, date).await?,
        None => list_places(workspace).await?,
    };

    let mut points = std::collections::HashMap::new();
    let mut markers = Vec::new();
    for place in &places {
        // Prefer the dated ADR 0024 geometry; fall back to the scalar coordinate (`AssertCoordinates`)
        // so a place nobody has drawn a boundary/point for yet — the common case for GEDCOM-imported
        // or manually-geocoded places — still shows up rather than silently vanishing from the map.
        let geometry = place
            .resolved_geometry
            .as_ref()
            .map(|geometry_ref| geometry_ref.geometry.clone())
            .or_else(|| place.coordinates_point.map(PlaceGeometry::Point));
        let Some(geometry) = geometry else { continue };
        let Some(point) = geometry.representative_point() else {
            continue;
        };
        points.insert(place.id.clone(), point);
        markers.push(PlaceMarker {
            human_id: place.human_id.clone(),
            id: place.id.clone(),
            name: place
                .names
                .first()
                .map_or_else(|| place.human_id.clone(), |name| name.text.clone()),
            place_type: place.place_type.clone(),
            geometry,
        });
    }

    let mut events = Vec::new();
    for event in list_events(workspace).await? {
        let Some(place) = &event.place else { continue };
        let Some(&point) = points.get(&place.id) else { continue };
        events.push(EventPin {
            human_id: event.human_id.clone(),
            id: event.id.clone(),
            event_type: event.event_type.clone(),
            date: event.date.clone(),
            place_human_id: place.human_id.clone(),
            point,
        });
    }

    Ok(GeographySummary {
        markers,
        events,
        resolved_as_of: as_of,
    })
}

#[cfg(test)]
mod tests {
    use super::year_only_date;

    #[test]
    fn a_bare_year_sorts_before_a_more_specific_date_the_same_year() {
        // year * 10_000 with month/day both 0 — matches `dto::year_of`'s inverse (place.rs uses the
        // same convention for `resolved_as_of`'s sort key).
        let date = year_only_date(1900);
        assert_eq!(date.sort_value, 19_000_000);
    }

    #[test]
    fn a_negative_year_stays_ordered_before_ce_years() {
        let bce = year_only_date(-100);
        let ce = year_only_date(100);
        assert!(bce.sort_value < ce.sort_value);
    }
}
