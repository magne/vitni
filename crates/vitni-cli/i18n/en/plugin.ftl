# Plugin trust tiers & capability grants (ADR 0014)
plugin-trust-sanctioned = sanctioned
plugin-trust-user-trusted = user-trusted
plugin-trust-untrusted = untrusted
plugin-list-line = { $id }  [{ $trust }]  declared: { $declared }  granted: { $granted }
plugin-list-empty = No plugin bundles found in any layer. Run `cargo xtask build-plugins` in a dev checkout.
plugin-grants-saved = Saved grants for { $id }: { $approved }
plugin-trust-list-line = { $publisher }  { $fingerprint }…
plugin-trust-list-empty = No publisher is pinned.
plugin-trust-pinned = Pinned publisher { $publisher }.
plugin-trust-unpinned = Unpinned publisher { $publisher }.
