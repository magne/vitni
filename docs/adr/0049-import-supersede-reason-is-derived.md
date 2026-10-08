# 49. An import's supersede reason is derived from its run, not stored

- **Status:** Accepted
- **Date:** 2026-10-08
- **Supersedes:** ADR 0029 §1 and §5, in part — the generated `rationale` they describe

## Context

ADR 0029 §1 lets a re-import supersede a single-valued field when the file's export date is no
earlier than when the live value was recorded. It says the write carries "a `rationale` naming the
source file and its export date (the reason is generated, not typed)", and §5 names that rationale as
the record of *why* the value changed, read on the record's History tab.

That rationale was never written. `origin_gate::supersede` and `import_assert_sex` put no reason on the
command, and the plugin host's provenance deliberately carries none. History also folds every change a
run made to a record into one row (ADR 0037 §5) and never showed the rows inside it, so a user could
not see that an import had replaced one of their values, let alone why (#534).

Writing the reason as the ADR described would store an English sentence in `EventContext.rationale`.
That field is record data: it is shown as written, in every UI language, for as long as the event
exists. A Norwegian user would read an English reason the app generated, which ADR 0003 rules out for
anything the app itself says.

Everything the reason states is already recorded. The supersession's provenance names its run through
its record origin, and the `ImportRun` aggregate holds the run's source label and the file's export
date (`file_asserted_at`, ADR 0029 §2).

## Decision

1. **An import's supersession stores no rationale.** `EventContext.rationale` stays what the operator
   typed. An import has typed nothing, so it stays empty.

2. **History derives the reason when it renders.** `RunRef` carries the run's `file_asserted_at`. The
   History tab gives an `AssertionSuperseded` entry that belongs to a run with a known export date a
   localized reason: the value it replaced was recorded before the file was exported, so the file's
   value replaced it. A run whose file had no readable export date cannot supersede (ADR 0029 §3), and
   its entries get no reason.

3. **A run's History row expands to the changes it made to the record.** The folded row stays the
   default, with the count beside it. A disclosure shows its entries with who, when and the derived
   reason. The row keeps the single undo it has today (#306), and its entries carry none of their own.

## Consequences

- The reason follows the UI language, and rewording it is a catalogue change rather than a migration
  of stored events.
- The reason is only as available as the run is. A supersession whose provenance names no run cannot be
  given one. A re-import that supersedes a person's sex (`import_assert_sex`) writes no record origin
  today, so it reads as a bare software-agent change. That is tracked in `docs/issues.md`.
- Exporting the event log (a backup, ADR 0041) carries no reason text for these supersessions. A
  reader of the log alone still has the run id and the run's export date.

## References

- ADR 0003: UI localization.
- ADR 0029: import merge/sync reconciliation, the rule this explains.
- ADR 0037 §5: a run's entries fold into one History row.
