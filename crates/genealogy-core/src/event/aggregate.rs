//! The thin `cqrs-es` adapter over the pure Event decision core (ADR 0002, ADR 0004 §3).
//!
//! Like Citation, Event has a cross-aggregate reference, so its `Services` is an
//! [`Arc<dyn EventRefResolver>`]. `handle` resolves the cross-aggregate facts (does the linked
//! place exist?) through that resolver — the aggregate-tax read of ADR 0004 §3 — then passes the
//! resolved [`EventRefs`](crate::event::ref_resolver::EventRefs) into the pure `decide`.

use std::sync::Arc;

use cqrs_es::Aggregate;
use cqrs_es::event_sink::EventSink;

use crate::event::command::{EventCommand, EventCommandEnvelope};
use crate::event::decide::{decide, evolve};
use crate::event::error::EventError;
use crate::event::events::EventEvent;
use crate::event::ref_resolver::EventRefResolver;
use crate::event::state::EventState;

impl Aggregate for EventState {
    const TYPE: &'static str = "event";
    type Command = EventCommandEnvelope;
    type Event = EventEvent;
    type Error = EventError;
    type Services = Arc<dyn EventRefResolver>;

    async fn handle(
        &mut self,
        command: Self::Command,
        services: &Self::Services,
        sink: &EventSink<Self>,
    ) -> Result<(), Self::Error> {
        let EventCommandEnvelope { meta, command }: EventCommandEnvelope = command;
        let command: EventCommand = command;
        let refs = services.resolve(&command).await;
        let events = decide(self, command, &meta, &refs)?;
        for event in events {
            sink.write(event, self).await;
        }
        Ok(())
    }

    fn apply(&mut self, event: Self::Event) {
        evolve(self, &event);
    }
}
