//! Per-instance host state and the capability implementations behind the WIT interfaces.
//!
//! Each capability checks [`Grants`] before acting (deny-by-default, ADR 0011 §2) and drives the
//! `genealogy-app` use-cases through a [`Session`] whose operator is `AgentKind::Software`, so every
//! plugin-authored change is audited as a Software operator (ADR 0007 §7).
//!
//! The bulk capabilities (`progress`, `import-source`, `export-sink`, ADR 0013) let a long-running
//! import/export report how far it has advanced and stream its document through a host-owned file
//! handle. The host owns the path: a plugin reads from the source the frontend selected and writes
//! to a destination the host resolves from the plugin's suggested file name. One source and one sink
//! back each instance — a plugin runs exactly one import or one export.

use std::fs::File;
use std::io::{Read, Write};

use genealogy_app::{
    Address, AssociationRole, Calendar, DateInput, DateModifier, DatePoint, DateQuality, ExternalId, FactType,
    GenealogicalDate, GenealogicalDateBody, NameType, NewCitation, NewEvent, NewMedia, NewNote, NewPerson, NewPlace,
    NewSource, PersonNameParts, Session, Workspace, build_genealogical_date,
};
use genealogy_core::enums::{EventType, EvidenceLevel, ParticipantRole, PlaceType, Restriction, Sex};
use wasmtime::StoreLimits;
use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

use crate::bindings::imports::genealogy::host_api::{
    commands, export_sink, import_source, log, progress, query, types,
};
use crate::capability::{Capability, Grants};
use crate::{BulkIo, ProgressControl, ProgressUpdate};

/// The data owned by one plugin instance's Wasmtime store.
pub struct HostState {
    wasi: WasiCtx,
    table: ResourceTable,
    /// Memory/instance caps enforced by Wasmtime (ADR 0011 §4).
    pub limits: StoreLimits,
    grants: Grants,
    workspace: Workspace,
    session: Session,
    /// The bulk source/sink configuration and progress sink (ADR 0013).
    io: BulkIo,
    /// The opened import source, set by `import-source.open`.
    source: Option<File>,
    /// The opened export sink, set by `export-sink.open`.
    sink: Option<File>,
}

impl HostState {
    /// Builds instance state. `wasi` is the (empty) WASI context that denies ambient `files`/`net`
    /// by construction (ADR 0011 §3); `limits` is the memory cap; `io` carries the bulk source/sink
    /// and progress sink (ADR 0013).
    pub fn new(
        wasi: WasiCtx,
        limits: StoreLimits,
        grants: Grants,
        workspace: Workspace,
        session: Session,
        io: BulkIo,
    ) -> Self {
        Self {
            wasi,
            table: ResourceTable::new(),
            limits,
            grants,
            workspace,
            session,
            io,
            source: None,
            sink: None,
        }
    }

    /// Recovers the workspace once the instance has run (the store is consumed afterwards).
    pub fn into_workspace(self) -> Workspace {
        self.workspace
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// Maps an application error onto the capability-error a guest sees. Domain rejections and missing
/// references are `invalid-input`; infrastructure failures are `backend`.
fn to_capability_error(error: &genealogy_app::AppError) -> types::CapabilityError {
    use genealogy_app::AppError;
    match error {
        AppError::Db(_) | AppError::Config(_) | AppError::Workspace(_) => {
            types::CapabilityError::Backend(error.to_string())
        }
        _ => types::CapabilityError::InvalidInput(error.to_string()),
    }
}

impl types::Host for HostState {}

impl log::Host for HostState {
    async fn log(&mut self, level: log::Level, message: String) {
        if !self.grants.allows(Capability::Log) {
            return;
        }
        match level {
            log::Level::Debug => tracing::debug!(target: "plugin", "{message}"),
            log::Level::Info => tracing::info!(target: "plugin", "{message}"),
            log::Level::Warn => tracing::warn!(target: "plugin", "{message}"),
            log::Level::Error => tracing::error!(target: "plugin", "{message}"),
        }
    }
}

impl commands::Host for HostState {
    async fn create_person(
        &mut self,
        name: Option<types::PersonName>,
        external_id: Option<types::ExternalId>,
    ) -> Result<types::ImportResult, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        let name = name.map(to_person_name);
        // Without an external identity, always create (no record to resolve against).
        let Some(external_id) = external_id else {
            let new = NewPerson {
                human_id: None,
                name,
                evidence_level: EvidenceLevel::Persona,
            };
            let human_id = genealogy_app::create_person(&self.workspace, &self.session, new)
                .await
                .map_err(|error| to_capability_error(&error))?;
            return Ok(types::ImportResult {
                human_id,
                created: true,
            });
        };
        // Resolve-or-create against the external id (idempotent, additive re-import).
        genealogy_app::import_person(&self.workspace, &self.session, to_external_id(external_id), name)
            .await
            .map(|(human_id, created)| types::ImportResult { human_id, created })
            .map_err(|error| to_capability_error(&error))
    }

    async fn create_family(
        &mut self,
        external_id: Option<types::ExternalId>,
    ) -> Result<types::ImportResult, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        let Some(external_id) = external_id else {
            let human_id = genealogy_app::create_family(&self.workspace, &self.session)
                .await
                .map_err(|error| to_capability_error(&error))?;
            return Ok(types::ImportResult {
                human_id,
                created: true,
            });
        };
        genealogy_app::import_family(&self.workspace, &self.session, to_external_id(external_id))
            .await
            .map(|(human_id, created)| types::ImportResult { human_id, created })
            .map_err(|error| to_capability_error(&error))
    }

    async fn add_partner(&mut self, family: String, person: String) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::import_add_partner(&self.workspace, &self.session, &family, &person)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn add_child(&mut self, family: String, child: String) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::import_add_child(&self.workspace, &self.session, &family, &child)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn assert_sex(&mut self, person: String, sex: types::Sex) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::assert_sex(&self.workspace, &self.session, &person, to_sex(sex))
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn assert_fact(
        &mut self,
        person: String,
        fact: types::FactType,
        value: Option<String>,
        date: Option<types::GenealogicalDate>,
    ) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        let date = date.map(to_genealogical_date);
        let new = genealogy_app::NewFact {
            fact_type: to_fact_type(fact),
            value,
            date,
        };
        genealogy_app::assert_fact(
            &self.workspace,
            &self.session,
            &person,
            new,
            genealogy_app::Provenance::default(),
            &[],
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }

    async fn assert_association(
        &mut self,
        person: String,
        other: String,
        role: types::AssociationRole,
    ) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::assert_association(
            &self.workspace,
            &self.session,
            &person,
            &other,
            to_association_role(role),
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }

    async fn create_place(&mut self, name: String) -> Result<String, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::create_place(
            &self.workspace,
            &self.session,
            NewPlace {
                human_id: None,
                // GEDCOM `PLAC` carries no granularity; record it as a custom type.
                place_type: PlaceType::Custom("place".to_owned()),
                name: Some(name),
            },
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }

    async fn create_event(&mut self, kind: types::EventType) -> Result<String, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::create_event(
            &self.workspace,
            &self.session,
            NewEvent {
                human_id: None,
                event_type: to_event_type(kind),
            },
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }

    async fn set_event_date(
        &mut self,
        event: String,
        date: types::GenealogicalDate,
    ) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        let date = to_genealogical_date(date);
        genealogy_app::assert_event_date_value(&self.workspace, &self.session, &event, date)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_event_address(
        &mut self,
        event: String,
        address: types::Address,
    ) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::assert_event_address(&self.workspace, &self.session, &event, to_address(address))
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn link_event_place(&mut self, event: String, place: String) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::link_place(&self.workspace, &self.session, &event, &place)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn add_event_participant(
        &mut self,
        person: String,
        event: String,
        role: types::ParticipantRole,
    ) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::assert_participation(&self.workspace, &self.session, &person, &event, to_role(role))
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn create_source(&mut self, title: Option<String>) -> Result<String, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::create_source(&self.workspace, &self.session, NewSource { human_id: None, title })
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn create_citation(
        &mut self,
        source: String,
        page: Option<String>,
    ) -> Result<String, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::create_citation(
            &self.workspace,
            &self.session,
            NewCitation {
                human_id: None,
                source,
                page,
            },
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }

    async fn create_media(&mut self, file: Option<String>) -> Result<String, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::create_media(
            &self.workspace,
            &self.session,
            NewMedia {
                human_id: None,
                path: file,
            },
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }

    async fn create_note(&mut self, text: String) -> Result<String, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        genealogy_app::create_note(
            &self.workspace,
            &self.session,
            NewNote {
                human_id: None,
                text: Some(text),
            },
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }

    async fn create_repository(&mut self, name: String) -> Result<String, types::CapabilityError> {
        self.guard()?;
        genealogy_app::create_repository(
            &self.workspace,
            &self.session,
            genealogy_app::NewRepository {
                human_id: None,
                name: Some(name),
            },
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }

    async fn create_tag(&mut self, name: String) -> Result<String, types::CapabilityError> {
        self.guard()?;
        genealogy_app::create_tag(&self.workspace, &self.session, name)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn attach_person_citation(&mut self, person: String, citation: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::add_person_citation(&self.workspace, &self.session, &person, &citation)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn attach_person_media(&mut self, person: String, media: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::attach_person_media(&self.workspace, &self.session, &person, &media)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn attach_person_note(&mut self, person: String, note: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::attach_person_note(&self.workspace, &self.session, &person, &note)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn attach_family_citation(&mut self, family: String, citation: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::add_family_citation(&self.workspace, &self.session, &family, &citation)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn attach_family_media(&mut self, family: String, media: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::attach_family_media(&self.workspace, &self.session, &family, &media)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn attach_family_note(&mut self, family: String, note: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::attach_family_note(&self.workspace, &self.session, &family, &note)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn attach_event_citation(&mut self, event: String, citation: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::add_event_citation(&self.workspace, &self.session, &event, &citation)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn attach_event_media(&mut self, event: String, media: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::import_attach_event_media(&self.workspace, &self.session, &event, &media)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn attach_event_note(&mut self, event: String, note: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::import_attach_event_note(&self.workspace, &self.session, &event, &note)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn apply_person_tag(&mut self, person: String, tag: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::tag_person(&self.workspace, &self.session, &person, parse_tag_id(&tag)?, false)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn apply_family_tag(&mut self, family: String, tag: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::tag_family(&self.workspace, &self.session, &family, &tag, false)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn apply_event_tag(&mut self, event: String, tag: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::tag_event(&self.workspace, &self.session, &event, &tag, false)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_source_author(&mut self, source: String, author: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::set_source_author(&self.workspace, &self.session, &source, author)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_source_pub_info(&mut self, source: String, pub_info: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::set_source_pub_info(&self.workspace, &self.session, &source, pub_info)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn link_source_repository(
        &mut self,
        source: String,
        repository: String,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        // A bulk import links a repository without a call number or medium; both default.
        genealogy_app::link_source_repository(
            &self.workspace,
            &self.session,
            &source,
            &repository,
            None,
            genealogy_core::enums::SourceMediaType::Custom(String::new()),
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }

    async fn set_citation_confidence(
        &mut self,
        citation: String,
        confidence: types::Confidence,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::set_citation_confidence(&self.workspace, &self.session, &citation, to_confidence(confidence))
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_place_type(
        &mut self,
        place: String,
        place_type: types::PlaceType,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::set_place_type(&self.workspace, &self.session, &place, to_place_type(place_type))
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_place_enclosed_by(&mut self, place: String, enclosing: String) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::assert_place_enclosed_by(&self.workspace, &self.session, &place, &enclosing)
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_person_restrictions(
        &mut self,
        person: String,
        restrictions: Vec<types::Restriction>,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::person::set_restrictions(&self.workspace, &self.session, &person, to_restrictions(restrictions))
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_family_restrictions(
        &mut self,
        family: String,
        restrictions: Vec<types::Restriction>,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::family::set_restrictions(&self.workspace, &self.session, &family, to_restrictions(restrictions))
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_event_restrictions(
        &mut self,
        event: String,
        restrictions: Vec<types::Restriction>,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::event::set_restrictions(&self.workspace, &self.session, &event, to_restrictions(restrictions))
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_source_restrictions(
        &mut self,
        source: String,
        restrictions: Vec<types::Restriction>,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::source::set_restrictions(&self.workspace, &self.session, &source, to_restrictions(restrictions))
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_citation_restrictions(
        &mut self,
        citation: String,
        restrictions: Vec<types::Restriction>,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::citation::set_restrictions(
            &self.workspace,
            &self.session,
            &citation,
            to_restrictions(restrictions),
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }

    async fn set_media_restrictions(
        &mut self,
        media: String,
        restrictions: Vec<types::Restriction>,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::media::set_restrictions(&self.workspace, &self.session, &media, to_restrictions(restrictions))
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_note_restrictions(
        &mut self,
        note: String,
        restrictions: Vec<types::Restriction>,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::note::set_restrictions(&self.workspace, &self.session, &note, to_restrictions(restrictions))
            .await
            .map_err(|error| to_capability_error(&error))
    }

    async fn set_repository_restrictions(
        &mut self,
        repository: String,
        restrictions: Vec<types::Restriction>,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::repository::set_restrictions(
            &self.workspace,
            &self.session,
            &repository,
            to_restrictions(restrictions),
        )
        .await
        .map_err(|error| to_capability_error(&error))
    }

    async fn set_place_restrictions(
        &mut self,
        place: String,
        restrictions: Vec<types::Restriction>,
    ) -> Result<(), types::CapabilityError> {
        self.guard()?;
        genealogy_app::place::set_restrictions(&self.workspace, &self.session, &place, to_restrictions(restrictions))
            .await
            .map_err(|error| to_capability_error(&error))
    }
}

impl HostState {
    /// Rejects a `commands` call when the instance lacks the [`Capability::Commands`] grant.
    fn guard(&self) -> Result<(), types::CapabilityError> {
        if self.grants.allows(Capability::Commands) {
            Ok(())
        } else {
            Err(types::CapabilityError::Denied)
        }
    }
}

/// Parses a tag id string into a [`TagId`](genealogy_core::ids::TagId).
fn parse_tag_id(id: &str) -> Result<genealogy_core::ids::TagId, types::CapabilityError> {
    uuid::Uuid::parse_str(id)
        .map(genealogy_core::ids::TagId::from_uuid)
        .map_err(|_| types::CapabilityError::InvalidInput(format!("invalid tag id: {id}")))
}

/// Maps a WIT `restriction` list onto the domain restriction set (GEDCOM `RESN` — data-model §6).
fn to_restrictions(restrictions: Vec<types::Restriction>) -> std::collections::BTreeSet<Restriction> {
    restrictions
        .into_iter()
        .map(|restriction| match restriction {
            types::Restriction::Confidential => Restriction::Confidential,
            types::Restriction::Locked => Restriction::Locked,
            types::Restriction::Privacy => Restriction::Privacy,
        })
        .collect()
}

/// Maps the domain restriction set back onto the WIT `restriction` list (for the read DTO).
fn from_restrictions(restrictions: &std::collections::BTreeSet<Restriction>) -> Vec<types::Restriction> {
    restrictions
        .iter()
        .map(|restriction| match restriction {
            Restriction::Confidential => types::Restriction::Confidential,
            Restriction::Locked => types::Restriction::Locked,
            Restriction::Privacy => types::Restriction::Privacy,
        })
        .collect()
}

/// Maps the WIT `sex` enum onto the domain [`Sex`] (data-model §10).
fn to_sex(sex: types::Sex) -> Sex {
    match sex {
        types::Sex::Male => Sex::Male,
        types::Sex::Female => Sex::Female,
        types::Sex::Unknown => Sex::Unknown,
        types::Sex::Intersex => Sex::Intersex,
    }
}

/// Maps the domain [`Sex`] back onto the WIT `sex` enum (for the read DTO an exporter uses). A
/// `Sex::Other` custom value has no enum slot and is reported as `unknown`.
fn from_sex(sex: &Sex) -> types::Sex {
    match sex {
        Sex::Male => types::Sex::Male,
        Sex::Female => types::Sex::Female,
        Sex::Intersex => types::Sex::Intersex,
        Sex::Unknown | Sex::Other(_) => types::Sex::Unknown,
    }
}

/// Maps the WIT `event-type` enum onto the domain [`EventType`] (data-model §10).
fn to_event_type(kind: types::EventType) -> EventType {
    match kind {
        types::EventType::Birth => EventType::Birth,
        types::EventType::Death => EventType::Death,
        types::EventType::Marriage => EventType::Marriage,
        types::EventType::Baptism => EventType::Baptism,
        types::EventType::Christening => EventType::Christening,
        types::EventType::Burial => EventType::Burial,
        types::EventType::Cremation => EventType::Cremation,
        types::EventType::Census => EventType::Census,
        types::EventType::Residence => EventType::Residence,
        types::EventType::Immigration => EventType::Immigration,
        types::EventType::Emigration => EventType::Emigration,
        types::EventType::Adoption => EventType::Adoption,
        types::EventType::Confirmation => EventType::Confirmation,
        types::EventType::BarMitzvah => EventType::BarMitzvah,
        types::EventType::BasMitzvah => EventType::BasMitzvah,
        types::EventType::FirstCommunion => EventType::FirstCommunion,
        types::EventType::Graduation => EventType::Graduation,
        types::EventType::Naturalization => EventType::Naturalization,
        types::EventType::Ordination => EventType::Ordination,
        types::EventType::Probate => EventType::Probate,
        types::EventType::Retirement => EventType::Retirement,
        types::EventType::Will => EventType::Will,
        types::EventType::Engagement => EventType::Engagement,
        types::EventType::Annulment => EventType::Annulment,
        types::EventType::Divorce => EventType::Divorce,
        types::EventType::DivorceFiled => EventType::DivorceFiled,
        types::EventType::MarriageBanns => EventType::MarriageBanns,
        types::EventType::MarriageContract => EventType::MarriageContract,
        types::EventType::MarriageLicense => EventType::MarriageLicense,
        types::EventType::MarriageSettlement => EventType::MarriageSettlement,
    }
}

/// Maps the WIT `participant-role` enum onto the domain [`ParticipantRole`] (data-model §10).
fn to_role(role: types::ParticipantRole) -> ParticipantRole {
    match role {
        types::ParticipantRole::Primary => ParticipantRole::Primary,
        types::ParticipantRole::Witness => ParticipantRole::Witness,
        types::ParticipantRole::Officiator => ParticipantRole::Officiator,
        types::ParticipantRole::Clergy => ParticipantRole::Clergy,
        types::ParticipantRole::Father => ParticipantRole::Father,
        types::ParticipantRole::Mother => ParticipantRole::Mother,
        types::ParticipantRole::Parent => ParticipantRole::Parent,
        types::ParticipantRole::Child => ParticipantRole::Child,
        types::ParticipantRole::Husband => ParticipantRole::Husband,
        types::ParticipantRole::Wife => ParticipantRole::Wife,
        types::ParticipantRole::Spouse => ParticipantRole::Spouse,
        types::ParticipantRole::Godparent => ParticipantRole::Godparent,
        types::ParticipantRole::Friend => ParticipantRole::Friend,
        types::ParticipantRole::Neighbour => ParticipantRole::Neighbour,
        types::ParticipantRole::Multiple => ParticipantRole::Multiple,
        types::ParticipantRole::Bride => ParticipantRole::Bride,
        types::ParticipantRole::Groom => ParticipantRole::Groom,
    }
}

/// Maps the WIT `name-type` variant onto the domain [`NameType`] (data-model §7).
fn to_name_type(name_type: types::NameType) -> NameType {
    match name_type {
        types::NameType::BirthName => NameType::BirthName,
        types::NameType::MarriedName => NameType::MarriedName,
        types::NameType::Maiden => NameType::Maiden,
        types::NameType::Immigrant => NameType::Immigrant,
        types::NameType::Professional => NameType::Professional,
        types::NameType::AlsoKnownAs => NameType::AlsoKnownAs,
        types::NameType::ReligiousName => NameType::ReligiousName,
        types::NameType::Custom(value) => NameType::Custom(value),
    }
}

/// Maps the domain [`NameType`] back onto the WIT `name-type` (for the read DTO an exporter uses).
fn from_name_type(name_type: NameType) -> types::NameType {
    match name_type {
        NameType::BirthName => types::NameType::BirthName,
        NameType::MarriedName => types::NameType::MarriedName,
        NameType::Maiden => types::NameType::Maiden,
        NameType::Immigrant => types::NameType::Immigrant,
        NameType::Professional => types::NameType::Professional,
        NameType::AlsoKnownAs => types::NameType::AlsoKnownAs,
        NameType::ReligiousName => types::NameType::ReligiousName,
        NameType::Custom(value) => types::NameType::Custom(value),
    }
}

/// Maps the WIT `person-name` record onto the application [`PersonNameParts`] (data-model §7).
fn to_person_name(name: types::PersonName) -> PersonNameParts {
    PersonNameParts {
        name_type: to_name_type(name.name_type),
        given: name.given,
        surname_prefix: name.surname_prefix,
        surname: name.surname,
        nickname: name.nickname,
        prefix: name.prefix,
        suffix: name.suffix,
    }
}

/// Maps the WIT `fact-type` variant onto the domain [`FactType`] (data-model §7).
fn to_fact_type(fact: types::FactType) -> FactType {
    match fact {
        types::FactType::Birth => FactType::Birth,
        types::FactType::Death => FactType::Death,
        types::FactType::Baptism => FactType::Baptism,
        types::FactType::Burial => FactType::Burial,
        types::FactType::Occupation => FactType::Occupation,
        types::FactType::Residence => FactType::Residence,
        types::FactType::Religion => FactType::Religion,
        types::FactType::Caste => FactType::Caste,
        types::FactType::PhysicalDescription => FactType::PhysicalDescription,
        types::FactType::Education => FactType::Education,
        types::FactType::Ethnicity => FactType::Ethnicity,
        types::FactType::NationalId => FactType::NationalId,
        types::FactType::Nationality => FactType::Nationality,
        types::FactType::NumberOfChildren => FactType::NumberOfChildren,
        types::FactType::NumberOfMarriages => FactType::NumberOfMarriages,
        types::FactType::Property => FactType::Property,
        types::FactType::SocialSecurityNumber => FactType::SocialSecurityNumber,
        types::FactType::NobilityTitle => FactType::NobilityTitle,
        types::FactType::Custom(value) => FactType::Custom(value),
    }
}

/// Maps the WIT `association-role` variant onto the domain [`AssociationRole`] (data-model §7).
fn to_association_role(role: types::AssociationRole) -> AssociationRole {
    match role {
        types::AssociationRole::Clergy => AssociationRole::Clergy,
        types::AssociationRole::Friend => AssociationRole::Friend,
        types::AssociationRole::Godparent => AssociationRole::Godparent,
        types::AssociationRole::Neighbour => AssociationRole::Neighbour,
        types::AssociationRole::Officiator => AssociationRole::Officiator,
        types::AssociationRole::Witness => AssociationRole::Witness,
        types::AssociationRole::Child => AssociationRole::Child,
        types::AssociationRole::Father => AssociationRole::Father,
        types::AssociationRole::Mother => AssociationRole::Mother,
        types::AssociationRole::Parent => AssociationRole::Parent,
        types::AssociationRole::Husband => AssociationRole::Husband,
        types::AssociationRole::Wife => AssociationRole::Wife,
        types::AssociationRole::Spouse => AssociationRole::Spouse,
        types::AssociationRole::Multiple => AssociationRole::Multiple,
        types::AssociationRole::Custom(value) => AssociationRole::Custom(value),
    }
}

/// Maps the WIT `address` record onto the domain [`Address`] (data-model §7).
fn to_address(address: types::Address) -> Address {
    Address {
        lines: address.lines,
        locality: address.locality,
        region: address.region,
        postal_code: address.postal_code,
        country: address.country,
        phone: address.phone,
        email: address.email,
        fax: address.fax,
        www: address.www,
        original_text: address.original_text,
    }
}

/// Maps a WIT `date-point` onto the domain [`DatePoint`].
fn to_date_point(point: types::DatePoint) -> DatePoint {
    DatePoint {
        year: point.year,
        month: point.month,
        day: point.day,
    }
}

/// Maps the WIT `genealogical-date` record onto a domain [`GenealogicalDate`], computing the sort key
/// via [`build_genealogical_date`].
fn to_genealogical_date(date: types::GenealogicalDate) -> GenealogicalDate {
    let calendar = match date.calendar {
        types::DateCalendar::Gregorian => Calendar::Gregorian,
        types::DateCalendar::Julian => Calendar::Julian,
        types::DateCalendar::Hebrew => Calendar::Hebrew,
        types::DateCalendar::FrenchRepublican => Calendar::FrenchRepublican,
        types::DateCalendar::Islamic => Calendar::Islamic,
        types::DateCalendar::Swedish => Calendar::Swedish,
    };
    let quality = match date.quality {
        types::DateQuality::Normal => DateQuality::Normal,
        types::DateQuality::Estimated => DateQuality::Estimated,
        types::DateQuality::Calculated => DateQuality::Calculated,
    };
    let body = match date.modifier {
        types::DateModifier::Exact(point) => GenealogicalDateBody::Structured(DateModifier::None(to_date_point(point))),
        types::DateModifier::Before(point) => {
            GenealogicalDateBody::Structured(DateModifier::Before(to_date_point(point)))
        }
        types::DateModifier::After(point) => {
            GenealogicalDateBody::Structured(DateModifier::After(to_date_point(point)))
        }
        types::DateModifier::About(point) => {
            GenealogicalDateBody::Structured(DateModifier::About(to_date_point(point)))
        }
        types::DateModifier::Range(range) => GenealogicalDateBody::Structured(DateModifier::Range {
            start: to_date_point(range.start),
            end: to_date_point(range.end),
        }),
        types::DateModifier::Span(range) => GenealogicalDateBody::Structured(DateModifier::Span {
            start: to_date_point(range.start),
            end: to_date_point(range.end),
        }),
        types::DateModifier::FromDate(point) => {
            GenealogicalDateBody::Structured(DateModifier::From(to_date_point(point)))
        }
        types::DateModifier::ToDate(point) => GenealogicalDateBody::Structured(DateModifier::To(to_date_point(point))),
        types::DateModifier::Interpreted(interpreted) => GenealogicalDateBody::Structured(DateModifier::Interpreted {
            date: to_date_point(interpreted.date),
            phrase: interpreted.phrase,
        }),
        types::DateModifier::TextOnly(text) => GenealogicalDateBody::TextOnly { text },
    };
    build_genealogical_date(DateInput {
        calendar,
        quality,
        body,
        new_year_begins: date.new_year_begins,
        original_text: date.original_text,
    })
}

/// Maps the WIT `external-id` record onto the domain [`ExternalId`] (data-model §11).
fn to_external_id(external_id: types::ExternalId) -> ExternalId {
    ExternalId {
        authority: external_id.authority,
        value: external_id.value,
        kind: external_id.kind,
        url: external_id.url,
    }
}

/// Maps the domain [`EventType`] back onto the WIT `event-type` enum. A [`EventType::Custom`] value
/// has no enum slot and is reported as `None`.
fn from_event_type(event_type: &EventType) -> Option<types::EventType> {
    let mapped = match event_type {
        EventType::Birth => types::EventType::Birth,
        EventType::Death => types::EventType::Death,
        EventType::Marriage => types::EventType::Marriage,
        EventType::Baptism => types::EventType::Baptism,
        EventType::Christening => types::EventType::Christening,
        EventType::Burial => types::EventType::Burial,
        EventType::Cremation => types::EventType::Cremation,
        EventType::Census => types::EventType::Census,
        EventType::Residence => types::EventType::Residence,
        EventType::Immigration => types::EventType::Immigration,
        EventType::Emigration => types::EventType::Emigration,
        EventType::Adoption => types::EventType::Adoption,
        EventType::Confirmation => types::EventType::Confirmation,
        EventType::BarMitzvah => types::EventType::BarMitzvah,
        EventType::BasMitzvah => types::EventType::BasMitzvah,
        EventType::FirstCommunion => types::EventType::FirstCommunion,
        EventType::Graduation => types::EventType::Graduation,
        EventType::Naturalization => types::EventType::Naturalization,
        EventType::Ordination => types::EventType::Ordination,
        EventType::Probate => types::EventType::Probate,
        EventType::Retirement => types::EventType::Retirement,
        EventType::Will => types::EventType::Will,
        EventType::Engagement => types::EventType::Engagement,
        EventType::Annulment => types::EventType::Annulment,
        EventType::Divorce => types::EventType::Divorce,
        EventType::DivorceFiled => types::EventType::DivorceFiled,
        EventType::MarriageBanns => types::EventType::MarriageBanns,
        EventType::MarriageContract => types::EventType::MarriageContract,
        EventType::MarriageLicense => types::EventType::MarriageLicense,
        EventType::MarriageSettlement => types::EventType::MarriageSettlement,
        EventType::Custom(_) => return None,
    };
    Some(mapped)
}

/// Maps the domain [`ParticipantRole`] back onto the WIT `participant-role` enum. A custom role has
/// no enum slot and is reported as `None`.
fn from_role(role: &ParticipantRole) -> Option<types::ParticipantRole> {
    let mapped = match role {
        ParticipantRole::Primary => types::ParticipantRole::Primary,
        ParticipantRole::Witness => types::ParticipantRole::Witness,
        ParticipantRole::Officiator => types::ParticipantRole::Officiator,
        ParticipantRole::Clergy => types::ParticipantRole::Clergy,
        ParticipantRole::Father => types::ParticipantRole::Father,
        ParticipantRole::Mother => types::ParticipantRole::Mother,
        ParticipantRole::Parent => types::ParticipantRole::Parent,
        ParticipantRole::Child => types::ParticipantRole::Child,
        ParticipantRole::Husband => types::ParticipantRole::Husband,
        ParticipantRole::Wife => types::ParticipantRole::Wife,
        ParticipantRole::Spouse => types::ParticipantRole::Spouse,
        ParticipantRole::Godparent => types::ParticipantRole::Godparent,
        ParticipantRole::Friend => types::ParticipantRole::Friend,
        ParticipantRole::Neighbour => types::ParticipantRole::Neighbour,
        ParticipantRole::Multiple => types::ParticipantRole::Multiple,
        ParticipantRole::Bride => types::ParticipantRole::Bride,
        ParticipantRole::Groom => types::ParticipantRole::Groom,
        ParticipantRole::Custom(_) => return None,
    };
    Some(mapped)
}

/// Maps the domain [`FactType`] back onto the WIT `fact-type` variant (for the read DTO an exporter uses).
fn from_fact_type(fact: &FactType) -> types::FactType {
    match fact {
        FactType::Birth => types::FactType::Birth,
        FactType::Death => types::FactType::Death,
        FactType::Baptism => types::FactType::Baptism,
        FactType::Burial => types::FactType::Burial,
        FactType::Occupation => types::FactType::Occupation,
        FactType::Residence => types::FactType::Residence,
        FactType::Religion => types::FactType::Religion,
        FactType::Caste => types::FactType::Caste,
        FactType::PhysicalDescription => types::FactType::PhysicalDescription,
        FactType::Education => types::FactType::Education,
        FactType::Ethnicity => types::FactType::Ethnicity,
        FactType::NationalId => types::FactType::NationalId,
        FactType::Nationality => types::FactType::Nationality,
        FactType::NumberOfChildren => types::FactType::NumberOfChildren,
        FactType::NumberOfMarriages => types::FactType::NumberOfMarriages,
        FactType::Property => types::FactType::Property,
        FactType::SocialSecurityNumber => types::FactType::SocialSecurityNumber,
        FactType::NobilityTitle => types::FactType::NobilityTitle,
        FactType::Custom(value) => types::FactType::Custom(value.clone()),
    }
}

/// Maps the domain [`AssociationRole`] back onto the WIT `association-role` variant.
fn from_association_role(role: &AssociationRole) -> types::AssociationRole {
    match role {
        AssociationRole::Clergy => types::AssociationRole::Clergy,
        AssociationRole::Friend => types::AssociationRole::Friend,
        AssociationRole::Godparent => types::AssociationRole::Godparent,
        AssociationRole::Neighbour => types::AssociationRole::Neighbour,
        AssociationRole::Officiator => types::AssociationRole::Officiator,
        AssociationRole::Witness => types::AssociationRole::Witness,
        AssociationRole::Child => types::AssociationRole::Child,
        AssociationRole::Father => types::AssociationRole::Father,
        AssociationRole::Mother => types::AssociationRole::Mother,
        AssociationRole::Parent => types::AssociationRole::Parent,
        AssociationRole::Husband => types::AssociationRole::Husband,
        AssociationRole::Wife => types::AssociationRole::Wife,
        AssociationRole::Spouse => types::AssociationRole::Spouse,
        AssociationRole::Multiple => types::AssociationRole::Multiple,
        AssociationRole::Custom(value) => types::AssociationRole::Custom(value.clone()),
    }
}

/// Maps the domain [`Address`] back onto the WIT `address` record (for the read DTO an exporter uses).
fn from_address(address: &Address) -> types::Address {
    types::Address {
        lines: address.lines.clone(),
        locality: address.locality.clone(),
        region: address.region.clone(),
        postal_code: address.postal_code.clone(),
        country: address.country.clone(),
        phone: address.phone.clone(),
        email: address.email.clone(),
        fax: address.fax.clone(),
        www: address.www.clone(),
        original_text: address.original_text.clone(),
    }
}

/// Maps a domain [`DatePoint`] back onto a WIT `date-point`.
fn from_date_point(point: &DatePoint) -> types::DatePoint {
    types::DatePoint {
        year: point.year,
        month: point.month,
        day: point.day,
    }
}

/// Maps a domain [`GenealogicalDate`] back onto the WIT `genealogical-date` record — the inverse of
/// [`to_genealogical_date`] (the host-computed sort key is not part of the wire shape).
fn from_genealogical_date(date: &GenealogicalDate) -> types::GenealogicalDate {
    let calendar = match date.calendar {
        Calendar::Gregorian => types::DateCalendar::Gregorian,
        Calendar::Julian => types::DateCalendar::Julian,
        Calendar::Hebrew => types::DateCalendar::Hebrew,
        Calendar::FrenchRepublican => types::DateCalendar::FrenchRepublican,
        Calendar::Islamic => types::DateCalendar::Islamic,
        Calendar::Swedish => types::DateCalendar::Swedish,
    };
    let quality = match date.quality {
        DateQuality::Normal => types::DateQuality::Normal,
        DateQuality::Estimated => types::DateQuality::Estimated,
        DateQuality::Calculated => types::DateQuality::Calculated,
    };
    let modifier = match &date.modifier {
        GenealogicalDateBody::Structured(DateModifier::None(point)) => {
            types::DateModifier::Exact(from_date_point(point))
        }
        GenealogicalDateBody::Structured(DateModifier::Before(point)) => {
            types::DateModifier::Before(from_date_point(point))
        }
        GenealogicalDateBody::Structured(DateModifier::After(point)) => {
            types::DateModifier::After(from_date_point(point))
        }
        GenealogicalDateBody::Structured(DateModifier::About(point)) => {
            types::DateModifier::About(from_date_point(point))
        }
        GenealogicalDateBody::Structured(DateModifier::Range { start, end }) => {
            types::DateModifier::Range(types::DateRange {
                start: from_date_point(start),
                end: from_date_point(end),
            })
        }
        GenealogicalDateBody::Structured(DateModifier::Span { start, end }) => {
            types::DateModifier::Span(types::DateRange {
                start: from_date_point(start),
                end: from_date_point(end),
            })
        }
        GenealogicalDateBody::Structured(DateModifier::From(point)) => {
            types::DateModifier::FromDate(from_date_point(point))
        }
        GenealogicalDateBody::Structured(DateModifier::To(point)) => {
            types::DateModifier::ToDate(from_date_point(point))
        }
        GenealogicalDateBody::Structured(DateModifier::Interpreted { date, phrase }) => {
            types::DateModifier::Interpreted(types::InterpretedDate {
                date: from_date_point(date),
                phrase: phrase.clone(),
            })
        }
        GenealogicalDateBody::TextOnly { text } => types::DateModifier::TextOnly(text.clone()),
    };
    types::GenealogicalDate {
        calendar,
        quality,
        modifier,
        new_year_begins: date.new_year_begins,
        original_text: date.original_text.clone(),
    }
}

/// Maps a domain fact onto the WIT `fact` read record.
fn from_fact(fact: &genealogy_core::fact::Fact) -> types::Fact {
    types::Fact {
        fact_type: from_fact_type(&fact.fact_type),
        value: fact.value.clone(),
        date: fact.date.as_ref().map(from_genealogical_date),
    }
}

impl query::Host for HostState {
    async fn list_persons(&mut self) -> Result<Vec<types::PersonDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let persons = genealogy_app::list_persons(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(persons
            .into_iter()
            .map(|person| types::PersonDto {
                human_id: person.human_id,
                given: person.given,
                surname: person.surname,
                surname_prefix: person.surname_prefix,
                nickname: person.nickname,
                name_prefix: person.name_prefix,
                name_suffix: person.name_suffix,
                name_type: person.name_type.map(from_name_type),
                sex: person.sex.as_ref().map(from_sex),
                facts: person.facts.iter().map(|summary| from_fact(&summary.fact)).collect(),
                associations: person
                    .associations
                    .into_iter()
                    .map(|assoc| types::AssociationRef {
                        other: assoc.other_id,
                        role: from_association_role(&assoc.role),
                    })
                    .collect(),
                participations: person
                    .participations
                    .into_iter()
                    .filter_map(|(event, role)| from_role(&role).map(|role| types::Participation { event, role }))
                    .collect(),
                citations: person.citations,
                media: person.media,
                notes: person.notes,
                tags: person.tags,
                restrictions: from_restrictions(&person.restrictions),
            })
            .collect())
    }

    async fn list_families(&mut self) -> Result<Vec<types::FamilyDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let families = genealogy_app::list_families(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(families
            .into_iter()
            .map(|family| types::FamilyDto {
                human_id: family.human_id,
                partners: family.partners.into_iter().map(|partner| partner.human_id).collect(),
                children: family.children.into_iter().map(|child| child.human_id).collect(),
                citations: family.citations.into_iter().map(|citation| citation.human_id).collect(),
                media: family.media.into_iter().map(|media| media.human_id).collect(),
                notes: family.notes.into_iter().map(|note| note.human_id).collect(),
                tags: family.tags.into_iter().map(|tag| tag.id).collect(),
                restrictions: from_restrictions(&family.restrictions),
            })
            .collect())
    }

    async fn list_events(&mut self) -> Result<Vec<types::EventDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let events = genealogy_app::list_events(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(events
            .into_iter()
            .map(|event| types::EventDto {
                human_id: event.human_id,
                event_type: event.event_type.as_ref().and_then(from_event_type),
                date: event.date.as_ref().map(from_genealogical_date),
                place: event.place.map(|p| p.human_id),
                description: event.description,
                addresses: event.addresses.iter().map(from_address).collect(),
                citations: event.citations.into_iter().map(|c| c.human_id).collect(),
                media: event.media.into_iter().map(|m| m.human_id).collect(),
                notes: event.notes.into_iter().map(|n| n.human_id).collect(),
                tags: event.tags.into_iter().map(|t| t.id).collect(),
                restrictions: from_restrictions(&event.restrictions),
            })
            .collect())
    }

    async fn list_sources(&mut self) -> Result<Vec<types::SourceDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let sources = genealogy_app::list_sources(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(sources
            .into_iter()
            .map(|source| types::SourceDto {
                human_id: source.human_id,
                title: source.title,
                author: source.author,
                pub_info: source.pub_info,
                repositories: source
                    .repositories
                    .into_iter()
                    .filter_map(|link| link.repository.map(|repository| repository.id))
                    .collect(),
                restrictions: from_restrictions(&source.restrictions),
            })
            .collect())
    }

    async fn list_citations(&mut self) -> Result<Vec<types::CitationDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let citations = genealogy_app::list_citations(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(citations
            .into_iter()
            .map(|citation| types::CitationDto {
                human_id: citation.human_id,
                source: citation.source,
                page: citation.page,
                confidence: citation.confidence.map(from_confidence),
                restrictions: from_restrictions(&citation.restrictions),
            })
            .collect())
    }

    async fn list_media(&mut self) -> Result<Vec<types::MediaDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let media = genealogy_app::list_media(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(media
            .into_iter()
            .map(|media| types::MediaDto {
                human_id: media.human_id,
                path: media.path,
                restrictions: from_restrictions(&media.restrictions),
            })
            .collect())
    }

    async fn list_notes(&mut self) -> Result<Vec<types::NoteDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let notes = genealogy_app::list_notes(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(notes
            .into_iter()
            .map(|note| types::NoteDto {
                human_id: note.human_id,
                text: note.text,
                restrictions: from_restrictions(&note.restrictions),
            })
            .collect())
    }

    async fn list_repositories(&mut self) -> Result<Vec<types::RepositoryDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let repositories = genealogy_app::list_repositories(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(repositories
            .into_iter()
            .map(|repository| types::RepositoryDto {
                human_id: repository.human_id,
                name: repository.name,
                restrictions: from_restrictions(&repository.restrictions),
            })
            .collect())
    }

    async fn list_tags(&mut self) -> Result<Vec<types::TagDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let tags = genealogy_app::list_tags(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(tags
            .into_iter()
            .map(|tag| types::TagDto {
                id: tag.id,
                name: tag.name,
            })
            .collect())
    }

    async fn list_places(&mut self) -> Result<Vec<types::PlaceDto>, types::CapabilityError> {
        if !self.grants.allows(Capability::Query) {
            return Err(types::CapabilityError::Denied);
        }
        let places = genealogy_app::list_places(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?;
        Ok(places
            .into_iter()
            .map(|place| types::PlaceDto {
                human_id: place.human_id,
                name: place.names.into_iter().next().map(|n| n.text),
                place_type: place.place_type.map(from_place_type),
                enclosed_by: place.enclosing.into_iter().map(|e| e.human_id).collect(),
                restrictions: from_restrictions(&place.restrictions),
            })
            .collect())
    }
}

/// Maps the WIT `confidence` enum onto the domain [`Confidence`](genealogy_app::Confidence).
fn to_confidence(confidence: types::Confidence) -> genealogy_app::Confidence {
    match confidence {
        types::Confidence::VeryLow => genealogy_app::Confidence::VeryLow,
        types::Confidence::Low => genealogy_app::Confidence::Low,
        types::Confidence::Normal => genealogy_app::Confidence::Normal,
        types::Confidence::High => genealogy_app::Confidence::High,
        types::Confidence::VeryHigh => genealogy_app::Confidence::VeryHigh,
    }
}

/// Maps the domain [`Confidence`](genealogy_app::Confidence) back onto the WIT `confidence` enum.
fn from_confidence(confidence: genealogy_app::Confidence) -> types::Confidence {
    match confidence {
        genealogy_app::Confidence::VeryLow => types::Confidence::VeryLow,
        genealogy_app::Confidence::Low => types::Confidence::Low,
        genealogy_app::Confidence::Normal => types::Confidence::Normal,
        genealogy_app::Confidence::High => types::Confidence::High,
        genealogy_app::Confidence::VeryHigh => types::Confidence::VeryHigh,
    }
}

/// Maps the WIT `place-type` variant onto the domain [`PlaceType`].
fn to_place_type(place_type: types::PlaceType) -> PlaceType {
    match place_type {
        types::PlaceType::Country => PlaceType::Country,
        types::PlaceType::County => PlaceType::County,
        types::PlaceType::Municipality => PlaceType::Municipality,
        types::PlaceType::Parish => PlaceType::Parish,
        types::PlaceType::City => PlaceType::City,
        types::PlaceType::Town => PlaceType::Town,
        types::PlaceType::Village => PlaceType::Village,
        types::PlaceType::Farm => PlaceType::Farm,
        types::PlaceType::Building => PlaceType::Building,
        types::PlaceType::Custom(value) => PlaceType::Custom(value),
    }
}

/// Maps the domain [`PlaceType`] back onto the WIT `place-type` variant.
fn from_place_type(place_type: PlaceType) -> types::PlaceType {
    match place_type {
        PlaceType::Country => types::PlaceType::Country,
        PlaceType::County => types::PlaceType::County,
        PlaceType::Municipality => types::PlaceType::Municipality,
        PlaceType::Parish => types::PlaceType::Parish,
        PlaceType::City => types::PlaceType::City,
        PlaceType::Town => types::PlaceType::Town,
        PlaceType::Village => types::PlaceType::Village,
        PlaceType::Farm => types::PlaceType::Farm,
        PlaceType::Building => types::PlaceType::Building,
        PlaceType::Custom(value) => types::PlaceType::Custom(value),
    }
}

impl progress::Host for HostState {
    async fn report(
        &mut self,
        step: String,
        processed: u32,
        total: Option<u32>,
    ) -> Result<progress::Control, types::CapabilityError> {
        if !self.grants.allows(Capability::Progress) {
            return Err(types::CapabilityError::Denied);
        }
        let control = (self.io.progress)(ProgressUpdate { step, processed, total });
        Ok(match control {
            ProgressControl::Proceed => progress::Control::Proceed,
            ProgressControl::Cancel => progress::Control::Cancel,
        })
    }
}

impl import_source::Host for HostState {
    async fn open(&mut self) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::ImportSource) {
            return Err(types::CapabilityError::Denied);
        }
        let path = self
            .io
            .source
            .as_ref()
            .ok_or_else(|| types::CapabilityError::Backend("no import source is configured".to_owned()))?;
        let file = File::open(path)
            .map_err(|error| types::CapabilityError::Backend(format!("opening {}: {error}", path.display())))?;
        self.source = Some(file);
        Ok(())
    }

    async fn read(&mut self, len: u32) -> Result<Vec<u8>, types::CapabilityError> {
        if !self.grants.allows(Capability::ImportSource) {
            return Err(types::CapabilityError::Denied);
        }
        let file = self
            .source
            .as_mut()
            .ok_or_else(|| types::CapabilityError::Backend("import source is not open".to_owned()))?;
        let mut buffer = vec![0u8; len as usize];
        let read = file
            .read(&mut buffer)
            .map_err(|error| types::CapabilityError::Backend(format!("reading import source: {error}")))?;
        buffer.truncate(read);
        Ok(buffer)
    }
}

impl export_sink::Host for HostState {
    async fn open(&mut self, suggested_name: String) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::ExportSink) {
            return Err(types::CapabilityError::Denied);
        }
        let target = self
            .io
            .sink
            .as_ref()
            .ok_or_else(|| types::CapabilityError::Backend("no export sink is configured".to_owned()))?;
        let path = target
            .resolve(&suggested_name)
            .map_err(types::CapabilityError::Backend)?;
        let file = File::create(&path)
            .map_err(|error| types::CapabilityError::Backend(format!("creating {}: {error}", path.display())))?;
        self.sink = Some(file);
        Ok(())
    }

    async fn write(&mut self, bytes: Vec<u8>) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::ExportSink) {
            return Err(types::CapabilityError::Denied);
        }
        let file = self
            .sink
            .as_mut()
            .ok_or_else(|| types::CapabilityError::Backend("export sink is not open".to_owned()))?;
        file.write_all(&bytes)
            .map_err(|error| types::CapabilityError::Backend(format!("writing export: {error}")))
    }

    async fn finish(&mut self) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::ExportSink) {
            return Err(types::CapabilityError::Denied);
        }
        let file = self
            .sink
            .as_mut()
            .ok_or_else(|| types::CapabilityError::Backend("export sink is not open".to_owned()))?;
        file.flush()
            .map_err(|error| types::CapabilityError::Backend(format!("flushing export: {error}")))
    }
}
