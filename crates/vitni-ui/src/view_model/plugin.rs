//! Plugin trust-tier and capability-grant view-models (ADR 0014 §3, §5).
//!
//! A renderer discovers a bundle through the plugin host (which sits above this crate), maps the
//! host's trust tier onto the frontend-visible [`PluginTrust`] DTO, and hands this module the plain
//! facts — id, tier, the declared capability interface names, and the operator's persisted approval —
//! to build a render-ready, already-localized view-model. This keeps `vitni-ui` free of
//! plugin-host types while owning all the presentation logic (labels, the pending state, the
//! effective grant preview).

use std::collections::BTreeSet;

use vitni_app::{PluginTrust, PluginTrustConfig};

use super::Localizer;

/// How many leading hex characters of a pinned publisher key the trust-store editor shows as a
/// fingerprint. The full 64-hex key is never rendered — a short prefix is enough for a human to tell
/// two pins apart without implying it is a checkable digest.
const FINGERPRINT_HEX_LEN: usize = 16;

/// One declared capability's grant row: the interface name, its localized label, and whether it is
/// currently approved (ADR 0014 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityGrantVm {
    /// The canonical interface name (`log`, `query`, `commands`, …) — the persisted-set key.
    pub name: String,
    /// The localized display label for the capability.
    pub label: String,
    /// Whether this capability is currently granted: the operator's recorded decision if any, else
    /// the trust-tier default (a trusted plugin grants all declared; an untrusted one grants none).
    pub approved: bool,
}

/// A discovered plugin's trust-and-grant view-model (ADR 0014 §3, §5): its id, trust tier and label,
/// whether the operator has recorded a decision yet, and the per-capability grant rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginGrantVm {
    /// The plugin id (the bundle directory name; the persisted-decision key).
    pub id: String,
    /// The trust tier the host placed it in.
    pub trust: PluginTrust,
    /// The localized trust-tier label.
    pub trust_label: String,
    /// True when no approval decision is recorded yet — the first-load "needs approval" state.
    pub pending: bool,
    /// True when the tier allows a one-action "approve all declared" (sanctioned / user-trusted).
    pub allow_approve_all: bool,
    /// The declared capabilities, each with its current approved state, in declaration order.
    pub capabilities: Vec<CapabilityGrantVm>,
}

impl PluginGrantVm {
    /// The interface names currently approved — the set an "approve all" or an unchanged save would
    /// persist through [`crate::approve_plugin_grants`].
    #[must_use]
    pub fn approved_names(&self) -> BTreeSet<String> {
        self.capabilities
            .iter()
            .filter(|capability| capability.approved)
            .map(|capability| capability.name.clone())
            .collect()
    }

    /// Every declared interface name — the set an "approve all declared" action persists.
    #[must_use]
    pub fn all_declared_names(&self) -> BTreeSet<String> {
        self.capabilities
            .iter()
            .map(|capability| capability.name.clone())
            .collect()
    }
}

/// Builds a [`PluginGrantVm`] from a discovered plugin's plain facts and the operator's persisted
/// approval (`None` when no decision has been recorded — the pending, trust-tier-default state).
///
/// The per-capability approved state mirrors the host's effective-grant resolution
/// (`PluginInfo::effective_grants`): with a recorded decision a capability is approved iff it is in
/// the set; with no decision a sanctioned/user-trusted plugin defaults every declared capability to
/// approved and an untrusted one to denied.
#[must_use]
pub fn plugin_grant_vm(
    loc: &Localizer,
    id: &str,
    trust: PluginTrust,
    declared: &[String],
    approved: Option<&BTreeSet<String>>,
) -> PluginGrantVm {
    let allow_approve_all = matches!(trust, PluginTrust::Sanctioned | PluginTrust::UserTrusted);
    let mut capabilities = Vec::with_capacity(declared.len());
    for name in declared {
        let is_approved = match approved {
            Some(set) => set.contains(name),
            None => allow_approve_all,
        };
        capabilities.push(CapabilityGrantVm {
            name: name.clone(),
            label: loc.plugin_capability_label(name),
            approved: is_approved,
        });
    }
    PluginGrantVm {
        id: id.to_owned(),
        trust,
        trust_label: loc.plugin_trust_label(trust),
        pending: approved.is_none(),
        allow_approve_all,
        capabilities,
    }
}

/// One pinned publisher in the client-scope trust store: the publisher identity and a short key
/// fingerprint (never the raw 64-hex key, ADR 0014 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedPublisherVm {
    /// The publisher identity (the trust-store key; used to unpin).
    pub publisher: String,
    /// A short leading-hex fingerprint of the pinned public key.
    pub fingerprint: String,
}

/// The pinned-publisher trust-store editor view-model (ADR 0014 §3): the list of pins, publisher by
/// publisher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustStoreVm {
    /// The pinned publishers, sorted by publisher (the config store is a `BTreeMap`).
    pub publishers: Vec<PinnedPublisherVm>,
}

/// Builds a [`TrustStoreVm`] from the client-scope `[plugin_trust]` config.
#[must_use]
pub fn trust_store_vm(trust: &PluginTrustConfig) -> TrustStoreVm {
    let mut publishers = Vec::with_capacity(trust.publishers.len());
    for (publisher, key_hex) in &trust.publishers {
        publishers.push(PinnedPublisherVm {
            publisher: publisher.clone(),
            fingerprint: key_fingerprint(key_hex),
        });
    }
    TrustStoreVm { publishers }
}

/// A short leading-hex fingerprint of a pinned key. A key shorter than the prefix is shown whole.
fn key_fingerprint(key_hex: &str) -> String {
    key_hex.chars().take(FINGERPRINT_HEX_LEN).collect()
}

#[cfg(test)]
mod tests {
    use super::{plugin_grant_vm, trust_store_vm};
    use crate::i18n::Localizer;
    use std::collections::BTreeSet;
    use vitni_app::{PluginTrust, PluginTrustConfig};

    fn en() -> Localizer {
        Localizer::with_languages(None, &["en".parse().expect("tag")])
    }

    fn declared() -> Vec<String> {
        vec!["log".to_owned(), "query".to_owned(), "commands".to_owned()]
    }

    #[test]
    fn a_sanctioned_plugin_with_no_decision_is_pending_and_defaults_to_grant_all() {
        let loc = en();
        let vm = plugin_grant_vm(&loc, "gedcom-import", PluginTrust::Sanctioned, &declared(), None);
        assert!(vm.pending, "no recorded decision is the pending first-load state");
        assert!(vm.allow_approve_all, "a sanctioned plugin may approve all declared");
        assert_eq!(vm.trust_label, "Sanctioned");
        assert_eq!(vm.capabilities.len(), 3, "every declared capability is listed");
        assert!(
            vm.capabilities.iter().all(|c| c.approved),
            "a trusted plugin defaults to grant-all when pending"
        );
        assert_eq!(vm.capabilities[0].label, "log");
        assert_eq!(vm.all_declared_names(), BTreeSet::from_iter(declared()));
    }

    #[test]
    fn an_untrusted_plugin_with_no_decision_grants_nothing() {
        let loc = en();
        let vm = plugin_grant_vm(&loc, "third-party", PluginTrust::Untrusted, &declared(), None);
        assert!(vm.pending);
        assert!(
            !vm.allow_approve_all,
            "an untrusted plugin forces a per-capability choice"
        );
        assert_eq!(vm.trust_label, "Untrusted");
        assert!(
            vm.capabilities.iter().all(|c| !c.approved),
            "an untrusted plugin grants nothing until approved"
        );
        assert!(vm.approved_names().is_empty());
    }

    #[test]
    fn a_recorded_decision_is_the_intersection_and_clears_pending() {
        let loc = en();
        let approved = BTreeSet::from(["log".to_owned(), "query".to_owned()]);
        let vm = plugin_grant_vm(
            &loc,
            "gedcom-import",
            PluginTrust::Sanctioned,
            &declared(),
            Some(&approved),
        );
        assert!(!vm.pending, "a recorded decision clears the pending state");
        let approved_now: Vec<_> = vm
            .capabilities
            .iter()
            .filter(|c| c.approved)
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(approved_now, vec!["log".to_owned(), "query".to_owned()]);
        assert!(
            !vm.capabilities
                .iter()
                .find(|c| c.name == "commands")
                .expect("commands row")
                .approved,
            "an unapproved declared capability is denied"
        );
        assert_eq!(vm.approved_names(), approved);
    }

    #[test]
    fn the_trust_store_vm_shows_a_short_fingerprint_not_the_raw_key() {
        let mut trust = PluginTrustConfig::default();
        let key = "0123456789abcdef".repeat(4);
        trust.publishers.insert("acme".to_owned(), key.clone());
        let vm = trust_store_vm(&trust);
        assert_eq!(vm.publishers.len(), 1);
        assert_eq!(vm.publishers[0].publisher, "acme");
        assert_eq!(vm.publishers[0].fingerprint, "0123456789abcdef");
        assert!(
            vm.publishers[0].fingerprint.len() < key.len(),
            "the raw key is never shown whole"
        );
    }
}
