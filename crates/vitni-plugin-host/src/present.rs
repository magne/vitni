//! The `present` capability's frontend contract (ADR 0017 §5).
//!
//! `present.show` is the first *suspending* host call: the guest hands the frontend an opaque payload
//! and the invocation waits until the user answers. Where `progress` is a synchronous
//! [`ProgressFn`](crate::ProgressFn) callback, [`Presenter`] is its async sibling — the host awaits
//! it inside the `present::show` implementation while the assisted-import invocation runs on a
//! background task. The GUI's implementation forwards the payload to a Dioxus signal and awaits the
//! user's response; a dropped channel becomes [`PresentError::Backend`], which the host maps onto
//! `capability-error::backend`.

use async_trait::async_trait;

/// Why presenting to the frontend failed. Every variant maps onto `capability-error::backend` — a
/// `present` failure is always an infrastructure fault (the frontend is gone or the channel dropped),
/// never a domain rejection.
#[derive(Debug, thiserror::Error)]
pub enum PresentError {
    /// The frontend could not be reached, or dropped the channel before answering (ADR 0017 §5's
    /// cancellation-by-channel-drop path).
    #[error("presenting to the frontend failed: {0}")]
    Backend(String),
}

/// A frontend that can show an assisted-import payload and return the user's response (ADR 0017 §5).
///
/// The payload and response are opaque strings to the host — the payload is the typed presentation
/// contract `vitni-ui` parses, not the ADR 0022 UI vocabulary. Implementations run on the
/// frontend side of a channel; the async `present` suspends the invocation until the user answers.
#[async_trait]
pub trait Presenter: Send {
    /// Shows `payload` and resolves with the user's response, or a [`PresentError`] if the frontend
    /// could not be reached.
    async fn present(&mut self, payload: String) -> Result<String, PresentError>;
}
