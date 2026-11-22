---
title: "Roadmap"
description: "Where Fidryn is going: United States law held as a body of checked programs, what exists today at each depth, and what comes next."
url: "https://fidryn.onlygass.dev/docs/roadmap"
markdown: "https://fidryn.onlygass.dev/docs/roadmap.md"
author: "Bryan Gass"
---

> Canonical HTML: https://fidryn.onlygass.dev/docs/roadmap
> This markdown mirror is for agents and plain-text readers.

# Roadmap

Fidryn is being built toward one destination: United States law held as a
body of checked programs. This page says what that means, what exists today,
and what comes next. It gives no dates. Everything it says exists is built
and tested; everything else is intent, and is marked as such.

## The destination

Every provision of federal and state positive law has a place in a
repository of Fidryn modules, at a declared depth:

| Depth | Meaning |
| --- | --- |
| D0 pinned | The official text is present and verified by hash, with its citation and where and when it was retrieved. |
| D1 addressed | The provision has a typed address. Its place in the hierarchy, its status and its cross-references are resolved and checked. |
| D2 classified | What kind of provision it is has been recorded, and a definition carries its term and its reach. |
| D3 formalized | Everything mechanical in it executes. Everything that calls for judgment is an explicit, typed open question that names who decides and under what standard. No operative word is left unrepresented. |

Two marks sit beside depth. **Tested** means the encoding reproduces worked
examples from authoritative material. **Reviewed** means a named, licensed
attorney signed off on a specific version.

Three rules hold at every depth:

- **Coverage is total and depth is graded.** A provision that is not yet
  formalized is still present, addressed and citable. A computation that
  reaches it stops and quotes it; it does not guess.
- **The weakest link shows.** A result reports every provision it relied on,
  with its address, hash, depth and marks. If any of them is unreviewed,
  every rendering of the result says so.
- **A likely answer is not the answer.** Statistics about how courts have
  ruled, when they come, attach to one named open question at a time, show
  their sample, and never turn an undetermined result into a determined one.

The language itself has one bar: any provision of law, public or private, can
be stated in it without the compiler knowing a single legal word. A clause of
a trust, a bylaw or a term of a contract is a provision on the same footing
as a section of a code, and every word of legal vocabulary is declared in
library modules written in Fidryn.

## Where the project is today

| Body of law | State |
| --- | --- |
| The United States Code | The importer has imported the whole Code at D1 (release point 119-142): 70,794 modules, 774,018 provisions, every cross-reference in operative text resolved or recorded with a reason. Definitions are extracted at D2 by a closed list of patterns; a sample of them has been drawn and has not yet been read against the official text by a person. All 27 release points the publisher offered for 2026 have been imported as a history, each with its provision-level change set. The full import is kept in a separate law repository that is not public yet. This repository holds a sample of it, checked in CI, and everything needed to reproduce the whole from the official archives. See [Law modules](https://fidryn.onlygass.dev/docs/law-modules.md) and [Coverage](https://fidryn.onlygass.dev/docs/coverage.md). |
| Federal court rules | In the Code's appendices, imported with it. |
| The Code of Federal Regulations | Not started. |
| The Constitution, public laws | Not started. |
| The fifty states, the District of Columbia, the territories | Not started. |
| Court rulings | Not started. |
| Meaning that executes (D3) | None yet on the imported Code: a curated module can bind a name to the text of a provision by hash and learns when that text changes, but the language cannot yet state what the provision means. The first edition of the language runs the example programs described in [Implementation status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md). |

Nothing has been reviewed by an attorney, and nothing here is legal advice.

## What comes next, in order

Each stage ends in a release.

1. **The semantic core, on real provisions.** Definitions, rules with
   exceptions, legal time, amounts and tables, typed facts, open questions
   with names and typed answers, evaluation as of a date, and results in
   which every step cites the provision it rests on. A conformance set of
   real provisions is taken to D3 with tests.
2. **Positions, instruments and organizations.** Duties, powers and
   entitlements; private documents as sources of rules, with the statutory
   defaults they may displace and the mandatory rules they may not; bodies
   and voting; statuses. The existing examples are ported and the first
   edition of the language is retired.
3. **Authority and interpretation.** The regulations imported and attached to
   the statutes they refine; which source prevails over which; precedential
   rulings as changes to the law, scoped to the courts they bind.
4. **Rulings as data.** Records of how courts resolved named open questions,
   and estimates drawn from them that are kept apart from the legal answer. A
   pilot on federal sentencing is to reproduce the Sentencing Commission's
   own published figures from its raw records.
5. **Scale.** Importers for the states; encodings drafted by machine, always
   labeled as such; a workflow for attorney review; jobs that commit each
   official release as it is published.
6. **A stable language.** A frozen specification, editor support,
   documentation written for lawyers, and public coverage figures for every
   body of law.

The import of text does not wait for the language. The regulations and the
states can reach D1 while the semantic core is being built.

## What will not be claimed

- That a provision is encoded when its operative words are not all
  represented.
- That an encoding is reliable because it runs. Reliance needs review by a
  licensed attorney, and the project shows at all times which logic has had
  it.
- That an estimate of how courts tend to rule is what the law requires.
- That text is official because it is hashed. A hash proves which bytes were
  encoded and where they came from.
