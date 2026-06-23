## Command output
list-empty = No persons yet.
summary = { $id }  { $name }  sex: { $sex }{ $restrictions }
no-name = (no name)
no-value = -

## Sex labels
sex-male = male
sex-female = female
sex-unknown = unknown
sex-intersex = intersex

## AppError
err-person-not-found = no person with human_id "{ $id }"

## PersonError (wrapped via AppError::Domain)
err-person-not-exist = person { $id } does not exist
err-person-exists = person { $id } already exists
err-empty-name = a name must have a given name or a surname
err-missing-assertion = assertion { $id } is not present or already retracted
err-invalid-date = invalid date: { $detail }
err-merge-conflict = persons { $surviving } and { $merged } cannot be merged: { $reason }
err-self-association = person { $id } cannot be associated with itself
