## Import plan and review (ADR 0040 §4)
import-plan-heading = Plan for { $source }:
import-plan-kind = { $kind ->
    [person] persons
    [family] families
    [event] events
    [place] places
    [source] sources
    [citation] citations
    [media] media
    [note] notes
    [repository] repositories
    [tag] tags
   *[other] { $kind }
}
import-plan-row = { $kind }: { $counts }
import-plan-new = { $count } new
import-plan-unchanged = { $count } unchanged
import-plan-updated = { $count } updated
import-plan-linked = { $count } already in the tree
import-plan-candidates = { $count } with possible matches
import-plan-withheld = { $count } kept as another tree recorded them
import-plan-all-unchanged = Every record is already on record: importing this file writes nothing.
import-plan-empty = The file holds no records.
import-plan-review-hint = { $count ->
    [one] 1 record has possible matches: import on a terminal to review it, or pass --defer-matches to decide it later.
   *[other] { $count } records have possible matches: import on a terminal to review them, or pass --defer-matches to decide them later.
}
import-review-kind = { $kind ->
    [person] person
    [place] place
    [source] source
    [repository] repository
    [family] family
    [event] event
    [citation] citation
    [media] media object
    [note] note
   *[other] { $kind }
}
import-review-band = { $band ->
    [deterministic] established identity
    [probable] probable match
   *[other] possible match
}
import-review-position = Possible match { $position } of { $total }: { $kind }, { $band } ({ $score }%)
import-review-stored = In your tree: { $label } ({ $id })
import-review-incoming = Importing:    { $label } ({ $record })
import-review-row = { $feature }: { $outcome }
import-review-prompt = Is it the same? [y] yes, [n] no, [l] decide later, [r] decide the rest later, [c] cancel the import:
import-review-prompt-group = Is it the same? [y] yes, [n] no, [l] decide later, { $count ->
    [one] [a] treat the last probable { $kind } match as the same,
   *[other] [a] treat all { $count } probable { $kind } matches as the same,
} [r] decide the rest later, [c] cancel the import:
import-review-feature-given-name = given name
import-review-feature-surname = surname
import-review-feature-sex = sex
import-review-feature-birth = birth
import-review-feature-death = death
import-review-feature-birth-place = birth place
import-review-feature-death-place = death place
import-review-feature-lifespan = lifespan
import-review-feature-father = father
import-review-feature-mother = mother
import-review-feature-partners = partners
import-review-feature-children = children
import-review-feature-patronymic = patronymic
import-review-feature-occupation = occupation
import-review-feature-record = source record entry
import-review-feature-partner = partner
import-review-feature-marriage = marriage
import-review-feature-marriage-place = marriage place
import-review-feature-event-type = event type
import-review-feature-date = date
import-review-feature-place = place
import-review-feature-principal = principal
import-review-feature-participants = participants
import-review-feature-place-name = place name
import-review-feature-place-type = place type
import-review-feature-enclosure = location
import-review-feature-coordinates = coordinates
import-review-feature-title = title
import-review-feature-author = author
import-review-feature-publication = publication
import-review-feature-repository = repository
import-review-feature-name = name
import-review-feature-address = address
import-review-feature-source = source
import-review-feature-page = page
import-review-feature-checksum = checksum
import-review-feature-path = file name
import-review-feature-text = text
import-review-outcome-agree = same
import-review-outcome-partial = similar
import-review-outcome-disagree = different
import-review-outcome-missing = not compared
import-review-outcome-conflict = conflicting
