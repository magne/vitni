//! Blocking keys (ADR 0038 §7): the loose keys candidates are generated from, so that only pairs
//! sharing a key are scored.
//!
//! Blocking must never be the reason a true match goes unscored, so a record is indexed under several
//! deliberately loose keys. A person is keyed by each given-name token — normalized, by phonetic key
//! (and every phonetic key one letter shorter, so a one-letter slip still meets), and by the given-name
//! equivalence classes of **every** installed pack — each qualified by the decade of the birth estimate.
//! Its surnames are keyed the same way, but a surname is never *required*: a patronymic or a farm name
//! that changed after a move still meets on the given name, and a given name recorded differently still
//! meets on the surname. Other kinds have their own keys: a place's name tokens, a source's title words, a media
//! checksum, a tag's folded name. Every kind is also keyed by its record origins and external ids, so an
//! established identity is always a candidate.
//!
//! A key is a base with an optional qualifier: `base@185` (a decade), `base#12` (a citation's page
//! word) or `base@?` / `base#?` (not known). A [`Probe`] is the lookup side: a decade meets its
//! neighbours, and an unknown qualifier meets every one, so a dated record and an undated one always
//! meet. [`BlockingKeys::fingerprint`] identifies the keying rules and the pack set, so an index built
//! under other packs is rebuilt rather than trusted.

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::date::GenealogicalDate;
use crate::matching::date::interval;
use crate::matching::event::is_principal;
use crate::matching::name::{Applied, fold, phonetic_key};
use crate::matching::pack::CulturePack;
use crate::matching::profile::{
    CitationProfile, EventProfile, FamilyProfile, MediaProfile, NoteProfile, PersonProfile, PlaceProfile,
    RepositoryProfile, SourceProfile, TagProfile, VitalEvent, VitalKind,
};
use crate::matching::{MatchData, estimate};
use crate::media_path::MediaPath;
use crate::name::PersonName;
use crate::origin::RecordOrigin;
use crate::text::ExternalId;

/// The version of the keying rules; bumping it rebuilds every index.
pub const KEYS_VERSION: u32 = 1;

/// The qualifier of a key whose decade is not known.
const UNKNOWN: &str = "?";

/// The mark before a decade qualifier.
const DECADE: char = '@';

/// The mark before a page qualifier.
const PAGE: char = '#';

/// A phonetic key at least this long is also keyed by every key one letter shorter.
const MIN_DELETION: usize = 4;

/// A date whose interval spans more years than this says nothing about the decade.
const MAX_SPAN_YEARS: i32 = 200;

/// A word of free text shorter than this is not a key.
const MIN_WORD: usize = 3;

/// A note is keyed by at most this many of its longest words.
const NOTE_WORDS: usize = 8;

/// The Julian Day Number of 1 January of year 1 (proleptic Gregorian), for turning a day into a year.
const JDN_YEAR_ONE: f64 = 1_721_426.0;

/// Days in a mean Gregorian year.
const MEAN_YEAR: f64 = 365.2425;

/// The kinds of record the engine matches (ADR 0038 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchableKind {
    /// A person.
    Person,
    /// A family.
    Family,
    /// An event.
    Event,
    /// A place.
    Place,
    /// A source.
    Source,
    /// A repository.
    Repository,
    /// A citation.
    Citation,
    /// A media object.
    Media,
    /// A note.
    Note,
    /// A tag.
    Tag,
}

impl MatchableKind {
    /// Every matchable kind.
    pub const ALL: [Self; 10] = [
        Self::Person,
        Self::Family,
        Self::Event,
        Self::Place,
        Self::Source,
        Self::Repository,
        Self::Citation,
        Self::Media,
        Self::Note,
        Self::Tag,
    ];

    /// The kind's aggregate type name, as the event store spells it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Family => "family",
            Self::Event => "event",
            Self::Place => "place",
            Self::Source => "source",
            Self::Repository => "repository",
            Self::Citation => "citation",
            Self::Media => "media",
            Self::Note => "note",
            Self::Tag => "tag",
        }
    }

    /// The kind of an aggregate type name, or `None` for a kind that is not matched.
    #[must_use]
    pub fn parse(aggregate_type: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == aggregate_type)
    }
}

/// The lookup side of a record's keys: the keys it meets exactly, and the key prefixes it meets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    /// Keys a candidate must hold one of exactly.
    pub exact: Vec<String>,
    /// Prefixes a candidate's key may start with.
    pub prefixes: Vec<String>,
}

impl Probe {
    /// The probe of a record indexed under `keys`.
    #[must_use]
    pub fn of(keys: &[String]) -> Self {
        let (mut exact, mut prefixes) = (BTreeSet::new(), BTreeSet::new());
        for key in keys {
            let Some((base, mark, qualifier)) = split(key) else {
                exact.insert(key.clone());
                continue;
            };
            if qualifier == UNKNOWN {
                prefixes.insert(format!("{base}{mark}"));
                continue;
            }
            exact.insert(format!("{base}{mark}{UNKNOWN}"));
            match (mark, qualifier.parse::<i32>()) {
                (DECADE, Ok(decade)) => {
                    for near in decade - 1..=decade + 1 {
                        exact.insert(format!("{base}{mark}{near}"));
                    }
                }
                _ => {
                    exact.insert(key.clone());
                }
            }
        }
        Self {
            exact: exact.into_iter().collect(),
            prefixes: prefixes.into_iter().collect(),
        }
    }

    /// Whether a record holding `key` is a candidate for this probe.
    #[must_use]
    pub fn meets(&self, key: &str) -> bool {
        self.exact.iter().any(|exact| exact == key)
            || self.prefixes.iter().any(|prefix| key.starts_with(prefix.as_str()))
    }
}

/// A key split into its base, qualifier mark and qualifier, or `None` for an unqualified key.
fn split(key: &str) -> Option<(&str, char, &str)> {
    let at = key.rfind([DECADE, PAGE])?;
    let mark = key[at..].chars().next()?;
    Some((&key[..at], mark, &key[at + mark.len_utf8()..]))
}

/// The keying rules over the installed packs.
#[derive(Debug, Clone)]
pub struct BlockingKeys<'d> {
    /// The normalizations a name is keyed under: every pack at once, `universal` alone, and `universal`
    /// with each other pack — so a token is keyed as any comparison could normalize it.
    normalizations: Vec<Applied<'d>>,
    fingerprint: String,
}

impl<'d> BlockingKeys<'d> {
    /// The keying rules over every pack in `data`.
    #[must_use]
    pub fn new(data: &'d MatchData) -> Self {
        let universal: Vec<&CulturePack> = data
            .packs
            .iter()
            .filter(|pack| pack.id().as_str() == "universal")
            .collect();
        let others: Vec<&CulturePack> = data
            .packs
            .iter()
            .filter(|pack| pack.id().as_str() != "universal")
            .collect();
        let mut every = universal.clone();
        every.extend(others.iter().copied());
        let mut normalizations = vec![Applied::new(every), Applied::new(universal.clone())];
        for pack in &others {
            let mut pair = universal.clone();
            pair.push(pack);
            normalizations.push(Applied::new(pair));
        }
        Self {
            normalizations,
            fingerprint: fingerprint(data),
        }
    }

    /// Identifies the keying rules and the pack set the keys were made under.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// A person's keys: each given name and surname by the birth decade, then its origins and external ids.
    #[must_use]
    pub fn person(&self, profile: &PersonProfile) -> Vec<String> {
        let mut keys = BTreeSet::new();
        self.person_into(profile, &mut keys);
        identity(&profile.origins, &profile.external_ids, &mut keys);
        keys.into_iter().collect()
    }

    /// A family's keys: its partners' person keys.
    #[must_use]
    pub fn family(&self, profile: &FamilyProfile) -> Vec<String> {
        let mut keys = BTreeSet::new();
        for partner in &profile.partners {
            self.person_into(partner, &mut keys);
        }
        identity(&profile.origins, &profile.external_ids, &mut keys);
        keys.into_iter().collect()
    }

    /// An event's keys: its principals' given names and its place's names, by the event's decade.
    #[must_use]
    pub fn event(&self, profile: &EventProfile) -> Vec<String> {
        let mut bases = BTreeSet::new();
        for participant in &profile.participants {
            if is_principal(&participant.role) {
                self.given_bases(&participant.person.names, &mut bases);
            }
        }
        if let Some(place) = &profile.place {
            self.place_bases(place, &mut bases);
        }
        if bases.is_empty() {
            bases.insert("any".to_owned());
        }
        let decades = profile.date.as_ref().map(decades_of).unwrap_or_default();
        let mut keys = BTreeSet::new();
        qualify(&bases, &decades, &mut keys);
        identity(&profile.origins, &[], &mut keys);
        keys.into_iter().collect()
    }

    /// A place's keys: the tokens of every name.
    #[must_use]
    pub fn place(&self, profile: &PlaceProfile) -> Vec<String> {
        let mut keys = BTreeSet::new();
        self.place_bases(profile, &mut keys);
        identity(&profile.origins, &[], &mut keys);
        keys.into_iter().collect()
    }

    /// A source's keys: the words of its title.
    #[must_use]
    pub fn source(&self, profile: &SourceProfile) -> Vec<String> {
        let mut keys = BTreeSet::new();
        self.source_bases(profile, &mut keys);
        identity(&profile.origins, &[], &mut keys);
        keys.into_iter().collect()
    }

    /// A repository's keys: the words of its name.
    #[must_use]
    pub fn repository(&self, profile: &RepositoryProfile) -> Vec<String> {
        let mut keys = BTreeSet::new();
        if let Some(name) = &profile.name {
            self.word_bases(name, &mut keys);
        }
        identity(&profile.origins, &[], &mut keys);
        keys.into_iter().collect()
    }

    /// A citation's keys: its source — the workspace source's id, and its title's words — by the words
    /// of the page.
    #[must_use]
    pub fn citation(&self, profile: &CitationProfile) -> Vec<String> {
        let mut bases = BTreeSet::new();
        if let Some(source) = &profile.source {
            if let Some(id) = &source.id {
                bases.insert(format!("src:{id}"));
            }
            self.source_bases(source, &mut bases);
        }
        if bases.is_empty() {
            bases.insert("any".to_owned());
        }
        let mut pages = BTreeSet::new();
        if let Some(page) = &profile.page {
            for applied in &self.normalizations {
                pages.extend(applied.tokens(page));
            }
        }
        let mut keys = BTreeSet::new();
        for base in &bases {
            if pages.is_empty() {
                keys.insert(format!("{base}{PAGE}{UNKNOWN}"));
            }
            for page in &pages {
                keys.insert(format!("{base}{PAGE}{page}"));
            }
        }
        identity(&profile.origins, &[], &mut keys);
        keys.into_iter().collect()
    }

    /// A media object's keys: its checksum, and the words of its file name.
    #[must_use]
    pub fn media(&self, profile: &MediaProfile) -> Vec<String> {
        let mut keys = BTreeSet::new();
        if let Some(checksum) = &profile.checksum {
            keys.insert(format!("sum:{}", checksum.trim().to_lowercase()));
        }
        if let Some(path) = &profile.path {
            let text = match path {
                MediaPath::File(file) => file.clone(),
                MediaPath::Web(url) => url.href.clone(),
            };
            let stem = text.rsplit(['/', '\\']).next().unwrap_or(&text);
            let stem = stem.rsplit_once('.').map_or(stem, |(stem, _)| stem);
            self.word_bases(&stem.replace(['_', '.'], " "), &mut keys);
        }
        identity(&profile.origins, &[], &mut keys);
        keys.into_iter().collect()
    }

    /// A note's keys: its longest words.
    #[must_use]
    pub fn note(&self, profile: &NoteProfile) -> Vec<String> {
        let mut keys = BTreeSet::new();
        if let Some(text) = &profile.text {
            let mut words: Vec<String> = self.normalizations[0].tokens(text);
            words.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then_with(|| a.cmp(b)));
            words.dedup();
            for word in words.iter().take(NOTE_WORDS) {
                self.word_bases(word, &mut keys);
            }
        }
        identity(&profile.origins, &[], &mut keys);
        keys.into_iter().collect()
    }

    /// A tag's key: its case-folded name.
    #[must_use]
    pub fn tag(&self, profile: &TagProfile) -> Vec<String> {
        vec![format!("tag:{}", fold(profile.name.trim()))]
    }

    /// A person's name keys, by the birth decade, into `keys`.
    fn person_into(&self, profile: &PersonProfile, keys: &mut BTreeSet<String>) {
        let mut bases = BTreeSet::new();
        self.given_bases(&profile.names, &mut bases);
        let mut surnames = BTreeSet::new();
        for name in &profile.names {
            for surname in &name.surnames {
                self.token_bases(&surname.surname, &mut surnames);
            }
        }
        bases.extend(surnames.into_iter().map(|base| format!("s{base}")));
        qualify(&bases, &birth_decades(&profile.vitals), keys);
    }

    /// The name bases of every given name in `names`.
    fn given_bases(&self, names: &[PersonName], bases: &mut BTreeSet<String>) {
        for name in names {
            for given in [&name.given, &name.call_name, &name.nickname].into_iter().flatten() {
                self.token_bases(given, bases);
            }
        }
    }

    /// The name bases of every name of a place.
    fn place_bases(&self, place: &PlaceProfile, bases: &mut BTreeSet<String>) {
        for name in &place.names {
            self.token_bases(&name.text, bases);
        }
    }

    /// The word bases of a source's title.
    fn source_bases(&self, source: &SourceProfile, bases: &mut BTreeSet<String>) {
        if let Some(title) = &source.title {
            self.word_bases(title, bases);
        }
    }

    /// The name bases of every token of `text`: the token, its phonetic keys and its classes.
    fn token_bases(&self, text: &str, bases: &mut BTreeSet<String>) {
        for applied in &self.normalizations {
            for token in applied.tokens(text) {
                bases.insert(format!("t:{token}"));
                phonetic_bases(&token, bases);
                for class in applied.classes_of(&token) {
                    bases.insert(format!("c:{class}"));
                }
            }
        }
    }

    /// The name bases of the words of free text, without particles.
    fn word_bases(&self, text: &str, bases: &mut BTreeSet<String>) {
        for applied in &self.normalizations {
            for word in applied.tokens(text) {
                if word.chars().count() < MIN_WORD && !word.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                bases.insert(format!("t:{word}"));
                phonetic_bases(&word, bases);
            }
        }
    }
}

/// A token's phonetic key and, when it is long enough, every key one letter shorter.
fn phonetic_bases(token: &str, bases: &mut BTreeSet<String>) {
    let key: Vec<char> = phonetic_key(token).chars().collect();
    if key.is_empty() {
        return;
    }
    bases.insert(format!("p:{}", key.iter().collect::<String>()));
    if key.len() < MIN_DELETION {
        return;
    }
    for skip in 0..key.len() {
        let shorter: String = key
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != skip)
            .map(|(_, c)| c)
            .collect();
        bases.insert(format!("p:{shorter}"));
    }
}

/// Each base qualified by each decade, or by the unknown decade when there is none.
fn qualify(bases: &BTreeSet<String>, decades: &[i32], keys: &mut BTreeSet<String>) {
    for base in bases {
        if decades.is_empty() {
            keys.insert(format!("{base}{DECADE}{UNKNOWN}"));
        }
        for decade in decades {
            keys.insert(format!("{base}{DECADE}{decade}"));
        }
    }
}

/// The origin and external-id keys of a record.
fn identity(origins: &[RecordOrigin], external_ids: &[ExternalId], keys: &mut BTreeSet<String>) {
    for origin in origins {
        let item = origin.item.as_deref().unwrap_or("");
        keys.insert(format!("o:{}|{}|{item}", origin.dataset, origin.record));
    }
    for id in external_ids {
        keys.insert(format!("x:{}|{}", id.authority, id.value));
    }
}

/// The decades the birth estimate — the birth, or the baptism standing in for it — spans.
fn birth_decades(vitals: &[VitalEvent]) -> Vec<i32> {
    estimate(vitals, VitalKind::Birth).map_or_else(Vec::new, |estimate| {
        span_decades(year_of_day(estimate.interval.lo), year_of_day(estimate.interval.hi))
    })
}

/// The decades a date spans.
fn decades_of(date: &GenealogicalDate) -> Vec<i32> {
    interval(date).map_or_else(Vec::new, |days| {
        span_decades(year_of_day(days.lo), year_of_day(days.hi))
    })
}

/// The decades from the year `lo` to the year `hi`, or none when the span says nothing.
fn span_decades(lo: i32, hi: i32) -> Vec<i32> {
    if hi - lo > MAX_SPAN_YEARS {
        return Vec::new();
    }
    (lo.div_euclid(10)..=hi.div_euclid(10)).collect()
}

/// The (approximate, proleptic Gregorian) year a Julian Day Number falls in. Keys and probes use the
/// same conversion, and a probe reaches a decade either side, so a day's error at a year's edge is
/// harmless.
fn year_of_day(day: i32) -> i32 {
    let years = (f64::from(day) - JDN_YEAR_ONE) / MEAN_YEAR;
    // A Julian Day Number within i32 gives a year well within i32.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the year of an i32 day number fits an i32"
    )]
    let year = years.floor() as i32;
    year + 1
}

/// The fingerprint of the keying rules and the packs: FNV-1a over the rules' version and every pack's
/// parsed rules, in id order. It detects a change; it is not a security digest.
fn fingerprint(data: &MatchData) -> String {
    let mut text = format!("keys-v{KEYS_VERSION}\n");
    for pack in data.packs.iter() {
        // Writing to a String cannot fail.
        let _ = writeln!(text, "{pack:?}");
    }
    format!("{:016x}", fnv1a(text.as_bytes()))
}

/// The 64-bit FNV-1a hash of `bytes`.
fn fnv1a(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests;
