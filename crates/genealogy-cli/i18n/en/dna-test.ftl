## DNA-test output
dna-test-list-empty = No DNA tests yet.
dna-test-summary = { $id }  person: { $person }  provider: { $provider }  type: { $test_type }  haplogroups: { $haplogroups }

## DNA-provider labels
dna-provider-ancestry = AncestryDNA
dna-provider-23andme = 23andMe
dna-provider-myheritage = MyHeritage
dna-provider-ftdna = FamilyTreeDNA
dna-provider-gedmatch = GEDmatch
dna-provider-livingdna = Living DNA

## DNA-test-type labels
dna-test-type-autosomal = autosomal
dna-test-type-ydna = Y-DNA
dna-test-type-mtdna = mtDNA
dna-test-type-xdna = X-DNA

## AppError
err-dna-test-not-found = no dna test with human_id "{ $id }"

## DnaTestError (wrapped via AppError::DnaTestDomain)
err-dna-test-not-exist = dna test { $id } does not exist
err-dna-test-exists = dna test { $id } already exists
err-dna-test-unknown-person = dna test references unknown person { $id }
