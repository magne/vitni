#!/usr/bin/env bash
# Opens the stacked PR for the Person/Citation stable-id backfill (Phase 5 PR 13).
# Idempotent: safe to re-run. Pushes the branch, then creates the PR if absent.
set -euo pipefail

BRANCH="feat/phase5-pr13-stable-id-backfill"
# Stacked on the screens-split PR; override with BASE once that lands.
BASE="${BASE:-feat/phase5-pr12-screens-split}"

git push --set-upstream origin "$BRANCH"

if gh pr view "$BRANCH" >/dev/null 2>&1; then
    echo "PR for $BRANCH already exists; skipping create."
    exit 0
fi

gh pr create \
    --base "$BASE" \
    --head "$BRANCH" \
    --title "feat(app,ui): backfill Person/Citation to the stable-id join pattern" \
    --body "$(
        cat <<'EOF'
Brings the two legacy aggregates onto the cross-aggregate join pattern PRs 7–11
established, resolving the "cross-aggregate joins need stable ids" dependency note.

App layer:
- `PersonSummary` now carries `citations: Vec<CitationRef>` (joined to Citation/
  Source), `media: Vec<MediaRefSummary>`, `notes: Vec<AggRef>`,
  `associations[].other: AggRef`, and `participations: Vec<ParticipationRef>`
  (event `AggRef` + role + date) — all with stable ids.
- `CitationSummary.source` is now `Option<AggRef>`.
- The joins moved into `show_person`/`list_persons` via the shared
  `dto::citation_refs` / `dto::media_refs` helpers.

UI layer:
- The person citation/event tabs were stitched in the dispatcher
  (`build_citations` / `build_events`, which re-queried every citation/event —
  the N+1 the dependency note rejected). Those are deleted; `PersonDetail::from_summary`
  now fills both tabs from the app-joined summary. `citation_ref_vm` (its only
  caller) is removed.

The other ten `*Summary` DTOs were already on-pattern. Workspace build, clippy
(-D warnings), all 416 tests, and i18n-check pass.
EOF
    )"
