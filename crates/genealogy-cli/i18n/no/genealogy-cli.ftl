# Norwegian catalogue for the genealogy CLI (ADR 0003), keyed `no` (generic Norwegian).
# Requests for nb-NO/nb and nn-NO/nn resolve here via the macrolanguage fallback chain
# (nb-NO -> nb -> no -> en, nn-NO -> nn -> no -> en); add `nb`/`nn` catalogues later to specialize.
# `err-self-association` is intentionally omitted to exercise fallback to the en baseline.

## Command output
created = Opprettet { $id }
updated = Oppdatert { $id }
init-success = Initialiserte arbeidsområde "{ $name }" i { $path }
config-line = Konfigurasjon: { $path }
list-empty = Ingen personer ennå.
summary = { $id }  { $name }  kjønn: { $sex }{ $private }
no-name = (uten navn)
no-value = -
private-tag = { " " }[privat]
error-prefix = feil: { $message }

## Sex labels
sex-male = mann
sex-female = kvinne
sex-unknown = ukjent

## AppError
err-config = konfigurasjonsfeil: { $detail }
err-workspace = arbeidsområdefeil: { $detail }
err-human-id-taken = human_id "{ $id }" er allerede i bruk
err-person-not-found = ingen person med human_id "{ $id }"

## PersonError (wrapped via AppError::Domain)
err-person-not-exist = person { $id } finnes ikke
err-person-exists = person { $id } finnes allerede
err-empty-name = et navn må ha et fornavn eller et etternavn
err-missing-assertion = påstand { $id } finnes ikke eller er allerede trukket tilbake
err-invalid-date = ugyldig dato: { $detail }
err-merge-conflict = personer { $surviving } og { $merged } kan ikke slås sammen: { $reason }

## DbError
err-db-unsupported = ikke støttet: { $detail }
err-db-backend = lagringsfeil: { $detail }
err-db-malformed = ugyldige inndata: { $detail }
