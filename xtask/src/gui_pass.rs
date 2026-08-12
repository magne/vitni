//! `gui-pass` — drives the real GUI on a headless X display and checks scripted scenarios.
//!
//! SSR tests stop at the markup; anything that only exists in a running `WebKitGTK` webview
//! (`document::eval`, CSS, the `MapLibre` canvas) needs the actual window. On a Wayland desktop the
//! window is not scriptable — synthetic input reaches it only while the compositor has focused it —
//! so this command runs the GUI on its own **Xvfb** display instead, where X focus *is* focus and
//! `xdotool` is deterministic. `MapLibre` renders there over software GL.
//!
//! Scenarios are **data, not code**: each is a TOML file under
//! `crates/genealogy-ui-dioxus/tests/gui-pass/`, so adding one needs no recompile. A top-level
//! `window = [w, h]` sets the size the window is resized to before its steps run, defaulting to
//! [`WINDOW`] when omitted — the narrow-window case (below `--bp-lg`) needs its own coordinates, never
//! a single-pane layout's carried over (see `CLAUDE.md`'s "Writing one"). A file lists `[[step]]`s (a
//! click, a chord, a drag, a wheel, a screenshot, `wait` to sleep and let a timed effect fire,
//! `wm-close` to ask the window to close the way a window manager does, or `await-exit` to wait for the
//! GUI process itself to quit) and `[[assert]]`s over the shots it took —
//! `differ` for "the UI reacted",
//! `match` for "the UI returned to this state", `painted` for "this area is not a flat fill". The
//! first two compare with an RMSE tolerance, so a caret blink is not a difference. Any assertion may
//! add `region = [x, y, w, h]` to work on a single window sub-rectangle instead of the whole shot —
//! needed when a change is provably confined to one area but the rest of the window can legitimately
//! repaint either way (e.g. the tabstrip repaints on every Save, so a whole-window `differ` cannot
//! isolate a list-column change), and needed by `painted`, whose whole-window form the surrounding
//! chrome would always answer for. `manifest` is different again: it checks
//! `target/gui-pass/workspace/workspace.toml` on disk for a substring, proving a write reached disk
//! rather than only an in-memory signal — unavailable under `--real-config`, where that path is the
//! caller's own workspace.
//!
//! Each scenario's shot directory also holds a `gui.log` — the GUI child's own stdout and stderr, run
//! at `RUST_LOG=info`, so a `tracing` line or a webview diagnostic is readable beside the shots it
//! belongs to instead of being discarded.
//!
//! The run is isolated by default: a throwaway `XDG_CONFIG_HOME`/`XDG_DATA_HOME` under
//! `target/gui-pass/home` and a seeded fixture workspace, so a scripted click run can never append
//! assertions to real genealogy data. `--real-config` (optionally with `--workspace <name>`) points
//! the same scripts at the caller's own config and workspaces when reproducing something in real data.
//!
//! Running no window manager does not put the **window-manager close** out of reach: [`Step::WmClose`]
//! sends the toplevel the `WM_DELETE_WINDOW` `ClientMessage` the ICCCM defines for it, and GDK dispatches
//! it from its own event handling with no WM in sight — so the titlebar `✕` / session-logout path
//! (issue #281) is scriptable, and `wm-close-confirm.toml` drives it.
//!
//! What it cannot settle: pan/zoom smoothness, click latency and motion. Software GL is not the
//! user's GPU. Those stay the `manual-verify` residual (see `docs/issue-tracking.md`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use genealogy_core::media_path::{MEDIA_DIR, workspace_media_path};
use serde::Deserialize;

use crate::util::{copy_dir, run_cargo};

/// Where the isolated home, the fixture workspace and the shots are written.
const OUT_DIR: &str = "target/gui-pass";
/// Where the scenario files live, beside the SSR tests of the crate they exercise.
const SCRIPT_DIR: &str = "crates/genealogy-ui-dioxus/tests/gui-pass";
/// The Xvfb display the GUI is driven on. Overridable with `--display`.
const DEFAULT_DISPLAY: &str = ":99";
/// The virtual screen Xvfb serves. Larger than the window so a resize never clips.
const SCREEN: &str = "2560x1600x24";
/// The window size a scenario's coordinates are written against, when it declares no `window` of its
/// own. There is no window manager on the display, so the window keeps whatever size `xdotool
/// windowsize` gives it.
const WINDOW: (u32, u32) = (1800, 1200);
/// The largest x a [`focus_click`] uses, matching today's value at the default [`WINDOW`] — see
/// [`focus_click`].
const MAX_FOCUS_X: i32 = 900;
/// The fixture workspace name.
const FIXTURE_WORKSPACE: &str = "gui-pass";
/// The pristine copy of the seeded workspace, restored before every scenario.
const SEED_DIR: &str = "workspace-seed";
/// The pristine copy of the seeded global config, restored before every scenario — the
/// `map-provider-switch` scenario writes to it (ADR 0033), and scenario order must stay irrelevant
/// exactly like the workspace's own [`SEED_DIR`].
const CONFIG_SEED_FILE: &str = "config-seed.toml";
/// A `MapLibre` style that looks nothing like OSM raster tiles (`MapLibre`'s own free demo style), seeded
/// as a second, inactive `[map.providers.*]` choice so `map-provider-switch.toml` can prove a switch
/// repaints the canvas — a same-looking basemap would let a stuck repaint pass by accident.
const DEMO_MAP_PROVIDER: &str = "
[map.providers.demo]
kind = \"maplibre-style\"
style-url = \"https://demotiles.maplibre.org/style.json\"
attribution = \"© MapLibre demo tiles\"
";
/// The seeded media image's path below the fixture workspace's media root. `media-preview.toml` opens
/// the Media record that points at it (#301).
const SEED_MEDIA_REL: &str = "portraits/portrait.png";
/// The seeded media image's side in pixels — big enough that the preview frame scales it down rather
/// than up, so the `painted` region measures real image pixels.
const SEED_MEDIA_SIZE: u32 = 480;
/// How long to wait for the window to map before giving up.
const WINDOW_TIMEOUT: Duration = Duration::from_secs(45);
/// How long [`Step::AwaitExit`] waits for the GUI process to exit before failing.
const AWAIT_EXIT_TIMEOUT: Duration = Duration::from_secs(15);
/// Standard deviation below which a screenshot is treated as blank (an unpainted or black window).
const MIN_STANDARD_DEVIATION: f64 = 0.005;
/// Normalized RMSE below which two shots count as the same screen. Above the caret blink and text
/// antialiasing that differ between two grabs of an unchanged screen, far below any real repaint.
const SAME_SCREEN_RMSE: f64 = 0.01;

/// One scenario: what it proves, the steps to drive, and the assertions over the shots taken.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Script {
    /// What this scenario demonstrates, printed as the run header.
    description: String,
    /// The window size this scenario's coordinates are written against; `None` defaults to
    /// [`WINDOW`]. `deny_unknown_fields` on this struct is what makes a typo'd key (e.g. `windwo`)
    /// fail to parse instead of silently running the scenario at the default window.
    window: Option<[u32; 2]>,
    #[serde(default, rename = "step")]
    steps: Vec<Step>,
    #[serde(default, rename = "assert")]
    asserts: Vec<Assertion>,
}

/// `script`'s window size: its own [`Script::window`], or [`WINDOW`] when it declares none.
fn window_size(script: &Script) -> (u32, u32) {
    match script.window {
        Some([width, height]) => (width, height),
        None => WINDOW,
    }
}

/// Empty top-bar space for a `window`-sized run, clicked once at startup to hand the webview keyboard
/// focus (see [`focus`]). `.search` is `margin-left: auto` (`components.css`), so the left half of the
/// top bar is empty at every window width; clamping to half the width keeps the click there even at a
/// narrow `window` while leaving every current (1800-wide) scenario's [`MAX_FOCUS_X`] unchanged.
fn focus_click(window: (u32, u32)) -> (i32, i32) {
    let half = window.0 / 2;
    let half = i32::try_from(half).unwrap_or(MAX_FOCUS_X);
    (half.min(MAX_FOCUS_X), 60)
}

/// One scripted action. Coordinates are window pixels at the scenario's `window` (defaulting to
/// [`WINDOW`] — see [`window_size`]), read off an earlier screenshot — the window sits at the display
/// origin, so they are display coordinates too.
#[derive(Deserialize)]
#[serde(tag = "do", rename_all = "kebab-case", deny_unknown_fields)]
enum Step {
    /// Grab the window into `NN-<name>.png`, and make it referenceable by `name` in an assertion.
    Shot { name: String },
    /// Move the pointer to `at` and click button 1.
    Click { at: [i32; 2], label: String },
    /// Send a chord in `xdotool key` syntax (`ctrl+k`, `Escape`, `question`).
    Key { chord: String, label: String },
    /// Press at `from`, move by `by`, release — a canvas drag (map pan).
    Drag {
        from: [i32; 2],
        by: [i32; 2],
        label: String,
    },
    /// Scroll the wheel at a point: `clicks` notches up (button 4) or down (button 5) when negative.
    Wheel { at: [i32; 2], clicks: i32, label: String },
    /// Wait for the GUI process to exit (e.g. after a quit chord), failing if it is still up after
    /// [`AWAIT_EXIT_TIMEOUT`]. Proves a quit actually happened, rather than assuming a chord worked.
    AwaitExit { label: String },
    /// Ask the window to close the way a window manager does — the titlebar `✕`, a session logout,
    /// `wmctrl -c` — by sending it a `WM_DELETE_WINDOW` `ClientMessage` (see [`wm_close`]). Not
    /// `xdotool windowclose`, which is `XDestroyWindow` and never reaches the app at all.
    WmClose { label: String },
    /// Sleep for `seconds`, then let the webview settle — proving a timed effect (e.g. a toast's
    /// auto-dismiss) in the real webview rather than assuming it fires.
    Wait { seconds: u64, label: String },
}

/// One check over the shots the script took.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Assertion {
    /// The two shots must show different screens — the UI reacted.
    Differ {
        shots: [String; 2],
        because: String,
        /// The RMSE the difference must exceed. Lower it for a change that repaints few pixels (a
        /// dropped map point); defaults to [`SAME_SCREEN_RMSE`].
        tolerance: Option<f64>,
        /// `[x, y, w, h]` window pixels to compare instead of the whole shot. Absent compares the
        /// whole window, today's behaviour.
        region: Option<[u32; 4]>,
    },
    /// The two shots must show the same screen — the UI returned there (e.g. an overlay dismissed).
    Match {
        shots: [String; 2],
        because: String,
        tolerance: Option<f64>,
        /// See [`Self::Differ`]'s `region`.
        region: Option<[u32; 4]>,
    },
    /// One shot must not be a flat colour over `region` — the whole-shot [`assert_painted`] every
    /// grab already runs cannot see a blank *area*, because the rail, toolbar and tabstrip around it
    /// keep the window's own deviation high. Scope it to the map canvas and a blanked canvas fails.
    Painted {
        shot: String,
        because: String,
        /// See [`Self::Differ`]'s `region`; absent measures the whole window, which only the chrome
        /// around a blank area would then answer for.
        region: Option<[u32; 4]>,
        /// The standard deviation the region must exceed; defaults to [`MIN_STANDARD_DEVIATION`].
        min_deviation: Option<f64>,
    },
    /// `target/gui-pass/workspace/workspace.toml` must contain `contains` as a substring — proves a
    /// write reached disk, not just an in-memory signal (e.g. a recent list surviving a quit).
    /// Substring matching, not a TOML-path DSL: this has exactly one caller. Unavailable under
    /// `--real-config`, where the workspace path is the caller's own and unsafe to assert over.
    Manifest { contains: String, because: String },
}

/// How the run is configured.
struct Options {
    display: String,
    /// The scenarios to run; empty means every file in [`SCRIPT_DIR`].
    scripts: Vec<String>,
    /// Use the caller's own config and workspaces instead of the isolated fixture.
    real_config: bool,
    /// The workspace to open; `None` seeds and opens [`FIXTURE_WORKSPACE`].
    workspace: Option<String>,
    /// Leave Xvfb and the GUI running so a human can attach (e.g. `x11vnc -display :99`).
    keep: bool,
    /// Delete the isolated home and fixture workspace before seeding.
    reset: bool,
}

/// Kills the child processes the run started, whatever the outcome.
struct Session {
    xvfb: Child,
    gui: Option<Child>,
    keep: bool,
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        if let Some(gui) = self.gui.as_mut() {
            let _ = gui.kill();
            let _ = gui.wait();
        }
        let _ = self.xvfb.kill();
        let _ = self.xvfb.wait();
    }
}

/// Runs every requested scenario, each in its own GUI instance so one cannot leave state for the next.
pub fn run(args: &[String]) -> Result<()> {
    let options = parse_args(args)?;
    preflight()?;
    run_cargo(&["build", "-p", "genealogy-ui-dioxus", "--features", "desktop"])?;
    run_cargo(&["build", "-p", "genealogy-cli"])?;

    let out = PathBuf::from(OUT_DIR);
    if options.reset {
        reset(&out)?;
    }
    let home = absolute(&out.join("home"))?;
    if !options.real_config {
        seed_fixture(&out, &home)?;
    }

    let scripts = resolve_scripts(&options.scripts)?;
    let mut failed = Vec::new();
    for path in &scripts {
        let name = script_name(path);
        match run_one(&options, &out, &home, path) {
            Ok(()) => println!("gui-pass: {name} passed"),
            Err(error) => {
                eprintln!("gui-pass: {name} FAILED: {error:#}");
                failed.push(name);
            }
        }
    }
    if !failed.is_empty() {
        bail!(
            "gui-pass: {} of {} scenarios failed ({})",
            failed.len(),
            scripts.len(),
            failed.join(", ")
        );
    }
    println!(
        "gui-pass: {} scenarios passed — shots under {}/shots; smoothness and latency still need a human.",
        scripts.len(),
        out.display()
    );
    Ok(())
}

/// Runs one scenario end to end, from a fresh copy of the seeded workspace and an empty shot
/// directory — a scenario writes events (dropping a map point asserts coordinates), so sharing either
/// would make one scenario's result depend on which ran before it.
fn run_one(options: &Options, out: &Path, home: &Path, path: &Path) -> Result<()> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let script: Script = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let name = script_name(path);
    println!("\n=== {name} — {}", script.description);
    let shots = out.join("shots").join(&name);
    if shots.exists() {
        fs::remove_dir_all(&shots).with_context(|| format!("clearing {}", shots.display()))?;
    }
    fs::create_dir_all(&shots).with_context(|| format!("creating {}", shots.display()))?;
    let shots = shots.as_path();
    if !options.real_config {
        restore_workspace(out)?;
        restore_config(out, home)?;
    }

    let size = window_size(&script);
    let mut session = start_session(options, home, shots)?;
    let window = wait_for_window(&options.display)?;
    xdotool(
        &options.display,
        &["windowsize", &window, &size.0.to_string(), &size.1.to_string()],
    )?;
    focus(&options.display, &window, size)?;
    settle();

    let taken = drive(&options.display, &window, &script.steps, shots, &mut session)?;
    if options.keep {
        session.keep = true;
        println!(
            "gui-pass: leaving {} and the GUI up — attach with `x11vnc -display {}`",
            options.display, options.display
        );
    }
    // `--real-config` points the isolated fixture's workspace path at the caller's own workspace,
    // which a `manifest` assertion must not read — see `Assertion::Manifest`.
    let workspace = (!options.real_config).then(|| out.join("workspace"));
    check(&script.asserts, &taken, shots, workspace.as_deref())
}

/// The scenario files to run: the named ones (a bare name, or a path), else every file in
/// [`SCRIPT_DIR`] in name order.
fn resolve_scripts(named: &[String]) -> Result<Vec<PathBuf>> {
    let dir = Path::new(SCRIPT_DIR);
    if named.is_empty() {
        let mut found = Vec::new();
        for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry
                .with_context(|| format!("reading an entry under {}", dir.display()))?
                .path();
            if path.extension().is_some_and(|extension| extension == "toml") {
                found.push(path);
            }
        }
        found.sort();
        if found.is_empty() {
            bail!("gui-pass: no scenarios in {}", dir.display());
        }
        return Ok(found);
    }
    let mut chosen = Vec::new();
    for name in named {
        let direct = PathBuf::from(name);
        let path = if direct.is_file() {
            direct
        } else {
            dir.join(format!("{}.toml", name.trim_end_matches(".toml")))
        };
        if !path.is_file() {
            bail!("gui-pass: no scenario {name} (looked in {})", dir.display());
        }
        chosen.push(path);
    }
    Ok(chosen)
}

/// A scenario's name: its file stem.
fn script_name(path: &Path) -> String {
    path.file_stem().unwrap_or_default().to_string_lossy().into_owned()
}

/// Parses the command's flags and scenario names.
fn parse_args(args: &[String]) -> Result<Options> {
    let mut options = Options {
        display: DEFAULT_DISPLAY.to_owned(),
        scripts: Vec::new(),
        real_config: false,
        workspace: None,
        keep: false,
        reset: false,
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--keep" => options.keep = true,
            "--reset" => options.reset = true,
            "--real-config" => options.real_config = true,
            "--display" => options.display = value(&mut rest, "--display")?,
            "--workspace" => {
                options.workspace = Some(value(&mut rest, "--workspace")?);
                options.real_config = true;
            }
            other if other.starts_with("--") => bail!("gui-pass: unknown flag {other}"),
            name => options.scripts.push(name.to_owned()),
        }
    }
    Ok(options)
}

/// The value following a flag.
fn value<'a>(rest: &mut impl Iterator<Item = &'a String>, flag: &str) -> Result<String> {
    match rest.next() {
        Some(value) => Ok(value.clone()),
        None => bail!("gui-pass: {flag} needs a value"),
    }
}

/// Fails with an actionable message if a driver tool is missing.
fn preflight() -> Result<()> {
    let tools = [
        ("Xvfb", "xvfb"),
        ("xdotool", "xdotool"),
        ("import", "imagemagick"),
        ("identify", "imagemagick"),
        ("compare", "imagemagick"),
        ("convert", "imagemagick"),
    ];
    let mut missing = Vec::new();
    for (tool, package) in tools {
        let found = Command::new("sh")
            .args(["-c", &format!("command -v {tool}")])
            .stdout(Stdio::null())
            .status()
            .with_context(|| format!("looking for {tool}"))?;
        if !found.success() {
            missing.push(format!("{tool} (apt install {package})"));
        }
    }
    if !missing.is_empty() {
        bail!("gui-pass needs: {}", missing.join(", "));
    }
    Ok(())
}

/// Deletes the isolated home, the fixture workspace, the seeded global config and the shots.
fn reset(out: &Path) -> Result<()> {
    for dir in ["home", "workspace", SEED_DIR, "shots"] {
        let path = out.join(dir);
        if path.exists() {
            fs::remove_dir_all(&path).with_context(|| format!("removing {}", path.display()))?;
        }
    }
    let config_seed = out.join(CONFIG_SEED_FILE);
    if config_seed.exists() {
        fs::remove_file(&config_seed).with_context(|| format!("removing {}", config_seed.display()))?;
    }
    Ok(())
}

/// Creates the fixture workspace on first run: one place with coordinates, so the map has something
/// to plot, one media object pointing at a seeded image (see [`seed_media`]), plus an inactive
/// `[map.providers.demo]` `MapLibre` style (ADR 0033) the `map-provider-switch` scenario switches to.
/// Idempotent — an existing workspace directory is reused.
fn seed_fixture(out: &Path, home: &Path) -> Result<()> {
    let workspace = out.join("workspace");
    if workspace.exists() {
        // A workspace seeded before the media image was added would fail `media-preview` with an empty
        // Media list rather than a blank preview, which reads like the defect it is meant to catch.
        let seeded = out.join(SEED_DIR).join(MEDIA_DIR).join(SEED_MEDIA_REL);
        if !seeded.is_file() {
            bail!(
                "gui-pass: the fixture predates the seeded media image ({} is missing) — re-run with `--reset`",
                seeded.display()
            );
        }
        return Ok(());
    }
    fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let workspace = absolute(&workspace)?.to_string_lossy().into_owned();
    cli(home, &["init", FIXTURE_WORKSPACE, &workspace])?;
    let config = home.join(".config/genealogy/config.toml");
    if !config.exists() {
        bail!(
            "gui-pass: init wrote no config at {} — the isolation failed and a real config may have been \
             registered instead",
            config.display()
        );
    }
    let created = cli(
        home,
        &["place", "create", "--type", "municipality", "--name", "Kristiansand"],
    )?;
    let place = created
        .split_whitespace()
        .next_back()
        .with_context(|| format!("no place id in {created:?}"))?
        .to_owned();
    cli(
        home,
        &[
            "place",
            "set-coordinates",
            &place,
            "--lat",
            "58.1467",
            "--long",
            "7.9956",
        ],
    )?;
    seed_media(home, Path::new(&workspace))?;
    let mut text = fs::read_to_string(&config).with_context(|| format!("reading {}", config.display()))?;
    text.push_str(DEMO_MAP_PROVIDER);
    fs::write(&config, text).with_context(|| format!("writing {}", config.display()))?;
    fs::copy(&config, out.join(CONFIG_SEED_FILE)).with_context(|| format!("seeding {CONFIG_SEED_FILE}"))?;
    copy_dir(&out.join("workspace"), &out.join(SEED_DIR))?;
    println!("gui-pass: seeded workspace {FIXTURE_WORKSPACE} at {workspace} with place {place}");
    Ok(())
}

/// Writes an image into the fixture workspace's media library and records one Media object pointing at
/// it, so `media-preview.toml` has something whose preview can be blank (#301).
///
/// The image is *generated* with `ImageMagick` (already a hard requirement, see [`preflight`]) rather
/// than committed: a deterministic gradient with a filled circle, textured enough that a `painted`
/// assertion over the preview frame measures the image and not its background. The repo's own
/// `assets/genealogy.png` will not do — it is a 64×64 fully transparent placeholder, so it would fail
/// `painted` even when the preview loads perfectly.
///
/// The record deliberately carries **no MIME**: `genealogy media` has no `set-mime`, so this is the
/// state every record the CLI creates is in, and #301's two live causes (no inferred MIME, and the
/// stored `media/` prefix added twice) both fire on it.
fn seed_media(home: &Path, workspace: &Path) -> Result<()> {
    let target = workspace.join(MEDIA_DIR).join(SEED_MEDIA_REL);
    let parent = target
        .parent()
        .with_context(|| format!("{} has no parent directory", target.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    let side = SEED_MEDIA_SIZE;
    let centre = side / 2;
    let status = Command::new("convert")
        .args(["-size", &format!("{side}x{side}"), "gradient:#1f6feb-#f0b72f"])
        .args([
            "-fill",
            "#d2352c",
            "-draw",
            &format!("circle {centre},{centre} {centre},20"),
        ])
        .arg(&target)
        .status()
        .with_context(|| format!("generating {}", target.display()))?;
    if !status.success() {
        bail!("convert failed with {status} generating {}", target.display());
    }
    let stored = workspace_media_path(SEED_MEDIA_REL);
    cli(home, &["media", "create", "--path", &stored])?;
    println!("gui-pass: seeded media {stored}");
    Ok(())
}

/// Replaces the fixture workspace with a fresh copy of the seed, so every scenario starts from the
/// same data. Nothing is running against it yet — this is called before the GUI launches.
fn restore_workspace(out: &Path) -> Result<()> {
    let seed = out.join(SEED_DIR);
    if !seed.is_dir() {
        bail!(
            "gui-pass: no seed at {} — re-run with --reset to reseed the fixture",
            seed.display()
        );
    }
    let workspace = out.join("workspace");
    if workspace.exists() {
        fs::remove_dir_all(&workspace).with_context(|| format!("removing {}", workspace.display()))?;
    }
    copy_dir(&seed, &workspace)
}

/// Replaces the isolated global config with a fresh copy of the seed (ADR 0033's `map-provider-switch`
/// scenario writes to it, next to [`restore_workspace`]'s own reasoning) — nothing is running against
/// it yet, called before the GUI launches.
fn restore_config(out: &Path, home: &Path) -> Result<()> {
    let seed = out.join(CONFIG_SEED_FILE);
    if !seed.is_file() {
        bail!(
            "gui-pass: no seeded config at {} — re-run with --reset to reseed the fixture",
            seed.display()
        );
    }
    let config = home.join(".config/genealogy/config.toml");
    fs::copy(&seed, &config).with_context(|| format!("restoring {}", config.display()))?;
    Ok(())
}

/// Runs the CLI against the isolated home, returning its stdout.
fn cli(home: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("target/debug/genealogy")
        .args(args)
        .envs(isolated_home(home))
        .env("GENEALOGY_WORKSPACE", FIXTURE_WORKSPACE)
        .output()
        .with_context(|| format!("running genealogy {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "genealogy {} failed with {}: {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The `XDG_*` overrides that keep a run off the caller's real config and data (ADR 0005 paths come
/// from `directories`, which reads these).
///
/// The values **must** be absolute: per the XDG basedir spec a relative `XDG_CONFIG_HOME` is ignored,
/// and `directories` then silently falls back to the caller's real `$HOME` — which is how an early
/// version of this command registered its fixture workspace in a real config file.
fn isolated_home(home: &Path) -> Vec<(String, String)> {
    vec![
        (
            "XDG_CONFIG_HOME".to_owned(),
            home.join(".config").to_string_lossy().into_owned(),
        ),
        (
            "XDG_DATA_HOME".to_owned(),
            home.join(".local/share").to_string_lossy().into_owned(),
        ),
    ]
}

/// `path` made absolute against the working directory, without requiring it to exist.
fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    let cwd = std::env::current_dir().context("reading the working directory")?;
    Ok(cwd.join(path))
}

/// Starts Xvfb, then the GUI on it.
///
/// The GUI's own stdout/stderr go to `gui.log` in the scenario's shot directory rather than to
/// `/dev/null`: `tracing_subscriber::fmt::init()` and any webview or GTK diagnostic write there, and a
/// discarded stream makes a failing scenario undiagnosable. `RUST_LOG=info` because the default filter
/// is `ERROR` only, which hides every `info!` the app emits.
fn start_session(options: &Options, home: &Path, shots: &Path) -> Result<Session> {
    let xvfb = Command::new("Xvfb")
        .args([&options.display, "-screen", "0", SCREEN, "-nolisten", "tcp"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("starting Xvfb")?;
    let mut session = Session {
        xvfb,
        gui: None,
        keep: false,
    };
    sleep(Duration::from_secs(2));

    let log_path = shots.join("gui.log");
    let log = fs::File::create(&log_path).with_context(|| format!("creating {}", log_path.display()))?;
    let errors = log
        .try_clone()
        .with_context(|| format!("sharing {}", log_path.display()))?;
    let mut gui = Command::new("target/debug/genealogy-gui");
    gui.env("DISPLAY", &options.display)
        // GTK prefers Wayland when the session advertises it, which would put the window on the
        // caller's desktop instead of the headless display.
        .env("GDK_BACKEND", "x11")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("XDG_SESSION_TYPE")
        .env("RUST_LOG", "info")
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(errors));
    if !options.real_config {
        gui.envs(isolated_home(home));
    }
    if let Some(name) = options.workspace.as_deref() {
        gui.env("GENEALOGY_WORKSPACE", name);
    } else if !options.real_config {
        gui.env("GENEALOGY_WORKSPACE", FIXTURE_WORKSPACE);
    }
    session.gui = Some(gui.spawn().context("starting genealogy-gui")?);
    Ok(session)
}

/// Polls for the GUI window until it maps.
fn wait_for_window(display: &str) -> Result<String> {
    let poll = Duration::from_millis(500);
    let mut waited = Duration::ZERO;
    while waited < WINDOW_TIMEOUT {
        let found = Command::new("xdotool")
            .args(["search", "--name", "^Genealogy$"])
            .env("DISPLAY", display)
            .output()
            .context("running xdotool search")?;
        let ids = String::from_utf8_lossy(&found.stdout);
        if let Some(id) = ids.split_whitespace().next_back() {
            return Ok(id.to_owned());
        }
        sleep(poll);
        waited += poll;
    }
    bail!("gui-pass: no Genealogy window appeared on {display} within {WINDOW_TIMEOUT:?}")
}

/// Gives the window keyboard focus.
///
/// Both halves are needed. There is no window manager on the display, so X input focus starts at
/// `PointerRoot` and `windowfocus` is what points it at the window; but the webview only starts
/// delivering key events to its document after the page has been clicked, so a chord sent before any
/// click is silently dropped. [`focus_click`] is empty top-bar space at `size`, chosen so the click
/// activates nothing.
fn focus(display: &str, window: &str, size: (u32, u32)) -> Result<()> {
    xdotool(display, &["windowfocus", window])?;
    let (x, y) = focus_click(size);
    xdotool(display, &["mousemove", &x.to_string(), &y.to_string()])?;
    xdotool(display, &["click", "1"])
}

/// Runs every step, returning the shot names in the order they were taken.
fn drive(display: &str, window: &str, steps: &[Step], shots: &Path, session: &mut Session) -> Result<Vec<String>> {
    let mut taken = Vec::new();
    for step in steps {
        match step {
            Step::Shot { name } => {
                let path = shot_file(shots, taken.len() + 1, name);
                grab(display, window, &path)?;
                assert_painted(&path)?;
                println!("  shot {}", path.display());
                taken.push(name.clone());
            }
            Step::Click { at, label } => {
                println!("  {label}");
                xdotool(display, &["mousemove", &at[0].to_string(), &at[1].to_string()])?;
                xdotool(display, &["click", "1"])?;
                settle();
            }
            Step::Key { chord, label } => {
                println!("  {label}");
                xdotool(display, &["key", "--clearmodifiers", chord])?;
                settle();
            }
            Step::Drag { from, by, label } => {
                println!("  {label}");
                drag(display, *from, *by)?;
                settle();
            }
            Step::Wheel { at, clicks, label } => {
                println!("  {label}");
                wheel(display, *at, *clicks)?;
                settle();
            }
            Step::AwaitExit { label } => {
                println!("  {label}");
                await_exit(session)?;
            }
            Step::WmClose { label } => {
                println!("  {label}");
                wm_close(display, window)?;
                settle();
            }
            Step::Wait { seconds, label } => {
                println!("  {label}");
                sleep(Duration::from_secs(*seconds));
                settle();
            }
        }
    }
    Ok(taken)
}

/// Waits for the GUI child to exit, failing if it is still up after [`AWAIT_EXIT_TIMEOUT`].
///
/// Takes the child out of `session.gui` up front: a successful `try_wait` reaps the process, and once
/// reaped the OS is free to recycle its pid, so `Session::drop`'s `kill`/`wait` must never run against
/// it again. On timeout the child is put back so `drop` still cleans up the (still-running) process.
fn await_exit(session: &mut Session) -> Result<()> {
    let Some(mut gui) = session.gui.take() else {
        bail!("gui-pass: await-exit with no GUI process left to wait for");
    };
    let poll = Duration::from_millis(200);
    let mut waited = Duration::ZERO;
    loop {
        if gui.try_wait().context("polling the GUI process")?.is_some() {
            return Ok(());
        }
        if waited >= AWAIT_EXIT_TIMEOUT {
            session.gui = Some(gui);
            bail!("gui-pass: the GUI process is still running after {AWAIT_EXIT_TIMEOUT:?}");
        }
        sleep(poll);
        waited += poll;
    }
}

/// Asks the window to close the way a window manager does, by sending it the `WM_DELETE_WINDOW`
/// `ClientMessage` the ICCCM defines for it — the titlebar `✕`, a session logout and `wmctrl -c` all
/// arrive this way, and nothing the app does to itself ever produces one.
///
/// `xdotool windowclose` is not this: it is `XDestroyWindow`, which tears the window down in the server
/// without the client ever hearing about it. Sending the message straight to the toplevel works even
/// though this display runs no window manager — GDK dispatches it from its own event handling, so the
/// close reaches tao's `WindowEvent::CloseRequested` exactly as it would on a real desktop.
fn wm_close(display: &str, window: &str) -> Result<()> {
    use x11rb::protocol::xproto::{ClientMessageEvent, ConnectionExt as _, EventMask};
    use x11rb::wrapper::ConnectionExt as _;

    let id: u32 = window
        .parse()
        .with_context(|| format!("parsing the window id {window:?}"))?;
    let (connection, _) = x11rb::connect(Some(display)).with_context(|| format!("connecting to {display}"))?;
    let protocols = connection
        .intern_atom(false, b"WM_PROTOCOLS")
        .context("interning WM_PROTOCOLS")?
        .reply()
        .context("interning WM_PROTOCOLS")?
        .atom;
    let delete = connection
        .intern_atom(false, b"WM_DELETE_WINDOW")
        .context("interning WM_DELETE_WINDOW")?
        .reply()
        .context("interning WM_DELETE_WINDOW")?
        .atom;
    let event = ClientMessageEvent::new(32, id, protocols, [delete, x11rb::CURRENT_TIME, 0, 0, 0]);
    // `NO_EVENT` addresses the toplevel itself rather than whatever is selecting for events on it,
    // which is what a window manager sends and what GDK is listening for.
    connection
        .send_event(false, id, EventMask::NO_EVENT, event)
        .with_context(|| format!("sending WM_DELETE_WINDOW to {window}"))?;
    // A round trip, not a `flush`: the connection is dropped as this returns, and a flushed-but-not-yet
    // processed request can still be lost with the socket. `sync` waits for the reply to a
    // `GetInputFocus` queued behind the send, which the server can only answer once it has processed the
    // send itself — so the message is on its way to the GUI before this connection goes away.
    connection.sync().context("waiting for the X server to process it")?;
    Ok(())
}

/// Presses, moves and releases button 1 — a canvas drag.
fn drag(display: &str, from: [i32; 2], by: [i32; 2]) -> Result<()> {
    xdotool(display, &["mousemove", &from[0].to_string(), &from[1].to_string()])?;
    xdotool(display, &["mousedown", "1"])?;
    xdotool(
        display,
        &["mousemove_relative", "--", &by[0].to_string(), &by[1].to_string()],
    )?;
    sleep(Duration::from_millis(300));
    xdotool(display, &["mouseup", "1"])
}

/// Scrolls the wheel: button 4 up, button 5 down.
fn wheel(display: &str, at: [i32; 2], clicks: i32) -> Result<()> {
    xdotool(display, &["mousemove", &at[0].to_string(), &at[1].to_string()])?;
    let button = if clicks < 0 { "5" } else { "4" };
    for _ in 0..clicks.abs() {
        xdotool(display, &["click", button])?;
    }
    Ok(())
}

/// Lets the webview repaint and any network tile fetch land.
fn settle() {
    sleep(Duration::from_secs(4));
}

/// Runs `xdotool` against the headless display.
fn xdotool(display: &str, args: &[&str]) -> Result<()> {
    let status = Command::new("xdotool")
        .args(args)
        .env("DISPLAY", display)
        .status()
        .with_context(|| format!("running xdotool {}", args.join(" ")))?;
    if !status.success() {
        bail!("xdotool {} failed with {status}", args.join(" "));
    }
    Ok(())
}

/// Grabs the window into `path`.
fn grab(display: &str, window: &str, path: &Path) -> Result<()> {
    let status = Command::new("import")
        .args(["-window", window])
        .arg(path)
        .env("DISPLAY", display)
        .status()
        .with_context(|| format!("grabbing {} into {}", window, path.display()))?;
    if !status.success() {
        bail!("import -window {window} failed with {status}");
    }
    Ok(())
}

/// Fails if a screenshot is a flat colour — an unpainted webview, which is otherwise easy to mistake
/// for a passing run.
fn assert_painted(path: &Path) -> Result<()> {
    let deviation = standard_deviation(path, None)?;
    if painted_failed(deviation, MIN_STANDARD_DEVIATION) {
        bail!(
            "{} is blank (standard deviation {deviation}) — the webview painted nothing",
            path.display()
        );
    }
    Ok(())
}

/// Whether a measured deviation counts as blank. Inclusive at the threshold, so a perfectly uniform
/// fill measured as exactly the threshold still fails.
fn painted_failed(deviation: f64, threshold: f64) -> bool {
    deviation <= threshold
}

/// A screenshot's pixel standard deviation, normalized to 0..1, restricted to `region`
/// (`[x, y, w, h]` window pixels) when given.
fn standard_deviation(path: &Path, region: Option<[u32; 4]>) -> Result<f64> {
    let measured = Command::new("identify")
        .args(["-format", "%[fx:standard_deviation]"])
        .arg(read_region(path, region))
        .output()
        .with_context(|| format!("measuring {}", path.display()))?;
    if !measured.status.success() {
        bail!("identify failed on {}", path.display());
    }
    parse_metric(&String::from_utf8_lossy(&measured.stdout), path)
}

/// `path` with an `ImageMagick` read modifier appended, so only `region` is read in — the cropping
/// [`difference`] gets from `compare -extract`, which `identify` does not accept.
fn read_region(path: &Path, region: Option<[u32; 4]>) -> String {
    match region {
        Some([x, y, w, h]) => format!("{}[{w}x{h}+{x}+{y}]", path.display()),
        None => path.display().to_string(),
    }
}

/// The normalized RMSE between two shots (0 for identical), restricted to `region` (`[x, y, w, h]`
/// window pixels) when given.
fn difference(left: &Path, right: &Path, region: Option<[u32; 4]>) -> Result<f64> {
    // `-extract` is a read-time setting, so placed once before both file arguments it crops each of
    // them identically as `compare` reads it in — no temp files needed.
    let extract = region.map(|[x, y, w, h]| format!("{w}x{h}+{x}+{y}"));
    // `compare` exits 1 when the images differ, which is the normal case here, so the status is not
    // an error signal — only an unparsable metric is.
    let compared = Command::new("compare")
        .args(["-metric", "RMSE"])
        .args(extract.iter().flat_map(|extract| ["-extract", extract]))
        .args([left, right])
        .arg("null:")
        .output()
        .with_context(|| format!("comparing {} with {}", left.display(), right.display()))?;
    let text = String::from_utf8_lossy(&compared.stderr);
    let normalized = text
        .split('(')
        .next_back()
        .and_then(|tail| tail.split(')').next())
        .unwrap_or_default();
    parse_metric(normalized, left)
}

/// Parses an `ImageMagick` metric, quoting what came back when it is not a number.
fn parse_metric(text: &str, path: &Path) -> Result<f64> {
    text.trim()
        .parse()
        .with_context(|| format!("parsing the metric for {}: {text:?}", path.display()))
}

/// Checks every assertion, reporting all failures rather than the first. `workspace` is the fixture
/// workspace directory, `None` under `--real-config` (see [`Assertion::Manifest`]).
fn check(asserts: &[Assertion], taken: &[String], shots: &Path, workspace: Option<&Path>) -> Result<()> {
    let mut failures = Vec::new();
    for assertion in asserts {
        if let Some(failure) = check_one(assertion, taken, shots, workspace)? {
            failures.push(failure);
        }
    }
    if !failures.is_empty() {
        bail!("{}", failures.join("; "));
    }
    Ok(())
}

/// One assertion's verdict: `None` when it held, else the message describing how it did not.
fn check_one(
    assertion: &Assertion,
    taken: &[String],
    shots: &Path,
    workspace: Option<&Path>,
) -> Result<Option<String>> {
    match assertion {
        Assertion::Differ {
            shots: named,
            because,
            tolerance,
            region,
        } => {
            let (left, right, difference) = compare(named, taken, shots, *region)?;
            let tolerance = tolerance.unwrap_or(SAME_SCREEN_RMSE);
            Ok((difference <= tolerance).then(|| pair_failure(&left, &right, difference, *region, because)))
        }
        Assertion::Match {
            shots: named,
            because,
            tolerance,
            region,
        } => {
            let (left, right, difference) = compare(named, taken, shots, *region)?;
            let tolerance = tolerance.unwrap_or(SAME_SCREEN_RMSE);
            Ok((difference > tolerance).then(|| pair_failure(&left, &right, difference, *region, because)))
        }
        Assertion::Painted {
            shot,
            because,
            region,
            min_deviation,
        } => {
            let Some(path) = shot_path(taken, shots, shot) else {
                bail!("gui-pass: assertion names a shot the script never took: {shot}");
            };
            let deviation = standard_deviation(&path, *region)?;
            let threshold = min_deviation.unwrap_or(MIN_STANDARD_DEVIATION);
            Ok(painted_failed(deviation, threshold).then(|| {
                format!(
                    "{} {} is flat (standard deviation {deviation:.4} <= {threshold}): {because}",
                    name_of(&path),
                    describe_region(*region),
                )
            }))
        }
        Assertion::Manifest { contains, because } => {
            let Some(workspace) = workspace else {
                bail!(
                    "gui-pass: the manifest assertion needs the isolated fixture workspace; \
                     --real-config points at the caller's own workspace, which this cannot safely read"
                );
            };
            let manifest = workspace.join("workspace.toml");
            let text = fs::read_to_string(&manifest).with_context(|| format!("reading {}", manifest.display()))?;
            Ok((!text.contains(contains.as_str()))
                .then(|| format!("{} does not contain {contains:?}: {because}", manifest.display())))
        }
    }
}

/// Resolves an assertion's two shot names to paths and measures their difference.
fn compare(
    named: &[String; 2],
    taken: &[String],
    shots: &Path,
    region: Option<[u32; 4]>,
) -> Result<(PathBuf, PathBuf, f64)> {
    let [left, right] = named;
    let (Some(left), Some(right)) = (shot_path(taken, shots, left), shot_path(taken, shots, right)) else {
        bail!("gui-pass: assertion names a shot the script never took: {left} / {right}");
    };
    let difference = difference(&left, &right, region)?;
    Ok((left, right, difference))
}

/// The failure message for a two-shot assertion.
fn pair_failure(left: &Path, right: &Path, difference: f64, region: Option<[u32; 4]>, because: &str) -> String {
    format!(
        "{} vs {} over {} (RMSE {difference:.4}): {because}",
        name_of(left),
        name_of(right),
        describe_region(region),
    )
}

/// How a failure names the area it measured.
fn describe_region(region: Option<[u32; 4]>) -> String {
    match region {
        Some([x, y, w, h]) => format!("region {w}x{h}+{x}+{y}"),
        None => "whole window".to_owned(),
    }
}

/// A shot's file name, for messages.
fn name_of(path: &Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
}

/// Where the `index`-th shot named `name` is written.
fn shot_file(shots: &Path, index: usize, name: &str) -> PathBuf {
    shots.join(format!("{index:02}-{name}.png"))
}

/// The written path of the shot named `name`.
fn shot_path(taken: &[String], shots: &Path, name: &str) -> Option<PathBuf> {
    let index = taken.iter().position(|shot| shot == name)?;
    Some(shot_file(shots, index + 1, name))
}

#[cfg(test)]
mod tests {
    use super::{
        Assertion, MIN_STANDARD_DEVIATION, Script, WINDOW, describe_region, focus_click, painted_failed, read_region,
        window_size,
    };
    use std::path::Path;

    fn asserts(toml: &str) -> Vec<Assertion> {
        let script: Script = toml::from_str(toml).expect("the scenario parses");
        script.asserts
    }

    fn script(toml: &str) -> Script {
        toml::from_str(toml).expect("the scenario parses")
    }

    #[test]
    fn a_scenario_defaults_its_window_to_the_standard_size() {
        let parsed = script(r#"description = "a scenario""#);
        assert_eq!(window_size(&parsed), WINDOW);
    }

    #[test]
    fn a_scenario_can_declare_its_own_window() {
        let parsed = script(
            r#"
            description = "a scenario"
            window = [1280, 840]
            "#,
        );
        assert_eq!(window_size(&parsed), (1280, 840));
    }

    #[test]
    fn an_unknown_top_level_key_is_rejected() {
        // A typo'd key must fail to parse, not silently run the scenario at the default window.
        let parsed: Result<Script, _> = toml::from_str(
            r#"
            description = "a scenario"
            widnow = [1280, 840]
            "#,
        );
        assert!(parsed.is_err(), "an unknown top-level key must not parse");
    }

    #[test]
    fn the_focus_click_stays_in_empty_top_bar_space_at_both_sizes() {
        assert_eq!(focus_click(WINDOW), (900, 60), "the default window keeps today's value");
        assert_eq!(
            focus_click((1280, 840)),
            (640, 60),
            "a narrower window clamps to half its width, still left of .search's margin-left:auto"
        );
    }

    #[test]
    fn a_painted_assert_defaults_its_region_and_threshold() {
        let parsed = asserts(
            r#"
            description = "a scenario"

            [[assert]]
            kind = "painted"
            shot = "armed"
            because = "arming a draw tool must not blank the canvas"
            "#,
        );
        let [
            Assertion::Painted {
                shot,
                region,
                min_deviation,
                because,
            },
        ] = parsed.as_slice()
        else {
            panic!("one painted assertion, got {} others", parsed.len());
        };
        assert_eq!(shot, "armed");
        assert_eq!(*region, None);
        assert_eq!(*min_deviation, None);
        assert_eq!(because, "arming a draw tool must not blank the canvas");
    }

    #[test]
    fn a_painted_assert_carries_its_region_and_threshold_when_given() {
        let parsed = asserts(
            r#"
            description = "a scenario"

            [[assert]]
            kind = "painted"
            shot = "polygon-armed"
            region = [740, 140, 1050, 760]
            min_deviation = 0.02
            because = "the canvas region must show tiles, not a flat fill"
            "#,
        );
        let [
            Assertion::Painted {
                region, min_deviation, ..
            },
        ] = parsed.as_slice()
        else {
            panic!("one painted assertion, got {} others", parsed.len());
        };
        assert_eq!(*region, Some([740, 140, 1050, 760]));
        assert_eq!(*min_deviation, Some(0.02));
    }

    #[test]
    fn a_flat_region_fails_the_painted_predicate() {
        assert!(painted_failed(0.0, MIN_STANDARD_DEVIATION), "a uniform fill is blank");
        assert!(
            painted_failed(MIN_STANDARD_DEVIATION, MIN_STANDARD_DEVIATION),
            "the threshold itself is blank — the bound is inclusive, as assert_painted's is"
        );
    }

    #[test]
    fn a_textured_region_passes_the_painted_predicate() {
        assert!(!painted_failed(0.18, MIN_STANDARD_DEVIATION), "map tiles are textured");
        assert!(
            !painted_failed(0.03, 0.02),
            "a caller-raised threshold still passes on a region above it"
        );
    }

    #[test]
    fn a_failure_names_the_region_it_measured() {
        assert_eq!(describe_region(Some([740, 140, 1050, 760])), "region 1050x760+740+140");
        assert_eq!(describe_region(None), "whole window");
    }

    /// `identify` rejects the `-extract` flag `compare` takes, so a region reaches it as a read
    /// modifier on the file name instead. Getting this wrong measures the whole window and the
    /// assertion silently stops being able to see a blank canvas.
    #[test]
    fn a_region_reaches_identify_as_a_read_modifier() {
        let path = Path::new("target/gui-pass/shots/map-repaint/04-polygon-armed.png");
        assert_eq!(
            read_region(path, Some([740, 140, 1050, 760])),
            "target/gui-pass/shots/map-repaint/04-polygon-armed.png[1050x760+740+140]"
        );
        assert_eq!(
            read_region(path, None),
            "target/gui-pass/shots/map-repaint/04-polygon-armed.png"
        );
    }
}
