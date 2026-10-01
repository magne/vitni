//! Person clusters (ADR 0039 §4, §5): the read side of `PersonsMerged`.
//!
//! A merge links records rather than rewriting them, so every merged record keeps its own stream.
//! The `identity_links` index names each merged record's cluster root, and this module turns it into
//! the one redirect every reader applies: a member resolves to its root, lists hide members, and a
//! reference to a member names the root ([`PersonReferences`]).

use std::collections::HashMap;

use uuid::Uuid;
use vitni_core::ids::PersonId;
use vitni_core::matching::MatchableKind;
use vitni_core::person::PersonView;
use vitni_db::{DbError, Store};

use crate::error::AppError;

/// Every person cluster in the workspace, from the `identity_links` index.
#[derive(Debug, Default, Clone)]
pub(crate) struct PersonClusters {
    root_of: HashMap<PersonId, PersonId>,
    members_of: HashMap<PersonId, Vec<PersonId>>,
}

impl PersonClusters {
    /// Loads the person clusters.
    ///
    /// # Errors
    ///
    /// A store error, or [`DbError::Malformed`] if the index holds an id that is not a UUID.
    pub(crate) async fn load(store: &Store) -> Result<Self, AppError> {
        let mut root_of = HashMap::new();
        let mut members_of: HashMap<PersonId, Vec<PersonId>> = HashMap::new();
        for link in store.identity_links(MatchableKind::Person).await? {
            let (member, root) = (parse_person_id(&link.member)?, parse_person_id(&link.root)?);
            root_of.insert(member, root);
            members_of.entry(root).or_default().push(member);
        }
        Ok(Self { root_of, members_of })
    }

    /// The root of `id`'s cluster: `id` itself unless it is merged.
    pub(crate) fn root(&self, id: PersonId) -> PersonId {
        self.root_of.get(&id).copied().unwrap_or(id)
    }

    /// Whether `id` is merged into another record, and so hidden behind its root.
    pub(crate) fn is_member(&self, id: PersonId) -> bool {
        self.root_of.contains_key(&id)
    }

    /// How many records are merged into another.
    pub(crate) fn member_count(&self) -> usize {
        self.root_of.len()
    }

    /// The members of `root`'s cluster, not including the root, in id order.
    pub(crate) fn members(&self, root: PersonId) -> &[PersonId] {
        self.members_of.get(&root).map_or(&[], Vec::as_slice)
    }

    /// `root` followed by every member of its cluster.
    pub(crate) fn cluster(&self, root: PersonId) -> Vec<PersonId> {
        let mut cluster = vec![root];
        cluster.extend_from_slice(self.members(root));
        cluster
    }
}

/// Parses a person aggregate id read from the identity index.
fn parse_person_id(raw: &str) -> Result<PersonId, AppError> {
    let uuid =
        Uuid::parse_str(raw).map_err(|e| DbError::Malformed(format!("person id {raw:?} in identity index: {e}")))?;
    Ok(PersonId::from_uuid(uuid))
}

/// The projections of every record in `ids`, skipping any not found.
///
/// # Errors
///
/// A store error.
pub(crate) async fn person_views(store: &Store, ids: &[PersonId]) -> Result<Vec<PersonView>, AppError> {
    let mut views = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(human_id) = store.human_id_of("person", &id.to_string()).await? else {
            continue;
        };
        if let Some(view) = store.find_person(&human_id).await? {
            views.push(view);
        }
    }
    Ok(views)
}

/// How a reference names each person: a member by its root's id and `human_id`, everyone else by
/// their own (ADR 0039 §5).
pub(crate) struct PersonReferences {
    clusters: PersonClusters,
    human_ids: HashMap<PersonId, String>,
}

impl PersonReferences {
    /// Loads the clusters and every person's `human_id`.
    ///
    /// # Errors
    ///
    /// A store error.
    pub(crate) async fn load(store: &Store) -> Result<Self, AppError> {
        let mut human_ids = HashMap::new();
        for (id, human_id) in store.human_id_index("person").await? {
            human_ids.insert(parse_person_id(&id)?, human_id);
        }
        Ok(Self {
            clusters: PersonClusters::load(store).await?,
            human_ids,
        })
    }

    /// The id and `human_id` a reference to `id` names: its cluster root's.
    pub(crate) fn resolve(&self, id: PersonId) -> (PersonId, String) {
        let root = self.clusters.root(id);
        let human_id = self.human_ids.get(&root).cloned().unwrap_or_else(|| root.to_string());
        (root, human_id)
    }
}
