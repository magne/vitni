use super::{
    AttachedRefVm, CitationRefVm, ConfidenceLevel, DetailTab, FamilyMediaVm, HistoryEntryVm, Localizer,
    PlaceChangeSetRequest, PlaceEdit, RecordDraft, RestrictionKind, RowVm, TagRef, citation_ref_from_ref, non_blank,
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
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the name assertion.
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) that introduced this name — the target a per-row Edit
    /// supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
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
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// The `AssertionId` (a UUID string) that introduced this enclosing-by link — the target a
    /// per-row Edit supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
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
    /// The place's raw type, if set (seeds the whole-record editor's Type select).
    pub place_type: Option<genealogy_app::PlaceType>,
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
    /// The attached notes, each with its attach `AssertionId` (the Detach target).
    pub notes: Vec<AttachedRefVm>,
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
                let confidence = name.confidence.map(ConfidenceLevel::from);
                PlaceNameVm {
                    text: name.text.clone(),
                    language: name.language.clone(),
                    date: name.date.as_ref().map(|date| loc.date(date)),
                    confidence,
                    confidence_label: loc.confidence_label_opt(confidence),
                    source_count: name.source_count,
                    assertion_id: name.assertion_id.clone(),
                }
            })
            .collect();
        let hierarchy = summary
            .enclosing
            .iter()
            .map(|enclosing| {
                let confidence = enclosing.confidence.map(ConfidenceLevel::from);
                PlaceHierarchyVm {
                    human_id: enclosing.human_id.clone(),
                    id: enclosing.id.clone(),
                    name: enclosing.name.clone().unwrap_or_else(|| enclosing.human_id.clone()),
                    type_label: enclosing.place_type.as_ref().map(|t| loc.place_type_label(t)),
                    date: enclosing.date.as_ref().map(|date| loc.date(date)),
                    confidence,
                    confidence_label: loc.confidence_label_opt(confidence),
                    assertion_id: enclosing.assertion_id.clone(),
                }
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: place_title(summary),
            place_type: summary.place_type.clone(),
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
                    assertion_id: media.assertion_id.clone(),
                })
                .collect(),
            notes: summary.notes.iter().map(AttachedRefVm::from_ref).collect(),
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
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The editable user-facing id; blank ⇒ generated on save (edit) / auto-allocated (create).
    pub human_id: String,
    /// The place type (required).
    pub place_type: genealogy_app::PlaceType,
    /// The place's primary name (create-only; on edit, names are the Names collection).
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
            existing_human_id: None,
            human_id: String::new(),
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

    /// A draft pre-populated from an existing place for editing. Records the current `human_id` so
    /// [`Self::edits_against`] diffs (supersedes) rather than creates; splits the rendered `lat,long`
    /// coordinates back into the raw decimal-degree fields. `name` is create-only (on edit, names are
    /// the Names collection), so it seeds empty.
    #[must_use]
    pub fn from_detail(detail: &PlaceDetail) -> Self {
        let (latitude, longitude) = detail
            .coordinates
            .as_deref()
            .and_then(|pair| pair.split_once(','))
            .map_or_else(
                || (String::new(), String::new()),
                |(lat, long)| (lat.trim().to_owned(), long.trim().to_owned()),
            );
        Self {
            existing_human_id: Some(detail.human_id.clone()),
            human_id: detail.human_id.clone(),
            place_type: detail.place_type.clone().unwrap_or(DEFAULT_PLACE_TYPE),
            name: String::new(),
            latitude,
            longitude,
            code: detail.code.clone().unwrap_or_default(),
        }
    }

    /// Builds the [`PlaceChangeSetRequest`] the app commits on Save (create mode), or `None` when the
    /// coordinate pair is invalid (so Save is a no-op rather than committing a partial place).
    #[must_use]
    pub fn to_request(&self) -> Option<PlaceChangeSetRequest> {
        let coordinates = match self.coordinates() {
            Coordinates::Unset => None,
            Coordinates::Point(point) => Some(point),
            Coordinates::Invalid => return None,
        };
        Some(PlaceChangeSetRequest {
            human_id: non_blank(&self.human_id),
            place_type: self.place_type.clone(),
            name: non_blank(&self.name),
            coordinates,
            code: non_blank(&self.code),
        })
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode). The latitude/longitude pair commits as one [`PlaceEdit::SetCoordinates`] only when it
    /// changed to a valid point (a blank/half-filled pair emits nothing — there is no clear command);
    /// `SetHumanId` is emitted last so the record is only re-keyed after every other field has
    /// committed against its current id (a blank id regenerates).
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<PlaceEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        let mut edits = Vec::new();
        if self.place_type != seed.place_type {
            edits.push(PlaceEdit::SetType {
                human_id: human_id.clone(),
                place_type: self.place_type.clone(),
            });
        }
        let coordinates_changed = self.latitude != seed.latitude || self.longitude != seed.longitude;
        if coordinates_changed && let Coordinates::Point(coordinates) = self.coordinates() {
            edits.push(PlaceEdit::SetCoordinates {
                human_id: human_id.clone(),
                coordinates,
            });
        }
        if self.code != seed.code {
            edits.push(PlaceEdit::SetCode {
                human_id: human_id.clone(),
                code: self.code.clone(),
            });
        }
        if self.human_id.trim() != seed.human_id {
            edits.push(PlaceEdit::SetHumanId {
                human_id,
                new_human_id: non_blank(&self.human_id),
            });
        }
        edits
    }
}

impl RecordDraft for PlaceDraft {
    type Detail = PlaceDetail;

    fn from_detail(detail: &PlaceDetail) -> Self {
        Self::from_detail(detail)
    }

    fn is_valid(&self) -> bool {
        Self::is_valid(self)
    }
}

#[cfg(test)]
mod place_draft_tests {
    use super::PlaceDraft;
    use crate::navigation::PlaceEdit;
    use genealogy_app::PlaceType;

    fn seed() -> PlaceDraft {
        PlaceDraft {
            existing_human_id: Some("P0001".to_owned()),
            human_id: "P0001".to_owned(),
            place_type: PlaceType::City,
            name: String::new(),
            latitude: "59.9".to_owned(),
            longitude: "10.7".to_owned(),
            code: "0301".to_owned(),
        }
    }

    #[test]
    fn a_fresh_draft_is_valid() {
        assert!(PlaceDraft::new().is_valid());
    }

    #[test]
    fn an_unchanged_draft_yields_no_edits() {
        assert!(seed().edits_against(&seed()).is_empty());
    }

    #[test]
    fn a_changed_coordinate_pair_yields_one_set_coordinates() {
        let draft = PlaceDraft {
            latitude: "60.0".to_owned(),
            longitude: "11.0".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], PlaceEdit::SetCoordinates { .. }));
    }

    #[test]
    fn a_changed_type_and_code_each_yield_one_edit() {
        let draft = PlaceDraft {
            place_type: PlaceType::Country,
            code: "NO".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 2);
        assert!(matches!(&edits[0], PlaceEdit::SetType { .. }));
        assert!(matches!(&edits[1], PlaceEdit::SetCode { code, .. } if code == "NO"));
    }

    #[test]
    fn a_blank_human_id_regenerates_and_is_emitted_last() {
        let draft = PlaceDraft {
            human_id: String::new(),
            place_type: PlaceType::Country,
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 2);
        assert!(matches!(&edits[0], PlaceEdit::SetType { .. }));
        assert!(matches!(&edits[1], PlaceEdit::SetHumanId { new_human_id, .. } if new_human_id.is_none()));
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
