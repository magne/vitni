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
tab-attributes = Attributes
tab-history = History
tab-empty = Nothing here yet.
history-placeholder = The change log arrives in a later milestone.
history-empty = No changes recorded yet.
history-note = Every change is an immutable event recording who, when, and why — an audit trail that comes free from the event-sourced core. Any entry can be undone.

# Change-log summaries (History tab + activity feed) — one phrase per event type
history-person-created = Person created
history-name-asserted = Name asserted
history-sex-asserted = Sex asserted
history-fact-asserted = Fact asserted
history-participation-asserted = Added to an event
history-association-asserted = Association asserted
history-media-attached = Media attached
history-note-attached = Note attached
history-citation-added = Citation attached
history-external-id-added = External id added
history-tagged = Tag applied
history-untagged = Tag removed
history-restrictions-changed = Privacy restrictions changed
history-assertion-retracted = Assertion retracted
history-assertion-superseded = Assertion superseded
history-persons-merged = Persona merged
history-generic = Recorded a change

# Change-log operator line
history-operator-human = { $name } · { $confidence }
history-operator-agent = { $name } ({ $kind })
history-operator-software = software agent
history-operator-ai = AI model
history-operator-unknown = unknown operator
history-undo = Undo: { $what }
history-undo-short = Undo

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
field-language = Language
field-source = Source
field-surety = Surety
field-relationship = Relationship
field-page = Page
field-attribute-type = Type
field-evidence = Evidence

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
action-remove-tag = Remove tag
action-add-association = Add association
action-set-page = Set page
action-set-date = Set date
action-set-confidence = Set confidence
action-set-evidence = Set evidence analysis
action-add-attribute = Add attribute
action-compare = Compare

# Vital summary affixes (detail header)
vital-born = b. { $date }
vital-died = d. { $date }

# Overview section headings
section-vitals = Vital facts
section-family = Immediate family
overview-note = Every fact shows its surety and whether a source backs it. Facts without a citation are flagged — the evidence-first cue carried across every screen.
family-children = Children

# Evidence cues (colour is never the only signal)
no-source = No source
source-count = { $count } sources
provenance-title = Why we believe this

# Citations
citation-list-empty = No citations yet.

# Evidence Explained analysis axes (data-model §7)
evidence-original = Original
evidence-derivative = Derivative
evidence-primary = Primary
evidence-secondary = Secondary
evidence-direct = Direct
evidence-indirect = Indirect
evidence-negative = Negative

# Evidence level — the personas badge (data-model §7)
evidence-level-persona = Persona
evidence-level-conclusion = Conclusion

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

# Dashboard
dashboard-title = Workspace at a glance
dashboard-stat-people = People
dashboard-people-caption = { $families } families · { $events } events
dashboard-stat-evidence = Evidence health
dashboard-stat-evidence-caption = facts with at least one source
dashboard-stat-attention = Needs attention
dashboard-recent-activity = Recent activity — who changed what
dashboard-import-batch = { $count } records imported
dashboard-jump-back = Jump back in
dashboard-data-quality = Data quality
dashboard-no-source-facts = Facts without a source
dashboard-later-milestone = Coming in a later milestone
dashboard-activity-empty = No activity yet.

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
