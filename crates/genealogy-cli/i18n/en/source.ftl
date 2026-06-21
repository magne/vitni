## Source output
source-list-empty = No sources yet.
source-summary = { $id }  { $title }  author: { $author }  repos: { $repositories }  attrs: { $attributes }

## AppError
err-source-not-found = no source with human_id "{ $id }"

## SourceError (wrapped via AppError::SourceDomain)
err-source-not-exist = source { $id } does not exist
err-source-exists = source { $id } already exists
err-source-unknown-repository = source references unknown repository { $id }
