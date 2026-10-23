use super::{Localizer, PlaceError, PlaceKind, PlaceLevel, PlaceSummary, PlaceTypeStatement, fl};

impl Localizer {
    /// `No places yet.`
    #[must_use]
    pub fn place_list_empty(&self) -> String {
        fl!(self.loader, "place-list-empty")
    }

    /// One place line: `P0001  Vågå (Vaage)  type: parish  code: 0515  coords: 60.39,5.32`.
    #[must_use]
    pub fn place_summary_line(&self, summary: &PlaceSummary) -> String {
        let name = if summary.names.is_empty() {
            fl!(self.loader, "no-name")
        } else {
            summary
                .names
                .iter()
                .map(|n| n.text.clone())
                .collect::<Vec<_>>()
                .join(" / ")
        };
        let place_type = match &summary.place_type {
            Some(place_type) => self.place_type(place_type),
            None => fl!(self.loader, "no-value"),
        };
        let code = match &summary.code {
            Some(code) => code.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let coords = match &summary.coordinates {
            Some(coords) => coords.clone(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "place-summary",
            id = summary.human_id.clone(),
            name = name,
            place_type = place_type,
            code = code,
            coords = coords,
            restrictions = self.restrictions_tag(&summary.restrictions)
        )
    }

    /// The localized place-type label: a `std` key by its label, `custom:<text>` as typed, any other key
    /// after the kind (`ecclesiastical · no:prestegjeld`), no key by the kind, else the level, and an
    /// empty statement as "no type" (ADR 0031 §4).
    pub(super) fn place_type(&self, statement: &PlaceTypeStatement) -> String {
        let Some(key) = &statement.type_key else {
            return match (statement.kind, statement.level) {
                (Some(kind), _) => self.place_kind(kind),
                (None, Some(level)) => self.place_level(level),
                (None, None) => fl!(self.loader, "place-type-none"),
            };
        };
        match (key.namespace(), key.name()) {
            ("custom", text) => text.to_owned(),
            ("std", name) => self
                .std_place_type(name)
                .unwrap_or_else(|| self.keyed_place_type(statement, key.as_str())),
            _ => self.keyed_place_type(statement, key.as_str()),
        }
    }

    /// A key with no label of its own, after the statement's kind when it has one.
    fn keyed_place_type(&self, statement: &PlaceTypeStatement, key: &str) -> String {
        match statement.kind {
            Some(kind) => fl!(
                self.loader,
                "place-type-kind-key",
                kind = self.place_kind(kind),
                key = key
            ),
            None => key.to_owned(),
        }
    }

    /// The label of the `std` place type `name`, if the vocabulary has it.
    fn std_place_type(&self, name: &str) -> Option<String> {
        Some(match name {
            "country" => fl!(self.loader, "place-type-country"),
            "state" => fl!(self.loader, "place-type-state"),
            "county" => fl!(self.loader, "place-type-county"),
            "city" => fl!(self.loader, "place-type-city"),
            "parish" => fl!(self.loader, "place-type-parish"),
            "locality" => fl!(self.loader, "place-type-locality"),
            "street" => fl!(self.loader, "place-type-street"),
            "province" => fl!(self.loader, "place-type-province"),
            "region" => fl!(self.loader, "place-type-region"),
            "department" => fl!(self.loader, "place-type-department"),
            "neighborhood" => fl!(self.loader, "place-type-neighborhood"),
            "district" => fl!(self.loader, "place-type-district"),
            "borough" => fl!(self.loader, "place-type-borough"),
            "municipality" => fl!(self.loader, "place-type-municipality"),
            "town" => fl!(self.loader, "place-type-town"),
            "village" => fl!(self.loader, "place-type-village"),
            "hamlet" => fl!(self.loader, "place-type-hamlet"),
            "farm" => fl!(self.loader, "place-type-farm"),
            "building" => fl!(self.loader, "place-type-building"),
            "number" => fl!(self.loader, "place-type-number"),
            _ => return None,
        })
    }

    fn place_kind(&self, kind: PlaceKind) -> String {
        match kind {
            PlaceKind::Administrative => fl!(self.loader, "place-kind-administrative"),
            PlaceKind::Ecclesiastical => fl!(self.loader, "place-kind-ecclesiastical"),
            PlaceKind::Judicial => fl!(self.loader, "place-kind-judicial"),
            PlaceKind::Registration => fl!(self.loader, "place-kind-registration"),
            PlaceKind::Fiscal => fl!(self.loader, "place-kind-fiscal"),
            PlaceKind::Military => fl!(self.loader, "place-kind-military"),
            PlaceKind::Manorial => fl!(self.loader, "place-kind-manorial"),
            PlaceKind::Cadastral => fl!(self.loader, "place-kind-cadastral"),
            PlaceKind::Statistical => fl!(self.loader, "place-kind-statistical"),
            PlaceKind::Settlement => fl!(self.loader, "place-kind-settlement"),
            PlaceKind::Building => fl!(self.loader, "place-kind-building"),
            PlaceKind::Natural => fl!(self.loader, "place-kind-natural"),
        }
    }

    fn place_level(&self, level: PlaceLevel) -> String {
        match level {
            PlaceLevel::Country => fl!(self.loader, "place-level-country"),
            PlaceLevel::Adm1 => fl!(self.loader, "place-level-adm1"),
            PlaceLevel::Adm2 => fl!(self.loader, "place-level-adm2"),
            PlaceLevel::Adm3 => fl!(self.loader, "place-level-adm3"),
            PlaceLevel::Adm4 => fl!(self.loader, "place-level-adm4"),
            PlaceLevel::Adm5 => fl!(self.loader, "place-level-adm5"),
            PlaceLevel::Settlement => fl!(self.loader, "place-level-settlement"),
            PlaceLevel::SubSettlement => fl!(self.loader, "place-level-sub-settlement"),
        }
    }

    pub(super) fn place_error(&self, error: &PlaceError) -> String {
        match error {
            PlaceError::InvalidTypeKey(key) => fl!(self.loader, "err-place-type-key-invalid", key = key.clone()),
            PlaceError::InvalidTypeDate(date) => fl!(self.loader, "err-place-type-date-invalid", date = date.clone()),
            PlaceError::EmptyPlaceType => fl!(self.loader, "err-place-empty-type"),
            PlaceError::TextOnlyPlaceTypeDate => fl!(self.loader, "err-place-text-only-type-date"),
            PlaceError::NotFound(id) => fl!(self.loader, "err-place-not-exist", id = id.to_string()),
            PlaceError::AlreadyExists(id) => fl!(self.loader, "err-place-exists", id = id.to_string()),
            PlaceError::EmptyName => fl!(self.loader, "err-place-empty-name"),
            PlaceError::EmptyCode => fl!(self.loader, "err-place-empty-code"),
            PlaceError::InvalidGeometry => fl!(self.loader, "err-place-invalid-geometry"),
            PlaceError::UnknownPlace(id) => fl!(self.loader, "err-place-unknown", id = id.to_string()),
            PlaceError::EmptySuccessionEndpoints => fl!(self.loader, "err-place-empty-succession-endpoints"),
            PlaceError::SuccessionAnchorMismatch(id) => {
                fl!(self.loader, "err-place-succession-anchor-mismatch", id = id.to_string())
            }
            PlaceError::RetractsMissingAssertion(id) | PlaceError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
            PlaceError::MergeConflict {
                surviving,
                merged,
                reason,
            } => fl!(
                self.loader,
                "err-place-merge-conflict",
                surviving = surviving.to_string(),
                merged = merged.to_string(),
                reason = reason.clone()
            ),
            PlaceError::DistinctFromItself(id) => {
                fl!(self.loader, "err-place-distinct-from-itself", id = id.to_string())
            }
            PlaceError::IdentityDecided { place, other } => fl!(
                self.loader,
                "err-place-identity-decided",
                place = place.to_string(),
                other = other.to_string()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use vitni_app::{PlaceKind, PlaceLevel};

    const EN: [&str; 20] = [
        "country",
        "state",
        "county",
        "city",
        "parish",
        "locality",
        "street",
        "province",
        "region",
        "department",
        "neighborhood",
        "district",
        "borough",
        "municipality",
        "town",
        "village",
        "hamlet",
        "farm",
        "building",
        "number",
    ];

    const NO: [&str; 20] = [
        "land",
        "delstat",
        "grevskap",
        "by",
        "sogn",
        "lokalitet",
        "gate",
        "provins",
        "region",
        "departement",
        "nabolag",
        "distrikt",
        "bydel",
        "kommune",
        "tettsted",
        "landsby",
        "grend",
        "gård",
        "bygning",
        "nummer",
    ];

    fn kebab(name: &str) -> String {
        let mut out = String::new();
        for (n, c) in name.chars().enumerate() {
            if c.is_uppercase() && n > 0 {
                out.push('-');
            }
            out.extend(c.to_lowercase());
        }
        out
    }

    fn messages(path: &str) -> BTreeMap<String, String> {
        let text = std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        let mut found = BTreeMap::new();
        for line in text.lines() {
            let Some((id, value)) = line.split_once(" = ") else {
                continue;
            };
            found.insert(id.trim().to_owned(), value.trim().to_owned());
        }
        found
    }

    fn cli(lang: &str) -> BTreeMap<String, String> {
        messages(&format!("{}/i18n/{lang}/place.ftl", env!("CARGO_MANIFEST_DIR")))
    }

    fn ui(lang: &str) -> BTreeMap<String, String> {
        messages(&format!(
            "{}/../vitni-ui/i18n/{lang}/vitni-ui.ftl",
            env!("CARGO_MANIFEST_DIR")
        ))
    }

    fn vocabulary_ids() -> Vec<String> {
        let mut ids: Vec<String> = EN.iter().map(|name| format!("place-type-{name}")).collect();
        ids.push("place-type-none".to_owned());
        for kind in PlaceKind::ALL {
            ids.push(format!("place-kind-{}", kebab(&format!("{kind:?}"))));
        }
        for level in PlaceLevel::ALL {
            ids.push(format!("place-level-{}", kebab(&format!("{level:?}"))));
        }
        ids
    }

    #[test]
    fn each_catalogue_pins_the_twenty_std_labels() {
        for (lang, expected) in [("en", EN), ("no", NO)] {
            let catalogue = cli(lang);
            for (name, label) in EN.iter().zip(expected) {
                let id = format!("place-type-{name}");
                assert_eq!(catalogue.get(&id).map(String::as_str), Some(label), "{lang} {id}");
            }
        }
    }

    #[test]
    fn every_kind_level_and_the_no_type_label_exist_in_both_languages() {
        for lang in ["en", "no"] {
            let catalogue = cli(lang);
            for id in vocabulary_ids() {
                assert!(catalogue.contains_key(&id), "{lang} lacks {id}");
            }
        }
    }

    #[test]
    fn the_cli_and_the_ui_catalogues_agree_apart_from_case() {
        for lang in ["en", "no"] {
            let (cli, ui) = (cli(lang), ui(lang));
            for id in vocabulary_ids() {
                let (Some(left), Some(right)) = (cli.get(&id), ui.get(&id)) else {
                    panic!(
                        "{lang}: {id} is missing from the CLI ({}) or the UI ({})",
                        cli.contains_key(&id),
                        ui.contains_key(&id)
                    );
                };
                assert_eq!(left.to_lowercase(), right.to_lowercase(), "{lang} {id}");
            }
        }
    }
}
