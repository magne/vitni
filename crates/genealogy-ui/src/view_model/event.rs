use super::{
    CitationRefVm, ConfidenceLevel, DetailTab, EventChangeSetRequest, EventPlaceRequest, EventType, FamilyMediaVm,
    HistoryEntryVm, Localizer, RestrictionKind, RowVm, TagRef, citation_ref_from_ref, non_blank,
};

/// One event participant (Participants tab): the person, their role, surety, and source count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantVm {
    /// The participant's user-facing id (e.g. `I0001`).
    pub human_id: String,
    /// The participant's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The participant's display name (falls back to the `human_id`).
    pub name: String,
    /// The localized participant-role label.
    pub role_label: String,
    /// The operator's surety in the participation (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the participation.
    pub source_count: usize,
}

/// The place an event occurred (Overview link): its name and the navigation ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceLinkVm {
    /// The place's user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The place's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The place's display name (falls back to the `human_id`).
    pub name: String,
}

/// An event's detail view — type/date/place facts, participants, citations, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDetail {
    /// The user-facing id (e.g. `E0001`).
    pub human_id: String,
    /// The stable `EventId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the localized event-type label (falls back to the `human_id`).
    pub title: String,
    /// The localized event-type label.
    pub type_label: String,
    /// The localized date, if known.
    pub date: Option<String>,
    /// The operator's surety in the date (drives the confidence badge), if asserted.
    pub date_confidence: Option<ConfidenceLevel>,
    /// The localized date confidence label, if asserted.
    pub date_confidence_label: Option<String>,
    /// How many citations back the date assertion.
    pub date_source_count: usize,
    /// The date assertion's citations, for the provenance popover.
    pub date_citations: Vec<CitationRefVm>,
    /// The linked place, if any.
    pub place: Option<PlaceLinkVm>,
    /// The operator's surety in the place link, if linked.
    pub place_confidence: Option<ConfidenceLevel>,
    /// The localized place confidence label, if linked.
    pub place_confidence_label: Option<String>,
    /// The event's free-text description, if set.
    pub description: Option<String>,
    /// The participants, joined to the person projection.
    pub participants: Vec<ParticipantVm>,
    /// The citations backing the event, with source · page · surety · evidence axes.
    pub citations: Vec<CitationRefVm>,
    /// The attached media objects.
    pub media: Vec<FamilyMediaVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The event's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The event's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl EventDetail {
    /// Builds a detail view from an [`EventSummary`](genealogy_app::EventSummary), localizing labels,
    /// dates, and confidence. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::EventSummary, loc: &Localizer) -> Self {
        let type_label = summary.event_type.as_ref().map_or_else(
            || summary.human_id.clone(),
            |event_type| loc.event_type_label(event_type),
        );
        let participants = summary
            .participants
            .iter()
            .map(|participant| {
                let confidence = ConfidenceLevel::from(participant.confidence);
                ParticipantVm {
                    human_id: participant.human_id.clone(),
                    id: participant.id.clone(),
                    name: participant.name.clone().unwrap_or_else(|| participant.human_id.clone()),
                    role_label: loc.participant_role_label(&participant.role),
                    confidence,
                    confidence_label: loc.confidence_label(confidence),
                    source_count: participant.source_count,
                }
            })
            .collect();
        let date_confidence = summary.date_confidence.map(ConfidenceLevel::from);
        let place_confidence = summary.place_confidence.map(ConfidenceLevel::from);
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: type_label.clone(),
            type_label,
            date: summary.date.as_ref().map(|date| loc.date(date)),
            date_confidence,
            date_confidence_label: date_confidence.map(|level| loc.confidence_label(level)),
            date_source_count: summary.date_source_count,
            date_citations: summary
                .date_citations
                .iter()
                .map(|c| citation_ref_from_ref(c, loc))
                .collect(),
            place: summary.place.as_ref().map(|place| PlaceLinkVm {
                human_id: place.human_id.clone(),
                id: place.id.clone(),
                name: place.name.clone().unwrap_or_else(|| place.human_id.clone()),
            }),
            place_confidence,
            place_confidence_label: place_confidence.map(|level| loc.confidence_label(level)),
            description: summary.description.clone(),
            participants,
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

/// Builds a generic list row from an [`EventSummary`](genealogy_app::EventSummary): the type label,
/// a `date · place` subtitle, and a per-type avatar.
#[must_use]
pub fn event_row(summary: &genealogy_app::EventSummary, loc: &Localizer) -> RowVm {
    let title = summary.event_type.as_ref().map_or_else(
        || summary.human_id.clone(),
        |event_type| loc.event_type_label(event_type),
    );
    let date = summary.date.as_ref().map(|date| loc.date(date));
    let place = summary
        .place
        .as_ref()
        .map(|p| p.name.clone().unwrap_or_else(|| p.human_id.clone()));
    let subtitle = match (date, place) {
        (Some(date), Some(place)) => Some(format!("{date} · {place}")),
        (Some(date), None) => Some(date),
        (None, Some(place)) => Some(place),
        (None, None) => None,
    };
    RowVm {
        id: summary.human_id.clone(),
        title,
        subtitle,
        avatar: Some(event_avatar(summary.event_type.as_ref())),
        ..RowVm::default()
    }
}

/// The decorative avatar glyph for an event row, by type (a generic calendar otherwise).
fn event_avatar(event_type: Option<&EventType>) -> String {
    match event_type {
        Some(EventType::Marriage) => "💍",
        Some(EventType::Birth) => "👶",
        Some(EventType::Census) => "📋",
        Some(EventType::Burial | EventType::Cremation) => "⚰",
        Some(EventType::Baptism | EventType::Christening) => "✝",
        _ => "📅",
    }
    .to_owned()
}

/// The tab strip for an event's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn event_tabs(detail: &EventDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("participants", Some(detail.participants.len())),
        tab("citations", Some(detail.citations.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// The default event type a fresh create draft starts with (matching the mockup's Type select).
const DEFAULT_EVENT_TYPE: EventType = EventType::Birth;

/// How the create form's place field is set: unset, an existing place, or a new place created inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventPlaceKind {
    /// No place linked.
    #[default]
    None,
    /// Link an existing place (by `human_id`).
    Existing,
    /// Create a place inline (a §6b cascade).
    New,
}

/// The create form's in-memory draft for a new event (`record-editing.html` §6): a required type, a
/// description, and an optional place (unset, existing, or created inline — §6b). Structured date
/// editing is PR29. Create-only; nothing is written until Save commits an [`EventChangeSetRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDraft {
    /// The event type (required).
    pub event_type: EventType,
    /// The free-text description.
    pub description: String,
    /// How the place field is set.
    pub place_kind: EventPlaceKind,
    /// The existing place's `human_id` (when `place_kind` is `Existing`).
    pub existing_place: String,
    /// The inline place's type (when `place_kind` is `New`).
    pub new_place_type: genealogy_app::PlaceType,
    /// The inline place's name (when `place_kind` is `New`).
    pub new_place_name: String,
}

impl Default for EventDraft {
    fn default() -> Self {
        Self {
            event_type: DEFAULT_EVENT_TYPE,
            description: String::new(),
            place_kind: EventPlaceKind::None,
            existing_place: String::new(),
            new_place_type: genealogy_app::PlaceType::City,
            new_place_name: String::new(),
        }
    }
}

impl EventDraft {
    /// A fresh draft for creating a new event (default type, no place).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the operator has entered anything beyond the defaults — the Save gate.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.event_type != DEFAULT_EVENT_TYPE
            || non_blank(&self.description).is_some()
            || self.place_kind != EventPlaceKind::None
    }

    /// Builds the [`EventChangeSetRequest`] the app commits on Save.
    #[must_use]
    pub fn to_request(&self) -> EventChangeSetRequest {
        let place = match self.place_kind {
            EventPlaceKind::None => EventPlaceRequest::None,
            EventPlaceKind::Existing => match non_blank(&self.existing_place) {
                Some(human_id) => EventPlaceRequest::Existing(human_id),
                None => EventPlaceRequest::None,
            },
            EventPlaceKind::New => EventPlaceRequest::New {
                place_type: self.new_place_type.clone(),
                name: non_blank(&self.new_place_name),
            },
        };
        EventChangeSetRequest {
            event_type: self.event_type.clone(),
            description: non_blank(&self.description),
            place,
        }
    }
}

#[cfg(test)]
mod event_draft_tests {
    use super::{EventDraft, EventPlaceKind};
    use crate::navigation::EventPlaceRequest;
    use genealogy_app::{EventType, PlaceType};

    #[test]
    fn a_fresh_draft_is_not_dirty() {
        assert!(!EventDraft::new().is_dirty());
    }

    #[test]
    fn a_changed_type_or_description_makes_it_dirty() {
        assert!(
            EventDraft {
                event_type: EventType::Baptism,
                ..EventDraft::new()
            }
            .is_dirty()
        );
        assert!(
            EventDraft {
                description: "at church".to_owned(),
                ..EventDraft::new()
            }
            .is_dirty()
        );
    }

    #[test]
    fn a_new_place_maps_to_a_pending_place_request() {
        let draft = EventDraft {
            place_kind: EventPlaceKind::New,
            new_place_type: PlaceType::Building,
            new_place_name: "Trinity Church".to_owned(),
            ..EventDraft::new()
        };
        match draft.to_request().place {
            EventPlaceRequest::New { place_type, name } => {
                assert_eq!(place_type, PlaceType::Building);
                assert_eq!(name.as_deref(), Some("Trinity Church"));
            }
            other => panic!("expected a New place request, got {other:?}"),
        }
    }

    #[test]
    fn an_existing_place_id_maps_through() {
        let draft = EventDraft {
            place_kind: EventPlaceKind::Existing,
            existing_place: "P0001".to_owned(),
            ..EventDraft::new()
        };
        assert_eq!(
            draft.to_request().place,
            EventPlaceRequest::Existing("P0001".to_owned())
        );
    }
}
