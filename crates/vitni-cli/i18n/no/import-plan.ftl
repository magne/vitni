## Importplan og gjennomgang (ADR 0040 §4)
import-plan-heading = Plan for { $source }:
import-plan-kind = { $kind ->
    [person] personer
    [family] familier
    [event] hendelser
    [place] steder
    [source] kilder
    [citation] kildehenvisninger
    [media] medier
    [note] notater
    [repository] arkiver
    [tag] etiketter
   *[other] { $kind }
}
import-plan-row = { $kind }: { $counts }
import-plan-new = { $count } nye
import-plan-unchanged = { $count } uendret
import-plan-updated = { $count } oppdatert
import-plan-linked = { $count } allerede i treet
import-plan-candidates = { $count } med mulige treff
import-plan-withheld = { $count } beholdt slik et annet tre registrerte dem
import-plan-all-unchanged = Alle poster er allerede registrert: å importere denne filen skriver ingenting.
import-plan-empty = Filen har ingen poster.
import-plan-review-hint = { $count ->
    [one] 1 post har mulige treff: importer i en terminal for å gå gjennom det, eller bruk --defer-matches for å avgjøre det senere.
   *[other] { $count } poster har mulige treff: importer i en terminal for å gå gjennom dem, eller bruk --defer-matches for å avgjøre dem senere.
}
import-review-kind = { $kind ->
    [person] person
    [place] sted
    [source] kilde
    [repository] arkiv
   *[other] { $kind }
}
import-review-band = { $band ->
    [probable] sannsynlig treff
   *[other] mulig treff
}
import-review-position = Mulig treff { $position } av { $total }: { $kind }, { $band } ({ $score } %)
import-review-stored = I treet ditt: { $label } ({ $id })
import-review-incoming = Importeres:   { $label } ({ $record })
import-review-row = { $feature }: { $outcome }
import-review-prompt = Er det den samme? [y] ja, [n] nei, [l] avgjør senere, [r] avgjør resten senere, [c] avbryt importen:
import-review-prompt-group = Er det den samme? [y] ja, [n] nei, [l] avgjør senere, { $count ->
    [one] [a] behandle det siste sannsynlige treffet av typen { $kind } som det samme,
   *[other] [a] behandle alle { $count } sannsynlige treff av typen { $kind } som de samme,
} [r] avgjør resten senere, [c] avbryt importen:
import-review-feature-given-name = Fornavn
import-review-feature-surname = Etternavn
import-review-feature-sex = Kjønn
import-review-feature-birth = Fødsel
import-review-feature-death = Død
import-review-feature-birth-place = Fødested
import-review-feature-death-place = Dødssted
import-review-feature-lifespan = Levetid
import-review-feature-father = Far
import-review-feature-mother = Mor
import-review-feature-partners = Partnere
import-review-feature-children = Barn
import-review-feature-patronymic = Patronym
import-review-feature-occupation = Yrke
import-review-feature-record = Kildepost
import-review-feature-partner = Partner
import-review-feature-marriage = Ekteskap
import-review-feature-marriage-place = Vigselssted
import-review-feature-event-type = Hendelsestype
import-review-feature-date = Dato
import-review-feature-place = Sted
import-review-feature-principal = Hovedperson
import-review-feature-participants = Deltakere
import-review-feature-place-name = Stedsnavn
import-review-feature-place-type = Stedstype
import-review-feature-enclosure = Beliggenhet
import-review-feature-coordinates = Koordinater
import-review-feature-title = Tittel
import-review-feature-author = Forfatter
import-review-feature-publication = Utgivelse
import-review-feature-repository = Oppbevaringssted
import-review-feature-name = Navn
import-review-feature-address = Adresse
import-review-feature-source = Kilde
import-review-feature-page = Side
import-review-feature-checksum = Sjekksum
import-review-feature-path = Filnavn
import-review-feature-text = Tekst
import-review-outcome-agree = lik
import-review-outcome-partial = lignende
import-review-outcome-disagree = ulik
import-review-outcome-missing = ikke sammenlignet
import-review-outcome-conflict = i strid
