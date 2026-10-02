//! The similar-record view-models (ADR 0038 §8): the hint a record being created raises, the records
//! *Find similar* lists, and a picker's rows ranked by the engine.

use vitni_app::{DecidableKind, DraftRecord, MatchableKind, SimilarRecord};

use super::dashboard::record_ref;
use super::{HashMap, Localizer, NewRecordDraft, RecordRef};
use crate::list::RowVm;
use crate::navigation::Category;
use crate::picker::PICKER_MAX_ROWS;

/// One stored record the engine judges similar: the record, how similar, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimilarHitVm {
    /// The similar record, navigable.
    pub record: RecordRef,
    /// The engine's score as a whole percentage — a probability, never an asserted confidence.
    pub percent: u8,
    /// The already-localized band the engine put the pair in.
    pub band: String,
    /// The already-localized one-line hint: "Possibly the same as Guldbrand Olsen (I0042), 87%".
    pub line: String,
    /// The already-localized reasons behind the score, the strongest first.
    pub reasons: Vec<String>,
    /// The kind the pair is compared and decided as; `None` for a tag, which is never decided about.
    pub compare: Option<DecidableKind>,
}

impl SimilarHitVm {
    /// Builds the hit for `similar`, a record of `kind`, labelled `label` (its live name) or, without
    /// one, by its id.
    #[must_use]
    pub fn build(kind: MatchableKind, similar: &SimilarRecord, label: Option<String>, loc: &Localizer) -> Self {
        let mut record = record_ref(kind, &similar.record, &HashMap::new());
        if let Some(label) = label {
            record.label = label;
        }
        let evidence = similar.assessment.evidence();
        let named = if record.label == similar.record.human_id || kind == MatchableKind::Tag {
            record.label.clone()
        } else {
            format!("{} ({})", record.label, similar.record.human_id)
        };
        Self {
            line: loc.similar_hint(&named, evidence.percent()),
            percent: evidence.percent(),
            band: loc.match_band(evidence.band),
            reasons: loc.match_reasons(&evidence),
            compare: DecidableKind::from_matchable(kind),
            record,
        }
    }
}

/// The records similar to one record or draft, the most similar first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SimilarVm {
    /// The hits, the most similar first.
    pub hits: Vec<SimilarHitVm>,
}

/// The record a picker's typed `query` would create through its "+ New …" row, as the engine matches
/// it — `None` for a category the engine does not match a typed record of, or a blank query.
#[must_use]
pub fn query_draft(category: Category, query: &str) -> Option<DraftRecord> {
    NewRecordDraft::seed(category, query)?.similar_draft()
}

/// Whether a picker over `category` ranks its records by similarity to the typed query — the kinds a
/// typed query names a record of ([`query_draft`]).
#[must_use]
pub fn ranks_by_similarity(category: Category) -> bool {
    match category {
        Category::People | Category::Places | Category::Sources | Category::Repositories => true,
        Category::Dashboard
        | Category::Families
        | Category::Events
        | Category::Citations
        | Category::Media
        | Category::Notes
        | Category::ResearchNotes
        | Category::Tags
        | Category::DnaTests
        | Category::DnaMatches => false,
    }
}

/// A stored record the engine ranks for a picker's query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerHit {
    /// The record's `human_id`.
    pub human_id: String,
    /// The record's already-localized title.
    pub title: String,
    /// The engine's score as a whole percentage.
    pub percent: u8,
}

/// One picker result row, with the engine's score when the engine ranked it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerRowVm {
    /// The row.
    pub row: RowVm,
    /// The engine's score, when the row is one of its hits.
    pub percent: Option<u8>,
}

/// A picker's rows ranked by similarity: the engine's `hits` first, in its order — each as its row in
/// `matched` when the typed text matched it too, else as a row of its own — then the rest of `matched`,
/// minus any id in `exclude`, capped at [`PICKER_MAX_ROWS`].
#[must_use]
pub fn rank_picker_rows(matched: Vec<RowVm>, hits: &[PickerHit], exclude: &[String]) -> Vec<PickerRowVm> {
    let mut ranked: Vec<PickerRowVm> = Vec::new();
    for hit in hits {
        if exclude.contains(&hit.human_id) {
            continue;
        }
        let row = matched
            .iter()
            .find(|row| row.id == hit.human_id)
            .cloned()
            .unwrap_or_else(|| RowVm {
                id: hit.human_id.clone(),
                title: hit.title.clone(),
                ..RowVm::default()
            });
        ranked.push(PickerRowVm {
            row,
            percent: Some(hit.percent),
        });
    }
    for row in matched {
        if ranked.iter().any(|ranked| ranked.row.id == row.id) {
            continue;
        }
        ranked.push(PickerRowVm { row, percent: None });
    }
    ranked.truncate(PICKER_MAX_ROWS);
    ranked
}
