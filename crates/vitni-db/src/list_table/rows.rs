//! What each listed kind's row holds: its cells, computed from the record's projection and the records
//! it references one step away (ADR 0052 §1), and which references a change must reach (§5).

use std::collections::{BTreeSet, HashMap};

use cqrs_es::Aggregate;
use serde::Serialize;
use serde::de::DeserializeOwned;
use vitni_core::citation::{CitationState, CitationView};
use vitni_core::dna_match::{DnaMatchState, DnaMatchView};
use vitni_core::dna_test::{DnaTestState, DnaTestView};
use vitni_core::enums::{EventType, ParticipantRole};
use vitni_core::event::{EventState, EventView};
use vitni_core::family::{FamilyState, FamilyView};
use vitni_core::media::{MediaState, MediaView};
use vitni_core::media_path::MediaPath;
use vitni_core::name::PersonName;
use vitni_core::note::{NoteState, NoteView};
use vitni_core::person::{PersonState, PersonView};
use vitni_core::place::{PlaceState, PlaceView};
use vitni_core::repository::{RepositoryState, RepositoryView};
use vitni_core::research_note::{ResearchNoteState, ResearchNoteView};
use vitni_core::source::{SourceState, SourceView};
use vitni_core::tag::{TagState, TagView};

use super::{ListCell, ListedKind};
use crate::record_links::RecordLink;
use crate::tables::{
    CITATION_VIEW_TABLE, DNA_MATCH_VIEW_TABLE, DNA_TEST_VIEW_TABLE, EVENT_VIEW_TABLE, FAMILY_VIEW_TABLE,
    MEDIA_VIEW_TABLE, NOTE_VIEW_TABLE, PERSON_VIEW_TABLE, PLACE_VIEW_TABLE, REPOSITORY_VIEW_TABLE,
    RESEARCH_NOTE_VIEW_TABLE, SOURCE_VIEW_TABLE, TAG_VIEW_TABLE,
};

/// Whose rows read another record's values: `(dependent, relation, target)` — a `dependent` row reads
/// the `target` record its `relation` reference names, so a changed `target` recomputes the rows of the
/// records `record_links` lists as its `relation` sources.
pub(crate) const DEPENDENTS: &[(ListedKind, RecordLink, ListedKind)] = &[
    (ListedKind::Person, RecordLink::Participation, ListedKind::Event),
    (ListedKind::Family, RecordLink::FamilyPartner, ListedKind::Person),
    (ListedKind::Family, RecordLink::FamilyEvent, ListedKind::Event),
    (ListedKind::Event, RecordLink::EventPlace, ListedKind::Place),
    (ListedKind::Place, RecordLink::PlaceEnclosure, ListedKind::Place),
    (ListedKind::Citation, RecordLink::CitationSource, ListedKind::Source),
    (ListedKind::DnaTest, RecordLink::DnaTestPerson, ListedKind::Person),
    (ListedKind::DnaMatch, RecordLink::DnaMatchTest, ListedKind::DnaTest),
];

/// The ids of the records a batch of rows reads, by kind.
#[derive(Debug, Default)]
pub(crate) struct Wanted {
    pub(crate) persons: BTreeSet<String>,
    pub(crate) events: BTreeSet<String>,
    pub(crate) places: BTreeSet<String>,
    pub(crate) sources: BTreeSet<String>,
    pub(crate) dna_tests: BTreeSet<String>,
}

/// The records a batch of rows reads, by id.
#[derive(Default)]
pub(crate) struct Refs {
    pub(crate) persons: HashMap<String, PersonView>,
    pub(crate) events: HashMap<String, EventView>,
    pub(crate) places: HashMap<String, PlaceView>,
    pub(crate) sources: HashMap<String, SourceView>,
    pub(crate) dna_tests: HashMap<String, DnaTestView>,
}

/// One computed row: the record's ids and one cell per catalogue column (`None` for the tag column,
/// which the `tagged` links answer).
pub(crate) struct RowValues {
    pub(crate) id: String,
    pub(crate) human_id: Option<String>,
    pub(crate) cells: Vec<Option<ListCell>>,
}

/// A projection with a list table: its aggregate, its kind, and how its row is computed.
pub(crate) trait ListedRecord: DeserializeOwned + Send + Sync + 'static {
    /// The aggregate the projection folds.
    type State: Aggregate;

    /// The kind the row is listed as.
    const KIND: ListedKind;

    /// The projection table the view is read from.
    const VIEW_TABLE: &'static str;

    /// The record's aggregate id, once created.
    fn list_id(&self) -> Option<String>;

    /// The record's `human_id`, for a kind that has one.
    fn list_human_id(&self) -> Option<String>;

    /// Adds the records the row reads to `wanted`.
    fn wants(&self, _wanted: &mut Wanted) {}

    /// The row's cells, one per catalogue column, reading referenced records from `refs`.
    fn cells(&self, refs: &Refs) -> Vec<Option<ListCell>>;

    /// The computed row, once the record exists.
    fn row(&self, refs: &Refs) -> Option<RowValues> {
        Some(RowValues {
            id: self.list_id()?,
            human_id: self.list_human_id(),
            cells: self.cells(refs),
        })
    }
}

/// A closed enum value as its kebab-case variant name: `Female` → `female`, `VeryHigh` → `very-high`.
/// Read from the serde form, so a unit variant and an adjacently tagged one (`{"type": "Female"}`) name
/// the same; an escape variant names its tag (`custom`), not its free text.
pub(crate) fn choice_key<T: Serialize>(value: &T) -> Option<String> {
    let name = match serde_json::to_value(value).ok()? {
        serde_json::Value::String(name) => name,
        serde_json::Value::Object(object) => object.get("type")?.as_str()?.to_owned(),
        _ => return None,
    };
    Some(kebab(&name))
}

/// `PascalCase` → `kebab-case`.
pub(crate) fn kebab(name: &str) -> String {
    let mut kebab = String::with_capacity(name.len() + 4);
    for (index, character) in name.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            kebab.push('-');
        }
        kebab.extend(character.to_lowercase());
    }
    kebab
}

fn text(value: Option<&str>) -> Option<ListCell> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| ListCell::Text(value.to_owned()))
}

fn choice<T: Serialize>(value: Option<&T>) -> Option<ListCell> {
    value.and_then(choice_key).map(ListCell::Choice)
}

fn record(id: Option<String>, title: Option<String>) -> Option<ListCell> {
    id.map(|id| ListCell::Record { id, title })
}

/// A person as another record's row names it: its primary name.
fn person_title(person: &PersonView) -> Option<String> {
    person.names().first().map(|name| name.display())
}

/// A place as another record's row names it: its primary name.
fn place_title(place: &PlaceView) -> Option<String> {
    place.names().first().map(|name| name.text.clone())
}

/// A DNA test as a match's row names it: its kit id, else its `human_id`.
fn dna_test_title(test: &DnaTestView) -> Option<String> {
    test.kit_id()
        .map(str::to_owned)
        .or_else(|| test.human_id().map(|id| id.as_str().to_owned()))
}

/// The date of the `wanted` event the person is the primary participant of (ADR 0021 §2).
fn vital_date(person: &PersonView, refs: &Refs, wanted: &EventType) -> Option<ListCell> {
    person.participations().into_iter().find_map(|participation| {
        if participation.role != ParticipantRole::Primary {
            return None;
        }
        let event = refs.events.get(&participation.event_id.to_string())?;
        (event.event_type() == Some(wanted))
            .then(|| event.date().cloned().map(ListCell::Date))
            .flatten()
    })
}

impl ListedRecord for PersonView {
    type State = PersonState;

    const KIND: ListedKind = ListedKind::Person;
    const VIEW_TABLE: &'static str = PERSON_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.person_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn wants(&self, wanted: &mut Wanted) {
        for participation in self.participations() {
            if participation.role == ParticipantRole::Primary {
                wanted.events.insert(participation.event_id.to_string());
            }
        }
    }

    fn cells(&self, refs: &Refs) -> Vec<Option<ListCell>> {
        let primary = self.names().first().copied();
        let surname = primary.and_then(|name| name.surnames.first());
        vec![
            text(primary.map(PersonName::display).as_deref()),
            text(primary.and_then(|name| name.given.as_deref())),
            text(surname.map(|surname| surname.surname.as_str())),
            choice(self.sex()),
            vital_date(self, refs, &EventType::Birth),
            vital_date(self, refs, &EventType::Death),
            None,
        ]
    }
}

impl ListedRecord for FamilyView {
    type State = FamilyState;

    const KIND: ListedKind = ListedKind::Family;
    const VIEW_TABLE: &'static str = FAMILY_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.family_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn wants(&self, wanted: &mut Wanted) {
        for partner in self.partners() {
            wanted.persons.insert(partner.to_string());
        }
        for event in self.linked_events() {
            wanted.events.insert(event.to_string());
        }
    }

    fn cells(&self, refs: &Refs) -> Vec<Option<ListCell>> {
        let mut partners = Vec::new();
        for partner in self.partners() {
            if let Some(name) = refs.persons.get(&partner.to_string()).and_then(person_title) {
                partners.push(name);
            }
        }
        let marriage = self.linked_events().into_iter().find_map(|event| {
            let event = refs.events.get(&event.to_string())?;
            (event.event_type() == Some(&EventType::Marriage))
                .then(|| event.date().cloned().map(ListCell::Date))
                .flatten()
        });
        let children = u32::try_from(self.children().len()).unwrap_or(u32::MAX);
        vec![
            text(Some(&partners.join(" & "))),
            marriage,
            Some(ListCell::Number(f64::from(children))),
            None,
        ]
    }
}

impl ListedRecord for EventView {
    type State = EventState;

    const KIND: ListedKind = ListedKind::Event;
    const VIEW_TABLE: &'static str = EVENT_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.event_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn wants(&self, wanted: &mut Wanted) {
        if let Some(place) = self.place_id() {
            wanted.places.insert(place.to_string());
        }
    }

    fn cells(&self, refs: &Refs) -> Vec<Option<ListCell>> {
        let place = self.place_id().map(|id| id.to_string());
        let place_name = place.as_ref().and_then(|id| refs.places.get(id)).and_then(place_title);
        vec![
            choice(self.event_type()),
            self.date().cloned().map(ListCell::Date),
            record(place, place_name),
            text(self.description()),
            None,
        ]
    }
}

impl ListedRecord for PlaceView {
    type State = PlaceState;

    const KIND: ListedKind = ListedKind::Place;
    const VIEW_TABLE: &'static str = PLACE_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.place_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn wants(&self, wanted: &mut Wanted) {
        if let Some(enclosing) = self.primary_enclosed_by() {
            wanted.places.insert(enclosing.value.value.place_id.to_string());
        }
    }

    fn cells(&self, refs: &Refs) -> Vec<Option<ListCell>> {
        let enclosing = self
            .primary_enclosed_by()
            .map(|enclosing| enclosing.value.value.place_id.to_string());
        let enclosing_name = enclosing
            .as_ref()
            .and_then(|id| refs.places.get(id))
            .and_then(place_title);
        vec![
            text(place_title(self).as_deref()),
            choice(self.place_type()),
            record(enclosing, enclosing_name),
            None,
        ]
    }
}

impl ListedRecord for SourceView {
    type State = SourceState;

    const KIND: ListedKind = ListedKind::Source;
    const VIEW_TABLE: &'static str = SOURCE_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.source_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn cells(&self, _refs: &Refs) -> Vec<Option<ListCell>> {
        vec![text(self.title()), text(self.author()), text(self.abbrev()), None]
    }
}

impl ListedRecord for CitationView {
    type State = CitationState;

    const KIND: ListedKind = ListedKind::Citation;
    const VIEW_TABLE: &'static str = CITATION_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.citation_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn wants(&self, wanted: &mut Wanted) {
        if let Some(source) = self.source_id() {
            wanted.sources.insert(source.to_string());
        }
    }

    fn cells(&self, refs: &Refs) -> Vec<Option<ListCell>> {
        let source = self.source_id().map(|id| id.to_string());
        let title = source
            .as_ref()
            .and_then(|id| refs.sources.get(id))
            .and_then(|source| source.title().map(str::to_owned));
        vec![
            record(source, title),
            text(self.page()),
            self.date().cloned().map(ListCell::Date),
            choice(self.confidence()),
            None,
        ]
    }
}

impl ListedRecord for RepositoryView {
    type State = RepositoryState;

    const KIND: ListedKind = ListedKind::Repository;
    const VIEW_TABLE: &'static str = REPOSITORY_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.repository_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn cells(&self, _refs: &Refs) -> Vec<Option<ListCell>> {
        vec![text(self.name()), choice(self.repository_type()), None]
    }
}

impl ListedRecord for MediaView {
    type State = MediaState;

    const KIND: ListedKind = ListedKind::Media;
    const VIEW_TABLE: &'static str = MEDIA_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.media_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn cells(&self, _refs: &Refs) -> Vec<Option<ListCell>> {
        let path = self.path().map(|path| match path {
            MediaPath::File(file) => file.as_str(),
            MediaPath::Web(url) => url.href.as_str(),
        });
        vec![
            text(path),
            text(self.mime()),
            self.date().cloned().map(ListCell::Date),
            None,
        ]
    }
}

impl ListedRecord for NoteView {
    type State = NoteState;

    const KIND: ListedKind = ListedKind::Note;
    const VIEW_TABLE: &'static str = NOTE_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.note_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn cells(&self, _refs: &Refs) -> Vec<Option<ListCell>> {
        vec![
            text(self.text().map(|text| text.text.as_str())),
            choice(self.note_type()),
            None,
        ]
    }
}

impl ListedRecord for ResearchNoteView {
    type State = ResearchNoteState;

    const KIND: ListedKind = ListedKind::ResearchNote;
    const VIEW_TABLE: &'static str = RESEARCH_NOTE_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.research_note_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn cells(&self, _refs: &Refs) -> Vec<Option<ListCell>> {
        vec![
            text(self.title()),
            text(self.body().map(|body| body.text.as_str())),
            None,
        ]
    }
}

impl ListedRecord for DnaTestView {
    type State = DnaTestState;

    const KIND: ListedKind = ListedKind::DnaTest;
    const VIEW_TABLE: &'static str = DNA_TEST_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.dna_test_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn wants(&self, wanted: &mut Wanted) {
        if let Some(person) = self.person_id() {
            wanted.persons.insert(person.to_string());
        }
    }

    fn cells(&self, refs: &Refs) -> Vec<Option<ListCell>> {
        let person = self.person_id().map(|id| id.to_string());
        let name = person
            .as_ref()
            .and_then(|id| refs.persons.get(id))
            .and_then(person_title);
        vec![
            record(person, name),
            choice(self.provider()),
            choice(self.test_type().as_ref()),
            text(self.kit_id()),
            None,
        ]
    }
}

impl ListedRecord for DnaMatchView {
    type State = DnaMatchState;

    const KIND: ListedKind = ListedKind::DnaMatch;
    const VIEW_TABLE: &'static str = DNA_MATCH_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.dna_match_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        self.human_id().map(|id| id.as_str().to_owned())
    }

    fn wants(&self, wanted: &mut Wanted) {
        for test in [self.test_a(), self.test_b()].into_iter().flatten() {
            wanted.dna_tests.insert(test.to_string());
        }
    }

    fn cells(&self, refs: &Refs) -> Vec<Option<ListCell>> {
        let test = |id: Option<String>| {
            let title = id
                .as_ref()
                .and_then(|id| refs.dna_tests.get(id))
                .and_then(dna_test_title);
            record(id, title)
        };
        // Hundredths of a cM well inside f64's exact integer range, so the division is exact enough.
        #[expect(clippy::cast_precision_loss, reason = "shared cM never nears 2^53 hundredths")]
        let shared = self
            .shared_cm()
            .map(|cm| ListCell::Number(cm.as_hundredths() as f64 / 100.0));
        vec![
            test(self.test_a().map(|id| id.to_string())),
            test(self.test_b().map(|id| id.to_string())),
            shared,
            choice(self.status().as_ref()),
            text(self.predicted_relationship()),
            None,
        ]
    }
}

impl ListedRecord for TagView {
    type State = TagState;

    const KIND: ListedKind = ListedKind::Tag;
    const VIEW_TABLE: &'static str = TAG_VIEW_TABLE;

    fn list_id(&self) -> Option<String> {
        self.tag_id().map(|id| id.to_string())
    }

    fn list_human_id(&self) -> Option<String> {
        None
    }

    fn cells(&self, _refs: &Refs) -> Vec<Option<ListCell>> {
        vec![
            text(self.name()),
            self.priority().map(|priority| ListCell::Number(f64::from(priority))),
            text(self.color()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use vitni_core::enums::Sex;
    use vitni_core::provenance::Confidence;

    use super::{choice_key, kebab};

    #[test]
    fn a_choice_is_named_by_its_kebab_case_variant() {
        assert_eq!(choice_key(&Sex::Female).as_deref(), Some("female"));
        assert_eq!(choice_key(&Confidence::VeryHigh).as_deref(), Some("very-high"));
        assert_eq!(choice_key(&Sex::Other("x".to_owned())).as_deref(), Some("other"));
        assert_eq!(kebab("TwentyThreeAndMe"), "twenty-three-and-me");
    }
}
