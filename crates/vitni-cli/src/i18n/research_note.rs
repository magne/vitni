use super::{Localizer, ResearchNoteError, ResearchNoteSummary, SubjectRef, fl};

impl Localizer {
    /// `No research notes yet.`
    #[must_use]
    pub fn research_note_list_empty(&self) -> String {
        fl!(self.loader, "research-note-list-empty")
    }

    /// One research-note line: `A0001  subject: person <uuid>  Same person as the 1865 census?`.
    /// Multiple subjects (ADR 0028 §2) render comma-joined.
    #[must_use]
    pub fn research_note_summary_line(&self, summary: &ResearchNoteSummary) -> String {
        let subject = if summary.subjects.is_empty() {
            fl!(self.loader, "no-value")
        } else {
            summary
                .subjects
                .iter()
                .map(|&s| self.subject_label(s))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let title = summary.title.clone().unwrap_or_else(|| fl!(self.loader, "no-value"));
        fl!(
            self.loader,
            "research-note-summary",
            id = summary.human_id.clone(),
            subject = subject,
            title = title
        )
    }

    /// The subject reference rendered as `<kind> <uuid>` — resolving it to the subject's own
    /// human-readable id is a documented follow-up (ADR 0028; the DTO carries the internal id).
    fn subject_label(&self, subject: SubjectRef) -> String {
        match subject {
            SubjectRef::Person(id) => fl!(self.loader, "research-note-subject-person", id = id.to_string()),
            SubjectRef::Family(id) => fl!(self.loader, "research-note-subject-family", id = id.to_string()),
            SubjectRef::Event(id) => fl!(self.loader, "research-note-subject-event", id = id.to_string()),
            SubjectRef::Place(id) => fl!(self.loader, "research-note-subject-place", id = id.to_string()),
        }
    }

    pub(super) fn research_note_error(&self, error: &ResearchNoteError) -> String {
        match error {
            ResearchNoteError::NotFound(id) => fl!(self.loader, "err-research-note-not-exist", id = id.to_string()),
            ResearchNoteError::AlreadyExists(id) => {
                fl!(self.loader, "err-research-note-exists", id = id.to_string())
            }
            ResearchNoteError::UnknownSubject => fl!(self.loader, "err-research-note-unknown-subject"),
            ResearchNoteError::SubjectRequired => fl!(self.loader, "err-research-note-subject-required"),
            ResearchNoteError::EmptyBody => fl!(self.loader, "err-research-note-empty-body"),
            ResearchNoteError::RetractsMissingAssertion(id) | ResearchNoteError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }
}
