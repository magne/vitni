//! The configuration seam (ADR 0015): the language-request resolver and the [`ConfigStore`] trait.
//!
//! Configuration is grouped by *owner* into three scopes — workspace-functionality, operator, and
//! client/presentation (ADR 0015 §1) — read and written through [`ConfigStore`]. One
//! [`FileConfigStore`] backs it with the two ADR 0005 TOML files; a database backend plugs into the
//! same trait in Phase 13.
//!
//! The [`resolve_requested_languages`] resolver fixes the env-precedence bug (ADR 0015 §4): the
//! frontends built their Fluent localizers from the raw environment request and never consulted the
//! configured `ui_language`, so a bare `LANGUAGE` outranked stored config. The resolver keeps the
//! order **plain env < configured `ui_language` < `GENEALOGY_LANGUAGE`**. It is pure (takes the
//! environment as arguments) so every precedence case is unit-tested; the frontends supply the plain
//! request from `DesktopLanguageRequester`, keeping this crate free of `i18n_embed`.

use unic_langid::LanguageIdentifier;

/// The app-scoped UI-language override environment variable (ADR 0015 §4) — the highest-priority
/// signal, above configuration.
const LANGUAGE_ENV: &str = "GENEALOGY_LANGUAGE";

/// Resolves the ordered language request from the three signals, highest priority last (ADR 0015 §4):
/// the ambient `plain_env` (`LANGUAGE`/`LANG`), the configured `ui_language`, then the app-scoped
/// `prefixed_env` (`GENEALOGY_LANGUAGE`).
///
/// Returns `[prefixed_env]` if set, else `[config_ui_language]` if set, else `plain_env` verbatim —
/// so configuration wins over the ambient system locale (the bug fix) and the explicit env override
/// wins over both.
#[must_use]
pub fn resolve_requested_languages(
    config_ui_language: Option<&LanguageIdentifier>,
    plain_env: &[LanguageIdentifier],
    prefixed_env: Option<&LanguageIdentifier>,
) -> Vec<LanguageIdentifier> {
    if let Some(prefixed) = prefixed_env {
        return vec![prefixed.clone()];
    }
    if let Some(config) = config_ui_language {
        return vec![config.clone()];
    }
    plain_env.to_vec()
}

/// Reads and parses the `GENEALOGY_LANGUAGE` override (ADR 0015 §4); `None` when unset, empty, or not
/// a valid BCP-47 tag.
#[must_use]
pub fn genealogy_language_env() -> Option<LanguageIdentifier> {
    let value = std::env::var(LANGUAGE_ENV).ok()?;
    if value.is_empty() {
        return None;
    }
    value.parse().ok()
}

/// The language request for a real startup: overlays [`genealogy_language_env`] on top of the
/// configured `ui_language` and the ambient `plain_env` (ADR 0015 §4). Frontends call this with the
/// plain request from `DesktopLanguageRequester` to build their Fluent localizers.
#[must_use]
pub fn requested_languages_for(
    config_ui_language: Option<&LanguageIdentifier>,
    plain_env: &[LanguageIdentifier],
) -> Vec<LanguageIdentifier> {
    resolve_requested_languages(config_ui_language, plain_env, genealogy_language_env().as_ref())
}

#[cfg(test)]
mod tests {
    use super::resolve_requested_languages;
    use unic_langid::LanguageIdentifier;

    fn lang(tag: &str) -> LanguageIdentifier {
        tag.parse().expect("valid language tag")
    }

    #[test]
    fn plain_env_used_when_no_config_or_prefix() {
        let resolved = resolve_requested_languages(None, &[lang("en")], None);
        assert_eq!(resolved, vec![lang("en")]);
    }

    #[test]
    fn config_overrides_plain_env() {
        // The bug: configured ui_language must win over a bare LANGUAGE in the environment.
        let resolved = resolve_requested_languages(Some(&lang("no")), &[lang("en")], None);
        assert_eq!(resolved, vec![lang("no")]);
    }

    #[test]
    fn prefixed_env_overrides_config_and_plain() {
        let resolved = resolve_requested_languages(Some(&lang("no")), &[lang("en")], Some(&lang("de")));
        assert_eq!(resolved, vec![lang("de")]);
    }

    #[test]
    fn prefixed_env_overrides_plain_when_no_config() {
        let resolved = resolve_requested_languages(None, &[lang("en")], Some(&lang("de")));
        assert_eq!(resolved, vec![lang("de")]);
    }

    #[test]
    fn empty_everything_yields_empty() {
        let resolved = resolve_requested_languages(None, &[], None);
        assert!(resolved.is_empty());
    }
}
