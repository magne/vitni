use super::{
    AttachedRefVm, CitationChangeSetRequest, CitationEdit, CitationSourceRequest, CitationSummary, ConfidenceLevel,
    DateDraft, DetailTab, EvidenceAnalysis, EvidenceAxisVm, EvidenceKind, HistoryEntryVm, InformationKind, Localizer,
    NewSourceFields, RecordDraft, RecordLink, RestrictionKind, RowVm, SourceQuality, TagRef, evidence_axes, non_blank,
};
use crate::picker::PickerSelection;

/// Builds a generic list row from a [`CitationSummary`]: the cited source (or the citation id) as the
/// title, the page as the subtitle, and the quote glyph as the avatar.
#[must_use]
pub fn citation_row(summary: &CitationSummary, _loc: &Localizer) -> RowVm {
    RowVm {
        id: summary.human_id.clone(),
        title: summary
            .source
            .as_ref()
            .map_or_else(|| summary.human_id.clone(), |s| s.human_id.clone()),
        subtitle: summary.page.clone(),
        avatar: Some("❝".to_owned()),
        ..RowVm::default()
    }
}

/// One citation attribute (Citation › Attributes tab): a typed `(type, value)` pair plus the
/// `AssertionId` that introduced it — the target a per-row Edit supersedes and a Retract retracts
/// (ADR 0004 §2). The assertion id is never rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationAttributeVm {
    /// The attribute's type / key (verbatim — a free-text key).
    pub attribute_type: String,
    /// The attribute's value.
    pub value: String,
    /// The `AssertionId` (a UUID string) that introduced this attribute. Never rendered.
    pub assertion_id: String,
}

/// A citation's detail view — its evidence axes, confidence, source, page, date, attributes, and
/// attachments. The research-grade-citation differentiator (Evidence Explained axes) lives here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationDetail {
    /// The user-facing id (e.g. `C0001`).
    pub human_id: String,
    /// The cited source's `human_id`, if resolved.
    pub source: Option<String>,
    /// The page / locator within the source.
    pub page: Option<String>,
    /// The localized date of the cited record.
    pub date: Option<String>,
    /// The structured date of the cited record, if asserted (seeds the whole-record editor).
    pub date_value: Option<genealogy_app::GenealogicalDate>,
    /// The citation's confidence as a presentation level, if set (drives the badge).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: Option<String>,
    /// The raw source-quality evidence axis, if analysed (seeds the whole-record editor).
    pub source_quality: Option<SourceQuality>,
    /// The raw information-kind evidence axis, if analysed.
    pub information: Option<InformationKind>,
    /// The raw evidence-kind axis, if analysed.
    pub evidence_kind: Option<EvidenceKind>,
    /// The Evidence Explained axis chips (empty when no analysis is recorded).
    pub evidence_axes: Vec<EvidenceAxisVm>,
    /// The citation's privacy restrictions (GEDCOM `RESN`), as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The recorded attributes, each with the `AssertionId` that introduced it.
    pub attributes: Vec<CitationAttributeVm>,
    /// The media objects attached to this citation, each with its attach `AssertionId`.
    pub media: Vec<AttachedRefVm>,
    /// The notes attached to this citation, each with its attach `AssertionId`.
    pub notes: Vec<AttachedRefVm>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The citation's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl CitationDetail {
    /// Builds a detail view from a [`CitationSummary`], localizing labels and the date via `loc`.
    ///
    /// The History tab starts empty and is filled by the dispatcher
    /// ([`dispatch`](crate::intent::dispatch)), which has the change-log data.
    #[must_use]
    pub fn from_summary(summary: &CitationSummary, loc: &Localizer) -> Self {
        let confidence = summary.confidence.map(ConfidenceLevel::from);
        Self {
            human_id: summary.human_id.clone(),
            source: summary.source.as_ref().map(|s| s.human_id.clone()),
            page: summary.page.clone(),
            date: summary.date.as_ref().map(|date| loc.date(date)),
            date_value: summary.date.clone(),
            confidence,
            confidence_label: confidence.map(|level| loc.confidence_label(level)),
            source_quality: summary.evidence_analysis.as_ref().map(|analysis| analysis.source),
            information: summary.evidence_analysis.as_ref().map(|analysis| analysis.information),
            evidence_kind: summary.evidence_analysis.as_ref().map(|analysis| analysis.evidence),
            evidence_axes: evidence_axes(summary.evidence_analysis.as_ref(), loc),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            attributes: summary
                .attributes
                .iter()
                .map(|a| CitationAttributeVm {
                    attribute_type: a.attribute_type.clone(),
                    value: a.value.clone(),
                    assertion_id: a.assertion_id.clone(),
                })
                .collect(),
            media: summary
                .media
                .iter()
                .map(|m| AttachedRefVm {
                    human_id: m.human_id.clone(),
                    assertion_id: m.assertion_id.clone(),
                })
                .collect(),
            notes: summary.notes.iter().map(AttachedRefVm::from_ref).collect(),
            tags: summary.tags.clone(),
            history: Vec::new(),
        }
    }
}

/// The tab strip for a citation's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn citation_tabs(detail: &CitationDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("attributes", Some(detail.attributes.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// The in-memory draft for a citation (`record-editing.html` §6): a required source (existing or
/// created inline — §6b), a page, the cited-record's structured date, and the citation's own
/// confidence + Evidence Explained analysis (distinct from the provenance block).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CitationDraft {
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The editable user-facing id; blank ⇒ generated on save (edit mode only — create auto-allocates).
    pub human_id: String,
    /// The source this citation cites (existing or a new source created inline — §6b, §7). Locked on
    /// edit (the source pointer is set at creation); seeded from the record for display only.
    pub source: RecordLink<NewSourceFields>,
    /// The structured date of the cited record (`event.html` control cluster). On create it is carried
    /// in the change-set request and asserted after the commit; on edit a change emits a `SetDate` on
    /// Save. A blank draft emits nothing.
    pub date: DateDraft,
    /// The page / locator.
    pub page: String,
    /// The citation's own confidence, if chosen.
    pub confidence: Option<ConfidenceLevel>,
    /// The source-quality evidence axis, if chosen.
    pub source_quality: Option<SourceQuality>,
    /// The information-kind evidence axis, if chosen.
    pub information: Option<InformationKind>,
    /// The evidence-kind axis, if chosen.
    pub evidence_kind: Option<EvidenceKind>,
}

impl CitationDraft {
    /// A fresh draft for creating a new citation.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A draft pre-populated from an existing citation for editing. Records the current `human_id` so
    /// [`Self::edits_against`] diffs (supersedes) rather than creates; seeds the page, confidence, and
    /// the three evidence axes (the source pointer is locked on edit, seeded for display only).
    #[must_use]
    pub fn from_detail(detail: &CitationDetail) -> Self {
        let source = detail.source.clone().map_or(RecordLink::Empty, |human_id| {
            RecordLink::Existing(PickerSelection {
                title: human_id.clone(),
                human_id,
            })
        });
        Self {
            existing_human_id: Some(detail.human_id.clone()),
            human_id: detail.human_id.clone(),
            source,
            date: detail.date_value.as_ref().map_or_else(DateDraft::default, |value| {
                DateDraft::from_value(value, detail.date.clone().unwrap_or_default())
            }),
            page: detail.page.clone().unwrap_or_default(),
            confidence: detail.confidence,
            source_quality: detail.source_quality,
            information: detail.information,
            evidence_kind: detail.evidence_kind,
        }
    }

    /// Whether the draft is valid: the date (if entered) must parse, and on create the required source
    /// resolves (an existing source is picked, or a new one created). On edit the source pointer is
    /// locked, so only the date can block validity.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.date.is_invalid() && (self.existing_human_id.is_some() || self.source.is_set())
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode): a `SetPage`/`SetConfidence` per changed scalar, one `SetEvidenceAnalysis` only when all
    /// three axes are set and the triple changed (mirroring the create rule), and `SetHumanId` last so
    /// the record is only re-keyed after every other field has committed (a blank id regenerates).
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<CitationEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        let mut edits = Vec::new();
        if self.date != seed.date
            && let Ok(Some(date)) = self.date.to_input()
        {
            edits.push(CitationEdit::SetDate {
                human_id: human_id.clone(),
                date,
            });
        }
        if self.page != seed.page {
            edits.push(CitationEdit::SetPage {
                human_id: human_id.clone(),
                page: self.page.clone(),
            });
        }
        if self.confidence != seed.confidence
            && let Some(confidence) = self.confidence
        {
            edits.push(CitationEdit::SetConfidence {
                human_id: human_id.clone(),
                confidence,
            });
        }
        let triple_changed = (self.source_quality, self.information, self.evidence_kind)
            != (seed.source_quality, seed.information, seed.evidence_kind);
        if triple_changed
            && let (Some(source), Some(information), Some(evidence)) =
                (self.source_quality, self.information, self.evidence_kind)
        {
            edits.push(CitationEdit::SetEvidenceAnalysis {
                human_id: human_id.clone(),
                analysis: EvidenceAnalysis {
                    source,
                    information,
                    evidence,
                },
            });
        }
        if self.human_id.trim() != seed.human_id {
            edits.push(CitationEdit::SetHumanId {
                human_id,
                new_human_id: non_blank(&self.human_id),
            });
        }
        edits
    }

    /// Builds the [`CitationChangeSetRequest`] the app commits on Save, or `None` when the required
    /// source is missing.
    #[must_use]
    pub fn to_request(&self) -> Option<CitationChangeSetRequest> {
        let source = match &self.source {
            RecordLink::Existing(selection) => CitationSourceRequest::Existing(selection.human_id.clone()),
            RecordLink::New(fields) => CitationSourceRequest::New {
                title: non_blank(&fields.title),
            },
            RecordLink::Empty => return None,
        };
        let evidence = match (self.source_quality, self.information, self.evidence_kind) {
            (Some(source), Some(information), Some(evidence)) => Some(EvidenceAnalysis {
                source,
                information,
                evidence,
            }),
            _ => None,
        };
        Some(CitationChangeSetRequest {
            source,
            page: non_blank(&self.page),
            confidence: self.confidence,
            evidence,
            date: self.date.to_input().ok().flatten(),
        })
    }
}

impl RecordDraft for CitationDraft {
    type Detail = CitationDetail;

    fn from_detail(detail: &CitationDetail) -> Self {
        Self::from_detail(detail)
    }

    fn is_valid(&self) -> bool {
        Self::is_valid(self)
    }
}

#[cfg(test)]
mod citation_draft_tests {
    use super::{CitationDraft, DateDraft, NewSourceFields, RecordDraft, RecordLink};
    use crate::navigation::{CitationEdit, CitationSourceRequest};
    use crate::picker::PickerSelection;
    use crate::view_model::{ConfidenceLevel, EvidenceKind, InformationKind, SourceQuality};

    #[test]
    fn a_fresh_draft_is_invalid_and_not_dirty() {
        let draft = CitationDraft::new();
        assert!(!draft.is_valid(), "a fresh draft has no source picked");
        assert!(!draft.is_dirty_against(&CitationDraft::new()));
        assert!(draft.to_request().is_none());
    }

    fn edit_seed() -> CitationDraft {
        CitationDraft {
            existing_human_id: Some("C0001".to_owned()),
            human_id: "C0001".to_owned(),
            source: RecordLink::Existing(PickerSelection {
                human_id: "S0001".to_owned(),
                title: "S0001".to_owned(),
            }),
            page: "p. 42".to_owned(),
            confidence: Some(ConfidenceLevel::High),
            ..CitationDraft::new()
        }
    }

    #[test]
    fn an_edit_draft_is_valid_and_unchanged_yields_no_edits() {
        assert!(edit_seed().is_valid());
        assert!(edit_seed().edits_against(&edit_seed()).is_empty());
    }

    #[test]
    fn a_changed_page_yields_one_set_page() {
        let draft = CitationDraft {
            page: "p. 7".to_owned(),
            ..edit_seed()
        };
        let edits = draft.edits_against(&edit_seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], CitationEdit::SetPage { page, .. } if page == "p. 7"));
    }

    #[test]
    fn the_axes_triple_commits_as_one_analysis_only_when_all_three_are_set() {
        let partial = CitationDraft {
            source_quality: Some(SourceQuality::Original),
            ..edit_seed()
        };
        assert!(
            partial.edits_against(&edit_seed()).is_empty(),
            "a partial triple emits nothing"
        );
        let full = CitationDraft {
            source_quality: Some(SourceQuality::Original),
            information: Some(InformationKind::Primary),
            evidence_kind: Some(EvidenceKind::Direct),
            ..edit_seed()
        };
        let edits = full.edits_against(&edit_seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], CitationEdit::SetEvidenceAnalysis { .. }));
    }

    #[test]
    fn a_blank_id_regenerates_and_is_emitted_last() {
        let draft = CitationDraft {
            page: "p. 7".to_owned(),
            human_id: String::new(),
            ..edit_seed()
        };
        let edits = draft.edits_against(&edit_seed());
        assert_eq!(edits.len(), 2);
        assert!(matches!(&edits[0], CitationEdit::SetPage { .. }));
        assert!(matches!(&edits[1], CitationEdit::SetHumanId { new_human_id, .. } if new_human_id.is_none()));
    }

    #[test]
    fn a_picked_existing_source_maps_through() {
        let draft = CitationDraft {
            source: RecordLink::Existing(PickerSelection {
                human_id: "S0001".to_owned(),
                title: "Baptism register".to_owned(),
            }),
            ..CitationDraft::new()
        };
        assert!(draft.is_valid());
        assert_eq!(
            draft.to_request().expect("valid").source,
            CitationSourceRequest::Existing("S0001".to_owned())
        );
    }

    #[test]
    fn a_new_source_is_valid_even_without_a_title() {
        let draft = CitationDraft {
            source: RecordLink::New(NewSourceFields::default()),
            ..CitationDraft::new()
        };
        assert!(draft.is_valid());
        match draft.to_request().expect("valid").source {
            CitationSourceRequest::New { title } => assert_eq!(title, None),
            CitationSourceRequest::Existing(_) => panic!("expected a New source"),
        }
    }

    fn typed_date(text: &str) -> DateDraft {
        DateDraft {
            start: text.to_owned(),
            ..DateDraft::default()
        }
    }

    #[test]
    fn a_changed_date_makes_it_dirty_and_emits_set_date() {
        let draft = CitationDraft {
            date: typed_date("14 Jun 1876"),
            ..edit_seed()
        };
        assert!(draft.is_dirty_against(&edit_seed()));
        let edits = draft.edits_against(&edit_seed());
        assert_eq!(edits.len(), 1);
        let CitationEdit::SetDate { date, .. } = &edits[0] else {
            panic!("expected a SetDate, got {:?}", edits[0]);
        };
        assert_eq!(*date, typed_date("14 Jun 1876").to_input().unwrap().unwrap());
    }

    #[test]
    fn an_untouched_date_emits_no_set_date() {
        assert!(edit_seed().edits_against(&edit_seed()).is_empty());
    }

    #[test]
    fn an_invalid_date_blocks_validity() {
        let draft = CitationDraft {
            date: typed_date("gibberish"),
            ..edit_seed()
        };
        assert!(!draft.is_valid());
    }

    #[test]
    fn a_create_request_carries_a_parsed_date() {
        let draft = CitationDraft {
            source: RecordLink::New(NewSourceFields::default()),
            date: typed_date("14 Jun 1876"),
            ..CitationDraft::new()
        };
        assert_eq!(
            draft.to_request().expect("valid").date,
            typed_date("14 Jun 1876").to_input().unwrap()
        );
    }

    #[test]
    fn a_blank_create_date_maps_to_none() {
        let draft = CitationDraft {
            source: RecordLink::New(NewSourceFields::default()),
            ..CitationDraft::new()
        };
        assert!(draft.to_request().expect("valid").date.is_none());
    }
}
