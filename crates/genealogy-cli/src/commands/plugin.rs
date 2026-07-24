//! The `plugin` command group (ADR 0014 §3, §5): inspect discovered plugins and their trust tiers,
//! edit the per-workspace capability-grant set, and manage the client-scope pinned-publisher trust
//! store. Headless and config-driven — no interactive first-load prompt (that is the GUI's job, ADR
//! 0014 §5); the CLI only reads and writes the persisted decisions.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use genealogy_app::{
    AppError, ConfigStore, FileConfigStore, add_trusted_publisher, config, read_plugin_preferences,
    remove_trusted_publisher, resolve_bundles, save_plugin_grants,
};
use genealogy_plugin_host::{Capability, PluginHost};

use crate::commands::io::{plugin_layers, trust_roots};
use crate::i18n::Localizer;

/// How many leading hex characters of a pinned key the trust list shows (matches the GUI editor).
const FINGERPRINT_HEX_LEN: usize = 16;

/// `plugin` subcommands.
#[derive(Subcommand)]
pub enum PluginCmd {
    /// List discovered plugins: id, trust tier, declared capabilities, and current effective grant.
    List,
    /// Approve one or more capabilities for a plugin (records a decision in the workspace manifest).
    Grant {
        /// The plugin id (bundle directory name).
        id: String,
        /// The capability interface names to approve (e.g. `query commands`).
        #[arg(required = true, value_name = "CAPABILITY")]
        capabilities: Vec<String>,
    },
    /// Revoke one or more capabilities for a plugin (records a decision in the workspace manifest).
    Revoke {
        /// The plugin id (bundle directory name).
        id: String,
        /// The capability interface names to deny (e.g. `net`).
        #[arg(required = true, value_name = "CAPABILITY")]
        capabilities: Vec<String>,
    },
    /// Manage the client-scope pinned-publisher trust store.
    Trust {
        #[command(subcommand)]
        command: TrustCmd,
    },
}

/// `plugin trust` subcommands.
#[derive(Subcommand)]
pub enum TrustCmd {
    /// List pinned publishers and a short key fingerprint.
    List,
    /// Pin a publisher's ed25519 public key (64 hex characters).
    Add {
        /// The publisher identity.
        publisher: String,
        /// The 64-hex-character ed25519 public key.
        key: String,
    },
    /// Remove a pinned publisher.
    Remove {
        /// The publisher identity to unpin.
        publisher: String,
    },
}

/// Runs a `plugin` subcommand. `workspace` selects the workspace (by name) whose manifest and plugin
/// layer the list/grant/revoke subcommands operate on; the trust subcommands touch only the global
/// config and ignore it.
pub fn run(command: PluginCmd, workspace: Option<&str>, localizer: &Localizer) -> Result<(), AppError> {
    match command {
        PluginCmd::List => list(&resolve_workspace_dir(workspace)?, localizer),
        PluginCmd::Grant { id, capabilities } => {
            edit_grants(&resolve_workspace_dir(workspace)?, &id, &capabilities, true, localizer)
        }
        PluginCmd::Revoke { id, capabilities } => {
            edit_grants(&resolve_workspace_dir(workspace)?, &id, &capabilities, false, localizer)
        }
        PluginCmd::Trust { command } => trust(command, localizer),
    }
}

/// Resolves the workspace directory (by name) for the manifest-scoped subcommands.
fn resolve_workspace_dir(workspace: Option<&str>) -> Result<PathBuf, AppError> {
    let config = FileConfigStore::new(config::config_path()?, None).load_config()?;
    config.resolve_workspace(workspace)
}

/// Lists every discovered plugin with its trust tier, declared capabilities, and effective grant.
fn list(workspace_dir: &Path, localizer: &Localizer) -> Result<(), AppError> {
    let host = PluginHost::new().map_err(|error| AppError::Plugin(error.to_string()))?;
    let roots = trust_roots()?;
    let bundles = resolve_bundles(&plugin_layers(workspace_dir));
    if bundles.is_empty() {
        println!("{}", localizer.plugin_list_empty());
        return Ok(());
    }
    let prefs = read_plugin_preferences(workspace_dir);
    for bundle_dir in bundles.values() {
        let info = host
            .discover_bundle(bundle_dir, &roots)
            .map_err(|error| AppError::Plugin(error.to_string()))?;
        let declared: Vec<&str> = info.capabilities.iter().map(Capability::interface_name).collect();
        let granted = info.effective_grants(prefs.approved_grants(&info.id));
        let granted_names: Vec<&str> = info
            .capabilities
            .iter()
            .filter(|c| granted.allows(**c))
            .map(Capability::interface_name)
            .collect();
        println!(
            "{}",
            localizer.plugin_list_line(&info.id, info.trust, &declared.join(" "), &granted_names.join(" "))
        );
    }
    Ok(())
}

/// Merges (grant) or removes (revoke) `capabilities` in the workspace's persisted approved set for
/// `id`, recording an explicit decision (ADR 0014 §5). The baseline is the current recorded set, or
/// empty when none has been recorded yet.
fn edit_grants(
    workspace_dir: &Path,
    id: &str,
    capabilities: &[String],
    grant: bool,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let prefs = read_plugin_preferences(workspace_dir);
    let mut approved: BTreeSet<String> = prefs.approved_grants(id).cloned().unwrap_or_default();
    for capability in capabilities {
        if grant {
            approved.insert(capability.clone());
        } else {
            approved.remove(capability);
        }
    }
    save_plugin_grants(workspace_dir, id, &approved)?;
    println!(
        "{}",
        localizer.plugin_grants_saved(id, &approved.iter().cloned().collect::<Vec<_>>().join(" "))
    );
    Ok(())
}

/// Runs a `plugin trust` subcommand against the client-scope pinned-publisher store.
fn trust(command: TrustCmd, localizer: &Localizer) -> Result<(), AppError> {
    let config_path = config::config_path()?;
    match command {
        TrustCmd::List => {
            let trust = FileConfigStore::new(config_path, None).load_plugin_trust()?;
            if trust.publishers.is_empty() {
                println!("{}", localizer.plugin_trust_list_empty());
                return Ok(());
            }
            for (publisher, key_hex) in &trust.publishers {
                let fingerprint: String = key_hex.chars().take(FINGERPRINT_HEX_LEN).collect();
                println!("{}", localizer.plugin_trust_list_line(publisher, &fingerprint));
            }
            Ok(())
        }
        TrustCmd::Add { publisher, key } => {
            add_trusted_publisher(&config_path, &publisher, &key)?;
            println!("{}", localizer.plugin_trust_pinned(&publisher));
            Ok(())
        }
        TrustCmd::Remove { publisher } => {
            remove_trusted_publisher(&config_path, &publisher)?;
            println!("{}", localizer.plugin_trust_unpinned(&publisher));
            Ok(())
        }
    }
}
