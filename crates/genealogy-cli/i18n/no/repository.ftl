## Repository output
repository-list-empty = Ingen oppbevaringssteder ennå.
repository-summary = { $id }  { $name }  type: { $repository_type }  adresser: { $addresses }  nettadresser: { $urls }

## Repository-type labels
repository-type-library = bibliotek
repository-type-archive = arkiv
repository-type-church = kirke
repository-type-cemetery = gravlund
repository-type-museum = museum
repository-type-website = nettsted
repository-type-collection = samling

## AppError
err-repository-not-found = ingen oppbevaringssted med human_id "{ $id }"

## RepositoryError (wrapped via AppError::RepositoryDomain)
err-repository-not-exist = oppbevaringssted { $id } finnes ikke
err-repository-exists = oppbevaringssted { $id } finnes allerede
err-repository-empty-name = et oppbevaringssteds navn kan ikke være tomt
