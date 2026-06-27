#!/usr/bin/env bash
# Opens the stacked PR for the Media MIME import/export round-trip (Phase 5 PR 15).
# Idempotent: safe to re-run. Pushes the branch, then creates the PR if absent.
set -euo pipefail

BRANCH="feat/phase5-pr15-import-export-roundtrip"
# Stacked on the deferred-edits PR; override with BASE once that lands.
BASE="${BASE:-feat/phase5-pr14-deferred-edits}"

git push --set-upstream origin "$BRANCH"

if gh pr view "$BRANCH" >/dev/null 2>&1; then
    echo "PR for $BRANCH already exists; skipping create."
    exit 0
fi

gh pr create \
    --base "$BASE" \
    --head "$BRANCH" \
    --title "feat(plugins): round-trip a media object's MIME type (host-api 0.10.0)" \
    --body "$(
        cat <<'EOF'
Round-trips a media object's MIME type through the GEDCOM and Gramps import/export
plugins, closing the first part of the round-trip follow-up.

- Host-api WIT bumped 0.9.0 -> 0.10.0: `media-dto` gains a `mime` field and
  `commands` gains `set-media-mime`. Guest `with` keys updated in the four plugins.
- `genealogy-gedcom` `MediaObject.mime` <-> `OBJE.FILE.FORM`; `genealogy-gramps-xml`
  `MediaObject.mime` <-> `<file mime>` (model + parse + emit).
- The four `plugins/{gedcom,gramps}-{import,export}` glue paths and the host
  `list_media` / `set-media-mime` carry it.
- Both round-trip tests assert the MIME survives import -> export -> re-import
  (GEDCOM `OBJE.FILE.FORM`, Gramps `<file mime>`).

Still open (documented in docs/phase5/plan.md): family per-partner child
relationships (_FREL/_MREL), the explicit FamilyEventLinked link, and the
per-translation RichText translator. DNA aggregates have no standard
GEDCOM/Gramps representation and are out of scope.

Workspace build, clippy -D warnings, all 416 workspace tests, the GEDCOM/Gramps
round-trip tests, `cargo xtask build-plugins`, and i18n-check pass.
EOF
    )"
