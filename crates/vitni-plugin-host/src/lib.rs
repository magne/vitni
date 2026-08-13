//! `vitni-plugin-host` — the WebAssembly component plugin host (ADR 0007, ADR 0011).
//!
//! This crate sits above `vitni-app`: it owns Wasmtime, loads and instantiates plugin
//! components, wires the deny-by-default capability interfaces, and applies per-instance resource
//! limits. Plugins read views and submit commands only through the `vitni-app` use-cases, so
//! the pure core (`vitni-core`) never links Wasmtime and no storage/framework type crosses the
//! boundary (ADR 0007 §5).
//!
//! The host runtime is async: capability host functions call async use-cases and guests are invoked
//! with `call_async` (ADR 0011). The plugin roles — bulk import, bulk export (ADR 0013), the
//! plugin-UI panel, and a test-only fixture — each instantiate against their world over one shared
//! [`Grants`]-gated state.

mod ai;
mod bindings;
mod capability;
mod discovery;
mod error;
mod media;
mod net;
mod present;
pub mod signing;
mod state;
mod trust;

use std::path::{Path, PathBuf};

use vitni_app::{AiConfig, Confidence, Session, Workspace};
use wasmtime::component::{HasSelf, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder, Trap};
use wasmtime_wasi::WasiCtxBuilder;

use crate::bindings::{assisted_import_world, export_world, fixture_world, import_world, imports, ui_panel_world};
use crate::state::HostState;

pub use wasmtime::component::Component;

pub use crate::capability::{Capability, Grants};
pub use crate::discovery::{PluginInfo, PluginRole};
pub use crate::error::PluginError;
pub use crate::net::{HostPattern, NetPolicy};
pub use crate::present::{PresentError, Presenter};
pub use crate::trust::{TrustRoots, TrustTier, classify, resolve_trust_roots};

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

/// The per-run frontend I/O: the bulk source/sink and progress sink (ADR 0013) plus the assisted
/// `present` sink (ADR 0017 §5). Non-bulk, non-assisted plugins use [`BulkIo::none`].
pub(crate) struct BulkIo {
    pub(crate) source: Option<PathBuf>,
    pub(crate) sink: Option<ExportTarget>,
    pub(crate) progress: ProgressFn,
    /// The frontend presenter the `present` capability suspends on (ADR 0017 §5); `None` for runs
    /// that were granted no `present` access.
    pub(crate) presenter: Option<Box<dyn Presenter>>,
}

impl BulkIo {
    /// I/O for a non-bulk run: no source, no sink, no presenter, a progress sink that always proceeds.
    fn none() -> Self {
        Self {
            source: None,
            sink: None,
            progress: Box::new(|_| ProgressControl::Proceed),
            presenter: None,
        }
    }

    /// I/O for an import run.
    fn import(source: PathBuf, progress: ProgressFn) -> Self {
        Self {
            source: Some(source),
            progress,
            ..Self::none()
        }
    }

    /// I/O for an export run.
    fn export(sink: ExportTarget, progress: ProgressFn) -> Self {
        Self {
            sink: Some(sink),
            progress,
            ..Self::none()
        }
    }

    /// I/O for an assisted-import run (ADR 0017 §5): a `present` sink and a progress sink, no bulk
    /// source or sink.
    fn assisted(presenter: Box<dyn Presenter>, progress: ProgressFn) -> Self {
        Self {
            progress,
            presenter: Some(presenter),
            ..Self::none()
        }
    }
}

/// The common inputs to one plugin run: the open workspace, the operator session (a Software agent,
/// ADR 0007 §7), the capability grants, and the resource budget.
pub struct Invocation {
    /// The workspace the plugin reads and writes through `vitni-app`.
    pub workspace: Workspace,
    /// The operator session stamped onto every change the plugin makes.
    pub session: Session,
    /// The capabilities granted to this run (deny-by-default, ADR 0011 §2).
    pub grants: Grants,
    /// The fuel and memory limits for this run (ADR 0011 §4).
    pub budget: ResourceBudget,
    /// The network policy for host-mediated fetches (ADR 0017 §2). [`NetPolicy::deny_all`] for runs
    /// with no `net` access.
    pub net_policy: NetPolicy,
    /// The AI provider inventory for `ai.interpret-media` (ADR 0017 §4). [`AiConfig::default`] (empty)
    /// for runs with no `ai` access.
    pub ai_config: AiConfig,
    /// The default confidence stamped on every command the run issues (ADR 0017 §7). `None` keeps the
    /// pre-assisted behavior (no surety judgment recorded); the assisted caller sets
    /// `Some(Confidence::Low)`.
    pub provenance_confidence: Option<Confidence>,
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

impl ResourceBudget {
    /// The budget for an assisted-import run (ADR 0017 §8). Fuel does not tick during host awaits
    /// (`present`, `net`, `media-store`, `ai`), so this bounds only guest compute — but an assisted
    /// session parses many pages across a long-running invocation, so it gets 4× the default fuel.
    #[must_use]
    pub fn assisted() -> Self {
        Self {
            fuel: 4 * Self::default().fuel,
            ..Self::default()
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

    /// Loads a plugin component from `path`.
    ///
    /// # Errors
    /// Returns [`PluginError::Runtime`] if the file is missing or is not a valid component.
    pub fn load(&self, path: &Path) -> Result<Component, PluginError> {
        Component::from_file(&self.engine, path).map_err(|error| PluginError::Runtime(error.to_string()))
    }

    /// Loads the component of the plugin bundle at `bundle_dir` (its `plugin.wasm`, ADR 0014 §2). A
    /// caller resolves `bundle_dir` through the three-layer resolver (`vitni-app`), then loads
    /// it here.
    ///
    /// # Errors
    /// Returns [`PluginError::Runtime`] if `bundle_dir/plugin.wasm` is missing or is not a valid
    /// component.
    pub fn load_bundle(&self, bundle_dir: &Path) -> Result<Component, PluginError> {
        self.load(&bundle_dir.join("plugin.wasm"))
    }

    /// Builds a fresh store for one instantiation, applying the memory cap and fuel budget.
    #[expect(
        clippy::too_many_arguments,
        reason = "the store is built from every per-run input: workspace, operator session, grants, resource budget, net policy, ai config, provenance template, and bulk I/O"
    )]
    fn build_store(
        &self,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
        net_policy: NetPolicy,
        ai_config: AiConfig,
        provenance_confidence: Option<Confidence>,
        io: BulkIo,
    ) -> Result<Store<HostState>, PluginError> {
        let wasi = WasiCtxBuilder::new().build();
        let limits: StoreLimits = StoreLimitsBuilder::new().memory_size(budget.memory_bytes).build();
        let state = HostState::new(
            wasi,
            limits,
            grants,
            workspace,
            session,
            net_policy,
            ai_config,
            provenance_confidence,
            io,
        );
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
            net_policy,
            ai_config,
            provenance_confidence,
        } = run;
        let io = BulkIo::import(source, Box::new(progress));
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            net_policy,
            ai_config,
            provenance_confidence,
            io,
        )?;
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
            net_policy,
            ai_config,
            provenance_confidence,
        } = run;
        let io = BulkIo::export(target, Box::new(progress));
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            net_policy,
            ai_config,
            provenance_confidence,
            io,
        )?;
        let bindings = export_world::BulkExport::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_run_export(&mut store).await;
        let count = interpret_result(outcome)?;
        Ok((count, store.into_data().into_workspace()))
    }

    /// Runs an assisted-import plugin (ADR 0017 §5): instantiates the `assisted-import` world and
    /// drives one long-running `run-assisted` session, suspending on `presenter` each time the plugin
    /// calls `present` and reporting long steps to `progress`. Returns the plugin's JSON session
    /// summary and the workspace. The whole review session is one invocation; wizard state lives in
    /// guest memory. `run.grants` must carry `Present` (plus the flow's other capabilities) for the
    /// plugin to reach the frontend.
    ///
    /// # Errors
    /// As [`run_bulk_import`](Self::run_bulk_import).
    pub async fn run_assisted_import(
        &self,
        component: &Component,
        run: Invocation,
        request: &str,
        presenter: Box<dyn Presenter>,
        progress: impl FnMut(ProgressUpdate) -> ProgressControl + Send + 'static,
    ) -> Result<(String, Workspace), PluginError> {
        let Invocation {
            workspace,
            session,
            grants,
            budget,
            net_policy,
            ai_config,
            provenance_confidence,
        } = run;
        let io = BulkIo::assisted(presenter, Box::new(progress));
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            net_policy,
            ai_config,
            provenance_confidence,
            io,
        )?;
        let bindings = assisted_import_world::AssistedImport::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_run_assisted(&mut store, request).await;
        let summary = interpret_result(outcome)?;
        Ok((summary, store.into_data().into_workspace()))
    }

    /// Runs a plugin-UI plugin (ADR 0012): instantiates the `ui-panel` world and returns the form
    /// description the plugin emitted as an opaque JSON string, plus the workspace. The host does not
    /// parse or render the payload — a framework renderer parses it with `vitni-ui` and resolves
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
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            NetPolicy::deny_all(),
            AiConfig::default(),
            None,
            BulkIo::none(),
        )?;
        let bindings = ui_panel_world::UiPanel::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_run_ui_panel(&mut store).await;
        let json = interpret_result(outcome)?;
        Ok((json, store.into_data().into_workspace()))
    }

    /// Runs a plugin-UI submission (ADR 0022 §2): instantiates the `ui-panel` world and invokes
    /// `handle-action` with the activated `action` id and the form's `values` JSON, returning the
    /// plugin's `submit-result` JSON string (parsed by `vitni-ui`) and the workspace. As with
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
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            NetPolicy::deny_all(),
            AiConfig::default(),
            None,
            BulkIo::none(),
        )?;
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
        provenance_confidence: Option<Confidence>,
    ) -> Result<(String, Workspace), PluginError> {
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            NetPolicy::deny_all(),
            AiConfig::default(),
            provenance_confidence,
            BulkIo::none(),
        )?;
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
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            NetPolicy::deny_all(),
            AiConfig::default(),
            None,
            BulkIo::none(),
        )?;
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
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            NetPolicy::deny_all(),
            AiConfig::default(),
            None,
            BulkIo::none(),
        )?;
        let bindings = fixture_world::Fixture::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        match bindings.call_allocate(&mut store, mib).await {
            Ok(report) => Ok((report, store.into_data().into_workspace())),
            Err(error) => Err(map_trap(&error)),
        }
    }

    /// Instantiates the fixture and invokes `try-fetch` (proves the `net` grant and policy checks).
    /// Returns the fixture's `"status final-url body-len"` summary string and the workspace.
    ///
    /// # Errors
    /// As [`run_bulk_import`](Self::run_bulk_import); a denied capability or policy rejection surfaces
    /// as [`PluginError::Guest`].
    #[expect(
        clippy::too_many_arguments,
        reason = "a fixture net call carries the full invocation plus the net policy and the target url"
    )]
    pub async fn fixture_try_fetch(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
        net_policy: NetPolicy,
        url: &str,
    ) -> Result<(String, Workspace), PluginError> {
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            net_policy,
            AiConfig::default(),
            None,
            BulkIo::none(),
        )?;
        let bindings = fixture_world::Fixture::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_try_fetch(&mut store, url).await;
        let summary = interpret_result(outcome)?;
        Ok((summary, store.into_data().into_workspace()))
    }

    /// Instantiates the fixture and invokes `try-store` (proves the `media-store` grant, path safety,
    /// checksum, and dedup). Returns the fixture's
    /// `"relative-path checksum mime size existed"` summary string and the workspace.
    ///
    /// # Errors
    /// As [`fixture_try_fetch`](Self::fixture_try_fetch).
    #[expect(
        clippy::too_many_arguments,
        reason = "a fixture media-store call carries the full invocation plus the bytes and the suggested path"
    )]
    pub async fn fixture_try_store(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
        bytes: &[u8],
        suggested_path: &str,
    ) -> Result<(String, Workspace), PluginError> {
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            NetPolicy::deny_all(),
            AiConfig::default(),
            None,
            BulkIo::none(),
        )?;
        let bindings = fixture_world::Fixture::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_try_store(&mut store, bytes, suggested_path).await;
        let summary = interpret_result(outcome)?;
        Ok((summary, store.into_data().into_workspace()))
    }

    /// Instantiates the fixture and invokes `try-fetch-store` (proves the download + content-type +
    /// binary-cap path of `media-store.fetch-and-store`). Returns the same summary string.
    ///
    /// # Errors
    /// As [`fixture_try_fetch`](Self::fixture_try_fetch).
    #[expect(
        clippy::too_many_arguments,
        reason = "a fixture fetch-and-store call carries the full invocation plus the net policy, url, and suggested path"
    )]
    pub async fn fixture_try_fetch_store(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
        net_policy: NetPolicy,
        url: &str,
        suggested_path: &str,
    ) -> Result<(String, Workspace), PluginError> {
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            net_policy,
            AiConfig::default(),
            None,
            BulkIo::none(),
        )?;
        let bindings = fixture_world::Fixture::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_try_fetch_store(&mut store, url, suggested_path).await;
        let summary = interpret_result(outcome)?;
        Ok((summary, store.into_data().into_workspace()))
    }

    /// Instantiates the fixture and invokes `try-interpret` (proves the `ai` grant and provider
    /// resolution). `ai_config` is the provider inventory the host resolves against; `provider` names
    /// one (or `None` for the default). `net_policy` conveys `require_https` for the `vision-api`
    /// endpoint scheme (tests pass a relaxed policy to reach a local mock server; `command` providers
    /// ignore it). Returns the model's raw text and the workspace.
    ///
    /// # Errors
    /// As [`fixture_try_fetch`](Self::fixture_try_fetch); a denied capability, an unknown provider, or
    /// a provider failure surfaces as [`PluginError::Guest`].
    #[expect(
        clippy::too_many_arguments,
        reason = "a fixture ai call carries the full invocation plus the ai config, net policy, provider name, media path, and prompt"
    )]
    pub async fn fixture_try_interpret(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
        ai_config: AiConfig,
        net_policy: NetPolicy,
        provider: Option<&str>,
        media_path: &str,
        prompt: &str,
    ) -> Result<(String, Workspace), PluginError> {
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            net_policy,
            ai_config,
            None,
            BulkIo::none(),
        )?;
        let bindings = fixture_world::Fixture::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings
            .call_try_interpret(&mut store, provider, media_path, prompt)
            .await;
        let text = interpret_result(outcome)?;
        Ok((text, store.into_data().into_workspace()))
    }

    /// Instantiates the fixture and invokes `try-present` (proves the `present` grant and the
    /// suspend/answer round-trip). `presenter` scripts the frontend's answer; `payload` is the opaque
    /// string handed to it. Returns the presenter's response and the workspace.
    ///
    /// # Errors
    /// As [`fixture_try_fetch`](Self::fixture_try_fetch); a denied capability or a dropped presenter
    /// channel surfaces as [`PluginError::Guest`].
    #[expect(
        clippy::too_many_arguments,
        reason = "a fixture present call carries the full invocation plus the presenter and the payload"
    )]
    pub async fn fixture_try_present(
        &self,
        component: &Component,
        workspace: Workspace,
        session: Session,
        grants: Grants,
        budget: ResourceBudget,
        presenter: Box<dyn Presenter>,
        payload: &str,
    ) -> Result<(String, Workspace), PluginError> {
        let io = BulkIo::assisted(presenter, Box::new(|_| ProgressControl::Proceed));
        let mut store = self.build_store(
            workspace,
            session,
            grants,
            budget,
            NetPolicy::deny_all(),
            AiConfig::default(),
            None,
            io,
        )?;
        let bindings = fixture_world::Fixture::instantiate_async(&mut store, component, &self.linker)
            .await
            .map_err(|error| PluginError::Runtime(error.to_string()))?;
        let outcome = bindings.call_try_present(&mut store, payload).await;
        let response = interpret_result(outcome)?;
        Ok((response, store.into_data().into_workspace()))
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::ExportTarget;

    /// A `File` target pins the path: the plugin's suggested name has no say.
    #[test]
    fn a_file_target_ignores_the_suggested_name() {
        let target = ExportTarget::File(PathBuf::from("/ws/out/pinned.ged"));
        assert_eq!(target.resolve("export.ged"), Ok(PathBuf::from("/ws/out/pinned.ged")));
    }

    /// A `Directory` target takes the plugin's suggested name as the leaf.
    #[test]
    fn a_directory_target_uses_the_suggested_name() {
        let target = ExportTarget::Directory(PathBuf::from("/ws/exports"));
        assert_eq!(
            target.resolve("export.ged"),
            Ok(PathBuf::from("/ws/exports/export.ged"))
        );
    }

    /// A suggested name is reduced to its base name, so no plugin can steer the write out of the
    /// directory the host chose — whether by climbing out of it or by naming an absolute path.
    #[test]
    fn a_directory_target_cannot_be_escaped_by_the_suggested_name() {
        let target = ExportTarget::Directory(PathBuf::from("/ws/exports"));
        for suggested in ["../evil.ged", "../../../evil.ged", "/etc/evil.ged", "sub/dir/evil.ged"] {
            let resolved = target.resolve(suggested).expect("a base name remains");
            assert_eq!(
                resolved,
                PathBuf::from("/ws/exports/evil.ged"),
                "escaping name {suggested:?} must collapse to its base name"
            );
            assert!(
                resolved.starts_with(Path::new("/ws/exports")),
                "the write must stay inside the target directory: {resolved:?}"
            );
        }
    }

    /// A suggested name with no base name at all (`..`, a bare separator, empty) is refused outright
    /// — there is nothing safe to write to.
    #[test]
    fn a_directory_target_refuses_a_suggested_name_without_a_base_name() {
        let target = ExportTarget::Directory(PathBuf::from("/ws/exports"));
        for suggested in ["..", "/", "", "."] {
            assert!(
                target.resolve(suggested).is_err(),
                "suggested name {suggested:?} names no file"
            );
        }
    }
}
