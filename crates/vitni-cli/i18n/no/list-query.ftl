## List queries (ADR 0052): `list --search/--filter/--sort`
# A `list` whose `--search` or `--filter` matched no record.
list-no-match = Ingenting passer søket.

## ListQueryError (wrapped via AppError::ListQuery)
err-list-unknown-column = { $kind }-lista har ingen kolonne `{ $column }` (kolonnene: { $columns })
err-list-unknown-operator = filteret på `{ $column }` har ingen operator `{ $op }`
err-list-unsupported-operator = kolonnen `{ $column }` tar ikke filteret `{ $op }`
err-list-wrong-value = filteret `{ $column }:{ $op }` trenger en annen slags verdi
err-list-unknown-choice = `{ $value }` er ikke en verdi i kolonnen `{ $column }`
err-list-unknown-reference = filteret på `{ $column }` nevner `{ $value }`, som ikke finnes
err-list-unsortable = kolonnen `{ $column }` kan ikke sorteres
err-list-kind-mismatch = et { $found }-søk kan ikke liste { $expected }-poster
