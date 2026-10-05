# 46. An assisted import shows what it adds before it commits

- **Status:** Accepted
- **Date:** 2026-10-05

## Context

ADR 0040 §4 gives a bulk import a **Plan** stage, and #510 lists there, and again once the matches are
reviewed, each stored record the import adds to with the fields it gains. An assisted (one-record)
import shows neither. Each `submit` plans one record and the host commits it as soon as the last
possible match is answered, or at once when there are none, because ADR 0040 §4 says that "with no
candidates there is no extra stage at all".

So a census record whose source is decided *Same* writes onto the stored source what it lacks (the
repository that holds it, an author, a publication), and the user learns of it only from the source's
history. The plugin's own Summary cannot report it: its payload is the plugin's, and the plugin never
sees the plan (#512).

## Decision

1. **The host shows a Ready-to-import stage before it commits an assisted record that adds to a stored
   one.** After the record's last possible match is answered, the host takes the reviewed plan's
   summary. If any stored record would gain a field (`PlanSummary.records` is not empty), the host puts
   it to the frontend through the same ADR 0017 §5 channel as the Match stage. The stage lists those
   records with the fields each gains, in the bulk wizard's rows ("reused · adds repository").

2. **The user imports the record, skips it, or cancels the session**, as on the Match stage. A skip
   or a cancel writes nothing of the record, and the plugin learns only `skipped` or `cancelled`.

3. **A record that adds to no stored record still commits with no extra stage**, whether or not it had
   possible matches. With no frontend to ask, the host commits.

4. **This supersedes ADR 0040 §4 only in its sentence "with no candidates there is no extra stage at
   all".** ADR 0040 is not edited. The rest of its assisted flow stands.

## Rationale

- **Only a change to stored data earns a stop.** A record whose every entity is new, or reused
  unchanged, writes nothing the user has not already seen on the plugin's Confirm stage. Stopping on
  it would add a click to every record of a long session for nothing.
- **Candidates are not the trigger.** A source reused through its record origin, with no question
  asked, can still gain a field, so asking only after a Match stage would miss it.
- **The host owns the stage.** The plan is the host's (ADR 0040 §2), so the plugin protocol and its
  payloads stay as they are.

## Consequences

- An assisted session can stop on a record twice, at Match and at Ready to import, before it is written.
- The frontend's presenter answers one more host request. The GUI renders it with the bulk wizard's
  record rows. The CLI has no assisted import, so it is unaffected.

## References

- ADR 0017 §5 (the `present` channel), ADR 0040 §2 and §4 (the plan and the flow per world).
- Issues #510, #512.
