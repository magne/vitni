## DNA-match output
dna-match-list-empty = No DNA matches yet.
dna-match-summary = { $id }  shared: { $shared } cM  predicted: { $predicted }  status: { $status }  segments: { $segments }
dna-match-status-confirmed = confirmed
dna-match-status-rejected = rejected

## AppError
err-dna-match-not-found = no dna match with human_id "{ $id }"

## DnaMatchError (wrapped via AppError::DnaMatchDomain)
err-dna-match-not-exist = dna match { $id } does not exist
err-dna-match-exists = dna match { $id } already exists
err-dna-match-unknown-test = dna match references unknown test { $id }
err-dna-match-same-test = a match cannot be between test { $id } and itself
err-dna-match-negative-cm = shared centimorgans must not be negative
