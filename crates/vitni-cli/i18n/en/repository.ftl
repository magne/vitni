## Repository output
repository-list-empty = No repositories yet.
repository-summary = { $id }  { $name }  type: { $repository_type }  addresses: { $addresses }  urls: { $urls }

## Repository-type labels
repository-type-library = library
repository-type-archive = archive
repository-type-church = church
repository-type-cemetery = cemetery
repository-type-museum = museum
repository-type-website = website
repository-type-collection = collection

## AppError
err-repository-not-found = no repository with human_id "{ $id }"

## RepositoryError (wrapped via AppError::RepositoryDomain)
err-repository-not-exist = repository { $id } does not exist
err-repository-exists = repository { $id } already exists
err-repository-empty-name = a repository name must not be empty
err-repository-merge-conflict = repositories { $surviving } and { $merged } cannot be merged: { $reason }
err-repository-distinct-from-itself = repository { $id } cannot be distinguished from itself
err-repository-identity-decided = repositories { $repository } and { $other } already have a live identity decision; undo it first
