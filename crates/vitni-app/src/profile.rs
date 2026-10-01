//! The profiles the matching engine compares (ADR 0038 §2), built from the workspace's views.
//!
//! A person profile gathers a person's names, sex, occupations and external ids, the vital events they are the
//! primary participant in (a christening counts as a baptism) with each event's place, and their
//! relatives — parents, partners and children, each with names, sex and a birth. With no dated birth or
//! baptism, an age recorded at a dated event (a census) stands in, as a birth computed from that age.
//! Its places carry every enclosing place and the country, which, with the places of the person's other
//! events and their parents' vital events, select the name cultures. Its origin is the one of the
//! assertion that created the person, never a participation's: a church record's father, mother and
//! child all take part in one baptism item.
//!
//! A family profile holds its partners' person profiles — without their partners and children, which
//! the family compares itself — its children, and its linked marriage as an event profile. An event
//! profile holds its type, date and place, and every person taking part, with their role, names, sex
//! and birth; participation is the person's, so the participants are read from the persons. Each
//! profile's origin is the one of the assertion that created its record.
//!
//! A place profile holds its names, type, every enclosing place, country and coordinates. A source
//! profile holds its title, author and publication, and the repositories holding it, each as a
//! repository profile — name and addresses. A citation profile holds its source's profile, its page and
//! date. A media profile holds its checksum and path; a note profile its text and the text's language;
//! a tag profile its name. Each carries the origin of the assertion that created its record, except a
//! tag, which is its name.
//!
//! [`Profiles`] reads the views a set of kinds needs once — every person, event, place and family for
//! the person, family and event profiles — and each kind's creating origins in one query, so building
//! many profiles costs one load. The single-record builders below go through it too.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;
use vitni_core::citation::CitationView;
use vitni_core::date::{DateQuality, GenealogicalDate};
use vitni_core::enums::{ChildParentRelationship, EventType, FactType, ParticipantRole, PlaceType};
use vitni_core::event::EventView;
use vitni_core::family::{ChildEntry, FamilyView};
use vitni_core::ids::{
    CitationId, EventId, FamilyId, MediaId, NoteId, PersonId, PlaceId, RepositoryId, SourceId, TagId,
};
use vitni_core::matching::date::year;
use vitni_core::matching::profile::{
    CitationProfile, EventProfile, FamilyProfile, MediaProfile, NoteProfile, Participant, PersonProfile, PlaceMention,
    PlaceProfile, Relative, RepositoryProfile, SourceProfile, TagProfile, VitalEvent, VitalKind,
};
use vitni_core::matching::{DateBasis, MatchableKind};
use vitni_core::media::MediaView;
use vitni_core::note::NoteView;
use vitni_core::origin::RecordOrigin;
use vitni_core::person::PersonView;
use vitni_core::place::PlaceView;
use vitni_core::repository::RepositoryView;
use vitni_core::source::SourceView;
use vitni_core::tag::TagView;
use vitni_db::Store;

use crate::error::AppError;
use crate::event::{DateParts, gregorian_date};
use crate::person::resolve_person_id_public;
use crate::use_case;
use crate::workspace::Workspace;

/// Builds the matching profile of the person `human_id`.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or [`AppError`] on a store read failure.
pub async fn person_profile(workspace: &Workspace, human_id: &str) -> Result<PersonProfile, AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id_public(store, human_id).await?;
    let profiles = Profiles::load(store, &[MatchableKind::Person]).await?;
    profiles
        .person(person_id)
        .ok_or_else(|| AppError::PersonNotFound(human_id.to_owned()))
}

/// Builds the matching profile of the family `human_id`.
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if no such family exists, or [`AppError`] on a store read failure.
pub async fn family_profile(workspace: &Workspace, human_id: &str) -> Result<FamilyProfile, AppError> {
    let store = workspace.store();
    let not_found = || AppError::FamilyNotFound(human_id.to_owned());
    let family_id = use_case::resolve_id(store.find_family(human_id).await?, FamilyView::family_id, not_found)?;
    let profiles = Profiles::load(store, &[MatchableKind::Family]).await?;
    profiles.family(family_id).ok_or_else(not_found)
}

/// Builds the matching profile of the event `human_id`.
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or [`AppError`] on a store read failure.
pub async fn event_profile(workspace: &Workspace, human_id: &str) -> Result<EventProfile, AppError> {
    let store = workspace.store();
    let not_found = || AppError::EventNotFound(human_id.to_owned());
    let event_id = use_case::resolve_id(store.find_event(human_id).await?, EventView::event_id, not_found)?;
    let profiles = Profiles::load(store, &[MatchableKind::Event]).await?;
    profiles.event(event_id).ok_or_else(not_found)
}

/// Builds the matching profile of the place `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or [`AppError`] on a store read failure.
pub async fn place_profile(workspace: &Workspace, human_id: &str) -> Result<PlaceProfile, AppError> {
    let store = workspace.store();
    let not_found = || AppError::PlaceNotFound(human_id.to_owned());
    let place_id = use_case::resolve_id(store.find_place(human_id).await?, PlaceView::place_id, not_found)?;
    let profiles = Profiles::load(store, &[MatchableKind::Place]).await?;
    profiles.place(place_id).ok_or_else(not_found)
}

/// Builds the matching profile of the source `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or [`AppError`] on a store read failure.
pub async fn source_profile(workspace: &Workspace, human_id: &str) -> Result<SourceProfile, AppError> {
    let store = workspace.store();
    let not_found = || AppError::SourceNotFound(human_id.to_owned());
    let source_id = use_case::resolve_id(store.find_source(human_id).await?, SourceView::source_id, not_found)?;
    let profiles = Profiles::load(store, &[MatchableKind::Source]).await?;
    profiles.source(source_id).ok_or_else(not_found)
}

/// Builds the matching profile of the repository `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or [`AppError`] on a store read
/// failure.
pub async fn repository_profile(workspace: &Workspace, human_id: &str) -> Result<RepositoryProfile, AppError> {
    let store = workspace.store();
    let not_found = || AppError::RepositoryNotFound(human_id.to_owned());
    let found = store.find_repository(human_id).await?;
    let repository_id = use_case::resolve_id(found, RepositoryView::repository_id, not_found)?;
    let profiles = Profiles::load(store, &[MatchableKind::Repository]).await?;
    profiles.repository(repository_id).ok_or_else(not_found)
}

/// Builds the matching profile of the citation `human_id`.
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if no such citation exists, or [`AppError`] on a store read failure.
pub async fn citation_profile(workspace: &Workspace, human_id: &str) -> Result<CitationProfile, AppError> {
    let store = workspace.store();
    let not_found = || AppError::CitationNotFound(human_id.to_owned());
    let found = store.find_citation(human_id).await?;
    let citation_id = use_case::resolve_id(found, CitationView::citation_id, not_found)?;
    let profiles = Profiles::load(store, &[MatchableKind::Citation]).await?;
    profiles.citation(citation_id).ok_or_else(not_found)
}

/// Builds the matching profile of the media object `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media object exists, or [`AppError`] on a store read failure.
pub async fn media_profile(workspace: &Workspace, human_id: &str) -> Result<MediaProfile, AppError> {
    let store = workspace.store();
    let not_found = || AppError::MediaNotFound(human_id.to_owned());
    let media_id = use_case::resolve_id(store.find_media(human_id).await?, MediaView::media_id, not_found)?;
    let profiles = Profiles::load(store, &[MatchableKind::Media]).await?;
    profiles.media(media_id).ok_or_else(not_found)
}

/// Builds the matching profile of the note `human_id`.
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if no such note exists, or [`AppError`] on a store read failure.
pub async fn note_profile(workspace: &Workspace, human_id: &str) -> Result<NoteProfile, AppError> {
    let store = workspace.store();
    let not_found = || AppError::NoteNotFound(human_id.to_owned());
    let note_id = use_case::resolve_id(store.find_note(human_id).await?, NoteView::note_id, not_found)?;
    let profiles = Profiles::load(store, &[MatchableKind::Note]).await?;
    profiles.note(note_id).ok_or_else(not_found)
}

/// Builds the matching profile of the tag `id` — a tag has no human id.
///
/// # Errors
///
/// [`AppError::TagNotFound`] if no such tag exists, or [`AppError`] on a store read failure.
pub async fn tag_profile(workspace: &Workspace, id: &str) -> Result<TagProfile, AppError> {
    let not_found = || AppError::TagNotFound(id.to_owned());
    let tag_id = Uuid::parse_str(id).map(TagId::from_uuid).map_err(|_| not_found())?;
    let profiles = Profiles::load(workspace.store(), &[MatchableKind::Tag]).await?;
    profiles.tag(tag_id).ok_or_else(not_found)
}

/// A record's profile, tagged with its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Profile {
    Person(PersonProfile),
    Family(FamilyProfile),
    Event(EventProfile),
    Place(PlaceProfile),
    Source(SourceProfile),
    Repository(RepositoryProfile),
    Citation(CitationProfile),
    Media(MediaProfile),
    Note(NoteProfile),
    Tag(TagProfile),
}

/// The views the profiles of a set of kinds are built from, and the creating origins of each record,
/// read once.
#[derive(Default)]
pub(crate) struct Profiles {
    people: Option<ProfileLookups>,
    places: Option<PlaceLookup>,
    sources: HashMap<SourceId, SourceView>,
    repositories: HashMap<RepositoryId, RepositoryView>,
    citations: HashMap<CitationId, CitationView>,
    media: HashMap<MediaId, MediaView>,
    notes: HashMap<NoteId, NoteView>,
    tags: HashMap<TagId, TagView>,
    /// The origin of the creating assertion of each imported record, by aggregate id.
    origins: HashMap<String, Vec<RecordOrigin>>,
}

impl Profiles {
    /// Reads what the profiles of `kinds` need.
    pub(crate) async fn load(store: &Store, kinds: &[MatchableKind]) -> Result<Self, AppError> {
        use MatchableKind::{Citation, Event, Family, Media, Note, Person, Place, Repository, Source, Tag};
        let wants = |wanted: &[MatchableKind]| kinds.iter().any(|kind| wanted.contains(kind));
        let mut profiles = Self::default();
        let mut origin_kinds = Vec::new();
        if wants(&[Person, Family, Event]) {
            profiles.people = Some(ProfileLookups::load(store).await?);
            origin_kinds.extend([Person, Family, Event]);
        }
        if wants(&[Place]) {
            if profiles.people.is_none() {
                profiles.places = Some(PlaceLookup::load(store).await?);
            }
            origin_kinds.push(Place);
        }
        if wants(&[Source, Citation, Repository]) {
            profiles.sources = by_id(store.list_sources().await?, SourceView::source_id);
            profiles.repositories = by_id(store.list_repositories().await?, RepositoryView::repository_id);
            origin_kinds.extend([Source, Repository]);
        }
        if wants(&[Citation]) {
            profiles.citations = by_id(store.list_citations().await?, CitationView::citation_id);
            origin_kinds.push(Citation);
        }
        if wants(&[Media]) {
            profiles.media = by_id(store.list_media().await?, MediaView::media_id);
            origin_kinds.push(Media);
        }
        if wants(&[Note]) {
            profiles.notes = by_id(store.list_notes().await?, NoteView::note_id);
            origin_kinds.push(Note);
        }
        if wants(&[Tag]) {
            profiles.tags = by_id(store.list_tags().await?, TagView::tag_id);
        }
        for kind in origin_kinds {
            for (aggregate_id, origin) in store.created_origins(kind.as_str()).await? {
                profiles.origins.entry(aggregate_id).or_insert_with(|| vec![origin]);
            }
        }
        Ok(profiles)
    }

    /// Every pair of `kind` holding a live identity decision either way — merged or distinguished
    /// (ADR 0039 §3) — as aggregate ids, the lower first. Only persons can be decided yet.
    pub(crate) fn decided_pairs(&self, kind: MatchableKind) -> HashSet<(String, String)> {
        let mut pairs = HashSet::new();
        let (MatchableKind::Person, Some(people)) = (kind, &self.people) else {
            return pairs;
        };
        for (id, view) in &people.persons {
            for other in view.merged().into_iter().chain(view.distinguished()) {
                pairs.insert(ordered_pair(id.to_string(), other.to_string()));
            }
        }
        pairs
    }

    /// The aggregate id of every record of `kind` read, in id order.
    pub(crate) fn ids(&self, kind: MatchableKind) -> Vec<String> {
        let mut ids: Vec<String> = match kind {
            MatchableKind::Person => self
                .people
                .iter()
                .flat_map(|p| p.persons.keys().map(ToString::to_string))
                .collect(),
            MatchableKind::Family => self
                .people
                .iter()
                .flat_map(|p| p.families.keys().map(ToString::to_string))
                .collect(),
            MatchableKind::Event => self
                .people
                .iter()
                .flat_map(|p| p.events.keys().map(ToString::to_string))
                .collect(),
            MatchableKind::Place => self
                .place_lookup()
                .iter()
                .flat_map(|l| l.places.keys().map(ToString::to_string))
                .collect(),
            MatchableKind::Source => self.sources.keys().map(ToString::to_string).collect(),
            MatchableKind::Repository => self.repositories.keys().map(ToString::to_string).collect(),
            MatchableKind::Citation => self.citations.keys().map(ToString::to_string).collect(),
            MatchableKind::Media => self.media.keys().map(ToString::to_string).collect(),
            MatchableKind::Note => self.notes.keys().map(ToString::to_string).collect(),
            MatchableKind::Tag => self.tags.keys().map(ToString::to_string).collect(),
        };
        ids.sort_unstable();
        ids
    }

    /// The profile of the record `aggregate_id` of `kind`, or `None` when it was not read.
    pub(crate) fn profile(&self, kind: MatchableKind, aggregate_id: &str) -> Option<Profile> {
        let uuid = Uuid::parse_str(aggregate_id).ok()?;
        match kind {
            MatchableKind::Person => self.person(PersonId::from_uuid(uuid)).map(Profile::Person),
            MatchableKind::Family => self.family(FamilyId::from_uuid(uuid)).map(Profile::Family),
            MatchableKind::Event => self.event(EventId::from_uuid(uuid)).map(Profile::Event),
            MatchableKind::Place => self.place(PlaceId::from_uuid(uuid)).map(Profile::Place),
            MatchableKind::Source => self.source(SourceId::from_uuid(uuid)).map(Profile::Source),
            MatchableKind::Repository => self.repository(RepositoryId::from_uuid(uuid)).map(Profile::Repository),
            MatchableKind::Citation => self.citation(CitationId::from_uuid(uuid)).map(Profile::Citation),
            MatchableKind::Media => self.media(MediaId::from_uuid(uuid)).map(Profile::Media),
            MatchableKind::Note => self.note(NoteId::from_uuid(uuid)).map(Profile::Note),
            MatchableKind::Tag => self.tag(TagId::from_uuid(uuid)).map(Profile::Tag),
        }
    }

    /// The user-facing identifier of the record `aggregate_id` of `kind` — a tag's name, since a tag has
    /// no human id — or `None` when it was not read.
    pub(crate) fn human_id(&self, kind: MatchableKind, aggregate_id: &str) -> Option<String> {
        let uuid = Uuid::parse_str(aggregate_id).ok()?;
        let human = |id: Option<&vitni_core::ids::HumanId>| id.map(ToString::to_string);
        match kind {
            MatchableKind::Person => human(
                self.people
                    .as_ref()?
                    .persons
                    .get(&PersonId::from_uuid(uuid))?
                    .human_id(),
            ),
            MatchableKind::Family => human(
                self.people
                    .as_ref()?
                    .families
                    .get(&FamilyId::from_uuid(uuid))?
                    .human_id(),
            ),
            MatchableKind::Event => human(self.people.as_ref()?.events.get(&EventId::from_uuid(uuid))?.human_id()),
            MatchableKind::Place => human(self.place_lookup()?.places.get(&PlaceId::from_uuid(uuid))?.human_id()),
            MatchableKind::Source => human(self.sources.get(&SourceId::from_uuid(uuid))?.human_id()),
            MatchableKind::Repository => human(self.repositories.get(&RepositoryId::from_uuid(uuid))?.human_id()),
            MatchableKind::Citation => human(self.citations.get(&CitationId::from_uuid(uuid))?.human_id()),
            MatchableKind::Media => human(self.media.get(&MediaId::from_uuid(uuid))?.human_id()),
            MatchableKind::Note => human(self.notes.get(&NoteId::from_uuid(uuid))?.human_id()),
            MatchableKind::Tag => self.tags.get(&TagId::from_uuid(uuid))?.name().map(ToOwned::to_owned),
        }
    }

    /// The aggregate id of the record of `kind` known to the user as `human_id` — for a tag, its id — or
    /// `None` when there is none.
    pub(crate) fn aggregate_id_of(&self, kind: MatchableKind, human_id: &str) -> Option<String> {
        if kind == MatchableKind::Tag {
            let id = Uuid::parse_str(human_id).ok().map(TagId::from_uuid)?;
            return self.tags.contains_key(&id).then(|| id.to_string());
        }
        self.ids(kind)
            .into_iter()
            .find(|id| self.human_id(kind, id).as_deref() == Some(human_id))
    }

    /// The events that take place at any of `places`, whose keys carry the place's names.
    pub(crate) fn events_at(&self, places: &HashSet<String>) -> Vec<String> {
        let Some(people) = &self.people else {
            return Vec::new();
        };
        let mut events = Vec::new();
        for (id, view) in &people.events {
            if view.place_id().is_some_and(|place| places.contains(&place.to_string())) {
                events.push(id.to_string());
            }
        }
        events
    }

    /// The events every person of `persons` takes part in, and the families they are partners of — the
    /// records whose profiles carry theirs.
    pub(crate) fn dependents_of_persons(&self, persons: &HashSet<String>) -> (Vec<String>, Vec<String>) {
        let Some(people) = &self.people else {
            return (Vec::new(), Vec::new());
        };
        let mut events = Vec::new();
        for (event, taking_part) in &people.participants_of {
            if taking_part
                .iter()
                .any(|(person, _)| persons.contains(&person.to_string()))
            {
                events.push(event.to_string());
            }
        }
        let mut families = Vec::new();
        for (family, view) in &people.families {
            if view
                .partners()
                .iter()
                .any(|partner| persons.contains(&partner.to_string()))
            {
                families.push(family.to_string());
            }
        }
        (events, families)
    }

    /// The persons taking part in each of `events` as a principal, whose vital events come from them.
    pub(crate) fn participants_of(&self, events: &HashSet<String>) -> Vec<String> {
        let Some(people) = &self.people else {
            return Vec::new();
        };
        let mut persons = Vec::new();
        for (event, taking_part) in &people.participants_of {
            if events.contains(&event.to_string()) {
                persons.extend(taking_part.iter().map(|(person, _)| person.to_string()));
            }
        }
        persons
    }

    /// The citations of each of `sources`, whose profiles carry the source's.
    pub(crate) fn citations_of(&self, sources: &HashSet<String>) -> Vec<String> {
        let mut citations = Vec::new();
        for (id, view) in &self.citations {
            if view
                .source_id()
                .is_some_and(|source| sources.contains(&source.to_string()))
            {
                citations.push(id.to_string());
            }
        }
        citations
    }

    fn place_lookup(&self) -> Option<&PlaceLookup> {
        self.people
            .as_ref()
            .map(|people| &people.places)
            .or(self.places.as_ref())
    }

    fn origins_of(&self, aggregate_id: &impl ToString) -> Vec<RecordOrigin> {
        self.origins.get(&aggregate_id.to_string()).cloned().unwrap_or_default()
    }

    /// The person profile of `person_id`.
    pub(crate) fn person(&self, person_id: PersonId) -> Option<PersonProfile> {
        self.people.as_ref()?.profile(person_id, self.origins_of(&person_id))
    }

    /// The family profile of `family_id`: its partners without their own partners and children, its
    /// birth children, and its linked marriage.
    pub(crate) fn family(&self, family_id: FamilyId) -> Option<FamilyProfile> {
        let people = self.people.as_ref()?;
        let view = people.families.get(&family_id)?;
        let mut partners = Vec::new();
        for partner in view.partners() {
            if let Some(mut profile) = people.profile(partner, self.origins_of(&partner)) {
                profile.partners.clear();
                profile.children.clear();
                partners.push(profile);
            }
        }
        let children = view
            .children()
            .iter()
            .filter_map(|child| people.relative(child.child_id))
            .collect();
        let marriage = view
            .linked_events()
            .into_iter()
            .find(|event| people.events.get(event).and_then(EventView::event_type) == Some(&EventType::Marriage))
            .and_then(|event| people.event(event, self.origins_of(&event)));
        Some(FamilyProfile {
            partners,
            children,
            marriage,
            origins: self.origins_of(&family_id),
            external_ids: view.external_ids().into_iter().cloned().collect(),
        })
    }

    /// The event profile of `event_id`.
    pub(crate) fn event(&self, event_id: EventId) -> Option<EventProfile> {
        self.people.as_ref()?.event(event_id, self.origins_of(&event_id))
    }

    /// The place profile of `place_id`.
    pub(crate) fn place(&self, place_id: PlaceId) -> Option<PlaceProfile> {
        let lookup = self.place_lookup()?;
        lookup.places.get(&place_id)?;
        let mut profile = lookup.place(place_id);
        profile.origins = self.origins_of(&place_id);
        Some(profile)
    }

    /// The source profile of `source_id`, with every repository holding it.
    pub(crate) fn source(&self, source_id: SourceId) -> Option<SourceProfile> {
        let view = self.sources.get(&source_id)?;
        let repositories = view
            .repositories()
            .iter()
            .filter_map(|held| self.repository(held.repository_id))
            .collect();
        Some(SourceProfile {
            id: Some(source_id),
            title: view.title().map(ToOwned::to_owned),
            author: view.author().map(ToOwned::to_owned),
            publication: view.pub_info().map(ToOwned::to_owned),
            repositories,
            origins: self.origins_of(&source_id),
        })
    }

    /// The repository profile of `repository_id`.
    pub(crate) fn repository(&self, repository_id: RepositoryId) -> Option<RepositoryProfile> {
        let view = self.repositories.get(&repository_id)?;
        Some(RepositoryProfile {
            id: Some(repository_id),
            name: view.name().map(ToOwned::to_owned),
            addresses: view.addresses().into_iter().cloned().collect(),
            origins: self.origins_of(&repository_id),
        })
    }

    /// The citation profile of `citation_id`, with its source's.
    pub(crate) fn citation(&self, citation_id: CitationId) -> Option<CitationProfile> {
        let view = self.citations.get(&citation_id)?;
        Some(CitationProfile {
            source: view.source_id().and_then(|source| self.source(source)),
            page: view.page().map(ToOwned::to_owned),
            date: view.date().cloned(),
            origins: self.origins_of(&citation_id),
        })
    }

    /// The media profile of `media_id`.
    pub(crate) fn media(&self, media_id: MediaId) -> Option<MediaProfile> {
        let view = self.media.get(&media_id)?;
        Some(MediaProfile {
            checksum: view.checksum().map(ToOwned::to_owned),
            path: view.path().cloned(),
            origins: self.origins_of(&media_id),
        })
    }

    /// The note profile of `note_id`.
    pub(crate) fn note(&self, note_id: NoteId) -> Option<NoteProfile> {
        let text = self.notes.get(&note_id)?.text();
        Some(NoteProfile {
            text: text.map(|text| text.text.clone()),
            language: text.and_then(|text| text.language.clone()),
            origins: self.origins_of(&note_id),
        })
    }

    /// The tag profile of `tag_id`: its name.
    pub(crate) fn tag(&self, tag_id: TagId) -> Option<TagProfile> {
        let name = self.tags.get(&tag_id)?.name()?;
        Some(TagProfile { name: name.to_owned() })
    }
}

/// `views` keyed by their id, skipping any not yet created.
fn by_id<K: std::hash::Hash + Eq, V>(views: Vec<V>, id: fn(&V) -> Option<K>) -> HashMap<K, V> {
    let mut keyed = HashMap::with_capacity(views.len());
    for view in views {
        if let Some(key) = id(&view) {
            keyed.insert(key, view);
        }
    }
    keyed
}

/// The workspace's persons, events, places and family links, read once.
struct ProfileLookups {
    persons: HashMap<PersonId, PersonView>,
    events: HashMap<EventId, EventView>,
    families: HashMap<FamilyId, FamilyView>,
    places: PlaceLookup,
    participants_of: HashMap<EventId, Vec<(PersonId, ParticipantRole)>>,
    parents_of: HashMap<PersonId, Vec<PersonId>>,
    partners_of: HashMap<PersonId, Vec<PersonId>>,
    children_of: HashMap<PersonId, Vec<PersonId>>,
}

/// The two ids of a pair, the lower first.
pub(crate) fn ordered_pair(a: String, b: String) -> (String, String) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Appends `value` to `key`'s list unless it is already there.
fn link(map: &mut HashMap<PersonId, Vec<PersonId>>, key: PersonId, value: PersonId) {
    let values = map.entry(key).or_default();
    if !values.contains(&value) {
        values.push(value);
    }
}

impl ProfileLookups {
    async fn load(store: &Store) -> Result<Self, AppError> {
        let mut persons = HashMap::new();
        let mut participants_of: HashMap<EventId, Vec<(PersonId, ParticipantRole)>> = HashMap::new();
        for view in store.list_persons().await? {
            if let Some(id) = view.person_id() {
                for participation in view.participations() {
                    participants_of
                        .entry(participation.event_id)
                        .or_default()
                        .push((id, participation.role.clone()));
                }
                persons.insert(id, view);
            }
        }
        let mut events = HashMap::new();
        for view in store.list_events().await? {
            if let Some(id) = view.event_id() {
                events.insert(id, view);
            }
        }
        let mut lookups = Self {
            persons,
            events,
            families: HashMap::new(),
            places: PlaceLookup::load(store).await?,
            participants_of,
            parents_of: HashMap::new(),
            partners_of: HashMap::new(),
            children_of: HashMap::new(),
        };
        for family in store.list_families().await? {
            lookups.add_family(&family);
            if let Some(id) = family.family_id() {
                lookups.families.insert(id, family);
            }
        }
        Ok(lookups)
    }

    /// Records the partner, parent and child links of one family. A child is linked only to the partners
    /// it is a birth (or unspecified) child of: an adoptive, foster or step parent is not the parent a
    /// record of the child's birth names.
    fn add_family(&mut self, family: &FamilyView) {
        let partners = family.partners();
        let children = family.children();
        for &partner in &partners {
            for &other in &partners {
                if other != partner {
                    link(&mut self.partners_of, partner, other);
                }
            }
            for child in &children {
                if is_birth_child(child, partner) {
                    link(&mut self.parents_of, child.child_id, partner);
                    link(&mut self.children_of, partner, child.child_id);
                }
            }
        }
    }

    /// The profile of `person_id`, with its creating `origins`, or `None` when it is not projected.
    fn profile(&self, person_id: PersonId, origins: Vec<RecordOrigin>) -> Option<PersonProfile> {
        let view = self.persons.get(&person_id)?;
        let related = |map: &HashMap<PersonId, Vec<PersonId>>| -> Vec<Relative> {
            let ids = map.get(&person_id).map_or(&[][..], Vec::as_slice);
            ids.iter().filter_map(|id| self.relative(*id)).collect()
        };
        let parents = related(&self.parents_of);
        let mut lineage = self.mentions(view);
        for parent in self.parents_of.get(&person_id).into_iter().flatten() {
            if let Some(parent) = self.persons.get(parent) {
                lineage.extend(self.vitals(parent).iter().filter_map(mention));
            }
        }
        let occupations = view
            .facts()
            .into_iter()
            .filter(|fact| fact.value.fact_type == FactType::Occupation)
            .filter_map(|fact| fact.value.value.clone())
            .collect();
        Some(PersonProfile {
            names: view.names().into_iter().cloned().collect(),
            sex: view.sex().cloned(),
            vitals: self.vitals(view),
            lineage,
            occupations,
            parents,
            partners: related(&self.partners_of),
            children: related(&self.children_of),
            origins,
            external_ids: view.external_ids().into_iter().cloned().collect(),
        })
    }

    /// The profile of `event_id`, with its creating `origins`, or `None` when it is not projected.
    fn event(&self, event_id: EventId, origins: Vec<RecordOrigin>) -> Option<EventProfile> {
        let view = self.events.get(&event_id)?;
        let taking_part = self.participants_of.get(&event_id).map_or(&[][..], Vec::as_slice);
        let mut participants = Vec::new();
        for (person_id, role) in taking_part {
            if let Some(person) = self.relative(*person_id) {
                participants.push(Participant {
                    role: role.clone(),
                    person,
                });
            }
        }
        Some(EventProfile {
            event_type: view.event_type().cloned(),
            date: view.date().cloned(),
            place: view.place_id().map(|place| self.places.place(place)),
            participants,
            origins,
        })
    }

    /// A relative's names, sex and birth (or the baptism standing in for it).
    fn relative(&self, person_id: PersonId) -> Option<Relative> {
        let view = self.persons.get(&person_id)?;
        let vitals = self.vitals(view);
        let birth = [VitalKind::Birth, VitalKind::Baptism]
            .into_iter()
            .find_map(|kind| vitals.iter().find(|vital| vital.kind == kind && is_dated(vital)))
            .cloned();
        Some(Relative {
            names: view.names().into_iter().cloned().collect(),
            sex: view.sex().cloned(),
            birth,
        })
    }

    /// The vital events `view` is the primary participant in, and, when neither a birth nor a baptism
    /// is dated, a birth computed from an age recorded at a dated event.
    fn vitals(&self, view: &PersonView) -> Vec<VitalEvent> {
        let mut vitals = Vec::new();
        for participation in view.participations() {
            if participation.role != ParticipantRole::Primary {
                continue;
            }
            let Some(event) = self.events.get(&participation.event_id) else {
                continue;
            };
            let Some(kind) = event.event_type().and_then(vital_kind) else {
                continue;
            };
            vitals.push(VitalEvent {
                kind,
                date: event.date().cloned(),
                basis: DateBasis::Recorded,
                place: event.place_id().map(|place| self.places.place(place)),
            });
        }
        let born = vitals.iter().any(|vital| {
            let birth = match vital.kind {
                VitalKind::Birth | VitalKind::Baptism => true,
                VitalKind::Death | VitalKind::Burial => false,
            };
            birth && is_dated(vital)
        });
        if !born {
            vitals.extend(self.birth_from_age(view));
        }
        vitals
    }

    /// A birth computed from the first age recorded at a dated event.
    fn birth_from_age(&self, view: &PersonView) -> Option<VitalEvent> {
        view.participations().into_iter().find_map(|participation| {
            let years = participation.age.as_ref()?.years?;
            let event = self.events.get(&participation.event_id)?;
            let date = birth_from_age(event.date()?, years)?;
            Some(VitalEvent {
                kind: VitalKind::Birth,
                date: Some(date),
                basis: DateBasis::FromAge,
                place: None,
            })
        })
    }

    /// The countries of the places of `view`'s events that are not vital events — residences, censuses
    /// — which select name cultures without being compared.
    fn mentions(&self, view: &PersonView) -> Vec<PlaceMention> {
        let mut mentions = Vec::new();
        for participation in view.participations() {
            let Some(event) = self.events.get(&participation.event_id) else {
                continue;
            };
            if event.event_type().and_then(vital_kind).is_some() {
                continue;
            }
            let Some(country) = event.place_id().and_then(|place| self.places.place(place).country) else {
                continue;
            };
            mentions.push(PlaceMention {
                country,
                date: event.date().cloned(),
            });
        }
        mentions
    }
}

/// The workspace's places, read once.
struct PlaceLookup {
    places: HashMap<PlaceId, PlaceView>,
}

impl PlaceLookup {
    async fn load(store: &Store) -> Result<Self, AppError> {
        let mut places = HashMap::new();
        for view in store.list_places().await? {
            if let Some(id) = view.place_id() {
                places.insert(id, view);
            }
        }
        Ok(Self { places })
    }

    /// A place's names and type, every place enclosing it, its country and coordinates.
    fn place(&self, place_id: PlaceId) -> PlaceProfile {
        let Some(view) = self.places.get(&place_id) else {
            return PlaceProfile {
                id: Some(place_id),
                ..PlaceProfile::default()
            };
        };
        let enclosing = self.enclosing(place_id);
        let country = std::iter::once(&place_id)
            .chain(&enclosing)
            .filter_map(|id| self.places.get(id))
            .find(|place| place.place_type() == Some(&PlaceType::Country))
            .and_then(|country| country.names().first().map(|name| name.text.clone()));
        PlaceProfile {
            id: Some(place_id),
            names: view.names().into_iter().cloned().collect(),
            place_type: view.place_type().cloned(),
            enclosing,
            country,
            coordinates: view.coordinates().copied(),
            origins: Vec::new(),
        }
    }

    /// Every place enclosing `place_id`, nearest first, through every enclosure link; cycle-safe.
    fn enclosing(&self, place_id: PlaceId) -> Vec<PlaceId> {
        let mut seen = HashSet::from([place_id]);
        let mut found = Vec::new();
        let mut next = 0;
        let mut frontier = vec![place_id];
        while let Some(&current) = frontier.get(next) {
            next += 1;
            let Some(view) = self.places.get(&current) else {
                continue;
            };
            for parent in view.enclosed_by() {
                if seen.insert(parent.place_id) {
                    found.push(parent.place_id);
                    frontier.push(parent.place_id);
                }
            }
        }
        found
    }
}

/// The vital kind an event type is, if any. A christening is a baptism.
fn vital_kind(event_type: &EventType) -> Option<VitalKind> {
    match event_type {
        EventType::Birth => Some(VitalKind::Birth),
        EventType::Baptism | EventType::Christening => Some(VitalKind::Baptism),
        EventType::Death => Some(VitalKind::Death),
        EventType::Burial => Some(VitalKind::Burial),
        EventType::Marriage
        | EventType::Cremation
        | EventType::Census
        | EventType::Residence
        | EventType::Immigration
        | EventType::Emigration
        | EventType::Adoption
        | EventType::Confirmation
        | EventType::BarMitzvah
        | EventType::BasMitzvah
        | EventType::FirstCommunion
        | EventType::Graduation
        | EventType::Naturalization
        | EventType::Ordination
        | EventType::Probate
        | EventType::Retirement
        | EventType::Will
        | EventType::Engagement
        | EventType::Annulment
        | EventType::Divorce
        | EventType::DivorceFiled
        | EventType::MarriageBanns
        | EventType::MarriageContract
        | EventType::MarriageLicense
        | EventType::MarriageSettlement
        | EventType::Custom(_) => None,
    }
}

/// Whether `child` is a birth child of `partner`: the relationship is a birth, unknown, or unstated.
fn is_birth_child(child: &ChildEntry, partner: PersonId) -> bool {
    let Some((_, relationship)) = child.relationships.iter().find(|(parent, _)| *parent == partner) else {
        return true;
    };
    match relationship {
        ChildParentRelationship::Birth | ChildParentRelationship::Unknown => true,
        ChildParentRelationship::Adopted
        | ChildParentRelationship::Foster
        | ChildParentRelationship::Step
        | ChildParentRelationship::Sealed
        | ChildParentRelationship::Custom(_) => false,
    }
}

/// Whether a vital event carries a date with a year, the least the matcher can compare.
fn is_dated(vital: &VitalEvent) -> bool {
    vital.date.as_ref().and_then(year).is_some()
}

/// The country a vital event's place lies in, as a lineage mention.
fn mention(vital: &VitalEvent) -> Option<PlaceMention> {
    Some(PlaceMention {
        country: vital.place.as_ref()?.country.clone()?,
        date: vital.date.clone(),
    })
}

/// The birth year of someone `years` old at `date`, as a calculated date; `None` for an undated
/// (text-only or yearless) `date`.
fn birth_from_age(date: &GenealogicalDate, years: u16) -> Option<GenealogicalDate> {
    let year = year(date)?.checked_sub(i32::from(years))?;
    let mut birth = gregorian_date(DateParts {
        year,
        month: None,
        day: None,
    });
    birth.quality = DateQuality::Calculated;
    Some(birth)
}
