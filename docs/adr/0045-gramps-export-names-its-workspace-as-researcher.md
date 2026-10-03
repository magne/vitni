# 45. A Gramps export names its workspace as the researcher

- **Status:** Accepted
- **Date:** 2026-10-04

## Context

ADR 0043 gives every workspace an id and hands it to exporters through `export-sink.workspace-id`. The
GEDCOM exporter writes it as `HEAD.FILE`. The Gramps exporter wrote no header, so a vitni Gramps export
declared no fingerprint, and re-importing a later export was never proposed the dataset its first
export went into (#469).

A Gramps XML file's fingerprint is its header's researcher name, `<header><researcher><resname>` (ADR
0037 §3). The researcher has no field meant for an identifier: the others are an address, a phone
number and an email. Gramps shows the researcher's name to its user in its preferences, and writes it
into its own GEDCOM exports as the submitter.

## Decision

1. **The Gramps exporter writes the workspace id as the researcher's name**, as
   `<resname>Vitni workspace <id></resname>`, and no other researcher field.

2. **The text is fixed English, never localized.** It is a fingerprint, so an export from a Norwegian
   session has to match one from an English session.

## Rationale

- **The importer stays as it is.** Its fingerprint is already the researcher's name, so one field
  carries the id with no change to the parser, the importer or the recorded `dataset_hint`s.
- **The name explains itself in Gramps.** A Gramps user who opens the file sees where the tree came
  from, not a bare UUID.
- **No address field is misused.** Putting the id in `<resaddr>` or `<resemail>` and keeping `vitni`
  as the name would have needed a two-field fingerprint, like GEDCOM's `HEAD.SOUR` and `HEAD.FILE`,
  for a value Gramps would then show as a street address or an email.

## Consequences

- A Gramps database that imports a vitni export can adopt *Vitni workspace <id>* as its owner, and
  Gramps then names that as the submitter of its own GEDCOM exports.
- A tree that goes from vitni through Gramps and back keeps the fingerprint only if Gramps writes the
  same researcher, so the round trip is proposed its dataset when the operator has not changed it.

## References

- ADR 0037 §3 (dataset proposal), ADR 0043 (the workspace id).
- Issue #469.
