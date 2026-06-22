# Presentasjonslag-strenger (ADR 0003). Renderer-laget eier sin egen ramme-katalog; denne katalogen
# holder verdietiketter, feltetiketter og feiloverflaten visningsmodellene trenger.

# Verdi-plassholdere
no-name = (uten navn)
no-value = -
private-tag = (privat)

# Kjønnsetiketter
sex-male = mann
sex-female = kvinne
sex-unknown = ukjent
sex-intersex = interkjønn

# Feltetiketter
field-id = ID
field-name = Navn
field-given = Fornavn
field-surname = Etternavn
field-sex = Kjønn
field-private = Privat

# Personliste
list-empty = Ingen personer ennå.

# Feil
error-prefix = feil: { $message }
err-config = konfigurasjonsfeil: { $detail }
err-workspace = arbeidsområdefeil: { $detail }
err-not-found = { $id } finnes ikke
err-domain = ugyldig operasjon
err-plugin = programtilleggsfeil: { $detail }
err-db-unsupported = ikke støttet: { $detail }
err-db-backend = databasefeil: { $detail }
err-db-malformed = ødelagte data: { $detail }
