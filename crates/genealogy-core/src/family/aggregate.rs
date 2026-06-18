//! The thin `cqrs-es` adapter over the pure Family decision core (ADR 0002, ADR 0004 §3).
//!
//! This is the *only* place the Family aggregate touches the framework. `handle` unpacks the
//! supplied [`AssertionMeta`], calls the pure [`decide`], and writes the resulting events to the
//! sink (which also applies each to `self`); `apply` delegates to the pure [`evolve`]. All business
//! rules live in `decide`/`evolve`, keeping the door open to a different store (ADR 0002).

use cqrs_es::Aggregate;
use cqrs_es::event_sink::EventSink;

use crate::family::command::{FamilyCommand, FamilyCommandEnvelope};
use crate::family::decide::{decide, evolve};
use crate::family::error::FamilyError;
use crate::family::event::FamilyEvent;
use crate::family::state::FamilyState;

impl Aggregate for FamilyState {
    const TYPE: &'static str = "family";
    type Command = FamilyCommandEnvelope;
    type Event = FamilyEvent;
    type Error = FamilyError;
    type Services = ();

    async fn handle(
        &mut self,
        command: Self::Command,
        _services: &Self::Services,
        sink: &EventSink<Self>,
    ) -> Result<(), Self::Error> {
        let FamilyCommandEnvelope { meta, command }: FamilyCommandEnvelope = command;
        let command: FamilyCommand = command;
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
