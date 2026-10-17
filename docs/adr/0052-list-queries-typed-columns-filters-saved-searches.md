# 52. A list query is typed columns, filters and a sort, evaluated by the store

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

An Explorer list is filtered today by `ListQuery { query, sort }` (`vitni-ui/src/list.rs`): a text
box matched as a substring against the row's already-localized title, subtitle and id, and a sort
button cycling ID and name, ascending and descending. It is a component signal, rebuilt on every
mount and never stored. The list it filters is the whole kind, loaded once and windowed in the DOM
(#590). The page read #607 added (`list_*_page`, offset and limit in `human_id` order) does not serve
it: it cannot filter or sort, though since #673 scoped its summary joins to the page an app-level
page takes 640 µs at 100k persons ([`decisions.md`](../decisions.md) § Lists,
[`performance-profiling.md`](../research/performance-profiling.md)).

Three open design questions wait on the same missing piece:

- **Saved searches (#591).** A researcher working through 100k persons returns to the same questions
  — "the Olsens born before 1800", "the women tagged *To verify*", "the baptisms in Fana church". With
  only text and four sorts there is nothing worth saving.
- **Column chooser (#592).** The person mockup showed a *Columns* button no code backs: a row has a
  title and one subtitle line, and no column state exists.
- **Column sort.** Sorting by anything but ID or name — birth date, place, citation count — needs a
  typed value to sort by, where the row holds only display strings.

Two facts about the data shape the answer. A list's useful values are often not in the record's own
projection: a person's birth and death dates come from the Birth and Death events they are the
primary participant of (ADR 0021 §2), an event's place name from the place, a family's title from
its partners, a citation's title from its source. And a tag is stored on the record it tags, with no
reverse lookup: `TagUsage::load` scans every tag-bearing projection to count a tag's uses.

This ADR decides the query model all three build on, and what a saved search stores, where it
persists and where it appears. It builds none of them.

## Decision

1. **Each listed kind has a typed column catalogue, owned by `vitni-app`.** A column has a stable id
   (`person`: `name`, `given`, `surname`, `sex`, `birth`, `death`, `tags`; `event`: `type`, `date`,
   `place`; and so on per kind) and exactly one value type:

   | Type | Values | Example |
   | --- | --- | --- |
   | text | a string | a surname, a source title |
   | date | a `GenealogicalDate`, compared by its sort key | birth, event date |
   | number | an integer or decimal | child count, shared cM |
   | choice | a closed core enum | sex, event type, confidence |
   | tags | a set of `TagId` | the record's tags |
   | record | a reference, shown by the record's title | an event's place |

   The catalogue is the only list of columns. The frontends label a column through a Fluent id per
   column id, and the CLI names it by its id. Each kind has one **title column** that every row shows
   and that cannot be hidden. A column's value is the record's own or one reference away from it (an
   event's place, a person's birth event's date). A value further out, such as a place's enclosing
   county, is not a column.

2. **The value type fixes the operators.** text: *contains*, *is*, *is empty*. date: *before*,
   *after*, *between*, *is empty*. number: *less than*, *greater than*, *between*. choice: *is any
   of*, *is none of*. tags: *has any of*, *has all of*, *has none of*. record: *is*. A query's filters
   combine with AND. There is no OR and no grouping.

3. **One `ListQuery` DTO in `vitni-app` carries a query.** It is serde, versioned by additive change
   like an event payload (ADR 0004 §4), and holds:
   - `kind`, the listed kind;
   - `search`, a case-insensitive substring matched against the kind's text columns and, for a kind
     that has one, `human_id` (a tag has none);
   - `filters`, a list of `{ column, op, value }`;
   - `sort`, one `{ column, direction }`, with ties broken by `human_id` where the kind has one and
     then by the record id;
   - `columns`, the ids of the columns to return, in order.

   `search` no longer matches localized labels: typing *Female* finds no one, and the `sex` filter
   does. Tag and record values are held by id, never by display name, so a rename does not break a
   query.

4. **The store evaluates the query.** Filter, sort and paging run in SQL on both engines, and a page
   returns typed cells for the requested columns and the total match count. Like #607's page read, it
   leaves out a record merged into another (ADR 0039 §5), in the rows and in the count.
   - A page is addressed by **offset**. The Explorer's list box is as tall as the total count (#590),
     so a scrollbar jump lands on a row index and reads the window there.
   - A read that continues from a row it already holds (the CLI walking a list, `[`/`]` stepping past
     the loaded window) may use a **keyset** instead: the rows after that row's sort key and id. It
     stays fast deep in the list, where a large offset does not.

   The Explorer pages its fetch instead of loading the whole kind, and the CLI's paged list verbs read
   the same query. The tag list is the exception: a vocabulary of tens with no `human_id`, it stays a
   whole list ([`decisions.md`](../decisions.md) § Lists), and its query runs over all of it.

5. **Columns are materialised in a list table per kind.** One row per record, one typed SQL column per
   catalogue column. A column whose value comes from another projection (birth and death, an event's
   place, a family's partners, a citation's source) is written there, in the same transaction as the
   projections. When a record changes, the store recomputes the rows of the records that
   `record_links` (ADR 0047) names as referring to it: a renamed place updates its events' rows. The
   table is derived and rebuilt by `rebuild_projections` like `record_links`, so it is a disposable
   read model (ADR 0009 §5). A column from the record's own payload may read the JSON path instead
   (ADR 0009 §3). Indexes are added only where a measured query needs one (ADR 0009 §4). The
   cross-projection columns are the denormalised columns ADR 0009 §4 waits on a measured need for:
   the need is a filter or sort on a joined value, which no JSON path or index on one projection
   answers.

6. **Tagging becomes a `record_links` relation.** A `tagged` row (record → tag) is written from each
   tag-bearing projection. It serves the *tags* filter, and `list_tags` counts a tag's uses through it
   instead of `TagUsage`'s scan. The index's target is a record of a matchable kind today; a tag is
   not one, so the target kind widens to take it. The twelve tag-bearing kinds all become link
   sources, where only persons, families, events, places, sources and citations write links now.

7. **The chosen columns and sort are per-kind presentation state.** They live in `workspace.toml`
   under `[ui.lists.<kind>]` (`columns`, `sort`): client scope, like the recent list (ADR 0015 §1). An
   Explorer row keeps its two lines: the title column, then the chosen columns in their order, joined
   by ` · `, an empty cell skipped. The toolbar's sort control becomes a menu of the kind's columns
   with a direction, and a *Columns* menu picks and orders the columns.

8. **A saved search is a named `ListQuery`.** It is stored in `workspace.toml` as a
   `[[saved_searches]]` entry: an `id` (UUID v7, minted by the `Session`), a `name`, and the query.
   A name is unique within the workspace, compared case-insensitively, so it can address the search.
   It is workspace-functionality scope (ADR 0015 §1), not presentation: it belongs to the research in
   the dataset, so it travels with the workspace and its backups (ADR 0041 carries `workspace.toml`),
   everyone who opens the workspace sees it, and a Phase 13 server keeps it with the data. It is not
   an event: it asserts nothing about the past, so it has no place in the evidence layer.

9. **Saved searches appear in three places.**
   - **The Explorer toolbar** has a *Saved* menu: apply a saved search, save the current query as a
     new one, update the applied one with the current query, rename, delete.
   - **The command palette** has a *Saved searches* group. Running one switches to its kind's list
     with the query applied.
   - **The CLI**: the paged list verbs take `--search`, `--filter <column>:<op>:<value>`,
     `--sort <column>[:desc]` and `--saved <name>`, and a `saved-search` verb lists, saves and deletes
     them, so a script or an agent can do what the GUI can. `--saved` on a verb of another kind than
     the search's is an error naming both kinds, and `--search`, `--filter` and `--sort` beside it
     narrow or replace the saved query for that run only.

10. **A stale reference never widens the result.**
    - A tag or record a clause names that no longer exists is removed from the clause's values. It
      matches what that value would have matched: nothing. *has any of [To verify]* over a deleted
      tag finds no one, and *has none of* it finds everyone.
    - A clause naming a column the catalogue no longer has, or an operator its type no longer takes,
      cannot be evaluated. The saved search does not run. It opens with that clause marked, for the
      user to change or remove. The CLI fails naming the clause.

    Dropping such a clause and running the rest would return more records than the search asked for.
    A bulk action or an export over that result would then reach records nobody chose.

## Rationale

- **A typed catalogue (1, 2)** is what makes sorting, filtering and columns one feature rather than
  three. The row's display strings cannot be sorted by date or compared numerically, and a filter UI
  built per kind would drift across thirteen lists. Fixing the operators by type keeps the filter
  editor generic.
- **Evaluation in the store (4)** is the step [`decisions.md`](../decisions.md) recorded as missing
  before the Explorer can page its fetch. An in-memory filter over a paged read can only filter the
  page it has.
- **A materialised list table (5)** over read-time joins: #673's scoped joins make a page cheap only
  because the page is chosen by `human_id` first. A filter or sort on a joined value (birth date, an
  event's place) has to know that value for every record before it can pick the page. The
  write-side cost is bounded by `record_links`, which already names a record's
  dependents.
- **Workspace scope for saved searches (8)**, presentation scope for columns and sort (7): a saved
  search is part of a research effort on this dataset, shared with whoever works on it; which columns
  one person likes to see is not.
- **The two-line row (7)** keeps the Explorer usable at its width beside the editor. A table with a
  header per column needs a horizontal scroll there.
- **AND only (2)**: a researcher's recurring questions are conjunctions, and an OR is two saved
  searches. Grouping adds an expression editor nobody has asked for.

## Consequences

### Positive

- One query shape serves the Explorer, the palette, the CLI and saved searches.
- The Explorer pages its fetch at 100k, and the full tag scan is gone.
- A saved search survives backup and restore, and stays valid when a tag or record is renamed.

### Negative / costs

- A projection write now also writes the list table, and a change fans out to dependents' rows.
- One new table per listed kind, on both engines, filled by rebuild when a workspace predating it is
  opened.
- `record_links` widens past matchable kinds, and every tag-bearing kind becomes a link source (6).
- A column one reference away stays current only through that reference. A value two hops out is not
  a column (1), so the fan-out stays one level deep.
- `search` loses its match on localized labels, a behaviour change users of the text box will notice.
- A saved search is stored outside the event log, so it carries no operator or audit trail.

## Out of scope

- OR and grouped filters; full-text ranking; search across kinds (the palette stays the global
  search).
- A wide, table-shaped list view.
- Shared column layouts: columns and sort stay per client.
- The Matches queue filter (`MatchQueueFilter`), which filters pairs, not records, and stays its own.

## References

- ADR 0009 — the read model the list table extends (§3 JSON paths, §4 indexes on measurement,
  §5 disposable views).
- ADR 0015 — the configuration scopes saved searches (workspace) and columns (presentation) fall in.
- ADR 0021 §2 — vitals are events, so a person's dates are a cross-aggregate column.
- ADR 0041 — backups carry `workspace.toml`, and so the saved searches.
- ADR 0047 — `record_links`, which names a record's dependents and gains the `tagged` relation.
- #591 (saved searches) and #592 (column chooser), the design questions this answers.
