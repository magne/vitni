# 14. Plugin signing, trust tiers, capability-grant UX, and layered loading

- **Status:** Accepted
- **Date:** 2026-07-24

## Context

ADR 0007 §9 fixed the direction: sanctioned/bundled plugins **must be signed and verified before
load**, unsanctioned plugins may load unsigned but are treated as untrusted and never auto-granted
sanctioned-only capabilities, and signing protects integrity/authenticity only — it never widens the
sandbox. ADR 0007 §8 named a bundle-metadata format but deferred it. ADR 0011 §6 deferred "the
three-layer override order, plugin signing, trust tiers, and bundle verification ... implemented with
distribution (roadmap Phase 11, ADR 0014)." This is that ADR.

What exists today (grounded in `docs/research/plugin-signing-and-trust.md`):

- **Loader:** a single flat directory. `PluginHost::load_by_id(plugins_dir, id)` resolves
  `plugins_dir/{id}.wasm`; `plugins_dir()` is `$VITNI_PLUGIN_DIR` else `target/plugins`. No layers,
  no embedded plugins.
- **Discovery:** `discovery.rs` reads id/role/host-API-version/capabilities **off the compiled
  component itself** — there is no manifest, no signature, no publisher identity. The module comment
  already points here: "the bundle-metadata format (ADR 0007 §8) ... is deferred to ADR 0014."
- **Grants:** `Capability`/`Grants` (deny-by-default, `capability-error::denied`) are **hardcoded at
  each call site** (`assisted_grants()`, the CLI import/export grant sets). There is no per-plugin,
  user-controlled grant; per-plugin config is only an enable/disable `[plugins] disabled` list in the
  workspace manifest (`PluginPreferences`).
- **Crypto in tree:** `sha2` is a first-party dep (media-store checksums); `ed25519-dalek` and the
  `signature`/`curve25519-dalek` stack already resolve transitively in `Cargo.lock` (permissive,
  already `cargo deny`-cleared).

The research surveyed VS Code/Open VSX (distributor signs, client verifies against a baked root),
browser extensions (manifest-declared permissions surfaced for approval), Zed (first-party vs
third-party WASM components), and minisign (ed25519 + a one-key trust root). All converge on the shape
ADR 0007 §9 and ADR 0011's `Grants` already imply.

This ADR sits above `vitni-app` (ADR 0006), extends the plugin host (ADR 0011), and reuses the
`ConfigStore` scopes (ADR 0015) and the layered-override precedent (ADR 0003/0005). It does not restate
them.

## Decision

1. **Signing primitive: ed25519 detached signatures over a `sha2` digest, verified against an embedded
   trust root.** `ed25519-dalek` is promoted to a direct dependency of `vitni-plugin-host`. A
   bundle is signed by computing a canonical SHA-256 digest over its manifest **and** its component
   (so neither can be swapped independently) and producing a 64-byte detached signature. The
   **project's ed25519 public key is embedded in the binary** as the sanctioned trust root. RSA/ECDSA
   are rejected (larger keys, parameter/nonce hazards; only present transitively).

2. **The bundle format ADR 0007 §8 deferred.** A plugin is a directory containing:
   - `plugin.toml` — `id`, `version` (the plugin's own semver, distinct from the host-API version),
     `publisher`, `host_api` (the `vitni:host-api` version it pins), `role` (bulk-import /
     bulk-export / assisted-import / ui-panel), and `capabilities` (the **authoritative
     grant-request** the UX surfaces).
   - `plugin.wasm` — the component.
   - `plugin.sig` — the ed25519 detached signature over the canonical digest of `plugin.toml` +
     `plugin.wasm`.

   Discovery still cross-checks the manifest's declared `capabilities`/`role`/`host_api` against what
   the component actually imports/exports (as `discovery.rs::inspect` already reads); a manifest that
   under- or over-declares relative to the component's real world is a **load error**, so the manifest
   cannot lie about what the code will attempt. The bare-`.wasm` dev path is replaced by the bundle;
   `cargo xtask build-plugins` produces bundles and (for first-party plugins) signs them.

3. **Three trust tiers.**
   - **Sanctioned** — `plugin.sig` verifies against the embedded project key. Every declared capability
     is grantable.
   - **User-trusted** — `plugin.sig` verifies against a publisher key the user has **pinned** in a
     client/presentation-scope **trust store** (`(publisher, ed25519-pubkey)` entries, ADR 0015 client
     scope). Trusted like sanctioned, for that user only.
   - **Untrusted** — unsigned, or signed by an unknown key. **Loadable** (ADR 0007 §9 — not a walled
     garden) but **never auto-granted**; the grant UX forces an explicit per-capability decision.

   Verification failure of a *present* signature (tampered bundle, wrong key) is a hard load error —
   distinct from *absent* signature (untrusted-but-loadable). Trust tier never widens the sandbox: a
   sanctioned plugin still only ever gets its declared capabilities, still deny-by-default.

4. **Three-layer loading, mirroring the i18n `AssetsMultiplexor`** (ADR 0007 §4). Bundles are
   discovered across, highest precedence first:
   1. **Workspace** — `<workspace>/plugins/`.
   2. **App-dir** — the shared app plugin dir under the config resolver's `shared_app_dir`
      (`~/.local/share/vitni/plugins` / `~/.config/vitni/plugins`).
   3. **Embedded** — the sanctioned first-party fleet shipped with the binary.

   Resolution is **id-keyed**: a higher layer overrides a lower layer for the same id; within a layer,
   higher plugin semver wins. Missing layers are skipped (as `layered_assets` skips absent dirs). This
   replaces the flat `plugins_dir()` in `vitni-cli` and `vitni-ui-dioxus` with one shared
   resolver in `vitni-app`.

5. **Capability-grant UX: declared ∩ user-approved, persisted per plugin, surfaced interactively.**
   - The **effective grant** for a plugin is the intersection of the capabilities it **declares** in
     `plugin.toml` and the capabilities the **user has approved** — still deny-by-default, still gated
     per host call by `capability-error::denied`. Call sites stop hardcoding grants; they pass the
     resolved effective grant.
   - **Persisted approval** lives in the **workspace manifest** beside `[plugins] disabled`
     (workspace-functionality scope, `PluginPreferences`): a per-plugin approved-capability set. A
     shared/remote workspace therefore grants identically for every client, consistent with where
     `disabled` already lives.
   - **Interactive first-load approval (GUI).** With no persisted decision, the GUI shows the plugin's
     trust tier and declared capabilities and the user approves/denies. Sanctioned/user-trusted plugins
     may present "approve all declared" as one action; untrusted plugins force a per-capability choice.
     The decision persists. The `plugin_panel`/`preferences` screens (which already surface
     `discover()`) host this; the `plugin-manager.html` mockup is updated.
   - **CLI / headless** is config-driven only: grants come from the persisted manifest set; an
     ungranted capability produces an actionable error naming the capability and how to grant it. No
     hidden auto-grant on any frontend.

6. **Signing key management.** The project signing **private key never lives in the repository**; it is
   held as a release secret (CI secret / maintainer keychain) and used by `cargo xtask` at release
   time to sign the first-party bundles. Only the **public** key is committed and embedded. Key
   rotation is supported by embedding a small set of valid project public keys (current + previous) so
   a rotation does not instantly invalidate already-distributed bundles; revocation of a compromised
   key is a binary release that drops it from the embedded set. A full transparency log / OCSP-style
   online revocation (sigstore) is out of scope for a self-distributed first-party fleet.

7. **Packaging carries the trust root and the embedded fleet.** Because the sanctioned tier and the
   embedded loading layer both depend on shipping signed first-party bundles with the binary, the
   packaging workstream (Linux-first: AppImage + `.deb` for the GUI, tarball / `cargo install` for the
   CLI) bundles the signed fleet and the embedded public key(s). Cross-platform packaging
   (macOS/Windows, OS-level code signing/notarization) is **out of scope** here and deferred to a
   later cycle.

## Rationale

- **ed25519 + embedded root vs a checksum-only manifest.** A checksum manifest proves integrity but not
  authenticity — it cannot satisfy ADR 0007 §9's "signed and verified" for the sanctioned tier, and
  gives no basis for a user-trusted publisher tier. ed25519 is the minisign/signify primitive, already
  in-tree, with a trivial one-key trust root and no parameter footguns. minisign's *format* was
  rejected only to avoid a new dependency; the primitive is adopted.
- **Sign manifest + code together.** Signing only the `.wasm` would let an attacker keep a valid code
  signature while swapping the declared-capability manifest (a privilege-escalation request); signing
  the canonical digest of both closes that.
- **Three tiers vs two.** Two tiers (sanctioned/untrusted) is the minimum ADR 0007 §9 requires, but a
  user-pinned publisher tier is the difference between "first-party only" and a real third-party
  ecosystem for 1.0. Its only added surface is a client-scope trust store and its add/remove UX —
  modest, and it reuses the existing `ConfigStore` client scope.
- **Grants in the workspace manifest.** The plugin enable/disable list already lives there as
  workspace-functionality state; the approved-capability set is the same kind of dataset-scoped
  decision (shared identically across a remote workspace's clients), so it belongs beside it, not in
  client scope.
- **Loading mirrors `layered_assets` exactly** as ADR 0007 §4 requires, so the override semantics match
  locales and config — one mental model across the app, and one shared resolver instead of the two
  duplicated flat `plugins_dir()` helpers.
- **Trust never widens the sandbox.** Keeping effective grant = declared ∩ approved, gated per call,
  means signing changes *who you believe wrote it*, never *what it can do* — the ADR 0007 §12 boundary
  discipline.

## Consequences

### Positive

- Closes ADR 0007 §8 (bundle format) and §9 (signing/verification) and ADR 0011 §6 (layered loading,
  trust tiers) — the last plugin-system decisions deferred since the spike.
- A tampered or wrong-key bundle fails to load; a first-party bundle is provably authentic; a user can
  extend trust to a third-party publisher without a walled garden.
- The user finally sees and controls what each plugin may do (browser-extension-grade transparency),
  with the decision persisted per dataset.
- One layered loader replaces two duplicated flat helpers; the override model matches locales/config.

### Negative / costs

- The dev inner loop changes: `target/plugins/*.wasm` becomes signed bundles; `cargo xtask
  build-plugins` must generate a manifest, (dev-)sign, and lay out the bundle, and the CLI/UI loaders
  must switch to bundle discovery. A dev-mode signing key (checked-in *public*, ephemeral private, or a
  "dev-unsigned = untrusted-but-loadable" path) is needed so local iteration does not require the
  release secret.
- New key-management responsibility (release secret, rotation set, revocation-by-release).
- `ed25519-dalek` becomes a direct dependency (already `cargo deny`-cleared transitively).
- A trust-store UX and a first-load grant prompt are new UI surfaces (mockup + `vitni-ui`
  view-models + `vitni-ui-dioxus` screens), plus a CLI grant/trust command surface.

## Out of scope

- **Cross-platform packaging and OS-level code signing / notarization** (macOS Gatekeeper, Windows
  Authenticode). 1.0 is Linux-first (AppImage + `.deb` + tarball); other OSes are a later cycle.
- **A plugin marketplace / registry / auto-update / remote fetch.** Bundles are installed manually into
  a layer; discovery and remote distribution of third-party plugins is future work.
- **Transparency-log / online revocation (sigstore-style).** Revocation is by binary release dropping a
  key from the embedded set.
- **Finer-grained (sub-capability) permissions** — e.g. per-host `net` allowlists as user-editable
  grants. `net`'s allowlist stays the grant-site `NetPolicy` (ADR 0017); a user-editable override was
  already deferred there to this ADR and remains a follow-up, not built here.
- **Per-plugin resource-budget overrides** in config — fuel/memory limits stay host-set (ADR 0011 §4).
- **Signing the host binary itself** / supply-chain attestation of the app build.

## References

- ADR 0007 — the plugin system: §4 (layered loading like the i18n multiplexor), §8 (deferred bundle
  metadata), §9 (signed sanctioned plugins; untrusted loadable, never auto-granted), §12 (coarse
  capability boundary; signing never widens the sandbox).
- ADR 0011 — the host this extends: §2 (deny-by-default `Grants`, `capability-error::denied`), §4
  (resource limits, unchanged), §6 (layered override / signing / trust tiers deferred here).
- ADR 0003 / 0005 — the workspace > app-dir > embedded override precedent this loading order mirrors.
- ADR 0015 — `ConfigStore` scopes: the user trust store is client-scope; per-plugin grants are
  workspace-functionality scope.
- ADR 0017 — the grant-site `NetPolicy` allowlist whose user-editable override is deferred to this ADR
  (and stays out of scope here).
- `docs/research/plugin-signing-and-trust.md` — the ecosystem survey and the in-tree crypto findings
  this decision rests on.
- `crates/vitni-plugin-host/src/{lib.rs,discovery.rs,capability.rs}`, `vitni-i18n/src/lib.rs`
  (`layered_assets`), `vitni-app/src/{config.rs,workspace.rs}` — the code this ADR reshapes.
- `docs/roadmap.md` Phase 11; `docs/mockups/plugin-manager.html`.
