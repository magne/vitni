//! `genealogy-gedcom` — pure GEDCOM parse/emit over a small intermediate model.
//!
//! This crate holds the format logic of the GEDCOM import/export plugins, free of any WASM or host
//! types, so it is unit-tested and linted through the normal `cargo --workspace` path. The wasm glue
//! crates (`plugins/gedcom-*`) depend on it and only bridge its [`Tree`] to the host capabilities.
//!
//! The supported subset is deliberately minimal (roadmap Spike C): `INDI` + `NAME` → individual;
//! `FAM` + `HUSB`/`WIFE`/`CHIL` → family. Broader GEDCOM 7 / Gramps XML mapping is Phase 4 (ADR 0013).

mod emit;
mod model;
mod parse;

pub use emit::emit;
pub use model::{Family, Individual, Tree};
pub use parse::{GedcomError, parse};

#[cfg(test)]
mod tests {
    use super::{Family, Individual, Tree, emit, parse};

    fn sample() -> Tree {
        Tree {
            individuals: vec![
                Individual {
                    xref: "I0001".to_owned(),
                    uid: Some("D02D344F-F781-4337-BCF1-0A1A1A548280".to_owned()),
                    given: Some("John".to_owned()),
                    surname: Some("Smith".to_owned()),
                },
                Individual {
                    xref: "I0002".to_owned(),
                    uid: None,
                    given: Some("Jane".to_owned()),
                    surname: Some("Doe".to_owned()),
                },
                Individual {
                    xref: "I0003".to_owned(),
                    uid: Some("A673BB63-328E-4F79-B4E3-ABCF43460749".to_owned()),
                    given: Some("Sam".to_owned()),
                    surname: Some("Smith".to_owned()),
                },
            ],
            families: vec![Family {
                xref: "F0001".to_owned(),
                uid: Some("11111111-2222-3333-4444-555555555555".to_owned()),
                partners: vec!["I0001".to_owned(), "I0002".to_owned()],
                children: vec!["I0003".to_owned()],
            }],
        }
    }

    #[test]
    fn parses_individuals_names_and_family_structure() {
        let text = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME John /Smith/
0 @I2@ INDI
1 NAME Jane /Doe/
0 @F1@ FAM
1 HUSB @I1@
1 WIFE @I2@
1 CHIL @I3@
0 TRLR
";
        let tree = parse(text).expect("parse");
        assert_eq!(tree.individuals.len(), 2);
        assert_eq!(tree.individuals[0].given.as_deref(), Some("John"));
        assert_eq!(tree.individuals[0].surname.as_deref(), Some("Smith"));
        assert_eq!(tree.families.len(), 1);
        assert_eq!(tree.families[0].partners, vec!["I1".to_owned(), "I2".to_owned()]);
        assert_eq!(tree.families[0].children, vec!["I3".to_owned()]);
    }

    #[test]
    fn round_trips_through_emit_and_parse() {
        let tree = sample();
        let reparsed = parse(&emit(&tree)).expect("reparse");
        assert_eq!(reparsed, tree, "emit then parse must reproduce the tree");
    }

    #[test]
    fn captures_the_stable_uid_on_individuals_and_families() {
        let text = "\
0 @I37@ INDI
1 NAME Magne /Rasmussen/
1 _UID A673BB63-328E-4F79-B4E3-ABCF43460749
0 @F12@ FAM
1 HUSB @I37@
1 _UID 11111111-2222-3333-4444-555555555555
0 TRLR
";
        let tree = parse(text).expect("parse");
        assert_eq!(
            tree.individuals[0].uid.as_deref(),
            Some("A673BB63-328E-4F79-B4E3-ABCF43460749")
        );
        assert_eq!(
            tree.families[0].uid.as_deref(),
            Some("11111111-2222-3333-4444-555555555555")
        );
    }

    #[test]
    fn handles_a_given_only_name() {
        let tree = parse("0 @I1@ INDI\n1 NAME Madonna\n0 TRLR\n").expect("parse");
        assert_eq!(tree.individuals[0].given.as_deref(), Some("Madonna"));
        assert_eq!(tree.individuals[0].surname, None);
    }

    #[test]
    fn an_empty_document_parses_to_an_empty_tree() {
        assert_eq!(parse("").expect("parse"), Tree::default());
        assert_eq!(parse("0 HEAD\n0 TRLR\n").expect("parse"), Tree::default());
    }

    #[test]
    fn skips_unknown_tags_but_keeps_the_record() {
        let tree = parse("0 @I1@ INDI\n1 SEX M\n1 NOTE ignored\n0 TRLR\n").expect("parse");
        assert_eq!(tree.individuals.len(), 1);
        assert_eq!(tree.individuals[0].given, None);
    }

    #[test]
    fn tolerates_crlf_line_endings() {
        let tree = parse("0 @I1@ INDI\r\n1 NAME Ada /Lovelace/\r\n0 TRLR\r\n").expect("parse");
        assert_eq!(tree.individuals[0].surname.as_deref(), Some("Lovelace"));
    }

    #[test]
    fn rejects_a_line_without_a_numeric_level() {
        assert!(parse("INDI\n").is_err());
    }

    #[test]
    fn strips_a_leading_utf8_bom() {
        let tree = parse("\u{feff}0 @I1@ INDI\n1 NAME Ada /Lovelace/\n0 TRLR\n").expect("parse");
        assert_eq!(tree.individuals.len(), 1);
        assert_eq!(tree.individuals[0].surname.as_deref(), Some("Lovelace"));
    }
}
