# 3. Localize the UI with Fluent, embedded and overridable via i18n-embed

- **Status:** Accepted
- **Date:** 2026-06-17

## Context

The product runs one `genealogy-core` (ADR 0001/0002) behind several frontends: the
shippable **CLI today** (`genealogy-cli`), and a **native GUI** and **web app** planned
later. Each frontend must present its interface in the user's language. Two deployment
shapes drive the requirement:

- **Standalone GUI / CLI** — translations should be **embedded** in the binary so the app
  is localized with zero setup, while a user or translator can **override** them at runtime
  from a **shared application directory** or the open **workspace directory** — patching a
  wording or adding a language without a rebuild.
- **Web backend / frontend** — translations should be served from **non-embedded files** on
  disk, so strings can be updated by redeploying assets rather than recompiling.

This ADR is about **presentation-layer** localization: the UI message strings a frontend
shows. That is distinct from — and must not be confused with — the **data** language
metadata already designed in `docs/data-model.md` §14 (`LanguageTag`, `RichText.language`,
`PlaceName.language`, `PersonName.transliterations`). Those describe what language a
*record* is in; this ADR describes how the *application chrome* is translated. The two
share only the BCP-47 language-identifier vocabulary.

## Decision

1. **Message system: Mozilla Fluent** (`fluent` / `fluent-rs`), authored as `.ftl`
   (Fluent Translation List) files. Genealogy UI text is dense with gendered and
   relationship/plural forms (son/daughter, "N children", possessives, kinship terms);
   Fluent's selectors, plural categories, and term references keep that linguistic logic
   inside the translation files where translators own it, instead of in Rust branching.
   Language identifiers are BCP-47 via `unic-langid` — the same standard the data model's
   `LanguageTag` uses, so requested-language negotiation is consistent across the system.

2. **Embedding and runtime loading: `i18n-embed`** with the `fluent-system` feature,
   using `FluentLanguageLoader` and the `fl!()` macro (compile-time-checked message keys).
   - **Asset layering with `i18n_embed::AssetsMultiplexor`** is the mechanism that
     satisfies the embed-plus-override requirement. The standalone build composes
     `I18nAssets` sources in priority order, high to low:
     1. `FileSystemAssets` → the open **workspace directory** override,
     2. `FileSystemAssets` → the **shared application directory** override,
     3. `RustEmbed` → the **embedded** baseline.

     A message present in a higher layer overrides the lower layers; the embedded baseline
     always carries the complete **fallback language**, so the UI can never be left
     unlocalized regardless of what overrides exist.
   - **The web build uses `FileSystemAssets` alone**, pointing at the deployed locale
     directory — no `RustEmbed`, so strings update by redeploying files.
   - **Language requesting** is `DesktopLanguageRequester` (native/CLI, via `sys-locale`)
     or `WebLanguageRequester` (WASM, via `web-sys`), feature-gated per frontend.
     `i18n_embed::select()` negotiates the requested languages against those available and
     loads them with fallback.

3. **Crate boundary — localization lives per frontend; the core stays string-free.**
   - `genealogy-core` exposes **typed error enums and structured values only**. It emits no
     user-facing strings; its `tracing` output stays developer-facing English. This keeps
     the domain core free of CLI/UI concerns (CLAUDE.md) and means localization choices
     never reach into domain logic.
   - Each frontend owns its `i18n/{language}/{domain}.ftl` assets, its `i18n.toml`, its
     `RustEmbed` struct (where embedding applies), and the mapping from core error/value
     types to localized messages. `genealogy-cli` does this now; the GUI and web frontends
     follow the same pattern.
   - **No shared localization crate yet.** Frontends repeat a small amount of loader
     setup; we accept that until a third frontend proves the duplication real (YAGNI —
     three copies before abstracting).

4. **Locale-aware value formatting: ICU4X (`icu`, 2.x).** Fluent covers message-level
   plural/select; formatting the *values* — dates and numbers — per locale is ICU4X's job
   (CLDR-based), and genealogy is date-heavy (the data model's `GenealogicalDate`), so this
   is needed, not speculative. ICU4X is integrated **into Fluent** via `fluent-datetime`:
   `bundle.add_datetime_support()` registers a `DATETIME($date, dateStyle: …)` function in
   FTL, so translators choose the date style and the code passes a `FluentDateTime` value;
   numbers use ICU4X decimal formatting the same way.
   - **Partial/approximate dates.** ICU4X formats *precise* dates, but genealogical dates
     are routinely partial, approximate, or ranged (`ABT 1847`, `BET 1850 AND 1852`,
     non-Gregorian calendars). So ICU4X formats the resolved core date, and the fuzzy
     modifiers and calendar labels that wrap it are localized as **Fluent terms/messages**.
     The two compose: ICU4X for the calendar-correct core, Fluent for the qualifier text.
   - **Data and binary size.** ICU4X compiles CLDR data in by default; `icu4x-datagen`
     (with `ICU4X_DATA_DIR`) trims it to the locales actually shipped, which matters for
     the embedded standalone binary.

5. **Build, extract and verify workflow: `cargo-i18n` (`cargo i18n`) + `i18n.toml`.**
   `i18n.toml` (`fallback_language`, `[fluent] assets_dir`) is the single config the
   `fluent_language_loader!()` macro reads at compile time and the `cargo i18n` sub-command
   reads to build localization resources. Honest current state, recorded so we do not
   assume coverage we lack:
   - `cargo i18n` performs **no validation for the Fluent system yet** (tracking issue #31;
     its `xtr` string extraction is gettext-only). For Fluent it is the config/build entry
     point, not yet a validator.
   - Compile-time checking of **message ids and arguments** comes from **`i18n-embed-fl`'s
     `fl!()`** macro, which checks each call against the fallback-language FTL at build time.
   - Checking **completeness across all locales** (missing keys, undeclared variables) is
     the remaining gap; the mitigation — a Fluent-aware checker such as `es-fluent-cli`, or
     a small custom CI check — is chosen when the second locale lands, not before.

## Rationale

- **Fluent over `rust-i18n` / `gettext`.** `rust-i18n`'s `t!()`/YAML is lower-setup but
  weak on gender and plural logic and has no built-in embed-plus-filesystem-override layer
  — we would hand-build the override. `gettext` has a mature translator ecosystem but, by
  its own crate's admission, is technically inferior to Fluent: plurals only, no gender
  selectors. Genealogy's gendered, relationship-heavy strings are exactly Fluent's strength.
- **`i18n-embed` over a hand-rolled loader.** `AssetsMultiplexor` is a direct fit for
  "embedded baseline plus runtime overrides in priority order", and the same
  `FluentLanguageLoader` drops to `FileSystemAssets` alone for the web — one library covers
  both deployment shapes. `fl!()` gives compile-time key checking.
- **Per-frontend over a shared crate.** Keeping UI strings out of `genealogy-core`
  preserves the ADR 0001/0002 layering (core is pure domain logic) and avoids pulling
  presentation vocabulary toward the domain. A shared crate is a later refactor, not a
  starting assumption.

## Consequences

### Positive

- The standalone binary is localized out of the box with no setup, yet a user or translator
  can override or add languages at runtime from the workspace or shared app directory.
- The web frontend updates strings by redeploying files — no recompile.
- `fl!()` catches missing/mistyped message keys at compile time.
- The domain core stays presentation-neutral; localization choices never touch domain code.
- UI and data layers share one BCP-47 language-identifier vocabulary (`unic-langid`).

### Negative / costs

- Fluent has more upfront setup than a `t!()`/YAML approach, and FTL authoring requires
  discipline — every key must exist in the fallback language.
- `i18n-embed` brings `rust-embed`, plus `sys-locale` (desktop) or `web-sys` (WASM) per
  frontend.
- Each frontend repeats loader/boilerplate setup until a shared crate is justified.
- ICU4X compiles CLDR data into the binary; `icu4x-datagen` is needed to keep the embedded
  standalone build small.
- `cargo i18n` does not validate Fluent yet, so all-locale completeness checking needs an
  extra tool or CI step (the `fl!()` macro covers only id/argument checks against the
  fallback language).

## Out of scope

- The **data** language metadata of `docs/data-model.md` §14 — a separate concern.
- Pseudolocalization for QA.
- The concrete shared-app / workspace override directory **paths** — a config/runtime
  detail decided when each frontend is built.
- The human translator/vendor process (who translates, review cycle) — this ADR covers the
  tooling, not the workflow around people.

## References

- `i18n-embed` 0.16 — <https://docs.rs/i18n-embed> (`AssetsMultiplexor`,
  `FluentLanguageLoader`, `FileSystemAssets`, `RustEmbed`, `select`, the desktop/web
  language requesters). Exact versions resolve at `cargo add` time.
- `i18n-embed-fl` — the `fl!()` compile-time message check against the fallback language.
- `cargo-i18n` (`cargo i18n`) — <https://github.com/kellpossible/cargo-i18n>; `i18n.toml`
  config; issue #31 tracks the missing Fluent validation.
- Project Fluent / `fluent-rs` — <https://projectfluent.org>.
- `fluent-datetime` — <https://docs.rs/fluent-datetime> (ICU4X-backed `DATETIME` function
  for Fluent).
- `icu` (ICU4X 2.x) — <https://docs.rs/icu>; `icu4x-datagen` for trimming CLDR data.
- `unic-langid` — BCP-47 language identifiers, shared with the data model's `LanguageTag`.
- ADR 0001 / ADR 0002 — event-sourced core and its crate layering this ADR builds on.
- `docs/data-model.md` §14 — the data-language metadata this ADR is deliberately distinct
  from.
