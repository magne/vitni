use super::{Localizer, NoteError, NoteSummary, NoteType, fl};

impl Localizer {
    /// `No notes yet.`
    #[must_use]
    pub fn note_list_empty(&self) -> String {
        fl!(self.loader, "note-list-empty")
    }

    /// One note line: `N0001  type: general  Born in Bergen.`.
    #[must_use]
    pub fn note_summary_line(&self, summary: &NoteSummary) -> String {
        let note_type = match &summary.note_type {
            Some(note_type) => self.note_type(note_type),
            None => fl!(self.loader, "no-value"),
        };
        let text = match &summary.text {
            Some(text) => text.clone(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "note-summary",
            id = summary.human_id.clone(),
            note_type = note_type,
            text = text
        )
    }

    /// The localized note-type label; a custom value renders verbatim.
    fn note_type(&self, note_type: &NoteType) -> String {
        match note_type {
            NoteType::General => fl!(self.loader, "note-type-general"),
            NoteType::Research => fl!(self.loader, "note-type-research"),
            NoteType::Transcript => fl!(self.loader, "note-type-transcript"),
            NoteType::Citation => fl!(self.loader, "note-type-citation"),
            NoteType::Custom(value) => value.clone(),
        }
    }

    pub(super) fn note_error(&self, error: &NoteError) -> String {
        match error {
            NoteError::NotFound(id) => fl!(self.loader, "err-note-not-exist", id = id.to_string()),
            NoteError::AlreadyExists(id) => fl!(self.loader, "err-note-exists", id = id.to_string()),
            NoteError::RetractsMissingAssertion(id) | NoteError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }
}
