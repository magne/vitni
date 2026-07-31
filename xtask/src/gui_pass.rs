//! `gui-pass` — drives the real GUI on a headless X display and checks scripted scenarios.
//!
//! SSR tests stop at the markup; anything that only exists in a running `WebKitGTK` webview
//! (`document::eval`, CSS, the `MapLibre` canvas) needs the actual window. On a Wayland desktop the
//! window is not scriptable — synthetic input reaches it only while the compositor has focused it —
//! so this command runs the GUI on its own **Xvfb** display instead, where X focus *is* focus and
//! `xdotool` is deterministic. `MapLibre` renders there over software GL.
//!
//! Scenarios are **data, not code**: each is a TOML file under
//! `crates/genealogy-ui-dioxus/tests/gui-pass/`, so adding one needs no recompile. A file lists
//! `[[step]]`s (a click, a chord, a drag, a wheel, a screenshot) and `[[assert]]`s over the shots it
//! took — `differ` for "the UI reacted", `match` for "the UI returned to this state". Both compare
//! with an RMSE tolerance, so a caret blink is not a difference. An assertion may add `region = [x, y,
//! w, h]` to compare a single window sub-rectangle instead of the whole shot — needed when a change is
//! provably confined to one area but the rest of the window can legitimately repaint either way (e.g.
//! the tabstrip repaints on every Save, so a whole-window `differ` cannot isolate a list-column change).
//!
//! The run is isolated by default: a throwaway `XDG_CONFIG_HOME`/`XDG_DATA_HOME` under
//! `target/gui-pass/home` and a seeded fixture workspace, so a scripted click run can never append
//! assertions to real genealogy data. `--real-config` (optionally with `--workspace <name>`) points
//! the same scripts at the caller's own config and workspaces when reproducing something in real data.
//!
//! What it cannot settle: pan/zoom smoothness, click latency and motion. Software GL is not the
//! user's GPU. Those stay the `manual-verify` residual (see `docs/issue-tracking.md`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};
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
/// The window size every script's coordinates are written against. There is no window manager on the
/// display, so the window keeps whatever size `xdotool windowsize` gives it.
const WINDOW: (u32, u32) = (1800, 1200);
/// The fixture workspace name.
const FIXTURE_WORKSPACE: &str = "gui-pass";
/// The pristine copy of the seeded workspace, restored before every scenario.
const SEED_DIR: &str = "workspace-seed";
/// Empty top-bar space, clicked once at startup to hand the webview keyboard focus (see [`focus`]).
const FOCUS_CLICK: (i32, i32) = (900, 60);
/// How long to wait for the window to map before giving up.
const WINDOW_TIMEOUT: Duration = Duration::from_secs(45);
/// Standard deviation below which a screenshot is treated as blank (an unpainted or black window).
const MIN_STANDARD_DEVIATION: f64 = 0.005;
/// Normalized RMSE below which two shots count as the same screen. Above the caret blink and text
/// antialiasing that differ between two grabs of an unchanged screen, far below any real repaint.
const SAME_SCREEN_RMSE: f64 = 0.01;

/// One scenario: what it proves, the steps to drive, and the assertions over the shots taken.
#[derive(Deserialize)]
struct Script {
    /// What this scenario demonstrates, printed as the run header.
    description: String,
    #[serde(default, rename = "step")]
    steps: Vec<Step>,
    #[serde(default, rename = "assert")]
    asserts: Vec<Assertion>,
}

/// One scripted action. Coordinates are window pixels at [`WINDOW`], read off an earlier screenshot —
/// the window sits at the display origin, so they are display coordinates too.
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
}

/// One check over two shots the script took.
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
}

impl Assertion {
    fn shots(&self) -> &[String; 2] {
        match self {
            Self::Differ { shots, .. } | Self::Match { shots, .. } => shots,
        }
    }

    fn because(&self) -> &str {
        match self {
            Self::Differ { because, .. } | Self::Match { because, .. } => because,
        }
    }

    fn tolerance(&self) -> f64 {
        match self {
            Self::Differ { tolerance, .. } | Self::Match { tolerance, .. } => tolerance.unwrap_or(SAME_SCREEN_RMSE),
        }
    }

    fn region(&self) -> Option<[u32; 4]> {
        match self {
            Self::Differ { region, .. } | Self::Match { region, .. } => *region,
        }
    }
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
    }

    let mut session = start_session(options, home)?;
    let window = wait_for_window(&options.display)?;
    xdotool(
        &options.display,
        &["windowsize", &window, &WINDOW.0.to_string(), &WINDOW.1.to_string()],
    )?;
    focus(&options.display, &window)?;
    settle();

    let taken = drive(&options.display, &window, &script.steps, shots)?;
    if options.keep {
        session.keep = true;
        println!(
            "gui-pass: leaving {} and the GUI up — attach with `x11vnc -display {}`",
            options.display, options.display
        );
    }
    check(&script.asserts, &taken, shots)
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

/// Deletes the isolated home, the fixture workspace and the shots.
fn reset(out: &Path) -> Result<()> {
    for dir in ["home", "workspace", SEED_DIR, "shots"] {
        let path = out.join(dir);
        if path.exists() {
            fs::remove_dir_all(&path).with_context(|| format!("removing {}", path.display()))?;
        }
    }
    Ok(())
}

/// Creates the fixture workspace on first run: one place with coordinates, so the map has something
/// to plot. Idempotent — an existing workspace directory is reused.
fn seed_fixture(out: &Path, home: &Path) -> Result<()> {
    let workspace = out.join("workspace");
    if workspace.exists() {
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
    copy_dir(&out.join("workspace"), &out.join(SEED_DIR))?;
    println!("gui-pass: seeded workspace {FIXTURE_WORKSPACE} at {workspace} with place {place}");
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
fn start_session(options: &Options, home: &Path) -> Result<Session> {
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

    let mut gui = Command::new("target/debug/genealogy-gui");
    gui.env("DISPLAY", &options.display)
        // GTK prefers Wayland when the session advertises it, which would put the window on the
        // caller's desktop instead of the headless display.
        .env("GDK_BACKEND", "x11")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("XDG_SESSION_TYPE")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
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
/// click is silently dropped. [`FOCUS_CLICK`] is empty top-bar space, chosen so the click activates
/// nothing.
fn focus(display: &str, window: &str) -> Result<()> {
    xdotool(display, &["windowfocus", window])?;
    xdotool(
        display,
        &["mousemove", &FOCUS_CLICK.0.to_string(), &FOCUS_CLICK.1.to_string()],
    )?;
    xdotool(display, &["click", "1"])
}

/// Runs every step, returning the shot names in the order they were taken.
fn drive(display: &str, window: &str, steps: &[Step], shots: &Path) -> Result<Vec<String>> {
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
        }
    }
    Ok(taken)
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
    let deviation = standard_deviation(path)?;
    if deviation <= MIN_STANDARD_DEVIATION {
        bail!(
            "{} is blank (standard deviation {deviation}) — the webview painted nothing",
            path.display()
        );
    }
    Ok(())
}

/// A screenshot's pixel standard deviation, normalized to 0..1.
fn standard_deviation(path: &Path) -> Result<f64> {
    let measured = Command::new("identify")
        .args(["-format", "%[fx:standard_deviation]"])
        .arg(path)
        .output()
        .with_context(|| format!("measuring {}", path.display()))?;
    if !measured.status.success() {
        bail!("identify failed on {}", path.display());
    }
    parse_metric(&String::from_utf8_lossy(&measured.stdout), path)
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

/// Checks every assertion, reporting all failures rather than the first.
fn check(asserts: &[Assertion], taken: &[String], shots: &Path) -> Result<()> {
    let mut failures = Vec::new();
    for assertion in asserts {
        let [left, right] = assertion.shots();
        let (Some(left), Some(right)) = (shot_path(taken, shots, left), shot_path(taken, shots, right)) else {
            bail!("gui-pass: assertion names a shot the script never took: {left} / {right}");
        };
        let difference = difference(&left, &right, assertion.region())?;
        let tolerance = assertion.tolerance();
        let failed = match assertion {
            Assertion::Differ { .. } => difference <= tolerance,
            Assertion::Match { .. } => difference > tolerance,
        };
        if failed {
            failures.push(format!(
                "{} vs {} (RMSE {difference:.4}): {}",
                name_of(&left),
                name_of(&right),
                assertion.because()
            ));
        }
    }
    if !failures.is_empty() {
        bail!("{}", failures.join("; "));
    }
    Ok(())
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
