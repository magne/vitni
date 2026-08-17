use super::{
    ActionLabel, AttachedRefVm, CitationRefVm, ConfidenceLevel, DetailTab, EvidenceAxisVm, HistoryEntryVm, Localizer,
    MediaRefVm, RecordDraft, RestrictionKind, RowVm, SourceChangeSetRequest, SourceEdit, TagRef, citation_ref_from_ref,
    evidence_axes, line_label, non_blank,
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
    /// The raw medium (seeds the per-row edit form's medium select).
    pub media_type: vitni_app::SourceMediaType,
    /// The localized medium label (book, film, electronic, …).
    pub media_type_label: String,
    /// The operator's surety in the link (drives the confidence badge).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the link assertion.
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) that introduced this repository link — the target a per-row
    /// Edit supersedes and an Unlink retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// A record that uses a citation (Source › Citations "Backs record" cell): its kind drives the
/// route, plus the display label and the localized sub-context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitingRecordVm {
    /// The citing aggregate's kind (drives the navigation route and avatar).
    pub kind: vitni_app::CitingKind,
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

/// One source attribute (Source › Attributes tab): key and value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAttributeVm {
    /// The attribute's type / key (verbatim — a free-text key).
    pub attribute_type: String,
    /// The attribute's value.
    pub value: String,
    /// The `AssertionId` (a UUID string) that introduced this attribute — the target a per-row Edit
    /// supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
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
    pub media: Vec<MediaRefVm>,
    /// The attached notes, each with its attach `AssertionId` (the Detach target).
    pub notes: Vec<AttachedRefVm>,
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
    /// Builds a detail view from a [`SourceSummary`](vitni_app::SourceSummary), localizing labels,
    /// media-type labels, and confidence. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &vitni_app::SourceSummary, loc: &Localizer) -> Self {
        let repositories = summary
            .repositories
            .iter()
            .map(|link| {
                let confidence = link.confidence.map(ConfidenceLevel::from);
                RepositoryLinkVm {
                    human_id: link.repository.as_ref().map(|r| r.human_id.clone()),
                    id: link.repository.as_ref().map(|r| r.id.clone()),
                    name: link.name.clone().unwrap_or_else(|| {
                        link.repository
                            .as_ref()
                            .map_or_else(String::new, |r| r.human_id.clone())
                    }),
                    call_number: link.call_number.clone(),
                    media_type: link.media_type.clone(),
                    media_type_label: loc.source_media_type_label(&link.media_type),
                    confidence,
                    confidence_label: loc.confidence_label_opt(confidence),
                    source_count: link.source_count,
                    assertion_id: link.assertion_id.clone(),
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
                assertion_id: a.assertion_id.clone(),
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
            media: summary.media.iter().map(MediaRefVm::from_ref).collect(),
            notes: summary.notes.iter().map(|n| AttachedRefVm::from_ref(n, loc)).collect(),
            tags: summary.tags.clone(),
            reliability: reliability_vm(&summary.reliability, loc),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// Builds a [`CitingRecordVm`] from an app [`CitingRecordRef`](vitni_app::CitingRecordRef),
/// localizing its sub-context and falling back to the `human_id` for the label.
fn citing_record_vm(reference: &vitni_app::CitingRecordRef, loc: &Localizer) -> CitingRecordVm {
    CitingRecordVm {
        kind: reference.kind,
        human_id: reference.human_id.clone(),
        id: reference.id.clone(),
        label: reference.label.clone().unwrap_or_else(|| reference.human_id.clone()),
        context_label: loc.citing_context_label(&reference.context),
    }
}

/// Builds the reliability view-model from the app [`SourceReliability`](vitni_app::SourceReliability).
fn reliability_vm(reliability: &vitni_app::SourceReliability, loc: &Localizer) -> SourceReliabilityVm {
    let confidence = reliability.typical_surety.map(ConfidenceLevel::from);
    SourceReliabilityVm {
        confidence,
        confidence_label: confidence.map(|level| loc.confidence_label(level)),
        evidence_axes: evidence_axes(reliability.evidence.as_ref(), loc),
        citation_count: reliability.citation_count,
        record_count: reliability.record_count,
    }
}

/// Builds a generic list row from a [`SourceSummary`](vitni_app::SourceSummary): the title, an
/// `author · N citations` subtitle, and a 📚 avatar.
#[must_use]
pub fn source_row(summary: &vitni_app::SourceSummary, loc: &Localizer) -> RowVm {
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
    let tab = |id: &'static str, count: Option<usize>, action: Option<ActionLabel>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
        action,
    };
    vec![
        tab("overview", None, None),
        tab(
            "repositories",
            Some(detail.repositories.len()),
            Some(ActionLabel::LinkRepository),
        ),
        // Read-only: the citing records that use this source (a reverse index), not a collection
        // this record's own tab attaches to.
        tab("citations", Some(detail.citations.len()), None),
        tab(
            "attributes",
            Some(detail.attributes.len()),
            Some(ActionLabel::AddAttribute),
        ),
        tab("media", Some(detail.media.len()), Some(ActionLabel::AttachMedia)),
        tab("notes", Some(detail.notes.len()), Some(ActionLabel::AttachNote)),
        tab("tags", Some(detail.tags.len()), Some(ActionLabel::AddTag)),
        tab("history", None, None),
    ]
}

/// The buffered whole-record draft of a source (create + edit, one mechanism, `record-editing.html`
/// §2/§6): the editable user-facing id plus the bibliographic fields (all optional free text).
/// `existing_human_id` is `None` in create mode and `Some` in edit mode. Nothing is written until Save.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceDraft {
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The editable user-facing id; blank ⇒ generated on save (edit) / auto-allocated (create).
    pub human_id: String,
    /// The bibliographic title.
    pub title: String,
    /// The author.
    pub author: String,
    /// The publication info.
    pub publication: String,
    /// The abbreviation.
    pub abbreviation: String,
    /// The record's privacy restrictions (GEDCOM `RESN`); empty is unrestricted. Edit-only — the
    /// change-set request carries none, so a create form does not offer the field.
    pub restrictions: Vec<RestrictionKind>,
}

impl SourceDraft {
    /// A fresh empty draft for creating a new source.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A draft pre-populated from an existing source for editing. Records the current `human_id` so
    /// [`Self::edits_against`] diffs (supersedes) rather than creates.
    #[must_use]
    pub fn from_detail(detail: &SourceDetail) -> Self {
        Self {
            existing_human_id: Some(detail.human_id.clone()),
            human_id: detail.human_id.clone(),
            title: detail.title.clone(),
            author: detail.author.clone().unwrap_or_default(),
            publication: detail.pub_info.clone().unwrap_or_default(),
            abbreviation: detail.abbrev.clone().unwrap_or_default(),
            restrictions: detail.restrictions.clone(),
        }
    }

    /// Builds the [`SourceChangeSetRequest`] the app commits on Save (create mode), mapping each blank
    /// field to `None` ("not reported").
    #[must_use]
    pub fn to_request(&self) -> SourceChangeSetRequest {
        SourceChangeSetRequest {
            human_id: non_blank(&self.human_id),
            title: non_blank(&self.title),
            author: non_blank(&self.author),
            publication: non_blank(&self.publication),
            abbreviation: non_blank(&self.abbreviation),
        }
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode): one `Set*` per changed scalar, with `SetHumanId` emitted last so the record is only
    /// re-keyed after every other field has committed against its current id (a blank id regenerates)
    /// — the restriction set included, so it too commits against the id the record still has.
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<SourceEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        let mut edits = Vec::new();
        if self.title != seed.title {
            edits.push(SourceEdit::SetTitle {
                human_id: human_id.clone(),
                title: self.title.clone(),
            });
        }
        if self.author != seed.author {
            edits.push(SourceEdit::SetAuthor {
                human_id: human_id.clone(),
                author: self.author.clone(),
            });
        }
        if self.publication != seed.publication {
            edits.push(SourceEdit::SetPubInfo {
                human_id: human_id.clone(),
                pub_info: self.publication.clone(),
            });
        }
        if self.abbreviation != seed.abbreviation {
            edits.push(SourceEdit::SetAbbrev {
                human_id: human_id.clone(),
                abbrev: self.abbreviation.clone(),
            });
        }
        if self.restrictions != seed.restrictions {
            edits.push(SourceEdit::SetRestrictions {
                human_id: human_id.clone(),
                restrictions: self.restrictions.clone(),
            });
        }
        if self.human_id.trim() != seed.human_id {
            edits.push(SourceEdit::SetHumanId {
                human_id,
                new_human_id: non_blank(&self.human_id),
            });
        }
        edits
    }
}

impl RecordDraft for SourceDraft {
    type Detail = SourceDetail;

    fn from_detail(detail: &SourceDetail) -> Self {
        Self::from_detail(detail)
    }

    fn is_valid(&self) -> bool {
        true
    }

    fn display_label(&self) -> Option<String> {
        line_label(&self.title)
    }

    fn editable_restrictions(&self) -> Option<&[RestrictionKind]> {
        self.existing_human_id.is_some().then_some(self.restrictions.as_slice())
    }

    fn set_restrictions(&mut self, restrictions: Vec<RestrictionKind>) {
        self.restrictions = restrictions;
    }
}

#[cfg(test)]
mod source_draft_tests {
    use super::{RecordDraft, SourceDetail, SourceDraft, SourceReliabilityVm};
    use crate::navigation::SourceEdit;
    use crate::presentation::RestrictionKind;

    fn seed() -> SourceDraft {
        SourceDraft {
            existing_human_id: Some("S0001".to_owned()),
            human_id: "S0001".to_owned(),
            title: "Trinity Church baptisms".to_owned(),
            author: "Rev. Smith".to_owned(),
            publication: "vol. 3".to_owned(),
            abbreviation: "TCB".to_owned(),
            restrictions: vec![RestrictionKind::Confidential],
        }
    }

    fn detail() -> SourceDetail {
        SourceDetail {
            human_id: "S0009".to_owned(),
            id: "source-uuid".to_owned(),
            title: "Trinity Church baptisms".to_owned(),
            author: None,
            pub_info: None,
            abbrev: None,
            repositories: Vec::new(),
            citations: Vec::new(),
            attributes: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            reliability: SourceReliabilityVm {
                confidence: None,
                confidence_label: None,
                evidence_axes: Vec::new(),
                citation_count: 0,
                record_count: 0,
            },
            restrictions: vec![RestrictionKind::Locked],
            history: Vec::new(),
        }
    }

    #[test]
    fn a_changed_restriction_set_yields_one_restriction_edit() {
        let draft = SourceDraft {
            restrictions: vec![RestrictionKind::Confidential, RestrictionKind::Privacy],
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(
            &edits[0],
            SourceEdit::SetRestrictions { restrictions, .. }
                if restrictions == &[RestrictionKind::Confidential, RestrictionKind::Privacy]
        ));
    }

    #[test]
    fn an_unchanged_restriction_set_yields_no_restriction_edit() {
        let draft = SourceDraft {
            author: "Rev. Jones".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert!(
            !edits
                .iter()
                .any(|edit| matches!(edit, SourceEdit::SetRestrictions { .. }))
        );
    }

    #[test]
    fn a_draft_differing_only_in_restrictions_is_dirty() {
        let draft = SourceDraft {
            restrictions: Vec::new(),
            ..seed()
        };
        assert!(
            draft.is_dirty_against(&seed()),
            "a restriction change alone makes Save available"
        );
    }

    #[test]
    fn from_detail_seeds_the_restrictions_and_offers_the_field() {
        let draft = SourceDraft::from_detail(&detail());
        assert_eq!(draft.restrictions, vec![RestrictionKind::Locked]);
        assert_eq!(
            draft.editable_restrictions(),
            Some([RestrictionKind::Locked].as_slice()),
            "a stored record offers the restriction field"
        );
    }

    #[test]
    fn a_create_draft_offers_no_restriction_field() {
        assert_eq!(SourceDraft::new().editable_restrictions(), None);
    }

    #[test]
    fn to_request_trims_fields_and_maps_blanks_to_none() {
        let draft = SourceDraft {
            title: "  Trinity Church baptisms  ".to_owned(),
            author: String::new(),
            publication: "vol. 3".to_owned(),
            abbreviation: "   ".to_owned(),
            ..SourceDraft::new()
        };
        let request = draft.to_request();
        assert_eq!(request.title.as_deref(), Some("Trinity Church baptisms"));
        assert_eq!(request.author, None);
        assert_eq!(request.publication.as_deref(), Some("vol. 3"));
        assert_eq!(request.abbreviation, None);
    }

    #[test]
    fn an_unchanged_draft_yields_no_edits() {
        assert!(seed().edits_against(&seed()).is_empty());
    }

    #[test]
    fn each_changed_field_yields_exactly_one_edit() {
        let draft = SourceDraft {
            author: "Rev. Jones".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], SourceEdit::SetAuthor { author, .. } if author == "Rev. Jones"));
    }

    #[test]
    fn a_blank_human_id_regenerates() {
        let draft = SourceDraft {
            human_id: String::new(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], SourceEdit::SetHumanId { new_human_id, .. } if new_human_id.is_none()));
    }
}

#[cfg(test)]
mod source_display_label_tests {
    use super::{RecordDraft, SourceDraft};

    #[test]
    fn the_label_is_the_title() {
        let draft = SourceDraft {
            title: "Trinity Church baptisms".to_owned(),
            ..SourceDraft::new()
        };
        assert_eq!(draft.display_label(), Some("Trinity Church baptisms".to_owned()));
    }

    #[test]
    fn a_draft_with_no_title_has_no_label() {
        let draft = SourceDraft {
            author: "Rev. Smith".to_owned(),
            ..SourceDraft::new()
        };
        assert_eq!(draft.display_label(), None);
    }
}
