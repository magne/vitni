## Command output
list-empty = Ingen personer ennå.
summary = { $id }  { $name }  kjønn: { $sex }{ $restrictions }
no-name = (uten navn)
no-value = -

## Sex labels
sex-male = mann
sex-female = kvinne
sex-unknown = ukjent
sex-intersex = interkjønn

## AppError
err-person-not-found = ingen person med human_id "{ $id }"

## PersonError (wrapped via AppError::Domain)
err-person-not-exist = person { $id } finnes ikke
err-person-exists = person { $id } finnes allerede
err-empty-name = et navn må ha et fornavn eller et etternavn
err-missing-assertion = påstand { $id } finnes ikke eller er allerede trukket tilbake
err-invalid-date = ugyldig dato: { $detail }
err-merge-conflict = personer { $surviving } og { $merged } kan ikke slås sammen: { $reason }
err-self-association = person { $id } kan ikke knyttes til seg selv
