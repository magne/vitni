## Family output
family-list-empty = Ingen familier ennå.
family-summary = { $id }  partnere: { $partners }  barn: { $children }{ $restrictions }
family-none = (ingen)

## AppError
err-family-not-found = ingen familie med human_id "{ $id }"

## FamilyError (wrapped via AppError::FamilyDomain)
err-family-not-exist = familie { $id } finnes ikke
err-family-exists = familie { $id } finnes allerede
err-partner-present = person { $id } er allerede en partner i denne familien
err-partner-absent = person { $id } er ikke en partner i denne familien
err-child-present = person { $id } er allerede et barn i denne familien
err-child-absent = person { $id } er ikke et barn i denne familien
