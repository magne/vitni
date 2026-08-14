//! `icons` — rasterises the committed SVG art into the PNGs a desktop installs and the README shows,
//! and verifies the result decodes (#326).
//!
//! The SVG sources under `crates/vitni-ui-dioxus/assets/` are the design source of truth; the PNGs
//! beside them are generated and committed, because `.deb` and `AppImage` builds install files rather
//! than render vectors, and GitHub renders an `<img>` rather than an SVG pipeline. Rasterising happens
//! in-process through `resvg`, so a regenerated file depends on `Cargo.lock` alone — no system image
//! tool, no per-machine drift, and no system fonts (which is why every letterform in the brand art is
//! drawn as geometry: `resvg` is built with `default-features = false` and has no text shaping).
//!
//! Two tables, both `(source, …, output)`:
//!
//! - [`SIZES`] — the app icon, `icon/`. Four tiers, because the seal is disclosed by size: full
//!   impressed detail at 256/128, the cut flat on its rim alone at 64, a plain node at 48, and at 32
//!   and below a plain node over a mark grown to fill more of the plate, where the acceptance criterion
//!   is that the icon is indistinguishable from the plain V.
//! - [`BRAND`] — the lockups, `brand/`: the wordmark the README shows and the splash-shaped art (also
//!   the size GitHub wants for a social preview). Not square, so these carry their own dimensions.
//!
//! `--check` (part of `cargo xtask check`) re-reads every committed PNG instead of writing: present,
//! decodable, the expected dimensions, and carrying opaque pixels. That is the guard for the defect
//! that prompted this command — the shipped icon was a 144-byte stub whose IDAT stream did not decode
//! and whose pixels were all transparent, and nothing failed.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

/// Where the icon SVG tiers live and the generated icon PNGs are written.
const ICON_DIR: &str = "crates/vitni-ui-dioxus/assets/icon";
/// Where the brand lockups live and their PNGs are written.
const BRAND_DIR: &str = "crates/vitni-ui-dioxus/assets/brand";

/// The symbolic (monochrome, plate-less, seal-less) variant, shipped as SVG and never rasterised.
const SYMBOLIC_SRC: &str = "vitni-symbolic.svg";

/// Every installed icon size and the SVG tier it is rasterised from (see the module docs).
const SIZES: [(u16, &str); 7] = [
    (256, "vitni.svg"),
    (128, "vitni.svg"),
    (64, "vitni-notch.svg"),
    (48, "vitni-plain.svg"),
    (32, "vitni-small.svg"),
    (24, "vitni-small.svg"),
    (16, "vitni-small.svg"),
];

/// Each brand lockup: source, output width and height, and the PNG written beside it.
const BRAND: [(&str, u16, u16, &str); 2] = [
    ("vitni-wordmark.svg", 720, 200, "vitni-wordmark-720.png"),
    ("vitni-splash.svg", 1280, 640, "vitni-splash-1280x640.png"),
];

/// Runs the `icons` command: regenerates every PNG, or with `--check` verifies the committed ones.
pub fn run(args: &[String]) -> Result<()> {
    if args.iter().any(|arg| arg == "--check") {
        check()
    } else {
        generate()
    }
}

/// Verifies the committed art without writing anything — the `cargo xtask check` entry point.
pub fn check() -> Result<()> {
    let icons = icon_dir()?;
    for (size, _) in SIZES {
        verify(&icon_png_path(icons, size), size, size)?;
    }
    for (_, width, height, output) in BRAND {
        verify(&Path::new(BRAND_DIR).join(output), width, height)?;
    }
    println!(
        "icons: {} icon PNGs and {} brand PNGs decode at their declared size.",
        SIZES.len(),
        BRAND.len()
    );
    Ok(())
}

/// Rasterises every tier and lockup over its committed PNG, verifying each written file before moving
/// on.
fn generate() -> Result<()> {
    let icons = icon_dir()?;
    for (size, source) in SIZES {
        let png = icon_png_path(icons, size);
        write_png(&icons.join(source), &png, size, size)?;
    }
    let brand = Path::new(BRAND_DIR);
    for (source, width, height, output) in BRAND {
        write_png(&brand.join(source), &brand.join(output), width, height)?;
    }
    println!(
        "icons: {} icon PNGs and {} brand PNGs regenerated.",
        SIZES.len(),
        BRAND.len()
    );
    Ok(())
}

/// Rasterises `source` to `dest` at `width`×`height` and verifies what was written.
fn write_png(source: &Path, dest: &Path, width: u16, height: u16) -> Result<()> {
    let bytes = render(source, width, height)?;
    fs::write(dest, &bytes).with_context(|| format!("writing {}", dest.display()))?;
    verify(dest, width, height)?;
    let name = source.file_name().unwrap_or(source.as_os_str()).to_string_lossy();
    println!("icons: {} <- {name} ({} bytes)", dest.display(), bytes.len());
    Ok(())
}

/// The icon directory, checked for the symbolic variant that ships as SVG rather than as a raster.
fn icon_dir() -> Result<&'static Path> {
    let dir = Path::new(ICON_DIR);
    let symbolic = dir.join(SYMBOLIC_SRC);
    if !symbolic.is_file() {
        bail!(
            "{} is missing — the symbolic variant ships as SVG, not as a raster",
            symbolic.display()
        );
    }
    Ok(dir)
}

/// `<dir>/vitni-<size>.png` — the generated raster for one icon size.
fn icon_png_path(dir: &Path, size: u16) -> PathBuf {
    dir.join(format!("vitni-{size}.png"))
}

/// Rasterises `source` into a `width`×`height` PNG, scaling the source's own canvas to fit the width.
///
/// A source whose aspect ratio does not match the requested one is an error rather than a raster with
/// a transparent strip along one edge.
fn render(source: &Path, width: u16, height: u16) -> Result<Vec<u8>> {
    let svg = fs::read(source).with_context(|| format!("reading {}", source.display()))?;
    let tree = Tree::from_data(&svg, &Options::default()).with_context(|| format!("parsing {}", source.display()))?;

    let scale = f32::from(width) / tree.size().width();
    let scaled_height = tree.size().height() * scale;
    if (scaled_height - f32::from(height)).abs() > 1.0 {
        bail!(
            "{} is {}x{} in its own units, which is {width}x{scaled_height:.0} at the requested width \
             — not the declared {width}x{height}",
            source.display(),
            tree.size().width(),
            tree.size().height(),
        );
    }

    let mut pixmap = Pixmap::new(u32::from(width), u32::from(height))
        .with_context(|| format!("allocating a {width}x{height} pixmap for {}", source.display()))?;
    resvg::render(&tree, Transform::from_scale(scale, scale), &mut pixmap.as_mut());

    pixmap
        .encode_png()
        .with_context(|| format!("encoding the {width}x{height} raster of {}", source.display()))
}

/// Verifies one committed PNG: it exists, decodes, has the declared dimensions, and has opaque pixels.
fn verify(png: &Path, width: u16, height: u16) -> Result<()> {
    let bytes = fs::read(png).with_context(|| {
        format!(
            "reading {} (run `cargo xtask icons` to generate the rasters)",
            png.display()
        )
    })?;
    let pixmap = Pixmap::decode_png(&bytes).with_context(|| format!("{} does not decode as a PNG", png.display()))?;
    if pixmap.width() != u32::from(width) || pixmap.height() != u32::from(height) {
        bail!(
            "{} is {}x{}, expected {width}x{height}",
            png.display(),
            pixmap.width(),
            pixmap.height()
        );
    }
    if pixmap.pixels().iter().all(|pixel| pixel.alpha() == 0) {
        bail!(
            "{} is fully transparent — it would show as nothing at all",
            png.display()
        );
    }
    Ok(())
}
