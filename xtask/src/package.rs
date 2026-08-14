//! `package` — assembles a Linux release tarball under `target/dist/` (Phase 11 workstream C,
//! ADR 0014 §7).
//!
//! The tarball carries the two shipped binaries (the `vitni` launcher and the headless `vitni-cli`,
//! release profile — ADR 0035 §1), the **signed** first-party plugin fleet laid out as the embedded
//! loading layer (a `plugins/` directory beside the binaries), the project README, and a `.desktop`
//! launcher plus the icon set (`share/icons/hicolor/…` and a top-level `vitni.png`, which is where an
//! `AppImage` looks). Plugins are (re)built and signed through [`crate::build_plugins`], so the
//! signing key resolves exactly as it does there: the release key from `VITNI_PLUGIN_SIGNING_KEY`
//! when set, else the deterministic dev key ([`vitni_plugin_host::signing::resolve_signing_key`]).
//!
//! Every emitted bundle's signature is re-verified against that key before the tarball is written, so
//! a broken or unsigned bundle fails the command loudly rather than shipping.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use vitni_plugin_host::signing::{self, PluginManifest};

use crate::build_plugins;
use crate::util::copy_dir;

/// The release version (inherited from the workspace `[package] version`).
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Where the assembled tarball and its staging tree are written.
const DIST_DIR: &str = "target/dist";
/// Where `build-plugins` collects the signed bundle fleet (the embedded layer, ADR 0014 §4).
const PLUGIN_DIR: &str = "target/plugins";
/// The release binary output directory.
const RELEASE_DIR: &str = "target/release";

/// The `vitni` launcher binary (crate `vitni`, `gui` feature, release profile).
const LAUNCHER_BIN: &str = "vitni";
/// The headless `vitni-cli` binary (crate `vitni-cli`, release profile).
const CLI_BIN: &str = "vitni-cli";

/// The committed GUI launcher + icon set reused by both the tarball and the `.deb` (`cargo deb`).
const DESKTOP_SRC: &str = "crates/vitni-ui-dioxus/assets/vitni.desktop";
const ICON_DIR: &str = "crates/vitni-ui-dioxus/assets/icon";

/// The icon sizes `cargo xtask icons` generates, staged as a `hicolor` theme tree (#326).
const ICON_SIZES: [u16; 7] = [16, 24, 32, 48, 64, 128, 256];
/// The size copied to the staging root as `vitni.png`: an `AppImage` takes its icon from the `AppDir` root.
const ROOT_ICON_SIZE: u16 = 256;
/// The monochrome variant, installed into the theme's `symbolic` directory for GNOME and the tray.
const SYMBOLIC_ICON: &str = "vitni-symbolic.svg";

/// One verified bundle, for the closing summary.
struct Verified {
    id: String,
    manifest: PluginManifest,
}

/// Runs the `package` command (see module docs).
pub fn run() -> Result<()> {
    build_plugins::run().context("building and signing the plugin fleet")?;
    build_release_binaries()?;

    let package_name = format!("vitni-{VERSION}-{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let dist = Path::new(DIST_DIR);
    let stage = dist.join(&package_name);
    reset_dir(&stage)?;

    copy_binaries(&stage)?;
    copy_plugin_fleet(&stage)?;
    copy_docs(&stage)?;
    install_desktop_entry(&stage)?;
    install_icons(&stage)?;

    let verified = verify_fleet(&stage)?;
    let tarball = make_tarball(dist, &package_name)?;
    print_summary(&tarball, &verified);
    Ok(())
}

/// Builds both shipped binaries in the release profile. The launcher's `gui` feature (the webview
/// renderer) is on by default, so neither build needs a `--features` flag.
fn build_release_binaries() -> Result<()> {
    println!("package: building release binaries");
    crate::util::run_cargo(&["build", "--release", "-p", "vitni-cli"])?;
    crate::util::run_cargo(&["build", "--release", "-p", "vitni"])?;
    Ok(())
}

/// Copies the two release binaries to the staging root, so `plugins/` sits beside them (the embedded
/// layer a frontend resolves relative to the binary — ADR 0014 §4).
fn copy_binaries(stage: &Path) -> Result<()> {
    for binary in [LAUNCHER_BIN, CLI_BIN] {
        let source = Path::new(RELEASE_DIR).join(binary);
        if !source.is_file() {
            bail!(
                "release binary {} is missing — did the release build succeed?",
                source.display()
            );
        }
        let dest = stage.join(binary);
        fs::copy(&source, &dest).with_context(|| format!("copying {} to {}", source.display(), dest.display()))?;
        set_executable(&dest)?;
    }
    Ok(())
}

/// The manifest `role` of a plugin that exists only to exercise the host in tests; never shipped.
const TEST_FIXTURE_ROLE: &str = "test-fixture";

/// Copies the signed bundle fleet from `target/plugins` into `<stage>/plugins`, the embedded layer,
/// skipping any test-fixture bundle (built for the host tests, not part of the shipped fleet).
fn copy_plugin_fleet(stage: &Path) -> Result<()> {
    let source = Path::new(PLUGIN_DIR);
    if !source.is_dir() {
        bail!(
            "{} is missing — build-plugins did not produce a fleet",
            source.display()
        );
    }
    let dest = stage.join("plugins");
    for bundle in crate::util::child_dirs(source)? {
        if bundle_role(&bundle)?.as_deref() == Some(TEST_FIXTURE_ROLE) {
            let id = bundle.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            println!("package: skipping {id} (test-fixture, not shipped)");
            continue;
        }
        let Some(name) = bundle.file_name() else {
            continue;
        };
        copy_dir(&bundle, &dest.join(name))?;
    }
    Ok(())
}

/// The declared `role` in a bundle's `plugin.toml`, or `None` when the directory carries no manifest.
fn bundle_role(bundle: &Path) -> Result<Option<String>> {
    let manifest_path = bundle.join("plugin.toml");
    if !manifest_path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&manifest_path).with_context(|| format!("reading {}", manifest_path.display()))?;
    let manifest: PluginManifest =
        toml::from_str(&text).with_context(|| format!("parsing {}", manifest_path.display()))?;
    Ok(Some(manifest.role))
}

/// Copies the top-level docs the tarball ships (README, NOTICE, and the licence texts).
///
/// All three licences ship: the tarball carries both the AGPL application and the permissive
/// interchange crates, and `NOTICE` is what maps crate to licence (ADR 0034).
fn copy_docs(stage: &Path) -> Result<()> {
    copy_if_present(Path::new("README.md"), &stage.join("README.md"))?;
    copy_if_present(Path::new("NOTICE"), &stage.join("NOTICE"))?;
    for license in ["LICENSE-AGPL", "LICENSE-MIT", "LICENSE-APACHE"] {
        copy_if_present(Path::new(license), &stage.join(license))?;
    }
    Ok(())
}

/// Installs the GUI `.desktop` launcher into the staging root, copying the committed one when it
/// exists and generating a minimal launcher otherwise.
fn install_desktop_entry(stage: &Path) -> Result<()> {
    let desktop_src = Path::new(DESKTOP_SRC);
    let desktop_dest = stage.join("vitni.desktop");
    if desktop_src.is_file() {
        copy_if_present(desktop_src, &desktop_dest)?;
    } else {
        fs::write(&desktop_dest, generated_desktop_entry())
            .with_context(|| format!("writing {}", desktop_dest.display()))?;
    }
    Ok(())
}

/// Installs the icon set: a `hicolor` theme tree under `<stage>/share/icons` (every raster size plus
/// the symbolic SVG) and a copy of the largest raster as `<stage>/vitni.png`, which is where an
/// `AppImage` looks for its icon.
///
/// A missing raster is an error, not a warning: shipping a bundle whose launcher points at no icon is
/// the defect this replaced (#326). Regenerate them with `cargo xtask icons`.
fn install_icons(stage: &Path) -> Result<()> {
    let source = Path::new(ICON_DIR);
    let apps = |directory: &str| stage.join("share/icons/hicolor").join(directory).join("apps");

    for size in ICON_SIZES {
        let raster = source.join(format!("vitni-{size}.png"));
        let dest = apps(&format!("{size}x{size}"));
        copy_icon(&raster, &dest.join("vitni.png"))?;
        if size == ROOT_ICON_SIZE {
            copy_icon(&raster, &stage.join("vitni.png"))?;
        }
    }
    copy_icon(&source.join(SYMBOLIC_ICON), &apps("symbolic").join(SYMBOLIC_ICON))
}

/// Copies one icon file, creating its parent directory and failing loudly when the source is absent.
fn copy_icon(source: &Path, dest: &Path) -> Result<()> {
    if !source.is_file() {
        bail!(
            "icon {} is missing — run `cargo xtask icons` to generate the rasters",
            source.display()
        );
    }
    let parent = dest
        .parent()
        .with_context(|| format!("{} has no parent directory", dest.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    fs::copy(source, dest).with_context(|| format!("copying {} to {}", source.display(), dest.display()))?;
    Ok(())
}

/// A minimal `.desktop` launcher used only when the committed one is absent.
fn generated_desktop_entry() -> String {
    "[Desktop Entry]\n\
     Type=Application\n\
     Name=Vitni\n\
     Comment=Event-sourced genealogy program\n\
     Exec=vitni\n\
     Icon=vitni\n\
     Terminal=false\n\
     Categories=Office;Database;Utility;\n"
        .to_owned()
}

/// Re-verifies every staged bundle's `plugin.sig` against the signing key it was produced with
/// (release key when `VITNI_PLUGIN_SIGNING_KEY` is set, else the dev key), failing closed on any
/// missing or invalid signature so the tarball never ships an unverifiable bundle.
fn verify_fleet(stage: &Path) -> Result<Vec<Verified>> {
    let signing_key = signing::resolve_signing_key().context("resolving the plugin signing key")?;
    let verifying_key = signing_key.verifying_key();

    let plugins = stage.join("plugins");
    let mut bundles = crate::util::child_dirs(&plugins)?;
    bundles.sort();

    let mut verified = Vec::new();
    for bundle in bundles {
        verified.push(verify_bundle(&bundle, &verifying_key)?);
    }
    if verified.is_empty() {
        bail!("no plugin bundles were staged under {}", plugins.display());
    }
    Ok(verified)
}

/// Verifies one staged bundle: recomputes the canonical digest over `plugin.toml` + `plugin.wasm` and
/// checks `plugin.sig` against `verifying_key`.
fn verify_bundle(bundle: &Path, verifying_key: &signing::VerifyingKey) -> Result<Verified> {
    let id = bundle
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("non-UTF-8 bundle directory {}", bundle.display()))?
        .to_owned();

    let manifest_bytes = read_bundle_file(bundle, "plugin.toml")?;
    let wasm_bytes = read_bundle_file(bundle, "plugin.wasm")?;
    let signature_bytes = read_bundle_file(bundle, "plugin.sig")?;

    let signature = signing::signature_from_bytes(&signature_bytes)
        .with_context(|| format!("bundle {id} has a malformed plugin.sig"))?;
    let digest = signing::bundle_digest(&manifest_bytes, &wasm_bytes);
    signing::verify(verifying_key, &digest, &signature)
        .with_context(|| format!("bundle {id} signature does not verify against the signing key"))?;

    let manifest_text =
        String::from_utf8(manifest_bytes).with_context(|| format!("bundle {id} manifest is not UTF-8"))?;
    let manifest: PluginManifest =
        toml::from_str(&manifest_text).with_context(|| format!("parsing bundle {id} plugin.toml"))?;
    Ok(Verified { id, manifest })
}

/// Reads a required file from a bundle directory, erroring if it is absent.
fn read_bundle_file(bundle: &Path, name: &str) -> Result<Vec<u8>> {
    let path = bundle.join(name);
    fs::read(&path).with_context(|| format!("reading {} (a bundle must carry {name})", path.display()))
}

/// Creates `target/dist/<name>.tar.gz` from the staged tree with `tar`, and returns its path.
fn make_tarball(dist: &Path, package_name: &str) -> Result<PathBuf> {
    let tarball = dist.join(format!("{package_name}.tar.gz"));
    if tarball.exists() {
        fs::remove_file(&tarball).with_context(|| format!("removing stale {}", tarball.display()))?;
    }
    run_command(
        "tar",
        &[
            "-czf",
            &tarball.to_string_lossy(),
            "-C",
            &dist.to_string_lossy(),
            package_name,
        ],
    )?;
    Ok(tarball)
}

/// Prints the closing summary: the tarball path and each verified bundle's id + trust metadata.
fn print_summary(tarball: &Path, verified: &[Verified]) {
    println!();
    println!("package: summary");
    println!("  tarball: {}", tarball.display());
    println!("  verified plugin bundles ({}):", verified.len());
    for bundle in verified {
        let manifest = &bundle.manifest;
        println!(
            "    {id}: v{version} publisher={publisher} role={role} — signature verified",
            id = bundle.id,
            version = manifest.version,
            publisher = manifest.publisher,
            role = manifest.role,
        );
    }
    println!("package: {VERSION} tarball ready.");
}

/// Copies `source` to `dest` when `source` exists, reporting when it does not (a missing README or
/// license is a warning, not a failure).
fn copy_if_present(source: &Path, dest: &Path) -> Result<()> {
    if !source.is_file() {
        println!("package: skipping {} (not present)", source.display());
        return Ok(());
    }
    fs::copy(source, dest).with_context(|| format!("copying {} to {}", source.display(), dest.display()))?;
    Ok(())
}

/// Clears and recreates `dir` so a rebuild never leaves a stale staging tree behind.
fn reset_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        fs::remove_dir_all(dir).with_context(|| format!("clearing {}", dir.display()))?;
    }
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))
}

/// Marks a staged file executable on Unix (release binaries lose their mode through `fs::copy`).
#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .with_context(|| format!("reading permissions of {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).with_context(|| format!("setting mode on {}", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Runs `program` with `args`, failing if it exits non-zero or cannot be spawned.
fn run_command(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("running {program} {}", args.join(" ")))?;
    if !status.success() {
        bail!("{program} {} failed with {status}", args.join(" "));
    }
    Ok(())
}
