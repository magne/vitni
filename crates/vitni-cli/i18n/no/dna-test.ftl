## DNA-test output
dna-test-list-empty = Ingen DNA-tester ennå.
dna-test-summary = { $id }  person: { $person }  leverandør: { $provider }  type: { $test_type }  haplogrupper: { $haplogroups }

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
err-dna-test-not-found = ingen DNA-test med human_id "{ $id }"

## DnaTestError (wrapped via AppError::DnaTestDomain)
err-dna-test-not-exist = DNA-test { $id } finnes ikke
err-dna-test-exists = DNA-test { $id } finnes allerede
err-dna-test-unknown-person = DNA-test viser til ukjent person { $id }
