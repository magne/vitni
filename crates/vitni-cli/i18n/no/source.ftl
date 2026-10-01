## Source output
source-list-empty = Ingen kilder ennå.
source-summary = { $id }  { $title }  forfatter: { $author }  arkiv: { $repositories }  attr: { $attributes }

## AppError
err-source-not-found = ingen kilde med human_id "{ $id }"

## SourceError (wrapped via AppError::SourceDomain)
err-source-not-exist = kilde { $id } finnes ikke
err-source-exists = kilde { $id } finnes allerede
err-source-unknown-repository = kilde refererer til ukjent arkiv { $id }
err-source-merge-conflict = kildene { $surviving } og { $merged } kan ikke slås sammen: { $reason }
err-source-distinct-from-itself = kilde { $id } kan ikke skilles fra seg selv
err-source-identity-decided = kildene { $source } og { $other } har allerede en gjeldende identitetsavgjørelse; angre den først
