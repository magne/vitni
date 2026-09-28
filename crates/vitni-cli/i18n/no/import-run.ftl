## Import-run output
import-run-list-empty = Ingen importkjøringer ennå.
import-run-summary = { $id }  { $started }  { $status }  { $plugin } { $version }  { $source }  → { $dataset }  av { $operator }  { $created } opprettet, { $resolved } gjenkjent
import-run-status-running = pågår
import-run-status-finished = fullført
import-run-status-cancelled = avbrutt
import-run-status-failed = feilet
dataset-list-empty = Ingen datasett ennå.
dataset-summary = { $id }  { $label }  { $runs ->
    [one] 1 kjøring
   *[other] { $runs } kjøringer
}

## AppError
err-import-run-not-found = ingen importkjøring med id "{ $id }"

## ImportRunError (wrapped via AppError::ImportRunDomain)
err-import-run-not-exist = importkjøring { $id } finnes ikke
err-import-run-exists = importkjøring { $id } finnes allerede
err-import-run-ended = importkjøring { $id } er allerede avsluttet

## DatasetError (wrapped via AppError::Dataset)
err-dataset-not-found = ingen { $scheme }-datasett passer til «{ $query }» (se `vitni import-run datasets`)
err-dataset-ambiguous = «{ $query }» gjelder flere datasett ({ $candidates }); gi id-en til --dataset
err-dataset-required = arbeidsområdet har allerede { $scheme }-data fra { $candidates }; bruk --dataset <etikett-eller-id> for å importere inn i et av dem, eller --new-dataset hvis filen er et annet tre
err-dataset-global = { $scheme }-importer bruker alltid det ene globale datasettet; fjern --dataset og --new-dataset
