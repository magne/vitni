## Source output
source-list-empty = Ingen kilder ennå.
source-summary = { $id }  { $title }  forfatter: { $author }  arkiv: { $repositories }  attr: { $attributes }

## AppError
err-source-not-found = ingen kilde med human_id "{ $id }"

## SourceError (wrapped via AppError::SourceDomain)
err-source-not-exist = kilde { $id } finnes ikke
err-source-exists = kilde { $id } finnes allerede
err-source-unknown-repository = kilde refererer til ukjent arkiv { $id }
