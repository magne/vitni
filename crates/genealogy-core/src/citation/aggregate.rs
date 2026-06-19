//! The thin `cqrs-es` adapter over the pure Citation decision core (ADR 0002, ADR 0004 §3).
//!
//! Unlike the reference Person adapter, Citation has a cross-aggregate reference, so its `Services`
//! is an [`Arc<dyn CitationRefResolver>`] rather than `()`. `handle` resolves the cross-aggregate
//! facts (does the cited source exist?) through that resolver — the aggregate-tax read of
//! ADR 0004 §3 — then passes the resolved [`CitationRefs`](crate::citation::ref_resolver::CitationRefs)
//! into the pure `decide`. The impure projection read lives here; the rule stays in `decide`.

use std::sync::Arc;

use cqrs_es::Aggregate;
use cqrs_es::event_sink::EventSink;

use crate::citation::command::{CitationCommand, CitationCommandEnvelope};
use crate::citation::decide::{decide, evolve};
use crate::citation::error::CitationError;
use crate::citation::event::CitationEvent;
use crate::citation::ref_resolver::CitationRefResolver;
use crate::citation::state::CitationState;

impl Aggregate for CitationState {
    const TYPE: &'static str = "citation";
    type Command = CitationCommandEnvelope;
    type Event = CitationEvent;
    type Error = CitationError;
    type Services = Arc<dyn CitationRefResolver>;

    async fn handle(
        &mut self,
        command: Self::Command,
        services: &Self::Services,
        sink: &EventSink<Self>,
    ) -> Result<(), Self::Error> {
        let CitationCommandEnvelope { meta, command }: CitationCommandEnvelope = command;
        let command: CitationCommand = command;
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
