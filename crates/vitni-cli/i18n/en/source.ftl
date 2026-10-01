## Source output
source-list-empty = No sources yet.
source-summary = { $id }  { $title }  author: { $author }  repos: { $repositories }  attrs: { $attributes }

## AppError
err-source-not-found = no source with human_id "{ $id }"

## SourceError (wrapped via AppError::SourceDomain)
err-source-not-exist = source { $id } does not exist
err-source-exists = source { $id } already exists
err-source-unknown-repository = source references unknown repository { $id }
err-source-merge-conflict = sources { $surviving } and { $merged } cannot be merged: { $reason }
err-source-distinct-from-itself = source { $id } cannot be distinguished from itself
err-source-identity-decided = sources { $source } and { $other } already have a live identity decision; undo it first
