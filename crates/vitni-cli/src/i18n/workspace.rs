//! The warning a command prints on stderr when the workspace holds events it cannot read (ADR 0051).

use vitni_app::UnreadableEvent;

use super::{Localizer, fl};

impl Localizer {
    /// How many events could not be read and what to do about it, then each event on its own
    /// indented line, named by its aggregate and sequence.
    #[must_use]
    pub fn unreadable_events(&self, events: &[UnreadableEvent]) -> String {
        let mut warning = fl!(self.loader, "workspace-unreadable-events", count = events.len());
        for event in events {
            warning.push_str("\n  ");
            warning.push_str(&fl!(
                self.loader,
                "workspace-unreadable-event",
                kind = event.aggregate_type.clone(),
                id = event.aggregate_id.clone(),
                sequence = event.sequence.to_string(),
                event = event.event_type.clone(),
                reason = event.reason.clone()
            ));
        }
        warning
    }
}
