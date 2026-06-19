//! The thin `cqrs-es` adapter over the pure Place decision core (ADR 0002, ADR 0004 §3).
//!
//! The only place the Place aggregate touches the framework. `handle` unpacks the supplied
//! [`AssertionMeta`], calls the pure [`decide`], and writes the resulting events to the sink;
//! `apply` delegates to the pure [`evolve`]. Place has no cross-aggregate references, so its
//! `Services` is `()` (contrast the Citation/Event aggregates — ADR 0004 §3).

use cqrs_es::Aggregate;
use cqrs_es::event_sink::EventSink;

use crate::place::command::{PlaceCommand, PlaceCommandEnvelope};
use crate::place::decide::{decide, evolve};
use crate::place::error::PlaceError;
use crate::place::event::PlaceEvent;
use crate::place::state::PlaceState;

impl Aggregate for PlaceState {
    const TYPE: &'static str = "place";
    type Command = PlaceCommandEnvelope;
    type Event = PlaceEvent;
    type Error = PlaceError;
    type Services = ();

    async fn handle(
        &mut self,
        command: Self::Command,
        _services: &Self::Services,
        sink: &EventSink<Self>,
    ) -> Result<(), Self::Error> {
        let PlaceCommandEnvelope { meta, command }: PlaceCommandEnvelope = command;
        let command: PlaceCommand = command;
        let events = decide(self, command, &meta)?;
        for event in events {
            sink.write(event, self).await;
        }
        Ok(())
    }

    fn apply(&mut self, event: Self::Event) {
        evolve(self, &event);
    }
}
