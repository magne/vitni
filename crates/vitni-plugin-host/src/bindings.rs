//! Generated Wasmtime bindings for the `vitni:host-api` package (ADR 0011 §1).
//!
//! The capability interfaces (`log`, `query`, `commands`, `types`) are generated once from the
//! `host-imports` world; each plugin world reuses them via `with` so there is a single `Host` trait
//! per capability to implement and a single `add_to_linker` to wire.

#![allow(clippy::missing_const_for_fn, reason = "generated bindings are not const")]

/// The capability interfaces and the linker wiring, generated once.
pub mod imports {
    wasmtime::component::bindgen!({
        world: "host-imports",
        path: "wit",
        imports: { default: async },
        require_store_data_send: true,
    });
}

/// The bulk import plugin world (ADR 0013) — reuses the shared capability interfaces; reads its
/// document from the host-opened `import-source`.
pub mod import_world {
    wasmtime::component::bindgen!({
        world: "bulk-import",
        path: "wit",
        imports: { default: async },
        exports: { default: async },
        require_store_data_send: true,
        with: {
            "vitni:host-api/types": crate::bindings::imports::vitni::host_api::types,
            "vitni:host-api/log": crate::bindings::imports::vitni::host_api::log,
            "vitni:host-api/commands": crate::bindings::imports::vitni::host_api::commands,
            "vitni:host-api/progress": crate::bindings::imports::vitni::host_api::progress,
            "vitni:host-api/import-source": crate::bindings::imports::vitni::host_api::import_source,
        },
    });
}

/// The bulk export plugin world (ADR 0013) — reuses the shared capability interfaces; writes its
/// document to the host-resolved `export-sink`.
pub mod export_world {
    wasmtime::component::bindgen!({
        world: "bulk-export",
        path: "wit",
        imports: { default: async },
        exports: { default: async },
        require_store_data_send: true,
        with: {
            "vitni:host-api/types": crate::bindings::imports::vitni::host_api::types,
            "vitni:host-api/log": crate::bindings::imports::vitni::host_api::log,
            "vitni:host-api/query": crate::bindings::imports::vitni::host_api::query,
            "vitni:host-api/progress": crate::bindings::imports::vitni::host_api::progress,
            "vitni:host-api/export-sink": crate::bindings::imports::vitni::host_api::export_sink,
        },
    });
}

/// The plugin-UI world (ADR 0012, ADR 0022) — reuses the shared `log` and `commands` interfaces; its
/// `run-ui-panel` export returns the panel and `handle-action` submits its values, both as opaque
/// JSON strings the host does not parse.
pub mod ui_panel_world {
    wasmtime::component::bindgen!({
        world: "ui-panel",
        path: "wit",
        imports: { default: async },
        exports: { default: async },
        require_store_data_send: true,
        with: {
            "vitni:host-api/types": crate::bindings::imports::vitni::host_api::types,
            "vitni:host-api/log": crate::bindings::imports::vitni::host_api::log,
            "vitni:host-api/commands": crate::bindings::imports::vitni::host_api::commands,
        },
    });
}

/// The assisted-import plugin world (ADR 0017 §5) — reuses the shared capability interfaces; runs the
/// whole record-by-record session from one `run-assisted` invocation, suspending on `present`.
pub mod assisted_import_world {
    wasmtime::component::bindgen!({
        world: "assisted-import",
        path: "wit",
        imports: { default: async },
        exports: { default: async },
        require_store_data_send: true,
        with: {
            "vitni:host-api/types": crate::bindings::imports::vitni::host_api::types,
            "vitni:host-api/log": crate::bindings::imports::vitni::host_api::log,
            "vitni:host-api/query": crate::bindings::imports::vitni::host_api::query,
            "vitni:host-api/commands": crate::bindings::imports::vitni::host_api::commands,
            "vitni:host-api/progress": crate::bindings::imports::vitni::host_api::progress,
            "vitni:host-api/net": crate::bindings::imports::vitni::host_api::net,
            "vitni:host-api/media-store": crate::bindings::imports::vitni::host_api::media_store,
            "vitni:host-api/ai": crate::bindings::imports::vitni::host_api::ai,
            "vitni:host-api/present": crate::bindings::imports::vitni::host_api::present,
        },
    });
}

/// A test-only world for exercising host mechanics without GEDCOM.
pub mod fixture_world {
    wasmtime::component::bindgen!({
        world: "fixture",
        path: "wit",
        imports: { default: async },
        exports: { default: async },
        require_store_data_send: true,
        with: {
            "vitni:host-api/types": crate::bindings::imports::vitni::host_api::types,
            "vitni:host-api/log": crate::bindings::imports::vitni::host_api::log,
            "vitni:host-api/commands": crate::bindings::imports::vitni::host_api::commands,
            "vitni:host-api/net": crate::bindings::imports::vitni::host_api::net,
            "vitni:host-api/media-store": crate::bindings::imports::vitni::host_api::media_store,
            "vitni:host-api/ai": crate::bindings::imports::vitni::host_api::ai,
            "vitni:host-api/present": crate::bindings::imports::vitni::host_api::present,
        },
    });
}
