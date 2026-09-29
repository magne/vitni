//! Name comparison for record matching (ADR 0038 §4, §5), driven by the applied culture packs.
//!
//! A name is split into tokens, each abbreviation expanded (*Joh.* → *Johannes*), folded (lower case,
//! diacritics stripped) and rewritten by the packs' orthographic rules, and doubled letters are
//! collapsed. Two given names are compared token by token: equal after normalization, in one
//! equivalence class (*Peder* ↔ *Per*), equal by phonetic key, or close by Jaro–Winkler. A surname is
//! compared after its patronymic suffix is reduced to the canonical form (*Olsøn* → *Olsen*), and a
//! residence name also without its definite ending (*Haugen* ↔ *Haug*).

use strsim::jaro_winkler;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::matching::pack::CulturePack;

/// The similarity of two given names in one equivalence class.
const CLASS_SIMILARITY: f64 = 0.95;

/// The similarity of a residence name with and without its definite ending.
const DEFINITE_SIMILARITY: f64 = 0.95;

/// The least similarity two names equal by phonetic key are given.
const PHONETIC_SIMILARITY: f64 = 0.85;

/// A patronymic stem must keep this many letters once its suffix is removed.
const MIN_STEM: usize = 2;

/// Lower-cases `text` and strips its combining diacritics (é → e, å → a).
pub(crate) fn fold(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

/// The culture packs applied to one comparison, `universal` first.
#[derive(Debug, Clone)]
pub(crate) struct Applied<'a> {
    packs: Vec<&'a CulturePack>,
    /// Every applied given-name equivalence class, its members normalized.
    classes: Vec<Vec<String>>,
    /// Per patronymic suffix list (male, then female, pack by pack): the normalized canonical suffix
    /// and its normalized variants, longest first.
    patronymics: Vec<(String, Vec<String>)>,
    /// The normalized definite-article endings of every residence-name culture.
    definite_suffixes: Vec<String>,
}

impl<'a> Applied<'a> {
    /// The rules of `packs`, applied in the order given.
    pub fn new(packs: Vec<&'a CulturePack>) -> Self {
        let mut applied = Self {
            packs,
            classes: Vec::new(),
            patronymics: Vec::new(),
            definite_suffixes: Vec::new(),
        };
        let (mut classes, mut patronymics, mut definite_suffixes) = (Vec::new(), Vec::new(), Vec::new());
        for pack in &applied.packs {
            for class in pack.given_classes() {
                classes.push(class.iter().map(|name| applied.normalize_word(name)).collect());
            }
            let system = pack.surnames();
            if system.patronymic {
                for suffixes in [&system.male_suffixes, &system.female_suffixes] {
                    let Some(first) = suffixes.first() else {
                        continue;
                    };
                    let mut variants: Vec<String> = suffixes.iter().map(|s| applied.normalize_word(s)).collect();
                    variants.sort_by_key(|variant| std::cmp::Reverse(variant.len()));
                    patronymics.push((applied.normalize_word(first), variants));
                }
            }
            if system.residence {
                for suffix in &system.definite_suffixes {
                    definite_suffixes.push(applied.normalize_word(suffix));
                }
            }
        }
        applied.classes = classes;
        applied.patronymics = patronymics;
        applied.definite_suffixes = definite_suffixes;
        applied
    }

    /// Whether any applied culture treats surnames as patronymic or residence names, which makes a
    /// surname mismatch weak evidence rather than a disagreement.
    pub fn surnames_are_weak(&self) -> bool {
        self.packs
            .iter()
            .any(|pack| pack.surnames().patronymic || pack.surnames().residence)
    }

    /// The normalized tokens of `text`.
    pub fn tokens(&self, text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        for raw in text.split(|c: char| c.is_whitespace() || c == '-') {
            let folded = fold(raw);
            let expanded = self.packs.iter().find_map(|pack| pack.expand(&folded));
            for word in expanded.as_deref().unwrap_or(&folded).split_whitespace() {
                let token = self.normalize_word(word);
                if !token.is_empty() {
                    tokens.push(token);
                }
            }
        }
        tokens
    }

    /// One word folded, stripped of punctuation, rewritten and with doubled letters collapsed.
    fn normalize_word(&self, word: &str) -> String {
        let mut text: String = fold(word).chars().filter(|c| c.is_alphanumeric()).collect();
        for pack in &self.packs {
            for (from, to) in pack.rewrites() {
                text = text.replace(from.as_str(), to);
            }
        }
        collapse_doubles(&text)
    }

    /// The similarity in `0..=1` of two given names, or `None` when either is empty.
    pub fn given_similarity(&self, a: &str, b: &str) -> Option<f64> {
        let (a, b) = (self.tokens(a), self.tokens(b));
        let mut best: Option<f64> = None;
        for x in &a {
            for y in &b {
                let similarity = self.token_similarity(x, y);
                best = Some(best.map_or(similarity, |so_far: f64| so_far.max(similarity)));
            }
        }
        best
    }

    /// The similarity of two normalized given-name tokens.
    fn token_similarity(&self, x: &str, y: &str) -> f64 {
        if x == y {
            return 1.0;
        }
        if self.same_class(x, y) {
            return CLASS_SIMILARITY;
        }
        let similarity = jaro_winkler(x, y).max(jaro_winkler(y, x));
        if phonetic_key(x) == phonetic_key(y) {
            similarity.max(PHONETIC_SIMILARITY)
        } else {
            similarity
        }
    }

    /// Whether two normalized tokens share a given-name equivalence class in any applied pack.
    fn same_class(&self, x: &str, y: &str) -> bool {
        self.classes
            .iter()
            .any(|members| members.iter().any(|m| m == x) && members.iter().any(|m| m == y))
    }

    /// The similarity in `0..=1` of two surnames, or `None` when either is empty.
    pub fn surname_similarity(&self, a: &str, b: &str) -> Option<f64> {
        let (a, b) = (self.surname_form(a)?, self.surname_form(b)?);
        if a == b {
            return Some(1.0);
        }
        let mut similarity = jaro_winkler(&a, &b).max(jaro_winkler(&b, &a));
        if self.same_residence(&a, &b) {
            similarity = similarity.max(DEFINITE_SIMILARITY);
        }
        Some(similarity)
    }

    /// A surname normalized, with a patronymic suffix reduced to its culture's canonical form.
    fn surname_form(&self, surname: &str) -> Option<String> {
        let mut tokens = self.tokens(surname);
        let last = tokens.last_mut()?;
        if let Some(canonical) = self.canonical_patronymic(last) {
            *last = canonical;
        }
        Some(tokens.join(" "))
    }

    /// `token` with its patronymic suffix replaced by the canonical suffix of its gender.
    fn canonical_patronymic(&self, token: &str) -> Option<String> {
        for (canonical, variants) in &self.patronymics {
            for variant in variants {
                if let Some(stem) = token.strip_suffix(variant.as_str())
                    && stem.chars().count() >= MIN_STEM
                {
                    return Some(format!("{stem}{canonical}"));
                }
            }
        }
        None
    }

    /// The patronymic key of `surname` — its stem once the patronymic suffix is removed, keyed by
    /// [`patronymic_key`] — or `None` when no applied culture reads it as a patronymic.
    pub fn surname_patronymic_key(&self, surname: &str) -> Option<String> {
        let tokens = self.tokens(surname);
        let last = tokens.last()?;
        for (_, variants) in &self.patronymics {
            for variant in variants {
                if let Some(stem) = last.strip_suffix(variant.as_str())
                    && stem.chars().count() >= MIN_STEM
                {
                    return Some(patronymic_key(stem));
                }
            }
        }
        None
    }

    /// The patronymic keys a father's given name forms: its own, and those of every name in its
    /// equivalence classes (*Ole* also forms *Olavsen*).
    pub fn father_patronymic_keys(&self, given: &str) -> Vec<String> {
        let mut keys = Vec::new();
        for token in self.tokens(given) {
            keys.push(patronymic_key(&token));
            for members in &self.classes {
                if members.contains(&token) {
                    keys.extend(members.iter().map(|member| patronymic_key(member)));
                }
            }
        }
        keys
    }

    /// Whether two residence names differ only by a definite-article ending.
    fn same_residence(&self, a: &str, b: &str) -> bool {
        self.definite_suffixes
            .iter()
            .any(|suffix| a.strip_suffix(suffix.as_str()) == Some(b) || b.strip_suffix(suffix.as_str()) == Some(a))
    }
}

/// `text` with every run of one repeated letter reduced to a single letter.
fn collapse_doubles(text: &str) -> String {
    let mut collapsed = String::with_capacity(text.len());
    for c in text.chars() {
        if !collapsed.ends_with(c) {
            collapsed.push(c);
        }
    }
    collapsed
}

/// The form a given name and a patronymic stem share: without a genitive `s`, then without one final
/// vowel (*Hans* and *Han-sen* → `han`, *Ole* and *Ol-sen* → `ol`).
fn patronymic_key(name: &str) -> String {
    let long_enough = |rest: &str| rest.chars().count() >= MIN_STEM;
    let mut key = name;
    if let Some(rest) = key.strip_suffix('s')
        && long_enough(rest)
    {
        key = rest;
    }
    if let Some(rest) = key.strip_suffix(['a', 'e', 'i', 'o', 'u', 'y'])
        && long_enough(rest)
    {
        key = rest;
    }
    key.to_owned()
}

/// A coarse phonetic key: the first letter, then the consonants, with repeats collapsed.
pub(crate) fn phonetic_key(token: &str) -> String {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut key = String::from(first);
    for c in chars {
        if !"aeiouy".contains(c) {
            key.push(c);
        }
    }
    collapse_doubles(&key)
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use super::{Applied, fold, phonetic_key};
    use crate::matching::CultureId;
    use crate::matching::pack::CulturePacks;

    static PACKS: LazyLock<CulturePacks> = LazyLock::new(|| CulturePacks::embedded().unwrap());

    fn with(ids: &[&str]) -> Applied<'static> {
        Applied::new(ids.iter().map(|id| PACKS.get(&CultureId::new(*id)).unwrap()).collect())
    }

    fn norwegian() -> Applied<'static> {
        with(&["universal", "no", "da"])
    }

    fn universal() -> Applied<'static> {
        with(&["universal"])
    }

    #[test]
    fn folding_lowers_and_strips_diacritics() {
        assert_eq!(fold("Nordås Émile"), "nordas emile");
        assert_eq!(
            fold("Olsøn"),
            "olsøn",
            "ø has no separable diacritic; the universal pack rewrites it"
        );
    }

    #[test]
    fn spelling_variants_normalize_alike() {
        let no = norwegian();
        assert_eq!(no.tokens("Nordaas"), no.tokens("Nordås"));
        assert_eq!(no.tokens("Christian"), no.tokens("Kristian"));
        assert_eq!(no.tokens("Thorvald"), no.tokens("Torvald"));
        assert_eq!(no.tokens("Petter"), no.tokens("Peter"));
    }

    #[test]
    fn abbreviations_expand() {
        let no = norwegian();
        assert_eq!(no.tokens("Joh."), no.tokens("Johannes"));
        assert_eq!(no.tokens("Olsd."), no.tokens("Olsdatter"));
    }

    #[test]
    fn guldbrand_and_gulbrand_are_one_name_in_norway() {
        let similarity = norwegian().given_similarity("Guldbrand", "Gulbrand").unwrap();
        assert!(similarity >= 0.95, "{similarity}");
    }

    #[test]
    fn an_equivalence_class_matches_a_different_spelling() {
        assert!(norwegian().given_similarity("Per", "Peder").unwrap() >= 0.95);
        assert!(universal().given_similarity("Per", "Peder").unwrap() < 0.95);
    }

    #[test]
    fn a_diminutive_matches_only_under_its_culture() {
        let english = with(&["universal", "en"]);
        assert!(english.given_similarity("Bill", "William").unwrap() >= 0.95);
        assert!(norwegian().given_similarity("Bill", "William").unwrap() < 0.75);
    }

    #[test]
    fn the_best_given_token_pair_counts() {
        assert!((norwegian().given_similarity("Ole Johan", "Johan").unwrap() - 1.0).abs() < f64::EPSILON);
        assert_eq!(norwegian().given_similarity("", "Johan"), None);
    }

    #[test]
    fn olsen_and_olson_are_one_patronymic_in_norway() {
        let no = norwegian();
        assert!((no.surname_similarity("Olsen", "Olsøn").unwrap() - 1.0).abs() < f64::EPSILON);
        assert!((no.surname_similarity("Olsdatter", "Olsdotter").unwrap() - 1.0).abs() < f64::EPSILON);
        assert!(universal().surname_similarity("Olsen", "Olsøn").unwrap() < 1.0);
    }

    #[test]
    fn a_patronymic_needs_a_stem() {
        let no = norwegian();
        assert!(no.surname_similarity("Sen", "Søn").unwrap() < 1.0);
    }

    #[test]
    fn haugen_and_haug_are_one_farm() {
        let no = norwegian();
        let similarity = no.surname_similarity("Haugen", "Haug").unwrap();
        assert!((0.95..1.0).contains(&similarity), "{similarity}");
        assert!(no.surnames_are_weak());
        assert!(!universal().surnames_are_weak());
    }

    #[test]
    fn the_phonetic_key_keeps_the_consonants() {
        assert_eq!(phonetic_key("olsen"), "olsn");
        assert_eq!(phonetic_key("maren"), phonetic_key("marin"));
        assert_eq!(phonetic_key(""), "");
    }
}
