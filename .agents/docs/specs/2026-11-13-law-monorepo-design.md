# Fidryn as the language of a United States law monorepo — design

Date: 2026-11-13. Status: approved by the owner on 2026-11-13.
Baseline: `dev` at `1a70382` (the 0.2.0 release candidate, PR #3).

This document sets the destination for Fidryn and designs the first two
stages in enough detail to plan from. Later stages are outlined; each gets
its own design document when it is reached.

Decisions are marked by who made them. **Owner** means Bryan decided it in
conversation. **Call** means the author of this document chose it. The
owner read the document and approved it as a whole ("I think this seems
solid"), which settles the Calls listed in section 14 for planning. The
owner can still overrule any of them.

## 1. Intent

The owner asked for three things:

1. Fidryn should stop being a toy and become something practicing lawyers
   and judges would rely on.
2. The examples should grow into an encoding of all United States federal
   law and the law of all fifty states.
3. The repository, documentation and website should hold up to a hostile
   reader.

The owner then gave the framing that governs this design (**Owner**):

> What you describe basically just sounds like a massive monorepo with
> source control and massive amounts of daily commits. Our job is to make
> an expressive enough language such that we can make that monorepo real.

That framing is close to literal. Congress writes amendments as edit
instructions ("section 2611 is amended by striking … and inserting …").
The House publishes a complete new edition of the United States Code after
each public law. The District of Columbia publishes its official code from
a git repository. What does not exist is a language whose compiler can
check that body of text as a program and run the parts that are mechanical.

The owner also set the breadth (**Owner**): the language must work for
accounting law, corporate law, marriages, trusts, small businesses and
"basically any other form of U.S. law there is". Examples in every area
are not required. Easy support for every area is. Sections 4.2 and 5.2
turn that into something checkable.

The owner fixed the shape of the repositories (**Owner**): the language
lives in one repository, and the encoding of United States law is built
in a separate one, as an example of the language in use. That second
repository is the showcase, and it doubles as the language's integration
test.

The owner also proposed an empirical layer (**Owner**):

> We can run code and see likely output, but if that was statistically
> rooted in how actual rulings occurred then we have a system that
> improves for every ruling that occurs.

Section 4.4 says how that can be done without weakening the governing
rule in section 2.

The owner asked that this document supply the legal facts the work
depends on. Section 3 gives the ones a builder needs.

## 2. Where the implementation stands

Measured on `1a70382`.

**Healthy.** Fourteen crates, about 80,000 lines of Rust. All 1,188 Rust
tests pass (`cargo test --workspace --offline`). CI, release packaging,
branch protection and the site generator work.

**The surface is much larger than what executes.** These were verified in
the source, not taken from the status documents:

| Finding | Evidence |
| --- | --- |
| For most declaration kinds the parser reads the name and optional parameters, then skips the body. The body survives only as raw text. This covers `power`, `legal_act`, `duty`, `office`, `clause`, `decision`, `interpretation_family`, `conflict_doctrine`, `scenario`, `nomination`, `effect`. | `crates/fidryn-syntax/src/parser.rs:882-923` |
| Later stages recover fields by splitting that text on words (`"artifact"`, `" for "`, `" rank"`, `"alternative "`). | `crates/fidryn-hir/src/lib.rs:377-381`, `:914-979` |
| Rule verbs other than `establish`, `constitute`, `terminate`, `suspend` and `create` silently become `derive`. That includes `activate`, `defeat` and `make`. | `crates/fidryn-check/src/lib.rs:3634-3645` |
| The evaluator knows the example programs by name: `ChildSupportWaiver`, `SpousalSupportWaiver`, `Incapacitated`, `Eligible`, `AcceptOffice`, `OfficialFilingRecord`, `FormedLLC`, `PaymentRecord`, `invoice_date`. | `crates/fidryn-eval/src/lib.rs:3613`, `:4088`, `:4504`, `:4548`, `:4791`, `:4986`; `duty.rs:701`, `:944` |
| Federal income tax is a Rust function with four brackets. Income above $103,350 is taxed at 24%. | `crates/fidryn-eval/src/lib.rs:3174-3195` |
| The type system has built-in sorts for the three original examples (`Trust`, `PremaritalAgreement`, `ProposedLlc`). | `crates/fidryn-core/src/types.rs:110-132` |
| A duration cannot be added to a time in an expression. Date literals are untyped. There are no month or year units, no time zones, and `days`, `calendar_days` and `counted_days` compute identically. | checker `crates/fidryn-check/src/lib.rs:1381`; `crates/fidryn-core/src/time.rs:153-161`; `crates/fidryn-eval/src/duty.rs:858-920` |
| A module cannot import another module. Compilation takes one file plus a narrow `packages/` mechanism. An `import` without a digest is a no-op, and `alias` is ignored. | `crates/fidryn-check/src/lib.rs:2232-2270` |
| No outcome or trace node carries a reference to the source it rests on. | `crates/fidryn-eval/src/trace.rs:7-14` |
| `source` declarations are parsed and then dropped. Citation, authority, digest and effective interval never reach the intermediate representation. | no `CoreDecl::Source` is constructed in `crates/fidryn-check/src/lib.rs` |
| 94 of the 114 `.fr` files in the repository fail `fidryn check`, including all fifty state modules. No test covers them. Two READMEs say they all pass. | run of the 0.2.0 binary over `examples/`, `prelude/`, `packages/`, `tests/programs/` |

The three survey maps that produced the wider picture are summarized in
section 12; their unverified claims are to be re-checked before anyone
relies on them.

**What is sound and worth keeping.** The vocabulary of the language is
well chosen: propositions that are not Booleans, offices, judgments with
named deciders, interpretation families, duties, outcomes that can be
`Suspended` or `Contingent` instead of falsely determinate. The governing
rule is right and stays:

> A computation may return one determinate result only when that result is
> invariant across every still-admissible resolution of the unresolved
> issues, or when a competent authority has already made a determination
> that is operative in the relevant context.

The lexer, the expression parser, the lossless syntax tree, the outcome
and request types, the handler model, the report and trace formats, the
replay checker, the driver's artifact capture, the CLI, the local web
interface, the site generator and the CI configuration are all kept.

## 3. The law, measured

### 3.1 The United States Code

Profiled from the official XML edition published by the Office of the Law
Revision Counsel (release point "Public Law 119-142", published
2026-11-13, one 109 MB archive):

| Measure | Count |
| --- | --- |
| Files (titles and appendices) | 58 |
| Sections | 60,490 (51,009 operative; 5,131 repealed; 2,037 omitted; 1,911 transferred; 368 renumbered; 17 vacant; 16 reserved; 1 with an unrecognized status) |
| Addressable provisions, from section down to the deepest sub-item | 772,863 |
| Provisions with text of their own | 671,684 |
| Deepest nesting below a section | 9 levels |
| Words of operative text | 22.9 million (54.5 million including notes) |
| Machine-readable cross-reference links | 1,301,031 |
| Cross-references to the Code inside operative text | 90,719, of which 88,935 (98.0%) resolve to an exact provision in the same release. Of the other 1,784, 820 reach an existing section but not the exact sub-provision, and 964 reach no section. |
| Tables | 9,842 |
| Amendment notes | 25,944 |

Each provision has a stable official identifier, for example
`/us/usc/t29/s2611/2/B/ii`. Each section lists every public law that
amended it. The federal court rules (civil, criminal, appellate, evidence,
bankruptcy) are in the same release as title appendices, for example
`/us/usc/t28a/courtRules/Civil/rule6`.

Counts in this table cover the Code's own provisions. Text inside notes,
quoted provisions, tables of contents and source credits is excluded from
the provision, word and reference counts unless a row says otherwise.

Each title's metadata says whether it is positive law (`is-positive-law`
is `no` for Title 29 and `yes` for Title 18). Notes under a section are
grouped by the publisher into editorial notes, statutory notes and
executive documents. Statutory notes are enacted law that the editors set
out beneath a section instead of in it: effective dates, transition rules,
free-standing provisions. Notes have no stable identifier, only an id
that changes with each release.

The publisher currently offers 27 dated release points for 2026
(119-73 of 2026-01-23 through 119-142), each a complete edition, each
labeled with the titles it changed. That is a ready-made commit history.

How often provision text uses each drafting construct (provisions
containing the pattern, out of 671,684):

| What the text does | Indicative count |
| --- | --- |
| Imposes or permits: "shall" / "may" / "shall not" / "may not" | 207,947 / 97,915 / 25,155 / 11,068 |
| Defines a term: "the term … means" / "includes" / "does not include" / "has the meaning given" | 28,730 / 4,213 / 2,122 / 3,898 |
| Sets a definition's reach: "as used in this …" / "for purposes of this …" | 21,543 / 18,297 |
| States an exception or override: "subject to" / "except" / "unless" / "notwithstanding" / "shall not apply" | 23,010 / 22,983 / 11,990 / 9,185 / 7,688 |
| Hands a question to someone: "determines" / "regulations" / "considers appropriate" / "discretion" | 43,389 / 28,327 / 7,770 / 2,733 |
| Uses an open standard: "appropriate" / "reasonable" / "substantial" / "material" / "good faith" | 30,380 / 12,572 / 5,612 / 4,896 / 1,029 |
| Fixes time: explicit date / "fiscal year" / N days / N years / "not later than" / N months | 26,749 / 19,956 / 15,375 / 14,384 / 14,048 / 5,120 |
| Computes: "percent" / dollar amount / "exceeds" / "the lesser or greater of" / rounding | 17,883 / 16,752 / 14,175 / 1,417 / 650 |
| Attaches a consequence: imprisonment / liability / civil penalty / fine | 2,487 / 2,269 / 1,817 / 1,656 |
| Regulates procedure and proof: notice / hearing / appeal / presumption / standard of proof | 12,051 / 4,157 / 1,972 / 646 / 616 |
| Addresses other law: "construed" / State law / preemption | 5,037 / 3,405 / 962 |

These are pattern counts, not legal classifications. They are good enough
to rank what the language must express. The profiling script and its
output are working files under
`.agents/scratchpad/2026-11-13-usc-profile/` (git-ignored). Stage 1
reproduces the structural numbers in Rust as an independent cross-check.

### 3.2 Everything else

| Body of law | Official source | Notes |
| --- | --- | --- |
| Federal regulations (Code of Federal Regulations) | eCFR XML in bulk from govinfo; a versioning API with dated versions of each section | Verified live. Title 29 alone is 29 MB of XML. |
| Constitution | Short; pinned by hand from the Government Publishing Office text | Its meaning lives mostly in case law. |
| Public laws and Statutes at Large | govinfo | The Code release links to these 921,100 times, mostly in the credits that record which laws enacted and amended each section. |
| Fifty states | One publisher per state, with different formats and terms | Verified reachable: California daily database dumps, New York's legislation API, Texas statute downloads, Virginia's code API, DC's XML on GitHub. Most states publish HTML only. |
| Court opinions | CourtListener and the Harvard Caselaw Access Project (open bulk data) | Millions of opinions. They are the interpretive layer, not rules to transcribe. CourtListener publishes opinions, dockets, citations and a judge database as CSV snapshots each quarter, free of known copyright restrictions. |
| Coded outcomes of rulings | United States Sentencing Commission case-level datafiles; the Federal Judicial Center's Integrated Database of federal cases | Verified: the Commission publishes one record per sentenced individual for fiscal years 2002 to 2025 (CSV from 2012), with identifiers removed. Official coded outcomes exist for few other areas. |

### 3.3 Legal facts a builder needs

- **The Code is not the whole of federal law.** It restates most general
  and permanent statutes. Regulations, court rules, uncodified provisions
  and case law sit beside it.
- **Not every title is the law itself.** Titles enacted as "positive law"
  are legal evidence of the law. The other titles are only prima facie
  evidence; where they differ from the Statutes at Large, the Statutes
  control (1 U.S.C. § 204). A provision's evidentiary status is therefore
  data the language must carry.
- **Statutes delegate.** "As the Secretary may by regulation prescribe"
  gives an agency power to fill in a term. The regulation binds only
  within that grant. Since *Loper Bright Enterprises v. Raimondo* (2024),
  federal courts decide what a statute means without deferring to the
  agency's reading.
- **Courts fix meaning, and only within their reach.** A decision of a
  federal court of appeals binds the district courts in its circuit and
  nowhere else. Circuits disagree. The same statutory words can therefore
  mean different things in different courts until the Supreme Court
  resolves it.
- **Much private law is not in any statute.** Contract, tort and property
  rules are largely judge-made and differ by state.
- **Much law is a default, and private documents change it.** Corporate,
  partnership, trust, commercial and marital-property statutes are full
  of rules that apply "unless otherwise provided" in a charter, an
  agreement or a trust instrument. Beside them sit mandatory rules that no
  document can change. For the people it binds, the document is law. A
  language for corporate law, trusts or marriage must treat a private
  instrument as a source of rules on the same footing as a statute, and
  must know which statutory rules an instrument may displace.
- **Some binding standards are privately written.** United States
  accounting standards come from a private board whose work the
  securities regulator recognizes. Building, safety and industry codes
  are often written privately and adopted by reference. Their text is
  usually copyrighted: it can be cited and pinned by hash, but not
  necessarily stored or published.
- **Jurisdictions do not stop at federal and state.** The District of
  Columbia, the territories, tribal nations and thousands of local
  governments make law too. Business licenses, zoning and local wage
  rules are local.
- **Official text is free to use.** Works of the federal government have
  no copyright (17 U.S.C. § 105). Statutes and judicial opinions of any
  state are not copyrightable (*Georgia v. Public.Resource.Org*, 2020).
  Publishers' annotations and websites' terms of use are a separate
  matter and have to be checked state by state.
- **Online is not always official.** In many states the printed code is
  the official one and the website is not. A hash proves which bytes were
  encoded and where they came from. It does not make them official.
- **A ruling does two different jobs.** A precedential holding settles
  what words mean for the courts it binds. That changes the law. Most
  rulings do something else: they apply a settled standard to one set of
  facts ("was this delay reasonable?"). Those bind nobody else, but they
  are evidence of how that question tends to be answered.
- **Published rulings are not a fair sample.** Most disputes settle, and
  about one federal civil case in a hundred reaches trial. The opinions
  that get written and published over-represent close and contested
  questions. Counting them naively misleads.
- **Courts already use outcome statistics in one area.** The Sentencing
  Commission runs a tool "specifically developed with the needs of
  judges in mind" that shows how defendants with the same guideline,
  offense level and criminal history were actually sentenced, relative
  to the computed guideline range. It shows nothing when fewer than three
  cases match.
- **Statistics about named judges are a policy question.** They are
  lawful and sold commercially in the United States. France made
  publishing them a crime in 2019. Risk scores built from past outcomes
  have been challenged for carrying past bias forward (*State v. Loomis*,
  Wisconsin 2016).
- **Law has defects.** The Code contains bracketed editorial corrections
  such as "[probably should be 101(2)(A)]" and "[sic]", and about 2% of
  its internal references do not resolve. The language must represent
  such text faithfully and report the defect, not fail on it.
- **An encoding is a legal reading.** Lawyers answer for the tools they
  rely on (ABA Model Rules 1.1 and 5.3; ABA Formal Opinion 512). Courts
  have sanctioned lawyers for filing machine-generated material they did
  not verify (*Mata v. Avianca*, 2023). Neither the owner nor the author
  of this document is a licensed attorney. Review by one is a requirement
  for reliance, and the system must show at all times which logic has had
  it.

## 4. Destination

### 4.1 What "all of United States law is encoded" means

**Coverage is total; depth is graded.** Every provision of positive law
has a place in the monorepo. Each provision is at a declared depth:

| Depth | Meaning | How it is reached |
| --- | --- | --- |
| D0 Pinned | The official text is present and hash-verified, with its citation and where and when it was retrieved. | Mechanical |
| D1 Addressed | The provision has a typed address; its position in the hierarchy, its status and its cross-references are resolved and checked. | Mechanical |
| D2 Classified | What kind of provision it is (definition, duty, exception, delegation, penalty and so on) is recorded, and definitions carry their term and reach. | Mechanical for clear patterns; drafted otherwise |
| D3 Formalized | Everything mechanical in it executes. Everything that calls for judgment is an explicit, typed open question naming who decides and under what standard. No operative words are left unrepresented. | Drafted by people or machines |

Two independent marks sit beside depth: **tested** (the encoding
reproduces worked examples from authoritative material) and **reviewed**
(a named, licensed attorney signed off on a specific version).

Some text cannot be published: a copyrighted standard, a client's
confidential instrument. For such a source, D0 means its hash, citation
and retrieval record are present, and the text itself is held outside
the public repository.

**The weakest link shows.** A result reports every provision it relied
on, with address, hash, depth and marks. If any of them is unreviewed,
every rendering of the result says so, and no flag turns that off.

**Unformalized is not unusable.** If a derivation reaches a provision
below D3, the result is `Suspended` with a request that names the
provision and quotes its text. Total coverage at D1 therefore has value
on its own: the system knows the full dependency graph and knows exactly
what it does not know.

### 4.2 The bar for the language

**Expressive completeness (Call).** For any provision of positive law,
there is an encoding that:

1. preserves its structure and every operative element;
2. is bound to the exact pinned text it encodes, at the finest
   granularity the official text offers;
3. executes whatever is mechanical;
4. represents whatever is not mechanical as an explicit, typed open
   question;
5. carries its life in time: enactment, amendment, effective and
   applicability dates, repeal.

**Domain neutrality (Call, from the owner's breadth requirement).** A
"provision" here is not only a section of a code. It is equally a clause
of a trust, a bylaw, a term of a contract, a paragraph of an accounting
standard or a local ordinance. The bar applies to all of them, and adds
a sixth condition:

6. no area of law needs a change to the compiler or the evaluator. Every
   word of legal vocabulary (person, corporation, marriage, trust, share
   class, ledger) is declared in library modules written in Fidryn.

The bar is tested three ways: with a conformance set of real provisions
(section 10); once the Code is imported, by measuring what share of
provisions in each construct class the language can formalize; and for
condition 6, by the rule that adding a new area of law is done by writing
modules alone.

### 4.3 Two kinds of repository

```
official publishers                     this repository (the language)
  US Code release points                  compiler, runtime, standard library,
  eCFR versions                           ingesters, spec, conformance set
  state publishers                                     |
        |                                              | builds
        v                                              v
   ingest (deterministic)  ---->  law repository (the showcase monorepo)
                                    generated text modules   (never hand-edited)
                                    curated rule modules     (bound to text by hash)
                                    tests and review records
                                    ruling records           (section 4.4)
                                    one commit per official release
```

When a new official release arrives, the ingester regenerates the text
modules. Any curated encoding bound to text that changed is marked stale
and loses its `reviewed` mark until someone re-reviews it. That is the
mechanism that keeps a living monorepo honest.

The law repository is separate from this one (**Owner**). It is the
showcase: the place where Fidryn is used to encode United States law. It
is generated in part, large, and commits on the publishers' schedule.
This document calls it "the law repository" throughout.

It is also the language's integration test (**Owner**). How that works
(**Call**):

- The law repository records the version of the toolchain it is checked
  with.
- This repository's CI checks the law repository at a pinned commit. A
  compiler change that breaks it fails, unless the pin moves together
  with a matching change on the other side.
- A scheduled job checks the newest of each against the newest of the
  other and reports drift.
- This repository keeps a small pinned sample so that its own tests run
  without the other repository.

As the corpus grows past what one repository holds comfortably
(regulations, then states), the showcase becomes several repositories
under one owner, one per body of law, joined by a workspace.

Private work never goes into the law repository (**Call**). A matter,
such as one client's trust or one company's bylaws and ledgers, lives in
its own workspace. That workspace depends on a pinned release of the law
repository the way a program depends on a pinned library. The same
language and the same checks apply on both sides of that line.

### 4.4 Two answers: what the law determines, and what courts have done

The owner's idea is to ground the "likely output" of a program in how
courts have actually ruled, so the system improves with each ruling. The
formal encoding makes this unusually tractable, because it already
separates what is mechanical from what is someone's judgment. Statistics
do not have to predict a whole case. They attach to one open question at
a time, and the mechanical structure carries them through to the result.

Federal sentencing shows the shape. The guideline range is computed.
Whether to sentence inside it is the judge's decision, an open question.
Official data records how that question was answered in each case.

**Rulings enter the system in two ways, kept apart.**

| As | What it is | Effect |
| --- | --- | --- |
| Law | A precedential holding that fixes an interpretation for the courts it binds | A change to the law repository that selects or narrows a reading for that forum (Stage 4). The legal answer itself changes. |
| Data | An application of a settled standard to particular facts | A ruling record: one observation of how an open question was resolved (Stage 5). Only estimates change. |

**The rule that keeps it honest (Call).** A result carries two answers
and they are never merged:

1. The **legal answer** follows the governing rule and is untouched by
   statistics. An open question without a competent determination leaves
   the result `Suspended` or `Contingent`.
2. The **empirical answer** is an estimate: a distribution over the ways
   the open questions could be resolved, and therefore over the final
   result.

An estimate never removes a suspension and never yields a determinate
result. It always shows its evidence: the forum, the period, the number
of rulings, how each was labeled, the version of the law those rulings
applied, and the method. Below a stated minimum sample there is no
estimate, as in the Sentencing Commission's own tool.

**Ruling records (Call).** Each ruling is a datum in the law repository:
the court, the date, the citation or docket number, the address of the
open question it resolved, the version of the provision it applied, the
resolution, any findings on listed factors, the hash of the source
document, whether the ruling is precedential, and how the label was
obtained (a coded field in an official dataset, a person, or a model,
with that fact shown).

**The judge (Owner).** Who decided matters. A ruling record carries the
judge, or the panel with its author and dissenters, as an optional
field: present whenever the source identifies them, absent otherwise.
Estimates can therefore be narrowed to one judge when enough of that
judge's rulings are on record, and fall back to the court when they are
not. Judges are identified by a stable identifier from a public judge
database, not by a name string (**Call**).

Two things in the rest of this design exist to make that possible:

- Open questions are declared, typed and addressed, not anonymous
  (Stage 2). A ruling record needs something stable to point at.
- Provisions are versioned (Stages 1 and 2). Rulings under an older text
  or an overruled reading are not comparable with rulings under the
  current one, and the system can tell them apart.

**What makes it hard.** Section 3.3 gives the legal side: published
rulings are a biased sample, and statistics carry the past's biases
forward. The practical bottleneck is labeling. Most rulings are prose,
not coded outcomes, and official coded data exists for few areas.

**Mechanism (Call).** Open questions are already answered by
interchangeable handlers: the case file, a scenario, an exploration of
every admissible answer. The empirical layer is one more handler, which
answers with a distribution drawn from ruling records. The program is
the same.

**Method (Call).** Start plain: counts and intervals within a declared
comparison group, updated as each record arrives. Pooling across related
forums and conditioning on factor findings come later. Every estimate is
reproducible from a named snapshot of the records.

## 5. What the language needs

| Construct in the text | Language concept | Today |
| --- | --- | --- |
| Sections, subsections, clauses | **Provisions**: first-class, addressed, bound to pinned text | `source` blocks whose fields are dropped |
| "section 207 of this title" | **Typed citations** resolved across modules | Unchecked strings |
| "The term … means / includes / does not include" | **Definitions** with an exhaustive or extending body and exclusions | None |
| "As used in this subchapter" | **Scope** tied to a region of the hierarchy | One flat namespace per file |
| "has the meaning given in section …" | **Borrowed definitions** by address | None |
| General rule, then "except", "unless", "notwithstanding", "subject to" | **Defeasible rules**: a general case, exceptions and overrides, with priorities taken from the text | A fallback arm on rules; no priority |
| "shall", "may", "may not", "is entitled to" | **Positions** between parties: duty, prohibition, permission, power, entitlement | One specialized duty path |
| "as the Secretary determines", "reasonable", "good cause" | **Open questions**: who decides, on what record, under what standard | Present (`Determine`, `judgment`); narrow |
| "by regulation" | **Refinement**: a lower authority fills in a higher one's term, within its grant | None |
| Dates, periods, deadlines, fiscal and taxable years | **Legal time**: civil dates and time zones, month and year arithmetic with stated rounding, counting conventions, holiday calendars, windows and intervals | UTC instants; seven units; no expression arithmetic |
| Amounts, rates, tables, indexing | **Quantities**: money with rounding rules, tables, parameters with a value history, aggregation over collections | Decimal arithmetic; no aggregation; one hard-coded tax function |
| Facts about people and events | **Typed facts** with intervals | A map from strings to values |
| Amendments, effective dates, sunsets | **Versions**: each provision's text and rules over time; evaluation as of a date | Bitemporal case facts only |
| Statute over regulation, federal over state, binding precedent | **Authority and forum**: source hierarchy, preemption, interpretations scoped to a court's reach | Interpretation families; a name-level ranking |
| Filing, notice, hearing, appeal | **Procedures**: steps, actors, time limits and defaults | A sketch in `prelude/procedure.fr` that nothing loads |
| "Unless otherwise provided in the certificate of incorporation" | **Defaults an instrument may displace, and mandatory rules it may not**; what happens to a clause that oversteps (void, voidable, unenforceable) | None |
| Clauses of a trust, bylaws, a contract | **Instruments as sources of rules**, addressed and bound to their text like any provision | Three examples, each known to the evaluator by name |
| "A majority of the directors", "two-thirds of each class" | **Bodies**: membership, quorum, voting, and authority to act for an entity | One hard-coded succession pattern |
| "Is married", "is incorporated", "is licensed" | **Statuses**: how one is formed, what defects it can have, when it ends, whether another jurisdiction recognizes it | One hard-coded status path |
| "Spouse", "issue, per stirpes", "dependent" | **Relationships between persons**, and computations over them | None |
| "Debits equal credits"; fiscal periods | **Ledgers** with invariants the compiler checks | None |
| A standard adopted by reference | **Sources whose text cannot be stored**, pinned by hash and citation | None |
| How courts have answered an open question | **Ruling records and estimates**, kept apart from the legal answer (section 4.4) | None |
| "Why?" | **Cited explanation**: every step of a result names the provision it rests on | Free-text trace details |

The semantic core that supports these is small (**Call**):

1. A typed, total expression language over the value kinds above,
   including recursion that provably ends over finite structures such as
   a family tree or an ownership chain.
2. Definitions and rules evaluated as prioritized defaults. With no
   applicable rule the result is open, not false. With two applicable
   rules and no stated priority the result is a reported conflict, not a
   silent pick. This is the approach the Catala project demonstrated on
   French benefits law and on section 121 of the Internal Revenue Code,
   extended with Fidryn's outcomes.
3. Open questions as effects answered by handlers, as today. Evaluation
   under unanswered questions yields a value only when every admissible
   answer gives the same value.
4. Every node of the program carries the provisions it rests on, so every
   node of a derivation does too.
5. No legal vocabulary is built in. The core knows values, definitions,
   rules, positions, acts, open questions, time and provenance. Persons,
   entities, marriages, trusts, share classes and ledgers are declared in
   a standard library written in Fidryn, which anyone can extend.

### 5.1 One provision, before and after

29 U.S.C. § 2611(2) defines "eligible employee" for the Family and
Medical Leave Act. The official text has a general test (A), two
exclusions (B), a borrowed standard (C), a special rule for airline crews
(D) and a carve-out for one agency (E).

Today's fixture encodes it as:

```
query automatic eligibility_counts(hours: Int, months: Int, headcount: Int) -> Bool {
    return hours >= hours_required()
        and months >= months_required()
        and headcount >= employer_size()
}
```

A target encoding looks more like this. The syntax is illustrative only;
Stage 2 settles it.

```
rules for /us/usc/t29/s2611/2 at release "119-142" {

    term eligible_employee(e: Person, r: Employer, on: Date)
        scope /us/usc/t29/ch28/schI            // "As used in this subchapter"
    {
        general from (A) #5e1c.. {
            holds when
                  months_employed(e, r, until: on) >= 12                      // (A)(i)
              and hours_of_service(e, r, during: 12 months before on) >= 1250 // (A)(ii)
        }

        excluded from (B)(i) #91aa.. when
            covered_by(e, /us/usc/t5/ptIII/subptE/ch63/schV)

        excluded from (B)(ii) #c07d.. when
            count(employees_of(r) within 75 miles of worksite_of(e)) < 50

        // (C) borrows another provision's standard instead of stating one.
        hours_of_service follows /us/usc/t29/s207 from (C) #44b2..

        // (D) and (E) are not formalized yet. A query that reaches them
        // suspends and quotes them.
        pending (D), (E)
    }
}
```

Each `#…` is the hash of the exact official text the line encodes. If
Congress amends subparagraph (B)(ii), only that line goes stale.

### 5.2 Breadth: the areas the owner named

The Code profile in section 3.1 is public law. The owner's list reaches
into private law and private documents. What each area leans on:

| Area | What it needs from the language |
| --- | --- |
| Accounting | Ledgers with balancing invariants; fiscal periods; schedules and tables; privately written standards pinned without their text; "material" as an open question with a named decider |
| Corporations and other entities | Governing documents as sources of rules; statutory defaults those documents may vary and mandatory rules they may not; boards and share classes with quorum and voting; authority to act for an entity; a standard of judicial review that depends on context |
| Marriage and family | A status with formation requirements, defects and recognition across states; relationships between persons; property characterized by when and how it was acquired; guideline computations a judge may depart from on stated findings; multi-factor discretion |
| Trusts and estates | Instruments; offices and succession; discretionary powers bounded by a standard; interests that vest on conditions; shares computed over a family tree; principal and income accounting |
| Small business | One set of facts asked against many regimes at once ("which duties attach to this employer?"); thresholds by headcount and revenue over time; licenses from several levels of government; tables keyed by industry code |

Every entry maps to a concept in the table above. None needs a concept
the core lacks, which is the point of condition 6 in section 4.2. Two
consequences follow for the design:

- The Stage 2 design checks each of these areas on paper, construct by
  construct, before any syntax is fixed. Areas the owner did not name
  (criminal law, procedure, property, secured lending, intellectual
  property, immigration, tax) get the same check.
- Section 10 adds five probes, one from each named area, chosen because
  each exercises a construct that the Code-based items do not. They are
  conformance items, not a corpus of examples.

## 6. Approaches considered

### 6.1 How to reach "all of it"

| Approach | Trade-off | Verdict |
| --- | --- | --- |
| A. Import official text for everything; make the language express real provisions; add executable meaning by domain with visible review status | Slower to show executable breadth. Every claim is checkable. | **Chosen (Owner):** "This seems solid," then the monorepo framing. |
| B. Have a model draft encodings for every provision up front | Fast breadth. Fills the repository with law nobody can vouch for, which is the failure the language exists to prevent. Useful later as labeled drafts on top of A. | Rejected as the lead |
| C. Encode a few practice areas by hand, deeply | Credible but never reaches "all". | Kept as the ordering rule for depth inside A |

### 6.2 How to change the implementation (Call)

| Approach | Trade-off | Verdict |
| --- | --- | --- |
| Extend the current parser, representation and evaluator feature by feature | Every existing test keeps passing. But bodies are unparsed text, the representation is keyed on strings, and evaluation dispatches on example names; each new feature would add special cases. | Rejected |
| Build a new front end and semantic core inside this repository, selected per file by an explicit edition marker; port the existing examples; then delete the old path | Two paths coexist for a while. Keeps the infrastructure, the tests as a regression oracle, and the history. | **Chosen** |
| Start a new repository | No legacy constraints. Loses working infrastructure and governance for no gain over the second option. | Rejected |

### 6.3 How official text and encodings live together (Call)

| Approach | Trade-off | Verdict |
| --- | --- | --- |
| Generated text modules and curated rule modules in separate files, joined by address and hash | Regeneration never touches curated work. Staleness is exact. Reviewers see both in one change. | **Chosen** |
| One file per section with text and rules interleaved | Best to read. Every official release would rewrite files people edit. | Rejected |
| Keep text only as the publisher's XML and reference it from rule modules | Smallest. Text changes are unreadable in review. | Rejected |

### 6.4 How to ground "likely output" in actual rulings (Call)

| Approach | Trade-off | Verdict |
| --- | --- | --- |
| Attach ruling statistics to each named open question and let the encoded structure carry them to the result | Needs rulings labeled against specific questions. Every estimate is explainable, comparable across changes in the law, and separate from the legal answer. | **Chosen** |
| Train a model to predict a case's outcome from its facts or its text | Needs no encoding. Cannot say which legal question drove the prediction, cannot separate what the law requires from what courts tend to do, and inherits every bias in its training data invisibly. | Rejected |
| Fold likelihoods into the legal answer, so a probable result counts as the result | Simplest to present. Breaks the governing rule: a likely answer is not a determined one. | Rejected |

## 7. Stages

Each stage leaves the repository releasable. Version numbers are
suggestions.

| Stage | Outcome | Release |
| --- | --- | --- |
| 0. Presentable and truthful | Documentation matches behavior; every example, code sample and shown command is checked in CI; the examples that fail on typing alone are repaired; the front-end defects found on the way are fixed; the license is MIT | 0.2.0 |
| 1. The Code as a checked program | New front end and module system; provisions, citations and definitions as declarations; the whole United States Code imported and checked at D1, with definitions at D2; provision-level change sets between releases | 0.3 |
| 2. The semantic core on real provisions | Definitions, defeasible rules, legal time, quantities, typed facts, named and addressed open questions, refinement, evaluation as of a date, cited explanations; the start of a standard library that holds all legal vocabulary; first half of the conformance set at D3 with tests | 0.4 |
| 3. Positions, acts, instruments and organizations | Duties, powers, entitlements and processes on the same core; private instruments and the statutory defaults they may displace; bodies, voting and statuses; existing examples ported; the old path deleted; rest of the conformance set | 0.5 |
| 4. Authority and interpretation | Regulations imported and attached as refinements; source hierarchy and preemption; precedential rulings as changes to the law, scoped by court | 0.6 |
| 5. Rulings as data | Ruling records, the empirical handler and estimates kept apart from legal answers; a pilot on federal sentencing that reproduces the Sentencing Commission's published figures | 0.7 |
| 6. Scale-out | State ingesters; machine drafting of labeled encodings; attorney review workflow; an update job that commits each official release and each new ruling record | 0.8 – 0.9 |
| 7. A stable language | Frozen specification, editor support, documentation for lawyers, public coverage figures | 1.0 |

Stage 2's design starts while Stage 1 is being built, because it needs
the owner's eye on syntax and Stage 1 does not.

## 8. Stage 0 — presentable and truthful

The first version of this section put the work on `dev` before PR #3 was
merged. On 2026-11-13 the owner decided that PR #3 would merge first as
0.2.0 and Stage 0 would follow as 0.2.1. On 2026-11-14 the owner reversed
that: PR #3 is held, Stage 0 lands on `dev` first, and the two are
released together as 0.2.0, so no release carries the proprietary license
text. The same day the owner decided that the two front-end defects
Stage 0 had found and documented are fixed before the release. Fixing
them turned up more of the same kind, and those were fixed too. So
Stage 0 does change what the front end reads and rejects, in the parser,
the lowering and the checker: where a statement ends, how a `verify`
block is read, which block belongs to the header of a declaration, what
becomes of text that cannot be read (it is an error where it was stepped
over), how deeply text may nest, and comments, which are no longer read
as code. `CHANGELOG.md` lists each change, and
`docs/implementation-status.md` says what is still not read. No rule of
evaluation is changed: in the evaluator and the verifier, behavior
changed only in two readers of text, one that has a limit on nesting now,
and one that no longer splits inside a string and that reads the word
`assert` before a formula when a tab or a line end follows it, as it did
when a space followed it. The owner's other decisions are marked below
and listed in section 13.

### 8.1 Scope

**Examples and samples**

- Add a check that runs `fidryn check` on every `.fr` file under
  `examples/`, `prelude/`, `packages/` and `tests/programs/`, compares
  the result with a committed status file, and fails CI on any drift.
- Of the 94 failing examples, 56 show a date-arithmetic error that cannot
  be repaired until Stage 2; 29 fail only because a judgment query
  declares a result type and returns nothing; 9 show other typing errors.
  All are hand-typed slices that Stage 1 and Stage 2 replace with
  encodings bound to official text. The first version of this section
  recommended labeling all of them. The owner decided to label them and
  also to repair by hand those that fail for other reasons. Planning
  found that 34 can be repaired by typing-only edits, with no change to
  any rule, threshold or citation. A thirty-fifth, the United States Code
  catalog module, was repaired the same way once the parser fix exposed
  an unbound name in one of its rules. Of the 38 candidates, three hide
  date arithmetic behind another error and one declares a money result it
  never states; those stay labeled with the rest, 60 in all.
- State the status of every example plainly in `README.md`,
  `examples/README.md`, `examples/states/README.md` and
  `docs/examples.md`, generated from the status file. Wherever a reader
  document names a module that does not pass, mark it as a draft.
- Run every command a reader document shows, in CI, or mark the block as
  a synopsis. At the baseline the examples guide shows 77 commands and 16
  of them work.
- Add a check that every complete module shown in `docs/*.md` passes
  `fidryn check`. Fix or remove the samples that do not, starting with the
  deadline example in `docs/language.md`.

**Status documents**

- Rewrite the capability matrix in `docs/implementation-status.md` to
  match its own definitions. Under "a structured form, not only a source
  slice", most declaration kinds are not parsed. Record the verified
  findings from section 2 as known limits, with file references.
- Correct `docs/language.md`: `alias` is ignored; unknown body fields are
  rejected only in `source` blocks; `power`, `legal_act`,
  `conflict_doctrine` and `scenario` bodies are not interpreted.
- A public roadmap is deferred. The owner decided not to publish the
  program's destination until Stage 1 lands.

**Stale and inconsistent text**

- The site footer says "Fidryn v0.1" on every page
  (`xtask/src/site/templates/base.html:42`). Take it from the workspace
  version.
- `grammar.ebnf` line 1, `.agents/AGENTS.md` and `.agents/ARCHITECTURE.md`
  say v0.1.
- Reported by the infrastructure map, to verify before fixing: stale
  comments in `crates/fidryn-trace/src/lib.rs`; the "three malformed"
  count in `conformance/README.md`; `inputs` and `review` documented after
  "See also" in `docs/cli.md`; a POSIX-only path in the landing-page
  specimen; the semantics version written in four places.

**Repository hygiene**

- `.cargo/config.toml` forces macOS toolchain paths on every platform.
  Cargo cannot limit its `[env]` table to one platform. The owner decided
  to leave the file as it is.
- Separate reader documentation from internal ledgers in `docs/`. Move
  `WORKSTREAM-CONTRACT.md`, `REVIEW-FIX-CONTRACT.md`,
  `INTEGRATION-CONTRACT.md`, `INTEGRATION-SUITE.md`,
  `FULL-IMPLEMENTATION.md` and `OBLIGATIONS.md` under one clearly
  labeled directory and fix every link.
- Rewrite the top of `README.md` for an outside reader: what Fidryn is,
  what works today, a five-minute run that works as written, where it is
  going.
- Add a `SECURITY.md`.

**License**

- Return the project to the MIT License, as the owner decided. `main`
  still carries the original MIT text; the proprietary text exists only
  on `dev`.

### 8.2 Acceptance

1. `cargo xtask ci --fmt` passes, including the two new checks.
2. No document in `README.md`, `docs/` or `examples/` makes a claim the
   new checks contradict.
3. `cargo xtask site --check` passes and no generated page shows a
   version other than the workspace version.
4. A reader following `README.md` from a clean clone reaches a working
   `check` and `run` without editing anything.

## 9. Stage 1 — the Code as a checked program

### 9.1 Goals

1. **A new front end**, selected per file by an explicit edition marker.
   Every declaration it accepts is parsed into typed syntax with a closed
   set of fields; an unknown field is an error. Names resolve to symbols.
   Checker diagnostics carry source spans. In Stage 1 it accepts only the
   module header, imports, containers, provisions, citations, definitions
   and bindings. Rules, queries and every existing construct stay on the
   current path until Stages 2 and 3.
2. **A module system for a monorepo.** A workspace has a root manifest.
   A module's name is its address. Any module can import any other by
   name; imported names are qualified unless brought into scope. Checking
   a module needs only the interfaces of the modules it references. A
   whole workspace can be checked in one invocation, in parallel, with an
   on-disk cache keyed by content. A workspace can depend on a pinned
   release of another workspace, which is how a private matter builds on
   the public law repository without being part of it.
3. **Provisions as declarations.** A provision has an address, a number,
   a heading, text parts (introductory text, content, continuation text),
   a status, structured tables, child provisions and a hash. The hash of
   a provision covers its own text and its children's hashes, so a change
   anywhere changes every ancestor's hash and nothing else.
4. **Citations as values.** A reference from one provision to another is
   a typed edge with the linked words. References resolve across the
   workspace. A reference that cannot resolve is recorded with a reason
   (target repealed, renumbered, a range, outside the imported corpora,
   unknown) and never fails the build by itself.
5. **Definitions as declarations.** A defining provision yields a
   declaration with its term or terms, its kind (means, includes, does
   not include, borrowed), its reach, and for a borrowed definition the
   provision it borrows from. The body stays opaque text at this stage.
   Only unambiguous patterns are extracted; the rest stay plain
   provisions.
6. **An ingester for the United States Code.** One command turns an
   official release into a workspace: one generated text module per
   section, container modules for titles, chapters and subchapters, a
   lock file with artifact hashes, and a coverage ledger. Output is
   byte-identical across runs.
7. **Bindings.** A curated module can declare which provision it encodes
   and the hash of the text it was written against. Stage 1 implements the
   binding and its staleness check with a placeholder body; Stage 2 gives
   bound declarations real content.
8. **Change sets.** One command compares two ingested releases and
   reports provisions added, removed, amended and moved, and which bound
   encodings went stale.

### 9.2 Non-goals

- No executable semantics for provision content. That is Stage 2.
- No regulations, states or case law. The address scheme must not
  preclude them.
- No machine-drafted content.
- No change to how existing files compile or run. Files without the
  edition marker use the current path untouched.

### 9.3 Decisions

| Topic | Decision |
| --- | --- |
| Address scheme (Call) | For the Code, the publisher's own identifier, verbatim: `/us/usc/t29/s2611/2/B/ii`. First segment is the jurisdiction, second the corpus. Other corpora get a mapping when their ingester is written. The scheme must leave room for territories, tribal nations, localities, private standard-setters and instruments local to a workspace; Stage 1 implements only the Code. |
| Unit of compilation (Call) | One section is one module. It is the unit lawyers cite and Congress amends. |
| Hash (Call) | BLAKE3, as elsewhere in the project. Text is normalized before hashing by a rule the specification states exactly (Unicode form and whitespace). |
| Where generated output lives | In the separate law repository (**Owner**). Until the owner decides its name, visibility and license it is a local git repository and is not pushed (**Call**). Official downloads are cached under `/library/corpora/uscode/<release>/` (**Call**). |
| New crates (Call) | Provision model and addresses in one crate; the ingester in another; workspace support in the driver. |
| Sample in this repository (Call) | Title 1 and a handful of sections chosen to cover tables, continuation text, court rules, notes containing quoted provisions, and an unresolved reference. |

### 9.4 Notes, credits and what a hash covers (Call)

A generated module holds all of the pinned official text for its section,
in three layers that are hashed separately:

| Layer | Contents | Effect of a change |
| --- | --- | --- |
| Operative text | The provision tree of goal 3 | Any binding to a changed provision goes stale |
| Enacted notes | Statutory notes and executive documents set out under the section; each note hashed individually and the group as a whole | Every binding within that section goes stale |
| Editorial matter | Source credits, editorial notes, revision notes, descriptions of removed sections | Reported in change sets; never makes a binding stale |

The second row is deliberately conservative. A later law can change what
a section means by adding a note beneath it, for example by delaying an
effective date, without touching the section's words. Bindings also
record the release they were written against, so the rule can be narrowed
later without rewriting history.

A note that the publisher's grouping does not classify is treated as
enacted.

The ingester gives each enacted note an address beneath its section. The
address must not depend on the note's position alone, because notes are
inserted over time; it is derived from the enacting law the note cites
where there is one, and from the note's content otherwise. Provisions
quoted inside a note are kept as structured text within the note and get
no address of their own in Stage 1.

Every link in every layer becomes a typed reference. A link into a body
of law that has not been imported, such as the Statutes at Large, is
recorded with that reason.

### 9.5 Definitions: what is extracted and the bar for it (Call)

Extraction is by a closed list of textual patterns written into the
specification. A provision that matches none of them stays a plain
provision. For each definition the ingester records the term or terms
verbatim, the kind, and the reach.

Reach is three fields:

| Field | Content |
| --- | --- |
| Words | The reach phrase exactly as the text has it, or "unstated". No default is assumed. |
| Region | The part of the Code the words name, when they name one (a section, a subdivision of it, a part, chapter, subchapter or title). |
| Basis | `enacted` when the title is positive law. `restated` when it is not: there the editors rewrote the enacted words (for example "this Act" as "this chapter"), and the true reach is the enacted unit, which an editorial "References in Text" note may describe. The ingester links that note when it mentions the phrase. `unresolved` when the words name no region of the Code. |

The bar is about text, not law, so it does not need an attorney:

1. A random sample of 500 extracted definitions is compared by hand with
   the official text. Acceptance requires no wrong term, kind or reach
   words in the sample.
2. A second random sample of 500 provisions that look definitional under
   a looser pattern, but were not extracted, is read to estimate how many
   definitions the closed patterns miss. That estimate is published with
   the coverage figures. It is not a gate.

Only extracted definitions count as D2 in Stage 1. The ledger reports
them by basis, and reports the estimated share missed.

### 9.6 Acceptance

Measured on release point 119-142 unless stated.

1. The ingester produces a workspace that `fidryn check` accepts in one
   invocation.
2. Counts for the Code's own provisions match the independent profile in
   section 3.1: 60,490 sections with the same status breakdown; 772,863
   addressable provisions; 90,719 references to the Code in operative
   text, of which 88,935 resolve exactly, 820 reach the section only and
   964 reach no section. Any difference is explained in writing. Totals
   for the note layers are reported beside them.
3. Every unresolved reference in every layer has a recorded reason. None
   is unknown without a listed example.
4. For every provision and every note, the text rendered from the
   generated module equals the normalized text taken from the official
   XML, and every module verifies against the pinned artifact.
5. The change-set command, run on two real consecutive releases, reports
   exactly the provisions and notes whose normalized text or structure
   differs, separately for operative text, enacted notes and editorial
   matter, confirmed by an independent comparison of the XML.
6. Staleness follows section 9.4. A curated module bound to a provision
   that changes between those two releases is reported stale. One bound
   within a section whose enacted notes change is reported stale. One
   bound within a section where nothing or only editorial matter changes
   is not.
7. The definition samples of section 9.5 are drawn, read and recorded,
   and the first has no errors.
8. All 27 release points the publisher offers are replayed as commits in
   the local law repository, each with its change set as the commit
   message.
9. Checking the full workspace, notes included, takes at most 10 minutes
   from cold on 16 threads and at most 30 seconds when nothing changed,
   within 16 GB of memory. These are initial targets to confirm with a
   first spike.
10. The existing 1,188 tests and the Stage 0 checks still pass.
11. The language reference documents workspaces, provisions, citations,
    definitions and bindings, and the site shows the coverage ledger.
12. A second workspace that declares a dependency on the generated one at
    a pinned release can cite its provisions and checks cleanly. Pointing
    it at a different release produces a diagnostic, not a silent pass.
13. The integration test of section 4.3 runs. While the law repository
    is local, it runs on this machine against the full workspace and in
    CI against the in-repository sample. The cross-repository CI job is
    written and switched on when the law repository is published.

### 9.7 Left to the builder

- Concrete syntax for workspaces, provisions, citations, definitions and
  bindings, within these requirements: generated text modules are
  line-oriented, put one provision in one block, format deterministically
  and keep references inline.
- The edition marker's spelling.
- The interface summary format and the cache layout.
- How tables are represented, provided no cell is lost.
- The exact normalization rule for hashing, provided it is stated in the
  specification and tested.
- The form of a note's address, within the rule in section 9.4.
- The closed list of definition patterns, within the bar in section 9.5.
- Which sections make up the in-repository sample.

## 10. Later stages in outline

**Stage 2** takes the conformance set below to D3 with tests. Candidates,
all present in the pinned release unless marked:

| Provision | What it stresses |
| --- | --- |
| 29 U.S.C. § 2611 | Definitions, exclusions, borrowed meanings, reach |
| 29 U.S.C. § 2612(a)–(f) | An entitlement with a budget over a window, expiry, notice "as is practicable" |
| 29 C.F.R. § 825.110 (eCFR) | A regulation refining a statutory term |
| Fed. R. Civ. P. 6(a), (d) | Counting time: trigger day, roll-forward, hours, backward counts, holidays, time zones |
| 28 U.S.C. § 1332(a), (c), (d) | Thresholds, citizenship definitions, fractions, discretionary factors |
| 28 U.S.C. § 1446(b)–(c) | A deadline with later-served parties and a bar with a bad-faith exception |
| 28 U.S.C. § 1658 | "The earlier of" two periods; accrual and discovery |
| 26 U.S.C. § 1 | Rate tables, filing status, inflation adjustment, time-limited overrides |
| 26 U.S.C. § 121 | The benchmark the Catala project used |
| 26 U.S.C. § 152 | Multi-test definitions with tie-breakers |
| 42 U.S.C. §§ 415, 416(l) | Indexed parameter tables; a table keyed by birth year |
| 29 U.S.C. § 206(a)(1) | A value that steps over dates counted from enactment |
| 29 U.S.C. § 218(a) with one state statute | A federal floor that state law may exceed |
| 29 U.S.C. § 1144(a)–(b) | Preemption, a saving clause, and an exception to it |
| 11 U.S.C. § 362 | A general rule with a long list of exceptions and relief "for cause" |
| 11 U.S.C. § 707(b)(2) with § 104 | A formula whose dollar amounts adjust every three years |
| 18 U.S.C. § 3161 | A clock with excluded intervals and required findings |
| 18 U.S.C. § 1001 | Offense elements, mental state, graded penalties |
| 5 U.S.C. § 552(a)(6), (b) | Working-day deadlines, extensions, exemptions |
| 5 U.S.C. §§ 3345–3346; 3 U.S.C. § 19 | Who may hold an office, in what order, for how long |
| U.S. Const. art. II § 1 cl. 5; amends. XXII, XXV § 4 (pinned separately) | Qualifications, term arithmetic, a deadline-driven procedure |
| 8 U.S.C. § 1427 | Residence and presence windows; rebuttable presumptions |
| 42 U.S.C. § 2000e-5(e)(1) | A period that depends on the existence of state law |
| 15 U.S.C. § 1692g | A duty with a trigger, a window and a suspension |
| Fed. R. Evid. 609, 801–807 | Rule, exclusions, exceptions and balancing tests |
| The trust, prenuptial and company-formation examples | Private instruments with no names known to the evaluator |
| 26 U.S.C. § 168, with a ledger written as a library module | Accounting: depreciation schedules and conventions; a balancing invariant the compiler checks |
| 8 Del. C. § 141(b) with a sample set of bylaws (pinned separately) | Corporate: a statutory default an instrument may vary within limits; quorum and vote counting |
| One state's marriage statute and its child-support guideline (pinned separately) | Family: a status with formation requirements; a schedule-driven amount a judge may depart from on findings |
| Uniform Trust Code § 105 as enacted in one state, with the existing trust and a per stirpes distribution (pinned separately) | Trusts: default and mandatory rules against an instrument; a share computed over a family tree |
| 42 U.S.C. § 2000e(b), 29 U.S.C. § 630(b) and 29 U.S.C. § 2101(a), with § 2611(4) | Small business: one set of facts asked against several regimes at once |
| One uniform-act family across states (Stage 6 text) | One parameterized encoding with per-state values |

Stage 2 also fixes one thing that Stage 5 depends on: every open question
is a declaration with a name, an address and a typed answer (yes or no,
one of a list, an amount, findings on listed factors). Nothing in Stage 2
computes statistics.

**Stage 3** rebuilds duties, powers, entitlements, acts and procedures on
the core, ports the remaining examples and deletes the old path.

**Stage 4** imports regulations with their dated versions, attaches them
to statutes as refinements, and adds source hierarchy, preemption and
court-scoped interpretation. Precedential rulings enter here as changes
to the law.

**Stage 5** builds the empirical layer of section 4.4. The pilot is
federal sentencing (**Call**), for four reasons. The guideline range is a
large, purely mechanical computation. The judge's decision relative to
that range is a clean open question. The Sentencing Commission publishes
one coded record per case for fiscal years 2002 to 2025. And the
Commission's own tool for judges publishes figures the pilot can be
checked against: the acceptance test is to reproduce those figures from
the raw records for sampled combinations of guideline, offense level and
criminal history. The pilot requires encoding the sentencing guidelines,
which is a substantial corpus of its own. The Commission removes
identifiers from its records, so the pilot has no judge field;
estimates by judge first become possible with records built from
dockets and opinions. The second target is chosen from areas with
official coded outcomes before any area that needs rulings labeled from
prose.

**Stage 6** begins with a survey of all fifty state publishers (format,
bulk access, official status, terms) before any state ingester is
written. Machine drafting is introduced here, labeled, on top of the
review workflow. Labeling rulings from prose, by people or by models,
also belongs here, with the label's origin always shown.

**Stage 7** freezes the specification and ships editor support and
lawyer-facing documentation.

## 11. Goals, non-goals, anti-goals

**Goals.** A language that meets the bar in 4.2, in every area of United
States law, public or private. A monorepo in which every provision of
federal and state positive law is present at a declared depth. Results that cite their basis and show their assurance. A
repository an expert can audit without finding a false claim.

**Non-goals.** Replacing lawyers or courts. Giving legal advice. Encoding
case law as if it were rules. Predicting a whole case from its facts
with an opaque model; estimates attach to named open questions and show
their evidence (section 4.4).

**Anti-goals.** These would make the project a failure even if everything
ran:

- A determinate answer that rests on an unreviewed or stale encoding and
  does not say so.
- An estimate of how courts tend to rule presented as what the law
  requires, or allowed to turn a suspended result into a determinate one.
- An estimate shown without its sample: which courts, which period, how
  many rulings, how they were labeled.
- Law text in the repository that did not come from a pinned official
  artifact.
- Coverage numbers that count a provision as encoded when its operative
  words are not all represented.
- Special cases in the compiler or evaluator for a particular statute,
  example or name.
- Legal vocabulary built into the compiler or evaluator, so that a new
  area of law needs a compiler change.
- A client's private material in the public law repository.

## 12. Constraints

- **Branches.** `dev` is the head of PR #3. Stage 0 is built on a branch
  off `dev` and reaches `dev` before PR #3 is merged (owner decision of
  2026-11-14). Later stages use feature branches off `dev`.
- **Promotion.** `main` accepts only a pull request from `dev` with all
  required checks green. Each promotion raises the workspace version and
  adds a changelog entry.
- **Commits.** GPG-signed, focused, no mention of tooling authorship.
- **Gate.** `cargo xtask ci --fmt`, run through `lease` on this machine.
- **Toolchain.** Rust 1.98.1, edition 2024. Python tooling through `uv`.
- **Documentation.** `docs/*.md` is canonical; the site is generated from
  it. Agent material stays under `.agents/`. Where the two disagree, the
  human documents win.
- **Storage on this machine.** Reference data under `/library`, working
  outputs under `/work`, rebuildable caches under `/cache`.
- **Evidence.** A capability counts as implemented only when parsing,
  resolution, checking, evaluation, provenance and tests agree. Green
  tests are regressions, not proof of the general claim.
- **Survey findings.** The frontend, runtime and infrastructure maps
  gathered for this design were quick surveys, not line-by-line
  verification. Items in section 2 were re-verified in the source.
  Anything else from those maps must be re-checked before it is relied
  on.

## 13. Decisions that are the owner's

1. **License.** Decided on 2026-11-13: the MIT License. `LICENSE` on
   `dev` forbade any use by anyone; Stage 0 restores the MIT text the
   project began with. Decided on 2026-11-14: contributions are accepted
   under the same license, with no separate agreement.
2. **The law repository's name, visibility and license**, before anything
   is pushed.
3. **When to merge PR #3.** Decided on 2026-11-14, replacing the decision
   of 2026-11-13: PR #3 is held until Stage 0 is on `dev`, and both are
   released as 0.2.0.
4. **Whether to repair the 94 legacy examples** instead of labeling them.
   Decided on 2026-11-13: label them, and repair those that fail for
   reasons other than date arithmetic (section 8.1).
5. **Budget for machine drafting** in Stage 6.
6. **Attorney review.** Who reviews, and under what arrangement, before
   any encoding is presented as reviewed.
7. **Statistics about named judges.** Decided on 2026-11-13: the judge's
   identity matters and is recorded whenever the source provides it, as
   an optional field (section 4.4).

## 14. What the plan treats as settled on approval

None of the decisions in section 13 blocks Stage 0 or Stage 1. Approving
this document settles the following Calls for planning. Overruling any of
them restructures the plan, so each is listed here to be checked:

1. Stage 0 fixes the front-end defects it found (section 8) and changes no
   rule of evaluation. It is released as 0.2.0 together with PR #3, and it
   publishes no roadmap (owner decisions of 2026-11-13 and 2026-11-14,
   replacing earlier versions of this item).
2. The failing examples are labeled, and the 35 that fail on typing
   alone are repaired (owner decision of 2026-11-13, replacing the first
   version of this item).
3. The implementation changes by adding a new front end and semantic
   core beside the old path, selected per file, and deleting the old path
   once the examples are ported (section 6.2).
4. Official text and curated encodings live in separate files joined by
   address and hash (section 6.3).
5. One section is one module, addressed by the publisher's identifier
   (section 9.3).
6. The law repository, which the owner decided is separate, stays local
   until the owner decides its name, visibility and license. The
   integration test works as section 4.3 describes (sections 4.3, 9.3).
7. Enacted notes are inside staleness and editorial matter is outside it
   (section 9.4).
8. Definitions are extracted by closed patterns with a text-level audit,
   and reach is recorded as written with its basis (section 9.5).
9. The core has no built-in legal vocabulary. Every area of law,
   public or private, is added as library modules (sections 4.2, 5).
10. Private work lives in its own workspace, which depends on a pinned
    release of the law repository (section 4.3).
11. A result's legal answer and its empirical estimate are never merged,
    and an estimate always shows its sample (section 4.4).
12. The empirical layer is Stage 5, after authority and interpretation,
    and pilots on federal sentencing. Before then the only commitment is
    that Stage 2 makes open questions named, addressed and typed
    (sections 4.4, 10).
