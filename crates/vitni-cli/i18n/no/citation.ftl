## Citation output
citation-list-empty = Ingen sitater ennå.
citation-summary = { $id }  kilde: { $source }  side: { $page }  dato: { $date }  sikkerhet: { $confidence }

## AppError
err-citation-not-found = ingen sitat med human_id "{ $id }"

## CitationError (wrapped via AppError::CitationDomain)
err-citation-not-exist = sitat { $id } finnes ikke
err-citation-exists = sitat { $id } finnes allerede
err-unknown-source = sitat viser til ukjent kilde { $id }
err-citation-merge-conflict = kildehenvisningene { $surviving } og { $merged } kan ikke slås sammen: { $reason }
err-citation-distinct-from-itself = kildehenvisning { $id } kan ikke skilles fra seg selv
err-citation-identity-decided = kildehenvisningene { $citation } og { $other } har allerede en gjeldende identitetsavgjørelse; angre den først
