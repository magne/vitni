//! The `ResearchNote` slice's view-models (ADR 0028): a written proof argument about one or more
//! conclusion-bearing records.
//!
//! Two things set it apart from the Note slice it otherwise mirrors. Its subjects are a *forward*
//! collection it owns (a note's References tab is a reverse index it cannot edit), and neither its
//! `human_id` nor its title has an update verb — both are fixed at create time, so the edit draft
//! diffs only the body and its language.

use vitni_app::{ResearchNoteSubjectRef, ResearchNoteSummary};

use super::{
    ActionLabel, DetailTab, HistoryEntryVm, Localizer, RecordDraft, ResearchNoteChangeSetRequest, ResearchNoteEdit,
    RestrictionKind, RowVm, SubjectRequest, TagRef, line_label, non_blank,
};
use crate::navigation::Category;

/// One subject a research note argues about, resolved for display and navigation (ADR 0028 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectVm {
    /// Which entity category the subject belongs to (one of People / Families / Events / Places).
    pub category: Category,
    /// The subject record's user-facing id (e.g. `I0042`) — the link target.
    pub human_id: String,
    /// The subject record's aggregate id (a UUID string) — the join key; never rendered.
    pub id: String,
    /// The localized aggregate-kind label ("Person", "Place", …).
    pub kind_label: String,
}

impl SubjectVm {
    /// Builds a subject view-model from the app DTO, or `None` when the DTO names a kind outside the
    /// four conclusion-bearing aggregates (an unreachable state the UI drops rather than guesses at).
    fn from_ref(subject: &ResearchNoteSubjectRef, loc: &Localizer) -> Option<Self> {
        let category = match subject.kind.as_str() {
            "person" => Category::People,
            "family" => Category::Families,
            "event" => Category::Events,
            "place" => Category::Places,
            _ => return None,
        };
        Some(Self {
            category,
            human_id: subject.human_id.clone(),
            id: subject.id.clone(),
            kind_label: loc.subject_kind_label(category),
        })
    }

    /// The [`SubjectRequest`] a mutation carries for this subject.
    #[must_use]
    pub fn to_request(&self) -> SubjectRequest {
        SubjectRequest {
            category: self.category,
            human_id: self.human_id.clone(),
        }
    }
}

/// A research note's detail view — its title, the written argument, the subjects it argues about,
/// tags, restrictions, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchNoteDetail {
    /// The user-facing id (e.g. `A0001`).
    pub human_id: String,
    /// The stable `ResearchNoteId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the recorded title, else the first line of the argument, else the `human_id`.
    pub title: String,
    /// The written argument, if set.
    pub body: Option<String>,
    /// The argument's language tag, if recorded.
    pub language: Option<String>,
    /// The records this argument is about (Subjects tab); non-empty for any stored note.
    pub subjects: Vec<SubjectVm>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The note's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The note's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl ResearchNoteDetail {
    /// Builds a detail view from a [`ResearchNoteSummary`]. The History tab starts empty and is filled
    /// by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &ResearchNoteSummary, loc: &Localizer) -> Self {
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: research_note_title(summary),
            body: summary.body.clone(),
            language: summary.language.clone(),
            subjects: summary
                .subject_refs
                .iter()
                .filter_map(|subject| SubjectVm::from_ref(subject, loc))
                .collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// A research note's display title: its recorded title, else the first non-empty line of the argument
/// (a leading Markdown heading marker stripped, truncated), else the `human_id`.
fn research_note_title(summary: &ResearchNoteSummary) -> String {
    if let Some(title) = summary.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        return title.to_owned();
    }
    let first_line = summary.body.as_deref().and_then(line_label);
    first_line.unwrap_or_else(|| summary.human_id.clone())
}

/// Builds a generic list row from a [`ResearchNoteSummary`]: the title, a `subject ids · language`
/// subtitle, and a 🧾 avatar.
#[must_use]
pub fn research_note_row(summary: &ResearchNoteSummary, loc: &Localizer) -> RowVm {
    let mut parts: Vec<String> = Vec::new();
    if !summary.subject_refs.is_empty() {
        let ids: Vec<&str> = summary
            .subject_refs
            .iter()
            .map(|subject| subject.human_id.as_str())
            .collect();
        parts.push(loc.research_note_subjects(&ids.join(", ")));
    }
    if let Some(language) = &summary.language {
        parts.push(language.clone());
    }
    RowVm {
        id: summary.human_id.clone(),
        title: research_note_title(summary),
        subtitle: (!parts.is_empty()).then(|| parts.join(" · ")),
        avatar: Some("🧾".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a research note's detail: the argument, then the related-item tabs with counts.
#[must_use]
pub fn research_note_tabs(detail: &ResearchNoteDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>, action: Option<ActionLabel>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
        action,
    };
    vec![
        tab("content", None, None),
        tab("subjects", Some(detail.subjects.len()), Some(ActionLabel::AddSubject)),
        tab("tags", Some(detail.tags.len()), Some(ActionLabel::AddTag)),
        tab("history", None, None),
    ]
}

/// The buffered whole-record draft of a research note (create + edit, one mechanism,
/// `record-editing.html` §2/§6).
///
/// `existing_human_id` is `None` in create mode and `Some` in edit mode. The id, title, and subjects
/// are **create-only**: the aggregate has no rename, no title-set, and no bulk-subject verb, so on an
/// existing note the id and title are read-only and subjects are the per-row collection edited from
/// the Subjects tab. Nothing is written until Save.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResearchNoteDraft {
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The user-facing id; blank ⇒ auto-allocated on save. Create-only.
    pub human_id: String,
    /// The short title. Create-only.
    pub title: String,
    /// The written argument (Markdown).
    pub body: String,
    /// The argument's BCP-47 language.
    pub language: String,
    /// The subjects the operator has named (create-only; must be non-empty to save).
    pub subjects: Vec<SubjectVm>,
}

impl ResearchNoteDraft {
    /// A fresh empty draft for creating a new research note.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A draft pre-populated from an existing research note for editing. Records the current
    /// `human_id` so [`Self::edits_against`] diffs rather than creates; `subjects` stays empty (on an
    /// existing note they are the collection edited per-row, not this scalar draft).
    #[must_use]
    pub fn from_detail(detail: &ResearchNoteDetail) -> Self {
        Self {
            existing_human_id: Some(detail.human_id.clone()),
            human_id: detail.human_id.clone(),
            title: detail.title.clone(),
            body: detail.body.clone().unwrap_or_default(),
            language: detail.language.clone().unwrap_or_default(),
            subjects: Vec::new(),
        }
    }

    /// Names one more subject, ignoring a duplicate (the same `human_id` in the same category).
    pub fn add_subject(&mut self, subject: SubjectVm) {
        if self
            .subjects
            .iter()
            .any(|named| named.category == subject.category && named.human_id == subject.human_id)
        {
            return;
        }
        self.subjects.push(subject);
    }

    /// Removes the subject at `index`, ignoring an out-of-range index.
    pub fn remove_subject(&mut self, index: usize) {
        if index < self.subjects.len() {
            self.subjects.remove(index);
        }
    }

    /// Builds the [`ResearchNoteChangeSetRequest`] the app commits on Save (create mode).
    #[must_use]
    pub fn to_request(&self) -> ResearchNoteChangeSetRequest {
        ResearchNoteChangeSetRequest {
            human_id: non_blank(&self.human_id),
            subjects: self.subjects.iter().map(SubjectVm::to_request).collect(),
            title: non_blank(&self.title),
            body: non_blank(&self.body),
            language: non_blank(&self.language),
        }
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode). The argument and its language commit together as one [`ResearchNoteEdit::SetBody`] (they
    /// share a single `RichText`); nothing else on the scalar record is mutable.
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<ResearchNoteEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        if self.body == seed.body && self.language == seed.language {
            return Vec::new();
        }
        vec![ResearchNoteEdit::SetBody {
            human_id,
            text: self.body.clone(),
            language: non_blank(&self.language),
        }]
    }
}

impl RecordDraft for ResearchNoteDraft {
    type Detail = ResearchNoteDetail;

    fn from_detail(detail: &ResearchNoteDetail) -> Self {
        Self::from_detail(detail)
    }

    /// A create draft needs at least one subject (ADR 0028 §2 — the core rejects an empty set); an
    /// edit draft's subjects live in the Subjects tab, so it is always valid.
    fn is_valid(&self) -> bool {
        self.existing_human_id.is_some() || !self.subjects.is_empty()
    }

    fn display_label(&self) -> Option<String> {
        line_label(&self.title).or_else(|| line_label(&self.body))
    }
}

#[cfg(test)]
mod research_note_tests {
    use crate::navigation::{Category, ResearchNoteEdit};
    use crate::view_model::{
        RecordDraft, ResearchNoteDetail, ResearchNoteDraft, SubjectVm, research_note_row, research_note_tabs,
    };

    fn seed() -> ResearchNoteDraft {
        ResearchNoteDraft {
            existing_human_id: Some("A0001".to_owned()),
            human_id: "A0001".to_owned(),
            title: "Same person as the 1865 census?".to_owned(),
            body: "The parish register agrees on the birth year.".to_owned(),
            language: "en".to_owned(),
            subjects: Vec::new(),
        }
    }

    fn subject(category: Category, human_id: &str) -> SubjectVm {
        SubjectVm {
            category,
            human_id: human_id.to_owned(),
            id: format!("{human_id}-uuid"),
            kind_label: format!("{category:?}"),
        }
    }

    #[test]
    fn a_create_draft_is_only_valid_once_it_names_a_subject() {
        let mut draft = ResearchNoteDraft::new();
        assert!(!draft.is_valid(), "a research note must name at least one subject");
        draft.add_subject(subject(Category::People, "I0042"));
        assert!(draft.is_valid());
    }

    #[test]
    fn adding_the_same_subject_twice_is_ignored() {
        let mut draft = ResearchNoteDraft::new();
        draft.add_subject(subject(Category::People, "I0042"));
        draft.add_subject(subject(Category::People, "I0042"));
        assert_eq!(draft.subjects.len(), 1);
        draft.add_subject(subject(Category::Places, "I0042"));
        assert_eq!(draft.subjects.len(), 2, "the same id in another aggregate is distinct");
    }

    #[test]
    fn removing_a_subject_ignores_an_out_of_range_index() {
        let mut draft = ResearchNoteDraft::new();
        draft.add_subject(subject(Category::People, "I0042"));
        draft.remove_subject(7);
        assert_eq!(draft.subjects.len(), 1);
        draft.remove_subject(0);
        assert!(draft.subjects.is_empty());
    }

    #[test]
    fn to_request_carries_subjects_title_body_and_language() {
        let mut draft = ResearchNoteDraft::new();
        draft.human_id = "  ".to_owned();
        draft.title = "  Same person?  ".to_owned();
        draft.body = " Parish register agrees. ".to_owned();
        draft.language = "en".to_owned();
        draft.add_subject(subject(Category::Events, "E0007"));

        let request = draft.to_request();
        assert_eq!(request.human_id, None, "a blank id auto-allocates");
        assert_eq!(request.title.as_deref(), Some("Same person?"));
        assert_eq!(request.body.as_deref(), Some("Parish register agrees."));
        assert_eq!(request.language.as_deref(), Some("en"));
        assert_eq!(request.subjects.len(), 1);
        assert_eq!(request.subjects[0].category, Category::Events);
        assert_eq!(request.subjects[0].human_id, "E0007");
    }

    #[test]
    fn an_unchanged_draft_yields_no_edits() {
        assert!(seed().edits_against(&seed()).is_empty());
    }

    #[test]
    fn body_and_language_commit_as_one_set_body() {
        let draft = ResearchNoteDraft {
            language: "nb-NO".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(
            &edits[0],
            ResearchNoteEdit::SetBody { human_id, text, language }
                if human_id == "A0001"
                    && text == "The parish register agrees on the birth year."
                    && language.as_deref() == Some("nb-NO")
        ));
    }

    #[test]
    fn a_create_draft_emits_no_edits() {
        let mut draft = ResearchNoteDraft::new();
        draft.body = "Anything".to_owned();
        assert!(
            draft.edits_against(&ResearchNoteDraft::new()).is_empty(),
            "a create draft commits through to_request, never through edits"
        );
    }

    fn detail() -> ResearchNoteDetail {
        ResearchNoteDetail {
            human_id: "A0001".to_owned(),
            id: "a0001-uuid".to_owned(),
            title: "Same person as the 1865 census?".to_owned(),
            body: Some("The parish register agrees.".to_owned()),
            language: Some("en".to_owned()),
            subjects: vec![subject(Category::People, "I0042")],
            tags: Vec::new(),
            restrictions: Vec::new(),
            history: Vec::new(),
        }
    }

    #[test]
    fn tabs_are_content_subjects_tags_history_with_counts() {
        let loc = crate::i18n::Localizer::for_test("en");
        let tabs = research_note_tabs(&detail(), &loc);
        let ids: Vec<&str> = tabs.iter().map(|tab| tab.id).collect();
        assert_eq!(ids, ["content", "subjects", "tags", "history"]);
        assert_eq!(tabs[1].count, Some(1), "the Subjects tab counts the subjects");
        assert_eq!(tabs[0].count, None);
        assert_eq!(tabs[3].count, None);
    }

    #[test]
    fn the_row_shows_the_title_and_never_the_aggregate_id() {
        let loc = crate::i18n::Localizer::for_test("en");
        let summary = vitni_app::ResearchNoteSummary {
            human_id: "A0001".to_owned(),
            id: "a0001-uuid".to_owned(),
            subjects: std::collections::BTreeSet::new(),
            subject_refs: vec![vitni_app::ResearchNoteSubjectRef {
                kind: "person".to_owned(),
                human_id: "I0042".to_owned(),
                id: "i0042-uuid".to_owned(),
            }],
            title: Some("Same person as the 1865 census?".to_owned()),
            body: None,
            media_type: None,
            language: Some("en".to_owned()),
            tags: Vec::new(),
            restrictions: std::collections::BTreeSet::new(),
        };
        let row = research_note_row(&summary, &loc);
        assert_eq!(row.id, "A0001");
        assert_eq!(row.title, "Same person as the 1865 census?");
        assert!(
            row.subtitle
                .as_deref()
                .is_some_and(|subtitle| subtitle.contains("I0042")),
            "the subtitle names the subject: {:?}",
            row.subtitle
        );
        assert!(!row.title.contains("a0001-uuid"));
    }

    #[test]
    fn an_untitled_research_note_falls_back_to_its_first_body_line_then_its_id() {
        let loc = crate::i18n::Localizer::for_test("en");
        let mut summary = vitni_app::ResearchNoteSummary {
            human_id: "A0002".to_owned(),
            id: "a0002-uuid".to_owned(),
            subjects: std::collections::BTreeSet::new(),
            subject_refs: Vec::new(),
            title: None,
            body: Some("# Conflicting age, 1860\n\nThe census disagrees.".to_owned()),
            media_type: None,
            language: None,
            tags: Vec::new(),
            restrictions: std::collections::BTreeSet::new(),
        };
        assert_eq!(
            ResearchNoteDetail::from_summary(&summary, &loc).title,
            "Conflicting age, 1860"
        );
        summary.body = None;
        assert_eq!(ResearchNoteDetail::from_summary(&summary, &loc).title, "A0002");
    }

    #[test]
    fn a_detail_maps_each_subject_kind_onto_its_category() {
        let loc = crate::i18n::Localizer::for_test("en");
        let summary = vitni_app::ResearchNoteSummary {
            human_id: "A0003".to_owned(),
            id: "a0003-uuid".to_owned(),
            subjects: std::collections::BTreeSet::new(),
            subject_refs: ["person", "family", "event", "place", "note"]
                .into_iter()
                .map(|kind| vitni_app::ResearchNoteSubjectRef {
                    kind: kind.to_owned(),
                    human_id: format!("{kind}-id"),
                    id: format!("{kind}-uuid"),
                })
                .collect(),
            title: Some("Four kinds".to_owned()),
            body: None,
            media_type: None,
            language: None,
            tags: Vec::new(),
            restrictions: std::collections::BTreeSet::new(),
        };
        let detail = ResearchNoteDetail::from_summary(&summary, &loc);
        let categories: Vec<Category> = detail.subjects.iter().map(|subject| subject.category).collect();
        assert_eq!(
            categories,
            vec![Category::People, Category::Families, Category::Events, Category::Places],
            "the four conclusion-bearing kinds map onto their categories; anything else is dropped"
        );
        assert!(
            detail.subjects.iter().all(|subject| !subject.kind_label.is_empty()),
            "each subject carries a localized kind label"
        );
    }
}

#[cfg(test)]
mod research_note_display_label_tests {
    use crate::view_model::{RecordDraft, ResearchNoteDraft};

    #[test]
    fn the_title_names_the_draft() {
        let draft = ResearchNoteDraft {
            title: "Same person as the 1865 census?".to_owned(),
            body: "The parish register agrees.".to_owned(),
            ..ResearchNoteDraft::new()
        };
        assert_eq!(
            draft.display_label(),
            Some("Same person as the 1865 census?".to_owned())
        );
    }

    #[test]
    fn the_body_names_a_draft_with_no_title_yet() {
        // The list row already falls back to the argument's first line; a tab that did not would read
        // "New Research notes" beside a form with a paragraph typed into it.
        let draft = ResearchNoteDraft {
            body: "## The parish register agrees\n\nMore.".to_owned(),
            ..ResearchNoteDraft::new()
        };
        assert_eq!(draft.display_label(), Some("The parish register agrees".to_owned()));
    }

    #[test]
    fn a_draft_with_neither_has_no_label() {
        let draft = ResearchNoteDraft {
            language: "en".to_owned(),
            ..ResearchNoteDraft::new()
        };
        assert_eq!(draft.display_label(), None);
    }
}
