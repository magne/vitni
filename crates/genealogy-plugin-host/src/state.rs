//! Per-instance host state and the capability implementations behind the WIT interfaces.
//!
//! Each capability checks [`Grants`] before acting (deny-by-default, ADR 0011 §2) and drives the
//! `genealogy-app` use-cases through a [`Session`] whose operator is `AgentKind::Software`, so every
//! plugin-authored change is audited as a Software operator (ADR 0007 §7).

use genealogy_app::{NewPerson, Session, Workspace};
use genealogy_core::enums::{ChildParentRelationship, EvidenceLevel};
use wasmtime::StoreLimits;
use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

use crate::bindings::imports::genealogy::host_api::{commands, log, query, types};
use crate::capability::{Capability, Grants};

/// The data owned by one plugin instance's Wasmtime store.
pub struct HostState {
    wasi: WasiCtx,
    table: ResourceTable,
    /// Memory/instance caps enforced by Wasmtime (ADR 0011 §4).
    pub limits: StoreLimits,
    grants: Grants,
    workspace: Workspace,
    session: Session,
}

impl HostState {
    /// Builds instance state. `wasi` is the (empty, in the spike) WASI context that denies
    /// `files`/`net` by construction (ADR 0011 §3); `limits` is the memory cap.
    pub fn new(wasi: WasiCtx, limits: StoreLimits, grants: Grants, workspace: Workspace, session: Session) -> Self {
        Self {
            wasi,
            table: ResourceTable::new(),
            limits,
            grants,
            workspace,
            session,
        }
    }

    /// Recovers the workspace once the instance has run (the store is consumed afterwards).
    pub fn into_workspace(self) -> Workspace {
        self.workspace
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// Maps an application error onto the capability-error a guest sees. Domain rejections and missing
/// references are `invalid-input`; infrastructure failures are `backend`.
fn to_capability_error(error: &genealogy_app::AppError) -> types::CapabilityError {
    use genealogy_app::AppError;
    match error {
        AppError::Db(_) | AppError::Config(_) | AppError::Workspace(_) => {
            types::CapabilityError::Backend(error.to_string())
        }
        _ => types::CapabilityError::InvalidInput(error.to_string()),
    }
}

impl types::Host for HostState {}

impl log::Host for HostState {
    async fn log(&mut self, level: log::Level, message: String) {
        if !self.grants.allows(Capability::Log) {
            return;
        }
        match level {
            log::Level::Debug => tracing::debug!(target: "plugin", "{message}"),
            log::Level::Info => tracing::info!(target: "plugin", "{message}"),
            log::Level::Warn => tracing::warn!(target: "plugin", "{message}"),
            log::Level::Error => tracing::error!(target: "plugin", "{message}"),
        }
    }
}

impl commands::Host for HostState {
    async fn create_person(
        &mut self,
        given: Option<String>,
        surname: Option<String>,
    ) -> Result<String, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        let new = NewPerson {
            human_id: None,
            given,
            surname,
            evidence_level: EvidenceLevel::Persona,
        };
        genealogy_app::create_person(&self.workspace, &self.session, new)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn create_family(&mut self) -> Result<String, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::create_family(&self.workspace, &self.session)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn add_partner(&mut self, family: String, person: String) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::add_partner(&self.workspace, &self.session, &family, &person)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn add_child(&mut self, family: String, child: String) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::add_child(
            &self.workspace,
            &self.session,
            &family,
            &child,
            ChildParentRelationship::Birth,
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }
}

impl query::Host for HostState {
    async fn list_persons(&mut self) -> Result<Vec<types::PersonDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let persons = genealogy_app::list_persons(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(persons
            .into_iter()
            .map(|person| types::PersonDto {
                human_id: person.human_id,
                given: person.given,
                surname: person.surname,
            })
            .collect())
    }

    async fn list_families(&mut self) -> Result<Vec<types::FamilyDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let families = genealogy_app::list_families(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(families
            .into_iter()
            .map(|family| types::FamilyDto {
                human_id: family.human_id,
                partners: family.partners,
                children: family.children,
            })
            .collect())
    }
}
