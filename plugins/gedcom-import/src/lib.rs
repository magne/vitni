//! GEDCOM import plugin (ADR 0013): read the document from the host-opened import source, parse it
//! with `genealogy-gedcom`, then create persons and families through the host `commands` capability,
//! reporting progress as it goes. The format-neutral plumbing (streaming, progress, logging) lives
//! in `genealogy-plugin-api`; this crate only bridges the GEDCOM [`Tree`](genealogy_gedcom::Tree) to
//! the host capabilities.

wit_bindgen::generate!({
    world: "bulk-import",
    path: "../../crates/genealogy-plugin-host/wit",
    with: {
        "genealogy:host-api/types@0.4.0": genealogy_plugin_api::types,
        "genealogy:host-api/log@0.4.0": genealogy_plugin_api::log,
        "genealogy:host-api/commands@0.4.0": genealogy_plugin_api::commands,
        "genealogy:host-api/progress@0.4.0": genealogy_plugin_api::progress,
        "genealogy:host-api/import-source@0.4.0": genealogy_plugin_api::import_source,
    },
});

use std::collections::HashMap;

use genealogy_plugin_api::commands;
use genealogy_plugin_api::types::ExternalId;

struct Importer;

impl Guest for Importer {
    fn run_import() -> Result<u32, String> {
        let text = genealogy_plugin_api::read_source_to_string()?;
        let tree = genealogy_gedcom::parse(&text).map_err(|error| error.to_string())?;
        let individuals = tree.individuals.len() as u32;
        let families = tree.families.len() as u32;
        genealogy_plugin_api::log_info(&format!("importing {individuals} individuals and {families} families"));

        let mut xref_to_human: HashMap<String, String> = HashMap::new();
        let mut imported: u32 = 0;

        for (index, individual) in tree.individuals.iter().enumerate() {
            let human_id = commands::create_person(
                individual.given.as_deref(),
                individual.surname.as_deref(),
                Some(&external_id(individual.uid.as_deref(), &individual.xref)),
            )
            .map_err(|error| format!("create-person failed: {error:?}"))?;
            xref_to_human.insert(individual.xref.clone(), human_id);
            imported += 1;
            if !genealogy_plugin_api::report("persons", index as u32 + 1, Some(individuals))? {
                return Ok(imported);
            }
        }

        for (index, family) in tree.families.iter().enumerate() {
            let family_id = commands::create_family(Some(&external_id(family.uid.as_deref(), &family.xref)))
                .map_err(|error| format!("create-family failed: {error:?}"))?;
            for partner in &family.partners {
                if let Some(human_id) = xref_to_human.get(partner) {
                    commands::add_partner(&family_id, human_id)
                        .map_err(|error| format!("add-partner failed: {error:?}"))?;
                }
            }
            for child in &family.children {
                if let Some(human_id) = xref_to_human.get(child) {
                    commands::add_child(&family_id, human_id)
                        .map_err(|error| format!("add-child failed: {error:?}"))?;
                }
            }
            imported += 1;
            if !genealogy_plugin_api::report("families", index as u32 + 1, Some(families))? {
                return Ok(imported);
            }
        }

        Ok(imported)
    }
}

/// Builds the external id a record is resolved by on re-import: the stable `_UID` when present
/// (authority `gedcom-uid`), else the per-file cross-reference (authority `gedcom-xref`). Either is
/// stable across re-exports of the same document, so an unchanged record resolves to itself.
fn external_id(uid: Option<&str>, xref: &str) -> ExternalId {
    match uid {
        Some(uid) => ExternalId {
            authority: "gedcom-uid".to_owned(),
            value: uid.to_owned(),
            kind: None,
            url: None,
        },
        None => ExternalId {
            authority: "gedcom-xref".to_owned(),
            value: xref.to_owned(),
            kind: None,
            url: None,
        },
    }
}

export!(Importer);
