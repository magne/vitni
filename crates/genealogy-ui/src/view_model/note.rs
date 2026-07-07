use super::{
    DetailTab, HistoryEntryVm, Localizer, NoteChangeSetRequest, NoteEdit, RecordDraft, RestrictionKind, RowVm, TagRef,
    UsingRecordVm, non_blank, using_record_vm,
};

/// One translation of a note's content (Note Language tab): language, text, and translator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationVm {
    /// The translation's language tag (e.g. `nb-NO`), if recorded.
    pub language: Option<String>,
    /// The translated text.
    pub text: String,
    /// Who produced the translation, if recorded.
    pub translator: Option<String>,
}

/// A note's detail view — its type, rich-text content, language + translations, the records that
/// reference it, tags, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteDetail {
    /// The user-facing id (e.g. `N0001`).
    pub human_id: String,
    /// The stable `NoteId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the first line of the note's text (falls back to the `human_id`).
    pub title: String,
    /// The note's type, if set (carried for the edit form).
    pub note_type: Option<genealogy_app::NoteType>,
    /// The localized note-type label, if set.
    pub note_type_label: Option<String>,
    /// The note's primary text content, if set.
    pub text: Option<String>,
    /// The primary content's language tag, if recorded.
    pub language: Option<String>,
    /// Translations of the primary content into other languages.
    pub translations: Vec<TranslationVm>,
    /// The records that reference this note.
    pub references: Vec<UsingRecordVm>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The note's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The note's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl NoteDetail {
    /// Builds a detail view from a [`NoteSummary`](genealogy_app::NoteSummary), localizing the type
    /// label. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::NoteSummary, loc: &Localizer) -> Self {
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: note_title(summary.text.as_deref()).unwrap_or_else(|| summary.human_id.clone()),
            note_type: summary.note_type.clone(),
            note_type_label: summary.note_type.as_ref().map(|t| loc.note_type_label(t)),
            text: summary.text.clone(),
            language: summary.language.clone(),
            translations: summary
                .translations
                .iter()
                .map(|t| TranslationVm {
                    language: t.language.clone(),
                    text: t.text.clone(),
                    translator: t.translator.clone(),
                })
                .collect(),
            references: summary.references.iter().map(|u| using_record_vm(u, loc)).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// A note's title: the first non-empty line of its text, with a leading Markdown heading marker
/// stripped, truncated for the list/header. `None` when the note has no text.
fn note_title(text: Option<&str>) -> Option<String> {
    let line = text?.lines().map(str::trim).find(|line| !line.is_empty())?;
    let line = line.trim_start_matches('#').trim();
    let title: String = line.chars().take(60).collect();
    (!title.is_empty()).then_some(title)
}

/// Builds a generic list row from a [`NoteSummary`](genealogy_app::NoteSummary): a title from the
/// text, a `type · language · N references` subtitle, and a 🗒 avatar.
#[must_use]
pub fn note_row(summary: &genealogy_app::NoteSummary, loc: &Localizer) -> RowVm {
    let title = note_title(summary.text.as_deref()).unwrap_or_else(|| summary.human_id.clone());
    let mut parts: Vec<String> = Vec::new();
    if let Some(note_type) = &summary.note_type {
        parts.push(loc.note_type_label(note_type));
    }
    if let Some(language) = &summary.language {
        parts.push(language.clone());
    }
    if !summary.references.is_empty() {
        parts.push(loc.reference_count(summary.references.len()));
    }
    let subtitle = (!parts.is_empty()).then(|| parts.join(" · "));
    RowVm {
        id: summary.human_id.clone(),
        title,
        subtitle,
        avatar: Some("🗒".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a note's detail: the content, then the related-item tabs with counts.
#[must_use]
pub fn note_tabs(detail: &NoteDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("content", None),
        tab("language", Some(detail.translations.len() + 1)),
        tab("references", Some(detail.references.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// Builds a navigable reference vm (kind + ids + localized kind label) for a related record.
pub(crate) fn nav_ref(
    kind: genealogy_app::UsingKind,
    human_id: &str,
    id: &str,
    label: String,
    loc: &Localizer,
) -> UsingRecordVm {
    UsingRecordVm {
        kind,
        human_id: human_id.to_owned(),
        id: id.to_owned(),
        label,
        kind_label: loc.using_kind_label(kind),
    }
}

/// The buffered whole-record draft of a note (create + edit, one mechanism, `record-editing.html`
/// §2/§6): the editable user-facing id, an optional type, the Markdown content, and a BCP-47 language.
/// `existing_human_id` is `None` in create mode and `Some` in edit mode. Nothing is written until Save.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoteDraft {
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The editable user-facing id; blank ⇒ generated on save (edit) / auto-allocated (create).
    pub human_id: String,
    /// The note type, if chosen.
    pub note_type: Option<genealogy_app::NoteType>,
    /// The Markdown content.
    pub text: String,
    /// The content's BCP-47 language.
    pub language: String,
}

impl NoteDraft {
    /// A fresh empty draft for creating a new note.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A draft pre-populated from an existing note for editing. Records the current `human_id` so
    /// [`Self::edits_against`] diffs (supersedes) rather than creates.
    #[must_use]
    pub fn from_detail(detail: &NoteDetail) -> Self {
        Self {
            existing_human_id: Some(detail.human_id.clone()),
            human_id: detail.human_id.clone(),
            note_type: detail.note_type.clone(),
            text: detail.text.clone().unwrap_or_default(),
            language: detail.language.clone().unwrap_or_default(),
        }
    }

    /// Builds the [`NoteChangeSetRequest`] the app commits on Save (create mode).
    #[must_use]
    pub fn to_request(&self) -> NoteChangeSetRequest {
        NoteChangeSetRequest {
            human_id: non_blank(&self.human_id),
            note_type: self.note_type.clone(),
            text: non_blank(&self.text),
            language: non_blank(&self.language),
        }
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode). The primary text and its language commit together as one [`NoteEdit::SetText`] (they
    /// share a single `RichText`); `SetHumanId` is emitted last so the record is only re-keyed after
    /// every other field has committed against its current id (a blank id regenerates).
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<NoteEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        let mut edits = Vec::new();
        if self.note_type != seed.note_type
            && let Some(note_type) = self.note_type.clone()
        {
            edits.push(NoteEdit::SetType {
                human_id: human_id.clone(),
                note_type,
            });
        }
        if self.text != seed.text || self.language != seed.language {
            edits.push(NoteEdit::SetText {
                human_id: human_id.clone(),
                text: self.text.clone(),
                language: non_blank(&self.language),
            });
        }
        if self.human_id.trim() != seed.human_id {
            edits.push(NoteEdit::SetHumanId {
                human_id,
                new_human_id: non_blank(&self.human_id),
            });
        }
        edits
    }
}

impl RecordDraft for NoteDraft {
    type Detail = NoteDetail;

    fn from_detail(detail: &NoteDetail) -> Self {
        Self::from_detail(detail)
    }

    fn is_valid(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod note_draft_tests {
    use super::NoteDraft;
    use crate::navigation::NoteEdit;
    use genealogy_app::NoteType;

    fn seed() -> NoteDraft {
        NoteDraft {
            existing_human_id: Some("N0001".to_owned()),
            human_id: "N0001".to_owned(),
            note_type: Some(NoteType::Research),
            text: "An estate inventory".to_owned(),
            language: "en".to_owned(),
        }
    }

    #[test]
    fn to_request_carries_type_content_and_language() {
        let draft = NoteDraft {
            note_type: Some(NoteType::Research),
            text: "  An estate inventory  ".to_owned(),
            language: "en".to_owned(),
            ..NoteDraft::new()
        };
        let request = draft.to_request();
        assert_eq!(request.note_type, Some(NoteType::Research));
        assert_eq!(request.text.as_deref(), Some("An estate inventory"));
        assert_eq!(request.language.as_deref(), Some("en"));
        assert_eq!(request.human_id, None);
    }

    #[test]
    fn an_unchanged_draft_yields_no_edits() {
        assert!(seed().edits_against(&seed()).is_empty());
    }

    #[test]
    fn text_and_language_commit_as_one_set_text() {
        let draft = NoteDraft {
            language: "nb-NO".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(
            &edits[0],
            NoteEdit::SetText { text, language, .. } if text == "An estate inventory" && language.as_deref() == Some("nb-NO")
        ));
    }

    #[test]
    fn a_blank_human_id_regenerates_and_is_emitted_last() {
        let draft = NoteDraft {
            human_id: String::new(),
            note_type: Some(NoteType::General),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 2);
        assert!(matches!(&edits[0], NoteEdit::SetType { .. }));
        assert!(matches!(&edits[1], NoteEdit::SetHumanId { new_human_id, .. } if new_human_id.is_none()));
    }
}
