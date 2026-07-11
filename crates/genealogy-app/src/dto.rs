//! Shared cross-aggregate reference DTOs and their joins (ADR 0006).
//!
//! `*Summary` DTOs reference related aggregates by their stable `id` **and** their user-facing
//! `human_id`, so a frontend can navigate by the stable id rather than the mutable `human_id` (the
//! cross-aggregate-joins dependency note; ADR 0004 §2). These types are shared by more than one
//! aggregate's summary (family/event/place), so they live here rather than in any one module.

use std::collections::HashMap;

use genealogy_core::date::GenealogicalDate;
use genealogy_core::enums::{AssociationRole, FactType, ParticipantRole, SourceMediaType};
use genealogy_core::ids::{CitationId, MediaId, RepositoryId, TagId};
use genealogy_core::provenance::{Agent, AgentKind, Confidence, EvidenceAnalysis, Timestamp};
use genealogy_db::Store;

use crate::citation::TagRef;
use crate::error::AppError;
use crate::history::{OperatorKind, operator_kind};
use crate::person::PersonSummary;

/// A reference to a related aggregate, carrying both its user-facing `human_id` (the display label)
/// and its stable aggregate `id` (a UUID string) so a frontend can join/navigate by the stable id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggRef {
    /// The referenced aggregate's user-facing identifier (e.g. `C0001`).
    pub human_id: String,
    /// The referenced aggregate's stable id (a UUID string) — the join/navigation key.
    pub id: String,
}

/// A media object attached to an aggregate, with its per-use caption for the gallery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRefSummary {
    /// The media object's user-facing identifier (e.g. `O0001`).
    pub human_id: String,
    /// The media object's stable `MediaId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The per-use caption, if set.
    pub caption: Option<String>,
    /// The `AssertionId` (a UUID string) of the attach assertion — the target a Detach retracts
    /// (ADR 0004 §2). Never rendered; used only to build the detachment command.
    pub assertion_id: String,
}

/// A note or other record attached to an aggregate at the record level, carrying its `human_id` +
/// stable id (like [`AggRef`]) plus the `AssertionId` of the attach assertion so a Detach can
/// retract exactly that attachment (ADR 0004 §2). Distinct from [`AggRef`], which references a
/// related aggregate that carries no per-attach assertion (an association target, a merged persona).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachedRef {
    /// The attached record's user-facing identifier (e.g. `N0001`).
    pub human_id: String,
    /// The attached record's stable id (a UUID string) — the join/navigation key.
    pub id: String,
    /// The `AssertionId` (a UUID string) of the attach assertion — the Detach target. Never rendered.
    pub assertion_id: String,
}

/// A citation attached to an aggregate, joined to the Citation (and its Source) projection so the
/// detail tabs can render source · page · surety · the Evidence Explained axes, while navigating by
/// the stable citation/source ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationRef {
    /// The citation's user-facing identifier (e.g. `C0001`).
    pub human_id: String,
    /// The citation's stable `CitationId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The `AssertionId` (a UUID string) of the attach assertion when this ref is an owner's own
    /// attached citation — the target a Detach retracts (ADR 0004 §2). `None` in the shared
    /// citation lookup and wherever a citation is shown as evidence (a fact's backing citations),
    /// not as a detachable attachment; each owner stamps the per-attach id. Never rendered.
    pub assertion_id: Option<String>,
    /// The cited source (its `human_id` + stable id), for display and navigation.
    pub source: Option<AggRef>,
    /// The cited source's title, for display.
    pub source_title: Option<String>,
    /// The page / locator within the source, if set.
    pub page: Option<String>,
    /// The operator's confidence in this citation. Structured so the frontend localizes it.
    pub confidence: Option<Confidence>,
    /// The citation's Evidence Explained analysis (the three axes), if set.
    pub analysis: Option<EvidenceAnalysis>,
    /// The display name of the operator who created the citation (the "asserted by" provenance),
    /// falling back to a software/AI agent's name when it has no display name.
    pub asserted_by: Option<String>,
    /// The kind of operator who created the citation (human / software / AI), so the frontend can
    /// annotate a non-human "asserted by" line (finding 7, ADR 0021 §4).
    pub asserted_by_kind: Option<OperatorKind>,
    /// When the citation was created, as an RFC 3339 string (the frontend renders it friendlily).
    pub asserted_at: Option<String>,
}

/// Splits the creating operator into its display label and kind for [`CitationRef`]: the label is the
/// agent's display name, falling back to a software/AI agent's registered name (a human without a
/// display name has no label). Returns `(None, None)` for an un-created citation (finding 7).
fn creation_agent_fields(agent: Option<&Agent>) -> (Option<String>, Option<OperatorKind>) {
    let Some(agent) = agent else {
        return (None, None);
    };
    let label = agent.display.clone().or_else(|| match &agent.kind {
        AgentKind::Human => None,
        AgentKind::Software { name, .. } | AgentKind::AiModel { name, .. } => Some(name.clone()),
    });
    (label, Some(operator_kind(&agent.kind)))
}

/// A repository a source is held in, joined to the Repository projection: the repo's name + stable
/// id for navigation, the per-link call number and medium, and the link's surety + backing-citation
/// count (the evidence-first cue on the Source › Repositories rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryLinkRef {
    /// The repository (its `human_id` + stable id), if it is still projected.
    pub repository: Option<AggRef>,
    /// The repository's name, for display.
    pub name: Option<String>,
    /// The source's call number / shelf mark in this repository, if recorded.
    pub call_number: Option<String>,
    /// How the source is held here (book, film, electronic, …).
    pub media_type: SourceMediaType,
    /// The operator's surety in the link.
    pub confidence: Option<Confidence>,
    /// How many citations back the link assertion.
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) that introduced this repository link — the target a per-row
    /// Edit supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// A source held by a repository, joined to the Source projection: the source's title + stable id
/// for navigation, the per-link call number and medium, and how many citations cite the source (the
/// Repository › Sources rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLinkRef {
    /// The source (its `human_id` + stable id).
    pub source: AggRef,
    /// The source's title, for display.
    pub title: Option<String>,
    /// The source's call number / shelf mark in this repository, if recorded.
    pub call_number: Option<String>,
    /// How the source is held here (book, film, electronic, …).
    pub media_type: SourceMediaType,
    /// How many citations cite this source.
    pub citation_count: usize,
}

/// The aggregate kind a citation is used by — drives the navigation route and the row avatar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitingKind {
    /// A Person record.
    Person,
    /// A Family record.
    Family,
    /// An Event record.
    Event,
    /// A Place record.
    Place,
}

/// Where within a citing record a citation is attached — the sub-context the UI localizes (e.g. a
/// person's "Birth" fact, an event participant role). Structured (not a label) so the frontend
/// localizes it (ADR 0003).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitingContext {
    /// The citation backs the record itself (a row-level `SOUR`).
    Record,
    /// A name assertion on a person.
    Name,
    /// A single-person fact (birth, death, occupation, …).
    Fact(FactType),
    /// A person-to-person association.
    Association(AssociationRole),
    /// A person's participation in an event.
    Participant(ParticipantRole),
    /// A family partnership.
    Partner,
    /// A child-in-family relationship.
    Child,
    /// A family event link (e.g. a marriage).
    FamilyEvent,
    /// A place's type assertion.
    PlaceType,
}

/// A record that uses a citation — the Source › Citations "Backs record" cell. Carries the citing
/// aggregate's kind + stable id for navigation, its display label, and the sub-context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitingRecordRef {
    /// The citing aggregate's kind.
    pub kind: CitingKind,
    /// The citing record's user-facing identifier.
    pub human_id: String,
    /// The citing record's stable id (a UUID string) — the join/navigation key.
    pub id: String,
    /// The citing record's display label (a person/place name, an event description), if available.
    pub label: Option<String>,
    /// Where within the record the citation is attached.
    pub context: CitingContext,
}

/// The aggregate kind that *uses* a media object or note, or *carries* a tag — the inverse of an
/// attachment. Drives the navigation route and the row chip on the Media "Used by" card, the Note
/// "References" tab, and the Tag "Usage" tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsingKind {
    /// A Person record.
    Person,
    /// A Family record.
    Family,
    /// An Event record.
    Event,
    /// A Place record.
    Place,
    /// A Source record.
    Source,
    /// A Citation record.
    Citation,
    /// A Repository record.
    Repository,
    /// A Media record.
    Media,
    /// A Note record.
    Note,
    /// A DNA test record.
    DnaTest,
    /// A DNA match record.
    DnaMatch,
}

/// A record that references a media object or note — one row on the Media "Used by" card or the Note
/// "References" tab. Carries the referencing aggregate's kind + stable id for navigation and its
/// display label. Media/notes attach at the record level, so no sub-context is carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsingRecordRef {
    /// The referencing aggregate's kind.
    pub kind: UsingKind,
    /// The referencing record's user-facing identifier.
    pub human_id: String,
    /// The referencing record's stable id (a UUID string) — the join/navigation key.
    pub id: String,
    /// The referencing record's display label (a person/place name, an event description), if any.
    pub label: Option<String>,
}

/// A citation that uses a source, joined to its backing records — one row group in the Source ›
/// Citations tab (the citation's page/surety/evidence + the records it backs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCitationRef {
    /// The citation (page · surety · evidence axes · stable id).
    pub citation: CitationRef,
    /// The records that use this citation (the reverse index).
    pub backers: Vec<CitingRecordRef>,
}

/// The aggregated reliability of a source, derived from the citations that point at it (the Source ›
/// Overview "Reliability" card). The modal surety + evidence axes across its citation set, plus how
/// many citations and distinct records use it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceReliability {
    /// The most common surety across the source's citations, if any are confident.
    pub typical_surety: Option<Confidence>,
    /// The modal Evidence Explained analysis across the source's citations, if any analysed.
    pub evidence: Option<EvidenceAnalysis>,
    /// How many citations cite this source.
    pub citation_count: usize,
    /// How many distinct records use those citations.
    pub record_count: usize,
}

/// Builds a `RepositoryId -> (human_id, name)` lookup from the Repository projection, so a source's
/// repository links resolve to a name + stable id without a per-link query.
pub(crate) async fn repository_refs(
    store: &Store,
) -> Result<HashMap<RepositoryId, (String, Option<String>)>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_repositories().await? {
        if let (Some(id), Some(human_id)) = (view.repository_id(), view.human_id()) {
            map.insert(id, (human_id.as_str().to_owned(), view.name().map(ToOwned::to_owned)));
        }
    }
    Ok(map)
}

/// Builds a `CitationId -> CitationRef` lookup from the Citation projection, joined to the Source
/// projection for the source label, so an owner's attached citations resolve to a full row without a
/// per-citation query (the cross-aggregate join lives here — the app/db layer).
pub(crate) async fn citation_refs(store: &Store) -> Result<HashMap<CitationId, CitationRef>, AppError> {
    let sources: HashMap<_, (String, Option<String>)> = store
        .list_sources()
        .await?
        .iter()
        .filter_map(|s| {
            Some((
                s.source_id()?,
                (s.human_id()?.as_str().to_owned(), s.title().map(ToOwned::to_owned)),
            ))
        })
        .collect();

    let mut map = HashMap::new();
    for view in store.list_citations().await? {
        let (Some(id), Some(human_id)) = (view.citation_id(), view.human_id()) else {
            continue;
        };
        let source = view.source_id().and_then(|sid| {
            sources.get(&sid).map(|(human, _)| AggRef {
                human_id: human.clone(),
                id: sid.to_string(),
            })
        });
        let source_title = view
            .source_id()
            .and_then(|sid| sources.get(&sid))
            .and_then(|(_, title)| title.clone());
        let (asserted_by, asserted_by_kind) = creation_agent_fields(view.created_by());
        map.insert(
            id,
            CitationRef {
                human_id: human_id.as_str().to_owned(),
                id: id.to_string(),
                assertion_id: None,
                source,
                source_title,
                page: view.page().map(ToOwned::to_owned),
                confidence: view.confidence().copied(),
                analysis: view.evidence_analysis().copied(),
                asserted_by,
                asserted_by_kind,
                asserted_at: view.created_at().and_then(timestamp_to_rfc3339),
            },
        );
    }
    Ok(map)
}

/// The representative year of an asserted fact of `fact_type`, if its date carries one. Shared by
/// any join that needs a person's lifespan (family partners/children, the pedigree traversal).
pub(crate) fn year_of_fact(summary: &PersonSummary, fact_type: &FactType) -> Option<i32> {
    summary
        .facts
        .iter()
        .find(|fact| fact.fact.fact_type == *fact_type)
        .and_then(|fact| fact.fact.date.as_ref())
        .and_then(year_of)
}

/// The representative year of a date (from its integer sort key), or `None` for an undated/text date.
fn year_of(date: &GenealogicalDate) -> Option<i32> {
    let year = date.sort_value / 10_000;
    (year != 0).then(|| i32::try_from(year).unwrap_or_default())
}

/// Renders a "born – died" lifespan from the known birth/death years (either side may be absent).
pub(crate) fn lifespan(birth: Option<i32>, death: Option<i32>) -> Option<String> {
    match (birth, death) {
        (None, None) => None,
        (Some(b), None) => Some(format!("{b} – ")),
        (None, Some(d)) => Some(format!(" – {d}")),
        (Some(b), Some(d)) => Some(format!("{b} – {d}")),
    }
}

/// Renders a core [`Timestamp`] as its RFC 3339 string (the frontend renders it friendlily). `None`
/// only on the impossible case of a non-string serialization.
fn timestamp_to_rfc3339(at: Timestamp) -> Option<String> {
    serde_json::to_value(at)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
}

/// Loads a `MediaId -> (human_id, id)` lookup from the Media projection; the per-use caption is
/// supplied by the owning aggregate's `MediaRef`, not the media object itself.
pub(crate) async fn media_refs(store: &Store) -> Result<HashMap<MediaId, (String, String)>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_media().await? {
        if let (Some(id), Some(human_id)) = (view.media_id(), view.human_id()) {
            map.insert(id, (human_id.as_str().to_owned(), id.to_string()));
        }
    }
    Ok(map)
}

/// Builds a `TagId -> TagRef` lookup from the Tag projection, to render applied tags by name/colour/
/// priority (never by id — data-model §9).
pub(crate) async fn tag_refs(store: &Store) -> Result<HashMap<TagId, TagRef>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_tags().await? {
        if let (Some(id), Some(name)) = (view.tag_id(), view.name()) {
            map.insert(
                id,
                TagRef {
                    id: id.to_string(),
                    name: name.to_owned(),
                    color: view.color().map(ToOwned::to_owned),
                    priority: view.priority(),
                },
            );
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::creation_agent_fields;
    use crate::history::OperatorKind;
    use genealogy_core::ids::AgentId;
    use genealogy_core::provenance::{Agent, AgentKind};
    use uuid::Uuid;

    #[test]
    fn creation_agent_fields_none_when_uncreated() {
        assert_eq!(creation_agent_fields(None), (None, None));
    }

    #[test]
    fn creation_agent_fields_falls_back_to_software_name() {
        // A display-less software agent surfaces its registered name and its kind (finding 7).
        let agent = Agent {
            kind: AgentKind::Software {
                name: "genealogy-import".to_owned(),
                version: "1.0".to_owned(),
            },
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: None,
        };
        let (label, kind) = creation_agent_fields(Some(&agent));
        assert_eq!(label.as_deref(), Some("genealogy-import"));
        assert_eq!(kind, Some(OperatorKind::Software));
    }

    #[test]
    fn creation_agent_fields_prefers_display_name() {
        let agent = Agent {
            kind: AgentKind::Human,
            id: AgentId::from_uuid(Uuid::from_u128(2)),
            display: Some("magne".to_owned()),
        };
        let (label, kind) = creation_agent_fields(Some(&agent));
        assert_eq!(label.as_deref(), Some("magne"));
        assert_eq!(kind, Some(OperatorKind::Human));
    }
}
