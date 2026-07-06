use super::{
    CitationRefVm, ConfidenceLevel, DetailTab, EventType, FamilyMediaVm, HistoryEntryVm, Localizer, RestrictionKind,
    RowVm, TagRef, citation_ref_from_ref,
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
