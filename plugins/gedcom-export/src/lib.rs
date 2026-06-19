//! GEDCOM export plugin: read persons and families through the host `query` capability, then
//! serialize them to GEDCOM with `genealogy-gedcom`. Human ids become GEDCOM xrefs.

wit_bindgen::generate!({
    world: "gedcom-export",
    path: "../../crates/genealogy-plugin-host/wit",
});

use crate::genealogy::host_api::{log, query};

struct Exporter;

impl Guest for Exporter {
    fn run_export() -> Result<Vec<u8>, String> {
        let persons = query::list_persons().map_err(|error| format!("list-persons failed: {error:?}"))?;
        let families = query::list_families().map_err(|error| format!("list-families failed: {error:?}"))?;
        log::log(
            log::Level::Info,
            &format!("exporting {} individuals and {} families", persons.len(), families.len()),
        );

        let tree = genealogy_gedcom::Tree {
            individuals: persons
                .into_iter()
                .map(|person| genealogy_gedcom::Individual {
                    xref: person.human_id,
                    given: person.given,
                    surname: person.surname,
                })
                .collect(),
            families: families
                .into_iter()
                .map(|family| genealogy_gedcom::Family {
                    xref: family.human_id,
                    partners: family.partners,
                    children: family.children,
                })
                .collect(),
        };

        Ok(genealogy_gedcom::emit(&tree).into_bytes())
    }
}

export!(Exporter);
