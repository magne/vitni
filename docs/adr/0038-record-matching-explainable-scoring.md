# 38. Record matching: explainable probabilistic scoring

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

Genealogical records describing one individual rarely agree letter for letter:

- A Norwegian census taken by a Danish-trained clerk spells *Gulbrand* as *Guldbrand*.
- A church book Latinizes *Jon* to *Johannes*.
- A patronymic surname changes every generation, and a farm surname changes with every move.
- Census ages are routinely off by several years.
- A baptism date stands in for an unrecorded birth.

Matching has to tolerate all of this without either flooding the user with false candidates or
missing the true ones.

Today the only matcher is `vitni-app/src/duplicates.rs`. It uses a SymSpell/Levenshtein neighbourhood
over a lowercased display name, plus a same-birth-year test. It scores persons only, from the
`PersonSummary` DTO, with no explanation beyond a `MatchKind`. It feeds the `PossibleDuplicates`
data-quality check and the Merge tool, and nothing else.

Record matching is needed in several places, and it must give one answer everywhere:

- import resolution (ADR 0040)
- the duplicate check
- a warning when a user types in a person who already exists
- ranking in record pickers
- a *Find similar* action
- eventually plugins

Research (`docs/research/record-matching.md`) surveys the methods and the products. Probabilistic
record linkage (Fellegi–Sunter) is the standard method: per-feature agreement weights, summed into a
match weight, with an explicit clerical-review band. Its modern implementations (Splink) show that the
per-feature weights double as the explanation. Genealogy products (MyHeritage, FamilySearch, Ancestry)
present a match as the two records side by side, with the evidence that agrees and disagrees, and the
user decides. Beider–Morse phonetic matching shows that name rules must be language-aware and chosen
per name, not global.

## Decision

1. **A pure matching module in `vitni-core`.**
   - `vitni_core::matching` compares two *profiles* of one kind and returns a `MatchAssessment`.
   - It performs no I/O and reads no configuration. Name rules and weights are passed in as values.
   - It is AGPL like the rest of the core (ADR 0034). Plugins reach it only through the host (§8),
     never by linking it.

2. **One profile per matchable kind, built by `vitni-app` from views:**

   | Kind | Profile |
   | --- | --- |
   | Person | names (all, with language), sex, vital events as date intervals (birth, baptism, death, burial), places of those events and of residence, occupations, and **relatives**: parents, partners and children, each with names and a birth interval |
   | Family | the partners' person profiles, children, marriage event |
   | Event | type, date interval, place, participants (names + roles) |
   | Place | names (dated, with language), type, enclosing places, coordinates |
   | Source | title, author, publication info, repository |
   | Repository | name, address |
   | Citation | source, page/locator, date |
   | Media | checksum (exact); path and description as weak evidence |
   | Note | normalized text |
   | Tag | name (exact, case-folded) |

   DnaTest, DnaMatch, ResearchNote and ImportRun are not matched. Kit and match ids are exact
   identifiers already, a research note is a researcher's argument rather than a record an import
   duplicates, and a run is bookkeeping.

3. **Fellegi–Sunter scoring, with the evidence kept.**
   - Each comparator yields an outcome — `Agree`, `Partial(similarity)`, `Disagree`, `Missing` or
     `Conflict` — and a log-likelihood weight.
   - The weights sum to a match weight, mapped to a score in 0..1.
   - The result keeps every term:

   ```rust
   pub struct MatchAssessment {
       pub score: f64,
       pub band: MatchBand,
       pub features: Vec<FeatureComparison>, // feature, outcome, weight, left value, right value
       pub cultures: Vec<CultureId>,         // the name-culture packs applied (§5)
       pub engine: EngineVersion,
   }
   pub enum MatchBand { Deterministic, Probable, Possible, Unlikely }
   ```

   The features *are* the explanation. The compare UI renders them as "Birth 1852 ↔ baptism 1849:
   within census tolerance (+2.1)", so every score explains itself and there is no second
   explanation to drift from the score.

4. **Comparators are graded, never binary.**
   - A comparator returns a similarity, and the similarity maps onto a decaying weight curve. Exact
     equality is only the top of the curve. A near miss lowers the score but keeps the candidate.
     Only an *implausible* distance is `Disagree`, and only a logical impossibility is `Conflict`.
   - **Names:**
     - normalized by the active culture packs (§5)
     - compared by edit distance / Jaro–Winkler on normalized forms, by given-name equivalence
       class, and by phonetic key
     - abbreviations expanded (*Joh.* → *Johannes*, *Olsd.* → *Olsdatter*)
   - **Surnames are weak evidence.** Where a pack says the surname system is patronymic or residence
     derived, a surname mismatch is `Partial` or `Missing`, never a conflict. A patronymic is also
     compared with the candidate father's given name (*Olsen* ↔ father *Ole*).
   - **Dates:**
     - compared as intervals derived from `GenealogicalDate`, covering ranges, Before/After,
       About and partial dates
     - the tolerance scales with the date's quality and provenance
     - **a birth year computed from a census age** tolerates about ±5 years with a slowly decaying
       weight; only a gap beyond ~10 years is `Disagree`
     - **exact dates** decay over days and months, recognizing a transposed day/month and a
       Julian↔Gregorian offset
     - **baptism and christening** stand in for birth, and **burial** for death, with a typical
       offset
   - **Places:**
     - names are compared with the same culture-aware rules, including dated historical names
       (ADR 0026)
     - a place enclosed by the other (farm in parish) is `Partial`, not `Disagree`
     - coordinates decay with distance
   - **Relatives:** a match on a relative's name and birth interval is strong evidence. It is the
     "family match" that separates two *Ole Olsen*s born the same year.
   - **Hard conflicts cap the score:**
     - an asserted sex that differs
     - an impossible lifespan (death before the other's birth)
     - **two different items of the same record** (ADR 0037 §1), such as two members of one
       household, who are by construction different people

5. **Name rules are data: name-culture packs, selected from the evidence.**
   - A pack (`matching/cultures/<id>.toml`) holds:
     - orthographic normalization rules
     - given-name equivalence classes
     - abbreviations
     - diminutives
     - **surname-system traits**: patronymic suffixes and their gendered forms (*-sen/-datter*;
       *-ski/-ska*, *-icz*), and whether surnames follow residence
   - A *cross-culture* pack maps adapted names between two cultures (for example `pl-en`:
     *Wojciech* ↔ *Albert*, *Jan* ↔ *John*).
   - A `universal` pack always applies: Unicode case and diacritic folding, edit distance, a
     phonetic key.
   - The milestone ships `universal`, `no`, `da` and `en`. **Adding a culture is a TOML file plus
     evaluation cases, with no code change**, and a test proves it by loading a toy pack from a
     fixture.
   - Packs are embedded and layered like Fluent catalogues (ADR 0003): a workspace can override or
     add packs.
   - **Selection is per comparison.** The applied packs are listed in the assessment, and a
     comparison uses the union of both sides' sets, plus any cross-culture pack between two of them.
     A side's set comes from:
     - **place and time:** a region table (`matching/regions.toml`, also data) maps a country or
       region and a date range to packs. Norway before the 1907 orthography reform maps to `no` +
       `da`.
     - **data language:** `PersonName.language` and the other `LanguageTag`s of data-model §14.
     - **lineage:** the packs of the person's parents' and ancestors' places. A Polish immigrant's
       child in England gets `pl` + `en` and so `pl-en`.
   - With no signal at all, `universal` plus the workspace's default culture applies. The default is
     a `[matching]` config key (ADR 0005, 0015).

6. **Deterministic identity is not a score.**
   - `MatchBand::Deterministic` is assigned only when identity is already established, and never
     from a high score:
     - the same record origin
     - the same `ExternalId`
     - a pair already decided `same` (ADR 0039)
   - A pair already decided *distinct* is not a candidate at all.
   - The other bands come from the score, against thresholds in `[matching]` config: `Probable`
     above one threshold, `Possible` above a lower one, `Unlikely` below it (not shown).
   - **Only `Deterministic` is ever acted on without the user** (ADR 0040).

7. **Candidate generation is a blocking index looser than the comparators.**
   - Scoring every pair is quadratic, so candidates come from `match_keys`, a projection of
     `(kind, key, aggregate_id)`.
   - Blocking must never be the reason a true match goes unscored, so a record is indexed under
     several deliberately loose keys:
     - the phonetic and normalized given-name key
     - the equivalence-class key, with classes taken from *every* installed pack, not only the
       selected ones
     - the birth decade *and* its neighbours
   - **Surname is not a required blocking key.** Other kinds have per-kind keys: normalized place
     name, source-title tokens, a media checksum.
   - The index is rebuildable from the log plus the installed packs. It records a fingerprint of the
     pack set and rebuilds itself when that changes.
   - It replaces `duplicates.rs`'s in-memory SymSpell index and is justified under ADR 0009 §4.

8. **One engine, every consumer.**
   - `vitni-app` exposes `find_similar(kind, target, min_band, limit)` and `assess(kind, a, b)`.
     These are the only matching entry points, and `duplicates.rs` is removed.
   - Consumers:
     - the `PossibleDuplicates` data-quality check, extended to every matchable kind
     - import planning (ADR 0040)
     - the review queue (ADR 0039)
     - the similar-record hint on manual entry
     - picker ranking
     - *Find similar*
     - a read-only `find-similar` WIT query for plugins
   - All share one threshold configuration and one explanation format.

9. **Weights are data, measured against a labelled corpus.**
   - Initial m/u weights are reviewed constants in an embedded data file, not user-configurable.
   - An evaluation corpus of labelled pairs, with an `xtask` harness, reports precision and recall
     per band. The corpus holds invented pairs plus public-domain census and church records over 100
     years old.
   - The corpus deliberately includes the hard *true* matches: spelling variants, a surname changed
     after a move, a census age off by one to five years, a baptism standing in for a birth.
   - **Recall on those is gated in CI, not only precision.** A change to a comparator, a weight or a
     pack must not lose them.

## Rationale

- **Fellegi–Sunter is the standard, and its weights are its explanation.** Every term in the sum is a
  human-readable statement ("names agree after normalization", "relatives disagree"). This is what the
  side-by-side decision needs. A learned black-box classifier would score as well, but it could not
  tell the user *why*. The user makes the decision, so the reasons matter more than a point of
  accuracy.
- **Graded comparators keep the right candidates.** A binary equality test fails exactly the cases
  genealogy is full of. The decaying curves encode the domain knowledge — census ages drift, baptisms
  follow births, spellings vary — in one place, where the evaluation corpus can test it.
- **Culture packs as data keep the engine language-neutral.** The comparator logic knows about surname
  *systems*, such as patronymic or residence-derived. Which suffixes and which equivalences belong to
  which culture is data. A new culture is then a reviewable data change with its own test cases, and
  the selection is itself evidence the user can see.
- **Deterministic-only automation matches the user's rule.** The user asked to be involved whenever
  the system is less than certain. A score is never certainty, so a score never acts alone.
- **Blocking is where record linkage silently loses recall.** A strict blocking key such as the
  surname hides exactly the spelling and surname-change cases this ADR exists for. Keys that are loose
  on purpose cost some extra scoring and keep the promise.
- **One engine prevents drift.** If the duplicate check, the importer and the picker each had their
  own heuristic, the same pair would be "95%" in one place and absent in another.

## Consequences

### Positive

- Every "are these the same?" question has one answer and one explanation, everywhere in the app.
- Imports stop creating duplicates of people, places and sources the workspace already has, without
  guessing on the user's behalf.
- Norwegian, Danish and English naming work from day one. Other cultures are additive data.
- Matching quality is measured, not asserted, and regressions in recall are caught.

### Negative / costs

- **A real piece of engineering:** comparators, profiles for ten kinds, a blocking projection, data
  packs, a corpus and a harness.
- **Initial weights are informed guesses** until the corpus is large enough. Unsupervised estimation
  (EM) is future work.
- **Profiles require joined reads** (a person with relatives and places). At import scale that needs
  the blocking index to keep candidate counts small, and a benchmark at 100k persons to prove it.
- **The packs need curation.** An equivalence class that is too broad inflates candidates. The corpus
  is the check.
- **Possibly a new dependency** (`strsim`, MIT, for Jaro–Winkler), which must be justified against
  hand-rolling the one function in the implementing change.

## Out of scope

- **Unsupervised weight estimation (EM)** and learned models.
- **Name-culture packs beyond `universal`/`no`/`da`/`en`.** The backlog holds one bullet for them.
- **Cross-workspace or online matching** (MyHeritage-style matching against other users' trees).
- **Fact identity inside one person** (is an imported Occupation an update of an existing one?),
  which ADR 0029 leaves open. This ADR matches records, not claims within a record.

## References

- ADR 0003 — the layered catalogue chain culture packs mirror.
- ADR 0005, 0015 — the `[matching]` configuration.
- ADR 0009 §4 — the index escape hatch for `match_keys`.
- ADR 0026 — dated place names used by the place comparator.
- ADR 0034 — the licence split that keeps matching host-side for plugins.
- ADR 0037 — the record origin as a deterministic signal and as the same-record conflict.
- ADR 0039, 0040 — the identity decisions and the import flow consuming the assessment.
- `docs/research/record-matching.md` — Fellegi–Sunter, Splink, Beider–Morse, genealogy products,
  Scandinavian naming and census specifics.
- `docs/data-model.md` §7.1 (`GenealogicalDate`), §14 (data language).
