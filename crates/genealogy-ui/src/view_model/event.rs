use super::{
    AddressVm, AttachedRefVm, CitationRefVm, ConfidenceLevel, DateDraft, DetailTab, EventChangeSetRequest, EventEdit,
    EventPlaceRequest, EventRow, EventType, GenealogicalDate, HistoryEntryVm, Localizer, MediaRefVm, NewPlaceFields,
    RecordDraft, RecordLink, RestrictionKind, RowVm, TagRef, citation_ref_from_ref, non_blank,
};
use crate::picker::PickerSelection;

/// One event participant (Participants tab): the person, their role, surety, and source count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantVm {
    /// The participant's user-facing id (e.g. `I0001`).
    pub human_id: String,
    /// The participant's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The participant's display name (falls back to the `human_id`).
    pub name: String,
    /// The participant's raw role (seeds the edit form's role select).
    pub role: genealogy_app::ParticipantRole,
    /// The localized participant-role label.
    pub role_label: String,
    /// The participant's age at the event, for edit prefill (kept alongside `age_label`), if recorded.
    pub age: Option<genealogy_app::Age>,
    /// The localized age label (e.g. `over 42y`), if an age is recorded (ADR 0019).
    pub age_label: Option<String>,
    /// The participant-scoped typed attributes (ADR 0019), for the edit form's pre-fill.
    pub attributes: Vec<genealogy_app::Attribute>,
    /// The `human_id`s of notes about this participation (ADR 0019), for the edit form's pre-fill.
    pub notes: Vec<String>,
    /// The operator's surety in the participation (drives the confidence badge).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the participation.
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) that introduced this participation — the target a per-row
    /// Edit supersedes and a Remove retracts (ADR 0004 §2). Never rendered. Always the person-side
    /// (canonical) assertion.
    pub assertion_id: String,
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
    /// The event's raw type, if set (seeds the whole-record editor's Type select).
    pub event_type: Option<EventType>,
    /// The localized event-type label.
    pub type_label: String,
    /// The localized date, if known.
    pub date: Option<String>,
    /// The structured date, if asserted (seeds the whole-record editor).
    pub date_value: Option<genealogy_app::GenealogicalDate>,
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
    /// The recorded postal addresses, each with the `AssertionId` that introduced it (Addresses tab).
    pub addresses: Vec<AddressVm>,
    /// The participants, joined to the person projection.
    pub participants: Vec<ParticipantVm>,
    /// The citations backing the event, with source · page · surety · evidence axes.
    pub citations: Vec<CitationRefVm>,
    /// The attached media objects.
    pub media: Vec<MediaRefVm>,
    /// The attached notes, each with its attach `AssertionId` (the Detach target).
    pub notes: Vec<AttachedRefVm>,
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
                let confidence = participant.confidence.map(ConfidenceLevel::from);
                ParticipantVm {
                    human_id: participant.human_id.clone(),
                    id: participant.id.clone(),
                    name: participant.name.clone().unwrap_or_else(|| participant.human_id.clone()),
                    role: participant.role.clone(),
                    role_label: loc.participant_role_label(&participant.role),
                    age: participant.age.clone(),
                    age_label: participant.age.as_ref().map(|age| loc.age_label(age)),
                    attributes: participant.attributes.clone(),
                    notes: participant.notes.clone(),
                    confidence,
                    confidence_label: loc.confidence_label_opt(confidence),
                    source_count: participant.source_count,
                    assertion_id: participant.assertion_id.clone(),
                }
            })
            .collect();
        let date_confidence = summary.date_confidence.map(ConfidenceLevel::from);
        let place_confidence = summary.place_confidence.map(ConfidenceLevel::from);
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: type_label.clone(),
            event_type: summary.event_type.clone(),
            type_label,
            date: summary.date.as_ref().map(|date| loc.date(date)),
            date_value: summary.date.clone(),
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
            addresses: summary
                .addresses
                .iter()
                .map(|a| AddressVm {
                    address: a.address.clone(),
                    assertion_id: a.assertion_id.clone(),
                })
                .collect(),
            participants,
            citations: summary
                .citations
                .iter()
                .map(|citation| citation_ref_from_ref(citation, loc))
                .collect(),
            media: summary.media.iter().map(MediaRefVm::from_ref).collect(),
            notes: summary.notes.iter().map(AttachedRefVm::from_ref).collect(),
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
    let place = summary
        .place
        .as_ref()
        .map(|p| p.name.clone().unwrap_or_else(|| p.human_id.clone()));
    event_row_fields(
        &summary.human_id,
        summary.event_type.as_ref(),
        summary.date.as_ref(),
        place,
        loc,
    )
}

/// Builds a generic list row from a lightweight [`EventRow`] (the list view's per-row DTO), the same
/// rendering as [`event_row`] without loading a full summary.
#[must_use]
pub fn event_list_row(row: &EventRow, loc: &Localizer) -> RowVm {
    let place = row
        .place
        .as_ref()
        .map(|place| place.name.clone().unwrap_or_else(|| place.human_id.clone()));
    event_row_fields(&row.human_id, row.event_type.as_ref(), row.date.as_ref(), place, loc)
}

/// The shared [`RowVm`] builder behind [`event_row`] and [`event_list_row`]: the localized type label
/// (or the `human_id` when untyped) as the title, a `date · place` subtitle, and the per-type avatar.
fn event_row_fields(
    human_id: &str,
    event_type: Option<&EventType>,
    date: Option<&GenealogicalDate>,
    place: Option<String>,
    loc: &Localizer,
) -> RowVm {
    let title = event_type.map_or_else(|| human_id.to_owned(), |event_type| loc.event_type_label(event_type));
    let date = date.map(|date| loc.date(date));
    let subtitle = match (date, place) {
        (Some(date), Some(place)) => Some(format!("{date} · {place}")),
        (Some(date), None) => Some(date),
        (None, Some(place)) => Some(place),
        (None, None) => None,
    };
    RowVm {
        id: human_id.to_owned(),
        title,
        subtitle,
        avatar: Some(event_avatar(event_type)),
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
        tab("addresses", Some(detail.addresses.len())),
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

/// The in-memory draft for an event (`record-editing.html` §6): a required type, a structured date,
/// a description, and an optional place (unset, existing, or created inline — §6b). On create nothing
/// is written until Save commits an [`EventChangeSetRequest`]; on edit each changed field emits its
/// [`EventEdit`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDraft {
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The editable user-facing id; blank ⇒ generated on save (edit mode only — create auto-allocates).
    pub human_id: String,
    /// The event type (required).
    pub event_type: EventType,
    /// The structured date (`event.html` control cluster). On create it is carried in the change-set
    /// request and asserted after the commit; on edit a change emits a `SetDate` on Save. A blank
    /// draft emits nothing.
    pub date: DateDraft,
    /// The free-text description.
    pub description: String,
    /// The place the event occurred (unset, an existing place, or a new place created inline — §6b).
    /// On edit only the existing link is honoured (there is no inline-create-on-edit command).
    pub place: RecordLink<NewPlaceFields>,
}

impl Default for EventDraft {
    fn default() -> Self {
        Self {
            existing_human_id: None,
            human_id: String::new(),
            event_type: DEFAULT_EVENT_TYPE,
            date: DateDraft::default(),
            description: String::new(),
            place: RecordLink::Empty,
        }
    }
}

impl EventDraft {
    /// A fresh draft for creating a new event (default type, no place).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A draft pre-populated from an existing event for editing. Records the current `human_id` so
    /// [`Self::edits_against`] diffs (supersedes) rather than creates; seeds the type/description and
    /// the linked place's id (the create-only inline-new fields stay at their defaults).
    #[must_use]
    pub fn from_detail(detail: &EventDetail) -> Self {
        let place = detail.place.as_ref().map_or(RecordLink::Empty, |place| {
            RecordLink::Existing(PickerSelection {
                human_id: place.human_id.clone(),
                title: place.name.clone(),
            })
        });
        Self {
            existing_human_id: Some(detail.human_id.clone()),
            human_id: detail.human_id.clone(),
            event_type: detail.event_type.clone().unwrap_or(DEFAULT_EVENT_TYPE),
            date: detail.date_value.as_ref().map_or_else(DateDraft::default, |value| {
                DateDraft::from_value(value, detail.date.clone().unwrap_or_default())
            }),
            description: detail.description.clone().unwrap_or_default(),
            place,
        }
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode): one `Set*` per changed scalar, the place link as a [`EventEdit::LinkPlace`] when the
    /// existing-place id changed to a non-blank value (there is no unlink command), and `SetHumanId`
    /// last so the record is only re-keyed after every other field has committed (a blank id
    /// regenerates).
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<EventEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        let mut edits = Vec::new();
        if self.date != seed.date
            && let Ok(Some(date)) = self.date.to_input()
        {
            edits.push(EventEdit::SetDate {
                human_id: human_id.clone(),
                date,
            });
        }
        if self.event_type != seed.event_type {
            edits.push(EventEdit::SetType {
                human_id: human_id.clone(),
                event_type: self.event_type.clone(),
            });
        }
        if self.description != seed.description {
            edits.push(EventEdit::SetDescription {
                human_id: human_id.clone(),
                description: self.description.clone(),
            });
        }
        if self.place.existing_id() != seed.place.existing_id()
            && let Some(place_id) = self.place.existing_id()
        {
            edits.push(EventEdit::LinkPlace {
                human_id: human_id.clone(),
                place_id: place_id.to_owned(),
            });
        }
        if self.human_id.trim() != seed.human_id {
            edits.push(EventEdit::SetHumanId {
                human_id,
                new_human_id: non_blank(&self.human_id),
            });
        }
        edits
    }

    /// Builds the [`EventChangeSetRequest`] the app commits on Save.
    #[must_use]
    pub fn to_request(&self) -> EventChangeSetRequest {
        let place = match &self.place {
            RecordLink::Empty => EventPlaceRequest::None,
            RecordLink::Existing(selection) => EventPlaceRequest::Existing(selection.human_id.clone()),
            RecordLink::New(fields) => EventPlaceRequest::New {
                place_type: fields.place_type.clone(),
                name: non_blank(&fields.name),
            },
        };
        EventChangeSetRequest {
            event_type: self.event_type.clone(),
            description: non_blank(&self.description),
            place,
            date: self.date.to_input().ok().flatten(),
        }
    }
}

impl RecordDraft for EventDraft {
    type Detail = EventDetail;

    fn from_detail(detail: &EventDetail) -> Self {
        Self::from_detail(detail)
    }

    fn is_valid(&self) -> bool {
        !self.date.is_invalid()
    }
}

#[cfg(test)]
mod event_detail_tests {
    use super::EventDetail;
    use crate::i18n::Localizer;
    use genealogy_app::{Confidence, EventSummary, EventType, ParticipantRef, ParticipantRole};
    use std::collections::BTreeSet;

    fn participant(
        human_id: &str,
        confidence: Option<Confidence>,
        source_count: usize,
        assertion_id: &str,
    ) -> ParticipantRef {
        ParticipantRef {
            human_id: human_id.to_owned(),
            id: format!("{human_id}-id"),
            name: Some(human_id.to_owned()),
            role: ParticipantRole::Witness,
            age: None,
            attributes: Vec::new(),
            notes: Vec::new(),
            confidence,
            source_count,
            assertion_id: assertion_id.to_owned(),
        }
    }

    fn event_summary(participants: Vec<ParticipantRef>) -> EventSummary {
        EventSummary {
            human_id: "E0001".to_owned(),
            id: "e-id".to_owned(),
            event_type: Some(EventType::Marriage),
            event_type_confidence: None,
            date: None,
            date_confidence: None,
            date_source_count: 0,
            date_citations: Vec::new(),
            place: None,
            place_confidence: None,
            description: None,
            addresses: Vec::new(),
            participants,
            citations: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            restrictions: BTreeSet::new(),
        }
    }

    #[test]
    fn an_event_participants_tab_carries_each_participants_provenance() {
        let loc = Localizer::for_test("en");
        let summary = event_summary(vec![
            participant("I0001", Some(Confidence::High), 2, "a1"),
            participant("I0002", None, 0, "a2"),
        ]);
        let detail = EventDetail::from_summary(&summary, &loc);
        assert_eq!(detail.participants.len(), 2);
        assert_eq!(detail.participants[0].source_count, 2);
        assert_eq!(detail.participants[0].assertion_id, "a1");
        assert_eq!(detail.participants[1].source_count, 0);
        assert_eq!(detail.participants[1].assertion_id, "a2");
        assert_eq!(
            detail.participants[1].confidence_label, "No judgment",
            "a participation with no surety judgment renders the unset label (ADR 0021 §5)"
        );
    }
}

#[cfg(test)]
mod event_draft_tests {
    use super::{DateDraft, EventDraft, NewPlaceFields, RecordDraft, RecordLink};
    use crate::navigation::{EventEdit, EventPlaceRequest};
    use crate::picker::PickerSelection;
    use genealogy_app::{EventType, PlaceType};

    fn existing_place(human_id: &str) -> RecordLink<NewPlaceFields> {
        RecordLink::Existing(PickerSelection {
            human_id: human_id.to_owned(),
            title: human_id.to_owned(),
        })
    }

    #[test]
    fn a_fresh_draft_is_not_dirty() {
        assert!(!EventDraft::new().is_dirty_against(&EventDraft::new()));
    }

    #[test]
    fn a_changed_type_or_description_makes_it_dirty() {
        assert!(
            EventDraft {
                event_type: EventType::Baptism,
                ..EventDraft::new()
            }
            .is_dirty_against(&EventDraft::new())
        );
        assert!(
            EventDraft {
                description: "at church".to_owned(),
                ..EventDraft::new()
            }
            .is_dirty_against(&EventDraft::new())
        );
    }

    fn edit_seed() -> EventDraft {
        EventDraft {
            existing_human_id: Some("E0001".to_owned()),
            human_id: "E0001".to_owned(),
            event_type: EventType::Birth,
            description: "at home".to_owned(),
            place: existing_place("P0001"),
            ..EventDraft::new()
        }
    }

    #[test]
    fn an_unchanged_event_yields_no_edits() {
        assert!(edit_seed().edits_against(&edit_seed()).is_empty());
    }

    #[test]
    fn a_changed_type_and_description_each_yield_one_edit() {
        let draft = EventDraft {
            event_type: EventType::Baptism,
            description: "at church".to_owned(),
            ..edit_seed()
        };
        let edits = draft.edits_against(&edit_seed());
        assert_eq!(edits.len(), 2);
        assert!(matches!(&edits[0], EventEdit::SetType { .. }));
        assert!(matches!(&edits[1], EventEdit::SetDescription { .. }));
    }

    #[test]
    fn a_changed_place_yields_one_link_place() {
        let draft = EventDraft {
            place: existing_place("P0009"),
            ..edit_seed()
        };
        let edits = draft.edits_against(&edit_seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], EventEdit::LinkPlace { place_id, .. } if place_id == "P0009"));
    }

    #[test]
    fn a_blank_id_regenerates_and_is_emitted_last() {
        let draft = EventDraft {
            event_type: EventType::Baptism,
            human_id: String::new(),
            ..edit_seed()
        };
        let edits = draft.edits_against(&edit_seed());
        assert_eq!(edits.len(), 2);
        assert!(matches!(&edits[0], EventEdit::SetType { .. }));
        assert!(matches!(&edits[1], EventEdit::SetHumanId { new_human_id, .. } if new_human_id.is_none()));
    }

    #[test]
    fn a_new_place_maps_to_a_pending_place_request() {
        let draft = EventDraft {
            place: RecordLink::New(NewPlaceFields {
                place_type: PlaceType::Building,
                name: "Trinity Church".to_owned(),
            }),
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
            place: existing_place("P0001"),
            ..EventDraft::new()
        };
        assert_eq!(
            draft.to_request().place,
            EventPlaceRequest::Existing("P0001".to_owned())
        );
    }

    fn typed_date(text: &str) -> DateDraft {
        DateDraft {
            start: text.to_owned(),
            ..DateDraft::default()
        }
    }

    #[test]
    fn a_changed_date_makes_it_dirty_and_emits_set_date() {
        let draft = EventDraft {
            date: typed_date("14 Jun 1876"),
            ..edit_seed()
        };
        assert!(draft.is_dirty_against(&edit_seed()));
        let edits = draft.edits_against(&edit_seed());
        assert_eq!(edits.len(), 1);
        let EventEdit::SetDate { date, .. } = &edits[0] else {
            panic!("expected a SetDate, got {:?}", edits[0]);
        };
        assert_eq!(*date, typed_date("14 Jun 1876").to_input().unwrap().unwrap());
    }

    #[test]
    fn an_untouched_date_emits_no_set_date() {
        assert!(edit_seed().edits_against(&edit_seed()).is_empty());
    }

    #[test]
    fn an_invalid_date_blocks_validity() {
        let draft = EventDraft {
            date: typed_date("gibberish"),
            ..edit_seed()
        };
        assert!(!draft.is_valid());
    }

    #[test]
    fn a_create_request_carries_a_parsed_date() {
        let draft = EventDraft {
            date: typed_date("14 Jun 1876"),
            ..EventDraft::new()
        };
        assert_eq!(draft.to_request().date, typed_date("14 Jun 1876").to_input().unwrap());
    }

    #[test]
    fn a_blank_create_date_maps_to_none() {
        assert!(EventDraft::new().to_request().date.is_none());
    }
}
