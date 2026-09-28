//! Record origin: which source record an imported assertion was read from (ADR 0037 §1, §3).
//!
//! A [`RecordOrigin`] rides on [`crate::provenance::EventContext`] as mechanical provenance. It names
//! the dataset, the record within it, the entity within that record, and the import run that wrote
//! the assertion. An assertion made at the keyboard carries none. `(dataset, record, item)` names one
//! entity in one record.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ids::ImportRunId;

/// A dataset: the namespace that scopes a record id (ADR 0037 §3).
///
/// A **global** dataset is its scheme alone (`digitalarkivet`), because its record ids are unique
/// archive-wide. A **lineage** dataset is `<scheme>:<uuid>` (`gedcom:…`, `gramps:…`) and names one
/// family-tree file re-exported over time; the UUID v7 is minted by the application when the file is
/// first imported.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DatasetId(String);

impl DatasetId {
    /// The dataset whose record ids are global to `scheme`.
    #[must_use]
    pub fn global(scheme: &str) -> Self {
        Self(scheme.to_owned())
    }

    /// The file-lineage dataset `<scheme>:<lineage>`.
    #[must_use]
    pub fn lineage(scheme: &str, lineage: Uuid) -> Self {
        Self(format!("{scheme}:{lineage}"))
    }

    /// Wraps a dataset id read back from storage or typed by the user.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The dataset's scheme: the part before the first `:`, or the whole id of a global dataset.
    #[must_use]
    pub fn scheme(&self) -> &str {
        self.0.split_once(':').map_or(self.0.as_str(), |(scheme, _)| scheme)
    }

    /// The id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DatasetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How an importer's record ids are scoped (ADR 0037 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatasetScope {
    /// Ids are unique across the whole source (`digitalarkivet`): one dataset, never chosen.
    Global,
    /// Ids are local to one file lineage (`gedcom:<uuid>`): the operator picks or declares one.
    Lineage,
}

/// The dataset an importer writes into: its scheme and how its record ids are scoped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetSpec {
    /// The dataset scheme (`gedcom`, `gramps`, `digitalarkivet`).
    pub scheme: String,
    /// How the importer's record ids are scoped.
    pub scope: DatasetScope,
}

/// A digest of an item's canonical incoming fields, for no-op detection on re-import (ADR 0037 §4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentDigest(String);

impl ContentDigest {
    /// Wraps a digest string computed by an importer.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The digest as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where an imported assertion came from (ADR 0037 §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordOrigin {
    /// The dataset the record belongs to.
    pub dataset: DatasetId,
    /// The record's id within the dataset (a Digitalarkivet `pf…` id, a GEDCOM xref, a Gramps handle).
    pub record: String,
    /// The entity within the record this assertion describes, when a record yields more than one
    /// (`event:BIRT:0`, `citation:1`, …). Chosen by the importer and stable across its runs.
    pub item: Option<String>,
    /// A digest of the item's canonical incoming fields (ADR 0037 §4); `None` until an importer
    /// computes one.
    pub digest: Option<ContentDigest>,
    /// The import run that wrote the assertion (ADR 0037 §5).
    pub run: ImportRunId,
}

#[cfg(test)]
mod tests {
    use super::DatasetId;
    use uuid::Uuid;

    #[test]
    fn a_global_dataset_is_its_scheme() {
        let dataset = DatasetId::global("digitalarkivet");
        assert_eq!(dataset.as_str(), "digitalarkivet");
        assert_eq!(dataset.scheme(), "digitalarkivet");
    }

    #[test]
    fn a_lineage_dataset_is_scheme_colon_uuid() {
        let lineage = Uuid::from_u128(7);
        let dataset = DatasetId::lineage("gedcom", lineage);
        assert_eq!(dataset.as_str(), format!("gedcom:{lineage}"));
        assert_eq!(dataset.scheme(), "gedcom");
    }

    #[test]
    fn a_dataset_serializes_as_a_bare_string() {
        let json = serde_json::to_value(DatasetId::global("digitalarkivet")).unwrap();
        assert_eq!(json, serde_json::json!("digitalarkivet"));
    }
}
