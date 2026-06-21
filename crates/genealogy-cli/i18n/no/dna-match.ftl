## DNA-match output
dna-match-list-empty = Ingen DNA-treff ennå.
dna-match-summary = { $id }  delt: { $shared } cM  antatt: { $predicted }  status: { $status }  segmenter: { $segments }
dna-match-status-confirmed = bekreftet
dna-match-status-rejected = avvist

## AppError
err-dna-match-not-found = ingen DNA-treff med human_id "{ $id }"

## DnaMatchError (wrapped via AppError::DnaMatchDomain)
err-dna-match-not-exist = DNA-treff { $id } finnes ikke
err-dna-match-exists = DNA-treff { $id } finnes allerede
err-dna-match-unknown-test = DNA-treff viser til ukjent test { $id }
err-dna-match-same-test = et treff kan ikke være mellom test { $id } og seg selv
err-dna-match-negative-cm = delte centimorgan kan ikke være negative
