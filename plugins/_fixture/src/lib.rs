//! Test-only fixture plugin: proves capability gating, the fuel limit, and the memory cap.

wit_bindgen::generate!({
    world: "fixture",
    path: "../../crates/genealogy-plugin-host/wit",
});

use crate::genealogy::host_api::{commands, log, media_store, net, types};

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

    /// GETs `url` through the host `net` capability. On success returns `"status final-url body-len"`;
    /// the host's `denied`/policy error is surfaced as the error string.
    fn try_fetch(url: String) -> Result<String, String> {
        match net::fetch(&url) {
            Ok(response) => Ok(format!("{} {} {}", response.status, response.final_url, response.body.len())),
            Err(error) => Err(format!("{error:?}")),
        }
    }

    /// Stores `bytes` under `suggested_path` through the host `media-store` capability. On success
    /// returns `"relative-path checksum mime size existed"`.
    fn try_store(bytes: Vec<u8>, suggested_path: String) -> Result<String, String> {
        match media_store::store(&bytes, &suggested_path) {
            Ok(stored) => Ok(summarize(&stored)),
            Err(error) => Err(format!("{error:?}")),
        }
    }

    /// Downloads `url` and stores it under `suggested_path` through the host `media-store` capability.
    fn try_fetch_store(url: String, suggested_path: String) -> Result<String, String> {
        match media_store::fetch_and_store(&url, &suggested_path) {
            Ok(stored) => Ok(summarize(&stored)),
            Err(error) => Err(format!("{error:?}")),
        }
    }
}

/// Formats a stored-media record as `"relative-path checksum mime size existed"` for the tests.
fn summarize(stored: &media_store::StoredMedia) -> String {
    format!(
        "{} {} {} {} {}",
        stored.relative_path, stored.checksum, stored.mime, stored.size, stored.existed
    )
}

export!(Fixture);
