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
field-year = Year
field-month = Month
field-day = Day
field-code = Code
field-web-path = Web path
field-coordinates = Coordinates
field-latitude = Latitude
field-longitude = Longitude
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
history-fact-asserted-kind = { $fact } asserted
history-citation-created = Citation created
history-page-set = Page set
history-date-asserted = Date asserted
history-confidence-set = Confidence set
history-evidence-analysis-set = Evidence analysis set
history-attribute-added = Attribute added
history-dna-match-observed = DNA match observed
history-segment-added = Segment added
history-shared-ancestor-asserted = Shared ancestor asserted
history-match-confirmed = Match confirmed
history-match-rejected = Match rejected
history-dna-test-created = DNA test created
history-provider-set = Provider set
history-kit-id-set = Kit id set
history-test-type-set = Test type set
history-genome-build-set = Genome build set
history-haplogroup-asserted = Haplogroup asserted
history-family-created = Family created
history-partner-added = Partner added
history-partner-removed = Partner removed
history-child-added = Child added
history-child-removed = Child removed
history-family-event-linked = Family event linked
history-media-created = Media created
history-path-set = Path set
history-checksum-set = Checksum set
history-mime-set = MIME type set
history-note-created = Note created
history-note-type-set = Note type set
history-rich-text-set = Text set
history-place-created = Place created
history-place-type-set = Place type set
history-enclosed-by-asserted = Enclosing place asserted
history-coordinates-asserted = Coordinates asserted
history-code-set = Code set
history-repository-created = Repository created
history-repository-type-set = Repository type set
history-name-set = Name set
history-address-added = Address added
history-url-added = URL added
history-source-created = Source created
history-title-set = Title set
history-author-set = Author set
history-pub-info-set = Publication info set
history-abbrev-set = Abbreviation set
history-repository-linked = Repository linked
history-tag-created = Tag created
history-tag-renamed = Tag renamed
history-tag-color-set = Tag colour set
history-tag-priority-set = Tag priority set
history-event-created = Event created
history-event-type-set = Event type set
history-description-set = Description set
history-place-linked = Place linked
history-participant-role-added = Participant role added
history-participant-role-removed = Participant role removed
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
field-content = Content
field-source = Source
field-surety = Surety
field-relationship = Relationship
field-page = Page
field-attribute-type = Type
field-evidence = Evidence
field-human-id = ID (optional)
field-call-name = Call name
field-surname-prefix = Surname prefix

# Person dialog section headings
section-preferred-name = Preferred name
section-gender = Gender
section-tags = Tags
section-name-source = Source for this name
dialog-new-person = New person
dialog-edit-person = Edit person
dialog-attach-existing-citation = Existing citation
dialog-new-citation = New citation
dialog-no-citation = No source
dialog-no-tags = No tags applied
dialog-add-tag-hint = Add a tag

# Actions
action-new-source = + New source
action-new-citation = + New citation
action-save = Save
action-cancel = Cancel
action-saved = Saved
record-draft-badge = draft · not saved
record-unset = —
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
action-detach-citation = Detach citation

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
provenance-title-claim = Why we believe: { $claim }
provenance-asserted-by = asserted by { $who } · { $when }
provenance-asserted-by-undated = asserted by { $who }

# Citations
citation-list-empty = No citations yet.

# Provenance block on save (record-editing.html §5b)
provenance-heading = Provenance
provenance-reason-label = Reason for this change
provenance-reason-hint = optional · shown in History
provenance-attach-citation = Attach citation…
evidence-axis-source = Source quality
evidence-axis-information = Information kind
evidence-axis-evidence = Evidence kind
evidence-axis-unset = —

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
source-new-title = New source
repository-new-title = New repository
note-new-title = New note
media-new-title = New media
place-new-title = New place
family-new-title = New family
dna-test-new-title = New DNA test
event-new-title = New event
event-place-none = No place
event-place-existing = Existing place
event-place-new = New place
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

# Tag · DnaTest · DnaMatch slices (PR 11)
using-kind-media = Media
using-kind-note = Note
using-kind-dna-test = DNA test
using-kind-dna-match = DNA match

# Detail tabs (PR 11)
tab-usage = Usage
tab-haplogroups = Haplogroups
tab-matches = Matches
tab-segments = Segments
tab-ancestors = Shared ancestors

# Tag record (editable) — subtitles, draft, colour picker, validation
tag-row-subtitle = priority { $priority } · { $count ->
    [one] { $count } object
   *[other] { $count } objects
}
tag-header-subtitle = priority { $priority } · applied to { $count ->
    [one] { $count } object
   *[other] { $count } objects
}
tag-priority-badge = priority { $priority }
tag-new-title = New tag
tag-preview-label = Preview
tag-name-required = A name is required.
place-coordinate-invalid = Enter a valid coordinate.
dna-test-person-required = A person is required.
field-swatch = Swatch
action-revert = Revert to saved value
action-step-up = Increase
action-step-down = Decrease
color-picker-title = Choose a colour
color-picker-presets = Presets
color-picker-hex = Hex

# List empty states
tag-list-empty = No tags yet.
dna-test-list-empty = No DNA tests yet.
dna-match-list-empty = No DNA matches yet.

# Section notes
tag-overview-note = Tags are cross-cutting labels with a colour and a priority. Priority orders how tags stack on a record; the colour drives the dot shown everywhere the tag appears.
tag-usage-note = Everything carrying this tag, grouped by object type. Counts come straight from the projection.
dna-test-overview-note = The auditable DNA-test record — kit metadata, haplogroups, and the matches it produces. Rich DNA visualizations arrive in a later phase; here we keep the evidence.
dna-test-ethnicity-note = Admixture / ethnicity percentages are a later-phase visualization. The underlying estimate is stored as a cited assertion so it can be audited and superseded.
dna-match-overview-note = The shared-DNA numbers are a raw observation reported by the provider. The inferred relationship is a separate, cited assertion that carries its own surety and can be superseded without touching the observation — the evidence / conclusion model applied to DNA.
dna-match-segments-note = Matching segments as reported. Maternal / paternal side is phased where a parent kit is available.
dna-match-ancestors-note = Common ancestors inferred from the linked trees of both testers. These are conclusions, each independently cited.

# Overview section headings (PR 11)
section-tag = Tag
section-color = Colour
section-kit = Kit details
section-tested-person = Tested person
section-ethnicity = Ethnicity estimate
section-compared-tests = Compared tests
section-shared-dna = Shared DNA
section-inferred-relationship = Inferred relationship

# Field labels (PR 11)
field-priority = Priority
field-color = Colour
field-provider = Provider
field-test-type = Test type
field-kit-id = Kit id
field-genome-build = Genome build
field-person = Person
field-haplogroup = Haplogroup
field-lineage = Lineage
field-terminal-snp = Terminal SNP
field-shared-cm = Shared cM
field-percent-shared = Percent shared
field-largest-segment = Largest segment
field-segment-count = Segment count
field-predicted = Predicted
field-status = Status
field-compared-test = Compared test
field-test-a = Test A
field-test-b = Test B
field-ancestor = Ancestor
field-chromosome = Chr
field-start = Start (bp)
field-end = End (bp)
field-centimorgans = cM
field-snps = SNPs
field-side = Side
field-object-type = Object type
field-count = Count
field-examples = Examples

# Actions (PR 11)
action-add-haplogroup = Add haplogroup
action-set-name = Set name
action-set-priority = Set priority
action-set-color = Set colour
action-confirm = Confirm
action-reject = Reject

# DNA providers (data-model §7, §12)
dna-provider-ancestry = AncestryDNA
dna-provider-23andme = 23andMe
dna-provider-myheritage = MyHeritage
dna-provider-ftdna = FamilyTreeDNA
dna-provider-gedmatch = GEDmatch
dna-provider-livingdna = Living DNA

# DNA test types
dna-test-type-autosomal = Autosomal
dna-test-type-ydna = Y-DNA
dna-test-type-mtdna = mtDNA
dna-test-type-xdna = X-DNA

# DNA genome builds
dna-genome-build-37 = GRCh37
dna-genome-build-38 = GRCh38

# Chromosome side (segment phasing)
chromosome-side-maternal = maternal
chromosome-side-paternal = paternal
chromosome-side-unknown = unphased

# DNA match status
match-status-confirmed = Confirmed
match-status-rejected = Rejected
match-status-undecided = Undecided

# In-app help articles (Phase 5). Spaces at inline-run boundaries use the { " " }
# literal so Fluent's whitespace trimming keeps them.
help-section-overview = Overview
help-section-use-case = Guides
help-section-reference = Reference
help-topic-why-this-app = Why this app
help-label-most = Most tools
help-label-ours = This app

# Help · "Why this app" article (mirrors docs/phase5/strengths.html)
help-why-lede-1 = Most genealogy tools store{ " " }
help-why-lede-conclusions = conclusions
help-why-lede-2 = { " " }and quietly overwrite them. This one stores the{ " " }
help-why-lede-evidence = evidence and the reasoning
help-why-lede-3 = { " " }as an append-only event log, then derives the family tree from it. The result: you can always see who claimed what, on what basis, and undo any of it. Below, each differentiator is shown with the actual component you’ll meet in the app.

help-why-h-audit = Complete audit trail
help-why-audit-most = “Last modified” date, maybe. Who and why are lost.
help-why-audit-ours-1 = Every change is an immutable event:{ " " }
help-why-audit-ours-bold = who · when · why
help-why-audit-ours-2 = , and reversible.
help-why-spec-timeline = History timeline — straight from the event log

help-why-h-evidence = Evidence-first — confidence everywhere
help-why-evidence-most = A fact is just a value in a box. Sourced or not, it looks the same.
help-why-evidence-ours-1 = Every fact carries a surety level and flags when it has{ " " }
help-why-evidence-ours-bold = no source
help-why-evidence-ours-2 = .
help-why-spec-facts = Vital facts — surety + source on every row

help-why-h-citations = Research-grade citations
help-why-citations-most = A free-text “source” field. No analysis of how good it is.
help-why-citations-ours-1 = Each citation is graded on the three{ " " }
help-why-citations-ours-italic = Evidence Explained
help-why-citations-ours-2 = { " " }axes.
help-why-spec-evidence = Evidence-analysis axes — source · information · evidence

help-why-h-merge = Non-destructive, reversible merge
help-why-merge-most = Merge deletes one record. A mistake means re-typing from memory.
help-why-merge-ours = Merge is an event — field-by-field, with a survivor, and undoable.

help-why-h-provenance = Provenance — “why we believe this”
help-why-provenance-most = A date is just there. You can’t see what it rests on.
help-why-provenance-ours = One click shows every claim behind a conclusion, graded and attributed.

help-why-h-plugins = Sandboxed plugins & full localization
help-why-plugins-most = Plugins run with full access; UI is English-first, add-ons rarely translate.
help-why-plugins-ours-1 = Plugins are deny-by-default WASM;{ " " }
help-why-plugins-ours-bold = all
help-why-plugins-ours-2 = { " " }UI — including plugin UI — is localized.

help-why-h-glance = At a glance
help-why-tbl-h-capability = Capability
help-why-tbl-h-this = This app
help-why-tbl-h-typical = Typical tool
help-why-tbl-r1-cap = Who/when/why on every change
help-why-tbl-r1-this = Built-in, immutable
help-why-tbl-r1-typ = Last-modified date, if any
help-why-tbl-r2-cap = Confidence per fact
help-why-tbl-r2-this = 5-level surety
help-why-tbl-r2-typ = Not modeled
help-why-tbl-r3-cap = Citation analysis
help-why-tbl-r3-this = 3 evidence axes
help-why-tbl-r3-typ = Free-text field
help-why-tbl-r4-cap = Reversible merge
help-why-tbl-r4-this = Event-based, undoable
help-why-tbl-r4-typ = Destructive
help-why-tbl-r5-cap = Provenance view
help-why-tbl-r5-this = Per-conclusion
help-why-tbl-r5-typ = None
help-why-tbl-r6-cap = Plugin sandbox
help-why-tbl-r6-this = WASM, deny-by-default
help-why-tbl-r6-typ = Full access / none
help-why-tbl-r7-cap = Localization of plugin UI
help-why-tbl-r7-this = Same Fluent pipeline
help-why-tbl-r7-typ = English-only

# Help · topic titles for the recording guides and glossary
help-topic-recording = Recording your research
help-topic-record-person = Recording a person
help-topic-record-family = Recording a family
help-topic-record-census = Recording a census
help-topic-record-burial = Recording a burial entry
help-topic-personal-knowledge = Sourcing personal knowledge
help-topic-glossary = Glossary

# Help · "Recording your research" contents page
help-rec-lede = This app records research, not just answers. You enter what a record says as a sourced claim; the family tree is derived from those claims. These guides walk through the common cases — describing the records you create and how each claim is backed by evidence, not which buttons to press.
help-rec-h-guides = Guides
help-rec-link-person = Recording a person
help-rec-desc-person = { " " }— one individual: names and vital dates, each backed by a citation.
help-rec-link-family = Recording a family
help-rec-desc-family = { " " }— partners, a marriage event, and children.
help-rec-link-census = Recording a census
help-rec-desc-census = { " " }— a household at a farmstead on a date, from a scanned and transcribed source.
help-rec-link-burial = Recording a burial entry
help-rec-desc-burial = { " " }— a single line of a church book recording a death and burial.
help-rec-link-personal = Sourcing personal knowledge
help-rec-desc-personal = { " " }— citing yourself for what you know first-hand, and how that differs from a witness.
help-rec-h-reference = Reference
help-rec-link-glossary = Glossary
help-rec-desc-glossary = { " " }— what assertion, fact, event, citation, association, and family mean here.

# Help · "Recording a person"
help-person-lede = Start with the person, then add what you know as separate, sourced claims — a name, a birth, and (when it applies) a death.
help-person-h-record = Create the person and name
help-person-p-record = Add a new Person record, then assert a name (given and surname) on it. The name is itself a claim, so it carries a confidence and can be cited — useful when spellings vary between records.
help-person-h-vitals = Birth and death
help-person-p-vitals = Record birth as an Event with its date and Place, and link the person to it; do the same for death when you know it. Dates and places are properties of the event, shared by everyone who took part — not free-text fields on the person.
help-person-h-source = Back every claim
help-person-p-source = Each claim is backed by a Citation to a Source. For something you know first-hand, the source is your own knowledge — record it as such, an original/primary source, and set the confidence accordingly. A claim with no source is flagged rather than hidden, so gaps stay visible.
help-person-spec = Vital facts — surety and source on every row

# Help · "Recording a family"
help-family-lede = A family ties partners together and to their children. The marriage and each relationship are claims you record and source.
help-family-h-record = Create the family and partners
help-family-p-record = Add a new Family record and add the partners to it by reference to their Person records. Partner roles are neutral — the family is the union, not a fixed husband/wife pair.
help-family-h-marriage = The marriage event
help-family-p-marriage = Record the marriage as an Event with its date and Place, and link it to the family. The date and place live on the event; the family points at it.
help-family-h-children = Children
help-family-p-children = Add each child to the family by reference to their Person record. Each child link is a claim in its own right, so it can carry a confidence and be cited from the record that establishes the relationship.
help-family-p-source = As with a person, every link and date is backed by a Citation, so the whole family can be traced to the evidence behind it.

# Help · "Recording a census"
help-census-lede = A census entry places a household at a farmstead on a date and lists who was there, their relationships, ages, and occupations. The archive gives you two faces of one source — a scanned original and a transcription that is not always correct.
help-census-h-source = One source, two faces
help-census-p-source = Create one Source for the census record and a Citation pointing at the exact entry (roll, page, household). The scan is the original; the transcription is a derivative of it. Cite what you actually read, and note where the transcription and the scan disagree.
help-census-h-place = The farmstead and the event
help-census-p-place = Record the farmstead as a Place and the enumeration as a census Event at that place and date. Everyone listed participates in the same event.
help-census-h-people = The household
help-census-p-people = Add a Person for each individual (or link to an existing one), with their role in the household. A stated age yields an inferred birth-date claim — indirect evidence — so record it at a lower confidence than a date the record states outright.
help-census-h-evidence = Reading the evidence
help-census-p-evidence = Grade each citation on the three Evidence Explained axes: original (the scan) versus derivative (the transcription); primary versus secondary information; and direct versus indirect evidence — an age answers a birth date only by inference.
help-census-spec = Evidence-analysis axes — source · information · evidence

# Help · "Recording a burial entry"
help-burial-lede = A burial entry is a single line of a church book. It states a death and a burial, and sometimes a date of birth — each a claim you record from that one line.
help-burial-h-source = The church book and the line
help-burial-p-source = Create a Source for the church book and a Citation that pins the exact page and line. The precise locator is what lets anyone return to the same entry and check your reading.
help-burial-h-event = Death and burial events
help-burial-p-event = Record the death as an Event and the burial as an Event with its date and Place, and link the person to both. The burial place is a property of the burial event.
help-burial-h-evidence = What the entry proves
help-burial-p-evidence = The entry is original, primary, and direct evidence for the death and burial it was written to record. For a date of birth it is derivative and secondary — set that claim at a lower confidence and look for a baptism or birth record to confirm it.
help-burial-spec = Why we believe — every claim graded and attributed

# Help · "Sourcing personal knowledge"
help-pk-lede = Some of what you know has no document behind it — you were there, or it is your own family. You can still record it properly. "You" enters the record two ways: as the operator who made the claim, and as the source the claim rests on.
help-pk-h-operator = You are always the operator
help-pk-p-operator = Every claim you make is stamped with you as its operator — who asserted it, when, and why — automatically, from your configured identity. That audit trail is separate from the source, and you never create it by hand.
help-pk-h-source = Cite yourself as a source
help-pk-p-source = To back a claim with your own knowledge, create a Source such as "Personal knowledge of <your name>" with yourself as author, then a Citation into it. Grade the citation: an original source (your own knowledge, not a copy), and direct evidence (it answers the question outright). No repository — it is not an archived document.
help-pk-spec = Evidence-analysis axes — grade personal knowledge like any source
help-pk-h-firsthand = First-hand or hearsay?
help-pk-p-firsthand = If you witnessed it or it is about you, the information is primary — set a high confidence. If family told you, it is secondary (hearsay or tradition) — set a lower confidence and record who told you in the source. Either way the claim is visible and gradable, not an unmarked guess.
help-pk-h-witness = A witness is different
help-pk-p-witness = Being a source is not the same as being a witness. Witness is a role you take on an Event you were present at — add yourself as a participant with the witness role (or as a witness association between people). Use the personal-knowledge source for what you know; use the witness role only when you were actually there.

# Help · Glossary
help-gloss-lede = A few words mean something specific here. The core idea: the event log is the evidence layer (claims), and the records you browse are the conclusion layer (the current best synthesis), derived from those claims.
help-gloss-h-layers = Claims and conclusions
help-gloss-p-layers = You never edit a record directly. You make claims; the app derives the record. A correction is a new claim that supersedes an old one, so the reasoning is kept, not overwritten.
help-gloss-tbl-h-term = Term
help-gloss-tbl-h-meaning = What it means here
help-gloss-assertion-term = Assertion
help-gloss-assertion-def = One sourced claim made by an operator — an entry in the event log, the evidence layer. Carries a confidence and, usually, citations.
help-gloss-fact-term = Fact
help-gloss-fact-def = A property shown on a record (a birth date, an occupation) — a conclusion the app derives from one or more assertions.
help-gloss-event-term = Event
help-gloss-event-def = A dated, placed happening that people take part in — birth, marriage, census, burial. A record in its own right, not a field on a person.
help-gloss-citation-term = Citation
help-gloss-citation-def = A reference into a Source (with page or locator, confidence, and evidence analysis). It is what backs an assertion.
help-gloss-h-relations = Events, associations, and families
help-gloss-p-relations = Three ways people connect. Use the one that matches the kind of link you are recording.
help-gloss-rel-event-term = Event
help-gloss-rel-event-def = Ties people to a moment in time and place — a shared happening they each participated in.
help-gloss-association-term = Association
help-gloss-association-def = A direct person-to-person relationship that is not a family bond — godparent, witness, neighbour — carrying a role.
help-gloss-family-term = Family
help-gloss-family-def = The record linking partners and their children. Use it for the household; use an association for other person-to-person links.

# Pedigree tool (PR 18) — ancestor/descendant charts + the kinship calculator
pedigree-unknown-father-of = father of { $name }
pedigree-unknown-mother-of = mother of { $name }
pedigree-father-unresearched = father (line unresearched)
pedigree-mother-unresearched = mother (line unresearched)
pedigree-focus = Focus: { $name } · { $generations } generations
kinship-not-found = No known relationship found within the searched generations.
kinship-same = { $a } — the same person.
kinship-a-is-b-term = { $a } is { $b }’s { $term }.
kinship-a-and-b-are = { $a } and { $b } are { $term }.
kinship-full-sibling = full siblings
kinship-half-sibling = half siblings
kinship-parent = parent
kinship-grandparent = grandparent
kinship-great-grandparent = great-grandparent
kinship-great-n-grandparent = { $n }× great-grandparent
kinship-child = child
kinship-grandchild = grandchild
kinship-great-grandchild = great-grandchild
kinship-great-n-grandchild = { $n }× great-grandchild
kinship-aunt-uncle = aunt/uncle
kinship-great-aunt-uncle = great-aunt/uncle
kinship-great-n-aunt-uncle = { $n }× great-aunt/uncle
kinship-niece-nephew = niece/nephew
kinship-great-niece-nephew = great-niece/nephew
kinship-great-n-niece-nephew = { $n }× great-niece/nephew
kinship-cousins-removed = { $cousins }, { $removed }
cousin-first = first cousins
cousin-second = second cousins
cousin-third = third cousins
cousin-nth = { $n }th cousins
removed-once = once removed
removed-twice = twice removed
removed-n-times = { $n }× removed

## Compare / merge (Phase 5 PR 19) — the duplicates table and the compare/merge wizard.
duplicate-reason-name-variant = name variant
duplicate-reason-same-birth-year = identical name · birth years close
merge-field-name = Name
merge-field-birth = Birth
merge-field-death = Death
merge-field-occupation = Occupation
merge-result-summary = { $merged } becomes a persona of { $survivor }; one event added to History.
merge-result-summary-with-references = { $merged } becomes a persona of { $survivor }; { $count } other record(s) still reference { $merged }; one event added to History.
