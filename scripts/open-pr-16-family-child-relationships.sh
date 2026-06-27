#!/usr/bin/env bash
# Opens the stacked PR for the per-partner child-relationship round-trip (Phase 5 PR 16).
# Idempotent: safe to re-run. Pushes the branch, then creates the PR if absent.
set -euo pipefail

BRANCH="feat/phase5-pr16-family-child-relationships"
# Stacked on the MIME round-trip PR; override with BASE once that lands.
BASE="${BASE:-feat/phase5-pr15-import-export-roundtrip}"

git push --set-upstream origin "$BRANCH"

if gh pr view "$BRANCH" >/dev/null 2>&1; then
    echo "PR for $BRANCH already exists; skipping create."
    exit 0
fi

gh pr create \
    --base "$BASE" \
    --head "$BRANCH" \
    --title "feat(plugins): round-trip per-partner child relationships (host-api 0.11.0)" \
    --body "$(
        cat <<'EOF'
Round-trips a child's per-partner relationships (GEDCOM `_FREL`/`_MREL`, Gramps
`frel`/`mrel`) through the import/export plugins.

- Host-api WIT bumped 0.10.0 -> 0.11.0: a `child-relationship` variant +
  `child-parent-rel`/`family-child` records; `family-dto.children` becomes
  `list<family-child>`; `add-child` gains a `relationships` argument. Guest
  `with` keys updated in the four plugins.
- Both intermediate models carry a `ChildRef` with the raw relationship strings;
  `plugin-api/convert.rs` maps those strings <-> the WIT variant.
- The four glue paths key relationships by father = partner 0 / mother =
  partner 1; `import_add_child` forwards them to `add_child`.
- Both round-trip tests assert the per-partner relationships survive
  import -> export -> re-import; the model crates' emit->parse tests cover them too.

Still open (docs/phase5/plan.md): the explicit `FamilyEventLinked` link (today
implicit via the participant-set heuristic) and the per-translation RichText
`translator` (no standard GEDCOM/Gramps representation). DNA aggregates remain
out of scope.

Workspace build, clippy -D warnings, all 416 workspace tests, the GEDCOM/Gramps
round-trip tests, `cargo xtask build-plugins`, and i18n-check pass.
EOF
    )"
