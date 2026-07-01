//! Counts how many other records still name a person's id after a merge (Phase 5 PR 19).
//!
//! `PersonsMerged` only records a same-as link on the survivor (data-model §9, `decide.rs`'s fold)
//! — it does not rewrite any Family partner/child slot or Person association/participation that
//! names the merged person. This scans the projections once and counts those still-live references,
//! so the merge screen can report "N other records still reference the merged persona" instead of
//! fabricating a "relationships re-pointed" count.

use genealogy_core::ids::PersonId;

use crate::error::AppError;
use crate::family::list_families;
use crate::person::list_persons;
use crate::workspace::Workspace;

/// Counts family partner/child slots and person associations that still name `person`.
///
/// Deliberately does not resolve `person` to a `human_id` first: a merged person's own record is
/// untouched by the merge, so their id keeps resolving and this counts exactly what still points at
/// it, regardless of which side of a merge `person` is.
///
/// # Errors
///
/// A store/read-model error from the underlying `list_families`/`list_persons` scans.
pub(crate) async fn count_references(workspace: &Workspace, person: PersonId) -> Result<usize, AppError> {
    let id = person.to_string();
    let mut count = 0;

    for family in list_families(workspace).await? {
        count += family.partners.iter().filter(|partner| partner.id == id).count();
        count += family.children.iter().filter(|child| child.id == id).count();
    }
    for summary in list_persons(workspace).await? {
        count += summary
            .associations
            .iter()
            .filter(|association| association.other.id == id)
            .count();
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::count_references;
    use crate::config::{AppDefaults, IdFormats, OperatorConfig, WorkspaceDefaults};
    use crate::family::{add_partner, create_family};
    use crate::person::{NewPerson, create_person};
    use crate::session::Session;
    use crate::workspace::Workspace;
    use genealogy_core::enums::{AssociationRole, EvidenceLevel};
    use genealogy_core::ids::AgentId;
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

    fn defaults() -> WorkspaceDefaults {
        WorkspaceDefaults {
            id_formats: IdFormats {
                person: "I%04d".to_owned(),
                family: "F%04d".to_owned(),
                ..IdFormats::default()
            },
            ..Default::default()
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
        let workspace = Workspace::open(&ws, &operator(), &defaults()).await.expect("open");
        (workspace, session(), dir)
    }

    async fn bare_person(workspace: &Workspace, session: &Session) -> String {
        create_person(
            workspace,
            session,
            NewPerson {
                human_id: None,
                name: None,
                evidence_level: EvidenceLevel::Conclusion,
            },
        )
        .await
        .expect("create")
    }

    #[tokio::test]
    async fn counts_zero_for_a_person_nobody_references() {
        let (workspace, session, _dir) = setup().await;
        let solo = bare_person(&workspace, &session).await;

        let found = workspace
            .store()
            .find_person(&solo)
            .await
            .expect("find")
            .expect("exists");
        let id = found.person_id().expect("id");
        let count = count_references(&workspace, id).await.expect("count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn counts_a_family_partner_slot() {
        let (workspace, session, _dir) = setup().await;
        let person = bare_person(&workspace, &session).await;
        let family = create_family(&workspace, &session).await.expect("create family");
        add_partner(&workspace, &session, &family, &person)
            .await
            .expect("add partner");

        let found = workspace
            .store()
            .find_person(&person)
            .await
            .expect("find")
            .expect("exists");
        let id = found.person_id().expect("id");
        let count = count_references(&workspace, id).await.expect("count");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn counts_a_person_association() {
        let (workspace, session, _dir) = setup().await;
        let a = bare_person(&workspace, &session).await;
        let b = bare_person(&workspace, &session).await;
        crate::person::assert_association(&workspace, &session, &a, &b, AssociationRole::Godparent)
            .await
            .expect("associate");

        let found = workspace.store().find_person(&b).await.expect("find").expect("exists");
        let id = found.person_id().expect("id");
        let count = count_references(&workspace, id).await.expect("count");
        assert_eq!(count, 1, "b is referenced by a's association");
    }
}
