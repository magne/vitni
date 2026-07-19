//! The intermediate model a Gramps XML document parses into and emits from.
//!
//! Records are addressed by their Gramps `handle` (the `hlink` target); `gramps_id` is the
//! user-facing id (e.g. `I0001`). Cross-references between records are handle strings, kept as the
//! Gramps document holds them — the plugin glue resolves them to workspace human ids. Value types
//! (names, dates, event/association kinds) come from [`genealogy_interchange`].

use genealogy_interchange::{AssociationKind, Date, EventKind, Name};

/// A parsed Gramps XML database: the records we model, each keyed by its `handle`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Database {
    /// `<person>` records.
    pub people: Vec<Person>,
    /// `<family>` records.
    pub families: Vec<Family>,
    /// Top-level `<event>` records.
    pub events: Vec<Event>,
    /// `<placeobj>` records.
    pub places: Vec<Place>,
    /// `<source>` records.
    pub sources: Vec<Source>,
    /// `<citation>` records.
    pub citations: Vec<Citation>,
    /// `<repository>` records.
    pub repositories: Vec<Repository>,
    /// `<object>` (media) records.
    pub objects: Vec<MediaObject>,
    /// `<note>` records.
    pub notes: Vec<Note>,
    /// `<tag>` records.
    pub tags: Vec<Tag>,
}

/// Biological sex as Gramps records it (`<gender>` — `M`/`F`/`U`, plus `X` for GEDCOM 7 intersex).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    /// `M`.
    Male,
    /// `F`.
    Female,
    /// `U`.
    Unknown,
    /// `X` — intersex (no native Gramps numeric code; the lossy mapping is the plugin's concern).
    Intersex,
}

/// A `<person>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Person {
    /// The internal handle (the `hlink` target).
    pub handle: String,
    /// The user-facing id (e.g. `I0001`).
    pub gramps_id: Option<String>,
    /// The primary name.
    pub name: Option<Name>,
    /// The recorded gender.
    pub gender: Option<Gender>,
    /// Events the person took part in (`<eventref>`), each with its participation payload (role, the
    /// participant-scoped attributes including `"Age"`, and note/citation refs — ADR 0019).
    pub event_refs: Vec<EventRef>,
    /// Handles of citations backing the person's claims (`<citationref>`).
    pub citation_refs: Vec<String>,
    /// Handles of attached notes (`<noteref>`).
    pub note_refs: Vec<String>,
    /// Attached media (`<objref>`), each a handle plus an optional crop region (`<region>`).
    pub media_refs: Vec<MediaRef>,
    /// Person-to-person associations (`<personref>`).
    pub person_refs: Vec<PersonRef>,
    /// The Gramps privacy flag (the `priv` attribute). Gramps has no multi-value RESN, so this maps
    /// lossily to/from the restriction set (data-model §16).
    pub private: bool,
}

/// A `<personref>`: a handle to another person and the relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonRef {
    /// The associated person's handle.
    pub hlink: String,
    /// The relationship (`rel`); `None` when unspecified.
    pub rel: Option<AssociationKind>,
}

/// An `<objref>`: a handle to a media object plus an optional crop region (`<region>`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MediaRef {
    /// The media object's handle (`<objref hlink>`).
    pub hlink: String,
    /// The crop region of interest within the media (`<region>`), if any.
    pub region: Option<Region>,
}

impl MediaRef {
    /// A bare reference to `hlink` with no region — the common whole-image case.
    #[must_use]
    pub fn bare(hlink: impl Into<String>) -> Self {
        Self {
            hlink: hlink.into(),
            region: None,
        }
    }
}

/// A crop region within a media object, as left/top/width/height percentages (0–100), mirroring
/// `genealogy_core::text::Rect`. Gramps records it in the XML as a `<region>` element carrying two
/// opposite corners (`corner1_x`/`corner1_y`, `corner2_x`/`corner2_y`) in percent; this type holds
/// the normalized top-left origin + extent the domain uses, and [`Region::from_corners`] /
/// [`Region::corners`] convert between the two conventions.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// Left edge, percent of width (0–100).
    pub left: u8,
    /// Top edge, percent of height (0–100).
    pub top: u8,
    /// Width, percent (0–100).
    pub width: u8,
    /// Height, percent (0–100).
    pub height: u8,
}

impl Region {
    /// Builds a region from two Gramps corner points (percent, any order). Each coordinate is
    /// rounded to the nearest integer and clamped to 0–100; the origin is the smaller corner and the
    /// extent is the absolute difference, so a document that lists the corners in either order (or
    /// out of bounds) yields the same well-formed region.
    #[must_use]
    pub fn from_corners(corner1_x: f64, corner1_y: f64, corner2_x: f64, corner2_y: f64) -> Self {
        let (left, width) = origin_extent(corner1_x, corner2_x);
        let (top, height) = origin_extent(corner1_y, corner2_y);
        Self {
            left,
            top,
            width,
            height,
        }
    }

    /// The two Gramps corner points `(corner1_x, corner1_y, corner2_x, corner2_y)` for this region:
    /// the top-left origin and the bottom-right corner (`origin + extent`, clamped to 100).
    #[must_use]
    pub fn corners(&self) -> (u8, u8, u8, u8) {
        (
            self.left,
            self.top,
            self.left.saturating_add(self.width).min(100),
            self.top.saturating_add(self.height).min(100),
        )
    }
}

/// Normalizes one axis of a corner pair to `(origin, extent)`: rounds and clamps both coordinates to
/// 0–100, then returns the smaller as the origin and their difference as the extent.
fn origin_extent(a: f64, b: f64) -> (u8, u8) {
    let a = clamp_percent(a);
    let b = clamp_percent(b);
    (a.min(b), a.max(b) - a.min(b))
}

/// Rounds a percentage to the nearest integer and clamps it to 0–100. Uses a `u8`-range scan rather
/// than a lossy `f64 as u8` cast: for the rounded input it returns the first percentage `>=` it,
/// which is the value itself in range and saturates to `100` above it (and `0` below).
fn clamp_percent(value: f64) -> u8 {
    let rounded = value.round();
    (0u8..=100)
        .find(|&percent| f64::from(percent) >= rounded)
        .unwrap_or(100)
}

/// An `<eventref>`: the handle of a referenced event plus this participant's payload — the Gramps
/// `role` (an `EventRoleType` string), the eventref `<attribute>`s (including the `"Age"` attribute),
/// and its note/citation refs (Gramps DTD `eventref = (attribute*, noteref*, citationref*)`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EventRef {
    /// The referenced event's handle.
    pub hlink: String,
    /// The participant's role (the `role` attribute), kept verbatim; `None` when unspecified.
    pub role: Option<String>,
    /// The eventref attributes (`<attribute type=… value=…>`), including the `"Age"` attribute.
    pub attributes: Vec<EventRefAttribute>,
    /// Handles of notes about this participation (`<noteref>`).
    pub note_refs: Vec<String>,
    /// Handles of citations backing this participation (`<citationref>`).
    pub citation_refs: Vec<String>,
}

impl EventRef {
    /// A bare reference to `hlink` with no participation payload — the common primary-participant case.
    #[must_use]
    pub fn bare(hlink: impl Into<String>) -> Self {
        Self {
            hlink: hlink.into(),
            ..Self::default()
        }
    }

    /// Whether this reference carries no payload (role / attributes / note / citation refs), so it
    /// emits as a self-closing `<eventref hlink=…/>`.
    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.role.is_none() && self.attributes.is_empty() && self.note_refs.is_empty() && self.citation_refs.is_empty()
    }
}

/// A typed key/value attribute on an `<eventref>` (`<attribute type=… value=…>`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EventRefAttribute {
    /// The attribute type (the `type` attribute, e.g. `"Age"`).
    pub attribute_type: String,
    /// The attribute value (the `value` attribute).
    pub value: String,
}

/// A `<family>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Family {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `F0001`).
    pub gramps_id: Option<String>,
    /// The father's handle (`<father>`).
    pub father: Option<String>,
    /// The mother's handle (`<mother>`).
    pub mother: Option<String>,
    /// The children (`<childref>`) with their per-parent relationships (`frel`/`mrel`).
    pub child_refs: Vec<ChildRef>,
    /// The family's events (`<eventref>`), each with its participation payload (ADR 0019).
    pub event_refs: Vec<EventRef>,
    /// The Gramps privacy flag (the `priv` attribute; lossy to/from the restriction set — §16).
    pub private: bool,
}

/// A `<childref>` in a `<family>`: the child's handle and its relationship to the mother (`mrel`)
/// and father (`frel`), where present. The relationship values are the raw Gramps strings.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChildRef {
    /// The child's handle (`<childref hlink>`).
    pub hlink: String,
    /// The child's relationship to the mother (`mrel`), if recorded.
    pub mother_relationship: Option<String>,
    /// The child's relationship to the father (`frel`), if recorded.
    pub father_relationship: Option<String>,
}

/// A top-level `<event>` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `E0001`).
    pub gramps_id: Option<String>,
    /// The kind of event (`<type>`).
    pub kind: EventKind,
    /// When it occurred (`<dateval>`/`<daterange>`/`<datespan>`/`<datestr>`).
    pub date: Option<Date>,
    /// The handle of the place it occurred (`<place hlink>`).
    pub place_ref: Option<String>,
    /// A free-text description (`<description>`).
    pub description: Option<String>,
}

/// A `<placeobj>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Place {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `P0001`).
    pub gramps_id: Option<String>,
    /// The primary place name (`<pname value>`).
    pub name: Option<String>,
    /// The place type (the `type` attribute), kept verbatim.
    pub place_type: Option<String>,
    /// Handles of enclosing places (`<placeref>`), the hierarchy chain.
    pub enclosed_by: Vec<String>,
}

/// A `<source>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Source {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `S0001`).
    pub gramps_id: Option<String>,
    /// The title (`<stitle>`).
    pub title: Option<String>,
    /// The author (`<sauthor>`).
    pub author: Option<String>,
    /// Publication info (`<spubinfo>`).
    pub pub_info: Option<String>,
    /// Handles of linked repositories (`<reporef>`).
    pub repository_refs: Vec<String>,
}

/// A `<citation>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Citation {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `C0001`).
    pub gramps_id: Option<String>,
    /// The handle of the cited source (`<sourceref>`).
    pub source_ref: Option<String>,
    /// The page locator (`<page>`).
    pub page: Option<String>,
    /// The confidence (`<confidence>`, 0–4); `None` when unspecified.
    pub confidence: Option<u8>,
}

/// A `<repository>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Repository {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `R0001`).
    pub gramps_id: Option<String>,
    /// The repository name (`<rname>`).
    pub name: Option<String>,
}

/// An `<object>` (media) record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MediaObject {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `O0001`).
    pub gramps_id: Option<String>,
    /// The file path / URL (`<file src>`).
    pub file: Option<String>,
    /// The MIME type (`<file mime>`).
    pub mime: Option<String>,
}

/// A `<note>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Note {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `N0001`).
    pub gramps_id: Option<String>,
    /// The note text (`<text>`).
    pub text: Option<String>,
}

/// A `<tag>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tag {
    /// The internal handle.
    pub handle: String,
    /// The tag name (the `name` attribute).
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::Region;

    #[test]
    fn from_corners_uses_top_left_origin_and_extent() {
        let region = Region::from_corners(10.0, 20.0, 60.0, 80.0);
        assert_eq!(
            region,
            Region {
                left: 10,
                top: 20,
                width: 50,
                height: 60
            }
        );
    }

    #[test]
    fn from_corners_normalizes_reversed_corners() {
        // corner2 above/left of corner1 — Gramps allows either order; the region is the same box.
        let reversed = Region::from_corners(60.0, 80.0, 10.0, 20.0);
        assert_eq!(reversed, Region::from_corners(10.0, 20.0, 60.0, 80.0));
    }

    #[test]
    fn from_corners_rounds_and_clamps_to_percent() {
        let region = Region::from_corners(-5.0, 10.4, 120.0, 99.6);
        assert_eq!(
            region,
            Region {
                left: 0,
                top: 10,
                width: 100,
                height: 90
            }
        );
    }

    #[test]
    fn corners_round_trip_through_from_corners() {
        let region = Region {
            left: 12,
            top: 34,
            width: 40,
            height: 25,
        };
        let (c1x, c1y, c2x, c2y) = region.corners();
        assert_eq!((c1x, c1y, c2x, c2y), (12, 34, 52, 59));
        assert_eq!(
            Region::from_corners(f64::from(c1x), f64::from(c1y), f64::from(c2x), f64::from(c2y)),
            region
        );
    }

    #[test]
    fn corners_clamp_extent_at_the_far_edge() {
        let region = Region {
            left: 80,
            top: 80,
            width: 40,
            height: 40,
        };
        assert_eq!(region.corners(), (80, 80, 100, 100));
    }
}
