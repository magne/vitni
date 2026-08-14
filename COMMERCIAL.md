# Commercial licensing

Vitni's application layer is `AGPL-3.0-or-later`. For most people that is the end of it: use the
program, modify it, run it, distribute it, at no cost and with no agreement to sign.

You need a commercial licence only if the AGPL's reciprocity is a problem for you — typically
because you want to **embed Vitni's application crates in a product whose source you do not intend
to publish**, or **offer a modified Vitni as a network service without publishing those
modifications** (AGPL section 13).

## What you probably do not need a licence for

- **Using Vitni**, including inside a company, on any number of machines, on private data.
- **Modifying it for your own use** and never distributing the result.
- **Writing a plugin** — including a proprietary one you sell. A WebAssembly component that talks to
  the host only through the versioned `vitni:host-api` WIT world is covered by the additional
  permission under AGPLv3 section 7 stated in [`NOTICE`](NOTICE), so it is not required to be AGPL.
  That permission is what makes third-party plugins safe to redistribute, and it costs nothing.
- **Building on the interchange crates** — `vitni-interchange`, `vitni-gedcom`, `vitni-gramps-xml`
  and `vitni-i18n` are `MIT OR Apache-2.0`, with no reciprocity at all.
- **Redistributing Vitni**, including selling copies, under the AGPL's own terms. The name and logo
  are a separate matter (below).

If your case is on this list, no conversation is needed.

## What an exception covers

A commercial licence is an **additional grant to you**, on negotiated terms, over the same code:
typically the right to use and distribute the AGPL crates within a closed-source product without the
AGPL's source-availability obligations.

It never withdraws anything from anyone else. The public licences are irrevocable for the versions
they were granted under, and everything released stays released.

## What it does not cover

- **Support, maintenance or a service-level agreement.** Those are separate arrangements; a licence
  grant alone comes with the same "as is" disclaimer as the public licences.
- **A warranty or an indemnity**, unless one is negotiated explicitly.
- **The right to sublicense** — an exception is granted to you, not to your customers to pass on,
  unless the agreement says otherwise.
- **The Vitni name, logo or other trademarks.** A rebranded distribution needs a separate trademark
  permission regardless of which licence you hold.
- **Third-party dependencies.** Vitni's dependency tree is permissively licensed, but their terms
  are theirs; a Vitni exception says nothing about them.

## How to ask

Email **magne.rasmussen@gmail.com** with:

1. What you want to build and which parts of Vitni it would include.
2. Whether you intend to distribute it, host it, or both.
3. Rough scale — internal tool, product, number of end users.

There is no published price list. The reasoning behind the licence arrangement, including what it
does and does not protect, is in
[`docs/research/licensing-and-monetization.md`](docs/research/licensing-and-monetization.md) and
[ADR 0034](docs/adr/0034-licence-split-agpl-application-permissive-interchange.md).
