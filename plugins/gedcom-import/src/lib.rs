//! GEDCOM import plugin: parse bytes with `genealogy-gedcom`, then create persons and families
//! through the host `commands` capability, mapping GEDCOM xrefs to the assigned human ids.

wit_bindgen::generate!({
    world: "gedcom-import",
    path: "../../crates/genealogy-plugin-host/wit",
});

use std::collections::HashMap;

use crate::genealogy::host_api::{commands, log};

struct Importer;

impl Guest for Importer {
    fn run_import(bytes: Vec<u8>) -> Result<u32, String> {
        let text = String::from_utf8(bytes).map_err(|error| format!("input is not valid UTF-8: {error}"))?;
        let tree = genealogy_gedcom::parse(&text).map_err(|error| error.to_string())?;
        log::log(
            log::Level::Info,
            &format!(
                "importing {} individuals and {} families",
                tree.individuals.len(),
                tree.families.len()
            ),
        );

        let mut xref_to_human: HashMap<String, String> = HashMap::new();
        let mut imported: u32 = 0;

        for individual in &tree.individuals {
            let human_id = commands::create_person(individual.given.as_deref(), individual.surname.as_deref())
                .map_err(|error| format!("create-person failed: {error:?}"))?;
            xref_to_human.insert(individual.xref.clone(), human_id);
            imported += 1;
        }

        for family in &tree.families {
            let family_id = commands::create_family().map_err(|error| format!("create-family failed: {error:?}"))?;
            for partner in &family.partners {
                if let Some(human_id) = xref_to_human.get(partner) {
                    commands::add_partner(&family_id, human_id)
                        .map_err(|error| format!("add-partner failed: {error:?}"))?;
                }
            }
            for child in &family.children {
                if let Some(human_id) = xref_to_human.get(child) {
                    commands::add_child(&family_id, human_id).map_err(|error| format!("add-child failed: {error:?}"))?;
                }
            }
            imported += 1;
        }

        Ok(imported)
    }
}

export!(Importer);
