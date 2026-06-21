## Note output
note-list-empty = No notes yet.
note-summary = { $id }  type: { $note_type }  { $text }

## Note-type labels
note-type-general = general
note-type-research = research
note-type-transcript = transcript
note-type-citation = citation

## AppError
err-note-not-found = no note with human_id "{ $id }"

## NoteError (wrapped via AppError::NoteDomain)
err-note-not-exist = note { $id } does not exist
err-note-exists = note { $id } already exists
