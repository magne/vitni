---
name: wrap-session
description: End-of-session wrap-up for the Vitni project. Summarizes what was done this session and proposes durable additions to CLAUDE.md, then prompts for /clear once additions are applied. Use when the user says "wrap session", "end of session", "wrap up the session", or invokes /wrap-session.
---

## Wrap Session

Run at the end of a working session. Two outputs: a concise summary of the session, and concrete
proposed additions to `CLAUDE.md`. After any `CLAUDE.md` change is applied, prompt the user to
`/clear`.

### Steps

1. **Gather what happened this session.** Use the conversation plus:
   - `git log --oneline <session-start>..HEAD` (commits made this session) and `git status`.
   - Files created/changed, PRs opened or updated, decisions made.
   Do not re-read the whole repo — rely on the session context and git.

2. **Write the session summary.** Group under: **Shipped** (commits/PRs/files), **Decisions**
   (choices made and why), **Open / next** (follow-ups, deferred work, new ADRs flagged). Keep it
   tight — bullets, no narration.

3. **Propose CLAUDE.md additions.** Identify durable, project-level knowledge learned this session
   that is NOT already in `CLAUDE.md` and is NOT derivable from code or git history. Good candidates:
   - New conventions or invariants agreed during the session.
   - New crates/docs/roadmap that future sessions should know exist (e.g. a path + one-line purpose).
   - Gotchas or correction lessons that would otherwise repeat.
   Skip: transient task detail, anything already in an ADR/data-model (reference it instead), restating
   code structure. For each proposed line, show the exact text and where in `CLAUDE.md` it goes.
   If nothing qualifies, say so plainly — do not invent additions.

4. **Confirm before editing.** Present the proposed additions and ask the user to approve, edit, or
   skip. Apply only what is approved, editing `CLAUDE.md` in place. Keep additions terse and in the
   file's existing voice/section structure.

5. **Prompt for /clear.** Once `CLAUDE.md` additions are applied (or the user explicitly skips them),
   tell the user the session is wrapped and to run `/clear` to start fresh. Do not attempt to run
   `/clear` yourself — it is a CLI command only the user can issue.

### Notes

- Respect the repo conventions: ADRs are immutable (never edit a past ADR — note a needed supersede
  instead); CLAUDE.md stays concise (the project recently trimmed it for token load).
- If a lesson is personal/cross-project rather than repo-specific, suggest the persistent memory
  instead of `CLAUDE.md`.
