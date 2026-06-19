//! `genealogy-ui` — the framework-agnostic presentation layer (ADR 0008).
//!
//! This crate sits between `genealogy-app` (use-cases + DTOs, ADR 0006) and a concrete framework
//! renderer (`genealogy-ui-dioxus` today). It holds **all presentation logic and no framework
//! types**: view-models derived from the app's DTOs ([`view_model`]), navigation/data intents and
//! their async dispatch to use-cases ([`navigation`], [`intent`]), Fluent string resolution
//! ([`i18n`], ADR 0003), and the plugin-UI [`vocabulary`] types (ADR 0012).
//!
//! Dependency direction is one-way: `genealogy-app → genealogy-ui → genealogy-ui-<framework>`. No
//! `dioxus::` (or other framework) type appears here, and neither does the plugin host — a renderer
//! drives the host and hands this crate the plugin's JSON to [`vocabulary::parse`].

pub mod i18n;
pub mod intent;
pub mod navigation;
pub mod view_model;
pub mod vocabulary;

pub use i18n::Localizer;
pub use intent::{IntentOutcome, dispatch};
pub use navigation::{Intent, Screen};
pub use view_model::{PersonDetail, PersonRow};
pub use vocabulary::{Field, Form, SelectOption, VocabularyError, parse};
