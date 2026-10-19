//! The reverse tag index (Phase 5 PR 11): given a tag, which records carry it, grouped by object
//! type with a count and a few examples (the Tag › Usage tab).
//!
//! Tags are applied on the *tagged* record, not stored inversely on the Tag; the record links index
//! mirrors each application as a `tagged` link (ADR 0052 §6), so a tag's records are read through it —
//! each a cluster root (ADR 0039 §5), listed once — and only the few examples shown are loaded.

use std::collections::HashMap;

use vitni_core::dna_test::DnaTestView;
use vitni_core::family::FamilyView;
use vitni_core::ids::{PersonId, TagId};
use vitni_core::media_path::MediaPath;
use vitni_db::{ListedKind, Store};

use crate::dto::{UsingKind, UsingRecordRef};
use crate::error::AppError;
use crate::list_query::in_order;
use crate::workspace::Workspace;

/// How many examples to surface per object-type group on the Usage tab.
const MAX_EXAMPLES: usize = 3;

/// The upper bound on a derived note-snippet label, so a long note body does not blow out the
/// Examples column.
const NOTE_SNIPPET_LEN: usize = 40;

/// One object-type group on the Tag › Usage tab: the kind, how many records of that kind carry the
/// tag, and the first few as examples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagUsageGroup {
    /// The object type the rows belong to.
    pub kind: UsingKind,
    /// How many records of this kind carry the tag.
    pub count: usize,
    /// The first few carrying records, for the Examples column.
    pub examples: Vec<UsingRecordRef>,
}

/// How many records carry each tag in use, by tag id.
///
/// # Errors
///
/// A store error.
pub(crate) async fn use_counts(workspace: &Workspace) -> Result<HashMap<String, usize>, AppError> {
    let mut counts = HashMap::new();
    for (tag, uses) in workspace.store().tag_use_counts().await? {
        counts.insert(tag, usize::try_from(uses).unwrap_or(usize::MAX));
    }
    Ok(counts)
}

/// The records carrying `tag`, grouped by object type with counts and their first examples, and how
/// many there are in all.
///
/// # Errors
///
/// A store error.
pub(crate) async fn usage(workspace: &Workspace, tag: TagId) -> Result<(Vec<TagUsageGroup>, usize), AppError> {
    let uses = workspace.store().tag_uses(&tag.to_string()).await?;
    let mut by_kind: Vec<(ListedKind, usize, Vec<String>)> = Vec::new();
    for (kind, id) in &uses {
        match by_kind.last_mut() {
            Some((held, count, ids)) if held == kind => {
                *count += 1;
                if ids.len() < MAX_EXAMPLES {
                    ids.push(id.clone());
                }
            }
            _ => by_kind.push((*kind, 1, vec![id.clone()])),
        }
    }
    let mut groups = Vec::with_capacity(by_kind.len());
    for (kind, count, ids) in by_kind {
        if let Some((kind, examples)) = examples(workspace.store(), kind, &ids).await? {
            groups.push(TagUsageGroup { kind, count, examples });
        }
    }
    Ok((groups, uses.len()))
}

/// A carrying record, once it has an id and a `human_id`.
fn refer(
    kind: UsingKind,
    id: Option<String>,
    human_id: Option<&vitni_core::ids::HumanId>,
    label: Option<String>,
) -> Option<UsingRecordRef> {
    Some(UsingRecordRef {
        kind,
        human_id: human_id?.as_str().to_owned(),
        id: id?,
        label,
    })
}

/// `views` in the order of `ids` as carrying records of `kind`, `describe` giving each one's id,
/// `human_id` and label.
fn labelled<V>(
    views: Vec<V>,
    ids: &[String],
    kind: UsingKind,
    describe: impl Fn(&V) -> (Option<String>, Option<String>, Option<String>),
) -> Vec<UsingRecordRef> {
    let mut examples = Vec::with_capacity(views.len());
    for view in in_order(views, ids, |view| describe(view).0) {
        let (id, human_id, label) = describe(&view);
        if let (Some(id), Some(human_id)) = (id, human_id) {
            examples.push(UsingRecordRef {
                kind,
                human_id,
                id,
                label,
            });
        }
    }
    examples
}

fn hid(human_id: Option<&vitni_core::ids::HumanId>) -> Option<String> {
    human_id.map(|id| id.as_str().to_owned())
}

/// The example records `ids` of `kind`, in that order, each with a human-readable label (never a
/// UUID): partner names for a family, the cited source for a citation, the anchor for a DNA test.
async fn examples(
    store: &Store,
    kind: ListedKind,
    ids: &[String],
) -> Result<Option<(UsingKind, Vec<UsingRecordRef>)>, AppError> {
    let examples = match kind {
        ListedKind::Person => (
            UsingKind::Person,
            labelled(store.persons_by_ids(ids).await?, ids, UsingKind::Person, |v| {
                (
                    v.person_id().map(|i| i.to_string()),
                    hid(v.human_id()),
                    v.names().first().map(|n| n.display()),
                )
            }),
        ),
        ListedKind::Family => (UsingKind::Family, family_examples(store, ids).await?),
        ListedKind::Event => (
            UsingKind::Event,
            labelled(store.events_by_ids(ids).await?, ids, UsingKind::Event, |v| {
                (
                    v.event_id().map(|i| i.to_string()),
                    hid(v.human_id()),
                    v.description().map(ToOwned::to_owned),
                )
            }),
        ),
        ListedKind::Place => (
            UsingKind::Place,
            labelled(store.places_by_ids(ids).await?, ids, UsingKind::Place, |v| {
                (
                    v.place_id().map(|i| i.to_string()),
                    hid(v.human_id()),
                    v.names().first().map(|n| n.text.clone()),
                )
            }),
        ),
        ListedKind::Source => (
            UsingKind::Source,
            labelled(store.sources_by_ids(ids).await?, ids, UsingKind::Source, |v| {
                (
                    v.source_id().map(|i| i.to_string()),
                    hid(v.human_id()),
                    v.title().map(ToOwned::to_owned),
                )
            }),
        ),
        ListedKind::Citation => (UsingKind::Citation, citation_examples(store, ids).await?),
        ListedKind::Repository => (
            UsingKind::Repository,
            labelled(store.repositories_by_ids(ids).await?, ids, UsingKind::Repository, |v| {
                (
                    v.repository_id().map(|i| i.to_string()),
                    hid(v.human_id()),
                    v.name().map(ToOwned::to_owned),
                )
            }),
        ),
        ListedKind::Media => (
            UsingKind::Media,
            labelled(store.media_by_ids(ids).await?, ids, UsingKind::Media, |v| {
                (
                    v.media_id().map(|i| i.to_string()),
                    hid(v.human_id()),
                    v.path().and_then(media_label),
                )
            }),
        ),
        ListedKind::Note => (
            UsingKind::Note,
            labelled(store.notes_by_ids(ids).await?, ids, UsingKind::Note, |v| {
                let label = v.text().and_then(|text| note_snippet(&text.text));
                (v.note_id().map(|i| i.to_string()), hid(v.human_id()), label)
            }),
        ),
        ListedKind::ResearchNote => (
            UsingKind::ResearchNote,
            labelled(
                store.research_notes_by_ids(ids).await?,
                ids,
                UsingKind::ResearchNote,
                |v| {
                    (
                        v.research_note_id().map(|i| i.to_string()),
                        hid(v.human_id()),
                        v.title().map(ToOwned::to_owned),
                    )
                },
            ),
        ),
        ListedKind::DnaTest => (UsingKind::DnaTest, dna_test_examples(store, ids).await?),
        ListedKind::DnaMatch => (
            UsingKind::DnaMatch,
            labelled(store.dna_matches_by_ids(ids).await?, ids, UsingKind::DnaMatch, |v| {
                let label = v.predicted_relationship().map(ToOwned::to_owned);
                (v.dna_match_id().map(|i| i.to_string()), hid(v.human_id()), label)
            }),
        ),
        ListedKind::Tag => return Ok(None),
    };
    Ok(Some(examples))
}

/// The display names of the persons `ids`.
async fn person_names(store: &Store, ids: Vec<PersonId>) -> Result<HashMap<PersonId, String>, AppError> {
    let ids: Vec<String> = ids.iter().map(ToString::to_string).collect();
    let mut names = HashMap::new();
    for view in store.persons_by_ids(&ids).await? {
        if let (Some(id), Some(name)) = (view.person_id(), view.names().first()) {
            names.insert(id, name.display());
        }
    }
    Ok(names)
}

/// Families labelled by their partner pair (falling back to the `F####` id when no partner is named).
async fn family_examples(store: &Store, ids: &[String]) -> Result<Vec<UsingRecordRef>, AppError> {
    let views = in_order(store.families_by_ids(ids).await?, ids, |v| {
        v.family_id().map(|i| i.to_string())
    });
    let names = person_names(store, views.iter().flat_map(FamilyView::partners).collect()).await?;
    let mut examples = Vec::with_capacity(views.len());
    for view in &views {
        let partners: Vec<String> = view
            .partners()
            .into_iter()
            .filter_map(|partner| names.get(&partner).cloned())
            .collect();
        let label = (!partners.is_empty()).then(|| partners.join(" & "));
        examples.extend(refer(
            UsingKind::Family,
            view.family_id().map(|i| i.to_string()),
            view.human_id(),
            label,
        ));
    }
    Ok(examples)
}

/// Citations labelled by their cited source's title (plus page when present), falling back to the
/// `C####` id when the source is untitled.
async fn citation_examples(store: &Store, ids: &[String]) -> Result<Vec<UsingRecordRef>, AppError> {
    let views = in_order(store.citations_by_ids(ids).await?, ids, |v| {
        v.citation_id().map(|i| i.to_string())
    });
    let sources: Vec<String> = views
        .iter()
        .filter_map(|view| view.source_id().map(|id| id.to_string()))
        .collect();
    let mut titles = HashMap::new();
    for source in store.sources_by_ids(&sources).await? {
        if let (Some(id), Some(title)) = (source.source_id(), source.title()) {
            titles.insert(id, title.to_owned());
        }
    }
    let mut examples = Vec::with_capacity(views.len());
    for view in &views {
        let title = view.source_id().and_then(|source| titles.get(&source)).cloned();
        let label = match (title, view.page()) {
            (Some(title), Some(page)) => Some(format!("{title} — {page}")),
            (Some(title), None) => Some(title),
            (None, _) => None,
        };
        examples.extend(refer(
            UsingKind::Citation,
            view.citation_id().map(|i| i.to_string()),
            view.human_id(),
            label,
        ));
    }
    Ok(examples)
}

/// DNA tests labelled by their anchoring person's name, falling back to the `D####` id.
async fn dna_test_examples(store: &Store, ids: &[String]) -> Result<Vec<UsingRecordRef>, AppError> {
    let views = in_order(store.dna_tests_by_ids(ids).await?, ids, |v| {
        v.dna_test_id().map(|i| i.to_string())
    });
    let names = person_names(store, views.iter().filter_map(DnaTestView::person_id).collect()).await?;
    let mut examples = Vec::with_capacity(views.len());
    for view in &views {
        let label = view.person_id().and_then(|person| names.get(&person).cloned());
        examples.extend(refer(
            UsingKind::DnaTest,
            view.dna_test_id().map(|i| i.to_string()),
            view.human_id(),
            label,
        ));
    }
    Ok(examples)
}

/// Truncates a note body to its first line, trimmed and clipped to [`NOTE_SNIPPET_LEN`] characters,
/// as a readable example label. Returns `None` for an empty body (the `human_id` fallback applies).
pub(crate) fn note_snippet(text: &str) -> Option<String> {
    let first_line = text.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return None;
    }
    let snippet: String = first_line.chars().take(NOTE_SNIPPET_LEN).collect();
    if snippet.chars().count() < first_line.chars().count() {
        Some(format!("{snippet}…"))
    } else {
        Some(snippet)
    }
}

/// The file name (or web reference) of a media path, as a readable example label.
pub(crate) fn media_label(path: &MediaPath) -> Option<String> {
    match path {
        MediaPath::File(file) => file
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty())
            .map(str::to_owned),
        MediaPath::Web(url) => Some(url.href.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::{TagUsageGroup, note_snippet, usage};
    use crate::citation::{NewCitation, create_citation, tag_citation};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::dna_test::{NewDnaTest, create_dna_test, tag_dna_test};
    use crate::dto::UsingKind;
    use crate::family::{add_partner, create_family, tag_family};
    use crate::media::{NewMedia, create_media, tag_media};
    use crate::note::{NewNote, create_note, tag_note};
    use crate::person::{NewPerson, PersonNameParts, create_person, tag_person};
    use crate::research_note::{NewResearchNote, NewResearchNoteSubject, create_research_note, tag_research_note};
    use crate::session::Session;
    use crate::source::{NewSource, create_source};
    use crate::tag::create_tag;
    use crate::use_case::{MutationMeta, Provenance};
    use crate::workspace::Workspace;
    use tempfile::TempDir;
    use uuid::Uuid;
    use vitni_core::enums::EvidenceLevel;
    use vitni_core::ids::{AgentId, TagId};
    use vitni_core::provenance::{Agent, AgentKind};

    fn operator() -> OperatorConfig {
        OperatorConfig {
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
            email: None,
        }
    }

    fn session() -> Session {
        Session::new(Agent {
            kind: AgentKind::Human,
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
        })
    }

    async fn setup() -> (Workspace, Session, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
        let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
            .await
            .expect("open");
        (workspace, session(), dir)
    }

    fn name(given: &str, surname: &str) -> PersonNameParts {
        PersonNameParts::simple(Some(given.to_owned()), Some(surname.to_owned()))
    }

    fn example_label(groups: &[TagUsageGroup], kind: UsingKind) -> String {
        let group = groups
            .iter()
            .find(|g| g.kind == kind)
            .unwrap_or_else(|| panic!("no {kind:?} group"));
        group
            .examples
            .first()
            .expect("an example")
            .label
            .clone()
            .unwrap_or_else(|| panic!("{kind:?} example has no resolved label (would fall back to the id)"))
    }

    /// Creates + tags a person and a family, returning Alice's `human_id` (the DNA test's anchor).
    async fn seed_person_and_family_tagged(workspace: &Workspace, session: &Session, tag: &str) -> String {
        let new_person = |given: &str, surname: &str| NewPerson {
            human_id: None,
            name: Some(name(given, surname)),
            evidence_level: EvidenceLevel::Conclusion,
            external_ids: Vec::new(),
        };
        let alice = create_person(
            workspace,
            session,
            new_person("Alice", "Smith"),
            Provenance::default(),
            &[],
        )
        .await
        .expect("alice");
        let bob = create_person(
            workspace,
            session,
            new_person("Bob", "Jones"),
            Provenance::default(),
            &[],
        )
        .await
        .expect("bob");
        tag_person(workspace, session, &alice, tag, false, MutationMeta::default())
            .await
            .expect("tag person");

        let family = create_family(workspace, session, Provenance::default(), &[])
            .await
            .expect("family");
        add_partner(workspace, session, &family, &alice, MutationMeta::default())
            .await
            .expect("partner a");
        add_partner(workspace, session, &family, &bob, MutationMeta::default())
            .await
            .expect("partner b");
        tag_family(workspace, session, &family, tag, false, MutationMeta::default())
            .await
            .expect("tag family");
        alice
    }

    /// Creates + tags a source's citation.
    async fn seed_citation_tagged(workspace: &Workspace, session: &Session, tag: &str) {
        let source = create_source(
            workspace,
            session,
            NewSource {
                human_id: None,
                title: Some("Parish register".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("source");
        let citation = create_citation(
            workspace,
            session,
            NewCitation {
                human_id: None,
                source,
                page: Some("p. 14".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("citation");
        tag_citation(workspace, session, &citation, tag, false, MutationMeta::default())
            .await
            .expect("tag citation");
    }

    /// Creates + tags a media record and a note.
    async fn seed_media_and_note_tagged(workspace: &Workspace, session: &Session, tag: &str) {
        let media = create_media(
            workspace,
            session,
            NewMedia {
                human_id: None,
                path: Some("photos/ada.jpg".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("media");
        tag_media(workspace, session, &media, tag, false, MutationMeta::default())
            .await
            .expect("tag media");

        let note = create_note(
            workspace,
            session,
            NewNote {
                human_id: None,
                text: Some("Confirmed by baptism record".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("note");
        tag_note(workspace, session, &note, tag, false, MutationMeta::default())
            .await
            .expect("tag note");
    }

    /// Creates + tags a DNA test anchored on `person`.
    async fn seed_dna_test_tagged(workspace: &Workspace, session: &Session, tag: &str, person: String) {
        let dna_test = create_dna_test(
            workspace,
            session,
            NewDnaTest { human_id: None, person },
            Provenance::default(),
            &[],
        )
        .await
        .expect("dna test");
        tag_dna_test(workspace, session, &dna_test, tag, false, MutationMeta::default())
            .await
            .expect("tag dna test");
    }

    /// Creates + tags a research note about `person`.
    async fn seed_research_note_tagged(workspace: &Workspace, session: &Session, tag: &str, person: String) {
        let note = create_research_note(
            workspace,
            session,
            NewResearchNote {
                human_id: None,
                subjects: vec![NewResearchNoteSubject::Person(person)],
                title: Some("Who was Alice?".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("research note");
        tag_research_note(workspace, session, &note, tag, false, MutationMeta::default())
            .await
            .expect("tag research note");
    }

    /// Creates one record of every tag-bearing kind, applies `tag` to each, and returns `tag`'s id.
    /// Split into per-kind helpers so this — and each helper — stays under the 100-line limit.
    async fn seed_every_kind_tagged(workspace: &Workspace, session: &Session, tag: &str) -> TagId {
        let tag_id = TagId::from_uuid(Uuid::parse_str(tag).expect("uuid"));
        let alice = seed_person_and_family_tagged(workspace, session, tag).await;
        seed_citation_tagged(workspace, session, tag).await;
        seed_media_and_note_tagged(workspace, session, tag).await;
        seed_dna_test_tagged(workspace, session, tag, alice.clone()).await;
        seed_research_note_tagged(workspace, session, tag, alice).await;
        tag_id
    }

    #[tokio::test]
    async fn every_record_kind_resolves_a_human_readable_example_label() {
        let (workspace, session, _dir) = setup().await;
        let tag = create_tag(
            &workspace,
            &session,
            "Cross-cutting".to_owned(),
            Provenance::default(),
            &[],
        )
        .await
        .expect("tag");
        let tag_id = seed_every_kind_tagged(&workspace, &session, &tag).await;

        let (groups, count) = usage(&workspace, tag_id).await.expect("usage");

        assert_eq!(example_label(&groups, UsingKind::Person), "Alice Smith");
        assert_eq!(example_label(&groups, UsingKind::Family), "Alice Smith & Bob Jones");
        assert_eq!(example_label(&groups, UsingKind::Citation), "Parish register — p. 14");
        assert_eq!(example_label(&groups, UsingKind::Media), "ada.jpg");
        assert_eq!(example_label(&groups, UsingKind::Note), "Confirmed by baptism record");
        assert_eq!(example_label(&groups, UsingKind::DnaTest), "Alice Smith");
        assert_eq!(example_label(&groups, UsingKind::ResearchNote), "Who was Alice?");
        assert_eq!(count, 7, "one record of each of seven kinds");
        let kinds: Vec<UsingKind> = groups.iter().map(|group| group.kind).collect();
        assert_eq!(
            kinds,
            [
                UsingKind::Person,
                UsingKind::Family,
                UsingKind::Citation,
                UsingKind::Media,
                UsingKind::Note,
                UsingKind::ResearchNote,
                UsingKind::DnaTest,
            ],
            "in the list order of kinds"
        );

        // No example label leaks a raw UUID.
        for group in &groups {
            for example in &group.examples {
                if let Some(label) = &example.label {
                    assert!(
                        Uuid::parse_str(label).is_err(),
                        "example label {label:?} must not be a raw UUID"
                    );
                }
            }
        }
    }

    #[test]
    fn note_snippet_clips_to_the_first_line() {
        assert_eq!(note_snippet("First line\nSecond line").as_deref(), Some("First line"));
        assert_eq!(note_snippet("   \n").as_deref(), None);
        let long = "x".repeat(60);
        let snippet = note_snippet(&long).expect("snippet");
        assert!(snippet.ends_with('…'));
        assert_eq!(snippet.chars().count(), super::NOTE_SNIPPET_LEN + 1);
    }
}
