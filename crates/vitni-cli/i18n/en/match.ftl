## Match output (ADR 0039 §3)
match-list-empty = No possible matches.
match-line = { $kind }  { $a }  { $b }  { $band } ({ $score }%)
match-show-heading = { $kind } { $a } ⇄ { $b }: { $band } ({ $score }%)
match-show-decided = { $decision ->
    [same] Already decided: the two are one record.
   *[distinct] Already decided: the two are not the same.
}
match-decided-same = { $second } is merged into { $first }; one event added to History.
match-decided-distinct = { $first } and { $second } are marked as not the same; one event added to History.
