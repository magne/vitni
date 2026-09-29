//! Name-culture packs (ADR 0038 §5): the name rules of one culture, as data.
//!
//! A pack is a TOML file, `matching/cultures/<id>.toml`. It holds orthographic rewrites, given-name
//! equivalence classes, diminutives, abbreviations and the culture's surname system. A pack whose
//! `cultures` lists two or more other packs is a *cross-culture* pack, mapping names adapted between
//! them; it applies only when all of them do.
//!
//! The shipped packs are embedded here. A workspace (or the shared data directory) overrides a pack by
//! shipping a file with the same name and adds one with a new name — the layering of ADR 0003 — and the
//! app layer, which does the reading, hands the files in as [`PackSource`]s. Parsing is pure.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::matching::CultureId;
use crate::matching::name::fold;

/// A pack or region file handed in by the caller: its name (a file name or path, used for the
/// pack's identity and in errors) and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackSource {
    /// The file name or path, e.g. `no.toml` or `/home/…/matching/cultures/no.toml`.
    pub name: String,
    /// The file's TOML text.
    pub text: String,
}

impl PackSource {
    /// A source from a name and its text.
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
        }
    }
}

/// Why a pack or region file was rejected.
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    /// The file is not valid TOML, or not a valid pack or region table.
    #[error("{name}: {error}")]
    Parse {
        /// The file name or path.
        name: String,
        /// What the parser rejected.
        #[source]
        error: toml::de::Error,
    },
    /// The pack's `id` is not its file name, so layering by name would be ambiguous.
    #[error("{name}: the pack id `{id}` does not match the file name")]
    IdMismatch {
        /// The file name or path.
        name: String,
        /// The id the file declares.
        id: String,
    },
}

/// A culture's surname system: which suffixes form patronymics and whether surnames follow residence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SurnameSystem {
    /// Surnames are patronymics, formed from the father's given name.
    pub patronymic: bool,
    /// Surnames are residence (farm) names, changing when the family moves.
    pub residence: bool,
    /// The male patronymic suffix and its variants; the first is the canonical form.
    pub male_suffixes: Vec<String>,
    /// The female patronymic suffix and its variants; the first is the canonical form.
    pub female_suffixes: Vec<String>,
    /// Definite-article endings of residence names (*Haug-en*).
    pub definite_suffixes: Vec<String>,
}

/// One pack file, as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackFile {
    id: String,
    #[serde(default)]
    languages: Vec<String>,
    #[serde(default)]
    cultures: Vec<String>,
    #[serde(default)]
    rewrites: Vec<(String, String)>,
    #[serde(default)]
    given_classes: Vec<Vec<String>>,
    #[serde(default)]
    abbreviations: BTreeMap<String, String>,
    #[serde(default)]
    diminutives: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    surnames: SurnameSystem,
}

/// A parsed name-culture pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CulturePack {
    id: CultureId,
    languages: Vec<String>,
    cultures: Vec<CultureId>,
    rewrites: Vec<(String, String)>,
    given_classes: Vec<Vec<String>>,
    abbreviations: BTreeMap<String, String>,
    suffix_abbreviations: Vec<(String, String)>,
    surnames: SurnameSystem,
}

impl CulturePack {
    /// Parses a pack from its source. Its `id` must equal the file name without its extension.
    pub fn from_source(source: &PackSource) -> Result<Self, PackError> {
        let file: PackFile = toml::from_str(&source.text).map_err(|error| PackError::Parse {
            name: source.name.clone(),
            error,
        })?;
        if file_stem(&source.name) != file.id {
            return Err(PackError::IdMismatch {
                name: source.name.clone(),
                id: file.id,
            });
        }
        let mut given_classes = file.given_classes;
        for (name, diminutives) in file.diminutives {
            let mut class = vec![name];
            class.extend(diminutives);
            given_classes.push(class);
        }
        let mut abbreviations = BTreeMap::new();
        let mut suffix_abbreviations = Vec::new();
        for (short, long) in file.abbreviations {
            match short.strip_prefix('-') {
                Some(suffix) => suffix_abbreviations.push((fold(suffix), long)),
                None => {
                    abbreviations.insert(fold(&short), long);
                }
            }
        }
        suffix_abbreviations.sort_by_key(|(short, _)| std::cmp::Reverse(short.len()));
        Ok(Self {
            id: CultureId::new(file.id),
            languages: file.languages.iter().map(|tag| tag.to_lowercase()).collect(),
            cultures: file.cultures.into_iter().map(CultureId::new).collect(),
            rewrites: file.rewrites.iter().map(|(from, to)| (fold(from), fold(to))).collect(),
            given_classes,
            abbreviations,
            suffix_abbreviations,
            surnames: file.surnames,
        })
    }

    /// The pack's id.
    #[must_use]
    pub fn id(&self) -> &CultureId {
        &self.id
    }

    /// Whether names tagged with `language` (a BCP-47 tag) select this pack, by primary subtag.
    #[must_use]
    pub fn speaks(&self, language: &str) -> bool {
        let primary = language.split(['-', '_']).next().unwrap_or(language).to_lowercase();
        self.languages.contains(&primary)
    }

    /// The cultures a cross-culture pack bridges; empty for an ordinary pack.
    #[must_use]
    pub fn bridges(&self) -> &[CultureId] {
        &self.cultures
    }

    /// The ordered orthographic rewrites, over folded text.
    pub(crate) fn rewrites(&self) -> &[(String, String)] {
        &self.rewrites
    }

    /// The given-name equivalence classes, diminutives included, as written.
    pub(crate) fn given_classes(&self) -> &[Vec<String>] {
        &self.given_classes
    }

    /// The expansion of a whole abbreviated token (folded, with its full stop), if any.
    pub(crate) fn expand(&self, token: &str) -> Option<String> {
        if let Some(long) = self.abbreviations.get(token) {
            return Some(long.clone());
        }
        for (short, long) in &self.suffix_abbreviations {
            if let Some(stem) = token.strip_suffix(short.as_str())
                && !stem.is_empty()
            {
                return Some(format!("{stem}{long}"));
            }
        }
        None
    }

    /// The culture's surname system.
    #[must_use]
    pub fn surnames(&self) -> &SurnameSystem {
        &self.surnames
    }
}

/// The name of a file without its directory or extension.
fn file_stem(name: &str) -> &str {
    Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name)
}

/// The installed name-culture packs, by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CulturePacks {
    packs: BTreeMap<CultureId, CulturePack>,
}

/// The packs shipped with the engine.
const EMBEDDED: [(&str, &str); 4] = [
    (
        "universal.toml",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/matching/cultures/universal.toml")),
    ),
    (
        "no.toml",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/matching/cultures/no.toml")),
    ),
    (
        "da.toml",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/matching/cultures/da.toml")),
    ),
    (
        "en.toml",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/matching/cultures/en.toml")),
    ),
];

impl CulturePacks {
    /// The packs shipped with the engine: `universal`, `no`, `da` and `en`.
    pub fn embedded() -> Result<Self, PackError> {
        let sources = EMBEDDED.iter().map(|(name, text)| PackSource::new(*name, *text));
        Self { packs: BTreeMap::new() }.layered(sources)
    }

    /// These packs with `sources` layered over them: a source whose file name matches a pack
    /// replaces it, and any other adds a pack.
    pub fn layered(mut self, sources: impl IntoIterator<Item = PackSource>) -> Result<Self, PackError> {
        for source in sources {
            let pack = CulturePack::from_source(&source)?;
            self.packs.insert(pack.id.clone(), pack);
        }
        Ok(self)
    }

    /// The pack with `id`, if installed.
    #[must_use]
    pub fn get(&self, id: &CultureId) -> Option<&CulturePack> {
        self.packs.get(id)
    }

    /// Every installed pack, in id order.
    pub fn iter(&self) -> impl Iterator<Item = &CulturePack> {
        self.packs.values()
    }
}

#[cfg(test)]
mod tests {
    use super::{CulturePacks, PackError, PackSource};
    use crate::matching::CultureId;

    #[test]
    fn every_shipped_pack_parses() {
        let packs = CulturePacks::embedded().unwrap();
        let ids: Vec<&str> = packs.iter().map(|pack| pack.id().as_str()).collect();
        assert_eq!(ids, ["da", "en", "no", "universal"]);
    }

    #[test]
    fn the_norwegian_pack_has_a_patronymic_residence_system() {
        let packs = CulturePacks::embedded().unwrap();
        let no = packs.get(&CultureId::new("no")).unwrap();
        assert!(no.surnames().patronymic && no.surnames().residence);
        assert!(no.speaks("nb-NO") && no.speaks("nn") && !no.speaks("da"));
        assert_eq!(no.expand("olsd."), Some("olsdatter".to_owned()));
        assert_eq!(no.expand("joh."), Some("Johannes".to_owned()));
        assert_eq!(no.expand("ole"), None);
    }

    #[test]
    fn diminutives_join_the_given_classes() {
        let packs = CulturePacks::embedded().unwrap();
        let en = packs.get(&CultureId::new("en")).unwrap();
        assert!(
            en.given_classes()
                .iter()
                .any(|class| class.contains(&"Bill".to_owned()))
        );
    }

    #[test]
    fn a_layered_pack_replaces_by_name_and_adds_by_new_name() {
        let replacement = PackSource::new("/ws/matching/cultures/en.toml", "id = \"en\"\n");
        let toy = PackSource::new("toy.toml", include_str!("../../tests/fixtures/matching/toy.toml"));
        let packs = CulturePacks::embedded().unwrap().layered([replacement, toy]).unwrap();
        let en = packs.get(&CultureId::new("en")).unwrap();
        assert!(
            en.given_classes().is_empty(),
            "the workspace file replaced the embedded pack"
        );
        assert!(packs.get(&CultureId::new("toy")).unwrap().speaks("tq"));
    }

    #[test]
    fn a_malformed_pack_names_its_file() {
        let bad = PackSource::new("/ws/matching/cultures/xx.toml", "id = ");
        let error = CulturePacks::embedded().unwrap().layered([bad]).unwrap_err();
        assert!(matches!(error, PackError::Parse { .. }));
        assert!(error.to_string().starts_with("/ws/matching/cultures/xx.toml: "));
    }

    #[test]
    fn an_unknown_field_is_rejected() {
        let typo = PackSource::new("xx.toml", "id = \"xx\"\ngiven_clases = []\n");
        assert!(matches!(
            CulturePacks::embedded().unwrap().layered([typo]),
            Err(PackError::Parse { .. })
        ));
    }

    #[test]
    fn a_pack_id_must_match_its_file_name() {
        let misnamed = PackSource::new("xx.toml", "id = \"yy\"\n");
        let error = CulturePacks::embedded().unwrap().layered([misnamed]).unwrap_err();
        assert!(matches!(error, PackError::IdMismatch { ref id, .. } if id == "yy"));
    }
}
