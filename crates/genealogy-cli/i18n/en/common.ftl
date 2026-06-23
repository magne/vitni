# Fallback (English) catalogue for the genealogy CLI (ADR 0003).
# Every key must exist here — `fl!()` checks calls against this file at compile time.

## Command output
created = Created { $id }
updated = Updated { $id }
init-success = Initialized workspace "{ $name }" at { $path }
config-line = Config: { $path }
rebuild-success = Rebuilt all projections from the event log.
import-success = Imported { $count } record(s) with { $plugin }.
export-success = Exported { $count } record(s) to { $path }.
import-confirm = Workspace "{ $name }" already contains { $count } person(s). Import anyway? [y/N]
import-cancelled = Import cancelled.
error-prefix = error: { $message }

## Date qualifiers (the calendar date itself is formatted by ICU4X; these wrap it — data-model §7.1)
date-before = before { $date }
date-after = after { $date }
date-about = about { $date }
date-from = from { $date }
date-to = to { $date }
date-range = between { $start } and { $end }
date-span = { $start } to { $end }
date-estimated = estimated { $date }
date-calculated = calculated { $date }

## Privacy restrictions (GEDCOM v7 RESN — data-model §6)
restrictions-tag = { " " }[{ $value }]
restriction-confidential = confidential
restriction-locked = locked
restriction-privacy = privacy

## Confidence labels (data-model §8)
confidence-very-low = very low
confidence-low = low
confidence-normal = normal
confidence-high = high
confidence-very-high = very high

## AppError
err-config = configuration error: { $detail }
err-workspace = workspace error: { $detail }
err-human-id-taken = human_id "{ $id }" is already taken
err-plugin = plugin error: { $detail }

## DbError
err-db-unsupported = unsupported: { $detail }
err-db-backend = storage backend error: { $detail }
err-db-malformed = malformed input: { $detail }
