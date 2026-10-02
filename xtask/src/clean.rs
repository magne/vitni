//! `cargo xtask clean` — free the disk a workspace build fills, without forcing a dependency rebuild.
//!
//! `cargo clean` deletes everything, so the next build recompiles every dependency and the tests fail
//! until `cargo xtask build-plugins` repopulates `target/plugins`. This deletes only what is cheap to
//! regenerate: per profile of the workspace `target/` and of each `plugins/*/target/`, the
//! `incremental/` caches and the linked executables in `deps/` (the files with no extension). Every
//! `.rlib`/`.rmeta` and build-script output stays, so the next build recompiles and relinks only the
//! workspace's own crates. It also deletes the workspace's regenerable run output (`RUN_OUTPUT`), and
//! with `--full` the built plugins and the fetched external fixtures (`FULL_ONLY`).
//!
//! Deletion goes through `std::fs`, never the trash: the trash is on the same full filesystem.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::util;

/// Run output under the workspace `target/` that a later command regenerates on demand.
const RUN_OUTPUT: [&str; 6] = ["gui-pass", "screenshots", "doc", "criterion", "flycheck0", "tmp"];

/// Kept by default because rebuilding them is slow or needs the network: the plugin components
/// (`cargo xtask build-plugins`) and the fetched external fixtures (`cargo xtask fetch-fixtures`).
const FULL_ONLY: [&str; 2] = ["plugins", "external-fixtures"];

const USAGE: &str = "usage: cargo xtask clean [--full] [--dry-run]";

/// The flags `clean` accepts.
#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    full: bool,
    dry_run: bool,
}

/// One reported line: a directory, the paths deleted under it, and the bytes they hold.
#[derive(Debug)]
struct Removal {
    label: PathBuf,
    paths: Vec<PathBuf>,
    bytes: u64,
}

/// Deletes (or with `--dry-run`, only reports) the regenerable build output, printing what it freed
/// per directory.
pub fn run(args: &[String]) -> Result<()> {
    let options = parse_args(args)?;
    let plugins = Path::new("plugins");
    if !plugins.is_dir() {
        bail!("clean: no plugins/ directory here; run `cargo xtask clean` from the repository root");
    }
    let mut plugin_targets = Vec::new();
    for dir in util::child_dirs(plugins)? {
        plugin_targets.push(dir.join("target"));
    }
    let removals = plan(Path::new("target"), &plugin_targets, options.full)?;
    let verb = if options.dry_run { "would free" } else { "freed" };
    let mut total = 0;
    for removal in &removals {
        if !options.dry_run {
            apply(removal)?;
        }
        total += removal.bytes;
        let size = human_size(removal.bytes);
        let label = removal.label.display();
        if removal.label.ends_with("deps") {
            println!("{size:>10}  {label} ({} executables)", removal.paths.len());
        } else {
            println!("{size:>10}  {label}");
        }
    }
    println!("clean: {verb} {}", human_size(total));
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Options> {
    let mut options = Options::default();
    for arg in args {
        match arg.as_str() {
            "--full" => options.full = true,
            "--dry-run" => options.dry_run = true,
            other => bail!("clean: unknown argument {other:?}\n{USAGE}"),
        }
    }
    Ok(options)
}

/// Everything `clean` would delete, grouped by the directory it reports: the profiles of every target
/// root, then the workspace target's run output (and with `full`, its `FULL_ONLY` directories).
fn plan(workspace_target: &Path, plugin_targets: &[PathBuf], full: bool) -> Result<Vec<Removal>> {
    let mut removals = Vec::new();
    for root in std::iter::once(workspace_target).chain(plugin_targets.iter().map(PathBuf::as_path)) {
        for profile in profiles(root)? {
            plan_profile(&profile, &mut removals)?;
        }
    }
    let mut whole = RUN_OUTPUT.to_vec();
    if full {
        whole.extend(FULL_ONLY);
    }
    for name in whole {
        plan_whole(&workspace_target.join(name), &mut removals)?;
    }
    Ok(removals)
}

/// Adds the directory `dir` to `removals` as one line, unless it is absent or holds nothing.
fn plan_whole(dir: &Path, removals: &mut Vec<Removal>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let bytes = size_of(dir)?;
    if bytes > 0 {
        removals.push(Removal {
            label: dir.to_path_buf(),
            paths: vec![dir.to_path_buf()],
            bytes,
        });
    }
    Ok(())
}

/// The profile directories under a target root: `<root>/<profile>` for the host, and
/// `<root>/<triple>/<profile>` for a cross target such as `wasm32-wasip2`. A profile is recognised by
/// its `deps/` or `incremental/` directory; the `RUN_OUTPUT` and `FULL_ONLY` directories never are.
fn profiles(root: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for dir in util::child_dirs(root)? {
        let name = dir.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if RUN_OUTPUT.contains(&name) || FULL_ONLY.contains(&name) {
            continue;
        }
        if is_profile(&dir) {
            found.push(dir);
            continue;
        }
        for nested in util::child_dirs(&dir)? {
            if is_profile(&nested) {
                found.push(nested);
            }
        }
    }
    Ok(found)
}

fn is_profile(dir: &Path) -> bool {
    dir.join("deps").is_dir() || dir.join("incremental").is_dir()
}

/// Adds a profile's `incremental/` cache and its linked executables in `deps/` to `removals`.
fn plan_profile(profile: &Path, removals: &mut Vec<Removal>) -> Result<()> {
    plan_whole(&profile.join("incremental"), removals)?;
    let deps = profile.join("deps");
    if !deps.is_dir() {
        return Ok(());
    }
    let mut paths = Vec::new();
    let mut bytes = 0;
    for entry in fs::read_dir(&deps).with_context(|| format!("reading {}", deps.display()))? {
        let entry = entry.with_context(|| format!("reading an entry under {}", deps.display()))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).with_context(|| format!("reading {}", path.display()))?;
        if metadata.is_file() && path.extension().is_none() {
            bytes += metadata.len();
            paths.push(path);
        }
    }
    if !paths.is_empty() {
        paths.sort();
        removals.push(Removal {
            label: deps,
            paths,
            bytes,
        });
    }
    Ok(())
}

/// The bytes held by the files under `path`, not following symlinks.
fn size_of(path: &Path) -> Result<u64> {
    let metadata = fs::symlink_metadata(path).with_context(|| format!("reading {}", path.display()))?;
    if !metadata.is_dir() {
        return Ok(metadata.len());
    }
    let mut bytes = 0;
    for entry in fs::read_dir(path).with_context(|| format!("reading {}", path.display()))? {
        let entry = entry.with_context(|| format!("reading an entry under {}", path.display()))?;
        bytes += size_of(&entry.path())?;
    }
    Ok(bytes)
}

fn apply(removal: &Removal) -> Result<()> {
    for path in &removal.paths {
        let metadata = fs::symlink_metadata(path).with_context(|| format!("reading {}", path.display()))?;
        if metadata.is_dir() {
            fs::remove_dir_all(path).with_context(|| format!("deleting {}", path.display()))?;
        } else {
            fs::remove_file(path).with_context(|| format!("deleting {}", path.display()))?;
        }
    }
    Ok(())
}

/// `bytes` in the largest binary unit that keeps the number at least 1, to one decimal.
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a one-decimal display needs no more than f64's 52 bits"
    )]
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{Options, apply, human_size, parse_args, plan};

    fn write(root: &Path, relative: &str, bytes: usize) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0_u8; bytes]).unwrap();
    }

    /// A host profile as cargo lays it out: executables and libraries in `deps/`, build-script output
    /// in `build/`, the incremental cache, and the uplifted binaries at the profile root.
    fn host_profile(root: &Path, profile: &str) {
        for file in [
            "deps/vitni_core-0123456789abcdef",
            "deps/libserde-0123456789abcdef.rlib",
            "deps/libserde-0123456789abcdef.rmeta",
            "deps/serde-0123456789abcdef.d",
            "deps/libserde_derive-0123456789abcdef.so",
            "build/serde-0123456789abcdef/build-script-build",
            "build/serde-0123456789abcdef/out/private.rs",
            "incremental/vitni_core-abc/s-1/query-cache.bin",
            "vitni",
        ] {
            write(root, &format!("{profile}/{file}"), 10);
        }
    }

    fn clean(target: &Path, plugin_targets: &[std::path::PathBuf], full: bool) -> u64 {
        let removals = plan(target, plugin_targets, full).unwrap();
        let mut bytes = 0;
        for removal in &removals {
            apply(removal).unwrap();
            bytes += removal.bytes;
        }
        bytes
    }

    #[test]
    fn a_profile_loses_its_executables_and_incremental_cache_only() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        host_profile(&target, "debug");
        host_profile(&target, "release");

        assert_eq!(clean(&target, &[], false), 40);

        for profile in ["debug", "release"] {
            let profile = target.join(profile);
            assert!(!profile.join("deps/vitni_core-0123456789abcdef").exists());
            assert!(!profile.join("incremental").exists());
            for kept in [
                "deps/libserde-0123456789abcdef.rlib",
                "deps/libserde-0123456789abcdef.rmeta",
                "deps/serde-0123456789abcdef.d",
                "deps/libserde_derive-0123456789abcdef.so",
                "build/serde-0123456789abcdef/build-script-build",
                "build/serde-0123456789abcdef/out/private.rs",
                "vitni",
            ] {
                assert!(profile.join(kept).exists(), "{kept} was deleted");
            }
        }
    }

    #[test]
    fn cross_target_profiles_and_plugin_targets_are_cleaned() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let plugin = dir.path().join("plugins/gedcom-import/target");
        host_profile(&plugin, "release");
        write(
            &plugin,
            "wasm32-wasip2/release/deps/gedcom_import-0123456789abcdef.wasm",
            10,
        );
        write(
            &plugin,
            "wasm32-wasip2/release/incremental/gedcom_import-abc/s-1/dep-graph.bin",
            10,
        );
        write(&plugin, "wasm32-wasip2/release/deps/gedcom_import-0123456789abcdef", 10);

        assert_eq!(clean(&target, std::slice::from_ref(&plugin), false), 40);

        let wasm = plugin.join("wasm32-wasip2/release");
        assert!(wasm.join("deps/gedcom_import-0123456789abcdef.wasm").exists());
        assert!(!wasm.join("deps/gedcom_import-0123456789abcdef").exists());
        assert!(!wasm.join("incremental").exists());
        assert!(!plugin.join("release/incremental").exists());
        assert!(plugin.join("release/deps/libserde-0123456789abcdef.rlib").exists());
    }

    #[test]
    fn run_output_goes_but_plugins_and_fetched_fixtures_stay_unless_full() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        for name in ["gui-pass", "screenshots", "doc", "criterion", "flycheck0", "tmp"] {
            write(&target, &format!("{name}/nested/file"), 5);
        }
        write(&target, "plugins/gedcom_import.wasm", 7);
        write(&target, "external-fixtures/digitalarkivet/page.html", 7);

        assert_eq!(clean(&target, &[], false), 30);
        for name in ["gui-pass", "screenshots", "doc", "criterion", "flycheck0", "tmp"] {
            assert!(!target.join(name).exists(), "{name} survived");
        }
        assert!(target.join("plugins/gedcom_import.wasm").exists());
        assert!(target.join("external-fixtures/digitalarkivet/page.html").exists());

        assert_eq!(clean(&target, &[], true), 14);
        assert!(!target.join("plugins").exists());
        assert!(!target.join("external-fixtures").exists());
    }

    #[test]
    fn planning_alone_deletes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        host_profile(&target, "debug");
        write(&target, "doc/index.html", 10);

        let removals = plan(&target, &[], true).unwrap();
        let bytes: u64 = removals.iter().map(|removal| removal.bytes).sum();

        assert_eq!(bytes, 30);
        assert!(target.join("debug/deps/vitni_core-0123456789abcdef").exists());
        assert!(target.join("debug/incremental").exists());
        assert!(target.join("doc/index.html").exists());
    }

    #[test]
    fn missing_or_already_clean_targets_plan_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        assert!(
            plan(&target, &[dir.path().join("plugins/none/target")], true)
                .unwrap()
                .is_empty()
        );

        host_profile(&target, "debug");
        clean(&target, &[], false);
        assert!(plan(&target, &[], false).unwrap().is_empty());
    }

    #[test]
    fn empty_directories_are_not_reported() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        fs::create_dir_all(target.join("release/incremental")).unwrap();
        fs::create_dir_all(target.join("tmp")).unwrap();
        write(&target, "release/deps/libserde-0123456789abcdef.rlib", 10);

        assert!(plan(&target, &[], false).unwrap().is_empty());
    }

    #[test]
    fn a_symlinked_executable_target_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        write(dir.path(), "outside/keep", 10);
        write(&target, "debug/deps/libserde-0123456789abcdef.rlib", 10);
        std::os::unix::fs::symlink(dir.path().join("outside"), target.join("debug/deps/linked")).unwrap();

        assert_eq!(clean(&target, &[], false), 0);
        assert!(dir.path().join("outside/keep").exists());
    }

    #[test]
    fn flags_parse_in_any_order_and_unknown_ones_fail() {
        let args = |list: &[&str]| list.iter().map(|arg| (*arg).to_owned()).collect::<Vec<String>>();
        assert_eq!(parse_args(&args(&[])).unwrap(), Options::default());
        assert_eq!(
            parse_args(&args(&["--dry-run", "--full"])).unwrap(),
            Options {
                full: true,
                dry_run: true
            }
        );
        assert!(parse_args(&args(&["--force"])).is_err());
    }

    #[test]
    fn sizes_read_in_the_largest_whole_unit() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1023), "1023 B");
        assert_eq!(human_size(1024), "1.0 KiB");
        assert_eq!(human_size(1536 * 1024), "1.5 MiB");
        assert_eq!(human_size(196 * 1024 * 1024 * 1024), "196.0 GiB");
        assert_eq!(human_size(2048 * 1024 * 1024 * 1024 * 1024), "2048.0 TiB");
    }
}
