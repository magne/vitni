//! The thin `cqrs-es` adapter over the pure Source decision core (ADR 0002, ADR 0004 §3).
//!
//! Source has no cross-aggregate references, so its `Services` is `()` (contrast the Citation
//! aggregate, whose `Services` resolves the `UnknownSource` check — ADR 0004 §3).

use cqrs_es::Aggregate;
use cqrs_es::event_sink::EventSink;

use crate::source::command::{SourceCommand, SourceCommandEnvelope};
use crate::source::decide::{decide, evolve};
use crate::source::error::SourceError;
use crate::source::event::SourceEvent;
use crate::source::state::SourceState;

impl Aggregate for SourceState {
    const TYPE: &'static str = "source";
    type Command = SourceCommandEnvelope;
    type Event = SourceEvent;
    type Error = SourceError;
    type Services = ();

    async fn handle(
        &mut self,
        command: Self::Command,
        _services: &Self::Services,
        sink: &EventSink<Self>,
    ) -> Result<(), Self::Error> {
        let SourceCommandEnvelope { meta, command }: SourceCommandEnvelope = command;
        let command: SourceCommand = command;
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
