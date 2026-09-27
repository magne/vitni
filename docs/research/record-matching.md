# Research — record matching, duplicate detection, and identity resolution (gates ADR 0038)

- **Status:** Findings informing ADR 0038 (explainable probabilistic scoring for record matching).
- **Date:** 2026-09-27

## Question

Every import path (Digitalarkivet census/church-book transcriptions, GEDCOM, Gramps XML) and manual entry
produces **personas** — evidence-layer claims about a person — that must be resolved against existing
**conclusion persons**, or against each other, without a shared stable key across sources. The same problem
recurs as a data-quality "possible duplicate" check over an existing workspace, and as a hint surfaced during
manual entry. This asks what the record-linkage literature and shipping genealogy products already know about
scoring, thresholds, comparators, and — critically — the user-facing decision UX, so ADR 0038 chooses a
defensible, explainable design rather than inventing one from scratch. It does not decide the ADR; it collects
the evidence the ADR should weigh.

## 1. Fellegi–Sunter record linkage: the statistical baseline

The Fellegi–Sunter (1969) model is the baseline nearly every probabilistic linkage tool still implements,
including the one modern reference implementation surveyed here, **Splink** (UK Ministry of Justice). For each
comparison field it defines two conditional probabilities:

> "The `m` probability measures how often the comparison level occurs among matching records... The `u`
> probability measures how often the comparison level occurs among non-matching records." — Splink, *The
> Fellegi-Sunter model*

Their ratio, the Bayes factor `K = m/u`, is converted to a base-2 log-likelihood **match weight**
`ω = log₂(m/u)` so that per-field evidence **adds** instead of multiplying:

> "The log transform converts multiplicative Bayes Factors into additive match weights... it makes the maths of
> the Fellegi Sunter model particularly simple: to compute the final match weight, just add up partial match
> weights." — Robin Linacre, *m and u values in the Fellegi-Sunter model*

Fields are not binary agree/disagree: a **comparison** is split into ordered **levels** (exact match, then a
fuzzy-match level such as Jaro–Winkler ≥ 0.9, then "else"), each with its own `m`/`u` and therefore its own
partial weight — this is the "partial agreement level" Fellegi and Sunter's original paper already allowed for.
Splink's own worked example sums a prior weight plus per-field weights (forename, surname, DOB, city, email)
into a single match weight, which converts back to a match probability via
`Pr(match) = 2^M / (1 + 2^M)`; the tool's **waterfall chart** is exactly this decomposition rendered so a
reviewer can see which fields drove a score up or down (Splink docs, `topic_guides/theory/fellegi_sunter`).

Three decision **bands** — not a single accept/reject cut — are the field's standard practice: an upper
threshold above which a pair auto-links, a lower threshold below which it is a confident non-link, and a middle
**clerical-review** band routed to a human:

> "Two thresholds turn that score into a decision: at or above the upper threshold the pair auto-matches; below
> the lower threshold it is a non-match; the span between them is a clerical-review band where a human
> adjudicates." — clinical-data.org, *Probabilistic Patient Matching with the Fellegi–Sunter*

Weights are rarely hand-set. Splink's recommended hybrid trains `u` directly (random-sample the proportion of
each comparison level among all pairs, which are overwhelmingly non-matches), and `m` by **Expectation
Maximisation** — an iterative maximum-likelihood fit that treats true match/non-match status as a latent
variable, seeded ("anchored") by a rough estimate of the overall match prior from a small set of strict
deterministic rules plus a guessed recall (Splink, `training_rationale`). EM needs the comparison space
restricted to a plausible-match-heavy subset first, which is exactly what **blocking** is for — and blocking is
also the paper's central open risk: it is "an a priori judgment that these fields are error-free," and choosing
which fields to block on trades a smaller, faster candidate set against **recall** (silently losing true matches
that disagree on the blocking field). A comparative empirical study on a 59k-record mortality linkage found:

> "The multiple steps blocking strategy was more effective, allowing the identification of all the true matches,
> at the same time producing a total number of pairs which was smaller than the one obtained with two different
> single-step strategies." — Camargo Jr. & Coeli, *Evaluation of different blocking strategies in probabilistic
> record linkage* (2002)

i.e. several cheap, loose blocking passes (union of candidates) beats one tight pass, because a candidate lost
by one blocking key can still be caught by another. A large simulation study comparing probabilistic against
purely deterministic (exact-key) linkage found probabilistic linkage dominates on messy data and is only
matched by deterministic linkage when error/missingness is under about 5%:

> "Probabilistic linkage uniformly outperformed deterministic linkage as the former generated linkages with
> better trade-off between sensitivity and PPV regardless of data quality. However, with low rate of missing and
> error in data, deterministic linkage performed not significantly worse." — Zhu et al., *When to conduct
> probabilistic linkage vs. deterministic* (2015)

Historical genealogical sources (below) are firmly in the "not low-error" regime, which argues for the
probabilistic model as the default rather than an optimization applied only when deterministic matching fails.

## 2. String comparators and phonetic matching for names

**Edit-distance family.** Levenshtein distance counts single-character insertions, deletions and substitutions;
Damerau–Levenshtein adds adjacent-character transposition as a fourth unit-cost operation, which matters for
names because transposition ("Pendleton" ↔ "Pendelton") is a common transcription error and scores as distance 1
under Damerau–Levenshtein but 2 under plain Levenshtein (ISR/UNM, *Assessing Record Linkage Matches Using String
Distance Measures*). Jaro similarity is a different family — matching characters within a positional window
plus a transposition penalty, normalized to `[0,1]` — and **Jaro–Winkler** boosts it for strings sharing a
prefix, "developed for comparing names at the U.S. Census Bureau" (Winkler 1990, cited in the `comparator` R
package docs). It is the comparator most cited across the historical-linkage projects below (IPUMS/NAPP, HISTREG,
Link-Lives) specifically because names diverge more at the end (case endings, patronymic suffixes) than at the
start, and the prefix boost rewards exactly that pattern; it is not a metric in the formal sense (it fails the
triangle inequality) but performs well empirically and is cheap at scale (see IPUMS's own bitwise/SIMD
reimplementation, tech.popdata.org, *Implementing the Fastest (Pseudo) Jaro-Winkler Algorithm in Rust*, built to
link the full 1930/1940 US censuses).

**Phonetic keys.** Soundex (1918, four-character code) is coarse and English-biased; Daitch–Mokotoff Soundex
(1985) is a refinement built specifically for Slavic and Yiddish surnames — six digits instead of four, n-gram
(not single-letter) coding, and **multiple simultaneous encodings per name** to cover ambiguous pronunciations
(Wikipedia, *Daitch–Mokotoff Soundex*). Metaphone (1990) and Double Metaphone (2000) instead try to model actual
pronunciation rather than a letter-frequency code, with Double Metaphone producing a primary and an alternate
encoding and folding in some non-English pronunciation rules, though — unlike Daitch–Mokotoff — "it lumps all
the foreign rules together and doesn't distinguish which rule corresponds to which language" (Morse & Beider,
*Phonetic Matching: A Better Soundex*).

**Beider–Morse Phonetic Matching (BMPM)** is the most relevant precedent for a *per-culture* system: rather than
one fixed rule set, it first infers the likely source language(s) of a name from its spelling, then applies
language-specific pronunciation rules, falling back to generic rules only when the language is ambiguous:

> "Phonetic Matching is based on the principle that pronunciation depends on the language. So the first step is
> to determine the language from the spelling of the name. Then the name is converted into a sequence of
> phonetic tokens using pronunciation rules specific to that particular language." — Morse & Beider, *Phonetic
> Matching: A Better Soundex*

It ships language tables for around 16 languages (Catalan, Czech, Dutch, English, French, German, Greek, Hebrew,
Hungarian, Italian, Polish, Portuguese, Romanian, Russian, Spanish, Turkish), with dedicated Ashkenazic and
Sephardic variants, is table-driven ("all that is necessary is to add additional tables to support the
additional languages" — not hard-coded per language), and includes rules for gendered surname endings in Slavic
languages (defeminizing `Sucha` → `Suchy` before phonetic encoding) and for names that were themselves
transliterated from Hebrew. A head-to-head benchmark against American Soundex and Daitch–Mokotoff on searches
for "Washington" and "Obama" found BMPM's false-positive rate an order of magnitude lower than both older
systems (Morse & Beider, `bmpm2.pdf`) — the direct evidence for choosing a language-aware phonetic layer over a
single fixed one. Beider–Morse's own genealogy of predecessors is documented in the same source.

**Name-equivalence tables.** Beyond phonetics and edit distance, genealogy platforms maintain curated
nickname/variant dictionaries — FamilySearch's Family Tree stores an explicit `Nickname`/`AlsoKnownAs` name type
per person and a separate indexer "Lookup List" of known spelling variants (FamilySearch, *How do I add
nicknames to Family Tree?*; *How to Use the Lookup List*) — a complementary, curated-list mechanism alongside
algorithmic comparators, useful for the given-name-variant case (Bill/William, Peder/Petter/Per) that string
distance alone under- or over-generalizes.

## 3. How genealogy products do it: the UX of duplicate resolution

**MyHeritage** splits automatic matching into *Smart Matches* (tree-to-tree) and *Record Matches*
(tree-to-historical-record), both landing in a review queue with a per-match star **Confidence Score** (half a
star to five), reviewed one at a time with a side-by-side compare and a binary **Confirm / reject** decision;
confirming opens a further screen to selectively pull individual facts into the tree rather than accepting the
whole record:

> "Each match has a Confidence Score that ranges from half a star to 5 stars, indicating the likelihood that the
> historical record found is indeed relevant to the associated individual in your tree." — MyHeritage,
> *Introduction to Record Matches*

Notably, MyHeritage tells users the score is not a proxy for "should you look at this": a *low*-confidence match
can be the more valuable one, because it means the record disagrees with (or adds to) what the tree already
holds, whereas a high-confidence match may be pure duplication — a useful UX lesson for how vitni should present
scores rather than treat them as a plain sort key.

**FamilySearch Family Tree**'s Possible Duplicates flow is a three-way decision, not confirm/reject: **merge**
(same person), **not a match** (different people — permanently suppresses that pair, visible to all users on
the shared tree), or **defer** (cancel, leaving the flag for another user later). The merge screen is an explicit
three-column compare (candidate / current / result-to-be-kept) with a mandatory typed-or-picklist reason
statement before the merge commits — mirroring the reasoned-overwrite pattern already used for FamilySearch's
GEDCOM-conflict flow (see `docs/research/merge-sync-conflict-resolution.md`):

> "If they are about the same person, merge them. If they are about different people, mark them as 'not a
> match.' If you are unsure, do not merge them." — FamilySearch, *How do I merge possible duplicates in Family
> Tree?*

FamilySearch also hard-blocks certain merges structurally (sex mismatch, one record living/confidential, a
parent–child link between the two candidates, one already-deleted-by-prior-merge) rather than relying on the
reviewer to catch them (FamilySearch, *Why do merges fail in Family Tree?*) — a validation layer independent of
the match score.

**Ancestry**'s "Hints" (the shaky leaf) use a three-state decision — **Yes** (accept, pulls facts in for
field-by-field review), **No** (reject, permanently hidden), **Maybe** (defer to an Undecided queue) — applied
per hint rather than per person, with per-fact left/right compare on acceptance and explicit "record has
information not in your tree" vs. "differs from your tree" indicators per field (Ancestry, *Ancestry Hints*
help article). Hints cannot be globally disabled, only individually triaged, which keeps the review queue as
the sole gate.

**Geni** layers three independent detectors — internal *Tree Matches* (duplicate profiles within/across Geni
family trees), and MyHeritage-sourced *Record Matches* and *Smart Matches* (Geni is a MyHeritage property) — each
with its own confirm affordance, plus manual "Merge This Profile" and drag-and-drop merge in the tree view. Its
side-by-side compare colour-codes agreement itself:

> "Green text in the profile boxes indicate an exact match. Black text indicates data that doesn't match
> exactly. Red text would indicate that there is a substantial discrepancy." — Geni Help Center, *How To: Use
> Tree Matches*

with a three-way outcome — "Yes, Merge/Request to Merge", "No, remove match", "I'll decide later" — the same
shape FamilySearch and Ancestry converge on independently: **accept / reject / defer**, never a forced binary.

**Gramps' "Find Possible Duplicate People"** tool (documented behaviour only; Gramps is GPLv2+ and its source is
never to be copied into this AGPL-licensed codebase per `docs/CLAUDE.md`) computes a compounding "odds" score
across matching fields and reports pairs whose score exceeds a user-chosen **Low/Medium/High** threshold
(0.25 / 1.0 / 2.0), with Soundex matching as an optional toggle:

> "For each piece of information that corresponds, the quality of the match is considered to be marginally
> increased. The closer the particular pieces of data correlate, the larger the increase. When the chance is
> greater than the selected Match Threshold then a match will [be] reported." — Gramps 6.0 Wiki Manual, *Tools*

The result is a flat, rank-ordered "Potential Merges" list (score, first person, second person) that the user
works through one row at a time via the same Merge People dialog used for a manual merge — i.e. no separate
review affordance, no accept/reject/defer state machine, and (per a since-merged bug report) the historical
implementation only ever surfaced a person's single highest-scoring candidate pair rather than all of them
(`gramps-project/gramps` PR #1000) — a known limitation worth naming as something vitni's own tool should not
inherit.

Across all five products the recurring UX shape is: **a queue, not a blocking gate**; **a score or star rating
shown but not authoritative**; **side-by-side compare with per-field agreement highlighting**; and an
**accept / reject / defer trichotomy**, with "reject" (FamilySearch's "not a match", Ancestry's "No") persisted
so the same pair is never re-suggested. None of the five auto-merges on score alone without a human decision in
the loop.

## 4. Historical Scandinavian specifics

**Norwegian naming** before the 1923 Names Act was three-part — given name, patronymic (`-sen`/`-son` for a son,
`-datter`/`-dtr` for a daughter, reset every generation, so patronymics cannot trace a lineage), and a **farm
name** that functioned as an address, not a surname, and changed whenever the family moved:

> "If Peder moved from the Berg farm to the Vik farm, he would be known as Peder Johnsen Vik... from then on." —
> Digitalarkivet, *Start tracing your ancestry in Norway*

The same person can therefore appear under three different full names across a lifetime's records purely from
relocation, unrelated to any transcription error — the Digitalarkivet's own worked example follows one woman
from `Mathilde Matiasdtr.` (1900 census) to `Matilde Matiasen Nordaas` (1910) to `Mathilde Brukvik` (1977 death
registry) (Digitalarkivet, *Name variant results*). Digitalarkivet's own name search already folds patronymic
spelling variants (`Olsen`/`Olsøn`) and farm-name spelling variants (`Nordås`/`Nordaas`/`Noraas`) into one
query — evidence that this is treated as a search/matching problem, not merely a display concern, in the
canonical Norwegian source archive vitni's `vitni-digitalarkivet` crate parses.

Spelling itself is unstable independent of naming custom: pre-1900s clergy wrote names phonetically as spoken,
"Peter, Petter, Peder or Per may very well be the same person recorded by different clerks" (norwaydna.no,
*Norwegian Names*), and orthography changed under two documented spelling reforms — 1907 (the first official
Bokmål/Riksmål spelling, replacing near-Danish public-register spelling with Norwegianized forms) and 1917 (the
first reform shared by Bokmål and Nynorsk, including `aa`→`å` and Norwegianizing loanwords) — so a name's
"correct" written form is itself period-dependent (NDLA, *Rettskrivingsreformene mellom 1901 og 2012*; Wetås,
*100 years of language planning in Norway*). Church books before roughly 1900 also mix Norwegian with Latin —
liturgical calendar dates ("2 Pascha"), some legal/religious terminology, and occasional Latinized personal-name
forms in the record's formal Latin passages — compounding the transcription-variance problem (*Genealogy 201:
Reading the Norwegian Churchbooks*; Christensen, *Glossary and abbreviations*, `genealogicalresearchnorway.blog`).
**Not independently verified**: the extent to which individual given names (as opposed to liturgical/calendar
vocabulary) were routinely Latinized in ordinary baptism entries; the sources found document Latin usage for
dates and legal terms specifically, not a systematic Latinization of personal names.

**Census age is unreliable, not merely approximate.** "Age heaping" — digit preference toward ages ending in
0 or 5 — is a well-documented artifact across historical censuses generally, used by economic historians as a
numeracy proxy, and its magnitude is demographically confounded (an older population heaps more, independent of
any change in the population's numeracy) (Szołtysek et al., *Age heaping patterns in Mosaic data*; McLaughlin,
Colvin & Henderson, *Demography and Age Heaping*). A Catalan record-linkage study tracking the same individuals
across consecutive registers found ages ending in 0 or 5 correlate with a measurably *larger* individual error
in age when the same person's age is compared across sources — i.e. heaping is not just a rounding-to-a-nearby-
plausible-value artifact but a genuine marker of a less reliable age statement (Bogonat et al., *Numeracy and
consistency in age declarations*, Cliometrica 2023). Norway's own HISTREG explicitly names census age as one of
the two flagship uses of its linked data ("source criticism focusing on birth dates in censuses" — Holden,
Boudko & Thorvaldsen, *Historisk befolkningsregister*), i.e. the register exists partly *because* census ages
cannot be trusted at face value and must be checked against a linked life course.

**Baptism date is not birth date.** Where only a baptism record survives, treating its date as the birth date is
explicitly warned against by genealogy references — the interval between birth and baptism varies by faith,
region, era and individual circumstance (an itinerant priest's visiting schedule, a family postponing to gather
relatives, group baptisms of several children at once) and is not a fixed offset:

> "If you have a baptism date, note it as what it is: a date of baptism only. Do not try to make a baptism equal
> a birth." — *The French Genealogy Blog*, "Does a Baptism Date Imply a Date of Birth?"

This is a direct argument for a comparator that treats a birth-date field sourced from a baptism record as
carrying a wider, source-typed uncertainty band rather than a point value, distinct from a civil birth
certificate's near-exact date.

**Julian → Gregorian.** Denmark–Norway adopted the (solar) Gregorian calendar in one jump, Sunday 18 February
1700 followed by Monday 1 March 1700 (Wikipedia, *Adoption of the Gregorian calendar*; *1700 in Denmark*), but
kept computing Easter astronomically rather than under the Catholic lunar rule until 1743 — so two nominally
"Gregorian" Danish-Norwegian records can still disagree by up to a week on any moveable feast dated between 1700
and 1743 (tidsskrift.dk, *Saeculum confusionis*). Sweden's separate, aborted gradual transition (1700–1712,
producing the real calendar date 30 February 1712) is a reminder that "Nordic" is not one calendar history —
Denmark-Norway, Sweden and Finland converted on different schedules (kleiobase.com, *Julian or Gregorian?
Reading Old Record Dates*), so any date-proximity comparator needs the record's country/denomination, not just
its raw date, to interpret correctly for records before ~1750.

**HISTREG (Norwegian Historical Population Register)** is the most directly relevant precedent: a national,
crowdsourced + algorithmic linkage of Norway's censuses (1801–1920) and parish registers, run by Norsk
Regnesentral with the National Archives. Its hybrid approach — "more than 90% of the links in NHPR-O links are
generated [algorithmically]. The remaining links are made by crowdsourcing" — pairs automatic linking with
continuous, reversible human correction:

> "Crowdsourcing comes at the cost of not being able to enforce strict linking criteria... For this reason,
> linking must be iterative. Erroneous links are removed during regular quality controls, in contrast to many
> other population registers, where links, once added, are almost never removed." — Holden, Boudko & Thorvaldsen,
> *Historisk befolkningsregister*, 2025

Links are graded on an explicit 0–10 quality scale ("10: Completely secure link... 8: The same birth date and
same or similar names...") rather than left as an accept/reject binary, and the project's stated best evidence
of correctness is not any single field match but "the resulting set of connections forms a plausible life
course" — a whole-lifecourse coherence check no single pairwise comparison can offer (Thorvaldsen, Andersen &
Sommerseth, *Record linkage in the historical population register for Norway*). Reported linkage rates run
roughly 66–90% depending on period and region.

**IPUMS / NAPP (Ruggles et al.)** linked the 1865/1875/1900 Norwegian censuses (and the US 1880 complete count to
neighbouring years) using Jaro–Winkler name similarity plus an age tolerance window (Norway: ±3 years for
individuals, ±5 for couples), restricted to an exact match on birth municipality, then classified candidate
pairs with a trained **Support Vector Machine** rather than a hand-tuned threshold, deliberately discarding any
person with more than one positive-scoring candidate rather than guessing:

> "Whenever a person from a non-1880 sample had more than one true link in the 1880 data, we excluded the case
> from the linked files... Our experience with nineteenth-century census data suggests that this type of
> conservative approach is necessary." — MPC/IPUMS, *North Atlantic Population Project* linkage documentation

That conservatism — silence over a guess when multiple candidates tie — is a directly transferable design
principle. NAPP also weights the resulting linked sample by demographic characteristics to correct for who is
*not* linkable (the harder-to-link subpopulation is not a random sample of the whole).

**Link-Lives (Denmark)** links ten fully transcribed censuses (1787–1901), six parish-record types and a burial
register, combining manual "domain expert" links (used as training data and as a quality baseline), a
rule-based Fellegi–Sunter-shaped pipeline, and machine-learning models trained on the manual links. Its
rule-based algorithm blocks on sex and age ±2 years, scores name/birthplace similarity with Jaro–Winkler applied
separately to standardized first name, family name and patronym, and — notably — **iteratively loosens its
threshold** rather than fixing one cutoff:

> "Our initial threshold is a link score of maximum 0.03... In subsequent iterations, the tolerance is increased
> in steps of 0.02 until a maximum of 0.15 is reached." — Link-Lives, *Link-Lives Guide v.1*

Link-Lives also maintains hand-built "synonym catalogs" mapping historical name-spelling variants to a
canonical form, built by subject-matter experts specifically because spelling variance grows with the age of
the source ("Ane Laursdatter... may very well be the same person as Anna Larsdatter") — the same equivalence-
table pattern FamilySearch uses for nicknames, but here built for orthographic rather than nickname variation,
and it separately measured how much manual, rule-based and ML linking each capture relative to a domain-expert
baseline (manual ~80% recall of true links, rule-based ~70% of what manual captures, in a few hours of compute
versus person-hours of manual work) (Revuelta-Eugercios, *Link-Lives: Building Historical Big Data...*).

## 5. Multi-culture naming: the case for per-culture rule packs

Polish surnames illustrate a naming dimension none of the above address: **grammatical gender marking on the
surname itself**, not just the given name. Adjectival Polish surnames decline by sex (`Kowalski`/`Kowalska`,
`Nowotny`/`Nowotna`) and historically also took distinct suffixed feminine forms for married/unmarried women
(`-owa`, `-ówna`) before a post-WWII policy banned the latter in official use — meaning the *correct* expected
feminine form of a surname is itself period-dependent, and modern data shows a measurable trend of women using
the masculine form instead, so a same-surname comparator tuned only to modern Polish usage will silently
under-match older records (Walkowiak, *Feminine Surnames in Polish: Two Policies and the Practice*, 2012; CEEOL
abstract, *"Piekielny" czy "Piekielna"?*). This is structurally the same problem Beider–Morse's own
"defeminizing" preprocessing step solves for Slavic surnames generally before phonetic encoding — evidence that
gender-suffix stripping belongs in the comparator pipeline for any Slavic-language culture pack, not just the
Ashkenazic case BMPM was built for.

**Anglicization on immigration** is real but is popularly over-attributed to a single dramatic event (the "Ellis
Island name change" story is well-documented as a myth — inspectors worked from ship manifests created abroad
and had no authority to rename passengers; name changes instead accumulated afterward, through the immigrant's
own choice, mis-transcription at any of several transcription points before arrival, or later voluntary
naturalization-time changes) (NYPL, *Why Your Family Name Was Not Changed at Ellis Island*; USCIS, *Immigrant
Name Changes*). The practical consequence for matching is the same regardless of mechanism: a pre-immigration
source name and a post-immigration source name for the same person can differ by more than spelling — an
outright translation or a chosen new surname — which no string or phonetic comparator alone recovers; it is a
genealogical-reasoning problem (family/household continuity, shared migration record) rather than a
name-comparator one, and worth naming explicitly as something graded name comparators cannot fully solve.

**Beider–Morse's language-detection-then-rules architecture** (section 2) is the strongest existing precedent
for structuring per-culture comparators as *data* (a language/culture table selected by evidence) rather than
one hard-coded ruleset: it already demonstrates that (a) language/culture can often be inferred from the
spelling of the name itself, (b) rules do not need to be hard-coded per language — "the phonetic engine is table
driven and all that is necessary is to add additional tables" — and (c) a name can legitimately match under more
than one language's rules when its own culture is ambiguous or mixed (e.g. Jewish surnames from interwar
Poland, spelled under German orthographic convention). For vitni, the source record itself usually supplies a
stronger signal than name spelling alone — place, time period, and the parsed data's own `LanguageTag`/
`PlaceName` metadata (data-model §14) — arguing for **place/time/language-selected rule packs**, of which a
Beider–Morse-style spelling inference is a fallback for records lacking that context, not the primary
selector.

## 6. Implications for vitni (research implications, not a decision)

These are candidates the ADR should weigh, not conclusions it is bound by:

- **Graded, not binary, comparators.** Fellegi–Sunter's per-field comparison *levels* (exact / fuzzy-above-
  threshold / else), each with its own weight, is the shape that both lets Splink's waterfall-style breakdown
  work and matches every historical-linkage project surveyed (HISTREG, IPUMS/NAPP, Link-Lives) — none scored a
  plain equal/unequal per field.
- **Date comparators need a decay curve with source-typed tolerance, not a fixed window.** A census age is
  demonstrably noisier than a baptism or civil-registration date (age heaping, §4); a baptism date is a
  systematically-offset proxy for birth date, not birth date itself. A single tolerance (e.g. ±3 years for all
  "birth date" fields regardless of provenance) would either over-penalize good civil data or under-penalize bad
  census data; NAPP's own per-country, per-field-source tolerance choices (±3/±5 years for Norway, by role) are
  the closest existing precedent.
- **Loose, multi-pass blocking over one tight pass.** The Camargo/Coeli blocking-strategy study and Splink's own
  "round robin" of multiple EM training/blocking passes both argue for a union of several cheap blocking keys
  (e.g. phonetic surname key + birth-decade + parish) rather than one conjunctive key — recall lost to blocking
  is invisible and unrecoverable later, unlike precision lost to a bad score.
- **Never auto-link on score alone.** Every product surveyed in §3 and every research project in §4 routes
  matches through a human decision (clerical review, confirm/reject/defer, or crowdsourced correction) rather
  than committing a link purely from a score crossing a threshold — and HISTREG's own experience is that even
  its algorithmic majority still needs continuous, reversible human correction. This argues against any
  "auto-merge above threshold X" mode in vitni, consistent with the event-sourced model's own bias toward an
  auditable human (or explicitly Software-agent) decision recorded as an event, not a silent projection change.
- **Explainable features over an opaque score.** Splink's waterfall chart and Gramps' rating both expose *why*
  a score is what it is; an SVM/ML-classifier score (IPUMS, Link-Lives' ML tier) is harder to explain to a
  reviewer than an additive log-weight sum — ADR 0038's own working title ("explainable probabilistic scoring")
  already leans Fellegi–Sunter-shaped for this reason, and this research did not find a genealogy product that
  successfully explains an ML-only score to an end user.
- **A labelled evaluation corpus, scored by precision/recall per band, not one aggregate accuracy number.**
  Blocking, comparator choice, and threshold placement all trade recall against reviewer workload differently;
  IPUMS/NAPP, Link-Lives and HISTREG each report a linkage rate *and* a description of what was deliberately
  left unlinked (ties, sparse data) rather than a single score. A durable test corpus of known-same and
  known-different persona pairs, covering the Norwegian naming/spelling cases in §4 and at least one non-
  Scandinavian culture pack (§5), is what would let ADR 0038's chosen thresholds be validated rather than
  asserted.
- **Per-culture rule packs selected by record context, not spelling alone.** Beider–Morse's table-driven,
  language-selected phonetic rules are the clearest existing precedent for treating a culture pack as
  configuration data layered under one comparator engine; vitni's own place/time/language metadata is a
  stronger selector than spelling-based language guessing for records that carry it.

## References

- Fellegi, I. P. & Sunter, A. B. (1969). *A Theory for Record Linkage*, JASA 64(328) — the founding paper
  (not separately re-fetched here; summarized via the secondary sources below).
- Splink (UK Ministry of Justice), *The Fellegi-Sunter model* —
  <https://moj-analytical-services.github.io/splink/topic_guides/theory/fellegi_sunter.html>.
- Splink, *Training rationale* (EM, direct estimation of λ/u) —
  <https://moj-analytical-services.github.io/splink/topic_guides/training/training_rationale.html>.
- Splink, *Estimating model parameters* tutorial —
  <https://moj-analytical-services.github.io/splink/demos/tutorials/04_Estimating_model_parameters.html>.
- Splink, *Term frequency adjustments* —
  <https://moj-analytical-services.github.io/splink/topic_guides/comparisons/term-frequency.html>.
- Robin Linacre, *m and u values in the Fellegi-Sunter model* — <https://www.robinlinacre.com/m_and_u_values/>.
- clinical-data.org, *Probabilistic Patient Matching with the Fellegi–Sunter* —
  <https://www.clinical-data.org/hipaa-deidentification-patient-matching/patient-matching-and-mpi-strategies/probabilistic-patient-matching-with-fellegi-sunter/>.
- Sayers, Ben-Shlomo, Blom & Steele, *Probabilistic record linkage*, Int J Epidemiol (2015) —
  <https://pmc.ncbi.nlm.nih.gov/articles/PMC5005943/>.
- Camargo Jr. & Coeli, *Evaluation of different blocking strategies in probabilistic record linkage* (2002) —
  <https://www.researchgate.net/publication/262513220>.
- Steorts et al., *A Comparison of Blocking Techniques for Record Linkage* —
  <http://www2.stat.duke.edu/~rcs46/linkage_readings/2014-SteortsBlockingComparisons.pdf>.
- Zhu, Y. et al., *When to conduct probabilistic linkage vs. deterministic linkage* (2015) —
  <https://www.sciencedirect.com/science/article/pii/S1532046415000921>.
- Wikipedia, *Jaro–Winkler distance* — <https://en.wikipedia.org/wiki/Jaro%E2%80%93Winkler_distance>.
- IPUMS/popdata.org, *Implementing the Fastest (Pseudo) Jaro-Winkler Algorithm in Rust* —
  <https://tech.popdata.org/speeding-up-Jaro-Winkler-with-rust-and-bitwise-operations/>.
- ISR/UNM (Denman), *Assessing Record Linkage Matches Using String Distance Measures* —
  <https://isr.unm.edu/reports/2019/assessing-record-linkage-matches-using-string-distance-measures.pdf>.
- Beider, A. & Morse, S. P., *Beider-Morse Phonetic Matching: An Alternative to Soundex with Fewer False Hits* —
  <https://stevemorse.org/phonetics/bmpm.pdf>; overview page <https://stevemorse.org/phonetics/bmpm.htm>.
- Morse, S. P. & Beider, A., *Phonetic Matching: A Better Soundex* (benchmark vs. Soundex/Daitch-Mokotoff) —
  <https://stevemorse.org/phonetics/bmpm2.pdf>.
- Wikipedia, *Daitch–Mokotoff Soundex* — <https://en.wikipedia.org/wiki/Daitch-Mokotoff_Soundex>.
- PostgreSQL 18 documentation, *F.16. fuzzystrmatch* (Soundex/Daitch-Mokotoff/Metaphone/Double Metaphone) —
  <https://www.postgresql.org/docs/18/fuzzystrmatch.html>.
- FamilySearch, *How do I add nicknames to Family Tree?* —
  <https://www.familysearch.org/en/help/helpcenter/article/how-do-i-add-nicknames-to-family-tree>.
- FamilySearch, *How to Use the Lookup List* — <https://www.familysearch.org/en/blog/how-to-use-the-lookup-list>.
- MyHeritage, *Introduction to Record Matches* —
  <https://education.myheritage.com/article/introduction-to-record-matches/>.
- MyHeritage, *How do I confirm and reject Record Matches?* —
  <https://www.myheritage.com/help/en/articles/12851742-how-do-i-confirm-and-reject-record-matches>.
- FamilySearch, *How do I merge possible duplicates in Family Tree?* —
  <https://www.familysearch.org/en/help/helpcenter/article/how-do-i-merge-possible-duplicates-in-family-tree>.
- FamilySearch, *Why do merges fail in Family Tree?* —
  <https://www.familysearch.org/en/help/helpcenter/article/why-do-merges-fail-in-family-tree>.
- Ancestry, *Ancestry Hints* —
  <https://support.ancestry.com/articles/en_US/Support_Site/Ancestry-Hints?geo-lang=en-NO>.
- Geni Help Center, *How To: Use Tree Matches* —
  <https://help.geni.com/hc/en-us/articles/229703387-How-To-Use-Tree-Matches>.
- Geni Help Center, *Family Tree Legend* (Pending Matches) —
  <https://help.geni.com/hc/en-us/articles/24003262152343-Family-Tree-Legend>.
- Gramps 6.0 Wiki Manual, *Tools* (Find Possible Duplicate People — behaviour description only, no source
  reproduced) — <https://gramps-project.org/wiki/index.php/Gramps_6.0_Wiki_Manual_-_Tools>.
- `gramps-project/gramps` PR #1000, *Find possible duplicate people enhancements* —
  <https://github.com/gramps-project/gramps/pull/1000>.
- Digitalarkivet, *Start tracing your ancestry in Norway* —
  <https://www.digitalarkivet.no/en/content/1573/start-tracing-your-ancestry-in-norway>.
- Digitalarkivet, *Name variant results* — <https://www.digitalarkivet.no/en/content/search-names>.
- norwaydna.no, *Norwegian Names* — <https://www.norwaydna.no/gedcoms-and-genealogy/norwegian-names-en/>.
- NDLA, *Rettskrivingsreformene mellom 1901 og 2012* —
  <https://ndla.no/en/r/norsk-pb/rettskrivingsreformene-mellom-1901-og-2012/8074a9397f>.
- Wetås, Å., *100 years of language planning in Norway* —
  <http://efnil.nytud.hu/documents/conference-publications/mannheim-2017/EFNIL-Mannheim-30-Wetas.pdf>.
- *Genealogy 201: Reading the Norwegian Churchbooks* —
  <https://abcdocz.com/doc/31569/genealogy-201---reading-the-norwegian-churchbooks>.
- Christensen, L. B., *Appendix 3 – Glossary and abbreviations* —
  <https://genealogicalresearchnorway.blog/2023/10/28/glossary-and-abbreviations/>.
- Szołtysek, Poniat & Gruber, *Age heaping patterns in Mosaic data* —
  <https://ideas.repec.org/a/taf/vhimxx/v51y2018i1p13-38.html>.
- McLaughlin, Colvin & Henderson, *Demography and Age Heaping: Solving Ireland's Post-Famine Digit Preference
  Puzzle* — <https://www.quceh.org.uk/uploads/1/0/5/5/10558478/wp22-07.pdf>.
- Bogonat et al., *Numeracy and consistency in age declarations* (Cliometrica, 2023) —
  <https://link.springer.com/article/10.1007/s11698-023-00277-w>.
- Wikipedia, *Adoption of the Gregorian calendar* (Denmark-Norway) —
  <https://en.wikipedia.org/wiki/Adoption_of_the_Gregorian_calendar>; *1700 in Denmark* —
  <https://en.wikipedia.org/wiki/1700_in_Denmark>.
- tidsskrift.dk, *Saeculum confusionis: kalenderreformerne i år 1700 og år 1743 i Danmark-Norge* —
  <https://tidsskrift.dk/historisktidsskrift/article/download/53788/72028?inline=1>.
- kleiobase.com, *Julian or Gregorian? Reading Old Record Dates* —
  <https://kleiobase.com/blog/reading-dates-old-records-julian-gregorian-feast-days>.
- Holden, L., Boudko, S. & Thorvaldsen, G., *Norwegian Historical Population Register, 1801-present* —
  <https://home.nr.no/~holden/NorwHistPopReg-komplett.pdf>.
- Holden, Boudko & Thorvaldsen, *Historisk befolkningsregister som et autoritetsregister* —
  <https://www.scup.com/doi/full/10.18261/heimen.62.4.4>.
- Thorvaldsen, Andersen & Sommerseth, *Record linkage in the historical population register for Norway* —
  <https://elar.urfu.ru/bitstream/10995/102780/1/2-s2.0-84944207718.pdf>.
- IPUMS International, *Linked data details* (US/Norway/Sweden methodology) —
  <https://international.ipums.org/international/linked_data_details.shtml>.
- North Atlantic Population Project, *Linked samples* — <https://www.nappdata.org/napp/linked_samples.shtml>.
- Goeken et al., *New Methods of Census Record Linking* — <https://pmc.ncbi.nlm.nih.gov/articles/PMC3090184/>.
- Ruggles et al., *The North Atlantic Population Project: Progress and Prospects* —
  <https://pmc.ncbi.nlm.nih.gov/articles/PMC3244724/>.
- Rigsarkivet, *Link-Lives Guide v.1* —
  <https://www.rigsarkivet.dk/wp-content/uploads/2022/08/Link-lives-Guide-v.1.pdf>.
- Link-Lives, *Creation of life courses* — <https://link-lives.dk/en/creation-of-life-courses/>.
- Link-Lives Release 2 Guide (2025) —
  <https://digidata.rigsarkivet.dk/aflevering/14001/link_lives%20release%202%20guide%20(2025).pdf>.
- Revuelta-Eugercios, B., *Link-Lives: Building Historical Big Data from Archival Records* —
  <https://www.digitaltreasures.eu/wp-content/uploads/2021/12/LinkLives-project-Revuelta-Eugercios.pdf>.
- Walkowiak, J. B., *Feminine Surnames in Polish: Two Policies and the Practice* (2012) —
  exa.ai library mirror, <https://exa.ai/library/publication/2j6cyjq4d0j>.
- CEEOL, *"Piekielny" czy "Piekielna"?* (adjectival Polish surnames, feminine form choice) —
  <https://www.ceeol.com/search/article-detail?id=573923>.
- NYPL, *Why Your Family Name Was Not Changed at Ellis Island* —
  <https://www.nypl.org/blog/2013/07/02/name-changes-ellis-island>.
- USCIS, *Immigrant Name Changes* —
  <https://www.uscis.gov/records/genealogy/genealogy-notebook/immigrant-name-changes>.
- `docs/data-model.md` §14 (`LanguageTag`, `PlaceName`, `PersonName.transliterations`); the evidence/conclusion
  persona↔person model this research assumes throughout.
- `docs/research/merge-sync-conflict-resolution.md` — the reasoned-overwrite / `EventContext.rationale`
  precedent this note's §3 (FamilySearch merge reason) extends.
