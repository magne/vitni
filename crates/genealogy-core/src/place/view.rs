//! [`PlaceView`] — the conclusion-layer read model for a Place (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (it delegates to `evolve`). The denormalized
//! SQL read schema is deferred (ADR 0009); the view exposes its projected fields through accessors
//! over the folded state.

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::{Asserted, Attributed};
use crate::enums::{PlaceType, Restriction};
use crate::geo::GeoCoordinates;
use crate::ids::{CitationId, HumanId, NoteId, PlaceId, TagId};
use crate::place::decide::evolve;
use crate::place::state::PlaceState;
use crate::place_geometry::PlaceGeometryAssertion;
use crate::place_name::PlaceName;
use crate::place_ref::PlaceRef;
use crate::place_succession::PlaceSuccessionAssertion;
use crate::temporal::resolve_as_of;
use crate::text::MediaRef;

/// The current best synthesis of a Place, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceView {
    state: PlaceState,
}

impl PlaceView {
    /// Returns `true` once the place has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The place's id, once created.
    #[must_use]
    pub fn place_id(&self) -> Option<PlaceId> {
        self.state.place_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// The place's type.
    #[must_use]
    pub fn place_type(&self) -> Option<&PlaceType> {
        self.state.place_type.as_ref().map(|t| &t.value.value)
    }

    /// The place's type with its provenance (surety + backing citations), if set.
    #[must_use]
    pub fn asserted_place_type(&self) -> Option<&Asserted<PlaceType>> {
        self.state.place_type.as_ref().map(|t| &t.value)
    }

    /// All currently-live asserted names, in assertion order.
    #[must_use]
    pub fn names(&self) -> Vec<&PlaceName> {
        self.state.names.iter().map(|n| &n.value.value).collect()
    }

    /// All currently-live asserted names with their provenance, in assertion order.
    #[must_use]
    pub fn asserted_names(&self) -> Vec<&Asserted<PlaceName>> {
        self.state.names.iter().map(|n| &n.value).collect()
    }

    /// All currently-live enclosing-place relationships, in assertion order.
    #[must_use]
    pub fn enclosed_by(&self) -> Vec<&PlaceRef> {
        self.state.enclosed_by.iter().map(|e| &e.value.value).collect()
    }

    /// All currently-live enclosing-place relationships with their provenance, in assertion order.
    #[must_use]
    pub fn asserted_enclosed_by(&self) -> Vec<&Asserted<PlaceRef>> {
        self.state.enclosed_by.iter().map(|e| &e.value).collect()
    }

    /// The place's coordinates, if asserted.
    #[must_use]
    pub fn coordinates(&self) -> Option<&GeoCoordinates> {
        self.state.coordinates.as_ref().map(|c| &c.value.value)
    }

    /// The place's coordinates with their provenance (surety + backing citations), if asserted.
    #[must_use]
    pub fn asserted_coordinates(&self) -> Option<&Asserted<GeoCoordinates>> {
        self.state.coordinates.as_ref().map(|c| &c.value)
    }

    /// The place's coordinates paired with the `AssertionId` that introduced them, if asserted — the
    /// stable key a spatial index keys its undated `Point` row on (ADR 0024 §3).
    #[must_use]
    pub fn coordinates_with_assertion(&self) -> Option<&Attributed<Asserted<GeoCoordinates>>> {
        self.state.coordinates.as_ref()
    }

    /// All currently-live dated geometry assertions, in assertion order (ADR 0024). These accumulate
    /// rather than replace, unlike `coordinates` above.
    #[must_use]
    pub fn geometries(&self) -> Vec<&PlaceGeometryAssertion> {
        self.state.geometries.iter().map(|g| &g.value.value).collect()
    }

    /// All currently-live geometry assertions with their provenance, in assertion order.
    #[must_use]
    pub fn asserted_geometries(&self) -> Vec<&Asserted<PlaceGeometryAssertion>> {
        self.state.geometries.iter().map(|g| &g.value).collect()
    }

    /// Currently-live geometry assertions, each paired with its introducing `AssertionId` — the
    /// stable key a spatial index keys each dated shape's row on (ADR 0024 §3).
    #[must_use]
    pub fn geometries_with_assertions(&self) -> &[Attributed<Asserted<PlaceGeometryAssertion>>] {
        &self.state.geometries
    }

    /// All currently-live succession assertions this place took part in, in assertion order (ADR
    /// 0026). These accumulate rather than replace, like `geometries` above.
    #[must_use]
    pub fn successions(&self) -> Vec<&PlaceSuccessionAssertion> {
        self.state.successions.iter().map(|s| &s.value.value).collect()
    }

    /// Currently-live succession assertions, each paired with its introducing `AssertionId` — the
    /// stable key a per-row Edit supersedes and a Retract retracts.
    #[must_use]
    pub fn successions_with_assertions(&self) -> &[Attributed<Asserted<PlaceSuccessionAssertion>>] {
        &self.state.successions
    }

    /// The enclosing-place link in effect **as of** `target_sort_value` — the latest dated link
    /// whose date's `sort_value` is `<= target_sort_value`, or the first undated ("primary") link
    /// (ADR 0026 §1, the resolution rule every dated Place read shares). `None` when neither exists.
    #[must_use]
    pub fn enclosed_by_as_of(&self, target_sort_value: i64) -> Option<&Attributed<Asserted<PlaceRef>>> {
        resolve_as_of(self.state.enclosed_by.iter(), target_sort_value, |link| {
            link.value.value.date.as_ref().map(|d| d.sort_value)
        })
    }

    /// The **primary** enclosing-place link — the first asserted, used when no date context is
    /// available (ADR 0026 §1; the issues.md "primary (first) `PlaceRef`" convention).
    #[must_use]
    pub fn primary_enclosed_by(&self) -> Option<&Attributed<Asserted<PlaceRef>>> {
        self.state.enclosed_by.first()
    }

    /// The name in effect **as of** `target_sort_value`, by the same resolution rule as
    /// [`Self::enclosed_by_as_of`] (ADR 0026 §1) — drives the generated place title.
    #[must_use]
    pub fn name_as_of(&self, target_sort_value: i64) -> Option<&Attributed<Asserted<PlaceName>>> {
        resolve_as_of(self.state.names.iter(), target_sort_value, |name| {
            name.value.value.date.as_ref().map(|d| d.sort_value)
        })
    }

    /// The geometry in effect **as of** `target_sort_value`, by the same resolution rule (ADR 0026
    /// §1) — the entry point the geography view's time slider (ADR 0025) will use.
    #[must_use]
    pub fn geometry_as_of(&self, target_sort_value: i64) -> Option<&Attributed<Asserted<PlaceGeometryAssertion>>> {
        resolve_as_of(self.state.geometries.iter(), target_sort_value, |geometry| {
            geometry.value.value.date.as_ref().map(|d| d.sort_value)
        })
    }

    /// The place's code, if set.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.state.code.as_ref().map(|c| c.value.value.as_str())
    }

    /// The place's code with its provenance (surety + backing citations), if set.
    #[must_use]
    pub fn asserted_code(&self) -> Option<&Asserted<String>> {
        self.state.code.as_ref().map(|c| &c.value)
    }

    /// All currently-live citations backing the place's claims, in assertion order.
    #[must_use]
    pub fn citations(&self) -> Vec<CitationId> {
        self.state.citations.iter().map(|c| c.value).collect()
    }

    /// All currently-live attached media, in assertion order.
    #[must_use]
    pub fn media(&self) -> Vec<&MediaRef> {
        self.state.media.iter().map(|m| &m.value).collect()
    }

    /// All currently-live attached notes, in assertion order.
    #[must_use]
    pub fn notes(&self) -> Vec<NoteId> {
        self.state.notes.iter().map(|n| n.value).collect()
    }

    /// All currently-applied tags, in assertion order.
    #[must_use]
    pub fn tags(&self) -> Vec<TagId> {
        self.state.tags.iter().map(|t| t.value).collect()
    }

    /// The place's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }

    /// Currently-live names, each paired with the `AssertionId` that introduced it — the read side of
    /// the per-row correction (Edit supersedes it, Remove retracts it).
    #[must_use]
    pub fn names_with_assertions(&self) -> &[Attributed<Asserted<PlaceName>>] {
        &self.state.names
    }

    /// Currently-live enclosing-place relationships, each paired with its introducing `AssertionId`.
    #[must_use]
    pub fn enclosed_by_with_assertions(&self) -> &[Attributed<Asserted<PlaceRef>>] {
        &self.state.enclosed_by
    }

    /// Currently-live citations, each paired with its introducing `AssertionId`.
    #[must_use]
    pub fn citations_with_assertions(&self) -> &[Attributed<CitationId>] {
        &self.state.citations
    }

    /// Currently-live attached media, each paired with the attach `AssertionId` (the detach target).
    #[must_use]
    pub fn media_with_assertions(&self) -> &[Attributed<MediaRef>] {
        &self.state.media
    }

    /// Currently-live attached notes, each paired with the attach `AssertionId` (the detach target).
    #[must_use]
    pub fn notes_with_assertions(&self) -> &[Attributed<NoteId>] {
        &self.state.notes
    }
}

impl View<PlaceState> for PlaceView {
    fn update(&mut self, event: &EventEnvelope<PlaceState>) {
        evolve(&mut self.state, &event.payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assertions::Attributed;
    use crate::ids::AssertionId;
    use uuid::Uuid;

    use crate::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};

    /// A minimal dated `GenealogicalDate` carrying only the `sort_value` the resolution rule reads.
    fn dated(sort_value: i64) -> GenealogicalDate {
        GenealogicalDate {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
                year: None,
                month: None,
                day: None,
            })),
            time: None,
            new_year_begins: None,
            sort_value,
            original_text: None,
        }
    }

    fn enclosed_by_link(assertion: u128, date: Option<GenealogicalDate>) -> Attributed<Asserted<PlaceRef>> {
        Attributed {
            assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
            value: Asserted {
                value: PlaceRef {
                    place_id: PlaceId::from_uuid(Uuid::from_u128(assertion)),
                    date,
                },
                confidence: None,
                citations: Vec::new(),
            },
        }
    }

    #[test]
    fn enclosed_by_as_of_picks_the_latest_dated_link_at_or_before_the_target() {
        let state = PlaceState {
            enclosed_by: vec![
                enclosed_by_link(1, Some(dated(1801))),
                enclosed_by_link(2, Some(dated(1900))),
            ],
            ..Default::default()
        };
        let view = PlaceView { state };
        let resolved = view.enclosed_by_as_of(1920).expect("a resolved link");
        assert_eq!(resolved.value.value.place_id, PlaceId::from_uuid(Uuid::from_u128(2)));
    }

    #[test]
    fn enclosed_by_as_of_falls_back_to_the_undated_link() {
        let state = PlaceState {
            enclosed_by: vec![enclosed_by_link(1, None), enclosed_by_link(2, Some(dated(1950)))],
            ..Default::default()
        };
        let view = PlaceView { state };
        let resolved = view.enclosed_by_as_of(1900).expect("a resolved link");
        assert_eq!(resolved.value.value.place_id, PlaceId::from_uuid(Uuid::from_u128(1)));
    }

    #[test]
    fn primary_enclosed_by_is_the_first_asserted_link() {
        let state = PlaceState {
            enclosed_by: vec![enclosed_by_link(1, None), enclosed_by_link(2, None)],
            ..Default::default()
        };
        let view = PlaceView { state };
        let primary = view.primary_enclosed_by().expect("a primary link");
        assert_eq!(primary.value.value.place_id, PlaceId::from_uuid(Uuid::from_u128(1)));
    }

    fn named_at(assertion: u128, text: &str, date: Option<GenealogicalDate>) -> Attributed<Asserted<PlaceName>> {
        Attributed {
            assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
            value: Asserted {
                value: PlaceName {
                    text: text.to_owned(),
                    language: None,
                    date,
                },
                confidence: None,
                citations: Vec::new(),
            },
        }
    }

    /// The ADR 0026 §1 name-resolution example: Kristiania was renamed Oslo in 1925 — a query for an
    /// 1875 record must resolve to "Kristiania", one for 1950 to "Oslo".
    #[test]
    fn name_as_of_resolves_kristiania_before_1925_and_oslo_after() {
        let state = PlaceState {
            names: vec![
                named_at(1, "Kristiania", Some(dated(1877))),
                named_at(2, "Oslo", Some(dated(1925))),
            ],
            ..Default::default()
        };
        let view = PlaceView { state };
        assert_eq!(view.name_as_of(1900).expect("a name").value.value.text, "Kristiania");
        assert_eq!(view.name_as_of(1950).expect("a name").value.value.text, "Oslo");
    }

    #[test]
    fn geometry_as_of_picks_the_boundary_in_effect_at_the_target_year() {
        let point = |lat, lon| crate::geo::GeoCoordinates {
            latitude: crate::geo::Microdegrees::from_microdegrees(lat),
            longitude: crate::geo::Microdegrees::from_microdegrees(lon),
        };
        let geometry_at = |assertion: u128, lat: i32, date: GenealogicalDate| Attributed {
            assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
            value: Asserted {
                value: PlaceGeometryAssertion {
                    geometry: crate::geo::PlaceGeometry::Point(point(lat, 5_000_000)),
                    date: Some(date),
                },
                confidence: None,
                citations: Vec::new(),
            },
        };
        let state = PlaceState {
            geometries: vec![
                geometry_at(1, 60_000_000, dated(1801)),
                geometry_at(2, 61_000_000, dated(1900)),
            ],
            ..Default::default()
        };
        let view = PlaceView { state };
        let resolved = view.geometry_as_of(1850).expect("a geometry");
        assert_eq!(resolved.assertion_id, AssertionId::from_uuid(Uuid::from_u128(1)));
    }

    #[test]
    fn notes_with_assertions_exposes_the_attach_assertion() {
        let aid = AssertionId::from_uuid(Uuid::from_u128(7));
        let note = crate::ids::NoteId::from_uuid(Uuid::from_u128(8));
        let state = PlaceState {
            notes: vec![Attributed {
                assertion_id: aid,
                value: note,
            }],
            ..Default::default()
        };
        let view = PlaceView { state };
        assert_eq!(view.notes_with_assertions()[0].assertion_id, aid);
    }
}
