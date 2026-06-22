//! Per-instance host state and the capability implementations behind the WIT interfaces.
//!
//! Each capability checks [`Grants`] before acting (deny-by-default, ADR 0011 §2) and drives the
//! `genealogy-app` use-cases through a [`Session`] whose operator is `AgentKind::Software`, so every
//! plugin-authored change is audited as a Software operator (ADR 0007 §7).
//!
//! The bulk capabilities (`progress`, `import-source`, `export-sink`, ADR 0013) let a long-running
//! import/export report how far it has advanced and stream its document through a host-owned file
//! handle. The host owns the path: a plugin reads from the source the frontend selected and writes
//! to a destination the host resolves from the plugin's suggested file name. One source and one sink
//! back each instance — a plugin runs exactly one import or one export.

use std::fs::File;
use std::io::{Read, Write};

use genealogy_app::{ExternalId, NewPerson, Session, Workspace};
use genealogy_core::enums::EvidenceLevel;
use wasmtime::StoreLimits;
use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

use crate::bindings::imports::genealogy::host_api::{
    commands, export_sink, import_source, log, progress, query, types,
};
use crate::capability::{Capability, Grants};
use crate::{BulkIo, ProgressControl, ProgressUpdate};

/// The data owned by one plugin instance's Wasmtime store.
pub struct HostState {
    wasi: WasiCtx,
    table: ResourceTable,
    /// Memory/instance caps enforced by Wasmtime (ADR 0011 §4).
    pub limits: StoreLimits,
    grants: Grants,
    workspace: Workspace,
    session: Session,
    /// The bulk source/sink configuration and progress sink (ADR 0013).
    io: BulkIo,
    /// The opened import source, set by `import-source.open`.
    source: Option<File>,
    /// The opened export sink, set by `export-sink.open`.
    sink: Option<File>,
}

impl HostState {
    /// Builds instance state. `wasi` is the (empty) WASI context that denies ambient `files`/`net`
    /// by construction (ADR 0011 §3); `limits` is the memory cap; `io` carries the bulk source/sink
    /// and progress sink (ADR 0013).
    pub fn new(
        wasi: WasiCtx,
        limits: StoreLimits,
        grants: Grants,
        workspace: Workspace,
        session: Session,
        io: BulkIo,
    ) -> Self {
        Self {
            wasi,
            table: ResourceTable::new(),
            limits,
            grants,
            workspace,
            session,
            io,
            source: None,
            sink: None,
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
        external_id: Option<types::ExternalId>,
    ) -> Result<String, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        // Without an external identity, always create (no record to resolve against).
        let Some(external_id) = external_id else {
            let new = NewPerson {
                human_id: None,
                given,
                surname,
                evidence_level: EvidenceLevel::Persona,
            };
            return genealogy_app::create_person(&self.workspace, &self.session, new)
                .await
                .map_err(|error| to_capability_error(&error));
        };
        // Resolve-or-create against the external id (idempotent, additive re-import).
        genealogy_app::import_person(
            &self.workspace,
            &self.session,
            to_external_id(external_id),
            given,
            surname,
        )
        .await
        .map(|(human_id, _created)| human_id)
        .map_err(|error| to_capability_error(&error))
    }

    async fn create_family(
        &mut self,
        external_id: Option<types::ExternalId>,
    ) -> Result<String, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        let Some(external_id) = external_id else {
            return genealogy_app::create_family(&self.workspace, &self.session)
                .await
                .map_err(|error| to_capability_error(&error));
        };
        genealogy_app::import_family(&self.workspace, &self.session, to_external_id(external_id))
            .await
            .map(|(human_id, _created)| human_id)
            .map_err(|error| to_capability_error(&error))
    }

    async fn add_partner(&mut self, family: String, person: String) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::import_add_partner(&self.workspace, &self.session, &family, &person)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn add_child(&mut self, family: String, child: String) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::import_add_child(&self.workspace, &self.session, &family, &child)
            .await
            .map_err(|error| to_capability_error(&error))
    }
}

/// Maps the WIT `external-id` record onto the domain [`ExternalId`] (data-model §11).
fn to_external_id(external_id: types::ExternalId) -> ExternalId {
    ExternalId {
        authority: external_id.authority,
        value: external_id.value,
        kind: external_id.kind,
        url: external_id.url,
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

impl progress::Host for HostState {
    async fn report(
        &mut self,
        step: String,
        processed: u32,
        total: Option<u32>,
    ) -> Result<progress::Control, types::CapabilityError> {
        if !self.grants.allows(Capability::Progress) {
            return Err(types::CapabilityError::Denied);
        }
        let control = (self.io.progress)(ProgressUpdate { step, processed, total });
        Ok(match control {
            ProgressControl::Proceed => progress::Control::Proceed,
            ProgressControl::Cancel => progress::Control::Cancel,
        })
    }
}

impl import_source::Host for HostState {
    async fn open(&mut self) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::ImportSource) {
            return Err(types::CapabilityError::Denied);
        }
        let path = self
            .io
            .source
            .as_ref()
            .ok_or_else(|| types::CapabilityError::Backend("no import source is configured".to_owned()))?;
        let file = File::open(path)
            .map_err(|error| types::CapabilityError::Backend(format!("opening {}: {error}", path.display())))?;
        self.source = Some(file);
        Ok(())
    }

    async fn read(&mut self, len: u32) -> Result<Vec<u8>, types::CapabilityError> {
        if !self.grants.allows(Capability::ImportSource) {
            return Err(types::CapabilityError::Denied);
        }
        let file = self
            .source
            .as_mut()
            .ok_or_else(|| types::CapabilityError::Backend("import source is not open".to_owned()))?;
        let mut buffer = vec![0u8; len as usize];
        let read = file
            .read(&mut buffer)
            .map_err(|error| types::CapabilityError::Backend(format!("reading import source: {error}")))?;
        buffer.truncate(read);
        Ok(buffer)
    }
}

impl export_sink::Host for HostState {
    async fn open(&mut self, suggested_name: String) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::ExportSink) {
            return Err(types::CapabilityError::Denied);
        }
        let target = self
            .io
            .sink
            .as_ref()
            .ok_or_else(|| types::CapabilityError::Backend("no export sink is configured".to_owned()))?;
        let path = target
            .resolve(&suggested_name)
            .map_err(types::CapabilityError::Backend)?;
        let file = File::create(&path)
            .map_err(|error| types::CapabilityError::Backend(format!("creating {}: {error}", path.display())))?;
        self.sink = Some(file);
        Ok(())
    }

    async fn write(&mut self, bytes: Vec<u8>) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::ExportSink) {
            return Err(types::CapabilityError::Denied);
        }
        let file = self
            .sink
            .as_mut()
            .ok_or_else(|| types::CapabilityError::Backend("export sink is not open".to_owned()))?;
        file.write_all(&bytes)
            .map_err(|error| types::CapabilityError::Backend(format!("writing export: {error}")))
    }

    async fn finish(&mut self) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::ExportSink) {
            return Err(types::CapabilityError::Denied);
        }
        let file = self
            .sink
            .as_mut()
            .ok_or_else(|| types::CapabilityError::Backend("export sink is not open".to_owned()))?;
        file.flush()
            .map_err(|error| types::CapabilityError::Backend(format!("flushing export: {error}")))
    }
}
