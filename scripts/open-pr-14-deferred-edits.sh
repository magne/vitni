#!/usr/bin/env bash
# Opens the stacked PR for the deferred field-edit forms (Phase 5 PR 14).
# Idempotent: safe to re-run. Pushes the branch, then creates the PR if absent.
set -euo pipefail

BRANCH="feat/phase5-pr14-deferred-edits"
# Stacked on the stable-id backfill PR; override with BASE once that lands.
BASE="${BASE:-feat/phase5-pr13-stable-id-backfill}"

git push --set-upstream origin "$BRANCH"

if gh pr view "$BRANCH" >/dev/null 2>&1; then
    echo "PR for $BRANCH already exists; skipping create."
    exit 0
fi

gh pr create \
    --base "$BASE" \
    --head "$BRANCH" \
    --title "feat(ui): wire the deferred scalar field-edit forms (PRs 7-11)" \
    --body "$(
        cat <<'EOF'
Wires the scalar field-edit forms the PR 7-11 status notes deferred, end-to-end
(navigation *Edit variant -> dispatch_*_edit arm -> side-panel form component ->
overview trigger button), each calling the app use-case that already existed:

- Event: set type / date / description.
- Place: set type / coordinates / jurisdiction code.
- Source: set author / publication info / abbreviation.
- Repository: set name / type.
- Media: set file path / web path / checksum / date.
- DnaTest: set provider / kit id / type / genome build.

`GeoCoordinates`/`Microdegrees` are re-exported from genealogy-app for the place
coordinates form. New i18n keys field-{name,year,month,day,code,web-path,
coordinates,latitude,longitude} added to en + no. The privacy Restriction edit
flow is confirmed wired on every detail screen.

Out of scope (mockup-only fields with no core backing): DnaTest account /
date-tested / SNP count; DnaMatch segment lineage / terminal-SNP / fully-identical
regions; citations on DNA records.

Workspace build, clippy -D warnings, all 416 tests, and i18n-check pass.
EOF
    )"
