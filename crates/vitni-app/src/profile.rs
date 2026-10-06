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
//! [`Profiles`] reads the views the profiles are built from in one of two ways. Read whole, it lists every
//! record a set of kinds needs — every person, event, place and family for the person, family and
//! event profiles — and each kind's creating origins in one query, so scoring every record costs one
//! load. Read by record, it takes a few records by id with what their profiles reach, following the
//! references the record links index (ADR 0047) holds backwards — a person's families, an event's
//! participants — so one lookup costs what it scores. The single-record builders below read by record.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use uuid::Uuid;
use vitni_core::citation::CitationView;
use vitni_core::date::{DateQuality, GenealogicalDate};
use vitni_core::enums::{ChildParentRelationship, EventType, FactType, ParticipantRole, PlaceType};
use vitni_core::event::EventView;
use vitni_core::family::{ChildEntry, FamilyView};
use vitni_core::identity::ClusterRecord;
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
use vitni_core::place_name::PlaceName;
use vitni_core::repository::RepositoryView;
use vitni_core::source::SourceView;
use vitni_core::tag::TagView;
use vitni_db::{DbError, RecordLink, Store};

use crate::error::AppError;
use crate::event::{DateParts, gregorian_date};
use crate::identity::{
    CitationClusters, ClusterId, Clusters, EventClusters, FamilyClusters, MediaClusters, NoteClusters, PersonClusters,
    PlaceClusters, RepositoryClusters, SourceClusters,
};
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
    let profiles = Profiles::of_record(store, MatchableKind::Person, person_id).await?;
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
    let profiles = Profiles::of_record(store, MatchableKind::Family, family_id).await?;
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
    let profiles = Profiles::of_record(store, MatchableKind::Event, event_id).await?;
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
    let profiles = Profiles::of_record(store, MatchableKind::Place, place_id).await?;
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
    let profiles = Profiles::of_record(store, MatchableKind::Source, source_id).await?;
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
    let profiles = Profiles::of_record(store, MatchableKind::Repository, repository_id).await?;
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
    let profiles = Profiles::of_record(store, MatchableKind::Citation, citation_id).await?;
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
    let profiles = Profiles::of_record(store, MatchableKind::Media, media_id).await?;
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
    let profiles = Profiles::of_record(store, MatchableKind::Note, note_id).await?;
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
    let profiles = Profiles::of_record(workspace.store(), MatchableKind::Tag, tag_id).await?;
    profiles.tag(tag_id).ok_or_else(not_found)
}

/// The aggregate id of the record of `kind` known to the user as `human_id` — for a tag, its id — or
/// `None` when there is none.
///
/// # Errors
///
/// [`AppError`] on a store read failure.
pub(crate) async fn aggregate_id_of(
    store: &Store,
    kind: MatchableKind,
    human_id: &str,
) -> Result<Option<String>, AppError> {
    let found = match kind {
        MatchableKind::Person => id_of(store.find_person(human_id).await?.as_ref(), PersonView::person_id),
        MatchableKind::Family => id_of(store.find_family(human_id).await?.as_ref(), FamilyView::family_id),
        MatchableKind::Event => id_of(store.find_event(human_id).await?.as_ref(), EventView::event_id),
        MatchableKind::Place => id_of(store.find_place(human_id).await?.as_ref(), PlaceView::place_id),
        MatchableKind::Source => id_of(store.find_source(human_id).await?.as_ref(), SourceView::source_id),
        MatchableKind::Repository => id_of(
            store.find_repository(human_id).await?.as_ref(),
            RepositoryView::repository_id,
        ),
        MatchableKind::Citation => id_of(store.find_citation(human_id).await?.as_ref(), CitationView::citation_id),
        MatchableKind::Media => id_of(store.find_media(human_id).await?.as_ref(), MediaView::media_id),
        MatchableKind::Note => id_of(store.find_note(human_id).await?.as_ref(), NoteView::note_id),
        MatchableKind::Tag => {
            let Ok(uuid) = Uuid::parse_str(human_id) else {
                return Ok(None);
            };
            id_of(store.find_tag(&uuid.to_string()).await?.as_ref(), TagView::tag_id)
        }
    };
    Ok(found)
}

/// The aggregate id of `view`, if one was found and created.
fn id_of<V, I: ToString>(view: Option<&V>, id: fn(&V) -> Option<I>) -> Option<String> {
    view.and_then(id).map(|id| id.to_string())
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

/// The views the profiles are built from, with the creating origins and the identity clusters of their
/// records: read whole for a set of kinds ([`Profiles::load`]), or record by record with everything
/// each record's profile reads ([`Profiles::include`]).
#[derive(Default)]
pub(crate) struct Profiles {
    people: ProfileLookups,
    sources: HashMap<SourceId, SourceView>,
    repositories: HashMap<RepositoryId, RepositoryView>,
    citations: HashMap<CitationId, CitationView>,
    media: HashMap<MediaId, MediaView>,
    notes: HashMap<NoteId, NoteView>,
    tags: HashMap<TagId, TagView>,
    /// The origin of the creating assertion of each imported record, by aggregate id.
    origins: HashMap<String, Vec<RecordOrigin>>,
    person_clusters: PersonClusters,
    event_clusters: EventClusters,
    family_clusters: FamilyClusters,
    place_clusters: PlaceClusters,
    source_clusters: SourceClusters,
    repository_clusters: RepositoryClusters,
    citation_clusters: CitationClusters,
    media_clusters: MediaClusters,
    note_clusters: NoteClusters,
    /// The kinds read whole, of which [`Self::include`] reads nothing more.
    whole: HashSet<MatchableKind>,
    /// The kinds whose identity clusters were read.
    clustered: HashSet<MatchableKind>,
}

impl Profiles {
    /// Reads every record the profiles of `kinds` need.
    pub(crate) async fn load(store: &Store, kinds: &[MatchableKind]) -> Result<Self, AppError> {
        use MatchableKind::{Citation, Event, Family, Media, Note, Person, Place, Repository, Source, Tag};
        let wants = |wanted: &[MatchableKind]| kinds.iter().any(|kind| wanted.contains(kind));
        let mut profiles = Self::default();
        let mut whole = Vec::new();
        if wants(&[Person, Family, Event]) {
            profiles.people = ProfileLookups::load(store).await?;
            whole.extend([Person, Family, Event, Place]);
        } else if wants(&[Place]) {
            profiles.people.places = PlaceLookup::load(store).await?;
            whole.push(Place);
        }
        if wants(&[Source, Citation, Repository]) {
            profiles.sources = by_id(store.list_sources().await?, SourceView::source_id);
            profiles.repositories = by_id(store.list_repositories().await?, RepositoryView::repository_id);
            whole.extend([Source, Repository]);
        }
        if wants(&[Citation]) {
            profiles.citations = by_id(store.list_citations().await?, CitationView::citation_id);
            whole.push(Citation);
        }
        if wants(&[Media]) {
            profiles.media = by_id(store.list_media().await?, MediaView::media_id);
            whole.push(Media);
        }
        if wants(&[Note]) {
            profiles.notes = by_id(store.list_notes().await?, NoteView::note_id);
            whole.push(Note);
        }
        if wants(&[Tag]) {
            profiles.tags = by_id(store.list_tags().await?, TagView::tag_id);
            whole.push(Tag);
        }
        for kind in whole {
            profiles.read_clusters(store, kind).await?;
            if kind != Tag {
                for (aggregate_id, origin) in store.created_origins(kind.as_str()).await? {
                    profiles.origins.entry(aggregate_id).or_insert_with(|| vec![origin]);
                }
            }
            profiles.whole.insert(kind);
        }
        Ok(profiles)
    }

    /// Reads what the profile of the one record `id` of `kind` needs.
    async fn of_record(store: &Store, kind: MatchableKind, id: impl ToString) -> Result<Self, AppError> {
        let mut profiles = Self::default();
        profiles.include(store, kind, &[id.to_string()]).await?;
        Ok(profiles)
    }

    /// Reads the records `ids` (aggregate ids) of `kind` with everything their profiles read, and every
    /// other record of their identity clusters, whose distinctions [`Self::decisions`] needs. Reads
    /// nothing already read, and nothing of a kind read whole.
    pub(crate) async fn include(&mut self, store: &Store, kind: MatchableKind, ids: &[String]) -> Result<(), AppError> {
        self.read_clusters(store, kind).await?;
        if self.whole.contains(&kind) {
            return Ok(());
        }
        let ids = self.with_clusters(kind, ids);
        match kind {
            MatchableKind::Person => self.include_persons(store, &ids).await,
            MatchableKind::Family => self.include_families(store, &ids).await,
            MatchableKind::Event => self.include_events(store, &ids).await,
            MatchableKind::Place => self.read_places(store, ids).await,
            MatchableKind::Source => self.read_sources(store, ids).await,
            MatchableKind::Repository => self.read_repositories(store, ids).await,
            MatchableKind::Citation => self.read_citations(store, ids).await,
            MatchableKind::Media => {
                let read = read_views(
                    ids,
                    &mut self.media,
                    MediaId::from_uuid,
                    MediaView::media_id,
                    async |ids| store.media_by_ids(ids).await,
                )
                .await?;
                self.read_origins(store, kind, &read).await
            }
            MatchableKind::Note => {
                let read = read_views(
                    ids,
                    &mut self.notes,
                    NoteId::from_uuid,
                    NoteView::note_id,
                    async |ids| store.notes_by_ids(ids).await,
                )
                .await?;
                self.read_origins(store, kind, &read).await
            }
            MatchableKind::Tag => {
                read_views(ids, &mut self.tags, TagId::from_uuid, TagView::tag_id, async |ids| {
                    store.tags_by_ids(ids).await
                })
                .await?;
                Ok(())
            }
        }
    }

    /// Reads the identity clusters of `kind`, once.
    async fn read_clusters(&mut self, store: &Store, kind: MatchableKind) -> Result<(), AppError> {
        if !self.clustered.insert(kind) {
            return Ok(());
        }
        match kind {
            MatchableKind::Person => self.person_clusters = PersonClusters::load(store).await?,
            MatchableKind::Event => self.event_clusters = EventClusters::load(store).await?,
            MatchableKind::Family => self.family_clusters = FamilyClusters::load(store).await?,
            MatchableKind::Place => self.place_clusters = PlaceClusters::load(store).await?,
            MatchableKind::Source => self.source_clusters = SourceClusters::load(store).await?,
            MatchableKind::Repository => self.repository_clusters = RepositoryClusters::load(store).await?,
            MatchableKind::Citation => self.citation_clusters = CitationClusters::load(store).await?,
            MatchableKind::Media => self.media_clusters = MediaClusters::load(store).await?,
            MatchableKind::Note => self.note_clusters = NoteClusters::load(store).await?,
            MatchableKind::Tag => {}
        }
        Ok(())
    }

    /// `ids` with every record of their identity clusters.
    fn with_clusters(&self, kind: MatchableKind, ids: &[String]) -> Vec<String> {
        match kind {
            MatchableKind::Person => cluster_ids(&self.person_clusters, ids),
            MatchableKind::Event => cluster_ids(&self.event_clusters, ids),
            MatchableKind::Family => cluster_ids(&self.family_clusters, ids),
            MatchableKind::Place => cluster_ids(&self.place_clusters, ids),
            MatchableKind::Source => cluster_ids(&self.source_clusters, ids),
            MatchableKind::Repository => cluster_ids(&self.repository_clusters, ids),
            MatchableKind::Citation => cluster_ids(&self.citation_clusters, ids),
            MatchableKind::Media => cluster_ids(&self.media_clusters, ids),
            MatchableKind::Note => cluster_ids(&self.note_clusters, ids),
            MatchableKind::Tag => ids.to_vec(),
        }
    }

    /// Reads the creating origins of the records `ids` of `kind`.
    async fn read_origins(&mut self, store: &Store, kind: MatchableKind, ids: &[String]) -> Result<(), AppError> {
        for (aggregate_id, origin) in store.created_origins_of(kind.as_str(), ids).await? {
            self.origins.entry(aggregate_id).or_insert_with(|| vec![origin]);
        }
        Ok(())
    }

    /// Reads the persons `ids` with their profiles: their families, and every partner, child and parent
    /// in them.
    async fn include_persons(&mut self, store: &Store, ids: &[String]) -> Result<(), AppError> {
        self.read_persons(store, ids.to_vec()).await?;
        let mut families = Vec::new();
        for relation in [RecordLink::FamilyPartner, RecordLink::FamilyChild] {
            families.extend(linking(store, relation, ids).await?);
        }
        self.read_families(store, families.clone()).await?;
        let mut members = Vec::new();
        for family in self.people.families_of(&families) {
            members.extend(family.partners().iter().map(ToString::to_string));
            members.extend(family.children().iter().map(|child| child.child_id.to_string()));
        }
        self.read_persons(store, members).await
    }

    /// Reads the families `ids` with their profiles: their partners' profiles, their children, and their
    /// linked events.
    async fn include_families(&mut self, store: &Store, ids: &[String]) -> Result<(), AppError> {
        self.read_families(store, ids.to_vec()).await?;
        let (mut partners, mut children, mut events) = (Vec::new(), Vec::new(), Vec::new());
        for family in self.people.families_of(ids) {
            partners.extend(family.partners().iter().map(ToString::to_string));
            children.extend(family.children().iter().map(|child| child.child_id.to_string()));
            events.extend(family.linked_events().iter().map(ToString::to_string));
        }
        self.include_persons(store, &partners).await?;
        self.read_persons(store, children).await?;
        self.include_events(store, &events).await
    }

    /// Reads the events `ids` with their profiles: every person taking part.
    async fn include_events(&mut self, store: &Store, ids: &[String]) -> Result<(), AppError> {
        self.read_events(store, ids.to_vec()).await?;
        let participants = linking(store, RecordLink::Participation, ids).await?;
        self.read_persons(store, participants).await
    }

    /// Reads the persons `ids` as relatives: their views, and every event they take part in.
    async fn read_persons(&mut self, store: &Store, ids: Vec<String>) -> Result<(), AppError> {
        let unread = unread(ids, &self.people.persons, PersonId::from_uuid);
        if unread.is_empty() {
            return Ok(());
        }
        let mut events = Vec::new();
        for view in store.persons_by_ids(&unread).await? {
            events.extend(view.participations().iter().map(|p| p.event_id.to_string()));
            self.people.add_person(view);
        }
        self.read_origins(store, MatchableKind::Person, &unread).await?;
        self.read_events(store, events).await
    }

    /// Reads the events `ids`: their views, and their places.
    async fn read_events(&mut self, store: &Store, ids: Vec<String>) -> Result<(), AppError> {
        let events = &mut self.people.events;
        let read = read_views(ids, events, EventId::from_uuid, EventView::event_id, async |ids| {
            store.events_by_ids(ids).await
        })
        .await?;
        let places = self.people.events_of(&read).filter_map(EventView::place_id);
        let places = places.map(|place| place.to_string()).collect();
        self.read_origins(store, MatchableKind::Event, &read).await?;
        self.read_places(store, places).await
    }

    /// Reads the families `ids`, recording their partner, parent and child links.
    async fn read_families(&mut self, store: &Store, ids: Vec<String>) -> Result<(), AppError> {
        let unread = unread(ids, &self.people.families, FamilyId::from_uuid);
        if unread.is_empty() {
            return Ok(());
        }
        for family in store.families_by_ids(&unread).await? {
            self.people.add_family(family);
        }
        self.read_origins(store, MatchableKind::Family, &unread).await
    }

    /// Reads the places `ids` and every place enclosing them.
    async fn read_places(&mut self, store: &Store, ids: Vec<String>) -> Result<(), AppError> {
        let mut next = ids;
        while !next.is_empty() {
            let places = &mut self.people.places.places;
            let read = read_views(next, places, PlaceId::from_uuid, PlaceView::place_id, async |ids| {
                store.places_by_ids(ids).await
            })
            .await?;
            let mut enclosing = Vec::new();
            for id in &read {
                let Some(view) = Uuid::parse_str(id)
                    .ok()
                    .and_then(|uuid| places.get(&PlaceId::from_uuid(uuid)))
                else {
                    continue;
                };
                enclosing.extend(view.enclosed_by().iter().map(|parent| parent.place_id.to_string()));
            }
            self.read_origins(store, MatchableKind::Place, &read).await?;
            next = enclosing;
        }
        Ok(())
    }

    /// Reads the sources `ids` and the repositories holding them.
    async fn read_sources(&mut self, store: &Store, ids: Vec<String>) -> Result<(), AppError> {
        let read = read_views(
            ids,
            &mut self.sources,
            SourceId::from_uuid,
            SourceView::source_id,
            async |ids| store.sources_by_ids(ids).await,
        )
        .await?;
        let mut repositories = Vec::new();
        for id in &read {
            let Some(view) = Uuid::parse_str(id)
                .ok()
                .and_then(|uuid| self.sources.get(&SourceId::from_uuid(uuid)))
            else {
                continue;
            };
            repositories.extend(view.repositories().iter().map(|held| held.repository_id.to_string()));
        }
        self.read_origins(store, MatchableKind::Source, &read).await?;
        self.read_repositories(store, repositories).await
    }

    /// Reads the repositories `ids`.
    async fn read_repositories(&mut self, store: &Store, ids: Vec<String>) -> Result<(), AppError> {
        let repositories = &mut self.repositories;
        let read = read_views(
            ids,
            repositories,
            RepositoryId::from_uuid,
            RepositoryView::repository_id,
            async |ids| store.repositories_by_ids(ids).await,
        )
        .await?;
        self.read_origins(store, MatchableKind::Repository, &read).await
    }

    /// Reads the citations `ids` and their sources.
    async fn read_citations(&mut self, store: &Store, ids: Vec<String>) -> Result<(), AppError> {
        let citations = &mut self.citations;
        let read = read_views(
            ids,
            citations,
            CitationId::from_uuid,
            CitationView::citation_id,
            async |ids| store.citations_by_ids(ids).await,
        )
        .await?;
        let mut sources = Vec::new();
        for id in &read {
            let Some(view) = Uuid::parse_str(id)
                .ok()
                .and_then(|uuid| citations.get(&CitationId::from_uuid(uuid)))
            else {
                continue;
            };
            sources.extend(view.source_id().map(|source| source.to_string()));
        }
        self.read_origins(store, MatchableKind::Citation, &read).await?;
        self.read_sources(store, sources).await
    }

    /// The events each of `persons` (aggregate ids) takes part in, of those read.
    pub(crate) fn events_of_persons(&self, persons: &[String]) -> Vec<String> {
        let mut events = Vec::new();
        for id in persons {
            let Some(view) = Uuid::parse_str(id)
                .ok()
                .and_then(|uuid| self.people.persons.get(&PersonId::from_uuid(uuid)))
            else {
                continue;
            };
            events.extend(view.participations().iter().map(|p| p.event_id.to_string()));
        }
        events
    }

    /// The identity decisions on `kind` that keep a pair out of every suggestion (ADR 0039 §3, §4),
    /// among the records read. Every kind but tags can be decided.
    pub(crate) fn decisions(&self, kind: MatchableKind) -> Decisions {
        let people = &self.people;
        match kind {
            MatchableKind::Person => Decisions::of(&self.person_clusters, people.persons.values()),
            MatchableKind::Event => Decisions::of(&self.event_clusters, people.events.values()),
            MatchableKind::Family => Decisions::of(&self.family_clusters, people.families.values()),
            MatchableKind::Place => Decisions::of(&self.place_clusters, people.places.places.values()),
            MatchableKind::Source => Decisions::of(&self.source_clusters, self.sources.values()),
            MatchableKind::Repository => Decisions::of(&self.repository_clusters, self.repositories.values()),
            MatchableKind::Citation => Decisions::of(&self.citation_clusters, self.citations.values()),
            MatchableKind::Media => Decisions::of(&self.media_clusters, self.media.values()),
            MatchableKind::Note => Decisions::of(&self.note_clusters, self.notes.values()),
            MatchableKind::Tag => Decisions::default(),
        }
    }

    /// The aggregate id of every record of `kind` read, in id order.
    pub(crate) fn ids(&self, kind: MatchableKind) -> Vec<String> {
        let people = &self.people;
        let mut ids: Vec<String> = match kind {
            MatchableKind::Person => people.persons.keys().map(ToString::to_string).collect(),
            MatchableKind::Family => people.families.keys().map(ToString::to_string).collect(),
            MatchableKind::Event => people.events.keys().map(ToString::to_string).collect(),
            MatchableKind::Place => people.places.places.keys().map(ToString::to_string).collect(),
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
        let people = &self.people;
        match kind {
            MatchableKind::Person => human(people.persons.get(&PersonId::from_uuid(uuid))?.human_id()),
            MatchableKind::Family => human(people.families.get(&FamilyId::from_uuid(uuid))?.human_id()),
            MatchableKind::Event => human(people.events.get(&EventId::from_uuid(uuid))?.human_id()),
            MatchableKind::Place => human(people.places.places.get(&PlaceId::from_uuid(uuid))?.human_id()),
            MatchableKind::Source => human(self.sources.get(&SourceId::from_uuid(uuid))?.human_id()),
            MatchableKind::Repository => human(self.repositories.get(&RepositoryId::from_uuid(uuid))?.human_id()),
            MatchableKind::Citation => human(self.citations.get(&CitationId::from_uuid(uuid))?.human_id()),
            MatchableKind::Media => human(self.media.get(&MediaId::from_uuid(uuid))?.human_id()),
            MatchableKind::Note => human(self.notes.get(&NoteId::from_uuid(uuid))?.human_id()),
            MatchableKind::Tag => self.tags.get(&TagId::from_uuid(uuid))?.name().map(ToOwned::to_owned),
        }
    }

    fn origins_of(&self, aggregate_id: &impl ToString) -> Vec<RecordOrigin> {
        self.origins.get(&aggregate_id.to_string()).cloned().unwrap_or_default()
    }

    /// The names, sex and birth of the person `person_id`, as a relative of another.
    pub(crate) fn relative(&self, person_id: PersonId) -> Option<Relative> {
        self.people.relative(person_id)
    }

    /// The person profile of `person_id`.
    pub(crate) fn person(&self, person_id: PersonId) -> Option<PersonProfile> {
        self.people.profile(person_id, self.origins_of(&person_id))
    }

    /// The family profile of `family_id`: its partners without their own partners and children, its
    /// birth children, and its linked marriage.
    pub(crate) fn family(&self, family_id: FamilyId) -> Option<FamilyProfile> {
        let people = &self.people;
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
        self.people.event(event_id, self.origins_of(&event_id))
    }

    /// The place profile of `place_id`.
    pub(crate) fn place(&self, place_id: PlaceId) -> Option<PlaceProfile> {
        let lookup = &self.people.places;
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

/// Those of `ids` (aggregate ids) not yet in `read`, each once, in id order.
fn unread<K: Eq + Hash, V>(ids: Vec<String>, read: &HashMap<K, V>, id: fn(Uuid) -> K) -> Vec<String> {
    let mut out = BTreeSet::new();
    for raw in ids {
        let Ok(uuid) = Uuid::parse_str(&raw) else { continue };
        if !read.contains_key(&id(uuid)) {
            out.insert(raw);
        }
    }
    out.into_iter().collect()
}

/// Reads into `read` the views of those of `ids` not in it yet, through `fetch`; returns the ids it
/// asked for.
async fn read_views<K: Eq + Hash, V>(
    ids: Vec<String>,
    read: &mut HashMap<K, V>,
    id: fn(Uuid) -> K,
    key: fn(&V) -> Option<K>,
    fetch: impl AsyncFnOnce(&[String]) -> Result<Vec<V>, DbError>,
) -> Result<Vec<String>, AppError> {
    let asked = unread(ids, read, id);
    if asked.is_empty() {
        return Ok(asked);
    }
    for view in fetch(&asked).await? {
        if let Some(key) = key(&view) {
            read.insert(key, view);
        }
    }
    Ok(asked)
}

/// The records whose `relation` reaches any of `targets` (aggregate ids).
pub(crate) async fn linking(store: &Store, relation: RecordLink, targets: &[String]) -> Result<Vec<String>, AppError> {
    let mut sources = Vec::new();
    for (source, _) in store.linking(relation, targets).await? {
        sources.push(source);
    }
    Ok(sources)
}

/// `ids` with every other record of their identity clusters in `clusters`, each once, in id order.
fn cluster_ids<I: ClusterId>(clusters: &Clusters<I>, ids: &[String]) -> Vec<String> {
    let mut out = BTreeSet::new();
    for raw in ids {
        out.insert(raw.clone());
        let Ok(uuid) = Uuid::parse_str(raw) else { continue };
        let root = clusters.root(<I::View as ClusterRecord>::id_from_uuid(uuid));
        out.extend(clusters.cluster(root).iter().map(ToString::to_string));
    }
    out.into_iter().collect()
}

/// The workspace's persons, events, places and family links read so far.
#[derive(Default)]
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

/// The identity decisions on one kind (ADR 0039 §4): each merged record's cluster root, and every
/// pair of cluster roots held distinct, by aggregate id.
#[derive(Default)]
pub(crate) struct Decisions {
    root_of: HashMap<String, String>,
    distinct: HashSet<(String, String)>,
}

impl Decisions {
    /// The decisions recorded in `clusters` and on the `views` of their kind.
    fn of<'a, V: ClusterRecord<Id: ClusterId> + 'a>(
        clusters: &Clusters<V::Id>,
        views: impl Iterator<Item = &'a V>,
    ) -> Self {
        let mut root_of = HashMap::new();
        for (member, root) in clusters.links() {
            root_of.insert(member.to_string(), root.to_string());
        }
        let mut distinct = HashSet::new();
        for view in views {
            let Some(id) = view.record_id() else { continue };
            for other in view.distinguished_with_assertions() {
                let (a, b) = (clusters.root(id), clusters.root(other.value));
                distinct.insert(ordered_pair(a.to_string(), b.to_string()));
            }
        }
        Self { root_of, distinct }
    }

    /// The aggregate id of the root of `id`'s cluster: `id` itself unless it is merged.
    pub(crate) fn root(&self, id: &str) -> String {
        self.root_of.get(id).cloned().unwrap_or_else(|| id.to_owned())
    }

    /// Whether the pair `a`, `b` (aggregate ids) is never proposed: either is merged into another
    /// record and so hidden behind its root, both are one cluster, or their clusters are held
    /// distinct.
    pub(crate) fn exclude(&self, a: &str, b: &str) -> bool {
        if self.root_of.contains_key(a) || self.root_of.contains_key(b) {
            return true;
        }
        a == b || self.distinct.contains(&ordered_pair(a.to_owned(), b.to_owned()))
    }
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
        let mut lookups = Self {
            places: PlaceLookup::load(store).await?,
            ..Self::default()
        };
        for view in store.list_persons().await? {
            lookups.add_person(view);
        }
        for view in store.list_events().await? {
            if let Some(id) = view.event_id() {
                lookups.events.insert(id, view);
            }
        }
        for family in store.list_families().await? {
            lookups.add_family(family);
        }
        Ok(lookups)
    }

    /// Adds a person, recording the events they take part in.
    fn add_person(&mut self, view: PersonView) {
        let Some(id) = view.person_id() else { return };
        for participation in view.participations() {
            self.participants_of
                .entry(participation.event_id)
                .or_default()
                .push((id, participation.role.clone()));
        }
        self.persons.insert(id, view);
    }

    /// Adds a family, recording its partner, parent and child links.
    fn add_family(&mut self, family: FamilyView) {
        let Some(id) = family.family_id() else { return };
        self.link_family(&family);
        self.families.insert(id, family);
    }

    /// The families of `ids` (aggregate ids) read.
    fn families_of<'a>(&'a self, ids: &'a [String]) -> impl Iterator<Item = &'a FamilyView> + 'a {
        ids.iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .filter_map(|uuid| self.families.get(&FamilyId::from_uuid(uuid)))
    }

    /// The events of `ids` (aggregate ids) read.
    fn events_of<'a>(&'a self, ids: &'a [String]) -> impl Iterator<Item = &'a EventView> + 'a {
        ids.iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .filter_map(|uuid| self.events.get(&EventId::from_uuid(uuid)))
    }

    /// Records the partner, parent and child links of one family. A child is linked only to the partners
    /// it is a birth (or unspecified) child of: an adoptive, foster or step parent is not the parent a
    /// record of the child's birth names.
    fn link_family(&mut self, family: &FamilyView) {
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

/// The workspace's places read so far.
#[derive(Default)]
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
            place_type: crate::place::stated_place_type(view).cloned(),
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

/// A place name as a record gives it: no language, no date.
pub(crate) fn place_name(text: &str) -> PlaceName {
    PlaceName {
        text: text.to_owned(),
        language: None,
        date: None,
    }
}

/// The vital kind an event type is, if any. A christening is a baptism.
pub(crate) fn vital_kind(event_type: &EventType) -> Option<VitalKind> {
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
