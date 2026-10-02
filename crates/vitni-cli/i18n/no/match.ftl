## Match output (ADR 0039 §3)
match-list-empty = Ingen mulige treff.
match-line = { $kind }  { $a }  { $b }  { $band } ({ $score } %)
match-show-heading = { $kind } { $a } ⇄ { $b }: { $band } ({ $score } %)
match-show-decided = { $decision ->
    [same] Allerede avgjort: de to er én post.
   *[distinct] Allerede avgjort: de to er ikke de samme.
}
match-decided-same = { $second } er slått sammen med { $first }; én hendelse lagt til i historikken.
match-decided-distinct = { $first } og { $second } er markert som ikke de samme; én hendelse lagt til i historikken.
