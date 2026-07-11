use super::{PedigreeSlotVm, PedigreeVm, RelationshipVm};
use crate::i18n::Localizer;
use genealogy_app::{
    AncestorNode, AncestorSlot, Confidence, DescendantChart, DescendantNode, Kinship, PedigreeChart, PedigreePersonRef,
    RelationshipResult,
};
use std::collections::BTreeSet;

fn person(human_id: &str, name: &str) -> PedigreePersonRef {
    PedigreePersonRef {
        human_id: human_id.to_owned(),
        id: format!("{human_id}-id"),
        name: Some(name.to_owned()),
        vitals: Some("1850 – 1920".to_owned()),
        restrictions: BTreeSet::new(),
    }
}

fn no_descendants(focus: PedigreePersonRef) -> DescendantChart {
    DescendantChart {
        focus,
        children: Vec::new(),
    }
}

#[test]
fn ancestor_generations_pad_unknown_slots_with_localized_hints() {
    let loc = Localizer::for_test("en");
    let focus = person("I0001", "John Smith");
    let father = AncestorNode {
        person: person("I0002", "Thomas Smith"),
        confidence: Some(Confidence::Normal),
        source_count: 1,
        father: AncestorSlot::Unknown,
        mother: AncestorSlot::Unknown,
    };
    let chart = PedigreeChart {
        focus: focus.clone(),
        father: AncestorSlot::Known(Box::new(father)),
        mother: AncestorSlot::Unknown,
    };
    let vm = PedigreeVm::build(&chart, &no_descendants(focus), 3, &loc);

    assert_eq!(vm.focus.name, "John Smith");
    assert!(vm.focus.confidence.is_none(), "the focus is not itself an assertion");
    assert_eq!(vm.ancestor_generations.len(), 3);
    let PedigreeSlotVm::Known(dad) = &vm.ancestor_generations[0][0] else {
        panic!("father known")
    };
    assert_eq!(dad.name, "Thomas Smith");
    let PedigreeSlotVm::Unknown { hint } = &vm.ancestor_generations[0][1] else {
        panic!("mother unknown")
    };
    assert_eq!(hint, "mother of John Smith");
    // Gen 2 (grandparents): Thomas Smith's own two slots, both unresearched (named) since he has
    // no recorded parents.
    let PedigreeSlotVm::Unknown { hint: paternal_gf } = &vm.ancestor_generations[1][0] else {
        panic!("paternal grandfather unknown")
    };
    assert_eq!(paternal_gf, "father of Thomas Smith");
    // Gen 3: below an unresearched slot, the hint drops the name (a generic "unresearched" form).
    let PedigreeSlotVm::Unknown { hint: generic } = &vm.ancestor_generations[2][0] else {
        panic!("gen 3 slot unknown")
    };
    assert_eq!(generic, "father (line unresearched)");
    assert_eq!(vm.ancestor_generations[0].len(), 2, "gen 1 has 2 slots");
    assert_eq!(vm.ancestor_generations[1].len(), 4, "gen 2 has 4 slots");
    assert_eq!(
        vm.ancestor_generations[2].len(),
        8,
        "gen 3 has 8 slots — the fan stays rectangular"
    );
}

#[test]
fn descendant_generations_are_not_padded() {
    let loc = Localizer::for_test("en");
    let focus = person("I0001", "Grand Parent");
    let child = DescendantNode {
        person: person("I0002", "Mid Parent"),
        confidence: Some(Confidence::Normal),
        source_count: 0,
        children: Vec::new(),
    };
    let tree = DescendantChart {
        focus: focus.clone(),
        children: vec![child],
    };
    let chart = PedigreeChart {
        focus,
        father: AncestorSlot::Unknown,
        mother: AncestorSlot::Unknown,
    };
    let vm = PedigreeVm::build(&chart, &tree, 4, &loc);

    assert_eq!(vm.descendant_generations.len(), 1, "no grandchildren recorded");
    assert_eq!(vm.descendant_generations[0].len(), 1);
    assert_eq!(vm.descendant_generations[0][0].name, "Mid Parent");
    assert_eq!(vm.descendant_generations[0][0].source_count, 0);
}

#[test]
fn relationship_vm_localizes_the_kinship_summary() {
    let loc = Localizer::for_test("en");
    let result = RelationshipResult {
        person_a: person("I0001", "Alice"),
        person_b: person("I0002", "Bob"),
        kinship: Some(Kinship::Sibling { full: true }),
    };
    let vm = RelationshipVm::build(&result, &loc);
    assert_eq!(vm.summary, "Alice and Bob are full siblings.");
}

#[test]
fn relationship_vm_reports_when_no_kinship_is_found() {
    let loc = Localizer::for_test("en");
    let result = RelationshipResult {
        person_a: person("I0001", "Alice"),
        person_b: person("I0002", "Zoe"),
        kinship: None,
    };
    let vm = RelationshipVm::build(&result, &loc);
    assert_eq!(
        vm.summary,
        "No known relationship found within the searched generations."
    );
}
