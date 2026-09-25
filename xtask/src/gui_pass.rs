//! `gui-pass` — drives the real GUI on a headless X display and checks scripted scenarios.
//!
//! SSR tests stop at the markup; anything that only exists in a running `WebKitGTK` webview
//! (`document::eval`, CSS, the `MapLibre` canvas) needs the actual window. On a Wayland desktop the
//! window is not scriptable — synthetic input reaches it only while the compositor has focused it —
//! so this command runs the GUI on its own **Xvfb** display instead, where X focus *is* focus and
//! `xdotool` is deterministic. `MapLibre` renders there over software GL.
//!
//! Scenarios are **data, not code**: each is a TOML file under
//! `crates/vitni-ui-dioxus/tests/gui-pass/`, so adding one needs no recompile. A top-level
//! `window = [w, h]` sets the size the window is resized to before its steps run, defaulting to
//! [`WINDOW`] when omitted — the narrow-window case (below `--bp-lg`) needs its own coordinates, never
//! a single-pane layout's carried over (see `CLAUDE.md`'s "Writing one"). A file lists `[[step]]`s (a
//! click, a chord, a typed word, a drag, a wheel, a screenshot, `wait` to sleep and let a timed effect fire,
//! `wm-close` to ask the window to close the way a window manager does, or `await-exit` to wait for the
//! GUI process itself to quit) and `[[assert]]`s over the shots it took —
//! `differ` for "the UI reacted",
//! `match` for "the UI returned to this state", `painted` for "this area is not a flat fill". The
//! first two compare with an RMSE tolerance, so a caret blink is not a difference. Any assertion may
//! add `region = [x, y, w, h]` to work on a single window sub-rectangle instead of the whole shot —
//! needed when a change is provably confined to one area but the rest of the window can legitimately
//! repaint either way (e.g. the tabstrip repaints on every Save, so a whole-window `differ` cannot
//! isolate a list-column change), and needed by `painted`, whose whole-window form the surrounding
//! chrome would always answer for. `manifest` is different again: it checks the worker's
//! `workspace/workspace.toml` on disk for a substring, proving a write reached disk
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
//! Scenarios run [`default_jobs`] at a time (`--jobs N` to change it), each [`Worker`] on its own Xvfb
//! display with its own isolated home and workspace, restored from one shared seed. A scenario's
//! progress lines are held and printed with its verdict, so parallel scenarios never interleave; with
//! one worker they print live.
//!
//! The driving machinery is parameterised by a [`Fixture`], because a second caller needs the same
//! harness over different data: `cargo xtask screenshots` (see [`crate::screenshots`]) drives a demo
//! family through [`run_fixture`] to produce the README images. [`GUI_PASS`] is this command's own.
//!
//! Running no window manager does not put the **window-manager close** out of reach: [`Step::WmClose`]
//! sends the toplevel the `WM_DELETE_WINDOW` `ClientMessage` the ICCCM defines for it, and GDK dispatches
//! it from its own event handling with no WM in sight — so the titlebar `✕` / session-logout path
//! (issue #281) is scriptable, and `wm-close-confirm.toml` drives it.
//!
//! What it cannot settle: pan/zoom smoothness, click latency and motion. Software GL is not the
//! user's GPU. Those stay the `manual-verify` residual (see `docs/issue-tracking.md`).

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use vitni_core::media_path::{MEDIA_DIR, workspace_media_path};

use crate::util::{copy_dir, run_cargo};

/// The `gui-pass` fixture: the assertion scenarios, seeded with one place and two media objects.
const GUI_PASS: Fixture = Fixture {
    name: "gui-pass",
    out_dir: "target/gui-pass",
    script_dir: "crates/vitni-ui-dioxus/tests/gui-pass",
    workspace: "gui-pass",
    workspace_dir: "workspace",
    seed: seed_gui_pass,
    required_media: &[SEED_MEDIA_REL, SEED_MEDIA_NORDIC_REL],
    env: &[],
};

/// The `vitni` launcher (ADR 0035), which both halves of a run drive: spawned with no arguments it is
/// the GUI under test, and invoked with arguments it is the CLI that seeds the fixture — so one build
/// covers both, and each dispatch arm is exercised by every run.
const LAUNCHER: &str = "target/debug/vitni";

/// The Xvfb display the GUI is driven on — the first worker's, with each further worker on the next
/// number. Overridable with `--display`.
pub const DEFAULT_DISPLAY: &str = ":99";
/// The most scenarios [`default_jobs`] runs at once. Measured on 22 cores: 1 worker 657 s, 2 334 s,
/// 4 177 s, 6 143 s, and 8 125 s with `toast-auto-dismiss` failing — its toast lives 6 s of wall-clock,
/// which that much contention stretches past.
const MAX_DEFAULT_JOBS: usize = 4;
/// The virtual screen Xvfb serves. Larger than the window so a resize never clips.
const SCREEN: &str = "2560x1600x24";
/// The window size a scenario's coordinates are written against, when it declares no `window` of its
/// own. There is no window manager on the display, so the window keeps whatever size `xdotool
/// windowsize` gives it.
const WINDOW: (u32, u32) = (1800, 1200);
/// The largest x a [`focus_click`] uses, matching today's value at the default [`WINDOW`] — see
/// [`focus_click`].
const MAX_FOCUS_X: i32 = 900;
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
/// A second seeded image, named in the alphabet the real data uses: `slugify` and the plugin host's
/// `sanitize_component` both keep `æøå`, and an operator's own directories carry spaces. An ASCII-only
/// fixture is structurally incapable of catching a percent-encoding defect in the served URL, which is
/// how #301 shipped with the preview still blank for every Nordic filename.
const SEED_MEDIA_NORDIC_REL: &str = "02_folketelling/1920 greipstad_bergstøl-asbjørn.png";
/// The seeded media image's side in pixels — big enough that the preview frame scales it down rather
/// than up, so the `painted` region measures real image pixels.
const SEED_MEDIA_SIZE: u32 = 480;
/// How long to wait for the window to map before giving up.
const WINDOW_TIMEOUT: Duration = Duration::from_secs(45);
/// How long to wait for Xvfb to accept connections before giving up.
const XVFB_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the window must stay unchanged before a step counts as settled.
const SETTLE_QUIET: Duration = Duration::from_millis(600);
/// The longest a step waits to settle. A screen that never stops changing (a spinner, an animating
/// canvas) costs this and no more — the fixed sleep every step used to take.
const SETTLE_CAP: Duration = Duration::from_secs(4);
/// How often a settling window is re-grabbed.
const SETTLE_POLL: Duration = Duration::from_millis(100);
/// Pixels that may differ between two grabs of a settled window: a blinking text caret is ~40.
const SETTLE_NOISE_PIXELS: usize = 200;
/// How long [`Step::AwaitExit`] waits for the GUI process to exit before failing.
const AWAIT_EXIT_TIMEOUT: Duration = Duration::from_secs(15);
/// Standard deviation below which a screenshot is treated as blank (an unpainted or black window).
const MIN_STANDARD_DEVIATION: f64 = 0.005;
/// Normalized RMSE below which two shots count as the same screen. Above the caret blink and text
/// antialiasing that differ between two grabs of an unchanged screen, far below any real repaint.
const SAME_SCREEN_RMSE: f64 = 0.01;

/// One fixture the harness can drive: where its state lives, which scenarios belong to it, and how
/// its workspace is seeded.
///
/// Two exist. [`GUI_PASS`] is the assertion harness — one place, two media objects, measured
/// coordinates. `screenshots` (see [`crate::screenshots`]) seeds a demo family instead, because a
/// README image of a genealogy program whose rail reads `People 0` argues against the README. They
/// stay separate fixtures rather than one enriched fixture: every Explorer list and rail count the
/// scenarios here were measured against would move if persons appeared in this one.
pub struct Fixture {
    /// How the fixture names itself in progress and failure messages.
    pub name: &'static str,
    /// Where the isolated home, the workspace, its pristine seed copy and the shots are written.
    pub out_dir: &'static str,
    /// Where this fixture's scenario files live.
    pub script_dir: &'static str,
    /// The workspace name registered in the isolated config.
    pub workspace: &'static str,
    /// The workspace directory below [`Self::out_dir`] — also what the status bar prints, since it
    /// shows the open workspace's directory name.
    pub workspace_dir: &'static str,
    /// Fills a freshly `init`ed workspace, and may edit the isolated config before it is copied to
    /// the config seed. Receives the fixture, the isolated home and the absolute workspace directory.
    pub seed: fn(&Fixture, &Path, &Path) -> Result<()>,
    /// Media-library paths (below the workspace's media root) a *reused* seed must already contain.
    /// A fixture directory left over from before one of them was added is stale, and saying so beats
    /// failing a scenario in a way that reads like the defect it is meant to catch.
    pub required_media: &'static [&'static str],
    /// Extra environment applied to both the seeding CLI and the GUI. `screenshots` pins
    /// `VITNI_LANGUAGE` with it, so the committed images are English whatever the machine's locale is.
    pub env: &'static [(&'static str, &'static str)],
}

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
    /// Type `text` into whatever has keyboard focus: one step and one settle for a whole word, not a
    /// `key` step (and a settle) per character. Letters, digits, space and `-.,` only — see [`keysyms`].
    Text { text: String, label: String },
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
pub struct Options {
    display: String,
    /// The scenarios to run; empty means every file in the fixture's script directory.
    scripts: Vec<String>,
    /// Use the caller's own config and workspaces instead of the isolated fixture.
    real_config: bool,
    /// The workspace to open; `None` seeds and opens the fixture's own.
    workspace: Option<String>,
    /// Leave Xvfb and the GUI running so a human can attach (e.g. `x11vnc -display :99`).
    keep: bool,
    /// Delete the isolated home and fixture workspace before seeding.
    reset: bool,
    /// How many scenarios run at once, each worker on its own display with its own home and workspace.
    jobs: usize,
}

impl Options {
    /// An always-isolated run of the named scenarios, reseeding the fixture first.
    ///
    /// The `--real-config` / `--workspace` escape hatches are unreachable through this constructor
    /// by design: a caller that writes committed artefacts (`screenshots`) must never be able to
    /// drive real genealogy data.
    #[must_use]
    pub fn isolated(display: String, scripts: Vec<String>, keep: bool) -> Self {
        Self {
            display,
            scripts,
            real_config: false,
            workspace: None,
            keep,
            reset: true,
            jobs: 1,
        }
    }
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

/// Runs the `gui-pass` command: every requested assertion scenario against the [`GUI_PASS`] fixture.
pub fn run(args: &[String]) -> Result<()> {
    let options = parse_args(args)?;
    let out = run_fixture(&options, &GUI_PASS)?;
    println!(
        "gui-pass: passed — shots under {}/shots; smoothness and latency still need a human.",
        out.display()
    );
    Ok(())
}

/// Runs every requested scenario of `fixture`, each in its own GUI instance so one cannot leave state
/// for the next, and returns the fixture's output directory.
///
/// # Errors
///
/// Fails if a driver tool is missing, a build fails, the fixture cannot be seeded, or any scenario's
/// steps or assertions fail — reporting every failing scenario rather than stopping at the first.
pub fn run_fixture(options: &Options, fixture: &Fixture) -> Result<PathBuf> {
    preflight()?;
    run_cargo(&["build", "-p", "vitni"])?;

    let out = PathBuf::from(fixture.out_dir);
    if options.reset {
        reset(fixture, &out)?;
    }
    let home = absolute(&out.join("home"))?;
    if !options.real_config {
        seed_fixture(fixture, &out, &home)?;
    }

    let scripts = resolve_scripts(fixture, &options.scripts)?;
    let jobs = options.jobs.min(scripts.len()).max(1);
    let mut workers = Vec::new();
    for index in 0..jobs {
        workers.push(Worker::new(options, fixture, &out, index)?);
    }
    let outcomes = run_queue(jobs, &scripts, |index, path| {
        let mut log = Log::new(jobs == 1);
        let outcome = workers
            .get(index)
            .context("gui-pass: no worker for this job")
            .and_then(|worker| run_one(options, fixture, worker, path, &mut log));
        report(fixture, &script_name(path), &log, outcome)
    });
    let mut failed = Vec::new();
    for name in outcomes.into_iter().flatten() {
        failed.push(name);
    }
    if !failed.is_empty() {
        bail!(
            "{}: {} of {} scenarios failed ({})",
            fixture.name,
            failed.len(),
            scripts.len(),
            failed.join(", ")
        );
    }
    println!("{}: {} scenarios passed.", fixture.name, scripts.len());
    Ok(out)
}

/// Runs one scenario end to end, from a fresh copy of the seeded workspace and an empty shot
/// directory — a scenario writes events (dropping a map point asserts coordinates), so sharing either
/// would make one scenario's result depend on which ran before it.
fn run_one(options: &Options, fixture: &Fixture, worker: &Worker, path: &Path, log: &mut Log) -> Result<()> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let script: Script = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    for step in &script.steps {
        if let Step::Text { text, .. } = step {
            keysyms(text).with_context(|| format!("in {}", path.display()))?;
        }
    }
    let name = script_name(path);
    log.line(&format!("\n=== {name} — {}", script.description));
    let shots = worker.out.join("shots").join(&name);
    if shots.exists() {
        fs::remove_dir_all(&shots).with_context(|| format!("clearing {}", shots.display()))?;
    }
    fs::create_dir_all(&shots).with_context(|| format!("creating {}", shots.display()))?;
    let shots = shots.as_path();
    if !options.real_config {
        restore_workspace(fixture, worker)?;
        restore_config(fixture, worker)?;
    }

    let size = window_size(&script);
    let display = worker.display.as_str();
    let mut session = start_session(options, fixture, worker, shots)?;
    let window = wait_for_window(display)?;
    xdotool(
        display,
        &["windowsize", &window, &size.0.to_string(), &size.1.to_string()],
    )?;
    focus(display, &window, size)?;
    let window = Window::connect(display, window)?;
    window.settle()?;

    let taken = drive(&window, &script.steps, shots, &mut session, log)?;
    if options.keep {
        session.keep = true;
        log.line(&format!(
            "gui-pass: leaving {display} and the GUI up — attach with `x11vnc -display {display}`"
        ));
    }
    // `--real-config` points the isolated fixture's workspace path at the caller's own workspace,
    // which a `manifest` assertion must not read — see `Assertion::Manifest`.
    let workspace = (!options.real_config).then_some(worker.workspace.as_path());
    check(&script.asserts, &taken, shots, workspace)
}

/// Prints a finished scenario's buffered log and its verdict together, so parallel scenarios never
/// interleave. Returns the scenario's name when it failed.
fn report(fixture: &Fixture, name: &str, log: &Log, outcome: Result<()>) -> Option<String> {
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "{}", log.text);
    match outcome {
        Ok(()) => {
            let _ = writeln!(out, "{}: {name} passed", fixture.name);
            None
        }
        Err(error) => {
            drop(out);
            eprintln!("{}: {name} FAILED: {error:#}", fixture.name);
            Some(name.to_owned())
        }
    }
}

/// One scenario's progress lines. With one worker they print as they happen, as a serial run always
/// has; with several they are held until the scenario ends, see [`report`].
struct Log {
    live: bool,
    text: String,
}

impl Log {
    fn new(live: bool) -> Self {
        Self {
            live,
            text: String::new(),
        }
    }

    fn line(&mut self, line: &str) {
        if self.live {
            println!("{line}");
        } else {
            self.text.push_str(line);
            self.text.push('\n');
        }
    }
}

/// Where one worker's scenarios run: its own X display, isolated home and workspace. Worker 0 keeps
/// the single-worker layout (`<out>/home`, `<out>/workspace` on `--display`) that `--keep`,
/// `--real-config` and `screenshots` use; worker *i* lives under `<out>/workers/<i>` on the display
/// *i* above it. Every worker restores from the one shared seed and writes shots to `<out>/shots`,
/// whose directories are per scenario.
struct Worker {
    display: String,
    home: PathBuf,
    workspace: PathBuf,
    /// The fixture's output directory, holding the shared seeds and the shots.
    out: PathBuf,
}

impl Worker {
    fn new(options: &Options, fixture: &Fixture, out: &Path, index: usize) -> Result<Self> {
        let root = if index == 0 {
            out.to_owned()
        } else {
            out.join("workers").join(index.to_string())
        };
        Ok(Self {
            display: worker_display(&options.display, index)?,
            home: absolute(&root.join("home"))?,
            workspace: absolute(&root.join(fixture.workspace_dir))?,
            out: out.to_owned(),
        })
    }
}

/// The display for worker `index`: `base` (`:<n>`) plus `index`.
fn worker_display(base: &str, index: usize) -> Result<String> {
    let number: usize = base
        .strip_prefix(':')
        .and_then(|number| number.parse().ok())
        .with_context(|| format!("gui-pass: --display takes `:<number>`, not {base:?}"))?;
    Ok(format!(":{}", number + index))
}

/// The seeded global config with workspace `name` pointed at `path` — a worker's own copy.
fn worker_config(seed: &str, name: &str, path: &Path) -> Result<String> {
    let mut config: toml::Table = toml::from_str(seed).context("parsing the seeded config")?;
    let Some(workspace) = config
        .get_mut("workspaces")
        .and_then(toml::Value::as_table_mut)
        .and_then(|workspaces| workspaces.get_mut(name))
        .and_then(toml::Value::as_table_mut)
    else {
        bail!("gui-pass: the seeded config registers no workspace {name:?} — re-run with --reset");
    };
    workspace.insert(
        "path".to_owned(),
        toml::Value::String(path.to_string_lossy().into_owned()),
    );
    toml::to_string(&config).context("writing a worker's config")
}

/// Runs `run` over every item on up to `jobs` threads, each pulling the next item as it frees up, and
/// returns the results in item order. `run` receives the worker index (`0..jobs`) with the item, so
/// each worker can use resources only it touches.
fn run_queue<T: Sync, R: Send>(jobs: usize, items: &[T], run: impl Fn(usize, &T) -> R + Sync) -> Vec<R> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, PoisonError};

    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new(items.iter().map(|_| None).collect());
    std::thread::scope(|scope| {
        for worker in 0..jobs.clamp(1, items.len().max(1)) {
            let (next, results, run) = (&next, &results, &run);
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    let result = run(worker, item);
                    let mut results = results.lock().unwrap_or_else(PoisonError::into_inner);
                    if let Some(slot) = results.get_mut(index) {
                        *slot = Some(result);
                    }
                }
            });
        }
    });
    let results = results.into_inner().unwrap_or_else(PoisonError::into_inner);
    let mut ordered = Vec::new();
    for result in results.into_iter().flatten() {
        ordered.push(result);
    }
    ordered
}

/// The scenario files to run: the named ones (a bare name, or a path), else every file in the
/// fixture's script directory in name order.
fn resolve_scripts(fixture: &Fixture, named: &[String]) -> Result<Vec<PathBuf>> {
    let dir = Path::new(fixture.script_dir);
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
            bail!("{}: no scenarios in {}", fixture.name, dir.display());
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
            bail!("{}: no scenario {name} (looked in {})", fixture.name, dir.display());
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
        jobs: 1,
    };
    let mut jobs = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--jobs" => jobs = Some(job_count(&value(&mut rest, "--jobs")?)?),
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
    let single = options.keep || options.real_config;
    options.jobs = match jobs {
        Some(count) if single && count > 1 => {
            bail!("gui-pass: --keep and --real-config/--workspace drive one GUI, so they need --jobs 1")
        }
        Some(count) => count,
        None if single => 1,
        None => default_jobs(available_cores()),
    };
    Ok(options)
}

/// How many scenarios run at once when `--jobs` is not given: one worker per four cores, since each
/// runs a software-GL webview, capped at [`MAX_DEFAULT_JOBS`].
fn default_jobs(cores: usize) -> usize {
    (cores / 4).clamp(1, MAX_DEFAULT_JOBS)
}

/// The machine's core count, or 1 when it cannot be read.
fn available_cores() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
}

/// `--jobs`' value: a positive worker count.
fn job_count(text: &str) -> Result<usize> {
    let count: usize = text
        .parse()
        .with_context(|| format!("gui-pass: --jobs takes a number of workers, not {text:?}"))?;
    if count == 0 {
        bail!("gui-pass: --jobs needs at least one worker");
    }
    Ok(count)
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
        bail!("driving the GUI headlessly needs: {}", missing.join(", "));
    }
    Ok(())
}

/// Deletes the isolated home, the fixture workspace, the seeded global config and the shots.
fn reset(fixture: &Fixture, out: &Path) -> Result<()> {
    for dir in ["home", fixture.workspace_dir, SEED_DIR, "shots", "workers"] {
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

/// Creates the fixture's workspace on first run — `init`, then whatever [`Fixture::seed`] fills it
/// with — and copies both the workspace and the isolated config into the pristine seeds every
/// scenario is restored from. Idempotent: an existing workspace directory is reused.
fn seed_fixture(fixture: &Fixture, out: &Path, home: &Path) -> Result<()> {
    let workspace = out.join(fixture.workspace_dir);
    if workspace.exists() {
        return verify_seed(fixture, out);
    }
    fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let workspace = absolute(&workspace)?;
    let path = workspace.to_string_lossy().into_owned();
    cli(fixture, home, &["init", fixture.workspace, &path])?;
    let config = config_file(home);
    if !config.exists() {
        bail!(
            "{}: init wrote no config at {} — the isolation failed and a real config may have been \
             registered instead",
            fixture.name,
            config.display()
        );
    }
    (fixture.seed)(fixture, home, &workspace)?;
    fs::copy(&config, out.join(CONFIG_SEED_FILE)).with_context(|| format!("seeding {CONFIG_SEED_FILE}"))?;
    copy_dir(&workspace, &out.join(SEED_DIR))?;
    println!("{}: seeded workspace {} at {path}", fixture.name, fixture.workspace);
    Ok(())
}

/// Rejects a reused seed that predates one of the fixture's [`Fixture::required_media`] images: a
/// workspace seeded before one was added would fail `media-preview` with a missing Media row rather
/// than a blank preview, which reads like the defect the scenario is meant to catch.
fn verify_seed(fixture: &Fixture, out: &Path) -> Result<()> {
    for rel in fixture.required_media {
        let seeded = out.join(SEED_DIR).join(MEDIA_DIR).join(rel);
        if !seeded.is_file() {
            bail!(
                "{}: the fixture predates a seeded media image ({} is missing) — re-run with `--reset`",
                fixture.name,
                seeded.display()
            );
        }
    }
    Ok(())
}

/// Fills the [`GUI_PASS`] workspace: one place with coordinates, so the map has something to plot, two
/// media objects pointing at seeded images (see [`seed_media`]), plus an inactive
/// `[map.providers.demo]` `MapLibre` style (ADR 0033) the `map-provider-switch` scenario switches to.
fn seed_gui_pass(fixture: &Fixture, home: &Path, workspace: &Path) -> Result<()> {
    let created = cli(
        fixture,
        home,
        &["place", "create", "--type", "municipality", "--name", "Kristiansand"],
    )?;
    let place = created
        .split_whitespace()
        .next_back()
        .with_context(|| format!("no place id in {created:?}"))?
        .to_owned();
    cli(
        fixture,
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
    seed_media(fixture, home, workspace)?;
    let config = config_file(home);
    let mut text = fs::read_to_string(&config).with_context(|| format!("reading {}", config.display()))?;
    text.push_str(DEMO_MAP_PROVIDER);
    fs::write(&config, text).with_context(|| format!("writing {}", config.display()))?;
    println!("gui-pass: seeded place {place}");
    Ok(())
}

/// Writes two images into the fixture workspace's media library and records a Media object pointing at
/// each, so `media-preview.toml` has something whose preview can be blank (#301) in both the ASCII and
/// the Nordic/spaced spelling ([`SEED_MEDIA_NORDIC_REL`]) — a regression in either is then witnessed.
///
/// The images are *generated* with `ImageMagick` (already a hard requirement, see [`preflight`]) rather
/// than committed: a deterministic gradient with a filled circle, textured enough that a `painted`
/// assertion over the preview frame measures the image and not its background. The committed icon
/// rasters (`assets/icon/`) will not do — the largest is 256 px of near-flat plate, so a `painted`
/// calibration over the preview frame would measure the plate rather than the preview. Both are the
/// same image, so one `painted` calibration covers both rows.
///
/// The records deliberately carry **no MIME**: `vitni media` has no `set-mime`, so this is the
/// state every record the CLI creates is in, and #301's two live causes (no inferred MIME, and the
/// stored `media/` prefix added twice) both fire on it.
fn seed_media(fixture: &Fixture, home: &Path, workspace: &Path) -> Result<()> {
    for rel in [SEED_MEDIA_REL, SEED_MEDIA_NORDIC_REL] {
        let target = workspace.join(MEDIA_DIR).join(rel);
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
        let stored = workspace_media_path(rel);
        cli(fixture, home, &["media", "create", "--path", &stored])?;
        println!("gui-pass: seeded media {stored}");
    }
    Ok(())
}

/// Replaces the fixture workspace with a fresh copy of the seed, so every scenario starts from the
/// same data. Nothing is running against it yet — this is called before the GUI launches.
fn restore_workspace(fixture: &Fixture, worker: &Worker) -> Result<()> {
    let seed = worker.out.join(SEED_DIR);
    if !seed.is_dir() {
        bail!(
            "{}: no seed at {} — re-run with --reset to reseed the fixture",
            fixture.name,
            seed.display()
        );
    }
    let workspace = &worker.workspace;
    if workspace.exists() {
        fs::remove_dir_all(workspace).with_context(|| format!("removing {}", workspace.display()))?;
    }
    copy_dir(&seed, workspace)
}

/// Replaces the isolated global config with a fresh copy of the seed (ADR 0033's `map-provider-switch`
/// scenario writes to it, next to [`restore_workspace`]'s own reasoning) — nothing is running against
/// it yet, called before the GUI launches. The copy registers the fixture workspace at this worker's
/// own [`Worker::workspace`] (see [`worker_config`]).
fn restore_config(fixture: &Fixture, worker: &Worker) -> Result<()> {
    let seed = worker.out.join(CONFIG_SEED_FILE);
    if !seed.is_file() {
        bail!(
            "gui-pass: no seeded config at {} — re-run with --reset to reseed the fixture",
            seed.display()
        );
    }
    let text = fs::read_to_string(&seed).with_context(|| format!("reading {}", seed.display()))?;
    let config = config_file(&worker.home);
    if let Some(parent) = config.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let rewritten = worker_config(&text, fixture.workspace, &worker.workspace)?;
    fs::write(&config, rewritten).with_context(|| format!("restoring {}", config.display()))?;
    Ok(())
}

/// The global configuration file inside an isolated home (ADR 0005 paths under `XDG_CONFIG_HOME`).
#[must_use]
pub fn config_file(home: &Path) -> PathBuf {
    home.join(".config/vitni/config.toml")
}

/// Runs the CLI against the isolated home and the fixture's workspace, returning its stdout.
///
/// # Errors
///
/// Fails if the binary cannot be run, or if it exits non-zero — quoting its stderr.
pub fn cli(fixture: &Fixture, home: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new(LAUNCHER)
        .args(args)
        .envs(isolated_home(home))
        .envs(fixture.env.iter().copied())
        .env("VITNI_WORKSPACE", fixture.workspace)
        .output()
        .with_context(|| format!("running vitni {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "vitni {} failed with {}: {}",
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
fn start_session(options: &Options, fixture: &Fixture, worker: &Worker, shots: &Path) -> Result<Session> {
    let xvfb = Command::new("Xvfb")
        .args([&worker.display, "-screen", "0", SCREEN, "-nolisten", "tcp"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("starting Xvfb")?;
    let mut session = Session {
        xvfb,
        gui: None,
        keep: false,
    };
    wait_for_xvfb(&worker.display, &mut session.xvfb)?;

    let log_path = shots.join("gui.log");
    let log = fs::File::create(&log_path).with_context(|| format!("creating {}", log_path.display()))?;
    let errors = log
        .try_clone()
        .with_context(|| format!("sharing {}", log_path.display()))?;
    // No arguments, so the launcher takes its GUI arm (ADR 0035 §2) — which is what makes every
    // scenario a test of that dispatch as well as of the window it opens.
    let mut gui = Command::new(LAUNCHER);
    gui.env("DISPLAY", &worker.display)
        // GTK prefers Wayland when the session advertises it, which would put the window on the
        // caller's desktop instead of the headless display.
        .env("GDK_BACKEND", "x11")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("XDG_SESSION_TYPE")
        .env("RUST_LOG", "info")
        .envs(fixture.env.iter().copied())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(errors));
    if !options.real_config {
        gui.envs(isolated_home(&worker.home));
    }
    if let Some(name) = options.workspace.as_deref() {
        gui.env("VITNI_WORKSPACE", name);
    } else if !options.real_config {
        gui.env("VITNI_WORKSPACE", fixture.workspace);
    }
    session.gui = Some(gui.spawn().context("starting the GUI")?);
    Ok(session)
}

/// Polls until the new Xvfb accepts connections. Fails if it exits instead — it does when another
/// server already holds the display, and connecting then would drive whatever that server is showing.
fn wait_for_xvfb(display: &str, xvfb: &mut Child) -> Result<()> {
    let poll = Duration::from_millis(50);
    let started = Instant::now();
    while started.elapsed() < XVFB_TIMEOUT {
        if let Some(status) = xvfb.try_wait().context("polling Xvfb")? {
            bail!("gui-pass: Xvfb exited ({status}) — is another X server already on {display}?");
        }
        if x11rb::connect(Some(display)).is_ok() {
            return Ok(());
        }
        sleep(poll);
    }
    bail!("gui-pass: Xvfb did not accept connections on {display} within {XVFB_TIMEOUT:?}")
}

/// Polls for the GUI window until it maps.
fn wait_for_window(display: &str) -> Result<String> {
    let poll = Duration::from_millis(500);
    let mut waited = Duration::ZERO;
    while waited < WINDOW_TIMEOUT {
        let found = Command::new("xdotool")
            .args(["search", "--name", "^Vitni$"])
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
    bail!("gui-pass: no Vitni window appeared on {display} within {WINDOW_TIMEOUT:?}")
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
fn drive(window: &Window, steps: &[Step], shots: &Path, session: &mut Session, log: &mut Log) -> Result<Vec<String>> {
    let display = window.display.as_str();
    let mut taken = Vec::new();
    for step in steps {
        match step {
            Step::Shot { name } => {
                let path = shot_file(shots, taken.len() + 1, name);
                grab(display, &window.id, &path)?;
                assert_painted(&path)?;
                log.line(&format!("  shot {}", path.display()));
                taken.push(name.clone());
            }
            Step::Click { at, label } => {
                log.line(&format!("  {label}"));
                xdotool(display, &["mousemove", &at[0].to_string(), &at[1].to_string()])?;
                xdotool(display, &["click", "1"])?;
                window.settle()?;
            }
            Step::Key { chord, label } => {
                log.line(&format!("  {label}"));
                xdotool(display, &["key", "--clearmodifiers", chord])?;
                window.settle()?;
            }
            Step::Text { text, label } => {
                log.line(&format!("  {label}"));
                type_text(display, text)?;
                window.settle()?;
            }
            Step::Drag { from, by, label } => {
                log.line(&format!("  {label}"));
                drag(display, *from, *by)?;
                window.settle()?;
            }
            Step::Wheel { at, clicks, label } => {
                log.line(&format!("  {label}"));
                wheel(display, *at, *clicks)?;
                window.settle()?;
            }
            Step::AwaitExit { label } => {
                log.line(&format!("  {label}"));
                await_exit(session)?;
            }
            Step::WmClose { label } => {
                log.line(&format!("  {label}"));
                wm_close(display, &window.id)?;
                window.settle()?;
            }
            Step::Wait { seconds, label } => {
                log.line(&format!("  {label}"));
                sleep(Duration::from_secs(*seconds));
                window.settle()?;
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

/// The gap between a [`Step::Text`] step's keystrokes. Measured on Xvfb: keys 12 ms apart lose
/// characters (3 runs in 4), 30 ms apart none (4 of 4), so this leaves a margin over the latter.
const TYPE_DELAY_MS: &str = "40";

/// Types `text` as one `xdotool key` call over explicit keysyms.
///
/// Not `xdotool type`: it remaps keycodes on the fly for each character, and on Xvfb that drops or
/// reorders characters at any delay ("Oslo" came out as "Oso"). `key` with a fixed keysym per
/// character and [`TYPE_DELAY_MS`] between them types every character.
fn type_text(display: &str, text: &str) -> Result<()> {
    let keys = keysyms(text)?;
    let mut args = vec!["key", "--clearmodifiers", "--delay", TYPE_DELAY_MS];
    for key in &keys {
        args.push(key);
    }
    xdotool(display, &args)
}

/// The `xdotool key` keysym for each character of `text`: letters (upper case as `shift+`), digits,
/// space and `-.,`. Anything else fails, naming the character — the default Xvfb keymap has no key for
/// `æøå` and friends, and a remapped one would bring back the `xdotool type` unreliability.
fn keysyms(text: &str) -> Result<Vec<String>> {
    if text.is_empty() {
        bail!("gui-pass: a text step needs something to type");
    }
    let mut keys = Vec::new();
    for character in text.chars() {
        let key = match character {
            'a'..='z' | '0'..='9' => character.to_string(),
            'A'..='Z' => format!("shift+{}", character.to_ascii_lowercase()),
            ' ' => "space".to_owned(),
            '-' => "minus".to_owned(),
            '.' => "period".to_owned(),
            ',' => "comma".to_owned(),
            other => bail!("gui-pass: a text step cannot type {other:?} — use letters, digits, space or -.,"),
        };
        keys.push(key);
    }
    Ok(keys)
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

/// The GUI's toplevel window: the id `xdotool` and `import` address it by, plus an X connection that
/// reads its pixels back directly, so settling can poll many times a second without spawning a
/// process per frame.
struct Window {
    display: String,
    id: String,
    xid: u32,
    connection: x11rb::rust_connection::RustConnection,
}

impl Window {
    fn connect(display: &str, id: String) -> Result<Self> {
        let xid = id.parse().with_context(|| format!("parsing the window id {id:?}"))?;
        let (connection, _) = x11rb::connect(Some(display)).with_context(|| format!("connecting to {display}"))?;
        Ok(Self {
            display: display.to_owned(),
            id,
            xid,
            connection,
        })
    }

    /// Waits until the window has stopped changing for [`SETTLE_QUIET`], or [`SETTLE_CAP`] has passed.
    ///
    /// Every step used to sleep a flat 4 s, which was most of a run's wall-clock. Most steps repaint in
    /// well under a second; the ceiling keeps the old bound for a screen that never goes quiet (a
    /// spinner, or a map whose tiles keep landing). A frame counts as unchanged when it differs from the
    /// one that started the quiet period by at most [`SETTLE_NOISE_PIXELS`] — a text caret blinks on a
    /// ~600 ms cycle, which would otherwise hold every step with a focused input at the cap. A window
    /// that has gone (the step quit the app) has nothing left to settle.
    fn settle(&self) -> Result<()> {
        let started = Instant::now();
        sleep(SETTLE_POLL);
        let Some(mut reference) = self.frame()? else {
            return Ok(());
        };
        let mut quiet_since = Instant::now();
        while started.elapsed() < SETTLE_CAP {
            sleep(SETTLE_POLL);
            let Some(frame) = self.frame()? else {
                return Ok(());
            };
            if differing_pixels(&reference, &frame) > SETTLE_NOISE_PIXELS {
                reference = frame;
                quiet_since = Instant::now();
            } else if quiet_since.elapsed() >= SETTLE_QUIET {
                return Ok(());
            }
        }
        Ok(())
    }

    /// The window's current pixels, as the server's `ZPixmap` bytes, or `None` once the window is gone.
    fn frame(&self) -> Result<Option<Vec<u8>>> {
        use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat};

        let geometry = match self
            .connection
            .get_geometry(self.xid)
            .context("asking the window's geometry")?
            .reply()
        {
            Ok(geometry) => geometry,
            Err(error) if window_gone(&error) => return Ok(None),
            Err(error) => return Err(error).context("reading the window's geometry"),
        };
        let image = self
            .connection
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.xid,
                0,
                0,
                geometry.width,
                geometry.height,
                u32::MAX,
            )
            .context("grabbing the window")?
            .reply();
        match image {
            Ok(image) => Ok(Some(image.data)),
            Err(error) if window_gone(&error) => Ok(None),
            Err(error) => Err(error).context("reading the window's pixels"),
        }
    }
}

/// Whether a request failed because its window no longer exists — the app quit between two grabs.
fn window_gone(error: &x11rb::errors::ReplyError) -> bool {
    use x11rb::errors::ReplyError;
    use x11rb::protocol::ErrorKind;

    match error {
        ReplyError::X11Error(error) => {
            let kind = error.error_kind;
            kind == ErrorKind::Drawable || kind == ErrorKind::Window || kind == ErrorKind::Match
        }
        ReplyError::ConnectionError(_) => false,
    }
}

/// How many 4-byte pixels differ between two frames; every pixel, when a resize changed their size.
fn differing_pixels(a: &[u8], b: &[u8]) -> usize {
    if a.len() != b.len() {
        return a.len().max(b.len()) / 4;
    }
    let mut differing = 0;
    for (left, right) in a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0) {
        if left != right {
            differing += 1;
        }
    }
    differing
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
        Assertion, MIN_STANDARD_DEVIATION, Script, Step, WINDOW, available_cores, default_jobs, describe_region,
        differing_pixels, focus_click, keysyms, painted_failed, parse_args, read_region, run_queue, window_size,
        worker_config, worker_display,
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
    fn a_text_step_carries_the_whole_string() {
        let parsed = script(
            r#"
            description = "a scenario"

            [[step]]
            do = "text"
            text = "Oslo"
            label = "type: Oslo"
            "#,
        );
        let [Step::Text { text, label }] = parsed.steps.as_slice() else {
            panic!("expected one text step");
        };
        assert_eq!(text, "Oslo");
        assert_eq!(label, "type: Oslo");
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn the_default_worker_count_follows_the_cores_up_to_four() {
        // Each worker runs a software-GL webview; one per four cores kept every scenario green here,
        // and past four the gain was small while a timing-sensitive scenario started to fail.
        assert_eq!(default_jobs(1), 1);
        assert_eq!(default_jobs(4), 1);
        assert_eq!(default_jobs(8), 2);
        assert_eq!(default_jobs(12), 3);
        assert_eq!(default_jobs(22), 4);
        assert_eq!(default_jobs(64), 4);
    }

    #[test]
    fn jobs_default_to_the_parallel_default_and_take_a_count() {
        let default = parse_args(&args(&[])).expect("no flags parse");
        assert_eq!(default.jobs, default_jobs(available_cores()));
        let four = parse_args(&args(&["--jobs", "4", "map-zoom"])).expect("--jobs 4 parses");
        assert_eq!(four.jobs, 4);
        assert_eq!(four.scripts, ["map-zoom"]);
    }

    #[test]
    fn jobs_must_be_a_positive_count() {
        assert!(
            parse_args(&args(&["--jobs", "0"])).is_err(),
            "zero workers can run nothing"
        );
        assert!(
            parse_args(&args(&["--jobs", "many"])).is_err(),
            "a non-number is rejected"
        );
        assert!(parse_args(&args(&["--jobs"])).is_err(), "the count is required");
    }

    #[test]
    fn parallel_runs_refuse_keep_and_real_config() {
        // `--keep` leaves one GUI up to attach to, and `--real-config` shares one real workspace — neither
        // means anything across several workers at once.
        assert!(parse_args(&args(&["--jobs", "2", "--keep"])).is_err());
        assert!(parse_args(&args(&["--jobs", "2", "--real-config"])).is_err());
        assert!(parse_args(&args(&["--jobs", "2", "--workspace", "gen"])).is_err());
        let serial = parse_args(&args(&["--jobs", "1", "--keep"])).expect("one worker may keep its GUI");
        assert!(serial.keep);
    }

    #[test]
    fn keep_and_real_config_imply_one_worker_when_jobs_is_not_given() {
        assert_eq!(parse_args(&args(&["--keep"])).expect("parses").jobs, 1);
        assert_eq!(parse_args(&args(&["--workspace", "gen"])).expect("parses").jobs, 1);
    }

    #[test]
    fn each_worker_gets_the_next_display() {
        assert_eq!(worker_display(":99", 0).expect("valid"), ":99");
        assert_eq!(worker_display(":99", 3).expect("valid"), ":102");
        assert!(worker_display("99", 1).is_err(), "a display is `:<number>`");
    }

    #[test]
    fn a_worker_config_points_the_workspace_at_the_workers_copy() {
        let seed =
            "default = \"gui-pass\"\n\n[workspaces.gui-pass]\npath = \"/old/workspace\"\n\n[operator]\nid = \"x\"\n";
        let config = worker_config(seed, "gui-pass", Path::new("/w/1/workspace")).expect("rewrites");
        let parsed: toml::Table = toml::from_str(&config).expect("still TOML");
        assert_eq!(
            parsed["workspaces"]["gui-pass"]["path"].as_str(),
            Some("/w/1/workspace")
        );
        assert_eq!(parsed["operator"]["id"].as_str(), Some("x"), "everything else is kept");
    }

    #[test]
    fn a_worker_config_without_the_workspace_is_an_error() {
        let seed = "[workspaces.other]\npath = \"/x\"\n";
        assert!(worker_config(seed, "gui-pass", Path::new("/w")).is_err());
    }

    #[test]
    fn the_queue_runs_every_item_once_and_keeps_their_order() {
        let items: Vec<usize> = (0..10).collect();
        let workers = std::sync::Mutex::new(std::collections::BTreeSet::new());
        let results = run_queue(3, &items, |worker, item| {
            workers.lock().expect("not poisoned").insert(worker);
            item * 10
        });
        assert_eq!(results, (0..10).map(|item| item * 10).collect::<Vec<_>>());
        let used = workers.into_inner().expect("not poisoned");
        assert!(used.iter().all(|worker| *worker < 3), "only workers 0..3 run: {used:?}");
    }

    #[test]
    fn more_workers_than_items_is_fine() {
        assert_eq!(run_queue(8, &[1, 2], |_, item| *item), [1, 2]);
        assert_eq!(run_queue(1, &[1, 2, 3], |worker, item| worker + item), [1, 2, 3]);
    }

    #[test]
    fn text_maps_to_one_keysym_per_character() {
        let keys = keysyms("TRee-7 Oslo").expect("every character is typeable");
        assert_eq!(
            keys,
            [
                "shift+t", "shift+r", "e", "e", "minus", "7", "space", "shift+o", "s", "l", "o"
            ]
        );
    }

    #[test]
    fn an_untypeable_character_is_rejected_by_name() {
        let error = keysyms("Bærum").expect_err("æ has no key on the Xvfb keymap");
        assert!(
            format!("{error:#}").contains("'æ'"),
            "the error names the character: {error:#}"
        );
    }

    #[test]
    fn empty_text_is_rejected() {
        assert!(
            keysyms("").is_err(),
            "a text step with nothing to type is a scenario mistake"
        );
    }

    #[test]
    fn a_text_step_needs_its_text() {
        let parsed: Result<Script, _> = toml::from_str(
            r#"
            description = "a scenario"

            [[step]]
            do = "text"
            label = "type: nothing"
            "#,
        );
        assert!(parsed.is_err(), "a text step without `text` must not parse");
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

    #[test]
    fn identical_frames_have_no_differing_pixels() {
        let frame = [1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(differing_pixels(&frame, &frame), 0);
        assert_eq!(differing_pixels(&[], &[]), 0);
    }

    #[test]
    fn a_pixel_differs_when_any_of_its_bytes_do() {
        let before = [0, 0, 0, 0, 9, 9, 9, 9, 5, 5, 5, 5];
        let after = [0, 0, 0, 1, 9, 9, 9, 9, 6, 5, 5, 5];
        assert_eq!(differing_pixels(&before, &after), 2);
    }

    #[test]
    fn a_resized_frame_differs_everywhere() {
        let small = [0; 8];
        let large = [0; 16];
        assert_eq!(differing_pixels(&small, &large), 4);
        assert_eq!(differing_pixels(&large, &small), 4);
    }
}
