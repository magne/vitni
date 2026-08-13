## ResearchNote output (ADR 0028)
research-note-list-empty = No research notes yet.
research-note-summary = { $id }  subject: { $subject }  { $title }

## Subject labels (kind + internal id; resolving to the subject's own human id is a follow-up)
research-note-subject-person = person { $id }
research-note-subject-family = family { $id }
research-note-subject-event = event { $id }
research-note-subject-place = place { $id }

## AppError
err-research-note-not-found = no research note with human_id "{ $id }"

## ResearchNoteError (wrapped via AppError::ResearchNoteDomain)
err-research-note-not-exist = research note { $id } does not exist
err-research-note-exists = research note { $id } already exists
err-research-note-unknown-subject = the research note's subject does not exist
err-research-note-subject-required = a research note must name at least one subject
err-research-note-empty-body = a research note's body must not be empty
