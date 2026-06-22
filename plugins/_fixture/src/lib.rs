//! Test-only fixture plugin: proves capability gating, the fuel limit, and the memory cap.

wit_bindgen::generate!({
    world: "fixture",
    path: "../../crates/genealogy-plugin-host/wit",
});

use crate::genealogy::host_api::{commands, log, types};

struct Fixture;

impl Guest for Fixture {
    /// Attempts one `commands.create-person` call. Succeeds (returning the new human id) when the
    /// `commands` capability is granted; returns the host's `denied` as an error string otherwise.
    fn try_create() -> Result<String, String> {
        log::log(log::Level::Info, "fixture: attempting create-person");
        let name = types::PersonName {
            name_type: types::NameType::BirthName,
            given: Some("Fixture".to_owned()),
            surname_prefix: None,
            surname: Some("Person".to_owned()),
            nickname: None,
            prefix: None,
            suffix: None,
        };
        commands::create_person(Some(&name), None)
            .map(|result| result.human_id)
            .map_err(|error| format!("{error:?}"))
    }

    /// Spins forever consuming fuel — the host's fuel budget must trap this (ADR 0011 §4).
    fn busy_loop() {
        let mut counter: u64 = 0;
        loop {
            counter = counter.wrapping_add(1);
            core::hint::black_box(counter);
        }
    }

    /// Tries to allocate `mib` MiB of linear memory using a fallible reservation. Returns `1` on
    /// success and `0` when the host's memory cap denies the growth (no trap, no abort).
    fn allocate(mib: u32) -> u32 {
        let bytes = (mib as usize).saturating_mul(1024 * 1024);
        let mut buffer: Vec<u8> = Vec::new();
        match buffer.try_reserve(bytes) {
            Ok(()) => {
                buffer.resize(bytes, 1);
                core::hint::black_box(&buffer);
                1
            }
            Err(_) => 0,
        }
    }
}

export!(Fixture);
