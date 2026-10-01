## Citation output
citation-list-empty = No citations yet.
citation-summary = { $id }  source: { $source }  page: { $page }  date: { $date }  confidence: { $confidence }

## AppError
err-citation-not-found = no citation with human_id "{ $id }"

## CitationError (wrapped via AppError::CitationDomain)
err-citation-not-exist = citation { $id } does not exist
err-citation-exists = citation { $id } already exists
err-unknown-source = citation references unknown source { $id }
err-citation-merge-conflict = citations { $surviving } and { $merged } cannot be merged: { $reason }
err-citation-distinct-from-itself = citation { $id } cannot be distinguished from itself
err-citation-identity-decided = citations { $citation } and { $other } already have a live identity decision; undo it first
