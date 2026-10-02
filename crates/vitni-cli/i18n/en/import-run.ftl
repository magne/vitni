## Import-run output
import-run-list-empty = No import runs yet.
import-run-summary = { $id }  { $started }  { $status }  { $plugin } { $version }  { $source }  → { $dataset }  by { $operator }  { $created } created, { $resolved } resolved
import-run-status-running = running
import-run-status-finished = finished
import-run-status-cancelled = cancelled
import-run-status-failed = failed
dataset-list-empty = No datasets yet.
dataset-summary = { $id }  { $label }  { $runs ->
    [one] 1 run
   *[other] { $runs } runs
}

## Dataset proposal (ADR 0037 §3)
import-dataset-proposed = This file looks like a later export of "{ $label }": { $shared } of its { $keys } people and families were imported from it. Import into it? [y/N]
import-dataset-proposed-accepted = Importing into "{ $label }", which this file looks like a later export of: { $shared } of its { $keys } people and families were imported from it.

## AppError
err-import-run-not-found = no import run with id "{ $id }"

## ImportRunError (wrapped via AppError::ImportRunDomain)
err-import-run-not-exist = import run { $id } does not exist
err-import-run-exists = import run { $id } already exists
err-import-run-ended = import run { $id } has already ended

## DatasetError (wrapped via AppError::Dataset)
err-dataset-not-found = no { $scheme } dataset matches "{ $query }" (see `vitni import-run datasets`)
err-dataset-ambiguous = "{ $query }" names more than one dataset ({ $candidates }); pass its id to --dataset
err-dataset-required = this workspace already holds { $scheme } data from { $candidates }; pass --dataset <label-or-id> to import into one of them, or --new-dataset if this file is a different tree
err-dataset-global = { $scheme } imports always use its one global dataset; drop --dataset and --new-dataset
