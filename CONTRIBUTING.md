# Contributing to Vitni

Bug reports, reproductions and pull requests are all welcome. Two things are worth reading before
you open a pull request: how the repository expects a change to be shaped, and the licence terms
your contribution comes in under.

Participation is under the [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) (Contributor Covenant 3.0).

## Before you start

- **Build and test setup** is in [`docs/development.md`](docs/development.md) — platform
  prerequisites, the two GUI test layers, and the repository's own tooling. It is not repeated here.
- **The architecture is written down.** [`docs/data-model.md`](docs/data-model.md) is the domain
  vocabulary and [`docs/adr/`](docs/adr/) holds the decisions. A change that contradicts an accepted
  ADR needs a new ADR superseding it, not an edit — accepted ADRs are immutable.
- **The backlog is [`docs/issues.md`](docs/issues.md)**, and its *Decided — no action needed* section
  records deliberate non-tasks. Check there before fixing something that looks broken.
- **Discuss anything large first.** Open an issue. A rejected 2000-line pull request wastes your time
  more than a paragraph does.

## What a good pull request looks like

- One logical change per commit; imperative subject line, 72 characters or fewer.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets --all-features -- -D warnings` and
  `cargo nextest run --workspace --all-features --lib --bins --tests` all clean. The workspace denies
  `unwrap_used`, `panic`, `todo` and friends, and silencing a lint with `#[allow(…)]` is itself
  denied — fix the code.
- Tests that would fail without your change. Break it deliberately once and confirm the test notices.
- Every user-facing string goes through Fluent (ADR 0003); no hardcoded literals in a frontend.
- Every UI change updates [`docs/mockups/`](docs/mockups/) in the same change.
- `prek run` passes (`prek install` once, to get the hooks).
- The pull request description says what the code does now — not which approaches you discarded.

### The checks are local — CI does not run

The workflows under `.github/workflows/` are committed and lint-clean but **never execute**: GitHub
Actions billing is disabled for this repository. So no check runs on your pull request, and there is
no CI badge in the README to render a misleading green.

Run them yourself before pushing — `prek run` plus the commands above, and `cargo xtask check` for the
i18n, CSS and input-handling guards. Labels are reconciled with `cargo xtask labels --apply` rather
than through `labels.yml`. [`docs/development.md`](docs/development.md) has the full command set.

## Sign your commits (DCO)

Every commit needs a `Signed-off-by` line, which `git commit -s` adds:

```text
Signed-off-by: Your Name <your.email@example.com>
```

That line is the [Developer Certificate of Origin 1.1](https://developercertificate.org/): a
statement that you wrote the contribution, or have the right to submit it under the terms below.

## Licence of your contribution

Vitni is licensed per crate — `MIT OR Apache-2.0` on the interchange crates, `AGPL-3.0-or-later` on
the application, with an additional permission under AGPLv3 section 7 for WebAssembly plugin
components. [`NOTICE`](NOTICE) has the mapping and
[ADR 0034](docs/adr/0034-licence-split-agpl-application-permissive-interchange.md) the reasoning.

**By submitting a pull request you grant Magne Rasmussen the following, for each contribution you
submit, and you confirm this by ticking the box in the pull-request template.**

1. **Copyright licence.** You grant a perpetual, worldwide, non-exclusive, irrevocable, royalty-free
   licence to reproduce, prepare derivative works of, publicly display, publicly perform,
   **modify**, **distribute**, **sublicense** and **relicense** your contribution and such
   derivative works, in source or object form, **including under licence terms different from the
   ones this project currently uses, and including for commercial purposes** — for example, granting
   a paid exception to a company that needs the application layer under non-copyleft terms.
2. **Patent licence.** You grant a perpetual, worldwide, non-exclusive, irrevocable (except as
   stated below), royalty-free patent licence to make, use, sell, offer to sell, import and
   otherwise transfer your contribution, covering those patent claims of yours that the contribution
   alone, or its combination with the project, necessarily infringes. If you institute patent
   litigation alleging that the project or a contribution to it constitutes patent infringement,
   this patent licence terminates as of the date the litigation is filed.
3. **You keep your copyright.** This is a licence, not an assignment. You may use your contribution
   however else you like, including under other terms.
4. **Attribution.** Your authorship of your contribution will be credited in the git history and, for
   substantial contributions, in the project's public credits. Nothing here asks you to waive
   attribution or the other moral rights that Norwegian law
   ([åndsverkloven](https://lovdata.no/dokument/NL/lov/2018-06-15-40) §5) does not permit you to
   waive.
5. **Warranties.** You confirm you are legally entitled to grant the above — that the contribution is
   your original work, or that you have the necessary rights to submit it, and that your employer, if
   your employer has rights to work you produce, has waived them or authorised you to submit it. The
   contribution is provided "as is", without warranty of any kind.

### Why the grant is this broad, and why it is worded so explicitly

The broad part is deliberate. A commercial exception for a company that needs the application layer
under non-copyleft terms, a store build, and any future relicensing all require the ability to
licence the whole work on terms other than the public one. That ability ends the day a contribution
lands without a grant like this, and it cannot be recovered afterwards without tracking down every
contributor.

The honest objection — *"you may sell my volunteer work under closed terms"* — is real, and the
answer is not that it will not happen. It may. What is fixed in exchange is that everything you
contribute stays available to everyone under the public licences above, permanently; a paid exception
is an additional grant to one customer, never a withdrawal from anyone else. If that trade is not one
you want to make, please say so on the issue rather than sending code — no hard feelings, and a good
bug report is worth a great deal on its own.

The explicit part is legal necessity, not lawyering for its own sake. Norwegian copyright law
codifies the *spesialitetsprinsippet* in åndsverkloven §67 second paragraph: on a transfer of
copyright, the author is not deemed to have transferred a more extensive right than the agreement
clearly expresses. Breadth by implication is exactly what the statute refuses to read in, so
sublicensing, relicensing and commercial exploitation are named rather than gestured at.

### Third-party code

Do not paste code from another project into this one. In particular, **never copy
[Gramps](https://github.com/gramps-project/gramps) source** — Vitni's Gramps-shaped domain model is a
clean-room reimplementation from the published data model, and copying GPLv2+ code into it would
force a relicense of the whole application. If a contribution needs a new dependency, say so in the
pull request; it must be permissive-compatible, and `cargo deny check` enforces that.

## Reporting a security issue

Do not open a public issue. [`SECURITY.md`](SECURITY.md) has the private disclosure route, what is in
scope, and what to expect.
