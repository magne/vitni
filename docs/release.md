# Releasing (Linux)

Phase 11 workstream C. Scope is **Linux-first** (ADR 0014 §7): a tarball, a `.deb`, and an AppImage,
each carrying the two shipped binaries and the **signed** first-party plugin fleet. macOS/Windows and
OS-level code signing/notarization are **out of scope** (deferred cycle, ADR 0014 §7 / Out of scope).

## Artifacts

`cargo xtask package` and the release workflow produce:

| Artifact                                   | Contents                                                                                     |
| ------------------------------------------ | -------------------------------------------------------------------------------------------- |
| `vitni-<ver>-linux-<arch>.tar.gz`      | `vitni` (CLI) + `vitni-gui` (GUI) + `plugins/` (signed fleet) + README + `.desktop` + icon |
| `vitni_<ver>_<arch>.deb` (CLI)         | `/usr/bin/vitni` + fleet in `/usr/lib/vitni/plugins`                                  |
| `vitni-gui_<ver>_<arch>.deb` (GUI)     | `/usr/bin/vitni-gui` + `.desktop` + icon + fleet in `/usr/lib/vitni/plugins`         |
| `Vitni-x86_64.AppImage`                | self-contained GUI + bundled fleet (its `AppRun` points `VITNI_PLUGIN_DIR` at the fleet) |

The plugin fleet is laid out as the ADR 0014 §4 **embedded layer**: one bundle directory per plugin
(`<id>/{plugin.toml,plugin.wasm,plugin.sig,i18n/}`). Both binaries resolve the embedded layer from
`$VITNI_PLUGIN_DIR` (when set) else the dev source tree; a packaged install points that variable
at the shipped fleet.

## Signing keys (ADR 0014 §6)

- **Private signing key** — the ed25519 seed that signs the sanctioned fleet. **Never in the repo.**
  Held as the CI secret `VITNI_PLUGIN_SIGNING_KEY` (64 hex chars = a 32-byte seed) and used by
  `cargo xtask build-plugins` / `cargo xtask package` at release time. When unset, both fall back to
  the deterministic **dev key** (`vitni_plugin_host::signing::DEV_SEED`) — fine for local
  iteration, never a release.
- **Public trust root** — the matching ed25519 public key (64 hex). Provided to the release build as
  `VITNI_PROJECT_PUBLIC_KEY`, read via `option_env!` at **compile time** and embedded in every
  release binary as the sanctioned trust root (`trust.rs`). A debug/CI build embeds the dev public key
  instead, so locally-signed bundles classify as `Sanctioned` in tests; a release build that is given
  no `VITNI_PROJECT_PUBLIC_KEY` trusts **no** sanctioned key. The public key is safe to commit.

### Guarding against an accidental dev-signed release

The release workflow asserts both secrets are present before it builds anything and aborts with a
clear error otherwise, so a misconfigured tag build can never silently ship dev-signed bundles with no
sanctioned trust root.

### Rotation and revocation

- **Rotation** — embed the current **and** the previous public key(s) in the binary's sanctioned set
  so already-distributed bundles keep verifying across a key change (`trust.rs` classifies against any
  embedded key). (Today `VITNI_PROJECT_PUBLIC_KEY` carries one key; a rotation set extends this.)
- **Revocation** — cut a new binary release that **drops** the compromised key from the embedded set.
  There is no online/transparency-log revocation (out of scope, ADR 0014 §6) — revocation is a
  release.

## Cutting a release

1. Bump the workspace version (`Cargo.toml` `[workspace.package] version`) and land it.
2. Ensure the release secrets are configured on the repository: `VITNI_PLUGIN_SIGNING_KEY` and
   `VITNI_PROJECT_PUBLIC_KEY`.
3. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
4. `.github/workflows/release.yml` (triggered on `v*`) builds the release, release-signs the fleet,
   assembles the tarball (`cargo xtask package`), builds the `.debs` (`cargo deb`) and the AppImage,
   and uploads all of them to the GitHub Release for the tag.

> The release workflow only runs when the account's GitHub Actions billing is active. It is verified
> by zizmor, YAML validity, and local reproduction of its steps.

## Building artifacts locally

- **Tarball** (no external tooling): `cargo xtask package` → `target/dist/vitni-<ver>-linux-<arch>.tar.gz`.
  Set `VITNI_PLUGIN_SIGNING_KEY` to release-sign; otherwise the dev key is used and every staged
  bundle's signature is still re-verified before the tarball is written.
- **`.deb`** (needs `cargo install cargo-deb`): run `cargo xtask build-plugins` first (the fleet must
  exist), then `cargo deb -p vitni-cli` and `cargo deb -p vitni-ui-dioxus -- --features desktop`
  → `target/debian/*.deb`.
- **AppImage**: needs `appimagetool` (the workflow downloads a SHA-256-pinned `continuous` build).

## Residuals

- **System default embedded path.** The `.deb` installs the fleet to `/usr/lib/vitni/plugins`, but
  the loader has no built-in default for that path yet — a `.deb`-installed GUI/CLI needs
  `VITNI_PLUGIN_DIR=/usr/lib/vitni/plugins` (the AppImage sets it via `AppRun`; the tarball
  user sets it to the fleet beside the binary). Teaching the embedded layer a packaged default is a
  follow-up.
- **Architectures.** Only `x86_64` is wired (the AppImage step and the CI image publish amd64); arm64
  is a later addition mirroring the ci-image arch-aware setup.
