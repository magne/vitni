//! Shared test fixture: one Wasmtime host and one compilation of each plugin component per test
//! binary. `Component::from_file` is a full Cranelift compile; without sharing, every test
//! recompiled its component(s) from scratch (the discovery tests recompiled all six each time).
//! `Engine` and `Component` are `Send + Sync` and each test still builds its own `Store`, so
//! sharing is safe across the parallel `#[tokio::test]`s.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]
// Each test binary that includes this module uses only a subset of these helpers, so the
// unused ones are dead in that binary — no single binary exercises them all.
#![expect(dead_code, reason = "each test binary uses a subset of the shared helpers")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use genealogy_plugin_host::{Component, PluginHost, PluginInfo, TrustRoots};

/// The directory of built plugin components (`cargo xtask build-plugins`).
pub fn plugins_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/plugins");
    assert!(
        dir.is_dir(),
        "missing {} — run `cargo xtask build-plugins` first",
        dir.display()
    );
    dir
}

/// Resolves a built plugin component path, failing with an actionable message if it is missing.
pub fn plugin_path(id: &str) -> PathBuf {
    let path = plugins_dir().join(format!("{id}.wasm"));
    assert!(
        path.is_file(),
        "missing plugin component {} — run `cargo xtask build-plugins` first",
        path.display()
    );
    path
}

/// One host (Engine + Linker) shared across every test in this binary.
pub fn host() -> &'static PluginHost {
    static HOST: LazyLock<PluginHost> = LazyLock::new(|| PluginHost::new().expect("host"));
    &HOST
}

/// The compiled component for `id`, compiled at most once per test binary. `Component` is
/// Arc-backed, so the clone is cheap and each test still instantiates its own `Store`.
pub fn component(id: &str) -> Component {
    static CACHE: LazyLock<Mutex<HashMap<String, Component>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
    let mut cache = CACHE.lock().expect("component cache");
    cache
        .entry(id.to_owned())
        .or_insert_with(|| host().load(&plugin_path(id)).expect("load component"))
        .clone()
}

/// The discovery of the real `target/plugins` directory, computed once per test binary. This
/// compiles all six components a single time instead of once per discovery test.
pub fn discovered() -> &'static Vec<PluginInfo> {
    static DISCOVERED: LazyLock<Vec<PluginInfo>> = LazyLock::new(|| {
        host()
            .discover(&plugins_dir(), &TrustRoots::embedded())
            .expect("discover")
    });
    &DISCOVERED
}
