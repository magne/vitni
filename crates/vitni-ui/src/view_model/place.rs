use super::{
    ActionLabel, AttachedRefVm, CitationRefVm, ConfidenceLevel, DetailTab, EventPinVm, HistoryEntryVm, Localizer,
    MarkerShapeVm, MediaRefVm, PlaceChangeSetRequest, PlaceEdit, RecordDraft, RestrictionKind, RowVm, TagRef,
    citation_ref_from_ref, event_pin_vm, line_label, marker_shape, non_blank, year_of,
};

/// The succession kinds the Succession panel's Kind select offers, in display order — the closed
/// domain set (ADR 0026 §2–§3), so a new variant is a compile error here rather than a silently
/// unpickable kind. Labels come from [`Localizer::succession_kind_label`].
pub const SUCCESSION_KINDS: [vitni_app::SuccessionKind; 5] = [
    vitni_app::SuccessionKind::Merged,
    vitni_app::SuccessionKind::Split,
    vitni_app::SuccessionKind::Absorbed,
    vitni_app::SuccessionKind::Elevated,
    vitni_app::SuccessionKind::Renamed,
];

/// The place types offered by every "new place" type picker (a common subset; the model has more) —
/// the single list, so a place created from the Places category and a place created inline (an event's
/// "+ New place" cascade) always offer the same choices.
pub const NEW_PLACE_TYPES: [vitni_app::PlaceType; 9] = [
    vitni_app::PlaceType::Country,
    vitni_app::PlaceType::County,
    vitni_app::PlaceType::Municipality,
    vitni_app::PlaceType::Parish,
    vitni_app::PlaceType::City,
    vitni_app::PlaceType::Town,
    vitni_app::PlaceType::Village,
    vitni_app::PlaceType::Farm,
    vitni_app::PlaceType::Building,
];

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

/// One succession relation rendered on the Hierarchy tab's Succession card (ADR 0026 §3–§4): the
/// counterpart place, a localized kind label (merged/split/absorbed/elevated/renamed), the dated
/// effective caption, and the `AssertionId` a row Retract targets. Built from either a place's
/// `predecessors` or its `successors` — which list it came from decides whether the row reads
/// "counterpart → this place" or "this place → counterpart" (`place.rs`'s [`place_succession_card`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceSuccessionVm {
    /// The counterpart place's user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The counterpart place's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The counterpart place's display name (falls back to the `human_id`).
    pub name: String,
    /// The localized kind label.
    pub kind_label: String,
    /// The localized date the succession took effect, if known.
    pub date: Option<String>,
    /// The `AssertionId` (a UUID string) a row Retract retracts. Never rendered.
    pub assertion_id: String,
}

/// One dated geometry assertion for the Place Map tab's "Geometry over time" table (ADR 0024/0026,
/// Phase 9's map editor): its shape in the map component's decimal-degree convention (point or
/// polygon, mirroring [`MarkerShapeVm`]), a localized kind label, the year it sorts by (drives
/// [`resolve_geometry_as_of`]), the dated-effective caption, and surety/source cues. The
/// `AssertionId` is the row Retract's target — never rendered.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceGeometryVm {
    /// The shape to plot/edit.
    pub shape: MarkerShapeVm,
    /// The localized kind label ("Point" / "Polygon").
    pub kind_label: String,
    /// The year this assertion sorts by, if dated — `None` for an undated/primary assertion, which
    /// [`resolve_geometry_as_of`] treats as always eligible (the fallback when no dated one qualifies).
    pub year: Option<i32>,
    /// The localized dated-effective caption ("from 1898"), or `None` for an undated/primary assertion.
    pub date: Option<String>,
    /// The operator's surety in this geometry assertion.
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label.
    pub confidence_label: String,
    /// How many citations back this geometry assertion.
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) that introduced this geometry — the target a row Retract
    /// retracts. Never rendered.
    pub assertion_id: String,
}

/// Resolves which of a place's dated geometry assertions is in effect **as of** `year` — the
/// latest-dated one at or before `year`, falling back to the first **undated** (primary) one when
/// none qualifies, and to nothing when the place has neither. This is `vitni_app::resolve_as_of`
/// itself (ADR 0026 §1) — the one rule the server-side `resolved_geometry` runs — evaluated
/// client-side over the already-loaded [`PlaceDetail::geometries`] so the Map tab's time slider needs
/// no extra round trip.
///
/// The vm keys assertions by a bare year and core by a `sort_value`; both are monotonic and the rule
/// is `<=` on a monotonic key, so the year widens to `i64` and resolves identically.
#[must_use]
pub fn resolve_geometry_as_of(geometries: &[PlaceGeometryVm], year: i32) -> Option<&PlaceGeometryVm> {
    vitni_app::resolve_as_of(geometries.iter(), i64::from(year), |geometry| {
        geometry.year.map(i64::from)
    })
}

/// A place's single point coordinate, ready for the read-only Map tab: the parsed decimal-degree
/// latitude/longitude and the place title used as the marker's accessible label. `Some` only when the
/// place has an asserted coordinate whose both halves parse (Phase 6 map MVP).
#[derive(Debug, Clone, PartialEq)]
pub struct MapPointVm {
    /// The latitude in decimal degrees.
    pub lat: f64,
    /// The longitude in decimal degrees.
    pub lon: f64,
    /// The place title, used as the map marker's accessible label.
    pub label: String,
}

/// A place's detail view — type/coordinates/code facts, name history, the jurisdiction chain,
/// citations, and the audit history.
///
/// Not `Eq`: [`Self::map_point`] carries decimal-degree floats, which have no total equality.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceDetail {
    /// The user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The stable `PlaceId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the place's primary name (falls back to the `human_id`).
    pub title: String,
    /// The place's raw type, if set (seeds the whole-record editor's Type select).
    pub place_type: Option<vitni_app::PlaceType>,
    /// The localized place-type label, if set.
    pub type_label: Option<String>,
    /// The place's coordinates rendered as `lat, long`, if asserted.
    pub coordinates: Option<String>,
    /// The parsed point coordinate for the read-only Map tab (`Some` only when both halves parse).
    pub map_point: Option<MapPointVm>,
    /// The geometry in effect for the Map tab's default view (the latest-dated/undated ADR 0024
    /// assertion) — `None` when the place has no geometry assertions at all (the empty state).
    pub resolved_geometry: Option<PlaceGeometryVm>,
    /// The place's dated geometry assertions (ADR 0024), in assertion order — the Map tab's
    /// "Geometry over time" table; [`resolve_geometry_as_of`] picks which one the time slider shows.
    pub geometries: Vec<PlaceGeometryVm>,
    /// The operator's surety in the coordinates, if asserted.
    pub coordinates_confidence: Option<ConfidenceLevel>,
    /// The localized coordinates confidence label, if asserted.
    pub coordinates_confidence_label: Option<String>,
    /// The coordinate assertion's citations, for the provenance popover.
    pub coordinate_citations: Vec<CitationRefVm>,
    /// The place's code, if set.
    pub code: Option<String>,
    /// The operator's surety in the code, if asserted.
    pub code_confidence: Option<ConfidenceLevel>,
    /// The localized code confidence label, if asserted.
    pub code_confidence_label: Option<String>,
    /// The code assertion's citations, for the provenance popover.
    pub code_citations: Vec<CitationRefVm>,
    /// The asserted names, with language/date + surety.
    pub names: Vec<PlaceNameVm>,
    /// The jurisdiction chain (enclosing places), nearest first.
    pub hierarchy: Vec<PlaceHierarchyVm>,
    /// Places this place succeeded (what it came from) — the Hierarchy tab's Succession card, ADR
    /// 0026 §4. Empty until `show_place`/`show_place_as_of`'s single-place succession join fills it.
    pub predecessors: Vec<PlaceSuccessionVm>,
    /// Places this place was succeeded by (what it became) — the Succession card's other half.
    pub successors: Vec<PlaceSuccessionVm>,
    /// Events that occurred at this place (ADR 0025 §1's event-at-place pins, scoped to just this
    /// place) — the Map tab's own event layer, reusing the Geography atlas' `EventPinVm` unchanged.
    pub events: Vec<EventPinVm>,
    /// The citations backing the place, with source · page · surety · evidence axes.
    pub citations: Vec<CitationRefVm>,
    /// The attached media objects.
    pub media: Vec<MediaRefVm>,
    /// The attached notes, each with its attach `AssertionId` (the Detach target).
    pub notes: Vec<AttachedRefVm>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The place's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The research notes arguing about this record (Research notes tab, ADR 0028 §5) — the reverse
    /// index over the `ResearchNote` projection; filled by the dispatcher.
    pub research_notes: Vec<RowVm>,
    /// The place's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl PlaceDetail {
    /// Builds a detail view from a [`PlaceSummary`](vitni_app::PlaceSummary), localizing labels,
    /// dates, and confidence. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &vitni_app::PlaceSummary, loc: &Localizer) -> Self {
        let coordinates_confidence = summary.coordinates_confidence.map(ConfidenceLevel::from);
        let code_confidence = summary.code_confidence.map(ConfidenceLevel::from);
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
        let predecessors = summary.predecessors.iter().map(|rel| succession_vm(rel, loc)).collect();
        let successors = summary.successors.iter().map(|rel| succession_vm(rel, loc)).collect();
        let title = place_title(summary);
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            place_type: summary.place_type.clone(),
            type_label: summary.place_type.as_ref().map(|t| loc.place_type_label(t)),
            coordinates: summary.coordinates.clone(),
            map_point: map_point(summary.coordinates.as_deref(), &title),
            resolved_geometry: summary.resolved_geometry.as_ref().map(|g| place_geometry_vm(g, loc)),
            geometries: summary.geometries.iter().map(|g| place_geometry_vm(g, loc)).collect(),
            title,
            coordinates_confidence,
            coordinates_confidence_label: coordinates_confidence.map(|level| loc.confidence_label(level)),
            coordinate_citations: summary
                .coordinate_citations
                .iter()
                .map(|c| citation_ref_from_ref(c, loc))
                .collect(),
            code: summary.code.clone(),
            code_confidence,
            code_confidence_label: code_confidence.map(|level| loc.confidence_label(level)),
            code_citations: summary
                .code_citations
                .iter()
                .map(|c| citation_ref_from_ref(c, loc))
                .collect(),
            names,
            hierarchy,
            predecessors,
            successors,
            events: summary.events.iter().map(|pin| event_pin_vm(pin, loc)).collect(),
            citations: summary
                .citations
                .iter()
                .map(|citation| citation_ref_from_ref(citation, loc))
                .collect(),
            media: summary.media.iter().map(MediaRefVm::from_ref).collect(),
            notes: summary.notes.iter().map(AttachedRefVm::from_ref).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            research_notes: Vec::new(),
            history: Vec::new(),
        }
    }
}

/// Builds one [`PlaceGeometryVm`] from a [`vitni_app::PlaceGeometryRef`], reusing the Geography
/// VM's `marker_shape`/`year_of` conversions (Phase 9's "reuse the map machinery" — no duplicate
/// point/polygon-to-decimal-degrees logic).
fn place_geometry_vm(geometry: &vitni_app::PlaceGeometryRef, loc: &Localizer) -> PlaceGeometryVm {
    let confidence = geometry.confidence.map(ConfidenceLevel::from);
    let kind_label = match &geometry.geometry {
        vitni_app::PlaceGeometry::Point(_) => loc.geometry_kind_point(),
        vitni_app::PlaceGeometry::Polygon { .. } => loc.geometry_kind_polygon(),
    };
    PlaceGeometryVm {
        shape: marker_shape(&geometry.geometry),
        kind_label,
        year: geometry.date.as_ref().map(year_of),
        date: geometry.date.as_ref().map(|date| loc.date(date)),
        confidence,
        confidence_label: loc.confidence_label_opt(confidence),
        source_count: geometry.citations.len(),
        assertion_id: geometry.assertion_id.clone(),
    }
}

/// The Map tab's shape to display **as of** `year`: [`resolve_geometry_as_of`] over the place's own
/// dated ADR 0024 assertions, falling back to its scalar [`PlaceDetail::map_point`] when it has none
/// yet — the common case for a place nobody has drawn a boundary/point for (a GEDCOM import, or a
/// coordinate typed on the Overview tab). Mirrors `vitni_app::show_geography`'s identical
/// fallback for the Geography atlas, so a place's location shows up consistently in both.
#[must_use]
pub fn place_map_display_shape(detail: &PlaceDetail, year: i32) -> Option<MarkerShapeVm> {
    resolve_geometry_as_of(&detail.geometries, year)
        .map(|geometry| geometry.shape.clone())
        .or_else(|| {
            detail.map_point.as_ref().map(|point| MarkerShapeVm::Point {
                lat: point.lat,
                lon: point.lon,
            })
        })
}

/// The place's coordinate for display (the Overview tab's Latitude/Longitude read boxes): the
/// [`PlaceDetail::resolved_geometry`]'s representative point — a Point's own coordinate, or a Polygon's
/// first exterior vertex — falling back to the scalar [`PlaceDetail::map_point`] when the place has no
/// geometry assertion at all. Dropping a point on the Map tab only ever emits a `GeometryAsserted` (ADR
/// 0024/0026), which never touches the scalar `coordinates` field, so a display that only ever read the
/// scalar would keep showing a stale lat/long after a geometry save; the resolved-geometry-first order
/// here keeps the Overview in sync (the scalar is just the undated Point case, per ADR 0024). `None`
/// only when the place has neither a geometry assertion nor a scalar coordinate.
#[must_use]
pub fn display_coordinates(detail: &PlaceDetail) -> Option<(f64, f64)> {
    detail
        .resolved_geometry
        .as_ref()
        .and_then(|geometry| representative_point(&geometry.shape))
        .or_else(|| detail.map_point.as_ref().map(|point| (point.lat, point.lon)))
}

/// A shape's representative decimal-degree point for [`display_coordinates`]: a point's own coordinate,
/// or a polygon's first exterior vertex; `None` for a (malformed) polygon with no vertices.
fn representative_point(shape: &MarkerShapeVm) -> Option<(f64, f64)> {
    match shape {
        MarkerShapeVm::Point { lat, lon } => Some((*lat, *lon)),
        MarkerShapeVm::Polygon { exterior, .. } => exterior.first().copied(),
    }
}

/// Builds one [`PlaceSuccessionVm`] from a [`vitni_app::PlaceSuccessionRef`] (either a
/// predecessor or a successor — the caller's list decides which).
fn succession_vm(rel: &vitni_app::PlaceSuccessionRef, loc: &Localizer) -> PlaceSuccessionVm {
    PlaceSuccessionVm {
        human_id: rel.human_id.clone(),
        id: rel.id.clone(),
        name: rel.name.clone().unwrap_or_else(|| rel.human_id.clone()),
        kind_label: loc.succession_kind_label(rel.kind),
        date: rel.date.as_ref().map(|date| loc.date(date)),
        assertion_id: rel.assertion_id.clone(),
    }
}

/// Parses the DTO's `lat,long` coordinate string into a [`MapPointVm`] for the read-only Map tab,
/// reusing the coordinate split precedent ([`PlaceDraft::from_detail`]): `Some` only when the string
/// is present, splits on a comma, and both trimmed halves parse as decimal degrees; `None` when unset,
/// half-filled, or unparseable. `label` becomes the marker's accessible label (the place title).
fn map_point(coordinates: Option<&str>, label: &str) -> Option<MapPointVm> {
    let (lat, lon) = coordinates?.split_once(',')?;
    let lat = lat.trim().parse::<f64>().ok()?;
    let lon = lon.trim().parse::<f64>().ok()?;
    Some(MapPointVm {
        lat,
        lon,
        label: label.to_owned(),
    })
}

/// The place's primary name for the header (its first asserted name), or the `human_id` fallback.
fn place_title(summary: &vitni_app::PlaceSummary) -> String {
    summary
        .names
        .first()
        .map_or_else(|| summary.human_id.clone(), |name| name.text.clone())
}

/// Builds a generic list row from a [`PlaceSummary`](vitni_app::PlaceSummary): the primary name,
/// a `type · enclosing` subtitle, and a per-type avatar.
#[must_use]
pub fn place_row(summary: &vitni_app::PlaceSummary, loc: &Localizer) -> RowVm {
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
fn place_avatar(place_type: Option<&vitni_app::PlaceType>) -> String {
    use vitni_app::PlaceType;
    match place_type {
        Some(PlaceType::Parish) => "⛪",
        _ => "📍",
    }
    .to_owned()
}

/// The tab strip for a place's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn place_tabs(detail: &PlaceDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>, action: Option<ActionLabel>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
        action,
    };
    vec![
        tab("overview", None, None),
        tab("map", None, None),
        tab("names", Some(detail.names.len()), Some(ActionLabel::AddName)),
        tab(
            "hierarchy",
            Some(detail.hierarchy.len()),
            Some(ActionLabel::AddEnclosing),
        ),
        tab(
            "citations",
            Some(detail.citations.len()),
            Some(ActionLabel::AttachCitation),
        ),
        tab("media", Some(detail.media.len()), Some(ActionLabel::AttachMedia)),
        tab("notes", Some(detail.notes.len()), Some(ActionLabel::AttachNote)),
        tab(
            "research-notes",
            Some(detail.research_notes.len()),
            Some(ActionLabel::NewResearchNote),
        ),
        tab("tags", Some(detail.tags.len()), Some(ActionLabel::AddTag)),
        tab("history", None, None),
    ]
}

/// The default place type a fresh create draft starts with (matching the mockup's Type select).
const DEFAULT_PLACE_TYPE: vitni_app::PlaceType = vitni_app::PlaceType::City;

/// The parse outcome of the coordinate pair (both-or-neither): unset, a parsed point, or invalid.
enum Coordinates {
    /// Both latitude and longitude are blank — no coordinates asserted.
    Unset,
    /// Both parse to a point.
    Point(vitni_app::GeoCoordinates),
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
    pub place_type: vitni_app::PlaceType,
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
                latitude.parse::<vitni_app::Microdegrees>(),
                longitude.parse::<vitni_app::Microdegrees>(),
            ) {
                (Ok(latitude), Ok(longitude)) => Coordinates::Point(vitni_app::GeoCoordinates { latitude, longitude }),
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
        latitude.parse::<vitni_app::Microdegrees>().is_err()
    }

    /// Whether the longitude field is invalid (mirror of [`Self::latitude_invalid`]).
    #[must_use]
    pub fn longitude_invalid(&self) -> bool {
        let longitude = self.longitude.trim();
        if longitude.is_empty() {
            return !self.latitude.trim().is_empty();
        }
        longitude.parse::<vitni_app::Microdegrees>().is_err()
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

    fn display_label(&self) -> Option<String> {
        line_label(&self.name)
    }
}

#[cfg(test)]
mod map_point_tests {
    use super::{MapPointVm, map_point};

    #[test]
    fn both_halves_parse_to_a_point_labelled_by_the_title() {
        let point = map_point(Some("40.7128,-74.006"), "New York").expect("a point");
        assert_eq!(
            point,
            MapPointVm {
                lat: 40.7128,
                lon: -74.006,
                label: "New York".to_owned(),
            }
        );
    }

    #[test]
    fn each_half_is_trimmed_before_parsing() {
        let point = map_point(Some(" 59.9 , 10.7 "), "Oslo").expect("a point");
        assert!((point.lat - 59.9).abs() < f64::EPSILON);
        assert!((point.lon - 10.7).abs() < f64::EPSILON);
    }

    #[test]
    fn unset_coordinates_yield_no_point() {
        assert!(map_point(None, "Nordland").is_none());
    }

    #[test]
    fn a_half_filled_pair_yields_no_point() {
        assert!(map_point(Some("59.9,"), "X").is_none());
        assert!(map_point(Some(",10.7"), "X").is_none());
    }

    #[test]
    fn an_unparseable_half_yields_no_point() {
        assert!(map_point(Some("north,10.7"), "X").is_none());
        assert!(map_point(Some("59.9,east"), "X").is_none());
    }

    #[test]
    fn a_pair_without_a_comma_yields_no_point() {
        assert!(map_point(Some("59.9 10.7"), "X").is_none());
    }
}

#[cfg(test)]
mod place_draft_tests {
    use super::PlaceDraft;
    use crate::navigation::PlaceEdit;
    use vitni_app::PlaceType;

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

#[cfg(test)]
mod place_succession_tests {
    use super::succession_vm;
    use crate::i18n::Localizer;
    use vitni_app::{PlaceSuccessionRef, SuccessionKind};

    fn loc() -> Localizer {
        Localizer::for_test("en")
    }

    fn rel(kind: SuccessionKind) -> PlaceSuccessionRef {
        PlaceSuccessionRef {
            human_id: "P0021".to_owned(),
            id: "0190-aker".to_owned(),
            name: Some("Aker".to_owned()),
            kind,
            date: None,
            assertion_id: "0190-succession-assert-1".to_owned(),
        }
    }

    #[test]
    fn a_merged_relation_carries_its_localized_kind_label() {
        let vm = succession_vm(&rel(SuccessionKind::Merged), &loc());
        assert_eq!(vm.kind_label, "merged");
        assert_eq!(vm.name, "Aker");
        assert_eq!(vm.human_id, "P0021");
        assert_eq!(vm.assertion_id, "0190-succession-assert-1");
    }

    #[test]
    fn every_succession_kind_localizes_to_a_distinct_label() {
        let labels: Vec<String> = [
            SuccessionKind::Merged,
            SuccessionKind::Split,
            SuccessionKind::Absorbed,
            SuccessionKind::Elevated,
            SuccessionKind::Renamed,
        ]
        .iter()
        .map(|&kind| succession_vm(&rel(kind), &loc()).kind_label)
        .collect();
        let mut unique = labels.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), labels.len(), "each kind gets its own label: {labels:?}");
    }

    #[test]
    fn an_unnamed_counterpart_falls_back_to_its_human_id() {
        let unnamed = PlaceSuccessionRef {
            name: None,
            ..rel(SuccessionKind::Absorbed)
        };
        assert_eq!(succession_vm(&unnamed, &loc()).name, "P0021");
    }

    #[test]
    fn the_offered_succession_kinds_cover_every_variant_with_a_distinct_label() {
        let labels: Vec<String> = super::SUCCESSION_KINDS
            .iter()
            .map(|&kind| loc().succession_kind_label(kind))
            .collect();
        assert_eq!(labels.len(), 5, "the kind select offers all five variants: {labels:?}");
        let mut unique = labels.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), labels.len(), "no kind is offered twice: {labels:?}");
    }
}

#[cfg(test)]
mod place_geometry_tests {
    use super::{PlaceGeometryVm, place_geometry_vm, resolve_geometry_as_of};
    use crate::i18n::Localizer;
    use crate::view_model::MarkerShapeVm;
    use std::str::FromStr;
    use vitni_app::{
        Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody, GeoCoordinates,
        Microdegrees, PlaceGeometry,
    };

    fn loc() -> Localizer {
        Localizer::for_test("en")
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

    fn point_ref(date: Option<GenealogicalDate>) -> vitni_app::PlaceGeometryRef {
        vitni_app::PlaceGeometryRef {
            geometry: PlaceGeometry::Point(GeoCoordinates {
                latitude: Microdegrees::from_str("59.9").expect("lat"),
                longitude: Microdegrees::from_str("10.7").expect("lon"),
            }),
            date,
            confidence: None,
            citations: Vec::new(),
            assertion_id: "assert-geometry-1".to_owned(),
        }
    }

    #[test]
    fn a_point_geometry_carries_its_kind_label_and_year() {
        let vm = place_geometry_vm(&point_ref(Some(year_date(1898))), &loc());
        assert_eq!(vm.kind_label, "Point");
        assert_eq!(vm.year, Some(1898));
        assert!(vm.date.is_some());
        assert_eq!(vm.assertion_id, "assert-geometry-1");
    }

    #[test]
    fn an_undated_geometry_has_no_year() {
        let vm = place_geometry_vm(&point_ref(None), &loc());
        assert_eq!(vm.year, None);
        assert_eq!(vm.date, None);
    }

    #[test]
    fn a_polygon_geometry_carries_the_polygon_kind_label() {
        let polygon = vitni_app::PlaceGeometryRef {
            geometry: PlaceGeometry::Polygon {
                exterior: vec![
                    GeoCoordinates {
                        latitude: Microdegrees::from_str("60.0").expect("lat"),
                        longitude: Microdegrees::from_str("5.0").expect("lon"),
                    },
                    GeoCoordinates {
                        latitude: Microdegrees::from_str("61.0").expect("lat"),
                        longitude: Microdegrees::from_str("5.0").expect("lon"),
                    },
                    GeoCoordinates {
                        latitude: Microdegrees::from_str("61.0").expect("lat"),
                        longitude: Microdegrees::from_str("6.0").expect("lon"),
                    },
                ],
                holes: Vec::new(),
            },
            ..point_ref(None)
        };
        let vm = place_geometry_vm(&polygon, &loc());
        assert_eq!(vm.kind_label, "Polygon");
        assert!(matches!(vm.shape, MarkerShapeVm::Polygon { .. }));
    }

    fn dated(year: i32) -> PlaceGeometryVm {
        PlaceGeometryVm {
            shape: MarkerShapeVm::Point { lat: 59.9, lon: 10.7 },
            kind_label: "Point".to_owned(),
            year: Some(year),
            date: Some(format!("from {year}")),
            confidence: None,
            confidence_label: String::new(),
            source_count: 0,
            assertion_id: format!("assert-{year}"),
        }
    }

    fn undated() -> PlaceGeometryVm {
        PlaceGeometryVm {
            year: None,
            date: None,
            assertion_id: "assert-undated".to_owned(),
            ..dated(0)
        }
    }

    #[test]
    fn an_empty_list_resolves_to_nothing() {
        assert!(resolve_geometry_as_of(&[], 1900).is_none());
    }

    #[test]
    fn a_query_year_before_every_dated_assertion_falls_back_to_the_undated_one() {
        let geometries = vec![undated(), dated(1898)];
        let resolved = resolve_geometry_as_of(&geometries, 1850).expect("a fallback");
        assert_eq!(resolved.assertion_id, "assert-undated");
    }

    #[test]
    fn the_latest_dated_assertion_at_or_before_the_query_year_wins() {
        let geometries = vec![undated(), dated(1898), dated(1950)];
        let resolved = resolve_geometry_as_of(&geometries, 1920).expect("a match");
        assert_eq!(resolved.assertion_id, "assert-1898");
        let resolved_later = resolve_geometry_as_of(&geometries, 2000).expect("a match");
        assert_eq!(resolved_later.assertion_id, "assert-1950");
    }

    #[test]
    fn a_query_year_exactly_matching_an_assertion_is_eligible() {
        let geometries = vec![dated(1898)];
        let resolved = resolve_geometry_as_of(&geometries, 1898).expect("eligible at the boundary");
        assert_eq!(resolved.assertion_id, "assert-1898");
    }

    #[test]
    fn nothing_dated_at_or_before_the_year_and_nothing_undated_resolves_to_nothing() {
        // Mirrors the app-side rule: the fallback is the first **undated** assertion, so a set of
        // purely later-dated ones resolves to nothing rather than plotting a shape that is not in
        // effect. This is what lets the Map tab say "No geometry as of 1900." truthfully.
        let geometries = vec![dated(1950), dated(1980)];
        assert!(resolve_geometry_as_of(&geometries, 1900).is_none());
    }

    #[test]
    fn an_undated_assertion_listed_after_a_dated_one_is_still_the_fallback() {
        // Assertion order must not decide the fallback — only "is it undated" does. Falling back to
        // the first *item* plots the 1900 shape at 1850, the exact divergence from the app-side rule.
        let geometries = vec![dated(1900), undated()];
        let resolved = resolve_geometry_as_of(&geometries, 1850).expect("the undated assertion");
        assert_eq!(resolved.assertion_id, "assert-undated");
    }

    #[test]
    fn a_dated_assertion_at_or_before_the_year_beats_an_undated_one() {
        let geometries = vec![undated(), dated(1898)];
        let resolved = resolve_geometry_as_of(&geometries, 1950).expect("the dated assertion");
        assert_eq!(resolved.assertion_id, "assert-1898");
    }
}

#[cfg(test)]
mod place_map_display_shape_tests {
    use super::{PlaceDetail, place_map_display_shape};
    use crate::view_model::MarkerShapeVm;

    /// A minimal place detail with everything empty — the fields `place_map_display_shape` doesn't
    /// read stay bare, mirroring the Phase-6 `place_map.rs` test fixture pattern.
    fn bare() -> PlaceDetail {
        PlaceDetail {
            human_id: "P0090".to_owned(),
            id: "place-id".to_owned(),
            title: "Nordland".to_owned(),
            place_type: None,
            type_label: None,
            coordinates: None,
            map_point: None,
            resolved_geometry: None,
            geometries: Vec::new(),
            coordinates_confidence: None,
            coordinates_confidence_label: None,
            coordinate_citations: Vec::new(),
            code: None,
            code_confidence: None,
            code_confidence_label: None,
            code_citations: Vec::new(),
            names: Vec::new(),
            hierarchy: Vec::new(),
            predecessors: Vec::new(),
            successors: Vec::new(),
            events: Vec::new(),
            citations: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            restrictions: Vec::new(),
            research_notes: Vec::new(),
            history: Vec::new(),
        }
    }

    #[test]
    fn no_geometry_and_no_coordinate_shows_nothing() {
        assert_eq!(place_map_display_shape(&bare(), 1900), None);
    }

    #[test]
    fn a_scalar_coordinate_shows_as_a_point_when_no_geometry_assertion_exists() {
        let detail = PlaceDetail {
            map_point: Some(super::MapPointVm {
                lat: 67.0,
                lon: 15.0,
                label: "Nordland".to_owned(),
            }),
            ..bare()
        };
        assert_eq!(
            place_map_display_shape(&detail, 1900),
            Some(MarkerShapeVm::Point { lat: 67.0, lon: 15.0 })
        );
    }

    #[test]
    fn an_explicit_geometry_assertion_wins_over_the_scalar_coordinate() {
        let geometry_shape = MarkerShapeVm::Point { lat: 61.9, lon: 8.8 };
        let detail = PlaceDetail {
            map_point: Some(super::MapPointVm {
                lat: 67.0,
                lon: 15.0,
                label: "Nordland".to_owned(),
            }),
            geometries: vec![super::PlaceGeometryVm {
                shape: geometry_shape.clone(),
                kind_label: "Point".to_owned(),
                year: None,
                date: None,
                confidence: None,
                confidence_label: String::new(),
                source_count: 0,
                assertion_id: "assert-1".to_owned(),
            }],
            ..bare()
        };
        assert_eq!(place_map_display_shape(&detail, 1900), Some(geometry_shape));
    }
}

#[cfg(test)]
mod display_coordinates_tests {
    use super::{PlaceDetail, display_coordinates};
    use crate::view_model::MarkerShapeVm;

    /// A minimal place detail with everything empty, mirroring the neighbouring
    /// `place_map_display_shape_tests::bare` fixture.
    fn bare() -> PlaceDetail {
        PlaceDetail {
            human_id: "P0090".to_owned(),
            id: "place-id".to_owned(),
            title: "Nordland".to_owned(),
            place_type: None,
            type_label: None,
            coordinates: None,
            map_point: None,
            resolved_geometry: None,
            geometries: Vec::new(),
            coordinates_confidence: None,
            coordinates_confidence_label: None,
            coordinate_citations: Vec::new(),
            code: None,
            code_confidence: None,
            code_confidence_label: None,
            code_citations: Vec::new(),
            names: Vec::new(),
            hierarchy: Vec::new(),
            predecessors: Vec::new(),
            successors: Vec::new(),
            events: Vec::new(),
            citations: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            restrictions: Vec::new(),
            research_notes: Vec::new(),
            history: Vec::new(),
        }
    }

    #[test]
    fn no_geometry_and_no_scalar_shows_nothing() {
        assert_eq!(display_coordinates(&bare()), None);
    }

    #[test]
    fn a_scalar_coordinate_shows_when_no_geometry_assertion_exists() {
        let detail = PlaceDetail {
            map_point: Some(super::MapPointVm {
                lat: 67.0,
                lon: 15.0,
                label: "Nordland".to_owned(),
            }),
            ..bare()
        };
        assert_eq!(display_coordinates(&detail), Some((67.0, 15.0)));
    }

    #[test]
    fn a_resolved_geometry_point_wins_over_a_stale_scalar_coordinate() {
        // The regression this guards: dropping a point on the Map tab only ever emits
        // `GeometryAsserted`, which never updates the scalar `coordinates` field — so a display that
        // preferred the scalar would keep showing the pre-drop location.
        let detail = PlaceDetail {
            map_point: Some(super::MapPointVm {
                lat: 67.0,
                lon: 15.0,
                label: "Nordland".to_owned(),
            }),
            resolved_geometry: Some(super::PlaceGeometryVm {
                shape: MarkerShapeVm::Point { lat: 61.9, lon: 8.8 },
                kind_label: "Point".to_owned(),
                year: None,
                date: None,
                confidence: None,
                confidence_label: String::new(),
                source_count: 0,
                assertion_id: "assert-1".to_owned(),
            }),
            ..bare()
        };
        assert_eq!(display_coordinates(&detail), Some((61.9, 8.8)));
    }

    #[test]
    fn a_resolved_polygon_shows_its_first_exterior_vertex() {
        let detail = PlaceDetail {
            resolved_geometry: Some(super::PlaceGeometryVm {
                shape: MarkerShapeVm::Polygon {
                    exterior: vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)],
                    holes: Vec::new(),
                },
                kind_label: "Polygon".to_owned(),
                year: None,
                date: None,
                confidence: None,
                confidence_label: String::new(),
                source_count: 0,
                assertion_id: "assert-2".to_owned(),
            }),
            ..bare()
        };
        assert_eq!(display_coordinates(&detail), Some((60.0, 5.0)));
    }
}

#[cfg(test)]
mod place_detail_events_tests {
    use super::PlaceDetail;
    use crate::i18n::Localizer;
    use std::collections::BTreeSet;
    use std::str::FromStr;
    use vitni_app::{EventPin, EventType, GeoCoordinates, Microdegrees, PlaceSummary};

    fn loc() -> Localizer {
        Localizer::for_test("en")
    }

    /// A minimal summary with everything empty but a title and one event pin — `from_summary`'s other
    /// conversions are exercised elsewhere; this only checks the events thread through.
    fn summary_with_one_event() -> PlaceSummary {
        PlaceSummary {
            human_id: "P0090".to_owned(),
            id: "place-id".to_owned(),
            generated_title: "Nordgarden".to_owned(),
            resolved_name: Some("Nordgarden".to_owned()),
            resolved_as_of: None,
            place_type: None,
            place_type_confidence: None,
            names: Vec::new(),
            code: None,
            code_confidence: None,
            code_citations: Vec::new(),
            coordinates: None,
            coordinates_point: None,
            coordinates_confidence: None,
            coordinate_citations: Vec::new(),
            geometries: Vec::new(),
            resolved_geometry: None,
            enclosing: Vec::new(),
            predecessors: Vec::new(),
            successors: Vec::new(),
            events: vec![EventPin {
                human_id: "E0001".to_owned(),
                id: "event-1".to_owned(),
                event_type: Some(EventType::Birth),
                date: None,
                place_human_id: "P0090".to_owned(),
                point: GeoCoordinates {
                    latitude: Microdegrees::from_str("61.5").expect("lat"),
                    longitude: Microdegrees::from_str("9.0").expect("lon"),
                },
            }],
            citations: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            restrictions: BTreeSet::new(),
        }
    }

    #[test]
    fn from_summary_threads_the_place_events_into_the_detail() {
        let detail = PlaceDetail::from_summary(&summary_with_one_event(), &loc());
        assert_eq!(detail.events.len(), 1);
        assert_eq!(detail.events[0].human_id, "E0001");
        assert!((detail.events[0].lat - 61.5).abs() < 1e-6);
    }

    #[test]
    fn no_events_yields_an_empty_list() {
        let summary = PlaceSummary {
            events: Vec::new(),
            ..summary_with_one_event()
        };
        let detail = PlaceDetail::from_summary(&summary, &loc());
        assert!(detail.events.is_empty());
    }
}

#[cfg(test)]
mod place_display_label_tests {
    use super::{PlaceDraft, RecordDraft};

    #[test]
    fn the_label_is_the_place_name() {
        let draft = PlaceDraft {
            name: "  Kristiania  ".to_owned(),
            ..PlaceDraft::new()
        };
        assert_eq!(draft.display_label(), Some("Kristiania".to_owned()));
    }

    #[test]
    fn a_draft_with_no_name_has_no_label() {
        let draft = PlaceDraft {
            latitude: "59.9".to_owned(),
            longitude: "10.7".to_owned(),
            ..PlaceDraft::new()
        };
        assert_eq!(draft.display_label(), None);
    }
}
