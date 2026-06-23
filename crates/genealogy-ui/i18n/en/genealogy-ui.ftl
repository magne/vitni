# Presentation-layer strings (ADR 0003). The renderer owns its own chrome catalogue; this catalogue
# holds the value labels, field labels, and error surface the view-models need.

# Value placeholders
no-name = (no name)
no-value = -

# Privacy restrictions (GEDCOM v7 RESN — data-model §6)
restriction-confidential = Confidential
restriction-locked = Locked
restriction-privacy = Privacy

# Sex labels
sex-male = male
sex-female = female
sex-unknown = unknown
sex-intersex = intersex

# Field labels
field-id = ID
field-name = Name
field-given = Given name
field-surname = Surname
field-sex = Sex
field-private = Private

# Person list
list-empty = No persons yet.

# Detail tabs
tab-overview = Overview
tab-citations = Citations
tab-media = Media
tab-notes = Notes
tab-tags = Tags

# Errors
error-prefix = error: { $message }
err-config = configuration error: { $detail }
err-workspace = workspace error: { $detail }
err-not-found = { $id } not found
err-domain = invalid operation
err-plugin = plugin error: { $detail }
err-db-unsupported = unsupported: { $detail }
err-db-backend = database error: { $detail }
err-db-malformed = malformed data: { $detail }
