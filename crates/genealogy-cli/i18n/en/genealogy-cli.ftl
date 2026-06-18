# Fallback (English) catalogue for the genealogy CLI (ADR 0003).
# Every key must exist here — `fl!()` checks calls against this file at compile time.

## Command output
created = Created { $id }
updated = Updated { $id }
init-success = Initialized workspace "{ $name }" at { $path }
config-line = Config: { $path }
list-empty = No persons yet.
summary = { $id }  { $name }  sex: { $sex }{ $private }
no-name = (no name)
no-value = -
private-tag = { " " }[private]
error-prefix = error: { $message }

## Family output
family-list-empty = No families yet.
family-summary = { $id }  partners: { $partners }  children: { $children }{ $private }
family-none = (none)

## Sex labels
sex-male = male
sex-female = female
sex-unknown = unknown

## AppError
err-config = configuration error: { $detail }
err-workspace = workspace error: { $detail }
err-human-id-taken = human_id "{ $id }" is already taken
err-person-not-found = no person with human_id "{ $id }"

## PersonError (wrapped via AppError::Domain)
err-person-not-exist = person { $id } does not exist
err-person-exists = person { $id } already exists
err-empty-name = a name must have a given name or a surname
err-missing-assertion = assertion { $id } is not present or already retracted
err-invalid-date = invalid date: { $detail }
err-merge-conflict = persons { $surviving } and { $merged } cannot be merged: { $reason }
err-self-association = person { $id } cannot be associated with itself

## FamilyError (wrapped via AppError::FamilyDomain)
err-family-not-found = no family with human_id "{ $id }"
err-family-not-exist = family { $id } does not exist
err-family-exists = family { $id } already exists
err-partner-present = person { $id } is already a partner of this family
err-partner-absent = person { $id } is not a partner of this family
err-child-present = person { $id } is already a child of this family
err-child-absent = person { $id } is not a child of this family

## DbError
err-db-unsupported = unsupported: { $detail }
err-db-backend = storage backend error: { $detail }
err-db-malformed = malformed input: { $detail }
