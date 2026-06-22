//! GEDCOM export plugin (ADR 0013): read persons and families through the host `query` capability,
//! serialize them to GEDCOM with `genealogy-gedcom`, and write the document to the host-resolved
//! export sink, reporting progress. Human ids become GEDCOM xrefs. The format-neutral plumbing lives
//! in `genealogy-plugin-api`; this crate only bridges the DTOs to the GEDCOM
//! [`Tree`](genealogy_gedcom::Tree).

wit_bindgen::generate!({
    world: "bulk-export",
    path: "../../crates/genealogy-plugin-host/wit",
    with: {
        "genealogy:host-api/types@0.6.0": genealogy_plugin_api::types,
        "genealogy:host-api/log@0.6.0": genealogy_plugin_api::log,
        "genealogy:host-api/query@0.6.0": genealogy_plugin_api::query,
        "genealogy:host-api/progress@0.6.0": genealogy_plugin_api::progress,
        "genealogy:host-api/export-sink@0.6.0": genealogy_plugin_api::export_sink,
    },
});

use genealogy_gedcom::{Name, NameKind};
use genealogy_plugin_api::query;
use genealogy_plugin_api::types::{NameType, PersonDto};

struct Exporter;

impl Guest for Exporter {
    fn run_export() -> Result<u32, String> {
        let persons = query::list_persons().map_err(|error| format!("list-persons failed: {error:?}"))?;
        let families = query::list_families().map_err(|error| format!("list-families failed: {error:?}"))?;
        let person_count = persons.len() as u32;
        let family_count = families.len() as u32;
        let total = person_count + family_count;
        genealogy_plugin_api::log_info(&format!(
            "exporting {person_count} individuals and {family_count} families"
        ));

        let tree = genealogy_gedcom::Tree {
            individuals: persons.into_iter().map(individual).collect(),
            families: families
                .into_iter()
                .map(|family| genealogy_gedcom::Family {
                    xref: family.human_id,
                    uid: None,
                    partners: family.partners,
                    children: family.children,
                    events: Vec::new(),
                })
                .collect(),
            sources: Vec::new(),
        };

        if !genealogy_plugin_api::report("serialize", 0, Some(total))? {
            return Ok(0);
        }
        let document = genealogy_gedcom::emit(&tree).into_bytes();
        genealogy_plugin_api::write_export("export.ged", &document)?;
        genealogy_plugin_api::report("written", total, Some(total))?;

        Ok(total)
    }
}

/// Maps a person DTO onto a GEDCOM individual, reconstructing the structured `NAME` from its parts.
fn individual(person: PersonDto) -> genealogy_gedcom::Individual {
    let has_name = person.given.is_some()
        || person.surname.is_some()
        || person.surname_prefix.is_some()
        || person.nickname.is_some()
        || person.name_prefix.is_some()
        || person.name_suffix.is_some();
    let name = has_name.then(|| Name {
        name_type: person.name_type.map(name_kind),
        given: person.given,
        surname_prefix: person.surname_prefix,
        surname: person.surname,
        nickname: person.nickname,
        prefix: person.name_prefix,
        suffix: person.name_suffix,
    });
    genealogy_gedcom::Individual {
        xref: person.human_id,
        uid: None,
        name,
        sex: None,
        events: Vec::new(),
        facts: Vec::new(),
        associations: Vec::new(),
        citations: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
    }
}

/// Maps the host capability's `name-type` onto a GEDCOM name kind.
fn name_kind(name_type: NameType) -> NameKind {
    match name_type {
        NameType::BirthName => NameKind::BirthName,
        NameType::MarriedName => NameKind::MarriedName,
        NameType::Maiden => NameKind::Maiden,
        NameType::Immigrant => NameKind::Immigrant,
        NameType::Professional => NameKind::Professional,
        NameType::AlsoKnownAs => NameKind::AlsoKnownAs,
        NameType::ReligiousName => NameKind::ReligiousName,
        NameType::Custom(value) => NameKind::Other(value),
    }
}

export!(Exporter);
