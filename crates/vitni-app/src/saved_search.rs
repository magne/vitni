//! Saved searches (ADR 0052 §8-10): named list queries stored in `workspace.toml`.
//!
//! The manifest holds each entry as a raw TOML table so one entry this version cannot read never
//! blocks the others, and is written back with the same keys and values (the TOML is re-serialized, so
//! comments and key order are not kept). An entry with a field this version
//! does not know is unreadable too, so a rewrite never strips a field a newer version added, and so is
//! one that repeats the id of an earlier entry, so it cannot take the first one with it.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use vitni_db::ListedKind;

/// The identity of a saved search (UUID v7, minted by the `Session`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SavedSearchId(Uuid);

impl SavedSearchId {
    /// Wraps an id minted by the caller.
    #[must_use]
    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for SavedSearchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// The query a saved search stores. Clauses and the sort stay raw TOML so a clause on a column or
/// operator this version does not know still loads and is written back unchanged (ADR 0052 §10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredQuery {
    /// The kind of record the query lists.
    pub kind: ListedKind,
    /// The free-text search.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub search: String,
    /// The filter clauses, as stored.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<toml::Value>,
    /// The sort, as stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<toml::Value>,
    /// The column ids to show; absent means the kind's default columns, empty means none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
}

/// A named, stored query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedSearch {
    /// The saved search's identity.
    pub id: SavedSearchId,
    /// The user-given name, unique per workspace ignoring case.
    pub name: String,
    /// The stored query.
    pub query: StoredQuery,
}

/// One `[[saved_searches]]` entry as loaded: understood, or kept raw so a rewrite loses nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum SavedSearchEntry {
    /// An entry this version understands.
    Readable(SavedSearch),
    /// An entry this version cannot use, with the original table: one that cannot be parsed, carries a
    /// field this version does not know, or repeats the id of an earlier entry.
    Unreadable {
        /// The table exactly as found.
        raw: toml::Value,
    },
}

impl SavedSearchEntry {
    /// Interprets the entry at zero-based `position` in `[[saved_searches]]`.
    #[must_use]
    pub fn from_value(position: usize, raw: &toml::Value) -> Self {
        match raw.clone().try_into::<SavedSearch>() {
            Ok(search) => Self::Readable(search),
            Err(error) => {
                tracing::warn!(position, %error, "saved search is unreadable and is kept unchanged");
                Self::Unreadable { raw: raw.clone() }
            }
        }
    }

    /// The TOML table to store for this entry; an unreadable entry is returned as found.
    #[must_use]
    pub fn to_value(&self) -> toml::Value {
        match self {
            Self::Readable(search) => toml::Value::try_from(search).unwrap_or_else(|error| {
                // Unreachable: every field is a string, a uuid, a unit enum or already TOML.
                tracing::error!(%error, id = ?search.id, "saved search could not be serialised");
                toml::Value::Table(toml::Table::new())
            }),
            Self::Unreadable { raw } => raw.clone(),
        }
    }
}
