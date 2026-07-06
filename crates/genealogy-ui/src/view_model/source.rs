use super::{
    CitationRefVm, ConfidenceLevel, DetailTab, EvidenceAxisVm, FamilyMediaVm, HistoryEntryVm, Localizer,
    RestrictionKind, RowVm, TagRef, citation_ref_from_ref, evidence_axes,
};

/// One repository a source is held in (Source › Repositories tab): the repo, call number, medium,
/// and the link's surety + source count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryLinkVm {
    /// The repository's user-facing id (e.g. `R0001`), if still projected.
    pub human_id: Option<String>,
    /// The repository's stable id (a UUID string) — the navigation key, if still projected.
    pub id: Option<String>,
    /// The repository's display name (falls back to the `human_id`).
    pub name: String,
    /// The source's call number / shelf mark in this repository, if recorded.
    pub call_number: Option<String>,
    /// The localized medium label (book, film, electronic, …).
    pub media_type_label: String,
    /// The operator's surety in the link (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the link assertion.
    pub source_count: usize,
}

/// A record that uses a citation (Source › Citations "Backs record" cell): its kind drives the
/// route, plus the display label and the localized sub-context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitingRecordVm {
    /// The citing aggregate's kind (drives the navigation route and avatar).
    pub kind: genealogy_app::CitingKind,
    /// The citing record's user-facing id.
    pub human_id: String,
    /// The citing record's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The citing record's display label (a name/title, or the `human_id` fallback).
    pub label: String,
    /// The localized sub-context (e.g. "Birth", a participant role), empty for a row-level cite.
    pub context_label: String,
}

/// One citation that uses a source (Source › Citations tab): the citation row + the records it backs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCitationVm {
    /// The citation (source · page · surety · evidence axes).
    pub citation: CitationRefVm,
    /// The records that use this citation.
    pub backers: Vec<CitingRecordVm>,
}

/// One source attribute (Source › Attributes tab): key, value, and how many citations back it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAttributeVm {
    /// The attribute's type / key (verbatim — a free-text key).
    pub attribute_type: String,
    /// The attribute's value.
    pub value: String,
    /// How many citations back the attribute.
    pub source_count: usize,
}

impl SourceAttributeVm {
    /// Whether a source backs this attribute (drives the no-source flag — colour-not-alone).
    #[must_use]
    pub fn has_source(&self) -> bool {
        self.source_count > 0
    }
}

/// The reliability synthesis for a source (Source › Overview "Reliability" card).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceReliabilityVm {
    /// The modal surety across the source's citations (drives the badge), if any.
    pub confidence: Option<ConfidenceLevel>,
    /// The localized modal-surety label, if any.
    pub confidence_label: Option<String>,
    /// The modal Evidence Explained axis chips (empty when no citation is analysed).
    pub evidence_axes: Vec<EvidenceAxisVm>,
    /// How many citations cite this source.
    pub citation_count: usize,
    /// How many distinct records use those citations.
    pub record_count: usize,
}

/// A source's detail view — bibliographic facts, repository links, the citations that use it,
/// attributes, attachments, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDetail {
    /// The user-facing id (e.g. `S0001`).
    pub human_id: String,
    /// The stable `SourceId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the source's title (falls back to the `human_id`).
    pub title: String,
    /// The bibliographic author, if set.
    pub author: Option<String>,
    /// The publication info, if set.
    pub pub_info: Option<String>,
    /// The abbreviation, if set.
    pub abbrev: Option<String>,
    /// The repositories that hold this source.
    pub repositories: Vec<RepositoryLinkVm>,
    /// The citations that use this source, with their backing records.
    pub citations: Vec<SourceCitationVm>,
    /// The source's attributes.
    pub attributes: Vec<SourceAttributeVm>,
    /// The attached media objects.
    pub media: Vec<FamilyMediaVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The reliability synthesis derived from the source's citation set.
    pub reliability: SourceReliabilityVm,
    /// The source's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The source's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl SourceDetail {
    /// Builds a detail view from a [`SourceSummary`](genealogy_app::SourceSummary), localizing labels,
    /// media-type labels, and confidence. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::SourceSummary, loc: &Localizer) -> Self {
        let repositories = summary
            .repositories
            .iter()
            .map(|link| {
                let confidence = ConfidenceLevel::from(link.confidence);
                RepositoryLinkVm {
                    human_id: link.repository.as_ref().map(|r| r.human_id.clone()),
                    id: link.repository.as_ref().map(|r| r.id.clone()),
                    name: link.name.clone().unwrap_or_else(|| {
                        link.repository
                            .as_ref()
                            .map_or_else(String::new, |r| r.human_id.clone())
                    }),
                    call_number: link.call_number.clone(),
                    media_type_label: loc.source_media_type_label(&link.media_type),
                    confidence,
                    confidence_label: loc.confidence_label(confidence),
                    source_count: link.source_count,
                }
            })
            .collect();
        let citations = summary
            .citations
            .iter()
            .map(|row| SourceCitationVm {
                citation: citation_ref_from_ref(&row.citation, loc),
                backers: row.backers.iter().map(|b| citing_record_vm(b, loc)).collect(),
            })
            .collect();
        let attributes = summary
            .attributes
            .iter()
            .map(|a| SourceAttributeVm {
                attribute_type: a.attribute_type.clone(),
                value: a.value.clone(),
                source_count: a.source_count,
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: summary.title.clone().unwrap_or_else(|| summary.human_id.clone()),
            author: summary.author.clone(),
            pub_info: summary.pub_info.clone(),
            abbrev: summary.abbrev.clone(),
            repositories,
            citations,
            attributes,
            media: summary
                .media
                .iter()
                .map(|media| FamilyMediaVm {
                    human_id: media.human_id.clone(),
                    caption: media.caption.clone(),
                })
                .collect(),
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            reliability: reliability_vm(&summary.reliability, loc),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// Builds a [`CitingRecordVm`] from an app [`CitingRecordRef`](genealogy_app::CitingRecordRef),
/// localizing its sub-context and falling back to the `human_id` for the label.
fn citing_record_vm(reference: &genealogy_app::CitingRecordRef, loc: &Localizer) -> CitingRecordVm {
    CitingRecordVm {
        kind: reference.kind,
        human_id: reference.human_id.clone(),
        id: reference.id.clone(),
        label: reference.label.clone().unwrap_or_else(|| reference.human_id.clone()),
        context_label: loc.citing_context_label(&reference.context),
    }
}

/// Builds the reliability view-model from the app [`SourceReliability`](genealogy_app::SourceReliability).
fn reliability_vm(reliability: &genealogy_app::SourceReliability, loc: &Localizer) -> SourceReliabilityVm {
    let confidence = reliability.typical_surety.map(ConfidenceLevel::from);
    SourceReliabilityVm {
        confidence,
        confidence_label: confidence.map(|level| loc.confidence_label(level)),
        evidence_axes: evidence_axes(reliability.evidence.as_ref(), loc),
        citation_count: reliability.citation_count,
        record_count: reliability.record_count,
    }
}

/// Builds a generic list row from a [`SourceSummary`](genealogy_app::SourceSummary): the title, an
/// `author · N citations` subtitle, and a 📚 avatar.
#[must_use]
pub fn source_row(summary: &genealogy_app::SourceSummary, loc: &Localizer) -> RowVm {
    let title = summary.title.clone().unwrap_or_else(|| summary.human_id.clone());
    let citations = loc.source_count(summary.reliability.citation_count);
    let subtitle = match &summary.author {
        Some(author) => Some(format!("{author} · {citations}")),
        None => Some(citations),
    };
    RowVm {
        id: summary.human_id.clone(),
        title,
        subtitle,
        avatar: Some("📚".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a source's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn source_tabs(detail: &SourceDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("repositories", Some(detail.repositories.len())),
        tab("citations", Some(detail.citations.len())),
        tab("attributes", Some(detail.attributes.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}
