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
tab-names = Names
tab-facts = Facts
tab-events = Events
tab-associations = Associations
tab-families = Families
tab-citations = Citations
tab-media = Media
tab-notes = Notes
tab-tags = Tags
tab-history = History
tab-empty = Nothing here yet.
history-placeholder = The change log arrives in a later milestone.

# Edit field labels
field-nickname = Nickname
field-prefix = Prefix
field-suffix = Suffix
field-name-type = Name type
field-fact-type = Fact type
field-value = Value
field-date = Date
field-place = Place
field-confidence = Confidence
field-citation = Citation
field-media = Media
field-note = Note
field-tag = Tag
field-association = Person
field-role = Role

# Actions
action-save = Save
action-cancel = Cancel
action-saved = Saved
action-dismiss = Dismiss
action-edit = Edit
action-add-name = Add name
action-add-fact = Add fact
action-add-source = Add source
action-attach-citation = Attach citation
action-attach-media = Attach media
action-attach-note = Attach note
action-add-tag = Add tag
action-add-association = Add association

# Evidence cues (colour is never the only signal)
no-source = No source
source-count = { $count } sources
provenance-title = Why we believe this

# Confidence levels (data-model §8)
confidence-very-low = Very low
confidence-low = Low
confidence-normal = Normal
confidence-high = High
confidence-very-high = Very high

# Fact types (INDI attributes — data-model §7)
fact-birth = Birth
fact-death = Death
fact-baptism = Baptism
fact-burial = Burial
fact-occupation = Occupation
fact-residence = Residence
fact-religion = Religion
fact-caste = Caste
fact-physical-description = Physical description
fact-education = Education
fact-ethnicity = Ethnicity
fact-national-id = National ID
fact-nationality = Nationality
fact-number-of-children = Number of children
fact-number-of-marriages = Number of marriages
fact-property = Property
fact-social-security-number = Social security number
fact-nobility-title = Nobility title

# Name types
name-type-birth = Birth name
name-type-married = Married name
name-type-maiden = Maiden name
name-type-immigrant = Immigrant name
name-type-professional = Professional name
name-type-aka = Also known as
name-type-religious = Religious name

# Roles (shared by event participation and person associations)
role-primary = Primary
role-witness = Witness
role-officiator = Officiator
role-clergy = Clergy
role-father = Father
role-mother = Mother
role-parent = Parent
role-child = Child
role-husband = Husband
role-wife = Wife
role-spouse = Spouse
role-godparent = Godparent
role-friend = Friend
role-neighbour = Neighbour
role-multiple = Multiple
role-bride = Bride
role-groom = Groom

# Child–parent relationships (data-model §6)
rel-birth = Birth
rel-adopted = Adopted
rel-foster = Foster
rel-step = Step
rel-sealed = Sealed
rel-unknown = Unknown

# Date qualifiers (numeric rendering; qualifiers localized)
date-before = before { $date }
date-after = after { $date }
date-about = about { $date }
date-from = from { $date }
date-to = to { $date }
date-range = between { $start } and { $end }
date-span = { $start } to { $end }
date-estimated = estimated { $date }
date-calculated = calculated { $date }

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
