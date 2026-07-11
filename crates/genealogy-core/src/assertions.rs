//! Shared event-sourcing substrate: assertion tagging and the provenance envelope (ADR 0004).
//!
//! Every aggregate's events share one shape — an [`AssertionId`] and an [`EventContext`] in the
//! payload (ADR 0004 §1–§2), wrapped around a per-aggregate *body* enum. [`Envelope`] is that shape
//! once, generic over the body; each aggregate aliases it (`pub type PersonEvent =
//! Envelope<PersonEventBody>`) and implements [`EventBody`] on its body to supply the `cqrs-es`
//! event type and per-variant schema version (ADR 0004 §4).
//!
//! [`Attributed`] tags a folded value with the assertion that introduced it, so a non-destructive
//! correction ([`crate::provenance`] retract/supersede) can remove exactly the right entry. The
//! [`cqrs_adapter!`] macro generates the thin `cqrs-es` `Aggregate` adapter over a pure
//! `decide`/`evolve` core, which is otherwise identical across aggregates.

use cqrs_es::DomainEvent;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::ids::{AssertionId, CitationId};
use crate::provenance::{AssertionMeta, Confidence, EventContext};

/// A value tagged with the assertion that introduced it, so corrections can target it.
///
/// The event log keeps the original assertion forever; removing the matching `Attributed` entry
/// from the folded state is how a retraction or supersession stops the derived state reflecting a
/// withdrawn claim (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attributed<T> {
    /// The assertion that introduced this value.
    pub assertion_id: AssertionId,
    /// The value itself.
    pub value: T,
}

/// A folded value carrying the provenance the asserting operator stamped on it: the surety and the
/// backing citation ids, denormalized from the assertion's [`EventContext`] at fold time (ADR 0004
/// §1). Wrapped in an [`Attributed`] (`Attributed<Asserted<T>>`) so a correction still targets it by
/// `assertion_id`, while a read model can surface a row's surety + source count without re-reading
/// the log. Mirrors the per-aggregate `AssertedPartner`-style structs as one generic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asserted<T> {
    /// The value itself.
    pub value: T,
    /// The operator's surety when asserting the value (data-model §8). `None` = no judgment recorded.
    pub confidence: Option<Confidence>,
    /// The citations backing the value (`EventContext.citations`).
    pub citations: Vec<CitationId>,
}

impl<T> Asserted<T> {
    /// Builds an [`Asserted`] from a value and the asserting event's provenance context.
    #[must_use]
    pub fn from_context(value: T, context: &EventContext) -> Self {
        Self {
            value,
            confidence: context.confidence,
            citations: context.citations.iter().map(|c| c.citation_id).collect(),
        }
    }
}

/// The store-facing metadata a domain-event body exposes (ADR 0004 §4).
///
/// `type_name` is the `cqrs-es` event type (the variant name). `version` is the **per-variant**
/// payload schema version: a variant is bumped only when its own payload changes additively, so
/// an unevolved variant stays `"1.0"` while a sibling advances (see `event::EventEventBody`).
pub trait EventBody {
    /// The variant name, used as the `cqrs-es` event type.
    fn type_name(&self) -> &'static str;
    /// The payload schema version of this variant.
    fn version(&self) -> &'static str;
}

/// A single assertion plus its provenance envelope (ADR 0004 §1–§2).
///
/// Generic over the per-aggregate body; the body is internally tagged (`type` discriminator) and
/// flattened in, so a stored event is one flat JSON object carrying `assertion_id`, the context,
/// and the body fields under its `type`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope<B> {
    /// Identity of this assertion, so a correction can target it (ADR 0004 §2).
    pub assertion_id: AssertionId,
    /// Who / when / why / how sure / on what evidence (data-model §8).
    pub context: EventContext,
    /// The claim itself.
    #[serde(flatten)]
    pub body: B,
}

impl<B> Envelope<B> {
    /// Stamps `body` with the supplied assertion id and context (ADR 0004 §3).
    #[must_use]
    pub fn new(meta: &AssertionMeta, body: B) -> Self {
        Self {
            assertion_id: meta.assertion_id,
            context: meta.context.clone(),
            body,
        }
    }
}

impl<B> DomainEvent for Envelope<B>
where
    B: EventBody + Clone + PartialEq + std::fmt::Debug + Serialize + DeserializeOwned + Send + Sync,
{
    fn event_type(&self) -> String {
        self.body.type_name().to_owned()
    }

    fn event_version(&self) -> String {
        self.body.version().to_owned()
    }
}

/// Generates the thin `cqrs-es` [`Aggregate`](cqrs_es::Aggregate) adapter over a pure
/// `decide`/`evolve` core (ADR 0002, ADR 0004 §3).
///
/// The adapter is identical across aggregates apart from the cross-aggregate `Services`: `handle`
/// unpacks the command envelope (`meta` + `command`), optionally resolves cross-aggregate facts,
/// calls the pure `decide`, and writes the events; `apply` delegates to `evolve`. Two forms: with a
/// resolver `Services` (its `resolve(&command).await` feeds `decide`) and without (`Services = ()`).
#[macro_export]
macro_rules! cqrs_adapter {
    (
        state: $State:ty,
        type: $type_name:literal,
        command: $Command:ty,
        event: $Event:ty,
        error: $Error:ty,
        decide: $decide:path,
        evolve: $evolve:path $(,)?
    ) => {
        impl ::cqrs_es::Aggregate for $State {
            const TYPE: &'static str = $type_name;
            type Command = $Command;
            type Event = $Event;
            type Error = $Error;
            type Services = ();

            async fn handle(
                &mut self,
                command: Self::Command,
                _services: &Self::Services,
                sink: &::cqrs_es::event_sink::EventSink<Self>,
            ) -> ::core::result::Result<(), Self::Error> {
                let events = $decide(self, command.command, &command.meta)?;
                for event in events {
                    sink.write(event, self).await;
                }
                ::core::result::Result::Ok(())
            }

            fn apply(&mut self, event: Self::Event) {
                $evolve(self, &event);
            }
        }
    };
    (
        state: $State:ty,
        type: $type_name:literal,
        command: $Command:ty,
        event: $Event:ty,
        error: $Error:ty,
        services: $Services:ty,
        decide: $decide:path,
        evolve: $evolve:path $(,)?
    ) => {
        impl ::cqrs_es::Aggregate for $State {
            const TYPE: &'static str = $type_name;
            type Command = $Command;
            type Event = $Event;
            type Error = $Error;
            type Services = $Services;

            async fn handle(
                &mut self,
                command: Self::Command,
                services: &Self::Services,
                sink: &::cqrs_es::event_sink::EventSink<Self>,
            ) -> ::core::result::Result<(), Self::Error> {
                let refs = services.resolve(&command.command).await;
                let events = $decide(self, command.command, &command.meta, &refs)?;
                for event in events {
                    sink.write(event, self).await;
                }
                ::core::result::Result::Ok(())
            }

            fn apply(&mut self, event: Self::Event) {
                $evolve(self, &event);
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::Asserted;
    use crate::ids::AgentId;
    use crate::provenance::{Agent, AgentKind, Confidence, EventContext, Timestamp};
    use time::macros::datetime;
    use uuid::Uuid;

    fn context(confidence: Option<Confidence>) -> EventContext {
        EventContext {
            operator: Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(1)),
                display: None,
            },
            occurred_at: Timestamp::new(datetime!(2026-06-17 12:00:00 UTC)),
            rationale: None,
            confidence,
            citations: Vec::new(),
            evidence_analysis: None,
        }
    }

    #[test]
    fn from_context_passes_some_confidence_through() {
        let asserted: Asserted<&str> = Asserted::from_context("v", &context(Some(Confidence::High)));
        assert_eq!(asserted.confidence, Some(Confidence::High));
    }

    #[test]
    fn from_context_passes_none_confidence_through() {
        let asserted: Asserted<&str> = Asserted::from_context("v", &context(None));
        assert_eq!(asserted.confidence, None);
    }
}
