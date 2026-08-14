//! `screenshots` — regenerates the README images from the running GUI (#328).
//!
//! A genealogy program with no picture of itself reads as vapourware, and hand-cropped images rot the
//! first time the UI moves. This is the refresh path: one command that seeds a demo workspace, drives
//! the real GUI over it on a headless display, and writes the committed PNGs under [`ASSET_DIR`].
//!
//! It is [`crate::gui_pass`]'s harness over a second [`Fixture`], not a second harness. What differs
//! is the data and the intent: `gui-pass` seeds one place and asserts over what it grabs, while this
//! seeds a demo family and asserts nothing — [`SCENARIO`] exists to produce images. A `gui-pass`
//! fixture with people in it would move every Explorer list and rail count its scenarios were measured
//! against, which is why the two fixtures stay apart.
//!
//! **Every run reseeds.** The demo workspace is thrown away and rebuilt from [`seed_demo`] each time,
//! so there is no cached state a stale image could come from — and the acceptance bar for this command
//! is that two runs produce no diff at all. Three things would otherwise make each run differ:
//!
//! 1. **The clock.** Every assertion carries the wall-clock instant it was made, and the UI renders it
//!    (`friendly_timestamp`) in the Dashboard's activity feed, the History tab and the *Why we believe*
//!    popover. [`pin_clock`] restamps the seeded event log to fixed, evenly spaced instants and rebuilds
//!    the projections from it (ADR 0010), so the rendered timestamps are a property of the seed script
//!    rather than of the moment it ran.
//! 2. **The operator.** `init` defaults the operator's display name to the OS user, which would put
//!    whoever regenerated the images into them. [`pin_operator`] overwrites it with [`OPERATOR`].
//! 3. **The locale.** The fixture pins `VITNI_LANGUAGE`, so the images are English on a machine whose
//!    session is not.
//!
//! Aggregate ids and `AssertionId`s stay random per run — they are UUIDs the UI never renders. Human
//! ids are pinned explicitly by the seed (`--id I0001`), so nothing depends on allocation order.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use sqlx::Row;
use sqlx::sqlite::SqliteConnectOptions;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::datetime;
use vitni_core::provenance::Timestamp;

use crate::gui_pass::{self, Fixture, Options};

/// The demo fixture: its own output tree, workspace and seed, isolated from `gui-pass`'s.
///
/// The workspace directory is named `demo` because the status bar prints the open workspace's
/// *directory* name, and that name is in every shot.
const SCREENSHOTS: Fixture = Fixture {
    name: "screenshots",
    out_dir: "target/screenshots",
    script_dir: "crates/vitni-ui-dioxus/tests/screenshots",
    workspace: "demo",
    workspace_dir: "demo",
    seed: seed_demo,
    required_media: &[],
    env: &[("VITNI_LANGUAGE", "en")],
};

/// The scenario that takes the shots.
const SCENARIO: &str = "readme";

/// Where the committed images live, referenced from `README.md` by relative path.
const ASSET_DIR: &str = "docs/assets";

/// Each shot the scenario takes, the image it becomes, and the width it is scaled to. A shot named
/// here that the scenario never took is an error, not a silently missing image.
const IMAGES: [(&str, &str, u32); 3] = [
    ("hero", "dashboard.png", 1440),
    ("pedigree", "pedigree.png", 1440),
    ("geography", "geography.png", 1440),
];

/// The instant the first seeded assertion is stamped with.
const CLOCK: OffsetDateTime = datetime!(2026-03-14 09:00 UTC);

/// How far apart consecutive assertions are stamped. Distinct values, not one repeated instant: the
/// activity feed orders by `occurred_at`, and ties would leave its row order to the database.
const CLOCK_STEP: time::Duration = time::Duration::minutes(1);

/// The operator every demo assertion is attributed to.
const OPERATOR: &str = "Vitni demo";

/// Runs the `screenshots` command.
///
/// # Errors
///
/// Fails if a flag is unknown, the fixture cannot be seeded, the scenario fails to run, or a named
/// shot is missing from what it took.
pub fn run(args: &[String]) -> Result<()> {
    let (display, keep) = parse_args(args)?;
    let options = Options::isolated(display, vec![SCENARIO.to_owned()], keep);
    let out = gui_pass::run_fixture(&options, &SCREENSHOTS)?;
    export(&out)
}

/// Parses the command's flags: the display to drive and whether to leave the session up.
///
/// `--real-config` / `--workspace` are deliberately absent: this command writes committed artefacts,
/// so it must never be pointable at real genealogy data. `--reset` is absent too — every run reseeds.
fn parse_args(args: &[String]) -> Result<(String, bool)> {
    let mut display = gui_pass::DEFAULT_DISPLAY.to_owned();
    let mut keep = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--keep" => keep = true,
            "--display" => {
                display.clone_from(rest.next().context("screenshots: --display needs a value")?);
            }
            other => bail!("screenshots: unknown argument {other} (accepts --display :N and --keep)"),
        }
    }
    Ok((display, keep))
}

/// Copies each named shot into [`ASSET_DIR`], scaled to README width and quantised to a 256-colour
/// palette (which is a third of the bytes on the map shot and invisible on the flat-filled UI ones).
///
/// `-strip` is load-bearing: `ImageMagick` writes a `tIME` chunk into a PNG, so without it every run
/// would differ no matter how identical the pixels are.
fn export(out: &Path) -> Result<()> {
    let assets = Path::new(ASSET_DIR);
    fs::create_dir_all(assets).with_context(|| format!("creating {}", assets.display()))?;
    let shots = out.join("shots").join(SCENARIO);
    for (shot, file, width) in IMAGES {
        let source = shot_path(&shots, shot)?;
        let target = assets.join(file);
        let status = Command::new("convert")
            .arg(&source)
            .args(["-strip", "-define", "png:exclude-chunks=date,time"])
            .args(["-resize", &format!("{width}x")])
            .args(["-colors", "256"])
            .arg(&target)
            .status()
            .with_context(|| format!("scaling {} into {}", source.display(), target.display()))?;
        if !status.success() {
            bail!("convert failed with {status} writing {}", target.display());
        }
        println!("screenshots: {} <- {}", target.display(), source.display());
    }
    Ok(())
}

/// Finds the shot named `name` in a scenario's shot directory, whatever step index it carries.
fn shot_path(shots: &Path, name: &str) -> Result<PathBuf> {
    let suffix = format!("-{name}.png");
    for entry in fs::read_dir(shots).with_context(|| format!("reading {}", shots.display()))? {
        let path = entry
            .with_context(|| format!("reading an entry under {}", shots.display()))?
            .path();
        if path.to_string_lossy().ends_with(&suffix) {
            return Ok(path);
        }
    }
    bail!("screenshots: {SCENARIO}.toml took no shot named {name} — every image in IMAGES needs one")
}

/// Fills the demo workspace, then makes it reproducible: pin the operator before anything is asserted,
/// seed the records, and restamp the resulting log to a fixed clock.
fn seed_demo(fixture: &Fixture, home: &Path, workspace: &Path) -> Result<()> {
    pin_operator(&gui_pass::config_file(home))?;
    seed_archive(fixture, home)?;
    seed_places(fixture, home)?;
    seed_persons(fixture, home)?;
    seed_families(fixture, home)?;
    seed_events(fixture, home)?;
    let stamped = pin_clock(&database_file(workspace)?)?;
    // The projections carry the assertion timestamps the UI renders, so they are rebuilt from the
    // restamped log rather than left holding the wall-clock ones (ADR 0010).
    gui_pass::cli(fixture, home, &["rebuild"])?;
    println!("screenshots: seeded the demo workspace and pinned {stamped} assertions to {CLOCK}");
    Ok(())
}

/// Overwrites the configured operator's display name.
///
/// A line edit rather than a parse-and-reserialize: the file was written by `init` seconds earlier,
/// and rewriting the whole document to change one field risks reordering it into something the TOML
/// serializer rejects (a value after a table).
fn pin_operator(config: &Path) -> Result<()> {
    let text = fs::read_to_string(config).with_context(|| format!("reading {}", config.display()))?;
    let pinned = with_operator_display(&text, OPERATOR)?;
    fs::write(config, pinned).with_context(|| format!("writing {}", config.display()))
}

/// `config` with the `[operator]` section's `display` set to `name`, inserting the key when the
/// section carries none.
fn with_operator_display(config: &str, name: &str) -> Result<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut in_operator = false;
    let mut written = false;
    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_operator && !written {
                lines.push(format!("display = \"{name}\""));
                written = true;
            }
            in_operator = trimmed == "[operator]";
        }
        if in_operator && trimmed.starts_with("display") {
            lines.push(format!("display = \"{name}\""));
            written = true;
            continue;
        }
        lines.push(line.to_owned());
    }
    if in_operator && !written {
        lines.push(format!("display = \"{name}\""));
        written = true;
    }
    if !written {
        bail!("the config has no [operator] section to pin a display name in");
    }
    let mut text = lines.join("\n");
    text.push('\n');
    Ok(text)
}

/// The workspace's SQLite database file, read from its manifest rather than assumed.
fn database_file(workspace: &Path) -> Result<PathBuf> {
    let manifest = workspace.join("workspace.toml");
    let text = fs::read_to_string(&manifest).with_context(|| format!("reading {}", manifest.display()))?;
    let parsed: toml::Table = toml::from_str(&text).with_context(|| format!("parsing {}", manifest.display()))?;
    let url = parsed
        .get("database_url")
        .and_then(toml::Value::as_str)
        .with_context(|| format!("{} declares no database_url", manifest.display()))?;
    let Some(path) = url.strip_prefix("sqlite://") else {
        bail!("screenshots: the demo fixture must be SQLite-backed, not {url}");
    };
    let path = Path::new(path);
    Ok(if path.is_absolute() {
        path.to_owned()
    } else {
        workspace.join(path)
    })
}

/// Restamps every stored assertion to a fixed instant, returning how many were rewritten.
///
/// Ordered by `rowid` — insertion order, which is the order the seed script asserted things in.
/// Aggregate ids are UUID v7 minted per run, so ordering by them would hand each run a different
/// assignment of timestamps to events.
fn pin_clock(database: &Path) -> Result<usize> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting a runtime for the event-log restamp")?;
    runtime.block_on(restamp_log(database))
}

/// The `pin_clock` body: read every payload, restamp it, write it back.
async fn restamp_log(database: &Path) -> Result<usize> {
    let options = SqliteConnectOptions::new().filename(database);
    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .with_context(|| format!("opening {}", database.display()))?;
    let rows = sqlx::query("SELECT rowid, payload FROM events ORDER BY rowid")
        .fetch_all(&pool)
        .await
        .context("reading the event log")?;
    let mut stamped = 0;
    for (index, row) in rows.iter().enumerate() {
        let id: i64 = row.try_get("rowid").context("reading a row id")?;
        let payload: String = row.try_get("payload").context("reading an event payload")?;
        let payload = restamp(&payload, &stamp(index)?)?;
        sqlx::query("UPDATE events SET payload = ?1 WHERE rowid = ?2")
            .bind(payload)
            .bind(id)
            .execute(&pool)
            .await
            .context("writing a restamped event payload")?;
        stamped += 1;
    }
    pool.close().await;
    Ok(stamped)
}

/// The instant the `index`-th assertion is stamped with.
fn stamp(index: usize) -> Result<String> {
    let offset = CLOCK_STEP * i32::try_from(index).context("more assertions than a demo seed should hold")?;
    (CLOCK + offset)
        .format(&Rfc3339)
        .context("formatting a pinned assertion timestamp")
}

/// `payload` with its provenance envelope's `occurred_at` replaced by `at`.
///
/// Every stored event is an `Envelope { assertion_id, context, ..body }` (ADR 0004 §1), so one path
/// covers all 13 aggregates; a payload without that envelope is an error rather than a silent skip.
fn restamp(payload: &str, at: &str) -> Result<String> {
    if Timestamp::parse_rfc3339(at).is_none() {
        bail!("{at:?} is not an RFC 3339 timestamp the event log could be read back with");
    }
    let mut event: serde_json::Value = serde_json::from_str(payload).context("parsing a stored event payload")?;
    let context = event
        .get_mut("context")
        .and_then(serde_json::Value::as_object_mut)
        .context("a stored event carries no provenance context")?;
    if !context.contains_key("occurred_at") {
        bail!("a stored event's provenance context carries no occurred_at");
    }
    context.insert("occurred_at".to_owned(), serde_json::Value::String(at.to_owned()));
    serde_json::to_string(&event).context("re-encoding a restamped event payload")
}

/// The repository, the source it holds, and the citations drawn from it.
///
/// Invented, like every record here: no personal genealogy belongs in the repository. The citations
/// differ in surety, and one carries the *Evidence Explained* axes, so the detail pane's evidence
/// chips have something to show.
fn seed_archive(fixture: &Fixture, home: &Path) -> Result<()> {
    let cli = |args: &[&str]| gui_pass::cli(fixture, home, args);
    cli(&["repository", "create", "--id", "R0001", "--name", "Vestheim lokalarkiv"])?;
    cli(&["repository", "set-type", "R0001", "--type", "archive"])?;
    cli(&[
        "source",
        "create",
        "--id",
        "S0001",
        "--title",
        "Vestheim sokneprestembete, ministerialbok 1841–1878",
    ])?;
    cli(&["source", "set-author", "S0001", "Vestheim sokneprestembete"])?;
    cli(&[
        "source",
        "link-repository",
        "S0001",
        "--repository",
        "R0001",
        "--call-number",
        "Mf. 12/3",
        "--media-type",
        "fiche",
    ])?;
    for (id, page, confidence) in CITATIONS {
        cli(&["citation", "create", "--id", id, "--source", "S0001", "--page", page])?;
        cli(&["citation", "set-confidence", id, "--confidence", confidence])?;
    }
    cli(&[
        "citation",
        "set-evidence-analysis",
        "C0001",
        "--source",
        "original",
        "--information",
        "primary",
        "--evidence",
        "direct",
    ])?;
    Ok(())
}

/// The places the events happened in, clustered in southern Norway so the map's *Fit* frames a
/// legible region rather than a continent.
fn seed_places(fixture: &Fixture, home: &Path) -> Result<()> {
    for (id, kind, name, latitude, longitude) in PLACES {
        gui_pass::cli(
            fixture,
            home,
            &["place", "create", "--id", id, "--type", kind, "--name", name],
        )?;
        gui_pass::cli(
            fixture,
            home,
            &["place", "set-coordinates", id, "--lat", latitude, "--long", longitude],
        )?;
    }
    Ok(())
}

/// The three generations, each named by a separate assertion so the name can carry (or pointedly not
/// carry) a backing citation — an uncited name is what puts the `No source` flag and the `No judgment`
/// confidence on screen, which is the evidence-first thesis made visible.
fn seed_persons(fixture: &Fixture, home: &Path) -> Result<()> {
    for person in PERSONS {
        gui_pass::cli(fixture, home, &["person", "create", "--id", person.id])?;
        let mut args = vec![
            "person",
            "add-name",
            person.id,
            "--given",
            person.given,
            "--surname",
            person.surname,
            "--rationale",
            person.rationale,
        ];
        if let Some((citation, confidence)) = person.name_evidence {
            args.extend_from_slice(&["--citation", citation, "--confidence", confidence]);
        }
        gui_pass::cli(fixture, home, &args)?;
    }
    Ok(())
}

/// The two families, partners then children.
fn seed_families(fixture: &Fixture, home: &Path) -> Result<()> {
    for family in FAMILIES {
        let created = gui_pass::cli(fixture, home, &["family", "create"])?;
        let id = created
            .split_whitespace()
            .next_back()
            .with_context(|| format!("no family id in {created:?}"))?
            .to_owned();
        for partner in family.partners {
            gui_pass::cli(fixture, home, &["family", "add-partner", &id, partner])?;
        }
        for child in family.children {
            gui_pass::cli(fixture, home, &["family", "add-child", &id, child])?;
        }
    }
    Ok(())
}

/// The dated, placed events and who took part in them. Two are deliberately uncited.
fn seed_events(fixture: &Fixture, home: &Path) -> Result<()> {
    for event in EVENTS {
        let cli = |args: &[&str]| gui_pass::cli(fixture, home, args);
        cli(&["event", "create", "--id", event.id, "--type", event.kind])?;
        cli(&[
            "event",
            "assert-date",
            event.id,
            "--year",
            event.year,
            "--month",
            event.month,
            "--day",
            event.day,
        ])?;
        cli(&["event", "link-place", event.id, event.place])?;
        if let Some(citation) = event.citation {
            cli(&["event", "add-citation", event.id, "--citation", citation])?;
        }
        for (person, role) in event.participants {
            cli(&[
                "person",
                "add-participation",
                person,
                "--event",
                event.id,
                "--role",
                role,
            ])?;
        }
    }
    Ok(())
}

/// One demo person: the pinned human id, the name asserted for them, why, and the citation and surety
/// backing that name (`None` leaves it unsourced on purpose).
struct DemoPerson {
    id: &'static str,
    given: &'static str,
    surname: &'static str,
    rationale: &'static str,
    name_evidence: Option<(&'static str, &'static str)>,
}

/// One demo event: the pinned human id, its type, its date and place, its backing citation (`None`
/// leaves it unsourced), and who took part.
struct DemoEvent {
    id: &'static str,
    kind: &'static str,
    year: &'static str,
    month: &'static str,
    day: &'static str,
    place: &'static str,
    citation: Option<&'static str>,
    participants: &'static [(&'static str, &'static str)],
}

/// One demo family: its partners and its children, by person human id.
struct DemoFamily {
    partners: &'static [&'static str],
    children: &'static [&'static str],
}

/// The citations, by human id, page and surety.
const CITATIONS: [(&str, &str, &str); 4] = [
    ("C0001", "fol. 42b, no. 7", "high"),
    ("C0002", "fol. 91a, no. 3", "normal"),
    ("C0003", "fol. 118b, no. 12", "very-high"),
    ("C0004", "s. 204, no. 41", "low"),
];

/// The places, by human id, type, name and coordinates.
const PLACES: [(&str, &str, &str, &str, &str); 4] = [
    ("P0001", "farm", "Vestheim", "58.1600", "7.8600"),
    ("P0002", "municipality", "Kristiansand", "58.1467", "7.9956"),
    ("P0003", "parish", "Søgne", "58.0906", "7.7855"),
    ("P0004", "town", "Mandal", "58.0294", "7.4534"),
];

/// The seven persons, oldest generation first.
const PERSONS: [DemoPerson; 7] = [
    DemoPerson {
        id: "I0001",
        given: "Anders",
        surname: "Olsen Vestheim",
        rationale: "Baptismal entry, named with the farm he was born on",
        name_evidence: Some(("C0001", "high")),
    },
    DemoPerson {
        id: "I0002",
        given: "Ingeborg Marie",
        surname: "Larsdotter",
        rationale: "Baptismal entry",
        name_evidence: Some(("C0002", "normal")),
    },
    DemoPerson {
        id: "I0003",
        given: "Ola",
        surname: "Andersen Vestheim",
        rationale: "Baptismal entry, patronymic from his father",
        name_evidence: Some(("C0003", "very-high")),
    },
    DemoPerson {
        id: "I0004",
        given: "Karen Sofie",
        surname: "Nilsdotter",
        rationale: "Marriage entry; no baptismal record found yet",
        name_evidence: Some(("C0004", "low")),
    },
    DemoPerson {
        id: "I0005",
        given: "Marit",
        surname: "Olsdotter Vestheim",
        rationale: "Baptismal entry",
        name_evidence: Some(("C0003", "high")),
    },
    DemoPerson {
        id: "I0006",
        given: "Nils",
        surname: "Olsen Vestheim",
        rationale: "Family recollection only — no record consulted",
        name_evidence: None,
    },
    DemoPerson {
        id: "I0007",
        given: "Johanne",
        surname: "Olsdotter Vestheim",
        rationale: "Family recollection only — no record consulted",
        name_evidence: None,
    },
];

/// The two families.
const FAMILIES: [DemoFamily; 2] = [
    DemoFamily {
        partners: &["I0001", "I0002"],
        children: &["I0003"],
    },
    DemoFamily {
        partners: &["I0003", "I0004"],
        children: &["I0005", "I0006", "I0007"],
    },
];

/// The events, in the order they happened.
const EVENTS: [DemoEvent; 11] = [
    DemoEvent {
        id: "E0001",
        kind: "birth",
        year: "1841",
        month: "6",
        day: "12",
        place: "P0001",
        citation: Some("C0001"),
        participants: &[("I0001", "primary")],
    },
    DemoEvent {
        id: "E0002",
        kind: "birth",
        year: "1845",
        month: "2",
        day: "28",
        place: "P0003",
        citation: Some("C0002"),
        participants: &[("I0002", "primary")],
    },
    DemoEvent {
        id: "E0003",
        kind: "marriage",
        year: "1869",
        month: "10",
        day: "3",
        place: "P0003",
        citation: Some("C0002"),
        participants: &[("I0001", "groom"), ("I0002", "bride")],
    },
    DemoEvent {
        id: "E0004",
        kind: "birth",
        year: "1871",
        month: "4",
        day: "17",
        place: "P0001",
        citation: Some("C0003"),
        participants: &[("I0003", "primary"), ("I0001", "father"), ("I0002", "mother")],
    },
    DemoEvent {
        id: "E0005",
        kind: "birth",
        year: "1874",
        month: "11",
        day: "5",
        place: "P0002",
        citation: Some("C0004"),
        participants: &[("I0004", "primary")],
    },
    DemoEvent {
        id: "E0006",
        kind: "marriage",
        year: "1897",
        month: "5",
        day: "22",
        place: "P0002",
        citation: Some("C0004"),
        participants: &[("I0003", "groom"), ("I0004", "bride")],
    },
    DemoEvent {
        id: "E0007",
        kind: "birth",
        year: "1899",
        month: "8",
        day: "14",
        place: "P0002",
        citation: Some("C0003"),
        participants: &[("I0005", "primary"), ("I0003", "father"), ("I0004", "mother")],
    },
    DemoEvent {
        id: "E0008",
        kind: "birth",
        year: "1902",
        month: "1",
        day: "30",
        place: "P0002",
        citation: None,
        participants: &[("I0006", "primary")],
    },
    DemoEvent {
        id: "E0009",
        kind: "birth",
        year: "1905",
        month: "9",
        day: "9",
        place: "P0004",
        citation: None,
        participants: &[("I0007", "primary")],
    },
    DemoEvent {
        id: "E0010",
        kind: "death",
        year: "1912",
        month: "3",
        day: "2",
        place: "P0002",
        citation: Some("C0001"),
        participants: &[("I0001", "primary")],
    },
    DemoEvent {
        id: "E0011",
        kind: "death",
        year: "1948",
        month: "7",
        day: "19",
        place: "P0002",
        citation: None,
        participants: &[("I0003", "primary")],
    },
];

#[cfg(test)]
mod tests {
    use super::{restamp, stamp, with_operator_display};

    #[test]
    fn a_payload_keeps_everything_but_its_stamp() {
        let payload = r#"{"assertion_id":"0198","context":{"operator":{"kind":"Human"},
            "occurred_at":"2026-08-14T13:45:19.123456Z","citations":[]},"type":"PersonCreated"}"#;
        let restamped = restamp(payload, "2026-03-14T09:00:00Z").expect("the payload restamps");
        assert!(
            restamped.contains(r#""occurred_at":"2026-03-14T09:00:00Z""#),
            "the pinned instant replaces the wall-clock one: {restamped}"
        );
        assert!(
            restamped.contains(r#""type":"PersonCreated""#),
            "the claim itself is untouched"
        );
        assert!(
            restamped.contains(r#""assertion_id":"0198""#),
            "the assertion keeps its identity"
        );
    }

    #[test]
    fn a_payload_without_a_provenance_envelope_is_an_error() {
        let restamped = restamp(r#"{"type":"PersonCreated"}"#, "2026-03-14T09:00:00Z");
        assert!(restamped.is_err(), "an event with no context must fail loudly");
    }

    #[test]
    fn an_unreadable_instant_is_rejected_before_it_reaches_the_log() {
        // Writing this would leave an event the application can no longer decode.
        let restamped = restamp(r#"{"context":{"occurred_at":"2026-08-14T13:45:19Z"}}"#, "14 March 2026");
        assert!(restamped.is_err(), "only RFC 3339 may be written back");
    }

    #[test]
    fn successive_assertions_are_stamped_a_step_apart() {
        assert_eq!(stamp(0).expect("the first stamp"), "2026-03-14T09:00:00Z");
        assert_eq!(stamp(1).expect("the second stamp"), "2026-03-14T09:01:00Z");
        assert_eq!(
            stamp(120).expect("a later stamp"),
            "2026-03-14T11:00:00Z",
            "the offset rolls over the hour rather than saturating"
        );
    }

    #[test]
    fn the_operator_display_is_replaced_in_place() {
        let config =
            "default = \"demo\"\n\n[operator]\nid = \"0198\"\ndisplay = \"magne\"\n\n[defaults]\nengine = \"sqlite\"\n";
        let pinned = with_operator_display(config, "Vitni demo").expect("the config pins");
        assert!(pinned.contains("display = \"Vitni demo\""), "the name is replaced");
        assert!(!pinned.contains("\"magne\""), "the OS user's name is gone: {pinned}");
        assert!(pinned.contains("[defaults]"), "the rest of the config survives");
    }

    #[test]
    fn a_display_less_operator_section_gains_one() {
        let config = "[operator]\nid = \"0198\"\n\n[defaults]\nengine = \"sqlite\"\n";
        let pinned = with_operator_display(config, "Vitni demo").expect("the config pins");
        let display = pinned.find("display = \"Vitni demo\"").expect("the name is inserted");
        let defaults = pinned.find("[defaults]").expect("the next section survives");
        assert!(display < defaults, "the key lands inside [operator]: {pinned}");
    }

    #[test]
    fn a_config_without_an_operator_is_an_error() {
        let pinned = with_operator_display("default = \"demo\"\n", "Vitni demo");
        assert!(pinned.is_err(), "a config with no [operator] cannot be pinned silently");
    }
}
