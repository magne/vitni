use super::{
    CitationRefVm, ConfidenceLevel, DetailTab, FamilyMediaVm, HistoryEntryVm, Localizer, PlaceChangeSetRequest,
    RestrictionKind, RowVm, TagRef, citation_ref_from_ref, non_blank,
};

/// One asserted place name (Names tab): text, language, date, surety, and source count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceNameVm {
    /// The name text.
    pub text: String,
    /// The BCP-47 language tag, if recorded.
    pub language: Option<String>,
    /// The localized date the name was in use, if known.
    pub date: Option<String>,
    /// The operator's surety in the name assertion (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the name assertion.
    pub source_count: usize,
}

/// One enclosing place (Hierarchy tab): the place, its type, the dated link, and surety.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceHierarchyVm {
    /// The enclosing place's user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The enclosing place's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The enclosing place's display name (falls back to the `human_id`).
    pub name: String,
    /// The enclosing place's localized type label, if resolved.
    pub type_label: Option<String>,
    /// The localized dated link (when the enclosing relationship was valid), if dated.
    pub date: Option<String>,
    /// The operator's surety in the enclosing-by assertion (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
}

/// A place's detail view — type/coordinates/code facts, name history, the jurisdiction chain,
/// citations, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceDetail {
    /// The user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The stable `PlaceId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the place's primary name (falls back to the `human_id`).
    pub title: String,
    /// The localized place-type label, if set.
    pub type_label: Option<String>,
    /// The place's coordinates rendered as `lat, long`, if asserted.
    pub coordinates: Option<String>,
    /// The operator's surety in the coordinates, if asserted.
    pub coordinates_confidence: Option<ConfidenceLevel>,
    /// The localized coordinates confidence label, if asserted.
    pub coordinates_confidence_label: Option<String>,
    /// The coordinate assertion's citations, for the provenance popover.
    pub coordinate_citations: Vec<CitationRefVm>,
    /// The place's code, if set.
    pub code: Option<String>,
    /// The asserted names, with language/date + surety.
    pub names: Vec<PlaceNameVm>,
    /// The jurisdiction chain (enclosing places), nearest first.
    pub hierarchy: Vec<PlaceHierarchyVm>,
    /// The citations backing the place, with source · page · surety · evidence axes.
    pub citations: Vec<CitationRefVm>,
    /// The attached media objects.
    pub media: Vec<FamilyMediaVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The place's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The place's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl PlaceDetail {
    /// Builds a detail view from a [`PlaceSummary`](genealogy_app::PlaceSummary), localizing labels,
    /// dates, and confidence. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::PlaceSummary, loc: &Localizer) -> Self {
        let coordinates_confidence = summary.coordinates_confidence.map(ConfidenceLevel::from);
        let names = summary
            .names
            .iter()
            .map(|name| {
                let confidence = ConfidenceLevel::from(name.confidence);
                PlaceNameVm {
                    text: name.text.clone(),
                    language: name.language.clone(),
                    date: name.date.as_ref().map(|date| loc.date(date)),
                    confidence,
                    confidence_label: loc.confidence_label(confidence),
                    source_count: name.source_count,
                }
            })
            .collect();
        let hierarchy = summary
            .enclosing
            .iter()
            .map(|enclosing| {
                let confidence = ConfidenceLevel::from(enclosing.confidence);
                PlaceHierarchyVm {
                    human_id: enclosing.human_id.clone(),
                    id: enclosing.id.clone(),
                    name: enclosing.name.clone().unwrap_or_else(|| enclosing.human_id.clone()),
                    type_label: enclosing.place_type.as_ref().map(|t| loc.place_type_label(t)),
                    date: enclosing.date.as_ref().map(|date| loc.date(date)),
                    confidence,
                    confidence_label: loc.confidence_label(confidence),
                }
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: place_title(summary),
            type_label: summary.place_type.as_ref().map(|t| loc.place_type_label(t)),
            coordinates: summary.coordinates.clone(),
            coordinates_confidence,
            coordinates_confidence_label: coordinates_confidence.map(|level| loc.confidence_label(level)),
            coordinate_citations: summary
                .coordinate_citations
                .iter()
                .map(|c| citation_ref_from_ref(c, loc))
                .collect(),
            code: summary.code.clone(),
            names,
            hierarchy,
            citations: summary
                .citations
                .iter()
                .map(|citation| citation_ref_from_ref(citation, loc))
                .collect(),
            media: summary
                .media
                .iter()
                .map(|media| FamilyMediaVm {
                    human_id: media.human_id.clone(),
                    caption: media.caption.clone(),
                })
                .collect(),
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// The place's primary name for the header (its first asserted name), or the `human_id` fallback.
fn place_title(summary: &genealogy_app::PlaceSummary) -> String {
    summary
        .names
        .first()
        .map_or_else(|| summary.human_id.clone(), |name| name.text.clone())
}

/// Builds a generic list row from a [`PlaceSummary`](genealogy_app::PlaceSummary): the primary name,
/// a `type · enclosing` subtitle, and a per-type avatar.
#[must_use]
pub fn place_row(summary: &genealogy_app::PlaceSummary, loc: &Localizer) -> RowVm {
    let type_label = summary.place_type.as_ref().map(|t| loc.place_type_label(t));
    let enclosing = summary
        .enclosing
        .first()
        .map(|e| e.name.clone().unwrap_or_else(|| e.human_id.clone()));
    let subtitle = match (type_label, enclosing) {
        (Some(type_label), Some(enclosing)) => Some(format!("{type_label} · {enclosing}")),
        (Some(type_label), None) => Some(type_label),
        (None, Some(enclosing)) => Some(enclosing),
        (None, None) => None,
    };
    RowVm {
        id: summary.human_id.clone(),
        title: place_title(summary),
        subtitle,
        avatar: Some(place_avatar(summary.place_type.as_ref())),
        ..RowVm::default()
    }
}

/// The decorative avatar glyph for a place row, by type (a generic pin otherwise).
fn place_avatar(place_type: Option<&genealogy_app::PlaceType>) -> String {
    use genealogy_app::PlaceType;
    match place_type {
        Some(PlaceType::Parish) => "⛪",
        _ => "📍",
    }
    .to_owned()
}

/// The tab strip for a place's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn place_tabs(detail: &PlaceDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("names", Some(detail.names.len())),
        tab("hierarchy", Some(detail.hierarchy.len())),
        tab("citations", Some(detail.citations.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// The default place type a fresh create draft starts with (matching the mockup's Type select).
const DEFAULT_PLACE_TYPE: genealogy_app::PlaceType = genealogy_app::PlaceType::City;

/// The parse outcome of the coordinate pair (both-or-neither): unset, a parsed point, or invalid.
enum Coordinates {
    /// Both latitude and longitude are blank — no coordinates asserted.
    Unset,
    /// Both parse to a point.
    Point(genealogy_app::GeoCoordinates),
    /// One is filled and the other blank, or a non-blank value does not parse.
    Invalid,
}

/// The create form's in-memory draft for a new place (`record-editing.html` §6): a required type plus
/// an optional name, coordinate pair (raw decimal-degree strings), and code. Latitude/longitude are
/// held as raw text and parsed both-or-neither at the boundary (`§7`); an unparseable or half-filled
/// pair blocks Save. Create-only; nothing is written until Save commits a [`PlaceChangeSetRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceDraft {
    /// The place type (required).
    pub place_type: genealogy_app::PlaceType,
    /// The place's primary name.
    pub name: String,
    /// The latitude as raw decimal-degree text.
    pub latitude: String,
    /// The longitude as raw decimal-degree text.
    pub longitude: String,
    /// The place's code.
    pub code: String,
}

impl Default for PlaceDraft {
    fn default() -> Self {
        Self {
            place_type: DEFAULT_PLACE_TYPE,
            name: String::new(),
            latitude: String::new(),
            longitude: String::new(),
            code: String::new(),
        }
    }
}

impl PlaceDraft {
    /// A fresh draft for creating a new place (default type, empty fields).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The parsed coordinate pair: unset (both blank), a point (both parse), or invalid.
    fn coordinates(&self) -> Coordinates {
        let latitude = self.latitude.trim();
        let longitude = self.longitude.trim();
        match (latitude.is_empty(), longitude.is_empty()) {
            (true, true) => Coordinates::Unset,
            (false, false) => match (
                latitude.parse::<genealogy_app::Microdegrees>(),
                longitude.parse::<genealogy_app::Microdegrees>(),
            ) {
                (Ok(latitude), Ok(longitude)) => {
                    Coordinates::Point(genealogy_app::GeoCoordinates { latitude, longitude })
                }
                _ => Coordinates::Invalid,
            },
            _ => Coordinates::Invalid,
        }
    }

    /// Whether the latitude field is invalid (drives `aria-invalid` + its field error): a non-blank
    /// value that does not parse, or a blank value while longitude is filled.
    #[must_use]
    pub fn latitude_invalid(&self) -> bool {
        let latitude = self.latitude.trim();
        if latitude.is_empty() {
            return !self.longitude.trim().is_empty();
        }
        latitude.parse::<genealogy_app::Microdegrees>().is_err()
    }

    /// Whether the longitude field is invalid (mirror of [`Self::latitude_invalid`]).
    #[must_use]
    pub fn longitude_invalid(&self) -> bool {
        let longitude = self.longitude.trim();
        if longitude.is_empty() {
            return !self.latitude.trim().is_empty();
        }
        longitude.parse::<genealogy_app::Microdegrees>().is_err()
    }

    /// Whether every field is valid — the coordinate pair is not half-filled or unparseable.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !matches!(self.coordinates(), Coordinates::Invalid)
    }

    /// Whether the operator has entered anything beyond the default type — the Save gate (with
    /// [`Self::is_valid`]).
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.place_type != DEFAULT_PLACE_TYPE
            || non_blank(&self.name).is_some()
            || non_blank(&self.latitude).is_some()
            || non_blank(&self.longitude).is_some()
            || non_blank(&self.code).is_some()
    }

    /// Builds the [`PlaceChangeSetRequest`] the app commits on Save, or `None` when the coordinate
    /// pair is invalid (so Save is a no-op rather than committing a partial place).
    #[must_use]
    pub fn to_request(&self) -> Option<PlaceChangeSetRequest> {
        let coordinates = match self.coordinates() {
            Coordinates::Unset => None,
            Coordinates::Point(point) => Some(point),
            Coordinates::Invalid => return None,
        };
        Some(PlaceChangeSetRequest {
            place_type: self.place_type.clone(),
            name: non_blank(&self.name),
            coordinates,
            code: non_blank(&self.code),
        })
    }
}

#[cfg(test)]
mod place_draft_tests {
    use super::PlaceDraft;
    use genealogy_app::PlaceType;

    #[test]
    fn a_fresh_draft_is_valid_but_not_dirty() {
        let draft = PlaceDraft::new();
        assert!(draft.is_valid());
        assert!(!draft.is_dirty(), "a bare default draft leaves Save disabled");
    }

    #[test]
    fn a_name_or_a_changed_type_makes_it_dirty() {
        assert!(
            PlaceDraft {
                name: "Oslo".to_owned(),
                ..PlaceDraft::new()
            }
            .is_dirty()
        );
        assert!(
            PlaceDraft {
                place_type: PlaceType::Country,
                ..PlaceDraft::new()
            }
            .is_dirty()
        );
    }

    #[test]
    fn both_coordinates_must_parse_or_the_draft_is_invalid() {
        let bad = PlaceDraft {
            latitude: "not-a-number".to_owned(),
            longitude: "10.0".to_owned(),
            ..PlaceDraft::new()
        };
        assert!(!bad.is_valid());
        assert!(bad.latitude_invalid());
        assert!(!bad.longitude_invalid());
        assert!(bad.to_request().is_none(), "an invalid pair yields no request");
    }

    #[test]
    fn a_half_filled_pair_flags_the_blank_field() {
        let half = PlaceDraft {
            latitude: "59.9".to_owned(),
            ..PlaceDraft::new()
        };
        assert!(!half.is_valid());
        assert!(half.longitude_invalid(), "the blank longitude is flagged");
        assert!(!half.latitude_invalid());
    }

    #[test]
    fn both_blank_is_valid_and_yields_no_coordinates() {
        let request = PlaceDraft::new().to_request().expect("valid");
        assert_eq!(request.coordinates, None);
    }

    #[test]
    fn a_valid_pair_parses_into_the_request() {
        let draft = PlaceDraft {
            latitude: "40.7128".to_owned(),
            longitude: "-74.006".to_owned(),
            ..PlaceDraft::new()
        };
        assert!(draft.is_valid());
        assert!(draft.to_request().expect("valid").coordinates.is_some());
    }
}
