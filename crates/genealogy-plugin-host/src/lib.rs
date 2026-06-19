//! `genealogy-plugin-host` — the WebAssembly component plugin host (ADR 0007, ADR 0011).
//!
//! This crate sits above `genealogy-app`: it owns Wasmtime, loads and instantiates plugin
//! components, wires the deny-by-default capability interfaces, and applies per-instance resource
//! limits. Plugins read views and submit commands only through the `genealogy-app` use-cases, so
//! the pure core (`genealogy-core`) never links Wasmtime and no storage/framework type crosses the
//! boundary (ADR 0007 §5).
//!
//! The host runtime is async: capability host functions call async use-cases and guests are invoked
//! with `call_async` (ADR 0011). The three plugin roles — GEDCOM import, GEDCOM export, and a
//! test-only fixture — each instantiate against their world over one shared [`Grants`]-gated state.

mod bindings;
mod capability;
mod error;
mod state;

use std::path::Path;

use genealogy_app::{Session, Workspace};
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder, Trap};
use wasmtime_wasi::WasiCtxBuilder;

use crate::bindings::{export_world, fixture_world, import_world, imports, ui_panel_world};
use crate::state::HostState;

pub use crate::capability::{Capability, Grants};
pub use crate::error::PluginError;

/// Per-instance resource limits (ADR 0011 §4). Fuel bounds execution (a runaway guest traps);
/// `memory_bytes` caps linear-memory growth.
#[derive(Debug, Clone, Copy)]
pub struct ResourceBudget {
    /// Fuel units the instance may consume before it traps with `OutOfFuel`.
    pub fuel: u64,
    /// Maximum linear-memory size the instance may grow to, in bytes.
    pub memory_bytes: usize,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            fuel: 1_000_000_000,
            memory_bytes: 64 * 1024 * 1024,
        }
    }
}

/// The plugin host: a configured Wasmtime engine and a linker with the capability interfaces and
/// WASI Preview 2 wired in. Reusable across instantiations.
pub struct PluginHost {
    engine: Engine,
    linker: Linker<HostState>,
}

impl PluginHost {
    /// Builds the host: component model + async + fuel metering enabled, WASI Preview 2 linked, and
    /// the `log`/`query`/`commands` capability interfaces wired (gated per instance by [`Grants`]).
    ///
    /// # Errors
    /// Returns [`PluginError::Runtime`] if the engine or linker cannot be configured.
    pub fn new() -> Result<Self, PluginError> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|error| PluginError::Runtime(error.to_string()))?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker).map_err(|error| PluginError::Runtime(error.to_string()))?;
        imports::HostImports::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
            .map_err(|error| PluginError::Runtime(error.to_string()))?;

        Ok(Self { engine, linker })
    }

    /// Loads a plugin component from `path` (the spike's directory-based "embedded" layer,
    /// ADR 0011 §6).
    ///
    /// # Errors
    /// Returns [`PluginError::Runtime`] if the file is missing or is not a valid component.
    pub fn load(&self, path: &Path) -> Result<Component, PluginError> {
        Component::from_file(&self.engine, path).map_err(|error| PluginError::Runtime(error.to_string()))
    }

    /// Builds a fresh store for one instantiation, applying the memory cap and fuel budget.
    fn build_store(
        &self,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
    ) -> Result<Store<HostState>, PluginError> {
        let wasi = WasiCtxBuilder::new().build();
        let limits: StoreLimits = StoreLimitsBuilder::new().memory_size(budget.memory_bytes).build();
        let state = HostState::new(wasi, limits, grants, workspace, session);
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(budget.fuel)
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        Ok(store)
    }

    /// Runs a GEDCOM import plugin: hands it `gedcom` bytes, returns the number of records imported
    /// and the workspace (recovered from the consumed store so the caller can keep using it).
    ///
    /// # Errors
    /// [`PluginError::ResourceLimit`] if the guest exhausts its fuel, [`PluginError::Guest`] if the
    /// plugin reports a failure, or [`PluginError::Runtime`] on instantiation/trap.
    pub async fn run_gedcom_import(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        gedcom: &[u8],
        budget: ResourceBudget,
    ) -> Result<(u32, Workspace), PluginError> {
        let mut store = self.build_store(workspace, session, grants, budget)?;
        let bindings = import_world::GedcomImport::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_run_import(&mut store, gedcom).await;
        let count = interpret_result(outcome)?;
        Ok((count, store.into_data().into_workspace()))
    }

    /// Runs a GEDCOM export plugin: returns the serialized GEDCOM document and the workspace.
    ///
    /// # Errors
    /// As [`run_gedcom_import`](Self::run_gedcom_import).
    pub async fn run_gedcom_export(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
    ) -> Result<(Vec<u8>, Workspace), PluginError> {
        let mut store = self.build_store(workspace, session, grants, budget)?;
        let bindings = export_world::GedcomExport::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_run_export(&mut store).await;
        let bytes = interpret_result(outcome)?;
        Ok((bytes, store.into_data().into_workspace()))
    }

    /// Runs a plugin-UI plugin (ADR 0012): instantiates the `ui-panel` world and returns the form
    /// description the plugin emitted as an opaque JSON string, plus the workspace. The host does not
    /// parse or render the payload — a framework renderer parses it with `genealogy-ui`. `locale` is
    /// the negotiated BCP-47 UI language the plugin localizes its labels to (ADR 0012 §5).
    ///
    /// # Errors
    /// As [`run_gedcom_import`](Self::run_gedcom_import).
    pub async fn run_ui_panel(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
        locale: &str,
    ) -> Result<(String, Workspace), PluginError> {
        let mut store = self.build_store(workspace, session, grants, budget)?;
        let bindings = ui_panel_world::UiPanel::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_run_ui_panel(&mut store, locale).await;
        let json = interpret_result(outcome)?;
        Ok((json, store.into_data().into_workspace()))
    }

    /// Instantiates the test fixture and invokes `try-create` (proves a granted/denied `commands`
    /// call). Returns the created human id and the workspace.
    ///
    /// # Errors
    /// As [`run_gedcom_import`](Self::run_gedcom_import).
    pub async fn fixture_try_create(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
    ) -> Result<(String, Workspace), PluginError> {
        let mut store = self.build_store(workspace, session, grants, budget)?;
        let bindings = fixture_world::Fixture::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_try_create(&mut store).await;
        let id = interpret_result(outcome)?;
        Ok((id, store.into_data().into_workspace()))
    }

    /// Instantiates the test fixture and invokes `busy-loop` (proves the fuel limit traps a runaway
    /// guest). Expected to return [`PluginError::ResourceLimit`].
    ///
    /// # Errors
    /// [`PluginError::ResourceLimit`] when fuel is exhausted (the expected outcome).
    pub async fn fixture_busy_loop(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
    ) -> Result<(), PluginError> {
        let mut store = self.build_store(workspace, session, grants, budget)?;
        let bindings = fixture_world::Fixture::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        match bindings.call_busy_loop(&mut store).await {
            Ok(()) => Ok(()),
            Err(error) => Err(map_trap(&error)),
        }
    }

    /// Instantiates the test fixture and invokes `allocate` (proves the memory cap). Returns the
    /// guest's report: `1` if the allocation succeeded, `0` if the limiter denied it.
    ///
    /// # Errors
    /// As [`run_gedcom_import`](Self::run_gedcom_import).
    pub async fn fixture_allocate(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
        mib: u32,
    ) -> Result<(u32, Workspace), PluginError> {
        let mut store = self.build_store(workspace, session, grants, budget)?;
        let bindings = fixture_world::Fixture::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        match bindings.call_allocate(&mut store, mib).await {
            Ok(report) => Ok((report, store.into_data().into_workspace())),
            Err(error) => Err(map_trap(&error)),
        }
    }
}

/// Interprets a guest call that returns `result<T, string>`: a host trap (fuel/instantiation), the
/// guest's own error, or success.
fn interpret_result<T>(outcome: wasmtime::Result<Result<T, String>>) -> Result<T, PluginError> {
    match outcome {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(message)) => Err(PluginError::Guest(message)),
        Err(error) => Err(map_trap(&error)),
    }
}

/// Maps a Wasmtime call error: `OutOfFuel` is the resource-limit signal (ADR 0011 §4); any other
/// trap or host error is a runtime fault.
fn map_trap(error: &wasmtime::Error) -> PluginError {
    if error.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
        return PluginError::ResourceLimit("guest exhausted its fuel budget".to_owned());
    }
    PluginError::Runtime(error.to_string())
}
