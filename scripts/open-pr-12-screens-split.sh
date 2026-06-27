#!/usr/bin/env bash
# Opens the stacked PR for the screens.rs split (Phase 5 PR 12).
# Idempotent: safe to re-run. Pushes the branch, then creates the PR if absent.
set -euo pipefail

BRANCH="feat/phase5-pr12-screens-split"
# Stacked on PR 11 until it lands on main; override with BASE=main once merged.
BASE="${BASE:-feat/phase5-pr11-tag-dna}"

git push --set-upstream origin "$BRANCH"

if gh pr view "$BRANCH" >/dev/null 2>&1; then
    echo "PR for $BRANCH already exists; skipping create."
    exit 0
fi

gh pr create \
    --base "$BASE" \
    --head "$BRANCH" \
    --title "refactor(ui-dioxus): split screens.rs into per-aggregate modules" \
    --body "$(
        cat <<'EOF'
Splits `crates/genealogy-ui-dioxus/src/screens.rs` (8652 lines) into a
`src/screens/` directory with one module per aggregate plus `dashboard`,
`plugin_panel`, a `prelude` of shared imports, and a `shared` module for the
cross-aggregate helpers.

- `shared.rs` holds the helpers used by more than one aggregate:
  `citation_table`, `id_list`, `media_gallery`, `tags_panel`, `non_empty`,
  `source_cue`, `family_media_gallery`, `source_media_type_choices`
  (the last four promoted to `pub` for cross-module use).
- `mod.rs` re-exports the public surface (`*Screen`, `*EditForm`, table/panel
  helpers) so the existing `genealogy_ui_dioxus::screens::{...}` import path used
  by the 12 `tests/*_detail.rs` and `shell/root.rs` is unchanged.
- Pure move: every line of code is identical apart from the four `pub`
  promotions; no behavior change.

cargo build / clippy (-D warnings) / test for the crate pass; i18n-check clean.
EOF
    )"
