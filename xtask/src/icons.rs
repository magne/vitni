//! `icons` — rasterises the committed SVG icon sources into the PNG sizes a desktop installs, and
//! verifies the result decodes (#326).
//!
//! The SVG sources under `crates/vitni-ui-dioxus/assets/icon/` are the design source of truth; the
//! PNGs beside them are generated and committed, because `.deb` and `AppImage` builds install files
//! rather than render vectors. Rasterising happens in-process through `resvg`, so a regenerated icon
//! depends on `Cargo.lock` alone — no system image tool, no per-machine drift.
//!
//! Four tiers exist because the seal is disclosed by size: full impressed detail at 256/128, the cut
//! flat on its rim alone at 64, a plain node at 48, and at 32 and below a plain node over a mark grown
//! to fill more of the plate — where the acceptance criterion is that the icon is indistinguishable
//! from the plain V. [`SIZES`] is that mapping.
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

/// Where the SVG sources live and the generated PNGs are written.
const ICON_DIR: &str = "crates/vitni-ui-dioxus/assets/icon";

/// The symbolic (monochrome, plate-less, seal-less) variant, shipped as SVG and never rasterised.
const SYMBOLIC_SRC: &str = "vitni-symbolic.svg";

/// Every installed PNG size and the SVG tier it is rasterised from (see the module docs).
const SIZES: [(u16, &str); 7] = [
    (256, "vitni.svg"),
    (128, "vitni.svg"),
    (64, "vitni-notch.svg"),
    (48, "vitni-plain.svg"),
    (32, "vitni-small.svg"),
    (24, "vitni-small.svg"),
    (16, "vitni-small.svg"),
];

/// Runs the `icons` command: regenerates every PNG, or with `--check` verifies the committed ones.
pub fn run(args: &[String]) -> Result<()> {
    if args.iter().any(|arg| arg == "--check") {
        check()
    } else {
        generate()
    }
}

/// Verifies the committed icon set without writing anything — the `cargo xtask check` entry point.
pub fn check() -> Result<()> {
    let dir = icon_dir()?;
    for (size, _) in SIZES {
        verify(&png_path(dir, size), size)?;
    }
    println!("icons: {} PNGs decode at their declared size.", SIZES.len());
    Ok(())
}

/// Rasterises every tier over its committed PNG, verifying each written file before moving on.
fn generate() -> Result<()> {
    let dir = icon_dir()?;
    for (size, source) in SIZES {
        let png = png_path(dir, size);
        let bytes = render(&dir.join(source), size)?;
        fs::write(&png, &bytes).with_context(|| format!("writing {}", png.display()))?;
        verify(&png, size)?;
        println!("icons: {} <- {source} ({} bytes)", png.display(), bytes.len());
    }
    println!("icons: {} PNGs regenerated in {}.", SIZES.len(), dir.display());
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

/// `<dir>/vitni-<size>.png` — the generated raster for one size.
fn png_path(dir: &Path, size: u16) -> PathBuf {
    dir.join(format!("vitni-{size}.png"))
}

/// Rasterises `source` into a `size`×`size` PNG, scaling the tier's own canvas to fit.
fn render(source: &Path, size: u16) -> Result<Vec<u8>> {
    let svg = fs::read(source).with_context(|| format!("reading {}", source.display()))?;
    let tree = Tree::from_data(&svg, &Options::default()).with_context(|| format!("parsing {}", source.display()))?;

    let side = u32::from(size);
    let mut pixmap = Pixmap::new(side, side)
        .with_context(|| format!("allocating a {size}x{size} pixmap for {}", source.display()))?;
    let scale = f32::from(size) / tree.size().width();
    resvg::render(&tree, Transform::from_scale(scale, scale), &mut pixmap.as_mut());

    pixmap
        .encode_png()
        .with_context(|| format!("encoding the {size}px raster of {}", source.display()))
}

/// Verifies one committed PNG: it exists, decodes, has the declared dimensions, and has opaque pixels.
fn verify(png: &Path, size: u16) -> Result<()> {
    let bytes = fs::read(png).with_context(|| {
        format!(
            "reading {} (run `cargo xtask icons` to generate the icon rasters)",
            png.display()
        )
    })?;
    let pixmap = Pixmap::decode_png(&bytes).with_context(|| format!("{} does not decode as a PNG", png.display()))?;
    let side = u32::from(size);
    if pixmap.width() != side || pixmap.height() != side {
        bail!(
            "{} is {}x{}, expected {size}x{size}",
            png.display(),
            pixmap.width(),
            pixmap.height()
        );
    }
    if pixmap.pixels().iter().all(|pixel| pixel.alpha() == 0) {
        bail!(
            "{} is fully transparent — it would install as no icon at all",
            png.display()
        );
    }
    Ok(())
}
