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

# Family slice
tab-children = Children
family-list-empty = No families yet.
family-overview-note = Partners are recorded with neutral roles — no gendered husband/wife assumption. Every family fact shows its surety and whether a source backs it.
section-partners = Partners
section-marriage = Marriage
family-children-count = { $count } children
field-born = Born
field-partner = Partner
field-child = Child
action-add-partner = Add partner
action-add-child = Add child
action-link-event = Link family event

# Event types (data-model §7) — shared by family events and the Event slice
event-type-birth = Birth
event-type-death = Death
event-type-marriage = Marriage
event-type-baptism = Baptism
event-type-christening = Christening
event-type-burial = Burial
event-type-cremation = Cremation
event-type-census = Census
event-type-residence = Residence
event-type-immigration = Immigration
event-type-emigration = Emigration
event-type-adoption = Adoption
event-type-confirmation = Confirmation
event-type-bar-mitzvah = Bar mitzvah
event-type-bas-mitzvah = Bas mitzvah
event-type-first-communion = First communion
event-type-graduation = Graduation
event-type-naturalization = Naturalization
event-type-ordination = Ordination
event-type-probate = Probate
event-type-retirement = Retirement
event-type-will = Will
event-type-engagement = Engagement
event-type-annulment = Annulment
event-type-divorce = Divorce
event-type-divorce-filed = Divorce filed
event-type-marriage-banns = Marriage banns
event-type-marriage-contract = Marriage contract
event-type-marriage-license = Marriage license
event-type-marriage-settlement = Marriage settlement

# Event · Place slices (PR8)
tab-participants = Participants
tab-hierarchy = Hierarchy
event-list-empty = No events yet.
event-overview-note = Dates are structured, not free text — the model keeps the precision and calendar so dates stay machine-comparable. Every fact shows its surety and source.
place-list-empty = No places yet.
place-overview-note = A place keeps its name history and jurisdiction chain over time, so a record resolves to the right historical name. Facts show surety and source.
place-names-note = Names are dated and language-tagged, so the gazetteer reflects how a place was called at any point in time.
place-hierarchy-note = Each enclosed-by link can be dated — jurisdictions change, so the chain is valid for a span, not forever.
action-add-participant = Add participant
action-add-enclosing = Add enclosing place
place-type-country = Country
place-type-county = County
place-type-municipality = Municipality
place-type-parish = Parish
place-type-city = City
place-type-town = Town
place-type-village = Village
place-type-farm = Farm
place-type-building = Building

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

# Source · Repository slices (Phase 5 PR9)
source-list-empty = No sources yet.
source-overview-note = A source is the master record; individual citations point into it with a page and an evidence analysis. The records that use it make provenance traceable in both directions.
source-citations-note = Citations that use this source — follow each into the record it backs.
repository-list-empty = No repositories yet.
repository-overview-note = A repository is the physical or virtual place that holds sources. Sources held here link back to their original archive — provenance you can follow from a fact all the way to the shelf.

# Source · Repository detail tabs
tab-repositories = Repositories
tab-sources = Sources
tab-addresses = Addresses
tab-urls = URLs

# Source · Repository overview sections
section-bibliographic = Bibliographic
section-reliability = Reliability
section-repository = Repository
section-contact = Primary contact

# Source · Repository field labels
field-title = Title
field-author = Author
field-publication = Publication
field-abbreviation = Abbreviation
field-call-number = Call number
field-media-type = Media type
field-used-by = Used by
field-typical-surety = Typical surety
field-type = Type
field-street = Street
field-locality = Locality
field-region = Region
field-postal-code = Postal code
field-country = Country
field-phone = Phone
field-email = Email
field-url = Link
field-description = Description
field-backs-record = Backs record
field-sources = Sources
field-citations = Citations

# Source · Repository actions
action-link-repository = Link repository
action-add-address = Add address
action-add-url = Add URL
action-link-source = Link source

# Repository types
repository-type-library = Library
repository-type-archive = Archive
repository-type-church = Church
repository-type-cemetery = Cemetery
repository-type-museum = Museum
repository-type-website = Website
repository-type-collection = Collection

# Source media types (GEDCOM MEDI)
media-type-book = Book
media-type-card = Card
media-type-electronic = Electronic
media-type-fiche = Fiche
media-type-film = Film
media-type-magazine = Magazine
media-type-manuscript = Manuscript
media-type-map = Map
media-type-newspaper = Newspaper
media-type-photo = Photo
media-type-tombstone = Tombstone
media-type-video = Video
media-type-audio = Audio

# Backs-record sub-context (Source citations tab)
citing-name = Name
citing-partner = Partner
citing-child = Child
citing-family-event = Family event
citing-place-type = Place type

# Media · Note slices (PR 10)
tab-content = Content
tab-language = Language
tab-references = References
note-type-general = General
note-type-research = Research
note-type-transcript = Transcript
note-type-citation = Citation
using-kind-person = Person
using-kind-family = Family
using-kind-event = Event
using-kind-place = Place
using-kind-source = Source
using-kind-citation = Citation
using-kind-repository = Repository
reference-count = { $count } references
field-file-path = File path
field-mime = MIME
field-checksum = Checksum
field-translator = Translator
field-translation = Translated text
field-object = Object
action-add-translation = Add translation
section-file = File
section-content = Content
section-primary-language = Primary language
section-related-media = Related media
media-preview = Preview
media-list-empty = No media yet.
note-list-empty = No notes yet.
media-used-by-note = Every record that uses this media object is listed here — the back-references the event-sourced core keeps for free.
note-references-note = What references this note. Notes are shared — one research note can inform a person, a family, and an event at once.
note-content-note = Notes carry a type and rich text — the working log behind a conclusion, which the audit trail keeps tied to the facts it informed.
