# Development

Platform setup, the test layers, and the repository's own tooling. [`README.md`](../README.md) has
the quickstart; this is the detail it links out to.

## Prerequisites

- **Rust** — latest stable via [`rustup`](https://rustup.rs). The `wasm32-wasip2` target is declared
  in `rust-toolchain.toml` and installs automatically.
- **Desktop GUI only** — a system webview, below. The CLI, the plugin host and the whole test suite
  need none of it.

### Desktop GUI system dependencies

`vitni-ui-dioxus` renders through a system webview (Dioxus desktop → `wry`/`tao`). The webview
sits behind a non-default **`desktop`** feature, so building the rest of the workspace and running
the tests needs no system libraries — only running the GUI does.

**Linux** — WebKitGTK and GTK development packages:

```bash
# Debian / Ubuntu (24.04+)
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev \
  libayatana-appindicator3-dev librsvg2-dev

# Fedora
sudo dnf install -y webkit2gtk4.1-devel gtk3-devel libxdo-devel \
  libappindicator-gtk3-devel librsvg2-devel

# Arch
sudo pacman -S --needed webkit2gtk-4.1 gtk3 xdotool libayatana-appindicator librsvg
```

**Windows** — WebView2. The runtime ships with Windows 11 and current Windows 10; on older systems
install the
[Evergreen WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/). Building
needs no extra step (it links the system WebView2 loader).

**macOS** — the built-in WebKit; nothing to install.

## Everyday commands

```bash
cargo build --workspace                                              # every crate
cargo run -p vitni                                               # the `vitni` launcher: the GUI
cargo run -p vitni -- person list                                # …and the CLI, with arguments
cargo run -p vitni-cli                                           # the CLI alone, no webview linked
cargo nextest run --workspace --all-features --lib --bins --tests    # tests
cargo test -p vitni-core <name>                                  # one test in one crate
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo xtask fmt                                                      # rustfmt the workspace + plugins/* (--check)
cargo deny --all-features check                                      # advisories, licences, bans
cargo xtask check                                                    # every static check, in one pass
cargo xtask build-plugins                                            # plugins/* → target/plugins
cargo xtask icons                                                    # SVG icon sources → installed PNGs
cargo xtask backup-fixture                                           # regenerate the golden backup fixture
cargo xtask match-eval                                               # score the matching evaluation corpus
prek run                                                             # the git hooks, by hand
```

**Always pass `--workspace`** (or `-p`, or `--all` for `fmt`). `default-members` is
`crates/vitni-cli`, so a bare `cargo test` or `cargo clippy` silently covers that one crate and
skips everything else, including `xtask`.

`--lib --bins --tests` deliberately excludes `benches/`: the `vitni-db` benchmarks take about
140 s each. Clippy still lints them through `--all-targets`. Run them deliberately:

```bash
cargo bench -p vitni-db --features sqlite
cargo bench -p vitni-app --bench similar    # record matching at 10k and 100k persons
```

The matching bench seeds a 100k-person workspace of several gigabytes under `target/`, so it keeps it
out of `/tmp`, which may be RAM-backed.

`cargo xtask` also exposes the individual checks (`i18n-check`, `css-check`, `input-guard`,
`licence-check`, `icons --check`, `backup-guard`, `fixture-guard`, `match-eval`) plus `issue-sync`, `labels`,
`package` (the Linux release tarball) and `screenshots` (the README images, below).

## The app icon and the brand art

`crates/vitni-ui-dioxus/assets/icon/` holds five SVG sources and the PNGs generated from them;
`assets/brand/` holds the two lockups and theirs. The sources are the design; the PNGs are committed
because a `.deb` and an AppImage install files rather than render vectors, and GitHub renders an
`<img>`. Edit an SVG, run `cargo xtask icons`, and commit both.

The mark is a **V with the weight a broad nib gives it** — heavy descending stroke, light ascending
one, three nodes — so the monogram is also a two-generation pedigree fragment. It stands in **three
ruled lines**: the record the conclusion is derived from. The heavy upper terminal is a **seal**, and
both it and the ruled lines are disclosed by size:

| Size | Source | The seal | The record |
| --- | --- | --- | --- |
| 256, 128 | `vitni.svg` | impressed ring + a chevron | three ruled lines |
| 64 | `vitni-notch.svg` | one cut flat on the rim | — |
| 48 | `vitni-plain.svg` | an ordinary node | — |
| 32, 24, 16 | `vitni-small.svg` | an ordinary node, mark 1.15× | — |
| any | `vitni-symbolic.svg` | never — monochrome, plate-less, for GNOME and the tray | — |

Palette, flat: plate `#142132`, mark `#e0a92e`, ruled lines `#8a6a26`. The gold matters — the mark
used to be the app's own `--accent`, which made the icon read as a chip from its own toolbar.

The impression is **subtracted** from the terminal through a mask, never added to it, so the
silhouette is the plain V's at every size and at 16 px the icon is indistinguishable from it. The seal
sits on the *heavy* stroke, which makes it exactly as wide as the stroke it caps, so it cannot read as
a notification badge.

`assets/brand/vitni-wordmark.svg` is the mark beside the name for `README.md`, and
`vitni-splash.svg` is 1280×640 — GitHub's social-preview size, and a ground for an About dialog or
splash when the app grows one. **Their letters are geometry, not text**: `resvg` is built without text
shaping, and a wordmark that depended on system fonts would render differently on every machine. Each
capital therefore takes the same nib logic as the mark; the layout parameters are in the wordmark's
header comment, and both files carry the lockup because SVG has no include.

`cargo xtask icons --check` (part of `cargo xtask check`) re-reads every committed raster: decodable,
the right size, and not fully transparent — which is what the previous stub was.

## Testing the GUI

Two layers, and they catch different things.

**SSR tests** (`crates/vitni-ui-dioxus/tests/*.rs`) render components to markup and assert over
it. Fast, and they cover view logic. They cannot see anything that only exists in a live webview:
`document::eval`, CSS, the map canvas, *which element a handler is attached to*, or *where focus
actually lands*.

**`cargo xtask gui-pass`** covers that layer. It runs the real GUI on its own **Xvfb** display,
drives it with `xdotool`, and asserts over screenshots — so it needs `xvfb`, `xdotool` and
`imagemagick`, but no desktop session. It is also *more* reliable than driving the GUI on a real
desktop, where the compositor hands synthetic input to whatever it thinks is focused rather than to
the window you aimed at.

```bash
cargo xtask gui-pass                     # every scenario, in parallel (1 worker per 4 cores, ≤4)
cargo xtask gui-pass --jobs 1            # one at a time, progress printed live
cargo xtask gui-pass map-canvas          # one, by name
cargo xtask gui-pass --reset             # wipe the fixture workspace, isolated home and old shots
cargo xtask gui-pass --keep              # leave it running; attach with `x11vnc -display :99`
```

Scenarios are **TOML, not Rust** — `crates/vitni-ui-dioxus/tests/gui-pass/*.toml` — so adding one
needs no rebuild. Each lists `[[step]]`s (`shot`, `click`, `key`, `text`, `drag`, `wheel`, `wait`,
`await-exit`) and `[[assert]]`s over the shots by name. Runs are isolated by default: a throwaway
`XDG_CONFIG_HOME`/`XDG_DATA_HOME` and a freshly seeded fixture workspace under `target/gui-pass/`,
because a scripted click run writes real events. Parallel workers each get their own display, home and
workspace (`target/gui-pass/workers/<n>/`), restored from the same seed. Shots land in
`target/gui-pass/shots/<scenario>/` and the GUI's own log in `gui.log` beside them.

When a screenshot disagrees with your reading of it, column-scan instead of squinting:

```bash
convert <shot> -crop 1xH+X+Y +repage txt:-     # exact pixel rows
convert <in> -crop WxH+X+Y +repage <out>       # crop a region to inspect
```

Some things remain human-only, and the `manual-verify` label in
[`issue-tracking.md`](issue-tracking.md) reserves them: pan and zoom smoothness, click latency,
motion. Software GL is not a GPU, and a still image has no frame rate.

## The README screenshots

`cargo xtask screenshots` regenerates the images in [`assets/`](assets/) that `README.md` shows. It is
the same harness as `gui-pass` over a second fixture, so it wants the same `xvfb`, `xdotool` and
`imagemagick`:

```bash
cargo xtask screenshots                  # reseed, drive the GUI, rewrite docs/assets/*.png
cargo xtask screenshots --keep           # leave it running; attach with `x11vnc -display :99`
```

The scenario is `crates/vitni-ui-dioxus/tests/screenshots/readme.toml` and takes one `shot` per image;
`IMAGES` in `xtask/src/screenshots.rs` maps each shot name to its committed PNG and the width it is
scaled to. There are no `[[assert]]`s in it — it exists to produce pixels.

**Two runs must produce no diff**, or the command is not a refresh path. Three things would otherwise
make each run differ, and the command pins all three: the **clock** (every assertion carries the
instant it was made, and the Dashboard's activity feed, the History tab and the *Why we believe*
popover all render it, so the seeded event log is restamped to fixed instants and the projections
rebuilt from it), the **operator** (`init` names it after the OS user), and the **locale**
(`VITNI_LANGUAGE=en`). Aggregate ids stay random per run — they are UUIDs no screen renders — and human
ids are pinned by the seed script.

The demo workspace is **invented data**, seeded from scratch on every run: seven people over three
generations, two families, eleven dated and placed events, and one archive → source → citations chain
whose surety deliberately varies. No personal genealogy belongs in the repository, and the fixture is
isolated (`target/screenshots/`, a throwaway `XDG_CONFIG_HOME`) so a run cannot reach real data.

## Backups and the backup fixtures

The backup archive is the project's one compatibility surface (ADR 0041). `vitni backup create
[--with-media] <file>` writes the event log, a portable `workspace.toml` and the media manifest into a
`.vitni-backup` zip; `vitni backup restore <file> --new NAME PATH [--database-url URL]` restores it
into a new workspace on either engine. The GUI has the same pair in Preferences → *Backup & restore*.

- **The format** is `0.N` before 1.0 (`crates/vitni-app/src/backup/format.rs`). Bump it whenever an
  event would no longer decode from an older archive, or the archive layout changes: append a record
  to `backup/upgrade/pre_release.rs`, give the previous one its `vN → vN+1` upgrader (pure functions
  over `serde_json::Value`), and add the new fixture. A restore reads the current format and the two
  before it.
- **The fixtures** under `crates/vitni-app/tests/fixtures/backup/v<format>/` are invented data. Each is
  an archive plus the projection digest it must restore to. `cargo xtask backup-fixture` rewrites the
  current format's archive from `xtask/src/backup_fixture/` (one module per aggregate, fixed ids and
  instants, so two runs produce no diff) and refreshes every fixture's digest; never rewrite an older
  archive. A digest diff means the projections changed: check that was intended.
- **A new event variant** fails `the_current_fixture_holds_every_event_variant_of_every_aggregate` until
  its aggregate's fixture module pushes it and the fixture is regenerated.
- **The cross-engine test** (`crates/vitni-app/tests/backup_postgres.rs`) needs Docker, like
  `vitni-db`'s Postgres tests: `cargo test -p vitni-app --features postgres --test backup_postgres`.

## Name-culture packs

Record matching (ADR 0038) takes its name rules from TOML packs in
`crates/vitni-core/matching/cultures/<id>.toml`, embedded at build time, and picks them per comparison
through `crates/vitni-core/matching/regions.toml` (country and period → packs) and the packs'
`languages`. The schema is `CulturePack`'s in `crates/vitni-core/src/matching/pack.rs`; unknown keys are
rejected and a pack's `id` must equal its file name.

- **Adding a culture** is a new pack file, an entry in `EMBEDDED` in `pack.rs`, a region row if a
  country selects it, and table cases in `crates/vitni-core/src/matching/tests.rs` showing the pair it
  exists for, plus [evaluation pairs](#the-matching-evaluation-corpus) from that culture's records. No
  comparator changes.
- **Rewrites** see lower-cased text with diacritics already stripped (`å` is `a` by then) and apply in
  order, `universal` first; doubled letters collapse after them.
- **A workspace override** is the same file under `<workspace>/matching/cultures/` (or
  `~/.local/share/vitni/matching/cultures/`): same name replaces, new name adds.
- **A change to a rule, class or weight** changes scores, so bump `ENGINE_VERSION` in
  `crates/vitni-core/src/matching/mod.rs` and run `cargo xtask match-eval`.
- **A patronymic** is read with the pack's `male_suffixes`/`female_suffixes`: the stem left once the
  suffix is removed must equal the father's given name (or a name in its class) after both drop a
  genitive `s` and one final vowel (`Olsdatter` ↔ `Ole`, `Andreassen` ↔ `Andreas`). A culture whose
  patronymics are formed otherwise needs a comparator change, not only a pack. It is read only where
  the two surnames do not already agree (the farm name that changed after a move): where they agree,
  the check would read the other record's own surname against its own father and count one fact twice.
- **A family** is scored through its partners: each pair is a person assessment under the families'
  shared cultures, summarised as one `Partner` term and kept whole in `MatchAssessment::parts`. A
  partner's support is capped (`PARTNER_SUPPORT`) because a remarriage shares a partner; only both
  agreeing make one family. **An event** pairs its principals (primary, husband, wife, spouse, groom,
  bride) one to one, never across an asserted sex; its other participants only ever support a pair.
- **A place** as a record of its own is compared by names, type, where it lies and coordinates; one
  place enclosing the other (a farm and its parish) is `Partial` on `Enclosure`, never a disagreement.
  **Titles, repository names, addresses and notes** are free text, scored by `Applied::text_similarity`:
  the share of both texts' letters in shared words, so word order does not matter; numbers must be
  equal and words under three letters (*i*, *på*) are skipped. **A citation** summarises its source pair
  as one `Source` term (capped, `SOURCE_SUPPORT`) with the source assessment in `parts`, and compares
  its page by its numbers. **Media** match on the checksum exactly, with the file name as weak support;
  the aggregate has no description to compare. **A tag** pair with one case-folded name is
  `Deterministic` — the only band a compared value sets.

## The matching index

`find_similar`, `assess` and `similar_pairs` (`crates/vitni-app/src/similar.rs`) are the only matching
entry points (ADR 0038 §8); a new consumer calls them rather than `assess_*` over profiles it builds
itself. Candidates come from the `match_keys` index (ADR 0038 §7): the keys are computed in
`crates/vitni-core/src/matching/keys.rs`, stored by `crates/vitni-db/src/match_keys/`, and kept current
by `similar.rs` before each lookup.

- **Keys are loose on purpose.** A person is keyed by each given-name and surname token — normalized
  under every pack, by phonetic key and every phonetic key one letter shorter, and by the classes of
  every installed pack — each qualified by the birth decade (`t:ole@185`, or `@?` when undated). A
  `Probe` meets the neighbouring decades and the unknown one. Other kinds key their names, titles,
  checksum or folded tag name, and every kind its record origins and external ids. The proptest
  `every_pair_the_engine_shows_meets` and the app test `blocking_loses_no_pair_a_score_of_every_pair_would_show`
  hold blocking to "no pair the engine would show is lost"; a key change that fails them loses recall.
- **A change to how keys are made bumps `KEYS_VERSION`** in `keys.rs`. It is part of the fingerprint
  stored with the index, so every workspace rebuilds its keys on next use; so does any change to the
  installed packs, and `vitni rebuild`, which clears the fingerprint.
- **Between rebuilds, commits mark records dirty** (`match_dirty`, fed on every commit of a matchable
  aggregate). A lookup rekeys them together with the records whose keys carry theirs — an event's
  principals, a person's events and families, a place's events, a source's citations
  (`similar::affected`). A new key that reads another record's data needs its dependency added there.
- **`[matching]`** in `workspace.toml`, or `[workspace-defaults.matching]` in the global config, sets
  `default_cultures` (pack ids beside `universal`) and the `probable`/`possible` thresholds as whole
  percentages, which a pair must score above (a pair whose evidence nets to nothing sits exactly at the
  default 50 and is not shown); an unset field falls back field by field to the engine's defaults. A bad value, or a
  pack that does not parse, fails the lookup that needs it, never the workspace open.

## The matching evaluation corpus

`cargo xtask match-eval` (`xtask/src/match_eval/`) measures the engine against labelled person pairs in
`crates/vitni-core/matching/corpus/*.toml` (ADR 0038 §9): invented pairs in `invented.toml`, and
transcriptions of public Norwegian census and church records over 100 years old in
`digitalarkivet.toml`. It runs each pair through blocking (`BlockingKeys::person`, as `match_keys`
would) and `assess_persons` with the default settings, then prints, per band, the true and false
matches there, the band's precision and the recall at or above it. Every pair on the wrong side of
`possible` is listed as *misjudged*: a false match the user would be shown, or a true match they would
not.

- **The gate is recall on the hard cases.** A `same` pair may name its `hard` case —
  `spelling-variant`, `surname-after-move`, `census-age` or `baptism-for-birth`. Each must be a
  candidate and score `possible` or better, and each case needs at least one pair. `match-eval` is part
  of `cargo xtask check`, so CI fails when a comparator, weight or pack change loses one. Precision is
  reported, not gated.
- **A pair is two records and a label.** Each record takes `given`, `surname`, `sex`, `occupations`,
  `birth`/`baptism`/`death`/`burial` (`year`, optional `month`/`day`, `country`, `place`, and
  `basis = "age"` for a birth year computed from a census age), and `parents`/`partners`/`children`
  (`given`, `surname`, `sex`, `born`). Unknown keys are rejected, ids are unique across files, and
  `source` is required: `invented`, or the records' Digitalarkivet references and the evidence that
  they are one person.
- **Only mark a pair `hard` if the engine surfaces it.** A true match the engine misses today goes in
  without `hard`, so it is reported as misjudged; add `hard` in the change that fixes it.

## Test fixtures and their provenance

A fixture is committed only if the project may redistribute it ([ADR 0042](adr/0042-test-fixture-provenance.md)).
That covers every `tests/fixtures/` tree and `crates/vitni-core/matching/corpus/`, and it gives a
fixture one of four origins:

- **`invented`**: data made up for the test.
- **`generated`**: written by a named xtask, such as `cargo xtask backup-fixture`.
- **`licensed`**: under a licence that permits redistribution, with the licence and attribution
  recorded.
- **`transcribed-facts`**: facts from public records, with each record's source URL, and no copied
  pages.

A page from Digitalarkivet or any other archive is none of these, so it goes in the external tier.

The origin is declared in a `PROVENANCE.toml` in the fixture's directory or an ancestor, and the
nearest one wins. `origin` is always required, plus what that origin needs:

```toml
origin = "generated"
generator = "cargo xtask backup-fixture"   # generated: the xtask that writes the files
# licensed:          licence = "…" and attribution = "…"
# transcribed-facts: sources = ["https://…"], or a statement that the files list them

[files."digitalarkivet.toml"]              # optional: one file in this directory with its own origin
origin = "transcribed-facts"
sources = "each pair's `source` lists its records' Digitalarkivet URLs"
```

`cargo xtask fixture-guard` (part of `cargo xtask check`, so prek and CI run it) fails on a tracked
fixture with no declaration, naming it; on a malformed declaration, including an unknown key, a
blank value, a non-https source or a `[files."…"]` table naming no tracked file beside it; and on a
tracked page from the external manifest.
The Digitalarkivet parser's own fixtures, under `crates/vitni-digitalarkivet/tests/fixtures/`, are
invented pages that reproduce only the DOM `src/html.rs` reads; `tests/fallbacks.rs` covers each
fallback rung those pages cannot reach.

### The external tier: checking the parser against the live site

`crates/vitni-digitalarkivet/tests/external/manifest.toml` lists the real Digitalarkivet pages the
parser is checked against. For each page it records the URL, the rights as its source states them
(always `redistributable = false`), and the facts it must parse to. Only those facts are committed,
never the pages:

```bash
cargo xtask fetch-fixtures                                    # into target/external-fixtures/digitalarkivet/
cargo nextest run -p vitni-digitalarkivet --run-ignored only  # every page against its expected facts
```

`fetch-fixtures` runs `curl` with an honest user agent and waits five seconds between pages, as the
archive's `robots.txt` asks. The test reports every mismatch at once, one line each
(`census-person: birth: expected "1886-07-08", parsed …`), and a page that was never fetched fails
naming the command above.

It is a manual check before a release, not a CI job: polling a public archive from CI is impolite, and
would make CI depend on the archive's uptime. When it fails, decide which side moved. If the site's
markup changed, fix `src/html.rs`, then update the invented pages under `tests/fixtures/` to the new
shape so the bundled tests pin it. If the record's transcription was corrected, update the expected
fact. The manifest's own shape, and the checking logic, are tested on every run over the invented
pages.

### Takedown: removing a file from history

The pre-ADR captures stay in existing commits (ADR 0042 §5). If a rights holder asks for a file to be
removed, removing it from the tree is not enough: every commit that ever held it has to be rewritten.
The procedure below was run against this repository on 2026-09-30 (without the push).

1. **List the blobs to remove, by id.** Filter by content, not by path: the path of a removed file
   may hold a different, legitimate file today. The Digitalarkivet captures, for example, were
   replaced by invented pages at the same paths. A rename also leaves the old path in history (the
   captures also live under `crates/genealogy-digitalarkivet/`), so match every path the file ever
   had, then keep the blobs the tip still needs. Run it from an up-to-date checkout: the lists land
   there, and step 2 clones the mirror beside them (trash both afterwards).

   ```bash
   git log --all --format= --name-only --diff-filter=AR -M -- '*digitalarkivet/tests/fixtures/*' | sort -u
   git rev-list --objects --all | rg ' crates/(genealogy|vitni)-digitalarkivet/tests/fixtures/.+' \
     | cut -d' ' -f1 | git cat-file --batch-check='%(objecttype) %(objectname)' \
     | awk '$1 == "blob" {print $2}' | sort -u > all.txt
   git ls-tree -r origin/main -- crates/vitni-digitalarkivet/tests/fixtures | awk '{print $3}' | sort -u > keep.txt
   comm -23 all.txt keep.txt > strip.txt
   ```

   The `cat-file --batch-check` step keeps blobs only: `rev-list --objects` also lists the directories'
   tree objects. Read `strip.txt` against the history before going on (`git cat-file -p <id> | head`
   for each): it must hold only the files you mean to remove. For the captures it holds five.

2. **Rewrite a fresh mirror clone.** `git filter-repo` refuses a repository that is not freshly
   cloned; a clone from a local path needs `--no-local`. If it isn't installed, `uvx git-filter-repo`
   runs it.

   ```bash
   git clone --mirror git@github.com:magne/vitni.git vitni-mirror.git
   cd vitni-mirror.git
   uvx git-filter-repo --strip-blobs-with-ids ../strip.txt
   ```

3. **Check that nothing is left, and nothing else went.**

   ```bash
   while read -r id; do git cat-file -e "$id" 2>/dev/null && echo "still present: $id"; done < ../strip.txt
   while read -r id; do git cat-file -e "$id" || echo "lost: $id"; done < ../keep.txt
   ```

   Both loops should print nothing, and `git rev-list --all | wc -l` should give the same commit count
   as before. `git filter-repo` drops a commit only if it becomes empty, and the commits that added
   the captures all changed other files too.

4. **Force-push the branches and tags.** Before you push:
   - lift the branch protection on `main` in the repository settings, since it blocks force-pushes;
   - re-add the remote, because `git filter-repo` removes `origin` to prevent an accidental push.

   Don't use `--mirror`: the clone carries GitHub's read-only `refs/pull/*`, and a mirror push fails
   on them. Push the branches and tags instead, then restore the protection:

   ```bash
   git remote add origin git@github.com:magne/vitni.git
   git push --force --all origin
   git push --force --tags origin
   ```

5. **Ask GitHub Support to purge the old objects.** GitHub keeps the `refs/pull/*` refs and cached
   views of old commits, and those still serve the removed file until Support deletes them.
6. **Tell everyone with a clone to re-clone.** A clone made before the rewrite still holds the file,
   and pushing it back would restore it.

What it costs:

- Every SHA from the first commit that touched the file onward changes. For the captures that means
  everything from 2026-07-19.
- The GPG signatures on rewritten commits are lost.
- Links to old commit SHAs in issues, PRs and docs stop resolving.

## Repository conventions

The ones that will fail a review if missed:

- **Lints are guardrails, not suggestions.** `unwrap_used`, `panic`, `todo`, `unimplemented`,
  `exit`, `dbg_macro` and others are denied workspace-wide, and `allow_attributes` is denied too —
  so an `#[allow(…)]` is not the way out. Fix the code. `expect_used` warns; justify it if you use
  it. `print_stdout`/`print_stderr` are denied everywhere except `vitni-cli`, whose stdout *is*
  the interface.
- **Events are append-only.** Never edit projected state directly; emit an event, so the audit trail
  keeps its operator and reason. Event payloads are self-contained and versioned, and changes are
  additive, so every historical event stays decodable.
- **Every user-facing string is localized** through Fluent — no hardcoded literals, no framework
  i18n. `vitni-core` emits no user-facing strings at all: typed errors, and English `tracing`
  for developers. The CLI's per-language catalogue is *generated* from tracked fragments by
  `build.rs`; edit a fragment, never the concatenated file.
- **Every UI change updates [`mockups/`](mockups/) in the same change.** The mockups are the design
  source of truth and describe shipped behaviour, so a change the mockups still contradict is
  incomplete. `mockups/assets/components.css` is the superset — the app sheet must not introduce a
  rule the mockups lack.
- **Never commit to `main`.** Feature branches and pull requests, merged `--no-ff`. Install the hooks
  with `prek install`.

[`docs/issues.md`](issues.md) is the backlog of record; its *Decided — no action needed* section
records deliberate non-tasks, so check there before "fixing" something.
[`issue-tracking.md`](issue-tracking.md) explains the labels, milestones and the doc ↔ tracker
linkage that `cargo xtask issue-sync` enforces.

> **Note on CI.** `ci.yml` runs `fmt + clippy`, `test`, `postgres integration tests` and `cargo-deny`
> on every push to `main` and every pull request — but it filters out `docs/**`, `*.md` and
> `LICENSE*`, so a documentation-only change starts no run. Run the checks locally anyway: `prek run`
> plus the commands above. `cargo xtask gui-pass` needs a graphical-capable machine and is not part of
> CI, so the live-webview layer stays a local step.
