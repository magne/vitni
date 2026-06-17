//! The thin `cqrs-es` adapter over the pure Person decision core (ADR 0002, ADR 0004 §3).
//!
//! This is the *only* place the Person aggregate touches the framework. `handle` unpacks the
//! supplied [`AssertionMeta`], calls the pure [`decide`], and writes the resulting events to the
//! sink (which also applies each to `self`); `apply` delegates to the pure [`evolve`]. All
//! business rules live in `decide`/`evolve`, keeping the door open to a different store (ADR 0002).

use cqrs_es::Aggregate;
use cqrs_es::event_sink::EventSink;

use crate::person::command::{PersonCommand, PersonCommandEnvelope};
use crate::person::decide::{decide, evolve};
use crate::person::error::PersonError;
use crate::person::event::PersonEvent;
use crate::person::state::PersonState;

impl Aggregate for PersonState {
    const TYPE: &'static str = "person";
    type Command = PersonCommandEnvelope;
    type Event = PersonEvent;
    type Error = PersonError;
    type Services = ();

    async fn handle(
        &mut self,
        command: Self::Command,
        _services: &Self::Services,
        sink: &EventSink<Self>,
    ) -> Result<(), Self::Error> {
        let PersonCommandEnvelope { meta, command }: PersonCommandEnvelope = command;
        let command: PersonCommand = command;
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
