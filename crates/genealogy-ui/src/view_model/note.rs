use super::{
    DetailTab, HistoryEntryVm, Localizer, NoteChangeSetRequest, RestrictionKind, RowVm, TagRef, UsingRecordVm,
    non_blank, using_record_vm,
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

/// The create form's in-memory draft for a new note (`record-editing.html` §6): an optional type,
/// Markdown content, and a BCP-47 language, buffered until Save. Create-only; nothing is written
/// until Save commits a [`NoteChangeSetRequest`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoteDraft {
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

    /// Whether the operator has entered anything — the Save gate.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.note_type.is_some() || non_blank(&self.text).is_some() || non_blank(&self.language).is_some()
    }

    /// Builds the [`NoteChangeSetRequest`] the app commits on Save.
    #[must_use]
    pub fn to_request(&self) -> NoteChangeSetRequest {
        NoteChangeSetRequest {
            note_type: self.note_type.clone(),
            text: non_blank(&self.text),
            language: non_blank(&self.language),
        }
    }
}

#[cfg(test)]
mod note_draft_tests {
    use super::NoteDraft;
    use genealogy_app::NoteType;

    #[test]
    fn a_fresh_draft_is_not_dirty() {
        assert!(!NoteDraft::new().is_dirty());
    }

    #[test]
    fn to_request_carries_type_content_and_language() {
        let draft = NoteDraft {
            note_type: Some(NoteType::Research),
            text: "  An estate inventory  ".to_owned(),
            language: "en".to_owned(),
        };
        let request = draft.to_request();
        assert_eq!(request.note_type, Some(NoteType::Research));
        assert_eq!(request.text.as_deref(), Some("An estate inventory"));
        assert_eq!(request.language.as_deref(), Some("en"));
    }
}
