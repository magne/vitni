#!/usr/bin/env bash
# Opens the stacked PR for the explicit FamilyEventLinked round-trip (Phase 5 PR 17).
# Idempotent: safe to re-run. Pushes the branch, then creates the PR if absent.
set -euo pipefail

BRANCH="feat/phase5-pr17-family-event-link"
# Stacked on the child-relationship PR; override with BASE once that lands.
BASE="${BASE:-feat/phase5-pr16-family-child-relationships}"

git push --set-upstream origin "$BRANCH"

if gh pr view "$BRANCH" >/dev/null 2>&1; then
    echo "PR for $BRANCH already exists; skipping create."
    exit 0
fi

gh pr create \
    --base "$BASE" \
    --head "$BRANCH" \
    --title "feat(plugins): round-trip explicit FamilyEventLinked (host-api 0.12.0)" \
    --body "$(
        cat <<'EOF'
Round-trips the explicit family<->event link through the import/export plugins,
so a family event nests under its FAM / <family> by its recorded link rather
than only by the participant-set heuristic.

- Host-api WIT bumped 0.11.0 -> 0.12.0: `family-dto` gains an `events` list and
  `commands` gains `link-family-event`. Guest `with` keys updated in the four
  plugins.
- Importers link each family event explicitly (in addition to adding the partners
  as participants); `import_add_child`-style idempotency via the create-once guard.
- Exporters route an event to its family by the explicit link first, falling back
  to the participant-set heuristic only for unlinked events (dedup-safe).
- Both round-trip tests assert a family event survives import -> export ->
  re-import as an explicit link; the existing event round-trips are unchanged.

Also records the completed follow-up PRs in the docs/phase5/plan.md PR sequence
(rows 12-17) and pushes the original feature PRs down to 18-23.

Out of scope (documented): the per-translation RichText `translator` (no standard
GEDCOM/Gramps representation) and DNA aggregates.

Workspace build, clippy -D warnings, all 416 workspace tests, the GEDCOM/Gramps
round-trip tests, `cargo xtask build-plugins`, and i18n-check pass.
EOF
    )"
