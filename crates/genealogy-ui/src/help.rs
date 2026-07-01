//! In-app help content: the framework-neutral article vocabulary and the topic catalogue (ADR 0008).
//!
//! Help articles are authored as Rust data here — exactly like [`shortcuts`](crate::shortcuts) and
//! [`rail_items`](crate::rail::rail_items), not loaded from disk. Every block's prose is a Fluent
//! message id (ADR 0003), resolved by the renderer's chrome catalogue; the renderer owns the
//! block→widget interpreter. Illustrative *specimens* are the one deliberate exception: their
//! interior is diagrammatic sample data hard-coded in the renderer (like an `aria-hidden` icon), so
//! only a specimen's caption is localized.

use serde::{Deserialize, Serialize};

use crate::presentation::ConfidenceLevel;

/// A help article's stable identity — a closed, `Copy` enum (help is first-party content) so it can
/// ride inside the `Copy` [`Destination::Help`](crate::navigation::Destination) navigation key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HelpTopicId {
    /// "Why this app" — the differentiators overview; the default landing topic.
    WhyThisApp,
    /// "Recording your research" — the contents page for the recording guides.
    RecordingOverview,
    /// How to record a person (name, birth, death).
    RecordPerson,
    /// How to record a family (partners, marriage, children).
    RecordFamily,
    /// How to record a census household.
    RecordCensus,
    /// How to record a burial entry from a church book.
    RecordBurial,
    /// How to source a claim with your own personal knowledge.
    SourcePersonalKnowledge,
    /// The domain-vocabulary glossary.
    Glossary,
}

impl HelpTopicId {
    /// Every topic, in index display order.
    #[must_use]
    pub const fn all() -> [Self; 8] {
        [
            Self::WhyThisApp,
            Self::RecordingOverview,
            Self::RecordPerson,
            Self::RecordFamily,
            Self::RecordCensus,
            Self::RecordBurial,
            Self::SourcePersonalKnowledge,
            Self::Glossary,
        ]
    }

    /// The topic shown when none is selected (or an unknown id is requested).
    #[must_use]
    pub const fn default_topic() -> Self {
        Self::WhyThisApp
    }

    /// The stable, section-namespaced id token (the index row key and a deep-link token).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::WhyThisApp => "overview.why-this-app",
            Self::RecordingOverview => "use-case.recording",
            Self::RecordPerson => "use-case.record-person",
            Self::RecordFamily => "use-case.record-family",
            Self::RecordCensus => "use-case.record-census",
            Self::RecordBurial => "use-case.record-burial",
            Self::SourcePersonalKnowledge => "use-case.personal-knowledge",
            Self::Glossary => "reference.glossary",
        }
    }

    /// The topic for a stable id token, or `None` for an unknown token. Callers fall back to
    /// [`Self::default_topic`]; this never panics.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::all().into_iter().find(|topic| topic.id() == id)
    }

    /// The section this topic is filed under.
    #[must_use]
    pub const fn section(self) -> HelpSection {
        match self {
            Self::WhyThisApp => HelpSection::Overview,
            Self::RecordingOverview
            | Self::RecordPerson
            | Self::RecordFamily
            | Self::RecordCensus
            | Self::RecordBurial
            | Self::SourcePersonalKnowledge => HelpSection::UseCase,
            Self::Glossary => HelpSection::Reference,
        }
    }

    /// The Fluent message id for this topic's index/title label.
    #[must_use]
    pub const fn title_id(self) -> &'static str {
        match self {
            Self::WhyThisApp => "help-topic-why-this-app",
            Self::RecordingOverview => "help-topic-recording",
            Self::RecordPerson => "help-topic-record-person",
            Self::RecordFamily => "help-topic-record-family",
            Self::RecordCensus => "help-topic-record-census",
            Self::RecordBurial => "help-topic-record-burial",
            Self::SourcePersonalKnowledge => "help-topic-personal-knowledge",
            Self::Glossary => "help-topic-glossary",
        }
    }
}

/// The section a help topic is filed under — the index taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HelpSection {
    /// Conceptual overviews (the "why" and "what").
    Overview,
    /// Task-oriented guides (the "how to").
    UseCase,
    /// Lookup material (shortcuts, glossaries).
    Reference,
}

impl HelpSection {
    /// The Fluent message id for this section's heading.
    #[must_use]
    pub const fn label_id(self) -> &'static str {
        match self {
            Self::Overview => "help-section-overview",
            Self::UseCase => "help-section-use-case",
            Self::Reference => "help-section-reference",
        }
    }
}

/// One help article: an ordered list of blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpDoc {
    /// The article body, top to bottom.
    pub blocks: Vec<HelpBlock>,
}

/// A block in a help article. Prose payloads are Fluent message ids (ADR 0003).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpBlock {
    /// The opening lead paragraph (`.lede`).
    Lede(Vec<Run>),
    /// A section heading (`<h2>`); the payload is a Fluent id.
    Heading(&'static str),
    /// A body paragraph.
    Paragraph(Vec<Run>),
    /// The "Most tools" vs "This app" two-column comparison (`.contrast`).
    Contrast {
        /// The "Most tools" column — a single Fluent id (no inline emphasis).
        most: &'static str,
        /// The "This app" column — inline runs.
        ours: Vec<Run>,
    },
    /// An illustrative component specimen. The interior is sample data owned by the renderer; only
    /// `caption` (when present) is localized.
    Specimen {
        /// Which specimen to draw.
        kind: SpecimenKind,
        /// The specimen's caption (`.label`), a Fluent id, or `None` for an uncaptioned specimen.
        caption: Option<&'static str>,
    },
    /// A comparison/reference table (`table.tbl`).
    Table {
        /// The column headers, left to right (Fluent ids).
        headers: Vec<&'static str>,
        /// The body rows, each a list of cells aligned to `headers`.
        rows: Vec<Vec<Cell>>,
    },
}

/// An inline text run within a paragraph-like block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    /// Plain text (a Fluent id).
    Text(&'static str),
    /// Bold-emphasised text (a Fluent id).
    Bold(&'static str),
    /// Italic-emphasised text (a Fluent id).
    Italic(&'static str),
    /// A keyboard key glyph (`<kbd>`) — literal text, not localized (e.g. `"⌘K"`).
    Kbd(&'static str),
    /// A monospace code span (`.mono`) — literal text, not localized (e.g. `"nb-NO"`).
    Mono(&'static str),
    /// A link that navigates the help browser to another topic. `label` is a Fluent id.
    TopicLink {
        /// The topic to navigate to.
        topic: HelpTopicId,
        /// The link text (a Fluent id).
        label: &'static str,
    },
}

/// One cell of a help [`HelpBlock::Table`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The cell text (a Fluent id).
    pub text: &'static str,
    /// An optional confidence badge shown before the text (the "This app" column cue).
    pub badge: Option<ConfidenceLevel>,
    /// Whether the cell renders muted (the "Typical tool" column).
    pub muted: bool,
}

/// The closed set of illustrative specimens the renderer can draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecimenKind {
    /// A history timeline straight from the event log.
    Timeline,
    /// Vital facts with surety + source cues.
    FactRows,
    /// The three Evidence Explained analysis axes.
    EvidenceAxes,
    /// A field-by-field non-destructive merge grid.
    MergeGrid,
    /// A per-conclusion provenance ("why we believe") block.
    Provenance,
    /// The plugin capability badges and the localization fallback chain.
    CapabilityBadges,
}

/// One entry in the help index (a left-pane row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpTopicMeta {
    /// The topic's stable identity.
    pub id: HelpTopicId,
    /// The section it is filed under.
    pub section: HelpSection,
    /// The Fluent message id for its index/title label.
    pub title_id: &'static str,
}

/// The full help index, in display order.
#[must_use]
pub fn help_topics() -> Vec<HelpTopicMeta> {
    HelpTopicId::all()
        .into_iter()
        .map(|id| HelpTopicMeta {
            id,
            section: id.section(),
            title_id: id.title_id(),
        })
        .collect()
}

/// The article for `topic`.
#[must_use]
pub fn help_doc(topic: HelpTopicId) -> HelpDoc {
    match topic {
        HelpTopicId::WhyThisApp => why_this_app_doc(),
        HelpTopicId::RecordingOverview => recording_overview_doc(),
        HelpTopicId::RecordPerson => record_person_doc(),
        HelpTopicId::RecordFamily => record_family_doc(),
        HelpTopicId::RecordCensus => record_census_doc(),
        HelpTopicId::RecordBurial => record_burial_doc(),
        HelpTopicId::SourcePersonalKnowledge => personal_knowledge_doc(),
        HelpTopicId::Glossary => glossary_doc(),
    }
}

/// One "At a glance" comparison row: a capability, this app's answer (graded), and a typical tool's.
fn glance_row(capability: &'static str, this_app: &'static str, typical: &'static str) -> Vec<Cell> {
    vec![
        Cell {
            text: capability,
            badge: None,
            muted: false,
        },
        Cell {
            text: this_app,
            badge: Some(ConfidenceLevel::VeryHigh),
            muted: false,
        },
        Cell {
            text: typical,
            badge: None,
            muted: true,
        },
    ]
}

/// The "Why this app" article (mirrors `docs/phase5/strengths.html`).
fn why_this_app_doc() -> HelpDoc {
    HelpDoc {
        blocks: vec![
            HelpBlock::Lede(vec![
                Run::Text("help-why-lede-1"),
                Run::Italic("help-why-lede-conclusions"),
                Run::Text("help-why-lede-2"),
                Run::Bold("help-why-lede-evidence"),
                Run::Text("help-why-lede-3"),
            ]),
            HelpBlock::Heading("help-why-h-audit"),
            HelpBlock::Contrast {
                most: "help-why-audit-most",
                ours: vec![
                    Run::Text("help-why-audit-ours-1"),
                    Run::Bold("help-why-audit-ours-bold"),
                    Run::Text("help-why-audit-ours-2"),
                ],
            },
            HelpBlock::Specimen {
                kind: SpecimenKind::Timeline,
                caption: Some("help-why-spec-timeline"),
            },
            HelpBlock::Heading("help-why-h-evidence"),
            HelpBlock::Contrast {
                most: "help-why-evidence-most",
                ours: vec![
                    Run::Text("help-why-evidence-ours-1"),
                    Run::Bold("help-why-evidence-ours-bold"),
                    Run::Text("help-why-evidence-ours-2"),
                ],
            },
            HelpBlock::Specimen {
                kind: SpecimenKind::FactRows,
                caption: Some("help-why-spec-facts"),
            },
            HelpBlock::Heading("help-why-h-citations"),
            HelpBlock::Contrast {
                most: "help-why-citations-most",
                ours: vec![
                    Run::Text("help-why-citations-ours-1"),
                    Run::Italic("help-why-citations-ours-italic"),
                    Run::Text("help-why-citations-ours-2"),
                ],
            },
            HelpBlock::Specimen {
                kind: SpecimenKind::EvidenceAxes,
                caption: Some("help-why-spec-evidence"),
            },
            HelpBlock::Heading("help-why-h-merge"),
            HelpBlock::Contrast {
                most: "help-why-merge-most",
                ours: vec![Run::Text("help-why-merge-ours")],
            },
            HelpBlock::Specimen {
                kind: SpecimenKind::MergeGrid,
                caption: None,
            },
            HelpBlock::Heading("help-why-h-provenance"),
            HelpBlock::Contrast {
                most: "help-why-provenance-most",
                ours: vec![Run::Text("help-why-provenance-ours")],
            },
            HelpBlock::Specimen {
                kind: SpecimenKind::Provenance,
                caption: None,
            },
            HelpBlock::Heading("help-why-h-plugins"),
            HelpBlock::Contrast {
                most: "help-why-plugins-most",
                ours: vec![
                    Run::Text("help-why-plugins-ours-1"),
                    Run::Bold("help-why-plugins-ours-bold"),
                    Run::Text("help-why-plugins-ours-2"),
                ],
            },
            HelpBlock::Specimen {
                kind: SpecimenKind::CapabilityBadges,
                caption: None,
            },
            HelpBlock::Heading("help-why-h-glance"),
            HelpBlock::Table {
                headers: vec![
                    "help-why-tbl-h-capability",
                    "help-why-tbl-h-this",
                    "help-why-tbl-h-typical",
                ],
                rows: vec![
                    glance_row("help-why-tbl-r1-cap", "help-why-tbl-r1-this", "help-why-tbl-r1-typ"),
                    glance_row("help-why-tbl-r2-cap", "help-why-tbl-r2-this", "help-why-tbl-r2-typ"),
                    glance_row("help-why-tbl-r3-cap", "help-why-tbl-r3-this", "help-why-tbl-r3-typ"),
                    glance_row("help-why-tbl-r4-cap", "help-why-tbl-r4-this", "help-why-tbl-r4-typ"),
                    glance_row("help-why-tbl-r5-cap", "help-why-tbl-r5-this", "help-why-tbl-r5-typ"),
                    glance_row("help-why-tbl-r6-cap", "help-why-tbl-r6-this", "help-why-tbl-r6-typ"),
                    glance_row("help-why-tbl-r7-cap", "help-why-tbl-r7-this", "help-why-tbl-r7-typ"),
                ],
            },
        ],
    }
}

/// The "Recording your research" contents page: links to each recording guide and the glossary.
fn recording_overview_doc() -> HelpDoc {
    HelpDoc {
        blocks: vec![
            HelpBlock::Lede(vec![Run::Text("help-rec-lede")]),
            HelpBlock::Heading("help-rec-h-guides"),
            HelpBlock::Paragraph(vec![
                Run::TopicLink {
                    topic: HelpTopicId::RecordPerson,
                    label: "help-rec-link-person",
                },
                Run::Text("help-rec-desc-person"),
            ]),
            HelpBlock::Paragraph(vec![
                Run::TopicLink {
                    topic: HelpTopicId::RecordFamily,
                    label: "help-rec-link-family",
                },
                Run::Text("help-rec-desc-family"),
            ]),
            HelpBlock::Paragraph(vec![
                Run::TopicLink {
                    topic: HelpTopicId::RecordCensus,
                    label: "help-rec-link-census",
                },
                Run::Text("help-rec-desc-census"),
            ]),
            HelpBlock::Paragraph(vec![
                Run::TopicLink {
                    topic: HelpTopicId::RecordBurial,
                    label: "help-rec-link-burial",
                },
                Run::Text("help-rec-desc-burial"),
            ]),
            HelpBlock::Paragraph(vec![
                Run::TopicLink {
                    topic: HelpTopicId::SourcePersonalKnowledge,
                    label: "help-rec-link-personal",
                },
                Run::Text("help-rec-desc-personal"),
            ]),
            HelpBlock::Heading("help-rec-h-reference"),
            HelpBlock::Paragraph(vec![
                Run::TopicLink {
                    topic: HelpTopicId::Glossary,
                    label: "help-rec-link-glossary",
                },
                Run::Text("help-rec-desc-glossary"),
            ]),
        ],
    }
}

/// "Recording a person" — Person record, names, vital events, and how every claim is sourced.
fn record_person_doc() -> HelpDoc {
    HelpDoc {
        blocks: vec![
            HelpBlock::Lede(vec![Run::Text("help-person-lede")]),
            HelpBlock::Heading("help-person-h-record"),
            HelpBlock::Paragraph(vec![Run::Text("help-person-p-record")]),
            HelpBlock::Heading("help-person-h-vitals"),
            HelpBlock::Paragraph(vec![Run::Text("help-person-p-vitals")]),
            HelpBlock::Heading("help-person-h-source"),
            HelpBlock::Paragraph(vec![Run::Text("help-person-p-source")]),
            HelpBlock::Specimen {
                kind: SpecimenKind::FactRows,
                caption: Some("help-person-spec"),
            },
        ],
    }
}

/// "Recording a family" — Family record, partners, the marriage event, and children.
fn record_family_doc() -> HelpDoc {
    HelpDoc {
        blocks: vec![
            HelpBlock::Lede(vec![Run::Text("help-family-lede")]),
            HelpBlock::Heading("help-family-h-record"),
            HelpBlock::Paragraph(vec![Run::Text("help-family-p-record")]),
            HelpBlock::Heading("help-family-h-marriage"),
            HelpBlock::Paragraph(vec![Run::Text("help-family-p-marriage")]),
            HelpBlock::Heading("help-family-h-children"),
            HelpBlock::Paragraph(vec![Run::Text("help-family-p-children")]),
            HelpBlock::Paragraph(vec![Run::Text("help-family-p-source")]),
        ],
    }
}

/// "Recording a census" — one source with two faces (scan + transcription), the household at a
/// place and date, and the Evidence Explained reading of it.
fn record_census_doc() -> HelpDoc {
    HelpDoc {
        blocks: vec![
            HelpBlock::Lede(vec![Run::Text("help-census-lede")]),
            HelpBlock::Heading("help-census-h-source"),
            HelpBlock::Paragraph(vec![Run::Text("help-census-p-source")]),
            HelpBlock::Heading("help-census-h-place"),
            HelpBlock::Paragraph(vec![Run::Text("help-census-p-place")]),
            HelpBlock::Heading("help-census-h-people"),
            HelpBlock::Paragraph(vec![Run::Text("help-census-p-people")]),
            HelpBlock::Heading("help-census-h-evidence"),
            HelpBlock::Paragraph(vec![Run::Text("help-census-p-evidence")]),
            HelpBlock::Specimen {
                kind: SpecimenKind::EvidenceAxes,
                caption: Some("help-census-spec"),
            },
        ],
    }
}

/// "Recording a burial entry" — a church-book line, the death/burial events, and what the entry
/// is good evidence for.
fn record_burial_doc() -> HelpDoc {
    HelpDoc {
        blocks: vec![
            HelpBlock::Lede(vec![Run::Text("help-burial-lede")]),
            HelpBlock::Heading("help-burial-h-source"),
            HelpBlock::Paragraph(vec![Run::Text("help-burial-p-source")]),
            HelpBlock::Heading("help-burial-h-event"),
            HelpBlock::Paragraph(vec![Run::Text("help-burial-p-event")]),
            HelpBlock::Heading("help-burial-h-evidence"),
            HelpBlock::Paragraph(vec![Run::Text("help-burial-p-evidence")]),
            HelpBlock::Specimen {
                kind: SpecimenKind::Provenance,
                caption: Some("help-burial-spec"),
            },
        ],
    }
}

/// "Sourcing personal knowledge" — you as operator (automatic) vs you as a source, firsthand vs
/// hearsay, and how that differs from a witness role.
fn personal_knowledge_doc() -> HelpDoc {
    HelpDoc {
        blocks: vec![
            HelpBlock::Lede(vec![Run::Text("help-pk-lede")]),
            HelpBlock::Heading("help-pk-h-operator"),
            HelpBlock::Paragraph(vec![Run::Text("help-pk-p-operator")]),
            HelpBlock::Heading("help-pk-h-source"),
            HelpBlock::Paragraph(vec![Run::Text("help-pk-p-source")]),
            HelpBlock::Specimen {
                kind: SpecimenKind::EvidenceAxes,
                caption: Some("help-pk-spec"),
            },
            HelpBlock::Heading("help-pk-h-firsthand"),
            HelpBlock::Paragraph(vec![Run::Text("help-pk-p-firsthand")]),
            HelpBlock::Heading("help-pk-h-witness"),
            HelpBlock::Paragraph(vec![Run::Text("help-pk-p-witness")]),
        ],
    }
}

/// One glossary row: a term and its definition (no badge, no muting).
fn gloss_row(term: &'static str, definition: &'static str) -> Vec<Cell> {
    vec![
        Cell {
            text: term,
            badge: None,
            muted: false,
        },
        Cell {
            text: definition,
            badge: None,
            muted: false,
        },
    ]
}

/// The domain-vocabulary glossary: the evidence/conclusion layering, and two sets of distinctions
/// — assertion/fact/event/citation, and event/association/family.
fn glossary_doc() -> HelpDoc {
    HelpDoc {
        blocks: vec![
            HelpBlock::Lede(vec![Run::Text("help-gloss-lede")]),
            HelpBlock::Heading("help-gloss-h-layers"),
            HelpBlock::Paragraph(vec![Run::Text("help-gloss-p-layers")]),
            HelpBlock::Table {
                headers: vec!["help-gloss-tbl-h-term", "help-gloss-tbl-h-meaning"],
                rows: vec![
                    gloss_row("help-gloss-assertion-term", "help-gloss-assertion-def"),
                    gloss_row("help-gloss-fact-term", "help-gloss-fact-def"),
                    gloss_row("help-gloss-event-term", "help-gloss-event-def"),
                    gloss_row("help-gloss-citation-term", "help-gloss-citation-def"),
                ],
            },
            HelpBlock::Heading("help-gloss-h-relations"),
            HelpBlock::Paragraph(vec![Run::Text("help-gloss-p-relations")]),
            HelpBlock::Table {
                headers: vec!["help-gloss-tbl-h-term", "help-gloss-tbl-h-meaning"],
                rows: vec![
                    gloss_row("help-gloss-rel-event-term", "help-gloss-rel-event-def"),
                    gloss_row("help-gloss-association-term", "help-gloss-association-def"),
                    gloss_row("help-gloss-family-term", "help-gloss-family-def"),
                ],
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{HelpTopicId, help_doc, help_topics};

    #[test]
    fn topic_ids_are_unique_and_round_trip() {
        let ids: Vec<&str> = HelpTopicId::all().iter().map(|topic| topic.id()).collect();
        let unique: BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "topic ids must be unique");
        for topic in HelpTopicId::all() {
            assert_eq!(HelpTopicId::from_id(topic.id()), Some(topic));
        }
    }

    #[test]
    fn unknown_topic_id_resolves_to_none() {
        assert_eq!(HelpTopicId::from_id("overview.does-not-exist"), None);
    }

    #[test]
    fn every_topic_has_a_non_empty_doc() {
        for topic in HelpTopicId::all() {
            assert!(!help_doc(topic).blocks.is_empty(), "empty doc for {topic:?}");
        }
    }

    #[test]
    fn index_lists_every_topic_once() {
        assert_eq!(help_topics().len(), HelpTopicId::all().len());
    }
}
