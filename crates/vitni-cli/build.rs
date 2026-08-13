// build.rs — generates `vitni-cli.ftl` for each locale by concatenating all per-aggregate
// `.ftl` files in the locale directory.  The merged file is the domain file that `fl!()`
// validates message keys against at compile time; the per-aggregate files are the authoritative
// source that developers edit.  New aggregates require only a new `.ftl` file — no edits to
// any shared catalogue.

use std::fs;
use std::path::Path;

fn main() {
    if let Err(e) = run() {
        eprintln!("build.rs error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let i18n_dir = Path::new("i18n");

    for entry in fs::read_dir(i18n_dir)? {
        let locale_dir = entry?.path();
        if !locale_dir.is_dir() {
            continue;
        }

        // Collect all .ftl files in the locale dir, sorted for deterministic output.
        // Exclude the generated vitni-cli.ftl itself to avoid reading our own output.
        let mut ftl_files: Vec<_> = fs::read_dir(&locale_dir)?
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "ftl"))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n != "vitni-cli.ftl")
            })
            .collect();
        ftl_files.sort();

        if ftl_files.is_empty() {
            continue;
        }

        // Concatenate with a blank separator line between files.
        let mut combined = String::new();
        for path in &ftl_files {
            let content = fs::read_to_string(path)?;
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(&content);
        }

        fs::write(locale_dir.join("vitni-cli.ftl"), &combined)?;

        // Re-run if any source file changes.
        for path in &ftl_files {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    println!("cargo:rerun-if-changed=i18n/");
    Ok(())
}
