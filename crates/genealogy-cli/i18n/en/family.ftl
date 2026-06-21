## Family output
family-list-empty = No families yet.
family-summary = { $id }  partners: { $partners }  children: { $children }{ $private }
family-none = (none)

## AppError
err-family-not-found = no family with human_id "{ $id }"

## FamilyError (wrapped via AppError::FamilyDomain)
err-family-not-exist = family { $id } does not exist
err-family-exists = family { $id } already exists
err-partner-present = person { $id } is already a partner of this family
err-partner-absent = person { $id } is not a partner of this family
err-child-present = person { $id } is already a child of this family
err-child-absent = person { $id } is not a child of this family
