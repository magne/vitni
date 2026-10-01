## Media output
media-list-empty = Ingen medier ennå.
media-summary = { $id }  sti: { $path }  sjekksum: { $checksum }  attributter: { $attributes }

## AppError
err-media-not-found = ingen medium med human_id "{ $id }"

## MediaError (wrapped via AppError::MediaDomain)
err-media-not-exist = medium { $id } finnes ikke
err-media-exists = medium { $id } finnes allerede
err-media-merge-conflict = medieobjektene { $surviving } og { $merged } kan ikke slås sammen: { $reason }
err-media-distinct-from-itself = medieobjekt { $id } kan ikke skilles fra seg selv
err-media-identity-decided = medieobjektene { $media } og { $other } har allerede en gjeldende identitetsavgjørelse; angre den først
