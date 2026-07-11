# Data-model class diagrams

- **Status:** Generated from code — regenerate when aggregate state changes
- **Date:** 2026-07-10
- **Source of truth:** `crates/genealogy-core/src/<aggregate>/state.rs` (+ `view.rs`); see
  [data-model.md](data-model.md) for the narrative model.

Class diagrams of the twelve aggregates (conclusion-layer shape) and the value objects they embed.
Each aggregate's `*View` wraps its `*State` and exposes the same shape, so one class per aggregate
is drawn, named after the aggregate.

## Legend

| Notation | Meaning |
| --- | --- |
| `T?` | `Option<T>` |
| `T[*]` | `Vec<T>` |
| `Set~T~` | `BTreeSet<T>` |
| `Asserted~T~` | row carries denormalized provenance: `{ value: T, confidence, citations }` |
| solid arrow `-->` | cross-aggregate reference — an **id link carried in event payloads**, never an object reference |
| `*--` | composition — value object embedded in the owning payload |
| `«enumeration»` | an enum; the lines below it are its variants (plain text instead of Mermaid's `<<…>>` annotation, which some Markdown previews mangle as HTML) |

Conventions:

- The `Attributed<T>` wrapper (`{ assertion_id, value }` — tags every asserted row with the
  assertion that introduced it, so retract/supersede can remove exactly that row) is **elided on
  every field**; see the provenance substrate diagram for its shape.
- `Asserted~T~` (and the bespoke `AssertedName` / `AssertedFact` / `AssertedPartner` / … structs)
  are shown, because whether a row denormalizes confidence + citations differs per field.
- Bookkeeping fields are omitted: `exists`, `live_assertions`, the aggregate's own id, and
  `restrictions_assertion` (Person only).
- Attachment links every aggregate carries (notes / tags / media / citations) are listed as fields
  and summarized in the [attachment matrix](#attachment-matrix), but not drawn as overview edges —
  they would dominate the picture.

## Overview — aggregates and cross-aggregate links

```mermaid
classDiagram
    direction LR

    class Person
    class Family
    class Event
    class Place
    class Source
    class Citation
    class Repository
    class Media
    class Note
    class Tag
    class DnaTest
    class DnaMatch

    Person --> Event : participations role+age
    Person --> Person : associations
    Person --> Person : merged personas
    Person --> Place : facts place_id
    Family --> Person : partners
    Family --> Person : children
    Family --> Event : linked_events
    Event --> Place : place_id
    Place --> Place : enclosed_by dated
    Citation --> Source : source_id
    Source --> Repository : repositories RepoRef
    Media --> Citation : citations
    DnaTest --> Person : person_id
    DnaMatch --> DnaTest : test_a
    DnaMatch --> DnaTest : test_b
    DnaMatch --> Person : shared_ancestors
```

Participation is owned by a single aggregate: the Person asserts
`participations (event_id, role, …)` (ADR 0019). An Event's participant list is a **projection**
over the person-side rows that reference it — the Event aggregate holds no participation state, so
there is one owner and one correction handle (the person-side `AssertionId`).

### Attachment matrix

Which aggregate carries which attachment/link lists (all `Vec<Attributed<…>>` unless noted):

| Aggregate | citations | media | notes | tags | external_ids | addresses | attributes | restrictions |
| --- | :-: | :-: | :-: | :-: | :-: | :-: | :-: | :-: |
| Person | ✓ | ✓ | ✓ | ✓ | ✓ | — | — | ✓ |
| Family | ✓ | ✓ | ✓ | ✓ | ✓ | — | — | ✓ |
| Event | ✓ | ✓ | ✓ | ✓ | — | ✓ | — | ✓ |
| Place | ✓ | ✓ | ✓ | ✓ | — | — | — | ✓ |
| Source | — | ✓ | ✓ | ✓ | — | — | ✓ | ✓ |
| Citation | — | ✓ | ✓ | ✓ | — | — | ✓ | ✓ |
| Repository | — | — | ✓ | ✓ | — | ✓ (+urls) | — | ✓ |
| Media | ✓ | — | ✓ | ✓ | — | — | ✓ | ✓ |
| Note | — | — | — | ✓ | — | — | — | ✓ |
| Tag | — | — | — | — | — | — | — | ✓ |
| DnaTest | — | — | ✓ | ✓ | — | — | — | ✓ |
| DnaMatch | — | — | ✓ | ✓ | — | — | — | ✓ |

A Source has no `citations` list because a Source is what citations point *into*.
Tag has no assertion chain at all (no `Attributed` rows, no `live_assertions`) — its fields are
last-writer-wins setters.

## Provenance substrate

The wrappers elided in the detail diagrams, and the envelope every stored event embeds
(`provenance.rs`, `assertions.rs`):

```mermaid
classDiagram
    direction LR

    class Attributed~T~ {
        +assertion_id AssertionId
        +value T
    }
    class Asserted~T~ {
        +value T
        +confidence Confidence
        +citations CitationId[*]
    }
    class EventContext {
        +operator Agent
        +occurred_at Timestamp
        +rationale String?
        +confidence Confidence
        +citations CitationRef[*]
        +evidence_analysis EvidenceAnalysis?
    }
    class Agent {
        +kind AgentKind
        +id AgentId
        +display String?
    }
    class AgentKind {
        «enumeration»
        Human
        Software name version
        AiModel name version
    }
    class Confidence {
        «enumeration»
        VeryLow
        Low
        Normal
        High
        VeryHigh
    }
    class EvidenceAnalysis {
        +source SourceQuality
        +information InformationKind
        +evidence EvidenceKind
    }
    class CitationRef {
        +citation_id CitationId
    }

    EventContext *-- Agent
    Agent *-- AgentKind
    EventContext *-- CitationRef
    EventContext *-- EvidenceAnalysis
    EventContext *-- Confidence
    Asserted~T~ *-- Confidence
```

`Asserted<T>` is not stored independently: it is **denormalized from the asserting event's
`EventContext` at fold time** (confidence + citation ids), so read models can show per-row surety
without re-reading the log. The Person and Family aggregates use bespoke equivalents
(`AssertedName`, `AssertedFact`, `AssertedAssociation`, `AssertedPartner`, `AssertedChild`,
`AssertedFamilyEvent`) instead of the generic `Asserted<T>`.

## Person & Family

```mermaid
classDiagram
    direction LR

    class Person {
        +human_id HumanId?
        +evidence_level EvidenceLevel?
        +sex Sex?
        +names AssertedName[*]
        +facts AssertedFact[*]
        +associations AssertedAssociation[*]
        +participations Participation[*]
        +citations CitationId[*]
        +media MediaRef[*]
        +notes NoteId[*]
        +tags TagId[*]
        +external_ids ExternalId[*]
        +merged PersonId[*]
        +restrictions Set~Restriction~
    }
    class AssertedName {
        +name PersonName
        +confidence Confidence
        +citations CitationId[*]
    }
    class PersonName {
        +name_type NameType
        +given String?
        +surnames Surname[*]
        +suffix String?
        +title String?
        +nickname String?
        +call_name String?
        +date GenealogicalDate?
        +language LanguageTag?
        +transliterations PersonName[*]
    }
    class Surname {
        +prefix String?
        +surname String
        +primary bool
        +connector String?
    }
    class AssertedFact {
        +fact Fact
        +confidence Confidence
        +citations CitationId[*]
    }
    class Fact {
        +fact_type FactType
        +date GenealogicalDate?
        +place_id PlaceId?
        +value String?
    }
    class AssertedAssociation {
        +association Association
        +confidence Confidence
        +citations CitationId[*]
    }
    class Association {
        +other PersonId
        +role AssociationRole
    }
    class Participation {
        +event_id EventId
        +role ParticipantRole
        +age Age?
        +attributes Attribute[*]
        +notes NoteId[*]
    }
    class ExternalId {
        +authority String
        +value String
        +kind String?
        +url String?
    }

    class Family {
        +human_id HumanId?
        +partners AssertedPartner[*]
        +children AssertedChild[*]
        +child_relationships Asserted~ChildRelationship~[*]
        +linked_events AssertedFamilyEvent[*]
        +citations CitationId[*]
        +media MediaRef[*]
        +notes NoteId[*]
        +tags TagId[*]
        +external_ids ExternalId[*]
        +restrictions Set~Restriction~
    }
    class AssertedPartner {
        +person_id PersonId
        +confidence Confidence
        +citations CitationId[*]
    }
    class AssertedChild {
        +child_id PersonId
        +confidence Confidence
        +citations CitationId[*]
    }
    class ChildRelationship {
        +child_id PersonId
        +parent_id PersonId
        +relationship ChildParentRelationship
    }
    class AssertedFamilyEvent {
        +event_id EventId
        +confidence Confidence
        +citations CitationId[*]
    }

    Person *-- AssertedName
    AssertedName *-- PersonName
    PersonName *-- Surname
    Person *-- AssertedFact
    AssertedFact *-- Fact
    Person *-- AssertedAssociation
    AssertedAssociation *-- Association
    Person *-- Participation
    Participation *-- Age
    Age *-- AgeBound
    Person *-- ExternalId
    Family *-- AssertedPartner
    Family *-- AssertedChild
    Family *-- ChildRelationship
    Family *-- AssertedFamilyEvent

    Association --> Person : other
    Participation --> Event : event_id
    Fact --> Place : place_id
    AssertedPartner --> Person : person_id
    AssertedChild --> Person : child_id
    ChildRelationship --> Person : child_id / parent_id
    AssertedFamilyEvent --> Event : event_id
```

`AssertedChild` is the child's **membership** only; each child-to-partner relationship is a separate
`ChildRelationship` row (`child_id`, `parent_id`, `ChildParentRelationship` — GEDCOM `_FREL`/`_MREL`),
so an adoption link can be retracted or re-cited without disturbing the membership or the other links
(ADR 0021). `FamilyView::children()` reconstructs the per-partner tuple list by folding the
`ChildRelationship` rows onto the membership. `ChildRelationship` rides the shared `Asserted~T~`
wrapper (`{ value, confidence, citations }`) like every other denormalized row.

## Event & Place

```mermaid
classDiagram
    direction LR

    class Event {
        +human_id HumanId?
        +event_type Asserted~EventType~?
        +date Asserted~GenealogicalDate~?
        +description Asserted~String~?
        +place_id Asserted~PlaceId~?
        +addresses Address[*]
        +citations CitationId[*]
        +media MediaRef[*]
        +notes NoteId[*]
        +tags TagId[*]
        +restrictions Set~Restriction~
    }
    class Place {
        +human_id HumanId?
        +place_type Asserted~PlaceType~?
        +names Asserted~PlaceName~[*]
        +enclosed_by Asserted~PlaceRef~[*]
        +coordinates Asserted~GeoCoordinates~?
        +code Asserted~String~?
        +citations CitationId[*]
        +media MediaRef[*]
        +notes NoteId[*]
        +tags TagId[*]
        +restrictions Set~Restriction~
    }
    class PlaceName {
        +text String
        +language LanguageTag?
        +date GenealogicalDate?
    }
    class PlaceRef {
        +place_id PlaceId
        +date GenealogicalDate?
    }
    class GeoCoordinates {
        +latitude Microdegrees
        +longitude Microdegrees
    }
    class Address {
        +lines String[*]
        +locality String?
        +region String?
        +postal_code String?
        +country String?
        +phone String?
        +email String?
        +fax String?
        +www String?
        +original_text String?
    }

    Event *-- Address
    Place *-- PlaceName
    Place *-- PlaceRef
    Place *-- GeoCoordinates

    Event --> Place : place_id
    PlaceRef --> Place : enclosed_by
```

### GenealogicalDate

Used by Person names/facts, Event, Place names/refs, Citation, and Media.

```mermaid
classDiagram
    direction LR

    class GenealogicalDate {
        +calendar Calendar
        +quality DateQuality
        +modifier GenealogicalDateBody
        +time TimeOfDay?
        +new_year_begins u8?
        +sort_value i64
        +original_text String?
    }
    class GenealogicalDateBody {
        «enumeration»
        Structured DateModifier
        TextOnly text
    }
    class DateModifier {
        «enumeration»
        None DatePoint
        Before DatePoint
        After DatePoint
        About DatePoint
        Range start end
        Span start end
        From DatePoint
        To DatePoint
        Interpreted date phrase
    }
    class DatePoint {
        +year i32?
        +month u8?
        +day u8?
    }
    class TimeOfDay {
        +hour u8
        +minute u8
        +second u8?
    }
    class Calendar {
        «enumeration»
        Gregorian
        Julian
        Hebrew
        FrenchRepublican
        Islamic
        Swedish
    }
    class DateQuality {
        «enumeration»
        Normal
        Estimated
        Calculated
    }

    GenealogicalDate *-- GenealogicalDateBody
    GenealogicalDateBody *-- DateModifier
    DateModifier *-- DatePoint
    GenealogicalDate *-- TimeOfDay
    GenealogicalDate *-- Calendar
    GenealogicalDate *-- DateQuality
```

## Evidence: Source, Citation, Repository, Media, Note, Tag

```mermaid
classDiagram
    direction LR

    class Source {
        +human_id HumanId?
        +title String?
        +author String?
        +pub_info String?
        +abbrev String?
        +repositories Asserted~RepoRef~[*]
        +attributes Attribute[*]
        +media MediaRef[*]
        +notes NoteId[*]
        +tags TagId[*]
        +restrictions Set~Restriction~
    }
    class Citation {
        +human_id HumanId?
        +source_id SourceId?
        +created_by String?
        +created_at Timestamp?
        +page String?
        +date GenealogicalDate?
        +confidence Confidence?
        +evidence_analysis EvidenceAnalysis?
        +attributes Attribute[*]
        +media MediaRef[*]
        +notes NoteId[*]
        +tags TagId[*]
        +restrictions Set~Restriction~
    }
    class Repository {
        +human_id HumanId?
        +repository_type RepositoryType?
        +name String?
        +addresses Address[*]
        +urls Url[*]
        +notes NoteId[*]
        +tags TagId[*]
        +restrictions Set~Restriction~
    }
    class Media {
        +human_id HumanId?
        +path MediaPath?
        +checksum String?
        +mime String?
        +date GenealogicalDate?
        +attributes Attribute[*]
        +citations CitationId[*]
        +notes NoteId[*]
        +tags TagId[*]
        +restrictions Set~Restriction~
    }
    class Note {
        +human_id HumanId?
        +note_type NoteType?
        +text RichText?
        +tags TagId[*]
        +restrictions Set~Restriction~
    }
    class Tag {
        +name String?
        +color String?
        +priority i32?
        +restrictions Set~Restriction~
    }
    class RepoRef {
        +repository_id RepositoryId
        +call_number String?
        +media_type SourceMediaType
    }
    class Attribute {
        +attribute_type String
        +value String
    }
    class Age {
        +bound AgeBound?
        +years u16?
        +months u16?
        +days u16?
        +phrase String?
    }
    class AgeBound {
        «enumeration»
        LessThan
        GreaterThan
    }
    class MediaRef {
        +media_id MediaId
        +crop Rect?
        +caption String?
        +citations CitationRef[*]
    }
    class Rect {
        +left u8
        +top u8
        +width u8
        +height u8
    }
    class MediaPath {
        «enumeration»
        File path
        Web url
    }
    class RichText {
        +text String
        +media_type MediaType
        +language LanguageTag?
        +translator String?
        +translations RichText[*]
    }
    class Url {
        +url_type String?
        +href String
        +description String?
    }

    Source *-- Attribute
    Citation *-- Attribute
    Source *-- MediaRef
    Media *-- MediaPath
    Note *-- RichText
    Repository *-- Address
    Repository *-- Url
    MediaRef *-- Rect

    Citation --> Source : source_id
    RepoRef --> Repository : repository_id
    Source *-- RepoRef
    MediaRef --> Media : media_id
    Media --> Citation : citations
```

`Address` appears here without fields — its shape is drawn once, in the Event & Place diagram.

## DNA

```mermaid
classDiagram
    direction LR

    class DnaTest {
        +human_id HumanId?
        +person_id PersonId?
        +provider Asserted~DnaProvider~?
        +kit_id Asserted~String~?
        +test_type Asserted~DnaTestType~?
        +genome_build Asserted~DnaGenomeBuild~?
        +haplogroups Asserted~String~[*]
        +notes NoteId[*]
        +tags TagId[*]
        +restrictions Set~Restriction~
    }
    class DnaMatch {
        +human_id HumanId?
        +test_a DnaTestId?
        +test_b DnaTestId?
        +provider DnaProvider?
        +shared_cm Centimorgans?
        +percent_shared PercentShared?
        +segment_count u32?
        +largest_segment_cm Centimorgans?
        +predicted_relationship String?
        +segments DnaSegment[*]
        +shared_ancestors SharedAncestor[*]
        +status MatchStatus?
        +notes NoteId[*]
        +tags TagId[*]
        +restrictions Set~Restriction~
    }
    class DnaSegment {
        +chromosome String
        +start u64
        +end u64
        +centimorgans Centimorgans
        +snps u32?
        +side ChromosomeSide
    }
    class SharedAncestor {
        +ancestor_person_id PersonId?
        +note String?
    }
    class MatchStatus {
        «enumeration»
        Confirmed
        Rejected
    }
    class ChromosomeSide {
        «enumeration»
        Maternal
        Paternal
        Unknown
    }

    DnaMatch *-- DnaSegment
    DnaMatch *-- SharedAncestor
    DnaMatch *-- MatchStatus
    DnaSegment *-- ChromosomeSide

    DnaTest --> Person : person_id
    DnaMatch --> DnaTest : test_a
    DnaMatch --> DnaTest : test_b
    SharedAncestor --> Person : ancestor_person_id
```

`Centimorgans` and `PercentShared` are fixed-decimal integer newtypes (`i64`), not floats, so
match observations compare exactly and round-trip losslessly.

## Enumerated types

Closed enums with a `Custom(String)` escape hatch unless noted (data-model §7).

| Enum | Variants |
| --- | --- |
| `Sex` | Male, Female, Unknown, Intersex, Other(String) |
| `Restriction` (closed) | Confidential, Locked, Privacy |
| `EvidenceLevel` (closed) | Persona, Conclusion |
| `NameType` | BirthName, MarriedName, Maiden, Immigrant, Professional, AlsoKnownAs, ReligiousName, Custom |
| `FactType` | Birth, Death, Baptism, Burial, Occupation, Residence, Religion, Caste, PhysicalDescription, Education, Ethnicity, NationalId, Nationality, NumberOfChildren, NumberOfMarriages, Property, SocialSecurityNumber, NobilityTitle, Custom |
| `EventType` | Birth, Death, Marriage, Baptism, Christening, Burial, Cremation, Census, Residence, Immigration, Emigration, Adoption, Confirmation, BarMitzvah, BasMitzvah, FirstCommunion, Graduation, Naturalization, Ordination, Probate, Retirement, Will, Engagement, Annulment, Divorce, DivorceFiled, MarriageBanns, MarriageContract, MarriageLicense, MarriageSettlement, Custom |
| `ParticipantRole` | Primary, Witness, Officiator, Clergy, Father, Mother, Parent, Child, Husband, Wife, Spouse, Godparent, Friend, Neighbour, Multiple, Bride, Groom, Custom |
| `AssociationRole` | Clergy, Friend, Godparent, Neighbour, Officiator, Witness, Child, Father, Mother, Parent, Husband, Wife, Spouse, Multiple, Custom |
| `ChildParentRelationship` | Birth, Adopted, Foster, Step, Sealed, Unknown, Custom |
| `PlaceType` | Country, County, Municipality, Parish, City, Town, Village, Farm, Building, Custom |
| `RepositoryType` | Library, Archive, Church, Cemetery, Museum, Website, Collection, Custom |
| `SourceMediaType` | Book, Card, Electronic, Fiche, Film, Magazine, Manuscript, Map, Newspaper, Photo, Tombstone, Video, Audio, Custom |
| `NoteType` | General, Research, Transcript, Citation, Custom |
| `DnaProvider` | AncestryDna, TwentyThreeAndMe, MyHeritage, FamilyTreeDna, GedMatch, LivingDna, Custom |
| `DnaTestType` (closed) | Autosomal, YDna, MtDna, XDna |
| `DnaGenomeBuild` (closed) | GRCh37, GRCh38 |

## Identifier newtypes (`ids.rs`)

UUID v7 newtypes: `PersonId`, `FamilyId`, `EventId`, `PlaceId`, `SourceId`, `CitationId`,
`RepositoryId`, `MediaId`, `NoteId`, `TagId`, `DnaTestId`, `DnaMatchId`, `AssertionId`, `AgentId`.
`HumanId` is a `String` newtype (the user-facing `gramps_id` analog).
