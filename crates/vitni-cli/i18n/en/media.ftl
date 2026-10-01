## Media output
media-list-empty = No media yet.
media-summary = { $id }  path: { $path }  checksum: { $checksum }  attributes: { $attributes }

## AppError
err-media-not-found = no media with human_id "{ $id }"

## MediaError (wrapped via AppError::MediaDomain)
err-media-not-exist = media { $id } does not exist
err-media-exists = media { $id } already exists
err-media-merge-conflict = media objects { $surviving } and { $merged } cannot be merged: { $reason }
err-media-distinct-from-itself = media object { $id } cannot be distinguished from itself
err-media-identity-decided = media objects { $media } and { $other } already have a live identity decision; undo it first
