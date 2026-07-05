//! The reverse tag index (Phase 5 PR 11): given a tag, which records carry it, grouped by object
//! type with a count and a few examples (the Tag › Usage tab).
//!
//! Tags are applied on the *tagged* aggregate, not stored inversely on the Tag, so this scans every
//! tag-bearing projection once (person, family, event, place, source, citation, repository, media,
//! note, `dna_test`, `dna_match`) and inverts `view.tags()` to a `TagId -> [UsingRecordRef]` map. The
//! join lives in the app/db layer (the cross-aggregate-joins dependency note).

use std::collections::HashMap;

use genealogy_core::ids::{PersonId, SourceId, TagId};
use genealogy_core::media_path::MediaPath;

use crate::dto::{UsingKind, UsingRecordRef};
use crate::error::AppError;
use crate::person::list_persons;
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

/// A `TagId -> [UsingRecordRef]` reverse index over the tag-bearing aggregates.
pub(crate) struct TagUsage {
    by_tag: HashMap<TagId, Vec<UsingRecordRef>>,
}

impl TagUsage {
    /// Scans every tag-bearing projection and inverts the applications.
    pub(crate) async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let person_names: HashMap<String, String> = list_persons(workspace)
            .await?
            .into_iter()
            .filter_map(|p| p.display_name.map(|name| (p.human_id, name)))
            .collect();
        let lookups = Lookups::load(workspace, &person_names).await?;

        let mut by_tag: HashMap<TagId, Vec<UsingRecordRef>> = HashMap::new();
        scan_persons(workspace, &person_names, &mut by_tag).await?;
        scan_families(workspace, &lookups, &mut by_tag).await?;
        scan_events(workspace, &mut by_tag).await?;
        scan_places(workspace, &mut by_tag).await?;
        scan_sources(workspace, &mut by_tag).await?;
        scan_citations(workspace, &lookups, &mut by_tag).await?;
        scan_repositories(workspace, &mut by_tag).await?;
        scan_media(workspace, &mut by_tag).await?;
        scan_notes(workspace, &mut by_tag).await?;
        scan_dna_tests(workspace, &lookups, &mut by_tag).await?;
        scan_dna_matches(workspace, &mut by_tag).await?;
        Ok(Self { by_tag })
    }

    /// How many records carry `tag` in total (across every object type).
    pub(crate) fn count(&self, tag: TagId) -> usize {
        self.by_tag.get(&tag).map_or(0, Vec::len)
    }

    /// The records carrying `tag`, grouped by object type (in scan order) with counts and examples.
    pub(crate) fn groups(&self, tag: TagId) -> Vec<TagUsageGroup> {
        let Some(records) = self.by_tag.get(&tag) else {
            return Vec::new();
        };
        let mut groups: Vec<TagUsageGroup> = Vec::new();
        for record in records {
            match groups.iter_mut().find(|g| g.kind == record.kind) {
                Some(group) => {
                    group.count += 1;
                    if group.examples.len() < MAX_EXAMPLES {
                        group.examples.push(record.clone());
                    }
                }
                None => groups.push(TagUsageGroup {
                    kind: record.kind,
                    count: 1,
                    examples: vec![record.clone()],
                }),
            }
        }
        groups
    }
}

/// Pushes one carrying record onto a tag's bucket.
fn push(map: &mut HashMap<TagId, Vec<UsingRecordRef>>, tag: TagId, record: UsingRecordRef) {
    map.entry(tag).or_default().push(record);
}

/// The cross-aggregate lookups the scans need to resolve a human-readable example label (so no bare
/// `human_id` — and never a UUID — is surfaced): partner/anchor names for families and DNA tests, and
/// cited-source titles for citations.
struct Lookups {
    /// `PersonId -> display name`, so a family renders its partner pair and a DNA test its anchor.
    person_names: HashMap<PersonId, String>,
    /// `SourceId -> title`, so a citation renders its cited source rather than its `C####` id.
    source_titles: HashMap<SourceId, String>,
}

impl Lookups {
    async fn load(workspace: &Workspace, person_names_by_human_id: &HashMap<String, String>) -> Result<Self, AppError> {
        let mut person_names = HashMap::new();
        for view in workspace.store().list_persons().await? {
            if let (Some(id), Some(human_id)) = (view.person_id(), view.human_id())
                && let Some(name) = person_names_by_human_id.get(human_id.as_str())
            {
                person_names.insert(id, name.clone());
            }
        }
        let mut source_titles = HashMap::new();
        for view in workspace.store().list_sources().await? {
            if let (Some(id), Some(title)) = (view.source_id(), view.title()) {
                source_titles.insert(id, title.to_owned());
            }
        }
        Ok(Self {
            person_names,
            source_titles,
        })
    }
}

/// Truncates a note body to its first line, trimmed and clipped to [`NOTE_SNIPPET_LEN`] characters,
/// as a readable example label. Returns `None` for an empty body (the `human_id` fallback applies).
fn note_snippet(text: &str) -> Option<String> {
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
fn media_label(path: &MediaPath) -> Option<String> {
    match path {
        MediaPath::File(file) => file
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty())
            .map(str::to_owned),
        MediaPath::Web(url) => Some(url.href.clone()),
    }
}

/// Inverts person tag applications.
async fn scan_persons(
    workspace: &Workspace,
    person_names: &HashMap<String, String>,
    map: &mut HashMap<TagId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_persons().await? {
        let (Some(id), Some(human_id)) = (view.person_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = person_names.get(&human_id).cloned();
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::Person,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts family tag applications, labelling each by its partner pair (falling back to the `F####`
/// id when a partner is unnamed).
async fn scan_families(
    workspace: &Workspace,
    lookups: &Lookups,
    map: &mut HashMap<TagId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_families().await? {
        let (Some(id), Some(human_id)) = (view.family_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let names: Vec<String> = view
            .partners()
            .into_iter()
            .filter_map(|partner| lookups.person_names.get(&partner).cloned())
            .collect();
        let label = if names.is_empty() {
            None
        } else {
            Some(names.join(" & "))
        };
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::Family,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts event tag applications.
async fn scan_events(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_events().await? {
        let (Some(id), Some(human_id)) = (view.event_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.description().map(ToOwned::to_owned);
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::Event,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts place tag applications.
async fn scan_places(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_places().await? {
        let (Some(id), Some(human_id)) = (view.place_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.names().first().map(|n| n.text.clone());
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::Place,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts source tag applications.
async fn scan_sources(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_sources().await? {
        let (Some(id), Some(human_id)) = (view.source_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.title().map(ToOwned::to_owned);
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::Source,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts citation tag applications, labelling each by its cited source's title (plus page when
/// present), falling back to the `C####` id when the source is untitled.
async fn scan_citations(
    workspace: &Workspace,
    lookups: &Lookups,
    map: &mut HashMap<TagId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_citations().await? {
        let (Some(id), Some(human_id)) = (view.citation_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let title = view
            .source_id()
            .and_then(|source| lookups.source_titles.get(&source))
            .cloned();
        let label = match (title, view.page()) {
            (Some(title), Some(page)) => Some(format!("{title} — {page}")),
            (Some(title), None) => Some(title),
            (None, _) => None,
        };
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::Citation,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts repository tag applications.
async fn scan_repositories(
    workspace: &Workspace,
    map: &mut HashMap<TagId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_repositories().await? {
        let (Some(id), Some(human_id)) = (view.repository_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.name().map(ToOwned::to_owned);
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::Repository,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts media tag applications, labelling each by its file name (or web reference), falling back
/// to the `O####` id when the media has no path yet.
async fn scan_media(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_media().await? {
        let (Some(id), Some(human_id)) = (view.media_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.path().and_then(media_label);
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::Media,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts note tag applications, labelling each by the first line of its body, falling back to the
/// `N####` id when the note is empty.
async fn scan_notes(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_notes().await? {
        let (Some(id), Some(human_id)) = (view.note_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.text().and_then(|text| note_snippet(&text.text));
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::Note,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts DNA-test tag applications, labelling each by its anchoring person's name, falling back to
/// the `D####` id when the anchor is unnamed.
async fn scan_dna_tests(
    workspace: &Workspace,
    lookups: &Lookups,
    map: &mut HashMap<TagId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_dna_tests().await? {
        let (Some(id), Some(human_id)) = (view.dna_test_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view
            .person_id()
            .and_then(|person| lookups.person_names.get(&person).cloned());
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::DnaTest,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts DNA-match tag applications.
async fn scan_dna_matches(
    workspace: &Workspace,
    map: &mut HashMap<TagId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_dna_matches().await? {
        let (Some(id), Some(human_id)) = (view.dna_match_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.predicted_relationship().map(ToOwned::to_owned);
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::DnaMatch,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{TagUsage, note_snippet};
    use crate::citation::{NewCitation, create_citation, tag_citation};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::dna_test::{NewDnaTest, create_dna_test, tag_dna_test};
    use crate::dto::UsingKind;
    use crate::family::{add_partner, create_family, tag_family};
    use crate::media::{NewMedia, create_media, tag_media};
    use crate::note::{NewNote, create_note, tag_note};
    use crate::person::{NewPerson, PersonNameParts, create_person, tag_person};
    use crate::session::Session;
    use crate::source::{NewSource, create_source};
    use crate::tag::create_tag;
    use crate::use_case::{MutationMeta, Provenance};
    use crate::workspace::Workspace;
    use genealogy_core::enums::EvidenceLevel;
    use genealogy_core::ids::{AgentId, TagId};
    use genealogy_core::provenance::{Agent, AgentKind};
    use tempfile::TempDir;
    use uuid::Uuid;

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

    fn example_label(usage: &TagUsage, tag: TagId, kind: UsingKind) -> String {
        let groups = usage.groups(tag);
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
    async fn seed_person_and_family_tagged(
        workspace: &Workspace,
        session: &Session,
        tag_id: TagId,
        tag: &str,
    ) -> String {
        let new_person = |given: &str, surname: &str| NewPerson {
            human_id: None,
            name: Some(name(given, surname)),
            evidence_level: EvidenceLevel::Conclusion,
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
        tag_person(workspace, session, &alice, tag_id, false, MutationMeta::default())
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

    /// Creates one record of every tag-bearing kind, applies `tag` to each, and returns `tag`'s id.
    /// Split into per-kind helpers so this — and each helper — stays under the 100-line limit.
    async fn seed_every_kind_tagged(workspace: &Workspace, session: &Session, tag: &str) -> TagId {
        let tag_id = TagId::from_uuid(Uuid::parse_str(tag).expect("uuid"));
        let alice = seed_person_and_family_tagged(workspace, session, tag_id, tag).await;
        seed_citation_tagged(workspace, session, tag).await;
        seed_media_and_note_tagged(workspace, session, tag).await;
        seed_dna_test_tagged(workspace, session, tag, alice).await;
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

        let usage = TagUsage::load(&workspace).await.expect("usage");

        assert_eq!(example_label(&usage, tag_id, UsingKind::Person), "Alice Smith");
        assert_eq!(
            example_label(&usage, tag_id, UsingKind::Family),
            "Alice Smith & Bob Jones"
        );
        assert_eq!(
            example_label(&usage, tag_id, UsingKind::Citation),
            "Parish register — p. 14"
        );
        assert_eq!(example_label(&usage, tag_id, UsingKind::Media), "ada.jpg");
        assert_eq!(
            example_label(&usage, tag_id, UsingKind::Note),
            "Confirmed by baptism record"
        );
        assert_eq!(example_label(&usage, tag_id, UsingKind::DnaTest), "Alice Smith");

        // No example label leaks a raw UUID.
        for group in usage.groups(tag_id) {
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
