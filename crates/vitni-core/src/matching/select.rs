//! Name-culture selection (ADR 0038 §5): which packs a comparison applies, chosen from the evidence.
//!
//! A side's cultures come from its places, looked up by country and year in the region table
//! (`matching/regions.toml`), and from its names' data languages. A person's places are those of its
//! vital events and lineage; a family's are its partners' and its marriage's; an event's are its own and
//! its participants' births; a place's is its own; a repository's are its addresses', a source's its
//! repositories' and a citation's its source's. A note's language selects too. A side with
//! no signal gets the workspace's default cultures. A comparison applies `universal`, the union of both
//! sides' cultures, and every cross-culture pack that bridges cultures in that union.

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::date::GenealogicalDate;
use crate::matching::date::year;
use crate::matching::name::fold;
use crate::matching::pack::{PackError, PackSource};
use crate::matching::profile::{
    CitationProfile, EventProfile, FamilyProfile, NoteProfile, PersonProfile, PlaceProfile, RepositoryProfile,
    SourceProfile,
};
use crate::matching::{CultureId, MatchData, MatchSettings};
use crate::name::{LanguageTag, PersonName};

/// The id of the pack every comparison applies.
pub(crate) const UNIVERSAL: &str = "universal";

/// The region table, as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegionsFile {
    #[serde(default, rename = "region")]
    regions: Vec<Region>,
}

/// A country or group of countries sharing name cultures.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Region {
    id: String,
    names: Vec<String>,
    #[serde(default, rename = "period")]
    periods: Vec<Period>,
}

/// The packs a region selects over a span of years.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Period {
    /// The first year of the period, inclusive; open when absent.
    from: Option<i32>,
    /// The year the period ends, exclusive; open when absent.
    until: Option<i32>,
    packs: Vec<String>,
}

impl Period {
    fn covers(&self, year: Option<i32>) -> bool {
        let Some(year) = year else {
            return true;
        };
        self.from.is_none_or(|from| year >= from) && self.until.is_none_or(|until| year < until)
    }
}

/// Which name cultures each region selects, by period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionTable {
    regions: Vec<Region>,
}

/// The region table shipped with the engine.
const EMBEDDED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/matching/regions.toml"));

impl RegionTable {
    /// The region table shipped with the engine.
    pub fn embedded() -> Result<Self, PackError> {
        Self::from_source(&PackSource::new("regions.toml", EMBEDDED))
    }

    /// Parses a region table.
    pub fn from_source(source: &PackSource) -> Result<Self, PackError> {
        let file: RegionsFile = toml::from_str(&source.text).map_err(|error| PackError::Parse {
            name: source.name.clone(),
            error,
        })?;
        let mut regions = file.regions;
        for region in &mut regions {
            region.names = region.names.iter().map(|name| fold(name.trim())).collect();
        }
        Ok(Self { regions })
    }

    /// The packs a place in `country` selects in `year`; every period's packs when the year is unknown.
    fn packs_for(&self, country: &str, year: Option<i32>) -> impl Iterator<Item = CultureId> + '_ {
        let country = fold(country.trim());
        self.regions
            .iter()
            .filter(move |region| region.names.contains(&country))
            .flat_map(move |region| region.periods.iter().filter(move |period| period.covers(year)))
            .flat_map(|period| period.packs.iter().map(CultureId::new))
    }
}

/// The evidence one side offers for choosing its cultures: the countries it is tied to, each with a
/// year, and the data languages of its names.
#[derive(Debug, Default)]
pub(crate) struct Signals<'p> {
    places: Vec<(&'p str, Option<i32>)>,
    languages: Vec<&'p str>,
}

impl<'p> Signals<'p> {
    /// A person's signals: the places of its vital events and lineage, and its names' languages.
    pub fn person(profile: &'p PersonProfile) -> Self {
        let mut signals = Self::default();
        signals.add_person(profile);
        signals
    }

    /// A family's signals: its partners' and its marriage's.
    pub fn family(profile: &'p FamilyProfile) -> Self {
        let mut signals = Self::default();
        for partner in &profile.partners {
            signals.add_person(partner);
        }
        if let Some(marriage) = &profile.marriage {
            signals.add_event(marriage);
        }
        signals
    }

    /// An event's signals: its place and year, and its participants' names and birth places.
    pub fn event(profile: &'p EventProfile) -> Self {
        let mut signals = Self::default();
        signals.add_event(profile);
        signals
    }

    /// A place's signals: its country, and its names' languages.
    pub fn place(profile: &'p PlaceProfile) -> Self {
        let mut signals = Self::default();
        signals.add_place(Some(profile), None);
        signals.languages.extend(
            profile
                .names
                .iter()
                .filter_map(|name| name.language.as_ref().map(LanguageTag::as_str)),
        );
        signals
    }

    /// A repository's signals: the countries of its addresses.
    pub fn repository(profile: &'p RepositoryProfile) -> Self {
        let mut signals = Self::default();
        signals.add_repository(profile);
        signals
    }

    /// A source's signals: its repositories'.
    pub fn source(profile: &'p SourceProfile) -> Self {
        let mut signals = Self::default();
        signals.add_source(profile);
        signals
    }

    /// A citation's signals: its source's.
    pub fn citation(profile: &'p CitationProfile) -> Self {
        let mut signals = Self::default();
        if let Some(source) = &profile.source {
            signals.add_source(source);
        }
        signals
    }

    /// A note's signals: the language of its text.
    pub fn note(profile: &'p NoteProfile) -> Self {
        let mut signals = Self::default();
        signals
            .languages
            .extend(profile.language.as_ref().map(LanguageTag::as_str));
        signals
    }

    fn add_source(&mut self, profile: &'p SourceProfile) {
        for repository in &profile.repositories {
            self.add_repository(repository);
        }
    }

    fn add_repository(&mut self, profile: &'p RepositoryProfile) {
        for address in &profile.addresses {
            if let Some(country) = &address.country {
                self.places.push((country.as_str(), None));
            }
        }
    }

    fn add_person(&mut self, profile: &'p PersonProfile) {
        for vital in &profile.vitals {
            self.add_place(vital.place.as_ref(), vital.date.as_ref());
        }
        for mention in &profile.lineage {
            self.places
                .push((mention.country.as_str(), mention.date.as_ref().and_then(year)));
        }
        self.add_names(&profile.names);
    }

    fn add_event(&mut self, profile: &'p EventProfile) {
        self.add_place(profile.place.as_ref(), profile.date.as_ref());
        for participant in &profile.participants {
            self.add_names(&participant.person.names);
            if let Some(birth) = &participant.person.birth {
                self.add_place(birth.place.as_ref(), birth.date.as_ref());
            }
        }
    }

    fn add_place(&mut self, place: Option<&'p PlaceProfile>, date: Option<&GenealogicalDate>) {
        if let Some(country) = place.and_then(|place| place.country.as_deref()) {
            self.places.push((country, date.and_then(year)));
        }
    }

    fn add_names(&mut self, names: &'p [PersonName]) {
        self.languages.extend(
            names
                .iter()
                .filter_map(|name| name.language.as_ref().map(LanguageTag::as_str)),
        );
    }
}

/// The cultures one side selects by its own evidence, or the default cultures when it has none.
fn side_cultures(signals: &Signals<'_>, data: &MatchData, settings: &MatchSettings) -> BTreeSet<CultureId> {
    let (packs, regions) = (&data.packs, &data.regions);
    let mut selected = BTreeSet::new();
    for (country, year) in &signals.places {
        selected.extend(regions.packs_for(country, *year));
    }
    for language in &signals.languages {
        selected.extend(
            packs
                .iter()
                .filter(|pack| pack.speaks(language))
                .map(|pack| pack.id().clone()),
        );
    }
    selected.retain(|id| id.as_str() != UNIVERSAL && packs.get(id).is_some());
    if selected.is_empty() {
        selected.extend(
            settings
                .default_cultures
                .iter()
                .filter(|id| packs.get(id).is_some())
                .cloned(),
        );
    }
    selected
}

/// The cultures a comparison of `a` and `b` applies: `universal` first, then the union of both sides'
/// cultures and the cross-culture packs bridging them, in id order.
pub(crate) fn comparison_cultures(
    a: &Signals<'_>,
    b: &Signals<'_>,
    data: &MatchData,
    settings: &MatchSettings,
) -> Vec<CultureId> {
    let packs = &data.packs;
    let mut union = side_cultures(a, data, settings);
    union.extend(side_cultures(b, data, settings));
    let bridges: Vec<CultureId> = packs
        .iter()
        .filter(|pack| !pack.bridges().is_empty() && pack.bridges().iter().all(|id| union.contains(id)))
        .map(|pack| pack.id().clone())
        .collect();
    union.extend(bridges);
    let mut cultures = Vec::with_capacity(union.len() + 1);
    let universal = CultureId::new(UNIVERSAL);
    if packs.get(&universal).is_some() {
        cultures.push(universal);
    }
    cultures.extend(union);
    cultures
}

#[cfg(test)]
mod tests {
    use super::{RegionTable, Signals, comparison_cultures};
    use crate::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
    use crate::matching::date::DateBasis;
    use crate::matching::pack::{CulturePacks, PackSource};
    use crate::matching::profile::{PersonProfile, PlaceMention, PlaceProfile, VitalEvent, VitalKind};
    use crate::matching::{CultureId, MatchData, MatchSettings};
    use crate::name::{LanguageTag, NameType, PersonName};

    fn in_year(year: i32) -> GenealogicalDate {
        GenealogicalDate {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
                year: Some(year),
                month: None,
                day: None,
            })),
            time: None,
            new_year_begins: None,
            sort_value: 0,
            original_text: None,
        }
    }

    fn born_in(country: &str, year: i32) -> PersonProfile {
        let place = PlaceProfile {
            country: Some(country.to_owned()),
            ..PlaceProfile::default()
        };
        PersonProfile {
            vitals: vec![VitalEvent {
                kind: VitalKind::Birth,
                date: Some(in_year(year)),
                basis: DateBasis::Recorded,
                place: Some(place),
            }],
            ..PersonProfile::default()
        }
    }

    fn speaking(language: &str) -> PersonProfile {
        let name = PersonName {
            name_type: NameType::BirthName,
            given: Some("Ole".to_owned()),
            surnames: Vec::new(),
            suffix: None,
            title: None,
            nickname: None,
            call_name: None,
            date: None,
            language: Some(LanguageTag::new(language)),
            transliterations: Vec::new(),
        };
        PersonProfile {
            names: vec![name],
            ..PersonProfile::default()
        }
    }

    fn cultures(a: &PersonProfile, b: &PersonProfile, packs: &CulturePacks, settings: &MatchSettings) -> Vec<String> {
        let data = MatchData::new(packs.clone(), RegionTable::embedded().unwrap());
        comparison_cultures(&Signals::person(a), &Signals::person(b), &data, settings)
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    fn embedded() -> CulturePacks {
        CulturePacks::embedded().unwrap()
    }

    #[test]
    fn norway_before_1907_selects_norwegian_and_danish() {
        let a = born_in("Norge", 1850);
        let got = cultures(&a, &a, &embedded(), &MatchSettings::default());
        assert_eq!(got, ["universal", "da", "no"]);
    }

    #[test]
    fn norway_after_1907_selects_norwegian_only() {
        let a = born_in("Norway", 1920);
        assert_eq!(
            cultures(&a, &a, &embedded(), &MatchSettings::default()),
            ["universal", "no"]
        );
    }

    #[test]
    fn a_data_language_selects_its_pack() {
        let got = cultures(
            &speaking("nb-NO"),
            &speaking("en-GB"),
            &embedded(),
            &MatchSettings::default(),
        );
        assert_eq!(got, ["universal", "en", "no"]);
    }

    #[test]
    fn lineage_selects_the_ancestors_cultures() {
        let mut child = born_in("England", 1880);
        child.lineage.push(PlaceMention {
            country: "Danmark".to_owned(),
            date: Some(in_year(1850)),
        });
        assert_eq!(
            cultures(&child, &child, &embedded(), &MatchSettings::default()),
            ["universal", "da", "en"]
        );
    }

    #[test]
    fn no_signal_falls_back_to_the_default_cultures() {
        let bare = PersonProfile::default();
        assert_eq!(
            cultures(&bare, &bare, &embedded(), &MatchSettings::default()),
            ["universal"]
        );
        let settings = MatchSettings {
            default_cultures: vec![CultureId::new("no")],
            ..MatchSettings::default()
        };
        assert_eq!(cultures(&bare, &bare, &embedded(), &settings), ["universal", "no"]);
    }

    #[test]
    fn an_unknown_country_selects_nothing() {
        let a = born_in("Atlantis", 1850);
        assert_eq!(cultures(&a, &a, &embedded(), &MatchSettings::default()), ["universal"]);
    }

    #[test]
    fn a_cross_culture_pack_applies_only_when_it_bridges_both() {
        let bridge = PackSource::new("da-en.toml", "id = \"da-en\"\ncultures = [\"da\", \"en\"]\n");
        let packs = embedded().layered([bridge]).unwrap();
        let settings = MatchSettings::default();
        let got = cultures(&born_in("Danmark", 1850), &born_in("England", 1880), &packs, &settings);
        assert_eq!(got, ["universal", "da", "da-en", "en"]);
        let got = cultures(&born_in("Danmark", 1850), &born_in("Danmark", 1880), &packs, &settings);
        assert_eq!(got, ["universal", "da"]);
    }
}
