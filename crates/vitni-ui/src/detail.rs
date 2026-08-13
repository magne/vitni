//! Detail-tab vocabulary: the framework-neutral description of one tab in a record's related-item
//! tab strip (ADR 0008). A renderer turns these into its own tab widget; the `id` drives the ARIA
//! wiring and the panel-content switch, the `label` is already localized, and `count` is the
//! optional related-item badge.

/// One tab in a record's detail tab strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailTab {
    /// The stable tab id (e.g. `"overview"`, `"citations"`) — ARIA wiring and the content switch.
    pub id: &'static str,
    /// The visible, already-localized label.
    pub label: String,
    /// An optional count badge (the number of related items the tab holds).
    pub count: Option<usize>,
}
