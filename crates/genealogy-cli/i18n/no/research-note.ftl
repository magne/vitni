## ResearchNote output (ADR 0028)
research-note-list-empty = Ingen forskningsnotater ennå.
research-note-summary = { $id }  emne: { $subject }  { $title }

## Subject labels (kind + internal id; resolving to the subject's own human id is a follow-up)
research-note-subject-person = person { $id }
research-note-subject-family = familie { $id }
research-note-subject-event = hendelse { $id }
research-note-subject-place = sted { $id }

## AppError
err-research-note-not-found = ingen forskningsnotat med human_id "{ $id }"

## ResearchNoteError (wrapped via AppError::ResearchNoteDomain)
err-research-note-not-exist = forskningsnotat { $id } finnes ikke
err-research-note-exists = forskningsnotat { $id } finnes allerede
err-research-note-unknown-subject = forskningsnotatets emne finnes ikke
err-research-note-subject-required = et forskningsnotat må ha minst ett emne
err-research-note-empty-body = et forskningsnotats innhold kan ikke være tomt
