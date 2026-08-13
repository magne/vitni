use super::{Localizer, PlaceError, PlaceSummary, PlaceType, fl};

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
            coords = coords
        )
    }

    /// The localized place-type label; a custom [`PlaceType::Custom`] value renders verbatim.
    fn place_type(&self, place_type: &PlaceType) -> String {
        match place_type {
            PlaceType::Country => fl!(self.loader, "place-type-country"),
            PlaceType::County => fl!(self.loader, "place-type-county"),
            PlaceType::Municipality => fl!(self.loader, "place-type-municipality"),
            PlaceType::Parish => fl!(self.loader, "place-type-parish"),
            PlaceType::City => fl!(self.loader, "place-type-city"),
            PlaceType::Town => fl!(self.loader, "place-type-town"),
            PlaceType::Village => fl!(self.loader, "place-type-village"),
            PlaceType::Farm => fl!(self.loader, "place-type-farm"),
            PlaceType::Building => fl!(self.loader, "place-type-building"),
            PlaceType::Custom(value) => value.clone(),
        }
    }

    pub(super) fn place_error(&self, error: &PlaceError) -> String {
        match error {
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
        }
    }
}
