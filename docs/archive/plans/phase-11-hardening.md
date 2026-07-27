# Plan — Phase 11: 1.0 hardening

- **Status:** **Implemented 2026-07-25** — Gate 2 shipped as PRs #176–#182 (all three
  workstreams). Archived; residuals tracked by area in [`docs/issues.md`](../../issues.md).
- **Date:** 2026-07-24
- **Gating ADR:** [0014](../../adr/0014-plugin-signing-trust-tiers-and-loading.md)
- **Research:** [plugin-signing-and-trust.md](../../research/plugin-signing-and-trust.md)

## Scope

Three roadmap workstreams: (1) plugin signing / trust tiers / capability-grant UX / three-layer
loading (ADR 0014); (2) performance profiling; (3) packaging & distribution (Linux-first).

## Delivery ritual

Gate 1 (this branch, `docs/phase-11-gate-1`): research + ADR 0014 + this plan. Gate 2: one PR per
workstream, stacked, each on a feature branch, `--no-ff` merge via PR, TDD. All gates pass the full
`--workspace --all-features` command set + `cargo xtask build-plugins` + `i18n-check` before merge.

## Workstream A — plugin trust (the ADR 0014 core)

Sequenced sub-PRs (each independently green):

1. **Bundle format + signing primitive.** New signing module in `genealogy-plugin-host` (or a small
   `genealogy-plugin-sign` seam): `plugin.toml` (de)serialize, canonical digest over manifest+wasm
   (`sha2`), ed25519 sign/verify (`ed25519-dalek`, promoted to a direct dep). Embedded project public
   key(s) as the trust root. Tests first: verify-good, verify-tampered-manifest, verify-tampered-wasm,
   verify-wrong-key, verify-unsigned. `cargo xtask build-plugins` emits bundles + dev-signs first-party.
2. **Trust tiers + verification on load.** `TrustTier { Sanctioned, UserTrusted, Untrusted }` resolved
   at discovery; present-but-invalid signature = hard load error; absent signature = untrusted-loadable.
   User trust store in client-scope config (`ConfigStore`): `(publisher, pubkey)` pins. Tests: each
   tier resolves correctly; tampered fails closed; pinned publisher promotes to user-trusted.
3. **Three-layer loader.** One shared resolver in `genealogy-app` mirroring `layered_assets`: workspace
   > app-dir > embedded, id-keyed, higher-layer/higher-semver override. Replace the duplicated flat
   `plugins_dir()` in `genealogy-cli/src/commands/io.rs` and `genealogy-ui-dioxus/src/app.rs`. Tests:
   override precedence, semver tiebreak, missing-layer skip.
4. **Capability-grant model.** Effective grant = declared ∩ user-approved. Extend `PluginPreferences`
   (workspace manifest) with a per-plugin approved-capability set beside `disabled`. Call sites
   (`assisted_grants()`, CLI import/export) pass the resolved effective grant instead of hardcoding.
   Tests: ungranted → `capability-error::denied` with actionable message; declared∩approved math.
5. **Grant/trust UX.** `genealogy-ui` view-models for the first-load grant prompt + trust-store editor;
   `genealogy-ui-dioxus` screens (extend `plugin_panel`/`preferences`); CLI grant/trust subcommands.
   All strings Fluent (`en` + `no`). Update `docs/mockups/plugin-manager.html`. SSR tests for the VMs.

## Workstream B — performance profiling

Greenfield (no `criterion`/benches today). Add a `benches/` harness (criterion, dev-dep) measuring the
cost centres ADR 0004 named: **event-log replay / projection rebuild** (`Store::rebuild_projections`)
at growing log sizes, and hot query paths (list/detail, `places_in_bbox`, `json_each` reverse
indexes). Generate a synthetic large workspace fixture. Produce a short findings doc
(`docs/research/performance-profiling.md`) with numbers. **Snapshotting verdict:** ADR 0004 defers
snapshotting until replay cost is *measured* to warrant it — this workstream provides the measurement
and states the verdict; write a snapshotting ADR only if the numbers justify it.

## Workstream C — packaging & distribution (Linux-first)

- **CLI:** release tarball + `cargo install` path; strip/optimize release profile.
- **GUI:** AppImage + `.deb` for `genealogy-ui-dioxus`, bundling the signed first-party plugin fleet
  and the embedded trust-root public key(s) (Workstream A dependency).
- **CI:** a release workflow (`.github/workflows/`) building the artifacts on the existing Linux
  runner; pin actions to SHA, `persist-credentials: false`, scan with `zizmor`. macOS/Windows +
  OS-level code signing are explicitly out of scope (deferred cycle).

## Documentation sweep (Gate 2 close)

- `docs/roadmap.md`: mark Phase 11 `✅ done` with a delivery summary; flip the ADR 0014 table row to
  **accepted** with its link.
- `docs/roadmap.html`: Phase 11 timeline row `chip breadth` → `chip done` + summary; ADR-table row 0014.
- `docs/issues.md`: move completed Phase 11 items to **Completed**; leave scoped residuals under a
  *Phase 11 residuals* heading (cross-platform packaging, marketplace/auto-update, sub-capability
  grants, transparency-log revocation, per-plugin resource overrides).
- Memory: record lessons.

## Verification

Per workstream: `cargo build --workspace`; `cargo nextest run --workspace --all-features --all-targets`;
`cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo fmt --all`;
`cargo deny check`; `cargo xtask i18n-check`; `cargo xtask build-plugins`; `prek run`. Plus: a
tampered-bundle load is rejected end-to-end; a first-party bundle loads as sanctioned; the profiling
harness runs and emits numbers; the AppImage/`.deb` build produces a launchable artifact.
