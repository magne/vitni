## List queries (ADR 0052): `list --search/--filter/--sort`
# A `list` whose `--search` or `--filter` matched no record.
list-no-match = Nothing matches the query.

## ListQueryError (wrapped via AppError::ListQuery)
err-list-unknown-column = the { $kind } list has no column `{ $column }` (its columns: { $columns })
err-list-unknown-operator = the `{ $column }` filter names no operator `{ $op }`
err-list-unsupported-operator = the `{ $column }` column takes no `{ $op }` filter
err-list-wrong-value = the `{ $column }:{ $op }` filter needs a value of another kind
err-list-unknown-choice = `{ $value }` is not a value of the `{ $column }` column
err-list-unknown-reference = the `{ $column }` filter names `{ $value }`, which does not exist
err-list-unsortable = the `{ $column }` column cannot be sorted
err-list-kind-mismatch = a { $found } query cannot list { $expected } records
