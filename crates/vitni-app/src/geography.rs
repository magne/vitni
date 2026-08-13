//! Geography use-cases (ADR 0025 §1): the read-only feed the framework-free map view-model
//! (`vitni-ui`) renders — every place with a resolved geometry (an ADR 0024 assertion, or the
//! scalar `AssertCoordinates` point when no dedicated geometry has been drawn) becomes a marker,
//! plus event pins at those places, plus every other place reported unplotted with why (#256) —
//! resolved **as of** an optional year (ADR 0026 §1) for the time slider.
//!
//! v1 loads every place with [`list_places`]/[`list_places_as_of`], mirroring the existing Places
//! list screen. Wiring the viewport through [`vitni_db::Store::places_in_bbox`] (ADR 0024 §3) to
//! skip summarizing places outside the current map view is a deferred follow-up once dataset scale
//! needs it — the Postgres mirror has no spatial index yet either (ADR 0024 §3's own deferral), so a
//! viewport-aware `show_geography` would need to degrade gracefully on that backend regardless.

use vitni_core::date::GenealogicalDate;
use vitni_core::enums::{EventType, PlaceType};
use vitni_core::geo::{GeoCoordinates, PlaceGeometry};

use crate::error::AppError;
use crate::event::{DateParts, EventSummary, gregorian_date, list_events};
use crate::place::{PlaceSummary, list_places, list_places_as_of};
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
/// Every field is an `Eq`-safe type (fixed-point `Microdegrees`, not `f64` — `crate::geo`'s own
/// rationale) so this can sit in `PlaceSummary`, which derives `Eq`.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Why an [`UnplottedPlace`] has no marker (#256): distinguishes "this place has geometry, just not
/// one that resolves as of the feed's year" from "this place has never been located at all", so the
/// rail can caption each row instead of implying every unplotted place once had a location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnplottedReason {
    /// The place holds one or more geometry assertions, but every one is dated after the feed's
    /// year (ADR 0026 §1) and none is undated/primary — nothing resolves as of `as_of`.
    DatedLater,
    /// The place has never been located: no geometry assertion, and no scalar `AssertCoordinates`
    /// point either.
    NoGeometry,
}

/// A place with no marker as of the feed's year (#256) — either it holds geometry that does not
/// resolve as of that year, or it has never been located at all. Reported rather than dropped so
/// the rail can list every place and say why each unlocated one is missing, instead of the list
/// silently shrinking as the slider moves or omitting places nobody has ever located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnplottedPlace {
    /// The place's user-facing identifier (e.g. `P0001`).
    pub human_id: String,
    /// The place's stable `PlaceId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The place's display name (falls back to the `human_id`).
    pub name: String,
    /// The place's type, if set. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub place_type: Option<PlaceType>,
    /// Why this place has no marker.
    pub reason: UnplottedReason,
}

/// The geography view's data feed (ADR 0025 §1): every resolved place marker and event pin, every
/// place with no marker as of the feed's year (#256), plus the date they were resolved **as of**
/// (echoed for the time-slider caption, mirroring [`crate::place::PlaceSummary::resolved_as_of`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeographySummary {
    /// Every place with a resolved geometry, ready to plot.
    pub markers: Vec<PlaceMarker>,
    /// Every event whose place resolved a geometry, ready to pin.
    pub events: Vec<EventPin>,
    /// Every place with no marker as of this feed's year — dated-later geometry or never located.
    pub unplotted: Vec<UnplottedPlace>,
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

/// The point a place's marker plots at: its dated ADR 0024 geometry's representative point, falling
/// back to the scalar `AssertCoordinates` point when it has none (the same fallback `show_geography`'s
/// marker loop applies) — shared by that loop and [`crate::place::show_place`]'s own single-place
/// event-pin filter, so a place's Map tab pins its events at the exact point the Geography atlas would.
pub(crate) fn place_point(place: &PlaceSummary) -> Option<GeoCoordinates> {
    place
        .resolved_geometry
        .as_ref()
        .map(|geometry_ref| geometry_ref.geometry.clone())
        .or_else(|| place.coordinates_point.map(PlaceGeometry::Point))
        .and_then(|geometry| geometry.representative_point())
}

/// A place's display name for the map rail and the unplotted report: its own name resolved **as
/// of** the feed's year (ADR 0026 §1) — the same resolution the marker's geometry uses, so a
/// renamed place's pin tracks the slider instead of always showing the first-asserted name. Falls
/// back to the `human_id` when the place has no name at all.
fn place_display_name(place: &PlaceSummary) -> String {
    place.resolved_name.clone().unwrap_or_else(|| place.human_id.clone())
}

/// Builds one [`EventPin`] per event whose place resolved a point in `points` (keyed by the place's
/// stable id) — shared by [`show_geography`]'s workspace-wide feed and [`crate::place::show_place`]'s
/// single-place filter (pass a `points` map holding just the one place to pin only its own events).
pub(crate) fn build_event_pins(
    events: &[EventSummary],
    points: &std::collections::HashMap<String, GeoCoordinates>,
) -> Vec<EventPin> {
    let mut pins = Vec::new();
    for event in events {
        let Some(place) = &event.place else { continue };
        let Some(&point) = points.get(&place.id) else { continue };
        pins.push(EventPin {
            human_id: event.human_id.clone(),
            id: event.id.clone(),
            event_type: event.event_type.clone(),
            date: event.date.clone(),
            place_human_id: place.human_id.clone(),
            point,
        });
    }
    pins
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
    let mut unplotted = Vec::new();
    for place in &places {
        // Prefer the dated ADR 0024 geometry; fall back to the scalar coordinate (`AssertCoordinates`)
        // so a place nobody has drawn a boundary/point for yet — the common case for GEDCOM-imported
        // or manually-geocoded places — still shows up rather than silently vanishing from the map.
        let geometry = place
            .resolved_geometry
            .as_ref()
            .map(|geometry_ref| geometry_ref.geometry.clone())
            .or_else(|| place.coordinates_point.map(PlaceGeometry::Point));
        let plotted = geometry.and_then(|geometry| geometry.representative_point().map(|point| (geometry, point)));
        let Some((geometry, point)) = plotted else {
            // Nothing to plot: either every geometry assertion is dated after `as_of` with none
            // undated, or the place was never located at all. Reported either way (#256) so the
            // rail can list every place instead of the marker-only set silently shrinking as the
            // slider moves or omitting places that were never locatable.
            let reason = if place.geometries.is_empty() {
                UnplottedReason::NoGeometry
            } else {
                UnplottedReason::DatedLater
            };
            unplotted.push(UnplottedPlace {
                human_id: place.human_id.clone(),
                id: place.id.clone(),
                name: place_display_name(place),
                place_type: place.place_type.clone(),
                reason,
            });
            continue;
        };
        points.insert(place.id.clone(), point);
        markers.push(PlaceMarker {
            human_id: place.human_id.clone(),
            id: place.id.clone(),
            name: place_display_name(place),
            place_type: place.place_type.clone(),
            geometry,
        });
    }

    let events = build_event_pins(&list_events(workspace).await?, &points);

    Ok(GeographySummary {
        markers,
        events,
        unplotted,
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
