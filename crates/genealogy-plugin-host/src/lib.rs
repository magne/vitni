//! `genealogy-plugin-host` — the WebAssembly component plugin host (ADR 0007, ADR 0011).
//!
//! This crate sits above `genealogy-app`: it owns Wasmtime, loads and instantiates plugin
//! components, wires the deny-by-default capability interfaces, and applies per-instance resource
//! limits. Plugins read views and submit commands only through the `genealogy-app` use-cases, so
//! the pure core (`genealogy-core`) never links Wasmtime and no storage/framework type crosses the
//! boundary (ADR 0007 §5).
//!
//! The host runtime is async: capability host functions call async use-cases and guests are invoked
//! with `call_async` (ADR 0011). The plugin roles — bulk import, bulk export (ADR 0013), the
//! plugin-UI panel, and a test-only fixture — each instantiate against their world over one shared
//! [`Grants`]-gated state.

mod bindings;
mod capability;
mod discovery;
mod error;
mod state;

use std::path::{Path, PathBuf};

use genealogy_app::{Session, Workspace};
use wasmtime::component::{HasSelf, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder, Trap};
use wasmtime_wasi::WasiCtxBuilder;

use crate::bindings::{export_world, fixture_world, import_world, imports, ui_panel_world};
use crate::state::HostState;

pub use wasmtime::component::Component;

pub use crate::capability::{Capability, Grants};
pub use crate::discovery::{PluginInfo, PluginRole};
pub use crate::error::PluginError;

/// A progress update a bulk plugin reports as it advances (ADR 0013). `total` is absent when the
/// plugin cannot yet know the record count (common during import).
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// The phase the plugin is in (e.g. `"persons"`, `"families"`).
    pub step: String,
    /// How many records the plugin has processed so far.
    pub processed: u32,
    /// The total it expects, if known.
    pub total: Option<u32>,
}

/// A frontend's answer to a progress report (ADR 0013): keep going, or cancel the operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressControl {
    /// Continue the operation.
    Proceed,
    /// Stop the operation as soon as the guest can.
    Cancel,
}

/// A frontend's progress sink — invoked for each [`ProgressUpdate`] a bulk plugin reports, returning
/// whether the operation should continue or be cancelled.
pub type ProgressFn = Box<dyn FnMut(ProgressUpdate) -> ProgressControl + Send>;

/// Where a bulk export writes (ADR 0013). The host owns the path; the plugin only proposes a name.
#[derive(Debug, Clone)]
pub enum ExportTarget {
    /// Write to exactly this file, ignoring the plugin's suggested name.
    File(PathBuf),
    /// Write into this directory under the plugin's suggested file name (its base name only, so the
    /// write cannot escape the directory).
    Directory(PathBuf),
}

impl ExportTarget {
    /// Resolves the destination from the plugin's `suggested_name`.
    pub(crate) fn resolve(&self, suggested_name: &str) -> Result<PathBuf, String> {
        match self {
            Self::File(path) => Ok(path.clone()),
            Self::Directory(dir) => {
                let name = Path::new(suggested_name)
                    .file_name()
                    .ok_or_else(|| format!("invalid export file name {suggested_name:?}"))?;
                Ok(dir.join(name))
            }
        }
    }
}

/// The bulk source/sink and progress sink for one run (ADR 0013). Non-bulk plugins use
/// [`BulkIo::none`].
pub(crate) struct BulkIo {
    pub(crate) source: Option<PathBuf>,
    pub(crate) sink: Option<ExportTarget>,
    pub(crate) progress: ProgressFn,
}

impl BulkIo {
    /// I/O for a non-bulk run: no source, no sink, a progress sink that always proceeds.
    fn none() -> Self {
        Self {
            source: None,
            sink: None,
            progress: Box::new(|_| ProgressControl::Proceed),
        }
    }

    /// I/O for an import run.
    fn import(source: PathBuf, progress: ProgressFn) -> Self {
        Self {
            source: Some(source),
            sink: None,
            progress,
        }
    }

    /// I/O for an export run.
    fn export(sink: ExportTarget, progress: ProgressFn) -> Self {
        Self {
            source: None,
            sink: Some(sink),
            progress,
        }
    }
}

/// The common inputs to one plugin run: the open workspace, the operator session (a Software agent,
/// ADR 0007 §7), the capability grants, and the resource budget.
pub struct Invocation {
    /// The workspace the plugin reads and writes through `genealogy-app`.
    pub workspace: Workspace,
    /// The operator session stamped onto every change the plugin makes.
    pub session: Session,
    /// The capabilities granted to this run (deny-by-default, ADR 0011 §2).
    pub grants: Grants,
    /// The fuel and memory limits for this run (ADR 0011 §4).
    pub budget: ResourceBudget,
}

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

    /// The configured Wasmtime engine, needed to introspect a [`Component`]'s type (imports/
    /// exports) — see [`Self::discover`].
    pub(crate) const fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Loads a plugin component from `path` (the spike's directory-based "embedded" layer,
    /// ADR 0011 §6).
    ///
    /// # Errors
    /// Returns [`PluginError::Runtime`] if the file is missing or is not a valid component.
    pub fn load(&self, path: &Path) -> Result<Component, PluginError> {
        Component::from_file(&self.engine, path).map_err(|error| PluginError::Runtime(error.to_string()))
    }

    /// Loads a plugin component by stable id from `plugins_dir` (the spike's directory-based loader,
    /// ADR 0011 §6; the three-layer override is deferred to ADR 0014).
    ///
    /// # Errors
    /// Returns [`PluginError::Runtime`] if the component is missing or invalid.
    pub fn load_by_id(&self, plugins_dir: &Path, id: &str) -> Result<Component, PluginError> {
        self.load(&plugins_dir.join(format!("{id}.wasm")))
    }

    /// Builds a fresh store for one instantiation, applying the memory cap and fuel budget.
    fn build_store(
        &self,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
        io: BulkIo,
    ) -> Result<Store<HostState>, PluginError> {
        let wasi = WasiCtxBuilder::new().build();
        let limits: StoreLimits = StoreLimitsBuilder::new().memory_size(budget.memory_bytes).build();
        let state = HostState::new(wasi, limits, grants, workspace, session, io);
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(budget.fuel)
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        Ok(store)
    }

    /// Runs a bulk import plugin (ADR 0013): the plugin reads its document from `source` through the
    /// host-mediated `import-source`, drives `commands`, and reports progress to `progress`. Returns
    /// the number of records imported and the workspace (recovered from the consumed store so the
    /// caller can keep using it).
    ///
    /// # Errors
    /// [`PluginError::ResourceLimit`] if the guest exhausts its fuel, [`PluginError::Guest`] if the
    /// plugin reports a failure, or [`PluginError::Runtime`] on instantiation/trap.
    pub async fn run_bulk_import(
        &self,
        component: &Component,
        run: Invocation,
        source: PathBuf,
        progress: impl FnMut(ProgressUpdate) -> ProgressControl + Send + 'static,
    ) -> Result<(u32, Workspace), PluginError> {
        let Invocation {
            workspace,
            session,
            grants,
            budget,
        } = run;
        let io = BulkIo::import(source, Box::new(progress));
        let mut store = self.build_store(workspace, session, grants, budget, io)?;
        let bindings = import_world::BulkImport::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_run_import(&mut store).await;
        let count = interpret_result(outcome)?;
        Ok((count, store.into_data().into_workspace()))
    }

    /// Runs a bulk export plugin (ADR 0013): the plugin reads via `query`, writes its document to the
    /// host-resolved `target` through `export-sink`, and reports progress to `progress`. Returns the
    /// number of records written and the workspace.
    ///
    /// # Errors
    /// As [`run_bulk_import`](Self::run_bulk_import).
    pub async fn run_bulk_export(
        &self,
        component: &Component,
        run: Invocation,
        target: ExportTarget,
        progress: impl FnMut(ProgressUpdate) -> ProgressControl + Send + 'static,
    ) -> Result<(u32, Workspace), PluginError> {
        let Invocation {
            workspace,
            session,
            grants,
            budget,
        } = run;
        let io = BulkIo::export(target, Box::new(progress));
        let mut store = self.build_store(workspace, session, grants, budget, io)?;
        let bindings = export_world::BulkExport::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_run_export(&mut store).await;
        let count = interpret_result(outcome)?;
        Ok((count, store.into_data().into_workspace()))
    }

    /// Runs a plugin-UI plugin (ADR 0012): instantiates the `ui-panel` world and returns the form
    /// description the plugin emitted as an opaque JSON string, plus the workspace. The host does not
    /// parse or render the payload — a framework renderer parses it with `genealogy-ui` and resolves
    /// the form's label IDs against the plugin's catalogue (ADR 0012 §5).
    ///
    /// # Errors
    /// As [`run_bulk_import`](Self::run_bulk_import).
    pub async fn run_ui_panel(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
    ) -> Result<(String, Workspace), PluginError> {
        let mut store = self.build_store(workspace, session, grants, budget, BulkIo::none())?;
        let bindings = ui_panel_world::UiPanel::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_run_ui_panel(&mut store).await;
        let json = interpret_result(outcome)?;
        Ok((json, store.into_data().into_workspace()))
    }

    /// Runs a plugin-UI submission (ADR 0022 §2): instantiates the `ui-panel` world and invokes
    /// `handle-action` with the activated `action` id and the form's `values` JSON, returning the
    /// plugin's `submit-result` JSON string (parsed by `genealogy-ui`) and the workspace. As with
    /// [`run_ui_panel`](Self::run_ui_panel) the host stays opaque to both payloads. Submission is the
    /// invocation that grants `commands` (deny-by-default, ADR 0022 §3), so the plugin can drive
    /// audited mutations through the app boundary; a guest `err` (a technical failure or a denied
    /// capability) surfaces as [`PluginError::Guest`], while validation feedback rides the returned
    /// `submit-result` `failure`.
    ///
    /// # Errors
    /// As [`run_bulk_import`](Self::run_bulk_import).
    #[expect(
        clippy::too_many_arguments,
        reason = "a submission carries the full invocation (component/workspace/session/grants/budget) plus the action id and its values"
    )]
    pub async fn run_ui_panel_action(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
        action: &str,
        values: &str,
    ) -> Result<(String, Workspace), PluginError> {
        let mut store = self.build_store(workspace, session, grants, budget, BulkIo::none())?;
        let bindings = ui_panel_world::UiPanel::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_handle_action(&mut store, action, values).await;
        let json = interpret_result(outcome)?;
        Ok((json, store.into_data().into_workspace()))
    }

    /// Instantiates the test fixture and invokes `try-create` (proves a granted/denied `commands`
    /// call). Returns the created human id and the workspace.
    ///
    /// # Errors
    /// As [`run_bulk_import`](Self::run_bulk_import).
    pub async fn fixture_try_create(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
    ) -> Result<(String, Workspace), PluginError> {
        let mut store = self.build_store(workspace, session, grants, budget, BulkIo::none())?;
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
        let mut store = self.build_store(workspace, session, grants, budget, BulkIo::none())?;
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
    /// As [`run_bulk_import`](Self::run_bulk_import).
    pub async fn fixture_allocate(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
        mib: u32,
    ) -> Result<(u32, Workspace), PluginError> {
        let mut store = self.build_store(workspace, session, grants, budget, BulkIo::none())?;
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
