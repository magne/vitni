use super::{DnaProvider, DnaTestError, DnaTestSummary, DnaTestType, Localizer, fl};

impl Localizer {
    /// `No DNA tests yet.`
    #[must_use]
    pub fn dna_test_list_empty(&self) -> String {
        fl!(self.loader, "dna-test-list-empty")
    }

    /// One DNA-test line: `D0001  person: <uuid>  provider: 23andMe  type: autosomal  haplogroups: 1`.
    #[must_use]
    pub fn dna_test_summary_line(&self, summary: &DnaTestSummary) -> String {
        let person = match &summary.person {
            Some(person) => person.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let provider = match &summary.provider {
            Some(provider) => self.dna_provider(provider),
            None => fl!(self.loader, "no-value"),
        };
        let test_type = match summary.test_type {
            Some(test_type) => self.dna_test_type(test_type),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "dna-test-summary",
            id = summary.human_id.clone(),
            person = person,
            provider = provider,
            test_type = test_type,
            haplogroups = summary.haplogroup_count.to_string()
        )
    }

    /// The localized DNA-provider label; a custom value renders verbatim.
    fn dna_provider(&self, provider: &DnaProvider) -> String {
        match provider {
            DnaProvider::AncestryDna => fl!(self.loader, "dna-provider-ancestry"),
            DnaProvider::TwentyThreeAndMe => fl!(self.loader, "dna-provider-23andme"),
            DnaProvider::MyHeritage => fl!(self.loader, "dna-provider-myheritage"),
            DnaProvider::FamilyTreeDna => fl!(self.loader, "dna-provider-ftdna"),
            DnaProvider::GedMatch => fl!(self.loader, "dna-provider-gedmatch"),
            DnaProvider::LivingDna => fl!(self.loader, "dna-provider-livingdna"),
            DnaProvider::Custom(value) => value.clone(),
        }
    }

    /// The localized DNA-test-type label.
    fn dna_test_type(&self, test_type: DnaTestType) -> String {
        match test_type {
            DnaTestType::Autosomal => fl!(self.loader, "dna-test-type-autosomal"),
            DnaTestType::YDna => fl!(self.loader, "dna-test-type-ydna"),
            DnaTestType::MtDna => fl!(self.loader, "dna-test-type-mtdna"),
            DnaTestType::XDna => fl!(self.loader, "dna-test-type-xdna"),
        }
    }

    pub(super) fn dna_test_error(&self, error: &DnaTestError) -> String {
        match error {
            DnaTestError::NotFound(id) => fl!(self.loader, "err-dna-test-not-exist", id = id.to_string()),
            DnaTestError::AlreadyExists(id) => fl!(self.loader, "err-dna-test-exists", id = id.to_string()),
            DnaTestError::UnknownPerson(id) => fl!(self.loader, "err-dna-test-unknown-person", id = id.to_string()),
            DnaTestError::RetractsMissingAssertion(id) | DnaTestError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }
}
