# Plugin trust tiers & capability grants (ADR 0014)
plugin-trust-sanctioned = godkjent
plugin-trust-user-trusted = brukerbetrodd
plugin-trust-untrusted = ubetrodd
plugin-list-line = { $id }  [{ $trust }]  deklarert: { $declared }  tildelt: { $granted }
plugin-list-empty = Fant ingen tilleggspakker i noe lag. Kjør `cargo xtask build-plugins` i et utviklingsutsjekk.
plugin-grants-saved = Lagret tildelinger for { $id }: { $approved }
plugin-trust-list-line = { $publisher }  { $fingerprint }…
plugin-trust-list-empty = Ingen utgiver er festet.
plugin-trust-pinned = Festet utgiver { $publisher }.
plugin-trust-unpinned = Løsnet utgiver { $publisher }.
