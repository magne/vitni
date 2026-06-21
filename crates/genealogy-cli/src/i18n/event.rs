use super::{EventError, EventSummary, EventType, Localizer, fl};

impl Localizer {
    /// `No events yet.`
    #[must_use]
    pub fn event_list_empty(&self) -> String {
        fl!(self.loader, "event-list-empty")
    }

    /// One event line: `E0001  type: birth  date: 1847-03-12  place: P0001`.
    #[must_use]
    pub fn event_summary_line(&self, summary: &EventSummary) -> String {
        let event_type = match &summary.event_type {
            Some(event_type) => self.event_type(event_type),
            None => fl!(self.loader, "no-value"),
        };
        let date = match &summary.date {
            Some(date) => self.date(date),
            None => fl!(self.loader, "no-value"),
        };
        let place = match &summary.place {
            Some(place) => place.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let description = match &summary.description {
            Some(description) => description.clone(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "event-summary",
            id = summary.human_id.clone(),
            event_type = event_type,
            date = date,
            place = place,
            description = description,
            participants = summary.participant_count.to_string()
        )
    }

    /// The localized event-type label; a custom [`EventType::Custom`] value renders verbatim.
    fn event_type(&self, event_type: &EventType) -> String {
        match event_type {
            EventType::Birth => fl!(self.loader, "event-type-birth"),
            EventType::Death => fl!(self.loader, "event-type-death"),
            EventType::Marriage => fl!(self.loader, "event-type-marriage"),
            EventType::Baptism => fl!(self.loader, "event-type-baptism"),
            EventType::Burial => fl!(self.loader, "event-type-burial"),
            EventType::Census => fl!(self.loader, "event-type-census"),
            EventType::Residence => fl!(self.loader, "event-type-residence"),
            EventType::Immigration => fl!(self.loader, "event-type-immigration"),
            EventType::Emigration => fl!(self.loader, "event-type-emigration"),
            EventType::Custom(value) => value.clone(),
        }
    }

    pub(super) fn event_error(&self, error: &EventError) -> String {
        match error {
            EventError::NotFound(id) => fl!(self.loader, "err-event-not-exist", id = id.to_string()),
            EventError::AlreadyExists(id) => fl!(self.loader, "err-event-exists", id = id.to_string()),
            EventError::UnknownPlace(id) => fl!(self.loader, "err-unknown-place", id = id.to_string()),
            EventError::RetractsMissingAssertion(id) | EventError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }
}
