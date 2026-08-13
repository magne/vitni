# Research — plugin signing, trust tiers, and layered loading (Phase 11, gates ADR 0014)

- **Status:** Findings informing ADR 0014 (plugin signing, trust tiers, capability-grant UX, and
  three-layer loading).
- **Date:** 2026-07-24

## Question

ADR 0007 §9 fixed the *direction* — sanctioned/bundled plugins **must be signed and verified before
load**; unsanctioned plugins may load unsigned but are treated as untrusted and never auto-granted
sanctioned-only capabilities — and ADR 0011 §6 deferred "the three-layer override order, plugin
signing, trust tiers, and bundle verification" to this decision. Today the loader is a single flat
directory (`PluginHost::load_by_id(plugins_dir, id)` → `plugins_dir/{id}.wasm`, `target/plugins` by
default), discovery reads metadata off the compiled component itself (no manifest), and every
capability grant is hardcoded at the call site (`Grants::none().with(..)` in the CLI/UI). There is no
bundle format, no signature, no trust distinction, and no user-facing grant surface.

This asks: what signing primitive, what trust taxonomy, what grant UX, and what loading precedence
should a 1.0 hardening slice adopt — grounded in what shipping plugin ecosystems actually do and in
the primitives already in this workspace's dependency tree.

## What comparable ecosystems do

- **VS Code / Open VSX.** Extensions are distributed as unsigned `.vsix` bundles historically;
  VS Code added Marketplace-side **signature verification** (2023) where the *marketplace* signs the
  package and the client verifies against a baked-in root. Capabilities are coarse (full Node access);
  there is no per-extension capability grant — the trust decision is binary (install or not). Lesson:
  a **publisher/distributor signs, the client verifies against an embedded root**; do not ask the
  plugin author to self-certify trust.
- **Zed extensions** (the model ADR 0007/0011 explicitly cite for WIT-world versioning). Extensions
  are WASM components pinned to a versioned host API; first-party/bundled extensions ship in-tree,
  third-party ones are fetched. Trust is "we built it" vs "someone else did". This maps directly onto
  the **sanctioned vs untrusted** split ADR 0007 §9 already committed to.
- **Browser extensions (Chrome/Firefox).** The store signs `.crx`/`.xpi`; the browser verifies; and —
  crucially — **permissions are declared in a manifest and surfaced to the user at install time** with
  an approve/deny prompt ("this extension can: read your browsing history"). This is the precedent for
  a **capability-grant UX driven by manifest-declared permissions**, exactly the shape ADR 0011's
  deny-by-default `Grants` already produces (the declared set is discoverable from the component today).
- **`cargo`/crates.io.** No signing of crates themselves; trust is TLS + the registry. `cargo-dist` and
  `sigstore` are the emerging Rust-native signing paths. Lesson: for a self-hosted first-party fleet,
  **detached signatures with a project-held key are simpler and sufficient** — a full transparency-log
  (sigstore) is overkill for a handful of bundled plugins.
- **`minisign`/`signify` (OpenBSD).** Ed25519 detached signatures over a file, tiny trust root (one
  public key), no PKI. This is the closest match to what a self-distributed app needs, and validates
  the **ed25519 + single embedded public key** choice; adopting the *format* would add a dependency,
  but adopting the *primitive* (`ed25519-dalek`) does not.

**Synthesis.** Every mainstream model converges on: (1) the *distributor* signs, not the author; (2)
the client verifies against a **root baked into the client**; (3) unsigned/third-party content is
loadable but **restricted and never silently privileged**; (4) permissions are **declared and
surfaced for approval**, not implicit. All four already align with ADR 0007 §9 and ADR 0011's
deny-by-default `Grants`.

## Signing primitive — what's already in the tree

- **`sha2` is a first-party direct dependency** (`sha2 = "0.11.0"`), already used by the plugin-host
  media-store (`media.rs`, `"sha256:<hex>"`). A signature over a `sha2` digest reuses proven code.
- **`ed25519-dalek`, `ed25519`, `signature`, `curve25519-dalek` already resolve in `Cargo.lock`**
  (transitively, via SSH tooling) — so they are permissive-licensed (BSD-3/MIT) and already build in
  this tree. Promoting `ed25519-dalek` to a direct dependency of `vitni-plugin-host` adds no new
  license or supply-chain surface that `cargo deny` hasn't already cleared.
- Ed25519 gives small (64-byte) detached signatures, small (32-byte) public keys, fast verification,
  no parameter/curve-choice footguns, and a trivial trust root (embed one 32-byte public key). This is
  the right primitive; RSA (large keys, parameter choices) and ECDSA/p256 (nonce hazards) are worse
  fits and only present transitively.

## Trust taxonomy — three tiers

The owner chose a three-tier model over the minimal two-tier (sanctioned/untrusted):

1. **Sanctioned.** The bundle is signed by the **project key** whose public half is embedded in the
   binary. Fully trusted: every capability the bundle declares is grantable (still subject to the
   deny-by-default grant, §grant-UX). These are the first-party plugins (`gedcom-*`, `gramps-*`,
   `digitalarkivet-import`, `ui-panel`) signed at build time by `cargo xtask`.
2. **User-trusted.** The bundle is signed by a **publisher key the user has explicitly pinned** into
   their client/presentation config (a local trust store of `(publisher-name, ed25519-pubkey)`).
   Verified against that pinned key; trusted like a sanctioned plugin *for that user only*. This is the
   third-party-ecosystem tier — a plugin author distributes their public key out-of-band, the user
   adds it once, and subsequent bundles from that author verify automatically.
3. **Untrusted.** Unsigned, or signed by an unknown key. Loadable (ADR 0007 §9 — the app is not a
   walled garden) but **never auto-granted**; the grant UX must force an explicit per-capability
   decision and may withhold the most dangerous capabilities entirely by policy.

The three-tier split's only real cost over two-tier is the **user trust store** (a config-scope list
of pinned keys + the UX to add/remove one) — modest, and it is the difference between "first-party
only" and "a real third-party plugin ecosystem" for 1.0.

## Bundle format — the deferred ADR 0007 §8 metadata

Signing requires something stable to sign and a place to put the signature and the declared
capabilities, because today's discovery reads everything off the compiled component and a raw `.wasm`
has nowhere to carry a detached signature or a publisher identity. So ADR 0014 must finally define the
**bundle** ADR 0007 §8 deferred: a plugin becomes a small directory (or a manifest + component) with

- `plugin.toml` — id, semver (plugin's own, not the host-API version), publisher name, declared
  capabilities (the authoritative grant-request the UX surfaces), host-API version, entry role;
- `plugin.wasm` — the component;
- `plugin.sig` — an ed25519 detached signature over a canonical digest of `plugin.toml` + `plugin.wasm`
  (sign the manifest **and** the code so neither can be swapped independently).

Discovery keeps cross-checking the manifest's declared capabilities against what the component actually
imports (a manifest that under-declares its imports, or over-declares beyond its world, is rejected) —
so the manifest cannot lie about what the code will attempt.

## Three-layer loading — mirror the i18n multiplexor

ADR 0007 §4 says plugin loading must layer "exactly as the i18n `AssetsMultiplexor`". That code is
`vitni-i18n::layered_assets(workspace_dir, shared_dir, embedded)` — an ordered list, **highest
precedence first**, missing layers skipped. Applied to plugins:

1. **Workspace** — `<workspace>/plugins/` (per-dataset plugins; highest precedence).
2. **App-dir** — `~/.config/vitni/plugins/` (or the shared-app-dir the config resolver already
   computes, `config.rs` `shared_app_dir`) — user-installed, cross-workspace plugins.
3. **Embedded** — the sanctioned first-party fleet shipped with the binary (today's `target/plugins`
   in dev; embedded/packaged at release).

Resolution: discover across all layers, key by stable plugin id, **a higher layer overrides a lower
one for the same id**, and when the same layer/id collides prefer the higher plugin semver. This lets a
workspace pin a specific plugin build, a user install one app-wide, and the binary always fall back to
the sanctioned baseline — the same override model ADR 0003 (locales) and ADR 0005 (config) already use.

## Capability-grant UX

The grant model exists (`Grants`, deny-by-default, `capability-error::denied`); what's missing is
*who decides the grant*. Today it's hardcoded per call site. ADR 0014 moves the decision to
per-plugin, persisted state plus an interactive first-load prompt:

- **Persisted grants** live in the **workspace manifest** alongside the existing `[plugins] disabled`
  list (workspace-functionality scope, `PluginPreferences` in `workspace.rs`) — a per-plugin granted
  capability set. This is dataset state (a remote/shared workspace grants identically for every client),
  which matches where `disabled` already lives.
- **Interactive prompt (GUI).** On first load of a plugin with no persisted grant decision, the GUI
  shows the plugin's declared capabilities and its trust tier, and the user approves/denies (browser-
  extension model). Sanctioned plugins may default to "grant all declared" with one confirmation;
  untrusted plugins force a per-capability choice. The decision persists to the manifest. The
  `plugin_panel`/`preferences` screens and the `plugin-manager.html` mockup already surface
  `discover()` output, so the data is in hand.
- **CLI / headless.** Non-interactive: grants come from config only; an ungranted capability yields a
  clear actionable error telling the user which capability to grant and where. No hidden auto-grant.

The call-site hardcoded grants (`assisted_grants()`, the CLI import/export grant sets) become the
**declared** set the plugin requests; the *effective* grant is `declared ∩ user-approved`, still
deny-by-default.

## Recommendation (feeds ADR 0014)

1. **Signing:** ed25519 detached signature (`ed25519-dalek`, promoted to a direct dep) over a `sha2`
   digest of the bundle manifest + component; one project public key embedded as the trust root.
2. **Trust tiers:** sanctioned (project key) / user-trusted (user-pinned publisher key in a
   client-scope trust store) / untrusted (unsigned or unknown key — loadable, never auto-granted).
3. **Bundle:** define the ADR 0007 §8 `plugin.toml` + `plugin.wasm` + `plugin.sig`; discovery
   cross-checks declared vs actual capability imports.
4. **Loading:** three-layer workspace > app-dir > embedded, mirroring `layered_assets`, id-keyed with
   higher-layer and higher-semver override.
5. **Grant UX:** per-plugin granted-capability set persisted in the workspace manifest; interactive
   GUI first-load approval keyed on trust tier; CLI config-driven with actionable denied errors.

## References

- ADR 0007 §4 (layered loading like the i18n multiplexor), §8 (deferred bundle metadata format), §9
  (signed-and-verified sanctioned plugins; untrusted loadable but never auto-granted), §12 (coarse
  capability boundary).
- ADR 0011 §2 (deny-by-default `Grants`, `capability-error::denied`), §6 (three-layer override /
  signing / trust tiers / bundle verification deferred to ADR 0014).
- ADR 0003 / 0005 — the workspace > app-dir > embedded override precedent (`layered_assets`, the config
  resolver) this loading order mirrors.
- ADR 0015 — the `ConfigStore` scopes: the user trust store is client/presentation scope; per-plugin
  grants are workspace-functionality scope.
- `crates/vitni-plugin-host/src/{lib.rs,discovery.rs,capability.rs}` — today's flat loader, the
  manifest-free discovery, and the `Grants` model.
- `crates/vitni-i18n/src/lib.rs` (`layered_assets`) — the multiplexor to mirror.
- minisign/signify, VS Code Marketplace signing, browser-extension permission prompts, Zed extension
  trust model — the ecosystem precedents.
