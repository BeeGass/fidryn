# Stage 1: The Code as a Checked Program (0.3) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Import the whole United States Code from the publisher's official XML into a workspace of generated Fidryn modules that `fidryn check` accepts in one invocation: every provision addressed, hashed and tied to a pinned official file, every reference resolved or recorded with a reason, definitions extracted by closed patterns, curated modules bound to provisions by hash, and provision-level change sets between releases.

**Architecture:** A corpus-neutral crate, `fidryn-provision`, holds the provision model: addresses, normalized text, BLAKE3 hashes in three layers, notes, references, definitions, bindings, interfaces and change sets. A second crate, `fidryn-ingest-uscode`, reads the publisher's archives into that model and writes modules deterministically; it is the only code that knows the publisher's XML. A marker on the first line of a file (`edition 2`) routes it to a new parser and checker that read only module headers, imports, containers, provisions, notes, citations, definitions and bindings; files without the marker take the present path unchanged. The driver gains workspaces: a root manifest, a lock, module discovery, a parallel check that reads only interface summaries across modules, a content-keyed disk cache, and dependencies on other workspaces pinned by lock digest.

**Tech Stack:** Rust 1.98.1 (edition 2024); the existing lexer of `fidryn-syntax`; `blake3`, `serde`, `serde_json` (already workspace dependencies); new dependencies `unicode-normalization`, `zip` and `roxmltree`; `std::thread::scope` for parallel work; Python 3.12 through `uv` with `defusedxml` and `blake3` for the independent cross-checks in `conformance/`.

**Spec:** `.agents/docs/specs/2026-11-13-law-monorepo-design.md`, section 9 (goals 9.1, non-goals 9.2, decisions 9.3, layers 9.4, definitions 9.5, acceptance 9.6, left to the builder 9.7). Sections 3.1, 3.3, 4.1 to 4.3, 11, 12, 13 and 14 give the facts, depths, repository split, anti-goals, constraints and settled decisions this plan relies on.

## Where this stands

Stage 0 is finished and merged into `dev` locally. Stage 1 starts from that commit on a new branch, `stage1/code-as-checked-program`. Nothing in this plan pushes, publishes or changes a setting on GitHub.

The plan was written after measuring the full release 119-142 and comparing it with 119-138 (section "Measured facts"). Every number in it that describes the Code comes from that measurement, and each says so.

## Decisions that are the owner's

Each has a default. Work that does not depend on a decision proceeds; the task that depends on one names it.

| # | Decision | Default the plan uses | Blocks |
| --- | --- | --- | --- |
| O-1 | A sentence in `LICENSING.md` for the committed sample of the Code. The sample is text published by the Office of the Law Revision Counsel; under 17 U.S.C. § 105(a) "Copyright protection under this title is not available for any work of the United States Government", so the MIT License cannot be said to cover it. | Add the section given in Task 7. | Task 7 |
| O-2 | Whether the public site shows coverage figures for the full Code while the law repository is local and unnamed. | The site shows the ledger of the in-repository sample only, with one sentence that says so. | Task 16 |
| O-3 | Who reads the two samples of 500 for the definition audit (design 9.5: "compared by hand with the official text"). The bar is textual and needs no attorney, but it needs a person. | None. The samples are drawn and prepared for reading; acceptance item 7 stays open until someone the owner names has read them. | Task 22, acceptance item 7 only |
| O-4 | A basis value for definitions in appendix content (court rules and the like), where neither `enacted` nor `restated` describes the text. A new value changes a decision of the design (9.5). | Follow the design to the letter: basis comes from the file's own positive-law flag; the ledger reports appendix definitions in a row of their own. | Nothing |
| O-5 | Whether the commits of the local law repository are signed. | Signed, with the same key as this repository. | Task 22 |
| O-6 | The law repository's name, visibility and license (design 13). | It stays local at `/work/projects/law-local/` and is not pushed. | Nothing in Stage 1 |
| O-7 | Addresses where the publisher's identifier cannot serve. The design decides that an address is "the publisher's own identifier, verbatim". In release 119-142, 416 identifiers are held by more than one element (two different sections are both `/us/usc/t28/s1932`), 171 elements have none, and 2,430 repealed or omitted sections carry one identifier that names several sections (F2, F3). | The rule of S7: the first holder keeps the identifier and a later one gets it with `~2`; an element without one gets a segment that begins with `~`; a list's first member is the address and the others are aliases. Every other address is the identifier, unchanged. | Task 8 |

## Choices this plan makes that the owner may overrule

The design leaves these to the builder (9.7) or does not settle them. Each is stated in full in the Specification.

1. The marker is `edition 2` as the first two words of a file. A workspace holds edition-2 modules only.
2. A generated module carries no release label, so a section that did not change is the same file, byte for byte, in the next release. The release is recorded once, in the workspace's manifest and lock.
3. The unit of compilation is the section and, where the Code has none, the elements listed in S7 (court rules and the like). Sections are filed by address, flat under their title.
4. A module's file path is derived from its address by an escaping rule that is safe on file systems that ignore case (S2). The address written inside the file is the name; the path is never the source of truth.
5. References need no import. `import` exists for names, and the only names in Stage 1 are citations and bindings declared in curated modules.
6. A reference that does not resolve is counted and listed with its reason in the report and the ledger. It is not a diagnostic.
7. A note's address is built from its group, the law it cites and its heading; the hash of its content names it only when it has neither a law nor a heading (S8).
8. Tables of contents are not imported. Their entries repeat the headings of the units they list. The ledger counts them as omitted, with the footnotes they hold.
9. The address is the citation of record. No display citation such as "1 U.S.C. § 8(b)" is derived in Stage 1.
10. The patterns that extract definitions live in the ingester, not in the compiler, because they are patterns of English statutory drafting.
11. The sample is three whole titles (1, 11 Appendix, 45): about 0.9 MB of archives and about 5 MB of generated modules in this repository. The design asks for "Title 1 and a handful of sections"; whole titles are what lets CI prove that every committed line of law came from a pinned archive (F11).
12. Notes the publisher puts in no group are enacted, as the design says, except descriptions of removed sections and revision notes, which the design itself names as editorial. That leaves 2,715 notes of the court rules and 1,205 notes of sections in the enacted layer by default rather than by classification; the ledger shows the group of every note (F8, S8).
13. A reference whose target has the form of a range is recorded as `range-form`. The publisher writes a range and a dashed section number with the same character, and the plan does not guess which is meant (F12, S9).
14. The estimate of missed definitions (design 9.5) is kept in the audit record beside the ledger of the law repository; `coverage.json` itself holds only what is computed from the workspace.
15. A footnote marker that the publisher marks as one is editorial matter. A bare superscript digit stays operative text even where it is probably a second marker of a footnote, because the markup does not tell it from the numerator of a fraction (F7a). The price: a binding goes stale when the publisher adds such a marker, which is all that changed in two of the nine sections of F10.
16. Statutory notes that the publisher sets out under a chapter or another container, about 1,200 of them, are reported in change sets and make no binding stale. The design's rule speaks of notes "set out under the section" (9.4); widening it to the containers above a section would make every binding in a chapter stale when one chapter note changes, and is left for the owner to ask for.
17. Where the publisher writes several repealed sections as one element (`s44...46`), a reference to a member with a whole number between the ends reaches that element; a member whose number carries a letter is not named by the identifier, and a reference to it is recorded as reaching no unit (S7, S9).

## How the work is run

- **Execution.** Subagent-driven, as in Stage 0: one implementer and one reviewer per task, and a controller that keeps a ledger of its rulings.
- **Lanes.** After Task 1, three lanes run at the same time in separate git worktrees, each on its own branch cut from the stage branch at a named commit: the Python cross-checks (Task 2), the model (Tasks 3, 4 and 4b), and the front end (Task 5). They touch disjoint files. The controller merges a lane into the stage branch when its last task is reviewed. Later groups that can run side by side are marked in the task list. Cargo jobs go through `lease` one at a time per lane; the scheduler serializes them.
- **Review bar.** A reviewer blocks a task for: a wrong result, lost or altered text, nondeterministic output, a missed requirement of the task, a document sentence that is false of the code in a way a reader would act on, or a change to the old path. Everything else is recorded in the ledger and does not open a fix round. A task gets at most two fix rounds; what is still open after the second is ruled on by the controller and listed for the owner.
- **Documents.** A task changes a reader document only where it adds something the document must describe or makes a sentence false. The reference for this stage is one file, `docs/law-modules.md`, and each task adds the sections it implements. One documentation pass at the end (Task 23) brings the other documents up to date.
- **Evidence.** A capability counts as done when its tests pass, the gate passes, and, where the task names a run on real data, that run is recorded with its command line and output.

## Measured facts

Measured on 2026-11-16 on the publisher's archive of release 119-142 (`xml_uscAll@119-142.zip`, 108,706,565 bytes, 58 files, 697,880,605 bytes of XML), and on 119-138 for item F10. The scripts, their commands and the full lists are kept outside the repository; Task 2 moves the counting into `conformance/` so that every number here can be reproduced from the repository. "Operative" means outside notes, quoted content, tables of contents and source credits.

**F1. The design's profile reproduces.** The profile script of 2026-11-13, run unchanged on the fresh archive, gives the numbers of design 3.1: 60,490 sections (none 51,009, repealed 5,131, omitted 2,037, transferred 1,911, renumbered 368, vacant 17, reserved 16, unknown 1); 772,863 addressable provisions; 90,719 operative references to the Code, of which 88,935 resolve exactly, 820 reach the section only and 964 reach no section. Those numbers count only nine kinds of level and only elements that carry an identifier; this plan's own counts (F4, F12) differ for stated reasons, and Task 21 writes the difference down.

**F2. Identifiers are not unique and not always present.** Of 783,636 operative level elements, 783,465 carry an identifier and 171 do not. 416 identifier values are held by more than one element (883 elements), always within one file: 7 by two different sections (`/us/usc/t5/s3598`, `/us/usc/t5/s5757`, `/us/usc/t10/s130g`, `/us/usc/t25/s1`, `/us/usc/t28/s1932`, `/us/usc/t38/s1167`, `/us/usc/t40/s3318`), 8 by two court rules (`/us/usc/t28a/courtRules/Civil/rule1` to `rule8`), 1 by two chapters (`/us/usc/t26/stD/ch38`), and 400 by small levels inside one unit. The 171 without identifier are: 110 generic `level` containers; 28 reserved chapters; 20 sections and 5 titles inside the compiled acts of two appendices; 3 compiled acts; 1 set of court rules; and 4 other generic levels. One section stands inside another (25 U.S.C. 5329 holds a model agreement marked up as a section with the identifier of 25 U.S.C. 1). No two units or containers differ only in the case of a letter; 15 pairs of small levels inside one section do.

**F3. What identifiers are made of.** The characters are `/`, ASCII letters and digits, the full stop (5,857 occurrences), the en dash U+2013 (70,411) and the space (679). There is no ASCII hyphen anywhere: a section number such as 1395i–3 is written with an en dash. Segment forms beyond letters and digits: a dashed number (`s1395i–3`, `schIX–A`); a court-rule number with a full stop (`rule71.1`); a range of sections written with three full stops (`s44...46`, 1,752 sections); and a list of several identifiers separated by a space in one attribute (`/us/usc/t2/s3 /us/usc/t2/s4`, 678 sections). Every one of those 2,430 sections has a status other than in force. The identifiers of one file, the Title 11 Appendix, have no title segment (`/us/usc/rule1001`, `/us/usc/ptI`). A section's identifier is not below its chapter's (`/us/usc/t1/s1` in `/us/usc/t1/ch1`); a small level's is always below its parent's by one segment.

**F4. Units.** With the unit kinds `section`, `courtRule`, `reorganizationPlan`, and a generic `level` that holds a text part and has no unit above it, there are 61,172 units (60,510, 544, 107 and 11), and every one of the 2,174,736 operative text nodes lies in exactly one outermost unit, except the 19,274 that are the number or heading of a container. Containers hold no text of their own. Court rules and reorganization plans hold no sections.

**F5. Containers.** The kinds that occur: `title` (85), `subtitle` (98), `chapter` (2,936), `subchapter` (3,881), `part` (1,892), `subpart` (544), `division` (41), `subdivision` (9), `article` (11), `courtRules` (5), `compiledAct` (5), `reorganizationPlans` (1), generic `level` (110), and the `appendix` element at the top of each of the five appendix files. At most six containers stand above a unit.

**F6. Sizes.** The largest unit is 42 U.S.C. 1395ww: 1,555,106 bytes of XML, 846,800 characters of text, 1,600 provisions, 167 notes. Three units exceed 1 MiB of XML and none 4 MiB. Small levels nest at most 8 deep. The largest file is Title 42: 113,748,800 bytes, 1,100,360 elements; a Python tree of it takes 1.7 seconds and about 530 MiB. The whole release scans in 13.5 seconds on four workers. From these: `E2_MAX_SOURCE_BYTES` is 64 MiB, `E2_MAX_TOKENS` is 16,777,216, and the ingester refuses an archive member larger than 512 MiB.

**F7. What text holds.** Text parts: `content` (554,577), `chapeau` (118,442), `continuation` (9,578, of which 905 stand between two children and 33 inside a `content`), `proviso` (40, 4 inside a `content`). Inside them: `p`, tables (`xhtml:table`, 350 in operative text, the largest 242 rows), links (`ref`), `date`, typography (`i`, `b`, `inline`, `xhtml:span`, `xhtml:b`, `xhtml:i`, `xhtml:a`, `br`), superscripts and subscripts (`sup` 2,778, `sub` 205, and their XHTML forms), footnotes (`note type="footnote"`, 9,324), and images (`img`, 22 operative, among them 8 in 15 U.S.C. 1278, and 33 in notes; each has only a `src` that names an `.eps` file, and no such file is in the archive). 493 text parts mix text of their own with block children, and 140 `p` elements hold a table. No level element holds text outside a text part. Quoted content occurs only in notes (38,258). Status values that occur: repealed, omitted, transferred, renumbered, vacant, reserved, unknown, on sections and on 12 other kinds of level.

**F7a. Footnote markers.** The 10,298 operative markers written as a link of class `footnoteRef` each point by `idref` at a footnote of their own unit or container, and all but two of the 9,701 operative footnotes have such a marker. Besides them, 2,913 superscripts in operative text are digits. 2,783 of those equal the number of a footnote of their unit, and are mostly second and later markers of a footnote whose first marker is a `footnoteRef`; but 35 of the 2,783 are numerators of fractions, and nothing in the markup tells a marker from a numerator, an exponent or the 3 of `C3H8`. The plan therefore does not guess: a bare superscript is a superscript, and part of the operative text.

**F8. Notes.** 54,825 `<notes>` elements, all of type `uscNote`. A note's group is the topic of the cross heading it follows: `editorialNotes` (41,869 headings), `statutoryNotes` (27,607), `execDoc` (3,814), `courtRules` (20). Under sections: 75,777 notes in the statutory group, 58,481 in the editorial group, 6,262 executive documents, 20 in the court-rules group, and 23,415 before any cross heading (14,081 with topic `removalDescription`, 8,129 `historicalAndRevision`, 1,205 others). Under court rules: 2,715, all before any cross heading. Under containers: 3,650. Notes outside any `<notes>` element: footnotes; 69 directly in sections and 12 in small levels (bracketed words such as "[Repealed]"); 27 `enacting` notes and 59 "Current through" notes directly under titles and appendices (the text is stale in one file, so it is no evidence of the release); 501 bracketed notes inside quoted content; a few under court-rule containers. By the address rule of S8, 8,970 of the 163,955 section notes share group, law and heading with another note of their section.

**F9. Releases.** The publisher lists 27 release points for 2026. From oldest to newest, in the publisher's order, which the names alone do not give: `119-73not60`, `119-73`, `119-83`, `119-84`, `119-87`, `119-88`, `119-90`, `119-93`, `119-94`, `119-95`, `119-99`, `119-100`, `119-102not101`, `119-102`, `119-103`, `119-108`, `119-110`, `119-111`, `119-117`, `119-118`, `119-120`, `119-121`, `119-124`, `119-134`, `119-136`, `119-138`, `119-142`. Their whole-Code archives total 2,930,114,329 bytes. This is the list design 9.6, item 8 means by "all 27 release points"; a release the publisher adds later is not part of the acceptance of this stage.

**F10. The pair 119-138 to 119-142.** No section added or removed. Operative text changed in 9 sections (4 amendments proper: 11 U.S.C. 109 and 1182, 26 U.S.C. 7803, 42 U.S.C. 1395m; the others a typographical correction, a comma, an editorial bracket, and two that gained only a footnote marker). Only enacted notes changed in 12 sections (for example 11 U.S.C. 101, 42 U.S.C. 1305). Only editorial matter changed in 14 (all in Title 26, for example 26 U.S.C. 26 and 457). The pair therefore holds the three cases of design 9.6, item 6. The 55 files of the titles the publisher does not list as changed are identical to the older release once the release name is masked; the three changed files were regenerated, and no element of theirs keeps its `@id`.

**F11. The sample.** Three whole titles, 1, 11 Appendix and 45, hold every feature design 9.3 asks of the sample (a table in operative text, continuation text, court rules, notes with quoted provisions, a reference that does not resolve) and also dashed section numbers, sections whose identifier names several, statuses, a proviso, footnotes, images, identifiers without a title segment, a container without an identifier, and one title that is positive law and two that are not. Their archives are 42,241, 375,834 and 524,586 bytes (942,661 in all) and 5,471,744 bytes of XML: 39, 230 and 294 units.

**F12. References.** 1,301,031 links have a target; the 10,750 that have none are all footnote markers. Targets name four bodies: `/us/pl` (599,269), `/us/usc` (326,173), `/us/stat` (321,831), `/us/act` (53,758). None is malformed. By layer: operative 106,918; notes of the enacted groups 404,716; notes of the editorial group 425,195; source credits 299,784; tables of contents 64,154. A range of sections is written `s251–260`, with the same en dash a dashed section number has, and 111 operative range targets are cut short by the end of the link. Of the 90,719 operative references to the Code, measured against every level identifier: 88,365 exact, 577 to a target with a status, 813 to a unit but not the provision, 291 of the range form, 673 to no unit.

**F13. File metadata.** Every file declares the namespace `http://xml.house.gov/schemas/uslm/1.0` and the schema file `USLM-1.0.15.xsd`, and names the release as `Online@119-142` in `docPublicationName`. The positive-law flag is "yes" in 27 files and "no" in 31, among them all five appendices. Member names are `usc` followed by two digits, an optional `a` or `A`, and `.xml`.

**F14. Definitions.** Of 682,600 operative text parts: 27,719 hold `the term(s) “X” mean(s)`, 2,444 `... include(s)`, 596 `... does/do not include`, 3,898 `has/have the (same) meaning(s) given`, 23 `the word(s) “X” include(s)`, and 5,707 hold a quoted term within 60 characters before one of those verbs without matching any of them. No term is written with straight quotation marks.

## Global Constraints

From the design (sections 9.2, 11 and 12) and the owner's standing decisions. Every task's requirements include this section.

- **The old path does not change** (9.2: "No change to how existing files compile or run. Files without the edition marker use the current path untouched."). Every test that passes at `43830cd` passes after every task (1,469 at that commit), and `cargo xtask examples` reports the same status for every module.
- **No executable semantics, no other corpora, no machine-drafted content** (9.2). Stage 1 imports the United States Code only. No rule, query or evaluation is added for provision content.
- **Law text comes only from a pinned official file** (11: "Law text in the repository that did not come from a pinned official artifact" is an anti-goal). Text of the Code appears in the repository only under `samples/uscode/`, generated from the archives committed there. Test fixtures use invented text, in the invented title 99 (`/us/usc/t99/...`) for the ingester and the invented corpus `/zz/demo/...` everywhere else, and say so in their first line.
- **The compiler knows no corpus and no legal vocabulary** (11: no "special cases in the compiler or evaluator for a particular statute, example or name", no "legal vocabulary built into the compiler"). Only `crates/fidryn-ingest-uscode` may name the Code, a title, a publisher element or a status value. `crates/fidryn-provision`, `crates/fidryn-syntax/src/e2`, `crates/fidryn-check/src/e2` and the workspace code of `crates/fidryn-driver` hold none of the words `usc`, `uscode` and `uslm` and nowhere the string `/us/`, their tests included; Task 1 adds a test that enforces it.
- **Coverage words** (11: no "coverage numbers that count a provision as encoded when its operative words are not all represented"). The ledger and the documents call provisions "imported", "addressed" and "classified", never "encoded". They say "every provision of the Code", never "every provision of positive law", and name the source as "the official XML release point published by the Office of the Law Revision Counsel", never "the official Code".
- **Deterministic output** (9.1 goal 6: "Output is byte-identical across runs."). Nothing from the clock, the host name, the user, an absolute path, the iteration order of a hash map or the number of worker threads reaches a generated file, a lock, a ledger, a change set or a report. Generated files use `\n` line ends and `/` in every path on every host. Retrieval times come from the retrieval record written at download.
- **No network in the toolchain.** No `fidryn` command opens a connection. Downloads are done by `tools/fetch_uscode.py` (Task 7), run by a person.
- **Branches.** Work happens on `stage1/code-as-checked-program`, cut from `dev` at `43830cd`, and on lane branches cut from it. Pushing, merging into `main`, changing GitHub settings and publishing the law repository are the owner's actions.
- **Commits.** GPG-signed (already configured), one per task unless the task says otherwise, an imperative sentence without a type prefix as subject, no body, no trailers, no mention of tooling authorship. If a commit fails to sign, stop and report. Never pass `--no-gpg-sign` and never change the signing configuration.
- **Gate.** `cargo xtask ci --fmt`, exit 0 with no warnings, on the tree that is committed. On this machine: `lease run --cpus 16 --mem 25G --time 2h -- cargo xtask ci --fmt`. A lane worktree sets its own `CARGO_TARGET_DIR` under `/cache/cargo/` (named in its dispatch) so that lanes do not wait on one another's build lock.
- **Toolchain.** Rust 1.98.1, edition 2024. Python through `uv` only, never `pip`. New Rust dependencies are added in Task 1 and nowhere else, pass `deny.toml`, and are fetched once with `cargo fetch --locked` before the first offline build.
- **Do not edit `.cargo/config.toml`.**
- **Existing code named in this plan** (functions, files, the order of the gate's steps) was read at `43830cd`. Where the code differs from what a task says about it, the code is right: follow it and say so in the report (design 12: findings from surveys "must be re-checked before" they are "relied on").
- **Python** follows the configuration in `pyproject.toml` (`ruff`, `mypy --strict`, Python 3.12) and the habits of the scripts already in `conformance/`: standard library first, typed functions, no new dependency beyond the two of Task 1.
- **Reader documents** (`README.md`, `docs/`, the site) describe what the commit does. They do not describe plans, stages, a roadmap or the program's destination. `docs/*.md` is canonical; files under `site/` are generated with `cargo xtask site` and committed with the change that alters them.
- **Spelling.** American spelling in every tracked file. No emojis. No mention of an AI, an assistant or a model in code, comments, documents or commits.
- **Storage on this machine.** Official downloads under `/library/corpora/uscode/<release>/`; the local law repository under `/work/projects/law-local/`; the check cache for full runs under `/cache/fidryn/`. Nothing of the full Code is written inside this repository.
- **Memory and time limits on this machine.** These are not optional; an uncapped run took the desktop down twice.
  - Cargo only through `lease` with explicit resources (`--cpus 16 --mem 25G --time 1h` for builds and single test targets, `--time 2h` for the gate). A job that waits as PENDING is waiting its turn: never resubmit it and never shrink the request.
  - Every program built from this repository runs as `timeout 600 prlimit --as=8589934592 --cpu=900 -- <program> <arguments>` when it reads fixtures or the sample, and inside `lease run --cpus 16 --mem 16G --time 1h -- ...` when it reads the full Code (16 GB is the bound of acceptance item 9). One such run at a time. A run that hits a limit is a finding to report, not something to retry with a larger limit.
  - Python that reads more than the sample runs inside `lease run --cpus 8 --mem 16G --time 2h -- uv run ...`.
  - Shell is zsh in which `test` and `dev` are functions that run `git checkout`: use `[[ ... ]]`, never a bare `test`; `git checkout dev` is safe. Never name a loop variable `path`. An unmatched glob aborts the command: list files with `find`. `rm -rf` takes literal absolute paths only.
- **Downloaded files are untrusted data.** An archive lives in a directory that holds nothing but archives and their retrieval record. No script or build runs from such a directory. Python reads them with `-I` and `defusedxml`. The ingester refuses a document type declaration, an entity declaration, an archive member whose name is not a plain file name ending in `.xml`, and a member larger than the limit in S7.

## Review Focus

Five kinds of input that the design implies, that the Code or its next release will present, and that the ordinary tests of a task would not meet. Each has a test in the task that owns the code.

1. **Two addresses that differ only in the case of a letter, or one identifier used twice.** On a file system that ignores case the two modules must not land on one path, and a repeated identifier must not silently replace a module. (Task 3: `paths_of_addresses_that_differ_only_by_case_are_distinct`, `layout_is_reversible_for_every_segment_form`; Task 8: `a_repeated_identifier_keeps_both_elements_at_addresses_of_their_own`.)
2. **A unit far larger than any in the sample.** The new parser has limits of its own, set from the measured largest unit, and reads a module of that size in bounded memory. (Task 5: `a_module_of_the_size_of_the_largest_unit_parses_within_the_limits`; Task 21 records the largest real one.)
3. **Structures that are not sections.** A court rule, a reorganization plan, text between two children of one level, a proviso, a level with no identifier, a file with no units at all. (Task 8: one fixture and one test for each, named there.)
4. **A release that rewrites a file without amending it.** New element ids, a new release name in the metadata, and nothing else: the generated modules must be the same bytes, and the change set must be empty. (Task 10: `two_files_that_differ_only_in_ids_and_release_name_give_identical_modules`; Task 20: `a_reissued_title_has_an_empty_change_set`.)
5. **Text the normalization rule did not expect.** A sequence that Unicode composition changes, a no-break space inside a number, a zero-width space, a bracketed editorial correction such as "[sic]", a footnote marker inside operative words, a superscript in a formula, an empty table cell and a cell that spans rows. (Task 3: the normalization table test; Task 4: `a_footnote_marker_is_outside_the_operative_hash`, `a_superscript_changes_the_hash`; Task 8: `table_cells_keep_their_spans_and_empty_cells`.)

## Specification

The tasks implement this section. Where a task says "S5", it means section S5 here. The same rules are written into `docs/law-modules.md` by the tasks that implement them, in the present tense and without the reasons.

### S1. The edition marker

The first two significant tokens of an edition-2 file are the word `edition` and the integer `2`. White space, line ends and comments before them are skipped. A file whose first significant token is anything else is an edition-1 file and takes the present path.

```rust
// crates/fidryn-syntax/src/edition.rs
pub enum Edition { One, Two }
pub fn edition_of(source: &str) -> Result<Edition, Diagnostic>;
```

- `edition 2` gives `Ok(Edition::Two)`.
- `edition` followed by anything else gives `Err` with code `E100` and the message ``unknown edition `<text of the next token>`; this toolchain reads edition 2 after the word `edition`, and files of the first edition carry no marker``, with the span of that token (or of `edition` at the end of the file).
- Anything else, the empty file included, gives `Ok(Edition::One)`.

Where the choice is made:

| Entry point | Edition 2 |
| --- | --- |
| `fidryn check FILE` | the single-module check of S12, then `ok` |
| `fidryn check DIR` | the workspace check of S12 |
| `fmt`, `inputs`, `run`, `explore`, `explain`, `verify`, `diff`, `render`, `review`, `file`, `ui` on an edition-2 file | exit 1 with `E100`: "an edition 2 module holds provisions and is not compiled for evaluation; `fidryn check` reads it" |
| `fidryn_cli::compile_source`, `compile_module` | the same `E100` |
| the documentation check (`xtask/src/docs_check.rs`) | a fenced block whose first non-blank line is `edition 2` is a complete module and gets the single-module check |

No edition-1 code is changed beyond the one call that asks for the edition.

### S2. Addresses and file paths

An address names a module, a provision or a note.

```
Address = "/" Segment { "/" Segment }
Segment = one or more of: ASCII letters, ASCII digits, "-", ".", "~" and the en dash U+2013
```

- Comparison is byte for byte. `/us/usc/t1/s8/a` and `/us/usc/t1/s8/A` are different addresses.
- A segment is never empty, never `.` and never `..`.
- The full stop and the en dash are there because the publisher's identifiers use them (F3). No publisher identifier of the Code holds `~`: an ingester uses it, and only it, to mark what it had to add to an identifier (S7).
- For the Code an address is the publisher's identifier, unchanged (design 9.3), wherever that identifier is one address and no other element has it. S7 gives the rule for the rest.
- The first segment names the authority that issues the names below it, the second the corpus. Nothing in Stage 1 interprets either.

```rust
// crates/fidryn-provision/src/address.rs
pub struct Address(/* private */);
pub enum AddressError { Empty, NoLeadingSlash, EmptySegment { index: usize }, DotSegment { index: usize }, Character { index: usize, character: char } }
impl Address {
    pub fn parse(text: &str) -> Result<Address, AddressError>;
    pub fn as_str(&self) -> &str;
    pub fn segments(&self) -> std::str::Split<'_, char>;
    pub fn parent(&self) -> Option<Address>;                    // None for a one-segment address
    pub fn child(&self, segment: &str) -> Result<Address, AddressError>;
    pub fn is_within(&self, other: &Address) -> bool;           // equal, or below `other` at a segment boundary
}
```

`Address` implements `Clone`, `Eq`, `Ord` (byte order of the text), `Hash`, `Display`, `Serialize` and `Deserialize` as a string.

**File paths.** A module with address `A` is the file `layout::module_path(A)` under its workspace's root:

1. Each segment of the address maps to one path segment, character by character: a lower-case ASCII letter, a digit or `~` is kept; an upper-case ASCII letter is written as `_` followed by its lower-case form; the en dash is written `-`; the ASCII hyphen is written `%2D`; a full stop is kept, except in three places, where it is written `%2E`: as the first character of a segment, as the last character of a segment, and before the letters `fr` that end a segment. So no path segment begins with a full stop, and none ends in a full stop or in `.fr`: the directory of one module is never hidden from discovery and never has the name of another module's file.
2. If the mapped segment, or its part before its first full stop, compared without regard to case, is one of `con`, `prn`, `aux`, `nul`, `com1` to `com9` or `lpt1` to `lpt9`, its first character is written as `%` followed by the two upper-case hexadecimal digits of its code.
3. The path is the mapped segments joined by `/`, followed by `.fr`.

`/us/usc/t29/s2611` is `us/usc/t29/s2611.fr`; `/us/usc/t42/s1395i–3` is `us/usc/t42/s1395i-3.fr`; `/us/usc/t2/s44...46` is `us/usc/t2/s44...46.fr`; `/us/usc/t5/ptIII/sptE` is `us/usc/t5/pt_i_i_i/spt_e.fr`; `/us/usc/t28a/courtRules/Civil/rule6` is `us/usc/t28a/court_rules/_civil/rule6.fr`; `/us/usc/t28/s1932~2` is `us/usc/t28/s1932~2.fr`. An address `/zz/demo/a.fr/b` would be `zz/demo/a%2Efr/b.fr`, beside the file `zz/demo/a.fr` of the address `/zz/demo/a`.

```rust
// crates/fidryn-provision/src/layout.rs
pub fn module_path(address: &Address) -> String;                           // always `/` separators
pub fn address_of_path(path: &str) -> Result<Address, LayoutError>;      // the inverse; rejects a path that `module_path` cannot produce
```

The mapping is one to one, so two addresses never share a path, also on a file system that ignores case.

### S3. Normalization

Text is normalized once, by the ingester, and stored normalized. The checker rejects text that is not.

`normalize(raw)`:

1. Put the text into Unicode Normalization Form C.
2. Replace each of these 25 characters, the ones with the Unicode property `White_Space`, with U+0020: U+0009 to U+000D, U+0020, U+0085, U+00A0, U+1680, U+2000 to U+200A, U+2028, U+2029, U+202F, U+205F, U+3000.
3. Replace each run of U+0020 with one.
4. Remove U+0020 at the start and at the end.

Nothing else changes: no case folding, no change to quotation marks or dashes, and U+200B (zero width space) and U+FEFF are kept, because they are not white space.

```rust
// crates/fidryn-provision/src/normalize.rs
pub const WHITE_SPACE: [char; 25];
pub fn normalize(raw: &str) -> String;
pub fn is_normalized(text: &str) -> bool;       // normalize(text) == text
```

The unit of normalization is the **block** (a paragraph, a heading, a number, a table cell paragraph). The text of a block is the concatenation of the words of its runs (S4), and that concatenation is what must be normalized: a run may begin or end with a space, a block may not.

### S4. The model

```rust
// crates/fidryn-provision/src/model.rs
pub enum Layer { Operative, Enacted, Editorial }

pub struct Text { pub runs: Vec<Run> }
pub enum Run {
    Words(String),
    Ref { words: String, target: String },   // a link: the linked words and the target as the source writes it
    Sup(String),                             // superscript words
    Sub(String),                             // subscript words
    Mark { shown: String, footnote: u32 },   // a footnote marker; its text is not part of the block's text
}
impl Text {
    pub fn plain(&self) -> String;            // the words of every run but `Mark`, concatenated
}

pub enum Block { Para(Text), Subheading(Text), Table(Table), Quoted(Quoted), Image { source: String } }
pub struct Table { pub rows: Vec<Row> }
pub enum RowGroup { Head, Body, Foot }
pub struct Row { pub group: RowGroup, pub cells: Vec<Cell> }
pub struct Cell { pub header: bool, pub colspan: u32, pub rowspan: u32, pub blocks: Vec<Block> }

pub struct Num { pub value: String, pub shown: Text }   // the machine form and the form printed, which may hold a footnote marker
pub enum PartRole { Chapeau, Content, Continuation, Proviso }
pub enum Item { Part { role: PartRole, blocks: Vec<Block> }, Child(Provision) }
pub struct Provision {
    pub address: Address,
    pub kind: String,             // an identifier the ingester chooses from its closed list, such as `section`
    pub num: Option<Num>,
    pub heading: Option<Text>,
    pub status: Option<String>,   // an identifier; `None` is the publisher's default, in force
    pub items: Vec<Item>,         // text parts and children in document order
}

pub struct Quoted { pub origin: Option<String>, pub items: Vec<QuotedItem> }
pub enum QuotedItem { Block(Block), Part { role: PartRole, blocks: Vec<Block> }, Level(QuotedLevel) }
pub struct QuotedLevel { pub kind: String, pub num: Option<Num>, pub heading: Option<Text>, pub items: Vec<QuotedItem> }

pub struct Note {
    pub address: Address,
    pub layer: Layer,             // Enacted or Editorial
    pub group: String,            // the publisher's group word, or `ungrouped`
    pub topics: Vec<String>,
    pub heading: Option<Text>,
    pub blocks: Vec<Block>,
}
pub struct Footnote { pub ordinal: u32, pub shown: String, pub blocks: Vec<Block> }

pub enum DefinitionKind { Means, Includes, DoesNotInclude, Borrowed }
pub enum Basis { Enacted, Restated, Unresolved }
pub struct Definition {
    pub at: Address,
    pub terms: Vec<String>,
    pub kind: DefinitionKind,
    pub words: Option<String>,    // the reach phrase as the text has it; `None` is "unstated"
    pub region: Option<Address>,
    pub basis: Basis,
    pub borrowed_from: Option<String>,
    pub see_note: Option<Address>,
}

pub struct UnitModule {
    pub address: Address,
    pub aliases: Vec<Address>,    // other addresses the source gives this unit (S7)
    pub within: Option<Address>,
    pub source: String,           // the member of the pinned archive the unit was read from
    pub positive_law: Option<bool>,
    pub root: Provision,          // root.address == address
    pub credit: Option<Text>,
    pub notes: Vec<Note>,         // document order
    pub footnotes: Vec<Footnote>, // ordinal order, from 1
    pub definitions: Vec<Definition>,
}
pub struct ContainerModule {
    pub address: Address,
    pub aliases: Vec<Address>,
    pub within: Option<Address>,
    pub source: String,
    pub positive_law: Option<bool>,
    pub kind: String,
    pub num: Option<Num>,
    pub heading: Option<Text>,
    pub status: Option<String>,
    pub members: Vec<Address>,    // sub-containers and units in document order
    pub credit: Option<Text>,     // eight containers of release 119-142 have one
    pub notes: Vec<Note>,
    pub footnotes: Vec<Footnote>,
}
pub struct Import { pub module: Address, pub alias: Option<String>, pub names: Vec<String> }
pub struct Citation { pub name: String, pub target: Address }
pub enum BindTarget { Address(Address), Name { module: Option<String>, name: String } }
pub struct Binding { pub name: String, pub binds: BindTarget, pub release: String, pub provision_hash: Hash32, pub notes_hash: Hash32 }
pub struct CuratedModule { pub address: Address, pub imports: Vec<Import>, pub citations: Vec<Citation>, pub bindings: Vec<Binding> }
pub enum Module { Unit(UnitModule), Container(ContainerModule), Curated(CuratedModule) }
```

Every type derives `Clone`, `Debug`, `PartialEq`, `Eq`. Invariants the constructors and the checker hold: adjacent `Words` runs are merged; no run has empty words; a `Part` has at least one block; a cell may have none; a child's address lies below its parent's.

### S5. Hashes

`Hash32` is 32 bytes, written as 64 lower-case hexadecimal digits. Every hash is BLAKE3 over a sequence of **frames**. A frame is one tag byte, the length of its value as eight bytes little-endian, and the value. The first frame of every hash has tag `0x00` and holds a domain string.

```rust
// crates/fidryn-provision/src/hash.rs
pub struct Hash32(pub [u8; 32]);
pub fn hash_provision(provision: &Provision) -> Hash32;           // the tree hash of goal 3
pub fn hash_note(note: &Note) -> Hash32;
pub fn hash_container(container: &ContainerModule) -> Hash32;
pub struct LayerHashes { pub operative: Hash32, pub enacted: Hash32, pub editorial: Hash32 }
pub fn layer_hashes_of_unit(unit: &UnitModule) -> LayerHashes;
pub fn layer_hashes_of_container(container: &ContainerModule) -> LayerHashes;
```

Encodings (`‖` is concatenation, `F(t, v)` a frame with tag `t`):

| What | Frames |
| --- | --- |
| text `T` | `F(0x10, UTF-8 of T.plain())`, then for each run that is not `Words`, in order: `Ref`: `F(0x11, start ‖ end ‖ UTF-8 of target)`; `Sup`: `F(0x12, start ‖ end)`; `Sub`: `F(0x13, start ‖ end)`; `Mark`: nothing. `start` and `end` are byte offsets into `T.plain()`, eight bytes little-endian each. |
| absent text | `F(0x1F, empty)` |
| block | `Para`: `F(0x20, text)`; `Subheading`: `F(0x21, text)`; `Table`: `F(0x22, table)`; `Quoted`: `F(0x23, quoted)`; `Image`: `F(0x24, UTF-8 of source)` |
| table | for each row `F(0x30 + group, cells)` with group 0 head, 1 body, 2 foot; a cell is `F(0x34 or 0x35, colspan ‖ rowspan ‖ blocks)` with `0x35` for a header cell and each span four bytes little-endian |
| quoted | `F(0x40, UTF-8 of origin or empty)`, then each item: a block as above; a part as below; a level `F(0x41, F(0x01, kind) ‖ num ‖ heading ‖ items)` |
| num | `F(0x02, UTF-8 of value) ‖ F(0x03, shown as text)`, or `F(0x04, empty)` when absent |
| part | `F(0x50 + role, blocks)` with role 0 chapeau, 1 content, 2 continuation, 3 proviso |
| provision (tree) | BLAKE3 of `F(0x00, "fidryn.provision/v1") ‖ F(0x01, kind) ‖ num ‖ heading ‖ F(0x05, status or empty) ‖ items`, where a child item is `F(0x60, its tree hash)` |
| note | BLAKE3 of `F(0x00, "fidryn.note/v1") ‖ F(0x70, group) ‖ F(0x71, topics joined by one space) ‖ heading ‖ blocks`. The address and the layer are not in it. |
| container | BLAKE3 of `F(0x00, "fidryn.container/v1") ‖ F(0x01, kind) ‖ num ‖ heading ‖ F(0x05, status or empty)`, then `F(0x61, address)` for each member in order |
| enacted layer | BLAKE3 of `F(0x00, "fidryn.layer.enacted/v1")`, then for each enacted note in the byte order of its address: `F(0x72, address) ‖ F(0x73, note hash)` |
| editorial layer | BLAKE3 of `F(0x00, "fidryn.layer.editorial/v1") ‖ credit as text or absent text`, then the editorial notes as in the enacted layer, then for each footnote in ordinal order `F(0x74, UTF-8 of shown) ‖ blocks`, then for each `Mark` run of the module in document order `F(0x75, UTF-8 of the address of the provision or note it stands in) ‖ F(0x76, UTF-8 of shown) ‖ F(0x77, footnote as four bytes little-endian)` |
| operative layer | of a unit, the tree hash of its root; of a container, its container hash |

The document order of the `Mark` runs of a module is: the root provision's tree depth first, taking for each provision its number, its heading and then its items in order; then the credit; then the notes in the byte order of their addresses, as the layers take them, the heading of each before its blocks; then each footnote in ordinal order. A container has its number, its heading, its credit, its notes and its footnotes. A marker in a provision or a note is recorded with that provision's or note's address, one in quoted content with the address of the note that holds the quotation, and one in a credit or a footnote with the module's address.

Consequences the tests pin: a change to any word, number, heading, status, link target, superscript, table cell or span of a provision changes its tree hash and the tree hash of every ancestor and of nothing else (goal 3); a footnote, a marker, and the provision a marker stands in change only the editorial layer (where exactly in that provision a marker stands is in no hash); the order of notes in the file does not change a layer hash; moving a note to a new address changes the layer hash and not the note's hash.

### S6. Edition-2 syntax

Tokens are those of the existing lexer (`fidryn_syntax::lexer::lex`), which is not changed. Words such as `module` and `content` are identifier tokens that the new parser compares by their text. A string is a JSON string literal, decoded as the present parser decodes one. Line ends are not significant. `//` begins a comment.

```ebnf
File        = "edition" "2" Module ;
Module      = "module" String "{" { Decl } "}" ;
Decl        = Alias | Within | Source | PositiveLaw | Provision | Container | Credit | Note
            | Footnote | Definition | Layers | Import | Citation | Binding ;

Alias       = "alias" String ;
Within      = "within" String ;
Source      = "source" String ;
PositiveLaw = "positive_law" ( "true" | "false" ) ;

Provision   = "provision" String Ident "{" [ Num ] [ "heading" Text ] [ "status" Ident ]
              { Part | Provision } "hash" String "}" ;
Num         = "num" String Text ;
Part        = Role ( Text | "{" Block { Block } "}" ) ;
Role        = "chapeau" | "content" | "continuation" | "proviso" ;
Block       = "p" Text | "subheading" Text | Table | Quoted | "image" String ;
Text        = Run { Run } ;
Run         = String
            | "[" String "->" String "]"
            | "sup" String | "sub" String
            | "mark" String Int ;

Table       = "table" "{" { Row } "}" ;
Row         = ( "head" | "body" | "foot" ) "{" { Cell } "}" ;
Cell        = ( "td" | "th" ) [ "span" Int Int ] ( Text | "{" { Block } "}" ) ;

Quoted      = "quoted" [ String ] "{" { QItem } "}" ;
QItem       = Block | Part | "level" Ident "{" [ Num ] [ "heading" Text ] { QItem } "}" ;

Container   = "container" String Ident "{" [ Num ] [ "heading" Text ] [ "status" Ident ]
              { "member" String } "hash" String "}" ;
Credit      = "credit" Text ;
Note        = "note" String ( "enacted" | "editorial" ) "{" "group" Ident { "topic" String }
              [ "heading" Text ] { Block } "hash" String "}" ;
Footnote    = "footnote" Int String "{" { Block } "}" ;
Definition  = "definition" String "{" "term" String { "term" String } "kind" Ident
              "words" ( String | "unstated" ) [ "region" String ] "basis" Ident
              [ "borrowed_from" String ] [ "see_note" String ] "}" ;
Layers      = "layers" "{" "operative" String "enacted" String "editorial" String "}" ;

Import      = "import" String ( "as" Ident | "{" Ident { "," Ident } "}" ) ;
Citation    = "citation" Ident "->" String ;
Binding     = "binding" Ident "{" "binds" ( String | Ident [ "." Ident ] )
              "release" String "provision_hash" String "notes_hash" String "pending" "}" ;
```

Rules the parser enforces, each as `E100` with the span of the offending token:

- Every block has the closed set of fields shown, in the order shown. A word that is no field of the block it stands in is "unknown field `<word>` in `<block>`" (goal 1).
- A word that begins no declaration is "unknown declaration `<word>`".
- Text after the module's closing brace is an error.
- Limits: a source of at most `E2_MAX_SOURCE_BYTES`, at most `E2_MAX_TOKENS` tokens, and blocks nested at most 64 deep; the values are in "Measured facts", item F6.

A module is one of three shapes, decided when it is lowered (S12): a **unit** has exactly one `provision` at the top; a **container** has exactly one `container`; a **curated** module has neither. `alias`, `within`, `source`, `positive_law`, `credit`, `note`, `footnote`, `definition` and `layers` belong to units and containers; `import`, `citation` and `binding` belong to curated modules.

**The canonical form** is what `fidryn_provision::render::render` writes, and generated modules are always in it:

- UTF-8, `\n` line ends, a final line end, two spaces of indentation for each open brace, no blank line, no trailing space.
- Line 1 is `edition 2`, line 2 is `module "<address>" {`.
- Declarations in this order: each `alias` in the order of S7, `within`, `source`, `positive_law`, the `provision` or `container`, `credit`, notes in document order, footnotes by ordinal, definitions in the order of their provisions in the document and then of their first term, `layers`.
- One field to a line. A text is written on one line, its runs separated by one space.
- A string is written with `\\` for a backslash, `\"` for a quotation mark, `\u00XX` for a character below U+0020 or U+007F, and every other character as itself.
- A part or a cell whose content is exactly one paragraph is written in the short form (`content "..."`); otherwise in the brace form with one block to a line. A cell with no block is written `td ""`, and that spelling is read back as a cell with no block.
- `span <colspan> <rowspan>` is written only when one of the two is not 1.
- Every `hash` and the three values of `layers` are computed by the rule of S5 from what is written above them.

An example of a unit, with invented text:

```fidryn
edition 2
module "/zz/demo/t1/s8" {
  within "/zz/demo/t1/ch1"
  source "demo01.xml"
  positive_law true
  provision "/zz/demo/t1/s8" section {
    num "8" "§ 8."
    heading "Words that include a widget"
    provision "/zz/demo/t1/s8/a" subsection {
      num "a" "(a)"
      content "As used in " ["section 2 of this title" -> "/zz/demo/t1/s2"] ", the term “widget” means a small device."
      hash "<64 hexadecimal digits>"
    }
    hash "<64 hexadecimal digits>"
  }
  credit "(Added by an invented act.)"
  definition "/zz/demo/t1/s8/a" {
    term "widget"
    kind means
    words "As used in section 2 of this title"
    region "/zz/demo/t1/s2"
    basis enacted
  }
  layers {
    operative "<64 hexadecimal digits>"
    enacted "<64 hexadecimal digits>"
    editorial "<64 hexadecimal digits>"
  }
}
```

### S7. Reading the publisher's XML

These rules are implemented twice, in `crates/fidryn-ingest-uscode` and in `conformance/uslm_common.py`, each from this text. Where the two disagree on real data, this section decides; where it is silent, the task that finds the case stops for a ruling. Its principles: no operative word is dropped; nothing is invented; an element the rules do not name is refused, not skipped; what is by rule not imported is counted.

**Files.** A release is a directory of archives with a retrieval record (S11). A member is read when its name is `usc`, two digits, an optional `a` or `A`, and `.xml`; any other member is refused, and so is one larger than 512 MiB, and so is a release in which two archives hold a member of the same name (an archive of the whole Code beside an archive of one title would otherwise give every unit of that title twice).

**A document is refused** when it has a document type declaration; when its root is not `uscDoc` in the namespace `http://xml.house.gov/schemas/uslm/1.0`; when the file named in its schema location is not `USLM-1.0.15.xsd`; when its `docPublicationName` is not `Online@` followed by the release of the retrieval record; when its property `is-positive-law` is neither `yes` nor `no`; when an element stands where its class below does not allow it or is in no class; when a `ref` has neither an `href` nor the class `footnoteRef`; when a `status` is not one of the 19 values of the schema: `proposed`, `withdrawn`, `cancelled`, `pending`, `operational`, `suspended`, `renumbered`, `repealed`, `expired`, `terminated`, `hadItsEffect`, `omitted`, `notAdopted`, `transferred`, `redesignated`, `reserved`, `vacant`, `crossReference`, `unknown`.

**Classes of elements.**

| Class | Elements |
| --- | --- |
| Document | `uscDoc`, `meta` (read for the release name and the positive-law flag; nothing else in it is imported), `main`. The top container of a file is the `title` inside `main`, or the `appendix` that stands directly under `uscDoc`: an appendix file has no `main` |
| Container | `title`, `appendix`, `subtitle`, `chapter`, `subchapter`, `part`, `subpart`, `division`, `subdivision`, `article`, `courtRules`, `compiledAct`, `reorganizationPlans`; and `level` when no unit stands above it and no text part or `p` is its child |
| Unit | `section`, `courtRule`, `reorganizationPlan`; and `level` when no unit stands above it and a text part or a `p` is its child |
| Provision inside a unit | any element of the two classes above, or `subsection`, `paragraph`, `subparagraph`, `clause`, `subclause`, `item`, `subitem`, `subsubitem`, `level`, that stands inside a unit. Its kind is its tag. |
| Label | `num` and `heading` of a level |
| Text part | `chapeau`, `content`, `continuation`, `proviso` |
| Block | `p` and `xhtml:p`, which are read alike; `xhtml:table`; `quotedContent`; `img`; in a note, and in quoted content inside one, also `signature`; in a note also any `heading` or `subheading` after the note's first heading, and a text part that stands directly in the note; in quoted content also `note` |
| Inline, kept as structure | `ref` with an `href` (a link); `ref` of class `footnoteRef` (a footnote marker); `sup` and `xhtml:sup`; `sub` and `xhtml:sub` |
| Inline, words kept and markup dropped | `date`, `i`, `b`, `inline`, `xhtml:span`, `xhtml:b`, `xhtml:i`, `xhtml:a`, `name`, `role`; `br` is one space |
| Table structure | `xhtml:thead`, `xhtml:tbody`, `xhtml:tfoot`, `xhtml:tr`, `xhtml:th`, `xhtml:td`; `xhtml:colgroup` and `xhtml:col` hold no text and are dropped |
| Note | `notes`, `note`, `sourceCredit` (S8) |
| Not imported, counted | `toc` and everything inside it; the release notes described in S8; the label of a cross heading (S8) |

The unit of a text is the outermost unit that holds it: the section that stands inside 25 U.S.C. 5329 is a provision of kind `section` of that unit.

**The blocks of a flow.** A text part, a `p`, a note, quoted content, a level of quoted content and a table cell are flows. Walk the children of a flow in document order. Text and inline elements gather into the current paragraph. A block element closes the current paragraph and is itself the next block. How each block is read: a `p` that holds only text and inline elements is one paragraph; a `p` that holds a block is replaced by the blocks of its own flow; a `heading` or `subheading` after a note's first heading is a `Subheading` of its text; a `signature` gives one paragraph for each `name` and each `role` in it, in document order; a text part that stands directly in a note, where the publisher sets a law out as loose paragraphs with a proviso among them, is replaced by the blocks of its own flow; a `note` inside quoted content is replaced by the blocks of its own flow, its heading, if it has one, first as a `Subheading`; a table, quoted content and an image are read as their own paragraphs below say. A footnote (`note` with `type="footnote"`) is taken out of the flow where it stands, without closing the paragraph, and becomes a footnote of the module (below). The end of the flow closes the current paragraph. A paragraph whose text is empty after normalization is dropped.

**The text of a paragraph.** Concatenate its text nodes in document order; an inline element gives its own text the same way; a `br` gives one space; the boundary of an element gives nothing; a footnote marker gives nothing, and its place is remembered. Normalize the whole (S3). The runs: a link is a `Ref` with the link's words and its `href`, unchanged; a `sup` or `sub` (of either namespace) is `Sup` or `Sub`; everything else is `Words`. The extent of a `Ref`, `Sup` or `Sub` is that of its own words with white space at its two ends left outside it. A link with no words left is dropped and counted. A footnote marker is a `Mark` with the marker's own text as `shown`; it stands directly after the last character before it that is not white space, which is at the offset `normalize(the text before the marker)` has as its length, so that a marker between two spaces leaves one space, after it. The same holds for the text of a `num` and of a `heading`.

**Footnotes.** Each `note` with `type="footnote"` of a unit or a container, wherever it stands in it (in a text part, a heading, a number, a note), becomes a footnote of that module, numbered from 1 in document order; `shown` is the text of its `num`, and its blocks are the flow of the rest. A marker that points at it by `idref` is `Mark { shown, footnote }` with that number; a marker whose `idref` is the `id` of no footnote of its module is refused. A footnote inside a table of contents is not imported. A bare superscript is never a marker (F7a).

**Parts and children.** The children of a level in document order give its items: a text part gives a part with the blocks of its flow; a level gives a child provision; a `p` directly under a level gives a `content` part. A `continuation` or a `proviso` that stands inside a `content` closes that part, is a part of its own, and what follows it in the `content` opens a further `content` part. A part with no block is dropped.

**Number, heading, status.** A `num` gives `value` from its `value` attribute (empty when there is none) and `shown` from its text. A `heading` is one text. A level with no `num` or no `heading` has none. The status is the `status` attribute, absent for a level in force.

**Tables.** A row belongs to the group of the `thead`, `tbody` or `tfoot` it stands in, and to the body when it stands in none. A cell is a header for `th`; its spans are the `colspan` and `rowspan` attributes, 1 when absent; its blocks are those of its flow. No cell is dropped, an empty one included (design 9.7: "provided no cell is lost").

**Images.** An `img` gives `Image { source }`, where `source` is its `src` attribute normalized by S3. An `img` with any other attribute, with text or with a child is refused. The file it names is not in the archive, so the picture itself is not imported; the ledger counts the provisions and notes that hold one.

**Addresses.**

1. *A unit or a container that has an identifier.* If the identifier holds a space, it is a list: its first member is the identifier used below, and the other members are aliases of the element. If the last segment of the identifier holds three full stops in a row, the element stands for a range: the identifier is used as it is, and its aliases are its two ends: the identifier cut before the full stops, and the identifier with its last segment replaced by that segment's leading lower-case letters followed by what stands after the full stops. When the two ends differ only in a whole number at their end, the second not smaller than the first and not more than 999 above it, every number between them gives an alias as well (`/us/usc/t2/s44...46` has the aliases `/us/usc/t2/s44`, `/us/usc/t2/s45` and `/us/usc/t2/s46`; `s60e–3...60e–14` has twelve). A member of a range whose number is followed by a letter is not named by the identifier and gets no alias. The address is the identifier, unless a unit or container before it in the release, taking the files in the order of their names and the elements in document order, already has that address; then it is the identifier followed by `~2`, by `~3` for the next, and so on. The count runs over the release and not over one file, so an implementation that reads members side by side first lists the units and containers of every member and then numbers them; the result is the same for any number of workers.
2. *A unit or a container that has no identifier.* The address of the container it stands in, or the prefix of the corpus for the top element of a file, followed by the segment `~<tag>.<n>`, where `n` counts from 1 the elements of that tag without an identifier in that same place, in document order.
3. *A provision inside a unit.* The address of its parent followed by the last segment of its identifier, and by `~2`, `~3` if a sibling before it already has that address; with no identifier, followed by `~<tag>.<n>` as in rule 2.
4. An alias that is the address of another element is dropped. Aliases are written in the order the rules give them.
5. The lock counts the elements whose address got `~2`, `~3` and so on by rule 1 or rule 3 (disambiguated), the elements whose own segment is `~<tag>.<n>` by rule 2 or by the last case of rule 3 (assigned), and the aliases. What stands inside such an element is not counted again.

So an address is the publisher's identifier in every case but these: the later holders of a repeated identifier; elements without one; and what stands inside either.

**Containers and units.** A container's members are the containers and units that are its children, in document order, by address. `within` of a unit or a container is the address of the container it stands in; the top container of a file has none. `source` is the member's file name. `positive_law` is the flag of the file.

**The source credit** of a unit or a container is the text of the `sourceCredit` that is its direct child. A second one, and one that stands anywhere else, is refused.

**Definitions** are extracted from finished units by S10.

### S8. Notes: their layers and addresses

**Which elements are notes.** (1) Each `note` child of a `<notes>` element, except one with `role="crossHeading"`, which is read for its topic and is not itself a note: its heading is the publisher's label of the group, is not imported, and is counted. Where a cross heading holds anything after its first heading, as nine do in release 119-142, those blocks are a note of the group it opens, with no heading and no topics. (2) Each `note` that is a direct child of a level and is not a footnote. Of the second kind, one that stands directly under a `title` or an `appendix` and whose text begins `Current through ` is a release note: it is not imported, and it is counted.

**Where a note belongs.** Its anchor is the level that holds it or its `<notes>` element. A note anchored anywhere in a unit belongs to that unit's module; one anchored at a container belongs to the container's module.

**Group.** Of a note in a `<notes>` element: the `topic` of the nearest cross heading before it in that element, and `ungrouped` when there is none. Of a note directly under a level: `ungrouped`.

**Layer.**

| Group | Layer |
| --- | --- |
| `editorialNotes` | editorial |
| `ungrouped`, and the note's topic is `removalDescription` or `historicalAndRevision` | editorial (design 9.4 names "revision notes" and "descriptions of removed sections" as editorial matter) |
| anything else: `statutoryNotes`, `execDoc`, `courtRules`, every other `ungrouped` note, and any group word a later release introduces | enacted (design 9.4: "A note that the publisher's grouping does not classify is treated as enacted") |

**Content.** `topics` is the `topic` attribute split at spaces. The heading is the note's first `heading`. The blocks are the flow of everything else in it.

**Address.** `<anchor address>/note/<group>/<law>/<heading>`, followed by `/<k>` where needed:

- `<group>` is the group word.
- `<law>`: take the `origin` of the note's first `quotedContent` if it begins `/us/pl/`, `/us/act/` or `/us/stat/`; otherwise the `href` of the first link in the note that begins `/us/pl/` or `/us/act/`. Keep its first four segments and write the last three joined by full stops: `pl.117.228`, `act.1947-07-30.ch388`, `stat.61.633`. A note with neither has `nolaw`. A reference of fewer than four segments, or one whose second, third or fourth segment holds a character an address segment cannot hold, is refused with the note named.
- `<heading>`: the heading's text with the letters `A` to `Z` put into lower case and no other character changed, then with every run of characters other than ASCII letters and digits replaced by one `-`, without `-` at either end, cut to 60 characters and again without `-` at its end. A note with no heading, or whose heading leaves nothing, has `noheading` when it has a law, and `h.` followed by the first 16 hexadecimal digits of its hash (S5) when it has neither (design 9.4: the address "is derived from the enacting law the note cites where there is one, and from the note's content otherwise").
- `<k>`: when several notes of one anchor agree in all three parts, the second in document order gets `/2`, the third `/3`.

A note's address therefore changes when its group, the law it cites first, or its heading changes, and for a note with neither a law nor a heading when its text changes; the change set pairs a note with its earlier self by equal hash where the hash did not change (S15). Inserting a note changes the address of another only when the two agree in group, law and heading.

**Staleness** counts only the enacted notes of a unit (S13). Notes of a container are in the container's layers and make no binding stale.

### S9. References and their outcomes

Every `Ref` run in every layer is a reference: the linked words and a target. Its layer is that of what it stands in: operative in a provision and in the number or heading of a container; the note's own layer in a note, quoted content included; editorial in a source credit and in a footnote. A reference needs no import. The workspace check gives each reference exactly one outcome, the first of this list that applies:

| Outcome | When |
| --- | --- |
| `malformed` | the target is not an address by S2 |
| `ambiguous` | some beginning `B` of the target, the target included, is an address of the workspace or of a dependency, and an address `B~2` exists beside it: the source gave one identifier to more than one element, and the reference cannot say which it means |
| `exact` | the target is the address of a module, a provision or a note, or an alias of a module, and neither it nor the unit it lies in has a status |
| `target-status` | as `exact`, but the target or its unit has a status, which is recorded |
| `unit-only` | the longest beginning of the target that is such an address is a unit or lies in one |
| `range-form` | otherwise: take that longest beginning, or, when no beginning is such an address, the prefix of the workspace or dependency the target lies below. The segment after it holds a dash (U+2013 or `-`), and the beginning followed by that segment cut at its first dash is such an address. The target is a range of units or a dashed number that is not in the corpus; the reference cannot say which. |
| `no-unit` | otherwise, the target lies below the prefix of the workspace or of a dependency |
| `outside` | the target lies below no such prefix; recorded with its first two segments, the body of law it names |

A beginning of a target is the target cut after any of its segments. An address wins over an alias: a target that is both the address of one module and an alias of another is the first. The list is closed and one of its last two rows always applies, so no reference is "unknown". An outcome is never a diagnostic (goal 4: a reference that cannot resolve "never fails the build by itself"). The report and the ledger count outcomes by layer, and `fidryn check DIR --unresolved FILE` writes every reference whose outcome is not `exact`, one JSON object to a line, sorted by the address it stands in.

A provision is counted as addressed (depth D1) when each of its references has an outcome, which is always. The ledger therefore reports the split by outcome beside the count, so that a reader sees how many references of the addressed provisions reach their target.

Against the counts of the design's acceptance item 2: `exact`, `target-status` and `ambiguous` together correspond to "resolve exactly", `unit-only` to "reach the section only", and `range-form` with `no-unit` to "reach no section", with the differences S16 explains.

### S10. Definitions

A definition is extracted by the ingester from the first paragraph of a `chapeau` or `content` part of an operative provision, when that paragraph's text matches one of the patterns below, and from nothing else. A provision that matches none stays a plain provision (design 9.5). `Q` is a term in curly quotation marks, `“` and `”`; the term is recorded without the marks.

**Verbs.**

| Kind | Verb forms |
| --- | --- |
| `means` | `means`, `mean`, `shall mean` |
| `includes` | `includes`, `include`, `shall include` |
| `does_not_include` | `does not include`, `do not include`, `shall not include` |
| `borrowed` | `has the meaning given`, `have the meaning given`, `has the meanings given`, `have the meanings given`, `has the same meaning as`, `have the same meaning as`, `shall have the meaning given` |

**Patterns.**

1. *Sentence form.* `[Reach ","] ("the"|"The") ("term"|"terms"|"word"|"words"|"phrase") Q { ("," | "and" | "or" | ", and" | ", or") Q } ["," Aside ","] Verb ...`. `Aside` is text with no quotation mark and no verb form of the table in it.
2. *List form.* The paragraph begins `Q { ("," | "and" | "or" | ", and" | ", or") Q } Verb ...`, and the provision's parent has a `chapeau` whose text is `Reach` followed by nothing but a dash (U+2014 or `-`) or a colon. The reach words are the parent's.

**Reach.** `Reach` is `("As used in" | "In" | "For purposes of" | "For the purposes of" | "When used in") Object`, where `Object` is text up to the comma or dash that ends the reach, with no quotation mark in it. After `In`, the `Object` must begin with `this` and a space, or be a single link; a paragraph that opens with `In` and anything else ("In the case of an individual, the term ...") states a condition, not a reach, and is not extracted. The reach words are recorded exactly as the text has them, without the closing comma or dash. A paragraph of the sentence form with no `Reach` has the words `unstated`. A paragraph whose text before `the term` is anything other than a `Reach` is not extracted.

**Region.** When `Object` is `this` followed by one of `title`, `subtitle`, `chapter`, `subchapter`, `part`, `subpart`, `division`, `section`, `subsection`, `paragraph`, `subparagraph`, `clause`, the region is the nearest ancestor of the provision (the provision itself included, then its unit's containers in order) whose kind is that word. When `Object` is a single link whose target lies below the prefix of the corpus, the region is that target. Otherwise there is no region; a link to a public law or to the Statutes at Large names no region of the Code.

**Basis.** No region: `unresolved`. A region, in a unit whose `positive_law` is true: `enacted`. A region, in any other unit: `restated`.

**Borrowed.** A definition of kind `borrowed` records as `borrowed_from` the target of the first link after the verb. If there is none, the paragraph is not extracted.

**The note.** When the basis is `restated` and the unit has an editorial note whose heading is "References in Text" and whose text contains, without regard to case, the `Object` followed by ", referred to in", the definition records that note's address as `see_note`.

The checker verifies of each definition, in any module: the provision it names is in the module; each term stands in that provision's text; the reach words, when stated, stand in the provision's text or in the chapeau of its parent; the kind and the basis are of the closed lists; a `borrowed` definition has `borrowed_from`.

### S11. Workspaces

A workspace is a directory with a file `workspace.json` at its top.

```json
{
  "schema": "fidryn.workspace/v0.1",
  "name": "uscode",
  "kind": "generated",
  "prefix": "/us/usc",
  "release": "119-142",
  "toolchain": "0.3.0",
  "dependencies": []
}
```

- `kind` is `generated` or `curated`. Every module of a workspace has an address below `prefix` and stands at the path S2 gives for it.
- `release` is the release of a generated workspace. A curated workspace has none, and its manifest has no such key.
- A **generated** workspace has a lock, `law.lock.json`, written by an ingester. Every `.fr` file in it is listed in the lock with the BLAKE3 of its bytes, and every listed file is present. It has no dependencies in Stage 1.
- A **curated** workspace has no lock. Each of its dependencies names a generated workspace:

```json
{ "name": "uscode", "path": "../workspace", "release": "119-142", "lock_digest": "<BLAKE3 of the dependency's law.lock.json>" }
```

- `toolchain` is the version of the toolchain the workspace was last checked with (design 4.3). A different running version is the warning `W760`.
- Unknown fields are refused. Paths in a manifest use `/` and are relative to the manifest's directory.
- A `.fr` file of the first edition anywhere in a workspace is an error (`E730`), so that nothing in the directory goes unchecked without a word. Directories whose name begins with a dot are not read.

The lock:

```json
{
  "schema": "fidryn.law-lock/v0.1",
  "prefix": "/us/usc",
  "release": "119-142",
  "generator": { "name": "fidryn-ingest-uscode", "version": "0.3.0" },
  "artifacts": [
    {
      "file": "xml_usc01@119-142.zip",
      "url": "https://uscode.house.gov/download/releasepoints/us/pl/119/142/xml_usc01@119-142.zip",
      "retrieved_at": "2026-11-16T22:14:03Z",
      "bytes": 42241,
      "blake3": "<64 hexadecimal digits>",
      "sha256": "<64 hexadecimal digits>",
      "members": [ { "name": "usc01.xml", "bytes": 284795, "blake3": "<64 hexadecimal digits>" } ]
    }
  ],
  "addresses": { "aliases": 0, "disambiguated": 0, "assigned": 0 },
  "not_imported": { "tables_of_contents": 0, "footnotes_in_tables_of_contents": 0, "links_in_tables_of_contents": 0, "release_notes": 0, "links_without_words": 0, "note_group_headings": 0 },
  "modules": { "us/usc/t1.fr": "<64 hexadecimal digits>" }
}
```

`addresses` and `not_imported` count what S7 says the ingester counts. `url`, `retrieved_at`, `bytes`, `blake3` and `sha256` of an artifact are copied from the retrieval record that stands beside the archives, `retrieval.json`:

```json
{
  "schema": "fidryn.retrieval/v0.1",
  "release": "119-142",
  "archives": [
    { "file": "xml_usc01@119-142.zip", "url": "https://...", "retrieved_at": "2026-11-16T22:14:03Z", "bytes": 42241, "sha256": "...", "blake3": "..." }
  ]
}
```

The ingester recomputes the size and both digests of every archive and refuses one that differs from its record, an archive with no record, and a directory with no `retrieval.json`. All three JSON files are written with keys in the order shown, two spaces of indentation, `modules` sorted by path, and a final line end. The examples show the keys and their order, not the line breaks: a file has one member to a line, every object and array that is not empty is spread over lines, and an empty one is written `[]` or `{}`.

### S12. What the checker checks

**One module** (`fidryn check FILE`, the documentation check, and the first phase of a workspace check). Every diagnostic carries the span of the text it is about.

| Check | Code |
| --- | --- |
| The module's address, and every address written in it, is an address by S2 | `E700` |
| The module has one of the three shapes of S6, and no declaration that belongs to another shape | `E100` |
| A unit's top provision has the module's address; every child provision's address is below its parent's; every note's address is below the module's; no address is declared twice | `E700` |
| The text of every block is normalized (S3) | `E710` |
| Every `hash` and every value of `layers` equals the value computed by S5 | `E710` |
| Every `mark` names a footnote of the module | `E710` |
| The checks of a definition listed in S10 | `E710` |
| In a curated module: no name is declared twice; a binding's two hashes are 64 hexadecimal digits | `E200`, `E710` |

**A workspace** (`fidryn check DIR`), in addition:

| Check | Code |
| --- | --- |
| The manifest and the lock are well formed; every module is below the prefix and at its path; in a generated workspace the files and the lock agree; no first-edition file | `E730` |
| A unit's or a container's `within` is a container of the workspace that lists it as a `member`, and every `member` is a module whose `within` is that container | `E720` |
| An `import` names a module of the workspace or of a dependency | `E720` |
| A name used in `binds` resolves: unqualified, to a citation of the module or to one brought into scope by an `import { ... }`, and to exactly one; qualified `alias.name`, to a citation of the module imported under that alias | `E200` |
| A dependency's lock has the digest and the release the manifest records | `E740` |
| The running toolchain's version equals the manifest's | `W760` |
| A binding's target exists and its two hashes equal those of the dependency (S13) | `W750`, never an error |

New diagnostic codes: `E700 InvalidAddress`, `E710 IntegrityMismatch`, `E720 UnresolvedModule`, `E730 WorkspaceViolation`, `E740 PinMismatch`, `W750 StaleBinding`, `W760 ToolchainMismatch`.

### S13. Interfaces, the parallel check, the cache, and staleness

- The **interface** of a module is a JSON document with schema `fidryn.interface/v0.1` that holds everything phase two, the ledger and the change sets read, so that none of them opens a module's source: its address, its shape, its aliases, `within`, `source`, `positive_law`; for a container its kind, status and members; each provision's address, kind, status and tree hash, and whether it holds an image; each note's address, layer, group and hash, and whether it holds an image; the number of footnotes; the three layer hashes; each reference's layer, the address of the provision or note it stands in, and its target; each definition's provision, terms, kind and basis; and for a curated module its imports, citations and bindings with the byte range of each. Its exact layout is the implementer's choice; it is deterministic, and it is versioned by the schema string. A task that needs a further field adds it there and to this list in the reference.
- The check runs in two phases. Phase one, for each module by itself: read, parse, lower, check as one module, build the interface. Phase two, with every interface of the workspace and of its dependencies in memory: the workspace checks of S12, the outcome of every reference (S9), and the staleness of every binding. Phase two reads no module source (goal 2: "Checking a module needs only the interfaces of the modules it references").
- Both phases run on `--jobs` worker threads (`std::thread::scope`; the default is the number the standard library reports as available). The diagnostics, the report and every written file are the same for every number of workers: results are sorted by module path and then by position before anything is printed.
- The **cache** holds the result of phase one. Its key is the BLAKE3 of the frames `"fidryn.cache/v1"`, the toolchain's version, and the module's bytes; its value is the module's interface and diagnostics. An entry is one file under `<cache>/v1/<first two digits>/<key>.json`, written to a temporary name and renamed. An entry that does not parse or whose recorded key differs is ignored and rewritten. The cache directory is `--cache-dir`, else `$FIDRYN_CACHE_DIR`, else `.fidryn-cache` inside the workspace; `--no-cache` turns it off. The report says how many modules were read from it.
- **Staleness.** A binding records the address it binds, the release, the tree hash of that provision and the enacted-layer hash of the unit the provision lies in. It is stale, with one of these reasons, when: the address is no longer a provision of the dependency ("provision removed or moved"); the provision's tree hash differs ("provision text changed"); the unit's enacted-layer hash differs ("enacted notes of the unit changed"). A change to editorial matter makes nothing stale (design 9.4). Notes set out under a container do not enter any unit's enacted layer; they are reported in change sets only.

### S14. Commands

| Command | Does |
| --- | --- |
| `fidryn check FILE` | as today for the first edition; the one-module check for an edition-2 file; prints `ok` |
| `fidryn check DIR [--jobs N] [--cache-dir DIR] [--no-cache] [--report FILE] [--unresolved FILE]` | the workspace check; prints the diagnostics, one summary line `modules: N; from cache: C; errors: E; warnings: W`, and `ok` when there is no error; exit 1 on an error |
| `fidryn ingest uscode ARCHIVE_DIR --out DIR [--title T]... [--jobs N] [--verify]` | writes a generated workspace from the archives of one release; with `--verify` writes nothing and reports every file under `DIR` that differs from what it would write |
| `fidryn text DIR [ADDRESS] [--json \| --hashes]` | prints the text of the provisions and notes of a workspace, or of one address and what lies below it; with `--json`, one text record (S15) to a line, in address order; with `--hashes` and an address, the two lines a binding to it records |
| `fidryn coverage DIR` | prints the coverage ledger of a workspace as JSON |
| `fidryn changes OLD_DIR NEW_DIR [--bindings DIR]... [--json]` | the change set between two generated workspaces of one prefix, and the bindings of the given curated workspaces that are stale against `NEW_DIR`. A binding is judged against `NEW_DIR` whatever release its workspace is pinned to: the command answers what would go stale if the pin moved there. |

`ingest` refuses to write into a directory that holds anything other than a previous output of itself with the same prefix. It writes every file to a temporary name and renames it, removes modules that the new output does not have, and writes the lock last.

### S15. Records: text, coverage, change sets

**A text record** (`fidryn.text-record/v0.1`) is what `fidryn text --json` prints for each provision and each note, and what `conformance/uslm_text.py` prints from the publisher's XML by the rules of S7, independently. Acceptance item 4 is the equality of the two outputs, line for line.

Each record is one line of compact JSON: no white space between tokens, keys in the order given here, every character outside ASCII written as itself, and the escapes that `serde_json::to_string` and Python's `json.dumps(record, ensure_ascii=False, separators=(",", ":"))` both write. The example is broken over several lines only to be read:

```json
{"address": "/zz/demo/t1/s8/a", "layer": "operative", "kind": "subsection", "status": null,
 "num": ["a", "(a)"], "heading": null,
 "items": [
   {"part": "content", "blocks": [
     {"p": "As used in section 2 of this title, the term “widget” means a small device.",
      "refs": [[11, 34, "/zz/demo/t1/s2"]], "sup": [], "sub": []}
   ]}
 ]}
```

A **text object** carries one text of the model (S4): `{"text": plain, "refs": [[start, end, target], ...], "sup": [[start, end], ...], "sub": [[start, end], ...]}`, followed by `"marks": [[offset, shown, footnote], ...]` when the text holds a footnote marker and not otherwise. Offsets count Unicode scalar values of the plain text; `end` is exclusive; the offset of a marker is the length of the plain text before it. Each list is in the order of the runs.

Where the texts stand:

- A paragraph is a text object with the key `p` in place of `text`; a subheading is one with the key `subheading`.
- `heading` is `null` or a text object.
- `num` is `null` when there is no number, and otherwise `[value, shown]`, where `shown` is the plain string when the shown text is words only and a text object when it holds a link, a superscript, a subscript or a marker.
- The other blocks: a table is `{"table": [[group, [[header, colspan, rowspan, [blocks]], ...]], ...]}` with one entry for each row, `group` one of `"head"`, `"body"` and `"foot"`, and `header` a boolean; an image is `{"image": source}`; quoted content is `{"quoted": {"origin": origin, "items": [...]}}` with `null` for no origin, where an item is a block, a part `{"part": role, "blocks": [...]}`, or a level `{"level": kind, "num": ..., "heading": ..., "items": [...]}`.

The records, each with its keys in order:

- A provision: `address`, `layer` (`"operative"`), `kind`, `status` (the word, or `null`), `num`, `heading`, `items`. An item is a part `{"part": role, "blocks": [...]}` or a child `{"child": address}`, at its place. The record of a unit's top provision has `aliases`, `within`, `source` and `positive_law` after these.
- A container: `address`, `layer` (`"operative"`), `kind`, `status`, `num`, `heading`, `aliases`, `within`, `members`.
- A note: `address`, `layer` (`"enacted"` or `"editorial"`), `group`, `topics`, `heading`, `blocks`.
- A credit: `address` (its module's), `record` (`"credit"`), then the keys of a paragraph.
- A footnote: `address` (its module's), `record` (`"footnote"`), `ordinal`, `shown`, `blocks`.

Records are sorted by the bytes of their address; for one address, the provision or container comes first, then the credit, then the footnotes by ordinal.

**The coverage ledger** (`fidryn.coverage/v0.1`), one file `coverage.json` per generated workspace, also printed by `fidryn coverage`:

```json
{
  "schema": "fidryn.coverage/v0.1",
  "prefix": "/us/usc",
  "release": "119-142",
  "totals": {
    "units": 0, "containers": 0, "provisions": 0,
    "units_by_kind": {}, "units_by_status": {}, "containers_by_kind": {},
    "addresses": { "aliases": 0, "disambiguated": 0, "assigned": 0 },
    "notes": { "enacted": 0, "editorial": 0 },
    "notes_by_group": {},
    "footnotes": 0,
    "with_images": 0,
    "references": {
      "operative": { "exact": 0, "target-status": 0, "ambiguous": 0, "unit-only": 0, "range-form": 0, "no-unit": 0, "outside": 0, "malformed": 0 },
      "enacted": {},
      "editorial": {}
    },
    "outside_by_body": {},
    "definitions": { "enacted": 0, "restated": 0, "unresolved": 0 },
    "definitions_by_kind": {},
    "not_imported": { "tables_of_contents": 0, "footnotes_in_tables_of_contents": 0, "links_in_tables_of_contents": 0, "release_notes": 0, "links_without_words": 0, "note_group_headings": 0 }
  },
  "by_source": [ { "source": "usc01.xml", "positive_law": true, "units": 0, "provisions": 0, "definitions": 0 } ]
}
```

`units` counts unit modules; `provisions` counts every provision of every unit, the unit's own included. "status" keys are the publisher's status words, with `in-force` for none; `with_images` counts the provisions and notes that hold an image, whose picture is not in the pinned archive. Nothing in the ledger comes from outside the workspace: the estimate of missed definitions (design 9.5) is a review record, shown beside the ledger and not inside it.

**A change set** (`fidryn.change-set/v0.1`):

```json
{
  "schema": "fidryn.change-set/v0.1",
  "prefix": "/us/usc",
  "old": { "release": "119-138", "lock_digest": "..." },
  "new": { "release": "119-142", "lock_digest": "..." },
  "operative": { "added": [], "removed": [], "amended_text": [], "amended_structure": [], "moved": [ { "from": "...", "to": "..." } ] },
  "enacted_notes": { "added": [], "removed": [], "amended": [], "moved": [] },
  "editorial": { "added": [], "removed": [], "amended": [], "moved": [], "credits_changed": [], "footnotes_changed": [] },
  "containers": { "added": [], "removed": [], "changed": [] },
  "stale_bindings": [ { "workspace": "...", "module": "...", "binding": "...", "address": "...", "reason": "provision text changed" } ]
}
```

- A provision is `amended_text` when the words of its own text parts and tables differ, and `amended_structure` when its number, heading, status, the order and addresses of its children, or the shape of a table differ; it may be both. A provision whose tree hash changed only because a descendant changed is not listed.
- `added` and `removed` list the highest added or removed provision only, not what lies below it.
- A removed and an added provision are one `moved` entry, and are taken out of the other two lists, when their tree hashes are equal once each one's own number is left out (a section that is renumbered and otherwise unchanged has moved). Notes are paired by equal note hashes. When several candidates match they are paired in address order.
- `credits_changed` lists the units and containers whose credit was added, removed or changed. `footnotes_changed` lists the units and containers whose footnotes or footnote markers differ in any way. `containers.changed` lists the containers present on both sides whose number, heading, status, aliases or members differ.
- Every list is sorted by address.
- The text form, printed without `--json` and used as a commit message, is: a first line `<prefix>: <old release> to <new release>`; a blank line; one line of counts for each of the four groups; then each non-empty list under its name, one address to a line.

### S16. Counting rules

`conformance/usc_profile.py` prints two profiles as one JSON object. Its key `baseline` holds the profile of the design and its key `import` the profile of this plan; tracked files call them by those two names and mention neither document.

**The profile of the design** (the rules of the script of 2026-11-13, which design 3.1 and 9.6 quote):

- Excluded: every `note`, `notes`, `quotedContent`, `toc` and `sourceCredit` element and everything inside one.
- A provision is a counted element: not excluded, with one of the nine tags `section`, `subsection`, `paragraph`, `subparagraph`, `clause`, `subclause`, `item`, `subitem`, `subsubitem`, and with a non-empty identifier. Elements are counted, not distinct identifiers.
- A section is a provision with the tag `section`; its status is the `status` attribute, or `operative` when it has none.
- A reference to the Code in operative text is a `ref` that is not excluded and whose `href` begins `/us/usc/`. It resolves exactly when its `href` is the identifier of a provision. Its section is the prefix matched by `^/us/usc/t[0-9A-Za-z]+/s[^/]+`; it reaches the section only when it does not resolve exactly and that prefix is the section prefix of some provision; otherwise it reaches no section.

**The profile of this plan**, by S7 to S9, with nothing shared with the Rust code: units by kind and by status; containers by kind; provisions (every provision of every unit, its top one included); notes by layer and by group; footnotes; images; references by layer and outcome, and those of outcome `outside` by body; aliases, disambiguated addresses and assigned addresses; tables of contents, the footnotes and links inside them, release notes, links without words and the group headings of notes, which are not imported.

The two differ where the design's profile leaves out what this plan imports: the 20 sections without an identifier; court rules, reorganization plans and the other units that are not sections, and their provisions; provisions of kinds other than the nine; and references whose target is a container or a unit that is not a section, which the design's profile counts as reaching no section.

## File Structure

New files, one responsibility each:

| Path | Responsibility |
| --- | --- |
| `crates/fidryn-provision/src/lib.rs` | The public surface of the provision model |
| `crates/fidryn-provision/src/address.rs` | Addresses: parse, compare, parent, containment |
| `crates/fidryn-provision/src/layout.rs` | The file path of a module, and back |
| `crates/fidryn-provision/src/normalize.rs` | The normalization rule |
| `crates/fidryn-provision/src/model.rs` | Texts, blocks, tables, provisions, notes, definitions, the three module shapes |
| `crates/fidryn-provision/src/hash.rs` | Frames, and the hashes of provisions, notes, containers and layers |
| `crates/fidryn-provision/src/render.rs` | The canonical text of a module |
| `crates/fidryn-provision/src/workspace.rs` | The manifest, the lock and the retrieval record as data |
| `crates/fidryn-provision/src/interface.rs` | Interface summaries |
| `crates/fidryn-provision/src/reference.rs` | The address index and the outcome of a reference |
| `crates/fidryn-provision/src/binding.rs` | Staleness |
| `crates/fidryn-provision/src/changes.rs` | Change sets |
| `crates/fidryn-provision/src/records.rs` | Text records and the coverage ledger |
| `crates/fidryn-ingest-uscode/src/lib.rs` | `ingest` and its options |
| `crates/fidryn-ingest-uscode/src/archive.rs` | Archives and the retrieval record; what is refused |
| `crates/fidryn-ingest-uscode/src/uslm.rs` | Reading the publisher's XML: namespaces, the element classes of S7 |
| `crates/fidryn-ingest-uscode/src/operative.rs` | Levels, text parts, tables and links into provisions |
| `crates/fidryn-ingest-uscode/src/containers.rs` | Big levels and units |
| `crates/fidryn-ingest-uscode/src/notes.rs` | Notes, groups, layers, note addresses, credits, footnotes |
| `crates/fidryn-ingest-uscode/src/definitions.rs` | The patterns of S10 |
| `crates/fidryn-syntax/src/edition.rs` | The marker |
| `crates/fidryn-syntax/src/e2/mod.rs`, `ast.rs`, `parser.rs` | The edition-2 parser |
| `crates/fidryn-check/src/e2/mod.rs`, `lower.rs`, `module_check.rs`, `names.rs` | Lowering, the one-module check, names |
| `crates/fidryn-driver/src/workspace/mod.rs`, `discover.rs`, `check.rs`, `writer.rs`, `index.rs`, `report.rs`, `cache.rs`, `deps.rs`, `changes.rs` | Workspaces |
| `crates/fidryn-cli/src/law.rs` | The commands `ingest`, `text`, `coverage`, `changes` |
| `samples/uscode/archives/` | The committed archives of the sample titles and their `retrieval.json` |
| `samples/uscode/workspace/` | The generated sample; never edited by hand |
| `samples/uscode/curated/` | A curated workspace that depends on the sample |
| `samples/uscode/README.md` | What the sample is, where it came from, how it is regenerated |
| `tools/fetch_uscode.py` | Downloads the archives of a release and writes the retrieval record |
| `tools/replay_uscode.py` | Replays releases as commits of the local law repository |
| `tools/draw_definition_samples.py` | Draws the two samples of the definition audit |
| `tools/check-law-repository.sh` | The integration test on this machine |
| `conformance/usc_profile.py`, `uslm_text.py`, `usc_compare.py`, `hash_vectors.py`, and a `test_*.py` for each | The independent cross-checks |
| `conformance/usc-counting-rules.md` | The counting rules behind the profile |
| `conformance/fixtures/uslm/` | Invented documents in the publisher's XML |
| `conformance/fixtures/hash/vectors.json` | Models and their digests |
| `schemas/workspace-v0.1.json`, `law-lock-v0.1.json`, `retrieval-v0.1.json`, `check-report-v0.1.json`, `text-record-v0.1.json`, `coverage-v0.1.json`, `change-set-v0.1.json` | Contracts for the new JSON files |
| `docs/law-modules.md` | The reference for this stage |
| `docs/coverage.md` | The coverage ledger of the sample |
| `xtask/src/uscode_sample.rs` | The CI step that regenerates and checks the sample |
| `xtask/tests/corpus_neutrality.rs` | No corpus is named outside the ingester |
| `.github/workflows/law-integration.yml`, `law-repository.pin.json` | The cross-repository job, written and off |

Changed files:

| Path | Change |
| --- | --- |
| `Cargo.toml`, `Cargo.lock`, `pyproject.toml`, `uv.lock`, `.gitattributes` | Two members, three Rust and two Python dependencies, `*.zip binary` (Task 1); the version (Task 23) |
| `crates/fidryn-core/src/diagnostics.rs` | Seven codes (Task 1) |
| `crates/fidryn-syntax/src/lib.rs` | Exports `edition` and `e2` |
| `crates/fidryn-check/src/lib.rs`, `Cargo.toml` | Exports `e2`; depends on `fidryn-provision` |
| `crates/fidryn-driver/src/lib.rs`, `Cargo.toml` | Exports `workspace`; depends on `fidryn-provision` |
| `crates/fidryn-cli/src/lib.rs`, `Cargo.toml` | The choice of front end; `check DIR`; the new commands |
| `xtask/src/ci.rs`, `main.rs`, `docs_check.rs`, `site/guides.rs` | The sample step; the marker in documentation samples; two guides |
| `conformance/check_json_contracts.py`, `conformance/README.md` | The new contracts; the new scripts |
| `LICENSING.md` | The section on the sample (Task 7, decision O-1) |
| `docs/*.md`, `README.md`, `CHANGELOG.md`, `site/` | As each task says |

Not changed: `crates/fidryn-syntax/src/parser.rs` and `lexer.rs`, `crates/fidryn-hir`, the evaluator, `grammar.ebnf` (it is the grammar of the first edition), the four roots of the examples ledger, `packages/`, `prelude/`, `.cargo/config.toml`.

## Tasks

Each task ends with the gate green on the tree it commits. "Tests" lists the tests a task must add, by name, with what each asserts; the implementer writes them first, sees them fail, then writes the code. A test that needs a fixture names it. Paths of new test files follow the habit of the crate they are in.

| Task | Needs | Task | Needs | Task | Needs |
| --- | --- | --- | --- | --- | --- |
| 1 | nothing | 8 | 4b, 7 | 16 | 15 |
| 2 | 1 | 9 | 8 | 17 | 12 |
| 3 | 1 | 10 | 9, 11 | 18 | 14, 17 |
| 4 | 3 | 11 | 4b, 6 | 19 | 18 |
| 4b | 4 | 12 | 11 | 20 | 14, 19 |
| 5 | 1 | 13 | 12 | 21 | 16, 20 |
| 6 | 4, 5 | 14 | 10, 13 | 22 | 21 |
| 7 | 2 | 15 | 14 | 23 | 22 |

So after Task 1 three lanes run side by side: Task 2 and then 7; Tasks 3, 4 and 4b; Task 5. Tasks 8 and 9 run beside Tasks 6 and 11 to 13; Task 17 may run beside Tasks 14 to 16.

### Task 1: Scaffold: crates, dependencies, diagnostic codes

Everything that more than one lane would otherwise edit is done here, once.

**Files:**
- Create: `crates/fidryn-provision/Cargo.toml`, `crates/fidryn-provision/src/lib.rs`
- Create: `crates/fidryn-ingest-uscode/Cargo.toml`, `crates/fidryn-ingest-uscode/src/lib.rs`
- Create: `xtask/tests/corpus_neutrality.rs`
- Modify: `Cargo.toml`, `Cargo.lock`, `pyproject.toml`, `uv.lock`, `.gitattributes`
- Modify: `crates/fidryn-core/src/diagnostics.rs`, and every file that lists the diagnostic codes (find them with `git grep -n W620`)
- Modify: `docs/ARCHITECTURE.md` (the crate list and its direction rule, the code list)

**Interfaces:**
- Produces: workspace members `fidryn-provision` (depends on `blake3`, `serde`, `serde_json`, `unicode-normalization`; on no other crate of the workspace) and `fidryn-ingest-uscode` (depends on `fidryn-provision`, `zip`, `flate2`, `roxmltree`, `blake3`, `sha2`, `serde`, `serde_json`, `thiserror`). Both inherit version, edition, license and authors from the workspace.
- Produces: workspace dependencies `unicode-normalization`, `roxmltree`, and `zip` with `default-features = false` and the one feature that reads and writes deflated archives through `flate2`, whose pure-Rust backend a workspace entry for `flate2` selects, each written with its full version number as the others are; `sha2`, which `xtask` already used and which the ingester needs for the SHA-256 of an archive (S11), becomes a workspace dependency; Python dependencies `defusedxml` and `blake3`, and `types-defusedxml` in the `dev` group.
- Produces: `DiagnosticCode::{E700, E710, E720, E730, E740, W750, W760}` with the titles `InvalidAddress`, `IntegrityMismatch`, `UnresolvedModule`, `WorkspaceViolation`, `PinMismatch`, `StaleBinding`, `ToolchainMismatch`; the two `W` codes have `Severity::Warning`.
- Produces: `.gitattributes` line `*.zip binary`.

**Tests:**
- `new_codes_have_their_names_titles_and_severities` (`crates/fidryn-core`): for each of the seven, `as_str()`, `title()` and `severity()` as above.
- `the_code_list_of_the_architecture_guide_is_the_enum` if no such test exists: every code of the enum appears once in the list of `docs/ARCHITECTURE.md` with its title.
- `corpus_neutral_code_names_no_corpus` (`xtask/tests/corpus_neutrality.rs`): reads every `.rs` file under the listed paths; fails, naming file and line, when a line holds `/us/`, or one of the words `usc`, `uscode`, `uslm` (a word is a maximal run of ASCII letters and digits, compared without regard to case); fails when a listed path does not exist. The list starts as `crates/fidryn-provision/src`. Later tasks add to it: `crates/fidryn-syntax/src/edition.rs` and `crates/fidryn-syntax/src/e2` (Task 5), `crates/fidryn-check/src/e2` (Task 6), `crates/fidryn-driver/src/workspace` (Task 11).

**Steps:**
- [ ] Write the three tests; run them and see the first two fail.
- [ ] Add the crates, the dependencies and the codes. Fetch once with the network (`cargo fetch --locked` after the lock is written; `uv lock`), then build offline.
- [ ] `cargo deny --offline check licenses bans sources` passes; if a new crate's license is not in `deny.toml`, stop and report instead of adding it.
- [ ] Gate. Commit: `Add the provision and ingest crates, their dependencies and the workspace diagnostic codes`

### Task 2: The counting rules, the independent profile and the independent text

This lane is Python only and touches nothing outside `conformance/`.

**Files:**
- Create: `conformance/usc_profile.py`, `conformance/uslm_text.py`, `conformance/uslm_common.py` (reading archives and the element classes of S7, shared by the two)
- Create: `conformance/test_usc_profile.py`, `conformance/test_uslm_text.py`
- Create: `conformance/fixtures/uslm/*.xml` and, beside each, `<name>.records.jsonl` and `<name>.profile.json`
- Create: `conformance/usc-counting-rules.md`
- Modify: `conformance/README.md`

**Interfaces:**
- Produces: `uv run --locked python conformance/usc_profile.py ARCHIVE_DIR_OR_XML [--out FILE]`, which prints the profile of S16 as JSON with sorted keys. Both scripts read every archive of a directory whether or not it holds a retrieval record, and refuse a member name that two archives share.
- Produces: `uv run --locked python conformance/uslm_text.py ARCHIVE_DIR_OR_XML`, which prints one text record of S15 to a line for every provision and every note, in the byte order of their addresses, reading the XML by the rules of S7 and S8 and normalizing by S3. It shares no code with the Rust ingester and is written from this plan alone.
- Produces: the fixtures. Each is one invented document in the publisher's XML, in the invented title 99 and the invented release `999-1`, whose first line is a comment that says it is invented. Together they hold one case of every rule of S7 and S8; the list is in the Tests below. The expected records and profile of each fixture are written by hand from S7, S8 and S16, not by running the scripts, and Tasks 8 and 9 hold the ingester to the same files. One value cannot be written by hand: the hash that names a note with neither a law nor a heading (S8). The test computes it from the frames of S5 written out in the test, byte by byte, and not by the script's own function; Task 9 computes the same value in Rust.
- Produces: `conformance/usc-counting-rules.md`: the rules of S16, and the profile of release 119-142 as measured by this script, with the command that produced it.
- Both scripts read archives with `zipfile` and XML with `defusedxml`, refuse what S7 says the ingester refuses, pass `mypy --strict` and `ruff`, and never write beside their input.

**Tests** (`test_uslm_text.py` and `test_usc_profile.py` run each fixture and compare with its expected file; one fixture for each line):
- a section with a heading, a chapeau, three subsections and paragraphs below one of them;
- `continuation` after the last child, `continuation` between two children, and `continuation` inside a `content`;
- a `proviso`, as a child of a level and inside a `content`;
- a `content` of text only, of several `p`, of text and then a `p`, and of a `p` that holds a table;
- a link to the Code, a link to a public law, a link whose words are only white space, a footnote marker with its footnote, a second marker of the same footnote, a marker between two spaces, a marker at the start of a heading, and one in a number;
- a superscript, a subscript, a bare superscript digit beside a footnote of the same number (it stays a superscript, F7a), the numerator of a fraction, and typography that is dropped (bold, italic, small capitals, a date, a line break);
- a table in operative text with a header row, a footer row, a cell that spans two columns, a cell that spans two rows, an empty cell, and cells whose text stands in one and in two `xhtml:p`;
- an image in a unit and one in a note;
- each status value of F7 on a section, a status on a paragraph and on a chapter, and a section with none;
- a court rule, a reorganization plan, and a generic level that is a unit;
- every container kind of F5, nested as the Code nests them, with a generic level that is a container;
- addresses: two sections with one identifier; two paragraphs of a section with one identifier; a section whose identifier lists two; a section whose identifier is a range of whole numbers (three aliases) and one whose ends carry letters (two); a container, a section and a small level without an identifier; a section inside a section; identifiers without a title segment; two files of one release that each hold a unit with one identifier (the fixture `two-files`, with the members `usc99.xml` and `usc99a.xml`, which the test packs into one archive: the unit of the second file ends in `~2`);
- a file with no units;
- notes in each of the four groups and before any group, among the last a removal description, a revision note and one of another topic; a note directly under a section and one under a paragraph; a note under a chapter; the release note under a title; a table of contents with a footnote and a link;
- a note that quotes a section with subsections and a bracketed note inside the quotation; a note with a table; a note with a signature; a note with a second heading;
- two notes that cite the same law with different headings; two with the same law and the same heading; a note with a heading and no law; a note with a law and no heading; a note with neither; a note whose law comes from the origin of its quotation;
- a source credit with links and a date;
- characters of Review Focus 5: U+00A0 inside a number, U+202F, U+2000, a composing sequence, U+200B, "[sic]".
- `the_scripts_refuse_what_the_rules_refuse`: each refusal S7 lists, one document for each.
- `text_records_are_sorted_and_match_the_schema_shape`.

**Steps:**
- [ ] Write the fixtures and their expected files by hand from S7, S8 and S16.
- [ ] Write the tests of the text; see them fail; write `uslm_common.py` and `uslm_text.py`. Gate. Commit: `Add the independent text of the Code's official XML and its fixtures`
- [ ] Write the tests of the profile; see them fail; write `usc_profile.py`.
- [ ] Run the profile on the full release: `lease run --cpus 8 --mem 16G --time 2h -- uv run --locked python conformance/usc_profile.py /library/corpora/uscode/119-142`. It must give the numbers of F1. Write them into `conformance/usc-counting-rules.md`.
- [ ] Gate. Commit: `Add the independent profile of the Code and its counting rules`

### Task 3: Addresses, normalization and file paths

**Files:**
- Create: `crates/fidryn-provision/src/address.rs`, `normalize.rs`, `layout.rs`
- Modify: `crates/fidryn-provision/src/lib.rs`
- Create: `docs/law-modules.md` with the sections "Addresses", "File paths" and "Normalization" (the rules of S2 and S3)
- Modify: `docs/README.md` (one line that links the new reference)

**Interfaces:**
- Produces: `Address`, `AddressError`, `layout::module_path`, `layout::address_of_path`, `LayoutError`, `normalize::{WHITE_SPACE, normalize, is_normalized}`, exactly as S2 and S3 give them.

**Tests** (fixtures use the corpus `/zz/demo`):
- `parse_accepts_the_forms_the_publisher_uses`: `/zz/demo/t29/s2611/2/B/ii`, `/zz/demo/t42/s1395i–3` (an en dash), `/zz/demo/t28a/courtRules/Civil/rule71.1`, `/zz/demo/t2/s44...46`, `/zz/demo/rule1001`, and the forms an ingester adds: `/zz/demo/t28/s1932~2`, `/zz/demo/t5a/~compiledAct.1/~section.2`, `/zz/demo/a-b`.
- `parse_rejects_what_is_no_address`: `""` is `Empty`; `zz/demo` is `NoLeadingSlash`; `/zz//demo` is `EmptySegment { index: 1 }`; `/zz/demo/` is `EmptySegment { index: 2 }`; `/zz/./a` and `/zz/../a` are `DotSegment { index: 1 }`; `/zz/a b` is `Character { index: 1, character: ' ' }`; `/zz/a_b` and `/zz/a%20b` are `Character` at their `_` and `%`.
- `addresses_that_differ_only_by_case_are_distinct`: `/zz/demo/t1/s8/a` and `/zz/demo/t1/s8/A` are unequal, and order by their bytes.
- `parent_child_and_is_within`: the parent of `/zz/demo/t1/s8/a` is `/zz/demo/t1/s8`; `/zz` has none; `/zz/demo/t1/s80` is not within `/zz/demo/t1/s8`; an address is within itself.
- `normalize_follows_the_table`: each row below.

| Raw | Normalized |
| --- | --- |
| two spaces, `a`, two spaces, `b`, one space | `a b` |
| `a`, U+00A0, `b` | `a b` |
| `§`, U+202F, `1` | `§ 1` |
| `Current`, U+2000, `through` | `Current through` |
| `a`, tab, `b`, line feed, `c`, carriage return, line feed, `d` | `a b c d` |
| `e`, U+0301 | U+00E9 |
| `1`, U+00A0, `000` | `1 000` |
| U+3000, `a`, U+1680 | `a` |
| `a`, U+200B, `b` | unchanged |
| U+FEFF, `a` | unchanged |
| `[sic]` | unchanged |
| `“x” — y` | unchanged |

- `the_white_space_list_is_the_unicode_property`: for every `char`, membership in `WHITE_SPACE` equals `char::is_whitespace`.
- `normalize_is_idempotent`: over 2,000 strings from a seeded generator that draws from letters, the 25 characters, combining marks and U+200B.
- `module_path_follows_the_examples`: `/zz/demo/t29/s2611` is `zz/demo/t29/s2611.fr`; `/zz/demo/t42/s1395i–3` is `zz/demo/t42/s1395i-3.fr`; `/zz/demo/a-b` is `zz/demo/a%2Db.fr`; `/zz/demo/t2/s44...46` is `zz/demo/t2/s44...46.fr`; `/zz/demo/a.` is `zz/demo/a%2E.fr`; `/zz/demo/t5/ptIII/sptE` is `zz/demo/t5/pt_i_i_i/spt_e.fr`; `/zz/demo/t28a/courtRules/Civil/rule6` is `zz/demo/t28a/court_rules/_civil/rule6.fr`; `/zz/demo/t28/s1932~2` is `zz/demo/t28/s1932~2.fr`; `/zz/demo/con` is `zz/demo/%63on.fr`; `/zz/demo/con.1` is `zz/demo/%63on.1.fr`; `/zz/demo/lpt1` is `zz/demo/%6Cpt1.fr`; `/zz/demo/.a` is `zz/demo/%2Ea.fr`; `/zz/demo/a.fr/b` is `zz/demo/a%2Efr/b.fr`; `/zz/demo/a..fr` is `zz/demo/a.%2Efr.fr`.
- `a_module_file_is_never_a_directory_of_another_module`: the paths of `/zz/demo/a` and `/zz/demo/a.fr/b` are `zz/demo/a.fr` and `zz/demo/a%2Efr/b.fr`; and over the 2,000 generated addresses, which must include segments that begin with a full stop and segments that end in `.fr`, no path segment begins with a full stop, and no path segment but the last ends in `.fr`.
- `paths_of_addresses_that_differ_only_by_case_are_distinct`: `/zz/demo/ch6A` and `/zz/demo/ch6a` give paths that differ when both are lower-cased.
- `layout_is_reversible_for_every_segment_form`: `address_of_path(module_path(a)) == a` over 2,000 generated addresses and over every form of F3; `address_of_path` refuses `zz/demo/CH1.fr`, `zz/demo/a_.fr`, `zz/demo/a%2d.fr`, `zz/demo/a%5F.fr`, `zz/demo/a.txt`, `zz\demo\a.fr` and `zz/demo/con.fr`, and reads `zz/demo/a-b.fr` as the address with an en dash.

**Steps:**
- [ ] Write the tests; run `lease run --cpus 16 --mem 25G --time 1h -- cargo test -p fidryn-provision --locked --offline`; see them fail.
- [ ] Implement the three files. NFC through `unicode-normalization`.
- [ ] Write the three sections of `docs/law-modules.md`.
- [ ] Gate. Commit: `Add addresses, the normalization rule and the file path of a module`

### Task 4: The provision model and its hashes

**Files:**
- Create: `crates/fidryn-provision/src/model.rs`, `hash.rs`, `records.rs`
- Create: `conformance/hash_vectors.py`, `conformance/test_hash_vectors.py`, `conformance/fixtures/hash/vectors.json`
- Modify: `crates/fidryn-provision/src/lib.rs`, `docs/law-modules.md` (sections "Provisions, notes and layers" and "Hashes", from S4 and S5)

**Interfaces:**
- Consumes: `Address`, `normalize` (Task 3).
- Produces: every type of S4, with `serde` derives; `Text::from_runs(Vec<Run>) -> Text` (merges adjacent `Words`, drops empty runs) and `Text::plain`; `Hash32` with `Display` and `FromStr` in 64 lower-case hexadecimal digits; the functions of S5; `records::text_records(module: &Module) -> Vec<TextRecord>`, where a `TextRecord` serializes to one text record of S15 with its keys in the order shown there.
- Produces: `conformance/hash_vectors.py`, a second implementation of S5 in Python (`blake3` package) that reads the models in `vectors.json` and, with `--write`, fills in their digests; without it, verifies them. `vectors.json` holds at least: a provision with text only; one with a link, a superscript, a footnote marker, a number that holds a footnote marker, an image and a table with a spanning cell and an empty cell; a three-level tree; a note with quoted content; a unit with two enacted notes, one editorial note, a credit and a footnote, with its three layer hashes.

**Tests:**
- `known_answers_match_the_independent_implementation` (Rust): every digest in `vectors.json` equals the Rust value. `test_hash_vectors.py`: the same file verifies in Python.
- `a_change_anywhere_changes_every_ancestor_and_nothing_else`: in a tree of a root, two children and two grandchildren under the first child, changing one grandchild's word changes the tree hash of that grandchild, its parent and the root, and of no other node. Repeated for a change to: `num.value`, `num.shown`, the heading, the status, a link's target, a link's extent over the same words, a superscript made of the same letters, a table cell's text, a `colspan`, a part's role, and the order of two children.
- `a_footnote_marker_is_outside_the_operative_hash`: adding a `Mark` run, moving it to another provision, and changing a footnote's text each change `editorial` and neither any provision's hash nor `enacted`.
- `a_superscript_changes_the_hash`: `Words("x2")` and `Words("x"), Sup("2")` hash differently.
- `fields_cannot_be_confused`: `num` `("ab", "c")` and `("a", "bc")` hash differently; an absent heading and an empty one hash differently.
- `note_order_does_not_change_a_layer_hash`, and `moving_a_note_changes_its_layer_and_not_its_hash`.
- `adjacent_words_merge`: `Text::from_runs` of `Words("a")`, `Words("b")` equals that of `Words("ab")`.
- `text_records_follow_S15`: the unit of S6 gives the record S15 shows for its subsection and one record for each provision and note in address order; a paragraph in which a link follows a curly quotation mark has its offsets counted in Unicode scalar values, not bytes.

**Steps:**
- [ ] Write `vectors.json` models and the Python implementation; `uv run --locked python conformance/hash_vectors.py --write`; commit the digests it writes.
- [ ] Write the Rust tests; see them fail; implement `model.rs` and `hash.rs`.
- [ ] Add the two sections to the reference.
- [ ] Gate. Commit: `Add the provision model and the hashes of provisions, notes and layers`

### Task 4b: The workspace file formats as data

The ingester (Task 8) and the workspace check (Task 11) both read these files and are built side by side, so the formats come before either.

**Files:**
- Create: `crates/fidryn-provision/src/workspace.rs`
- Modify: `crates/fidryn-provision/src/lib.rs`

**Interfaces:**
- Consumes: `Address`, `Hash32` (Tasks 3 and 4).
- Produces, in `fidryn_provision::workspace`: `Manifest`, `WorkspaceKind`, `Dependency`, `Lock`, `Generator`, `Artifact`, `Member`, `AddressCounts`, `NotImported`, `RetrievalRecord`, `RetrievedArchive`, with the fields of S11, all public. The lock's `addresses` is an `AddressCounts { aliases, disambiguated, assigned }` and its `not_imported` a `NotImported` with the six counts of S11; every count is a `u64`, and both types have `Default` and `fn add(&mut self, other: &Self)`.
- Produces: for each of the three files, `from_json(&str) -> Result<Self, String>`, which refuses an unknown field, a missing field and a wrong `schema` and names the field, and `to_json(&self) -> String` in the layout S11 fixes. The three schema strings are the constants `MANIFEST_SCHEMA`, `LOCK_SCHEMA` and `RETRIEVAL_SCHEMA`.
- Produces: `Lock::digest(text: &str) -> Hash32`, the BLAKE3 of the bytes of a lock file, which a dependency entry records.

**Tests** (names, digests and the URL are invented; the URL names no real host):
- `each_file_round_trips_byte_for_byte`: the three examples of S11, written out in the test with digests of 64 digits, give `to_json(from_json(text)) == text`.
- `an_unknown_field_a_missing_field_and_a_wrong_schema_are_refused_and_named`: for each of the three files, and for one nested object of each (a dependency, an artifact, a member, an archive).
- `modules_are_written_sorted_by_path`: two locks that differ only in the order their modules were added write the same bytes.
- `kind_is_generated_or_curated`: any other word is refused.
- `the_lock_digest_is_the_blake3_of_its_bytes`.

**Steps:**
- [ ] Tests first; implement. No reader document changes here: Task 11 writes the section "Workspaces".
- [ ] Gate. Commit: `Add the manifest, the lock and the retrieval record as data`

### Task 5: The edition marker and the edition-2 parser

**Files:**
- Create: `crates/fidryn-syntax/src/edition.rs`, `crates/fidryn-syntax/src/e2/mod.rs`, `e2/ast.rs`, `e2/parser.rs`, and their tests
- Modify: `crates/fidryn-syntax/src/lib.rs` (exports only), `xtask/tests/corpus_neutrality.rs` (two paths)

**Interfaces:**
- Produces: `fidryn_syntax::edition::{Edition, edition_of}` as S1 gives them.
- Produces: `fidryn_syntax::e2::parse(source: &str) -> e2::Parse`, with `pub struct Parse { pub module: Option<ast::Module>, pub diagnostics: Vec<Diagnostic> }`. The syntax tree mirrors the grammar of S6 one type to a production; every node and every string has a `Span`; strings are decoded; integers are `u32`. The parser stops at the first error, so `diagnostics` holds none or one, and `module` is `Some` exactly when it holds none.
- Produces: `e2::E2_MAX_SOURCE_BYTES`, `e2::E2_MAX_TOKENS` (values of F6) and `e2::MAX_NESTING: usize = 64`.
- No file of the present parser, and no file outside `crates/fidryn-syntax`, changes in this task.

**Tests:**
- `edition_of_follows_the_table`: `edition 2` before `module` is `Two`; the same after a comment line and blank lines is `Two`; a first-edition module, the empty string and a file of comments only are `One`; `edition 3`, `edition "2"`, `edition 2.0` and `edition` alone are `Err` with `E100`, a message that begins "unknown edition", and the span of the token after `edition` (of `edition` itself for the last).
- `the_example_of_the_specification_parses`: the unit of S6 gives a module with one provision that has one child, one credit, one definition and the layers.
- `a_container_and_a_curated_module_parse`: one of each, with every field of the grammar, two `alias` lines in the container among them.
- `every_run_form_parses`: words, a link, `sup`, `sub`, `mark`.
- `tables_images_and_quoted_content_parse`: a table with a head row and a body row with `span 2 1`, an empty cell and a cell in brace form; an `image` block; quoted content with an origin, a `level` two deep and a paragraph.
- `an_unknown_field_is_an_error_at_its_word`: for each of `provision`, `container`, `note`, `table`, a row, a cell, `quoted`, `level`, `definition`, `layers`, `binding` and the module itself, a block with the word `color` in it is `E100` with a message that names the word and the block, and the span of `color`.
- `a_field_out_of_order_or_twice_is_an_error`: `heading` before `num`; two `heading` fields.
- `a_missing_required_field_is_an_error`: a provision with no `hash`; a note with no `group`; a definition with no `term`; a binding with no `pending`.
- `text_needs_a_run`: `content }` is `E100` "expected text".
- `strings_are_json_strings`: `"\"\\é"` decodes to the three characters; `"\q"` and a string with a raw line end are `E100`.
- `text_after_the_module_is_an_error`.
- `limits_are_enforced`: a source one byte over `E2_MAX_SOURCE_BYTES`, and provisions nested 65 deep, are `E100` "module exceeds resource limits".
- `a_module_of_the_size_of_the_largest_unit_parses_within_the_limits`: a generated unit with as many provisions and as much text as F6 records for the largest unit, doubled, parses without a diagnostic.
- `no_input_makes_the_parser_panic`: the example cut at every character boundary, and with each single token removed, returns without panicking.

**Steps:**
- [ ] Write the tests; see them fail; implement `edition.rs`, then the parser.
- [ ] Gate. Commit: `Read the edition marker and parse edition 2 modules into typed syntax`

### Task 6: Rendering, lowering, the one-module check, and the choice of front end

This task joins the model lane and the front end lane.

**Files:**
- Create: `crates/fidryn-provision/src/render.rs`
- Create: `crates/fidryn-check/src/e2/mod.rs`, `e2/lower.rs`, `e2/module_check.rs`, and golden fixtures under `crates/fidryn-check/tests/fixtures/e2/`
- Modify: `crates/fidryn-check/Cargo.toml`, `crates/fidryn-check/src/lib.rs` (one `pub mod e2;`)
- Modify: `crates/fidryn-cli/src/lib.rs` (the choices of S1), `xtask/src/docs_check.rs`, `xtask/tests/corpus_neutrality.rs`
- Modify: `docs/law-modules.md` (sections "Edition 2", "Syntax", "What the checker verifies in one module"), `docs/cli.md` (`check`), `CHANGELOG.md`

**Interfaces:**
- Consumes: the model and hashes (Task 4), `e2::parse` (Task 5).
- Produces: `fidryn_provision::render::render(module: &Module) -> String`: the canonical form of S6 for a unit and a container, with every hash computed; a plain, parseable form for a curated module.
- Produces, in `fidryn_check::e2`:

```rust
pub struct Lowered { pub module: fidryn_provision::Module /* and private tables: declared hashes, spans by address */ }
pub fn lower(parse: &fidryn_syntax::e2::Parse) -> Result<Lowered, Vec<Diagnostic>>;
pub fn check_module(lowered: &Lowered) -> Vec<Diagnostic>;                 // the one-module checks of S12
pub fn check_source(source: &str) -> Result<Lowered, Vec<Diagnostic>>;    // edition_of, parse, lower, check_module
```

- Produces: the behavior of S1 at every entry point. `fidryn_cli::compile_source` and `compile_module` return the `E100` of S1 for an edition-2 source; `fidryn check FILE` prints `ok` for an edition-2 file that passes; the documentation check treats a block that begins with `edition 2` as a complete module.

**Tests:**
- `render_of_the_golden_models_is_the_golden_text`: three models built in the test (the unit of S6 with a note, a footnote and a table added; a container; a curated module) render to the committed files `unit.fr`, `container.fr`, `curated.fr`, and `check_source` of each file returns no diagnostic.
- `rendering_then_reading_gives_the_model_back`: for 500 units and 100 containers from a seeded generator (every run form, parts with one and with several blocks, tables with spans and empty cells, images, quoted levels, notes of both layers, footnotes, definitions, aliases, statuses, absent numbers and headings, addresses with every character of S2, children eight deep), `lower(parse(render(m))).module == m` and `check_module` is empty.
- `render_is_deterministic`: two calls give equal strings.
- `each_defect_is_reported_with_its_code_at_its_text`: each row is a change to `unit.fr` and the diagnostic it must give, with a primary span inside the changed text or on the `hash` it invalidates:

| Change | Code |
| --- | --- |
| one letter of a `content` string | `E710` at that provision's `hash` |
| one digit of a `hash` | `E710` |
| two spaces between two words of a string | `E710` "text is not normalized" |
| a child's address moved outside its parent's | `E700` |
| one address written twice | `E700` |
| the top provision's address not the module's | `E700` |
| a note's address outside the module's | `E700` |
| `mark "1" 2` where there is no footnote 2 | `E710` |
| one digit of a value of `layers` | `E710` |
| a definition's term that is not in the provision's text | `E710` |
| a definition that names a provision of another module | `E710` |
| an `import` added to the unit | `E100` |
| a second top `provision` | `E100` |

  and for `curated.fr`: a `within` added is `E100`; a citation name declared twice is `E200`; a `provision_hash` of 63 digits is `E710`.
- `every_diagnostic_of_the_one_module_check_has_a_span`: over all the rows above.
- `check_accepts_an_edition_2_file_and_refuses_to_run_one`: through the built binary, as `crates/fidryn-cli/tests/temporal_records.rs` runs it: `check unit.fr` prints `ok`; `check` of a changed copy exits 1 and names the file; `run`, `verify`, `fmt` and `render` of `unit.fr` exit 1 with the message of S1; `check examples/california/abc-test/abc-test.fr`, a module of the first edition, still prints `ok`.
- `the_documentation_check_reads_edition_2_blocks`: a Markdown text with a passing block and a failing block that begin with `edition 2` reports the second with its line.

**Steps:**
- [ ] Write the tests and the generator; see them fail.
- [ ] Implement `render`, then `lower`, then `check_module`, then the choices in the command line and the documentation check.
- [ ] Write the reference sections; the module shown in the reference is `unit.fr` itself, so that the documentation check proves it.
- [ ] `cargo xtask examples` reports no change. Gate. Commit: `Render, lower and check edition 2 modules and choose the front end by the marker`

### Task 7: The pinned sample and the tool that fetches a release

Needs decision O-1 for the `LICENSING.md` section; with no answer, the default text below is used and listed for the owner.

**Files:**
- Create: `tools/fetch_uscode.py`, `conformance/test_fetch_uscode.py`
- Create: `samples/uscode/archives/` with the archives of F11 and `retrieval.json`; `samples/uscode/README.md`
- Create: `xtask/src/uscode_sample.rs` (step 1 of Task 14 only)
- Modify: `xtask/src/main.rs`, `xtask/src/ci.rs`, `pyproject.toml` (`tools` joins the paths `mypy` checks), `LICENSING.md`, `.gitattributes` (`samples/uscode/workspace/** linguist-generated=true`)

**Interfaces:**
- Produces: `uv run --locked python tools/fetch_uscode.py RELEASE --out DIR (--all | --title T ...)`, which downloads from `https://uscode.house.gov/download/releasepoints/us/pl/<congress>/<law part>/xml_usc<NN>@<release>.zip` (or `xml_uscAll@<release>.zip` with `--all`), one request at a time, with the user agent `fidryn-fetch/<version>`, and writes or extends `DIR/retrieval.json` (S11). It refuses to replace an archive whose bytes differ from the record, and it is the only program in the repository that opens a connection.
- Produces: the sample archives, exactly as served, and their record. F11 names the titles: the smallest set of whole titles that, with Title 1, holds a table in operative text, continuation text, court rules, notes with quoted provisions, and a reference that does not resolve (design 9.3).
- Consumes: `usc_profile.py` (Task 2).
- Produces: `samples/uscode/profile.json`, the output of `usc_profile.py` on the sample archives: the independent counts that Tasks 8, 9 and 14 hold the ingester to.
- Produces: `cargo xtask uscode-sample`, which for now verifies the size and both digests of every archive against the record and that `profile.json` is what the script prints today, and is run by `cargo xtask ci`.
- Produces: this section of `LICENSING.md`, placed with the other material the MIT License does not cover:

```markdown
## United States Code sample

The archives under `samples/uscode/archives/` were published by the Office
of the Law Revision Counsel of the United States House of Representatives
and are kept unchanged. The files under `samples/uscode/workspace/` are
generated from them. Their text is a work of the United States Government,
for which copyright protection is not available under 17 U.S.C. § 105(a).
The MIT License does not apply to that text. It applies to the programs
that read and generate these files.
```

**Tests:**
- `test_fetch_uscode.py`, with a fake opener and no network: the URL of `119-142` title `1`, of title `5a`, of `119-73not60` title `11a`, and of `--all`; the record that is written; a second run that changes nothing; an archive whose bytes changed is refused.
- `the_sample_step_names_an_archive_that_differs_from_its_record`.
- `xtask/tests/licensing.rs` and `repo_hygiene.rs` pass with the new files; if either forbids them, change the test only where the owner's decision O-1 requires it and say so in the report.

**Steps:**
- [ ] Tests first; write the tool; fetch the sample titles with it; write `samples/uscode/profile.json` with `usc_profile.py`.
- [ ] `samples/uscode/README.md`: what the directory holds, the release, the URL of each archive and when it was retrieved, and the command that regenerates the workspace.
- [ ] Gate. Commit: `Pin a sample of the United States Code and add the tool that fetches a release`

### Task 8: The ingester: archives and the operative layer

**Files:**
- Create: `crates/fidryn-ingest-uscode/src/archive.rs`, `uslm.rs`, `operative.rs`, `containers.rs`, and their tests
- Modify: `crates/fidryn-ingest-uscode/src/lib.rs`

**Interfaces:**
- Consumes: the model (Task 4), the file formats with `Artifact`, `RetrievalRecord`, `AddressCounts` and `NotImported` (Task 4b), the fixtures and their expected records (Task 2), the sample archives (Task 7).
- Produces:

```rust
pub struct ReleaseArchives { /* the verified archives of one release and their record */ }
pub fn open(archive_dir: &Path) -> Result<ReleaseArchives, IngestError>;
impl ReleaseArchives {
    pub fn release(&self) -> &str;
    pub fn artifacts(&self) -> Vec<fidryn_provision::workspace::Artifact>;
    pub fn members(&self) -> Vec<String>;            // file names, sorted
}
pub struct IngestOptions { pub titles: Vec<String>, pub jobs: usize }
pub struct IngestSummary { pub units: u64, pub containers: u64, pub addresses: AddressCounts, pub not_imported: NotImported }   // the two count types of `fidryn_provision::workspace`, as the lock holds them
pub fn ingest(
    archives: &ReleaseArchives,
    options: &IngestOptions,
    sink: &(dyn Fn(fidryn_provision::Module) -> Result<(), IngestError> + Sync),
) -> Result<IngestSummary, IngestError>;
pub fn ingest_xml(name: &str, xml: &str, release: &str) -> Result<(Vec<fidryn_provision::Module>, NotImported), IngestError>;   // one document; what the tests call
```

- In this task a unit has its provision tree and nothing else; notes, credits and footnotes are read and set aside for Task 9. The rules are S7. `ingest` reads one member at a time on each of `jobs` threads and holds at most `jobs` documents in memory.
- `IngestError` names the archive, the member, and where it applies the element and its identifier or the identifier of its nearest ancestor that has one.

**Tests:**
- `open` refuses, each with an error that names the file: a directory with no `retrieval.json`; an archive whose size or digest differs from the record; an archive the record does not list; a member whose name is `../x.xml`, `dir/x.xml` or `x.txt`; a member larger than the limit of S7; two archives that hold a member of the same name.
- `ingest_xml` refuses: a document type declaration; an entity declaration; a root element in another namespace; a schema location that is not in the list of S7; a release name in the metadata that differs from the record's; an element that is in no class of S7 (`an_element_the_ingester_does_not_know_is_refused_and_named`).
- For each structural fixture of Task 2, the text records of the ingested modules (`fidryn_provision::records::text_records`) equal the fixture's expected records for the operative layer: the section with subsections; both placements of `continuation` (`text_between_two_children_keeps_its_place`); the `proviso`; several paragraphs; links, a link whose words are white space, superscript, subscript and dropped typography; the table (`table_cells_keep_their_spans_and_empty_cells`); every status; a court rule and each other unit kind; every container kind; the level without an identifier; the file with no units; the characters of Review Focus 5.
- Addresses, by the five rules of S7: `a_repeated_identifier_keeps_both_elements_at_addresses_of_their_own` (two sections, and two paragraphs of one section; the second of each ends in `~2`, and what stands inside the second section is below its new address); `a_repeated_identifier_in_two_members_is_numbered_in_the_order_of_their_names` (the fixture `two-files` of Task 2, through `ingest` with one and with eight workers); `an_identifier_that_lists_several_gives_one_address_and_aliases`; `a_range_identifier_gives_its_ends_and_the_whole_numbers_between_them_as_aliases`, with a range whose ends carry letters, which gives the two ends only; `an_element_without_an_identifier_gets_an_assigned_segment` (a container, a unit inside it, a small level); `a_section_inside_a_section_is_a_provision_of_the_outer_unit`; `an_alias_that_is_another_elements_address_is_dropped`; `identifiers_without_a_title_segment_are_kept_as_they_are`.
- `an_image_is_a_block_that_names_its_file`, and `a_bare_superscript_is_never_a_footnote_marker`.
- `containers_list_their_members_in_document_order_and_units_name_their_container`.
- `the_modules_are_the_same_for_one_and_eight_workers`.
- `the_sample_archives_ingest`: every archive of `samples/uscode/archives/` ingests, and the counts of units by kind and by status, of containers by kind and of provisions are those of `samples/uscode/profile.json` (Task 7).

**Steps:**
- [ ] Tests first; implement `archive.rs`, then `uslm.rs`, then the two builders.
- [ ] Gate. Commit: `Read the official archives of the Code into provisions and containers`

### Task 9: The ingester: notes, credits and editorial matter

**Files:**
- Create: `crates/fidryn-ingest-uscode/src/notes.rs` and its tests
- Modify: `crates/fidryn-ingest-uscode/src/operative.rs`, `containers.rs`, `lib.rs`

**Interfaces:**
- Consumes: Task 8.
- Produces: units and containers complete by S4: `notes` with their layer, group, topics, heading, blocks and address by S8; `credit`; `footnotes` with their ordinals and the `Mark` runs that point at them; quoted content as structured text with no address; `NotImported` counted by S7.

**Tests** (fixtures and expected records of Task 2):
- `the_group_of_a_note_is_the_heading_it_follows`: notes after each of the four group headings of F8, and before any, have the groups and layers S8 gives; a cross heading is no note.
- `an_ungrouped_removal_description_or_revision_note_is_editorial_and_any_other_is_enacted`.
- `a_note_directly_under_a_level_is_anchored_there`: under a section and under a paragraph; its address begins with the paragraph's.
- `a_note_under_a_container_belongs_to_the_container`.
- Addresses: `a_note_is_named_by_its_group_law_and_heading`; `the_law_of_a_note_is_its_quoted_origin_before_its_first_link`; `two_notes_with_one_law_and_one_heading_get_ordinals_in_document_order`; `a_note_with_no_law_is_named_nolaw_and_its_heading`; `a_note_with_a_law_and_no_heading_is_named_noheading`; `a_note_with_neither_is_named_by_its_hash`; `a_long_heading_is_cut_to_sixty_characters`; `inserting_a_note_changes_no_other_notes_address_unless_they_agree_in_group_law_and_heading`.
- `the_release_note_and_the_tables_of_contents_are_counted_and_not_imported`, with a footnote inside a table of contents.
- `a_source_credit_keeps_its_links`.
- `a_footnote_in_operative_words_is_editorial_matter`: the marker is a `Mark` run, the footnote is in `footnotes`, and the provision's hash is that of the same text without them.
- `a_quoted_section_keeps_its_structure_and_has_no_address`, with a bracketed note inside it; `a_note_with_a_table`; `a_note_with_a_signature_and_a_second_heading`.
- `the_three_layer_hashes_follow_S5`: an enacted note changes `enacted` only; a credit, an editorial note and a footnote change `editorial` only.
- For every fixture of Task 2, the full text records equal the expected file.
- `the_sample_archives_ingest_with_their_notes`: the counts of notes by layer and by group, of footnotes and of images are those of `samples/uscode/profile.json`.

**Steps:**
- [ ] Tests first; implement.
- [ ] Gate. Commit: `Read notes, source credits and footnotes into their layers`

### Task 10: The ingester's output and the `ingest` command

**Files:**
- Create: `crates/fidryn-cli/src/law.rs`, fixture pairs `conformance/fixtures/uslm/reissue/{old,new}.xml`
- Modify: `crates/fidryn-cli/src/lib.rs`, `crates/fidryn-cli/Cargo.toml` (depends on `fidryn-ingest-uscode`)
- Modify: `docs/law-modules.md` (section "The ingester for the United States Code": what it reads, the classes of S7, what it refuses, what it does not import), `docs/cli.md`, `CHANGELOG.md`

**Interfaces:**
- Consumes: `ingest` (Tasks 8 and 9), `GeneratedWriter` (Task 11), the workspace check (Task 11).
- Produces: `fidryn ingest uscode ARCHIVE_DIR --out DIR [--title T]... [--jobs N] [--verify]` as S14 gives it. The manifest it writes has `name` `uscode`, `kind` `generated`, `prefix` `/us/usc`, the record's release, and the running version as `toolchain`. `--title` takes the publisher's title designations (`1`, `5a`) and writes only those titles; the lock then lists only their artifacts. With `--verify` it writes into memory, compares with `DIR`, prints each path that is missing, extra or different, and exits 1 if there is one.
- The command prints one line when it is done: `ingested: U units, C containers from A archives (release R)`.

**Tests:**
- `ingest_writes_a_workspace_that_checks`: on the fixtures of Task 2 packed into an archive by the test, `fidryn check DIR` exits 0, and `fidryn check` of every single module file exits 0.
- `two_runs_give_the_same_bytes`, with `--jobs 1` and `--jobs 8`, into two directories.
- `two_files_that_differ_only_in_ids_and_release_name_give_identical_modules`: `reissue/old.xml` and `new.xml` differ in every element id, the publication name, the creation time and the release note; every module is the same byte for byte, and the two locks differ only in `release` and `artifacts`.
- `the_lock_records_the_archives_and_every_module`: every field of S11, with the retrieval time and URL of the record and no other time.
- `verify_reports_a_changed_a_missing_and_an_extra_file_and_writes_nothing`.
- `title_selects_titles`: with `--title 99a` only that file's modules are written.
- `ingest_refuses_a_directory_that_is_not_its_own`.
- `nothing_of_the_host_reaches_the_output`: the output of a run with another working directory, another `TZ` and another `HOME` is the same bytes.

**Steps:**
- [ ] Tests first; implement the command.
- [ ] Reference section, command reference, changelog. A command that writes files is shown in a block that the documentation check does not run, by that check's own convention for such blocks. Gate. Commit: `Write a checked workspace from a release of the Code with one command`

### Task 11: Workspaces: the manifest, the lock, and `fidryn check DIR`

**Files:**
- Create: `crates/fidryn-driver/src/workspace/mod.rs`, `discover.rs`, `check.rs`, `writer.rs`, and their tests
- Create: `schemas/workspace-v0.1.json`, `schemas/law-lock-v0.1.json`, `schemas/retrieval-v0.1.json`
- Modify: `crates/fidryn-driver/Cargo.toml`, `crates/fidryn-driver/src/lib.rs` (one `pub mod workspace;`), `crates/fidryn-cli/src/lib.rs` (`check` of a directory), `conformance/check_json_contracts.py`, `xtask/tests/corpus_neutrality.rs`
- Modify: `docs/law-modules.md` (section "Workspaces"), `docs/cli.md`, `CHANGELOG.md`

**Interfaces:**
- Consumes: `check_source` and `Lowered` (Task 6), `render`, `module_path`, and the file formats of `fidryn_provision::workspace` (Task 4b).
- Produces, in `fidryn_driver::workspace`:

```rust
pub struct CheckOptions { pub jobs: usize, pub cache_dir: Option<PathBuf>, pub toolchain: String }
pub struct WorkspaceReport { pub modules: usize, pub from_cache: usize, pub diagnostics: Vec<Diagnostic> /* Tasks 12 and 13 add fields */ }
pub fn check_workspace(root: &Path, options: &CheckOptions) -> WorkspaceReport;

pub struct GeneratedWriter { /* private */ }
impl GeneratedWriter {
    pub fn create(out: &Path, manifest: Manifest) -> Result<GeneratedWriter, WriteError>;
    pub fn write(&self, module: &Module) -> Result<(), WriteError>;     // callable from several threads at once
    pub fn finish(self, generator: Generator, artifacts: Vec<Artifact>, addresses: AddressCounts, not_imported: NotImported) -> Result<Lock, WriteError>;
}
```

  `create` refuses a directory that holds anything but a generated workspace of the same prefix. `write` renders a unit or a container and writes it to its path through a temporary name, and fails when the run has already written a module to that path, so that no module silently replaces another. `finish` removes every `.fr` file the run did not write, then writes `workspace.json`, then `law.lock.json`.
- In this task the check is: discovery, the one-module check of every file, and the `E730`, `E720` (hierarchy) and `W760` checks of S12. References, imports and bindings come in Tasks 12 and 17 to 19.

**Tests** (every workspace is built in a temporary directory by a helper that makes demo models and calls `GeneratedWriter`):
- `a_workspace_from_the_writer_checks_clean`: one title container, one chapter container and three units give `modules: 5; from cache: 0; errors: 0; warnings: 0` and `ok` on the command line, exit 0.
- `writing_twice_gives_the_same_bytes`: the same models written to two directories, once from one thread and once from eight, give equal files and equal locks.
- `the_writer_removes_what_is_gone_and_refuses_a_foreign_directory`: a second run without one unit removes its file and its lock entry; `create` fails on a directory that holds `notes.txt` and on a generated workspace of another prefix, and changes nothing in it.
- `each_layout_defect_is_E730_and_names_the_file`: a module changed after it was written; a listed module deleted; a module file that the lock does not list; in a curated workspace, a module that is not at `module_path` of its address, and one whose address is outside the prefix; a first-edition `.fr` file; a manifest with an unknown field; a generated workspace with no lock; a curated workspace with one.
- `hierarchy_defects_are_E720`: a `within` that names no module; a unit its container does not list; a member that is no module; a member whose `within` is another container.
- `a_directory_whose_name_begins_with_a_dot_is_not_read`.
- `a_toolchain_mismatch_is_a_warning`: `W760`, exit 0.
- `diagnostics_are_sorted_by_path_then_position_and_name_their_file`.
- `a_directory_with_no_manifest_is_refused`: `E730` that says no `workspace.json` was found in it.
- `the_three_files_match_their_schemas` (`conformance/`): what the writer wrote validates.

**Steps:**
- [ ] Write the helper and the tests; see them fail.
- [ ] Implement the data formats, the writer, discovery and the check; wire `fidryn check DIR`.
- [ ] Reference section, command reference, changelog. Gate. Commit: `Check a workspace of edition 2 modules against its manifest and lock`

### Task 12: Interfaces, reference outcomes and the parallel check

**Files:**
- Create: `crates/fidryn-provision/src/interface.rs`, `reference.rs`
- Create: `crates/fidryn-driver/src/workspace/index.rs`, `report.rs`; `schemas/check-report-v0.1.json`
- Modify: `crates/fidryn-driver/src/workspace/check.rs`, `crates/fidryn-cli/src/lib.rs` (`--jobs`, `--report`, `--unresolved`), `conformance/check_json_contracts.py`
- Modify: `docs/law-modules.md` (sections "References" and "How a workspace is checked"), `docs/cli.md`, `CHANGELOG.md`

**Interfaces:**
- Produces, in `fidryn_provision`:

```rust
pub struct Interface { /* the content S13 lists; spans of imports, citations and bindings included */ }
pub struct DeclSpans { /* the byte range of each import, citation and binding, in declaration order */ }
pub fn interface_of(module: &Module, spans: &DeclSpans) -> Interface;      // `Lowered` gains an accessor that supplies the spans
impl Interface { pub fn to_json(&self) -> String; pub fn from_json(text: &str) -> Result<Interface, String>; }

pub enum Outcome { Malformed, Ambiguous, Exact, TargetStatus(String), UnitOnly, RangeForm, NoUnit, Outside(String) }
pub struct AddressIndex { /* private */ }
impl AddressIndex {
    pub fn build<'a>(interfaces: impl IntoIterator<Item = &'a Interface>, prefixes: &[Address]) -> AddressIndex;
    pub fn outcome(&self, target: &str) -> Outcome;
    pub fn provision_hash(&self, address: &Address) -> Option<Hash32>;        // the tree hash, for Task 19
    pub fn unit_enacted_hash(&self, address: &Address) -> Option<Hash32>;     // of the unit the address lies in
}
```

- Produces: `check_workspace` in two phases as S13 describes, on `jobs` threads; `WorkspaceReport` gains `references: BTreeMap<Layer, BTreeMap<String, u64>>` and `outside_by_body: BTreeMap<String, u64>`; `--report FILE` writes it as `fidryn.check-report/v0.1`; `--unresolved FILE` writes one object `{"from": ..., "layer": ..., "target": ..., "outcome": ...}` to a line for every reference whose outcome is not `exact`, sorted by `from`, then `target`.
- Produces: `fn check_interfaces(interfaces: &[Interface], dependencies: &[Interface], manifest: &Manifest, toolchain: &str) -> PhaseTwo`, the whole of phase two as a function of interfaces alone.

**Tests:**
- `an_interface_round_trips_and_is_deterministic`.
- `each_outcome_from_a_fixture`: with the units `/zz/demo/t1/s1` (child `a`), `/zz/demo/t1/s2` (status `repealed`), `/zz/demo/t1/s3` (status `omitted`, alias `/zz/demo/t1/s4`), `/zz/demo/t1/s5` (child `a`) and `/zz/demo/t1/s5~2`, and the containers `/zz/demo/t1` and `/zz/demo/t1/ch1`, prefix `/zz/demo`:

| Target | Outcome |
| --- | --- |
| `/zz/demo/t1/s1/a`, `/zz/demo/t1/s1`, `/zz/demo/t1/ch1` | `Exact` |
| `/zz/demo/t1/s2` | `TargetStatus("repealed")` |
| `/zz/demo/t1/s4` (the alias) | `TargetStatus("omitted")` |
| `/zz/demo/t1/s5`, `/zz/demo/t1/s5/a` | `Ambiguous` |
| `/zz/demo/t1/s1/z`, `/zz/demo/t1/s2/a` | `UnitOnly` |
| `/zz/demo/t1/s1–9`, `/zz/demo/t1/s1–9/b` (an en dash; `s1–9` is no unit) | `RangeForm` |
| `/zz/demo/t1/s9`, `/zz/demo/t9/s1`, `/zz/demo/t1/s8–9` | `NoUnit` |
| `/zz/other/x` | `Outside("/zz/other")` |
| `/yy/stat/116/926` | `Outside("/yy/stat")` |
| `not an address`, `/zz/demo/t1/s1/a ` with a space at its end | `Malformed` |

- `an_unresolved_reference_is_never_a_diagnostic`: a workspace whose every reference is `NoUnit` or `Outside` has no diagnostic and exit 0, and its report counts them.
- `references_are_counted_by_layer`: one in operative text, one in an enacted note, one in a credit.
- `the_report_and_the_output_are_the_same_for_one_and_eight_workers`: a generated workspace of 200 modules with errors seeded in five of them.
- `phase_two_is_a_function_of_interfaces`: `check_interfaces` over interfaces built in memory gives the hierarchy diagnostics and the outcome counts of the same workspace on disk.
- `the_unresolved_list_is_sorted_and_complete`.
- `the_report_matches_its_schema` (`conformance/`).

**Steps:**
- [ ] Tests first. Implement the interface, the index, the two phases and the report.
- [ ] Reference sections, command reference, changelog. Gate. Commit: `Give every reference an outcome and check a workspace in parallel from interfaces`

### Task 13: The disk cache

**Files:**
- Create: `crates/fidryn-driver/src/workspace/cache.rs`
- Modify: `crates/fidryn-driver/src/workspace/check.rs`, `crates/fidryn-cli/src/lib.rs` (`--cache-dir`, `--no-cache`)
- Modify: `docs/law-modules.md` (section "The cache"), `docs/cli.md`, `CHANGELOG.md`

**Interfaces:**
- Produces: the cache of S13. `CheckOptions.cache_dir: Option<PathBuf>` is the resolved directory, `None` for no cache; the command line resolves `--cache-dir`, then `FIDRYN_CACHE_DIR`, then `<DIR>/.fidryn-cache`.

**Tests:**
- `a_second_check_reads_every_module_from_the_cache`: `from_cache` equals `modules`, and diagnostics and counts equal the first run's.
- `editing_one_module_rereads_only_it`: in a curated workspace of five modules, `from_cache` is 4 after one is edited.
- `diagnostics_come_back_from_the_cache`: a module with a one-module error reports it again, with the same span, when read from the cache.
- `a_damaged_entry_is_ignored_and_rewritten`: an entry cut short, an entry that is `{}`, and an entry copied from another key each give the result of a run without a cache, and are valid afterwards.
- `the_key_depends_on_the_toolchain_version`: no entry written under `0.3.0` is read under `0.3.1`.
- `no_cache_writes_nothing`, and `the_cache_directory_is_chosen_in_the_stated_order`.
- `two_checks_at_once_leave_a_valid_cache`: two threads check one workspace with one cache directory; both reports equal the uncached one, and every entry reads back.

**Steps:**
- [ ] Tests first; implement; reference, command reference, changelog. Gate. Commit: `Keep the result of checking each module in a cache keyed by its content`

### Task 14: `fidryn text`, the sample in CI, and the independent text comparison

**Files:**
- Create: `xtask/src/uscode_sample.rs` (extends the archive check of Task 7), `samples/uscode/workspace/**` (generated), `schemas/text-record-v0.1.json`
- Modify: `crates/fidryn-cli/src/law.rs` (`text`), `xtask/src/ci.rs`, `xtask/src/main.rs`, `xtask/src/docs_check.rs` (add `samples` to the Markdown roots), `conformance/check_json_contracts.py`
- Modify: `samples/uscode/README.md`, `docs/law-modules.md` (section "The sample"), `docs/cli.md`, `CHANGELOG.md`

**Interfaces:**
- Consumes: `fidryn ingest uscode` (Task 10), the workspace check (Tasks 11 to 13), `uslm_text.py` and `usc_profile.py` (Task 2), `records::text_records` (Task 4).
- Produces: `fidryn text DIR [ADDRESS] [--json]` as S14 gives it. Without `--json` it prints, for each provision, its address on one line and its number, heading and text parts below it, indented by depth; notes after the provisions of their unit.
- Produces: `cargo xtask uscode-sample [--update]`, run by `cargo xtask ci`, which fails with a message that names the first difference when any of these does not hold:
  1. every archive under `samples/uscode/archives/` has the size and digests of `retrieval.json`;
  2. `fidryn ingest uscode samples/uscode/archives --out <temporary directory>` gives, byte for byte, the tree committed at `samples/uscode/workspace/` (`--update` replaces the committed tree instead);
  3. a second ingest with `--jobs 1` gives the same bytes as one with `--jobs 8`;
  4. `fidryn check samples/uscode/workspace --no-cache` exits 0 with no error;
  5. `samples/uscode/profile.json` is what `usc_profile.py` prints for the archives, and its counts of references by layer and outcome equal those of the check report (Task 16 adds the counts the ledger holds: units, containers, provisions, notes, footnotes and images);
  6. the output of `uslm_text.py` on the archives equals the output of `fidryn text samples/uscode/workspace --json`, line for line (design 9.6, item 4, on the sample).

**Tests:**
- `text_prints_one_record_for_each_provision_and_note_in_address_order`, on the demo workspace; the records validate against the schema.
- `text_of_one_address_prints_it_and_what_lies_below_it`; an address that is not in the workspace exits 1 and says so.
- `the_sample_step_fails_on_a_hand_edit`: with a copy of the sample in which one letter of one generated module is changed, step 2 fails and names the file; with one byte of an archive changed, step 1 fails and names the archive.
- The step itself, in the gate.

**Steps:**
- [ ] Tests first; implement `text`; implement the six steps.
- [ ] `cargo xtask uscode-sample --update`; read the diff of what it generated before committing it: every file under `samples/uscode/workspace/` must begin with `edition 2`, and nothing outside that directory may change.
- [ ] If step 5 or step 6 fails on real text, the ingester or the script disagrees with S7. Find which one departs from S7 and correct it; if S7 itself is silent or wrong for the case, stop and report it with the element and the two readings.
- [ ] Reference section, sample README, command reference, changelog. Gate. Commit: `Generate the sample of the Code and check it in CI against the publisher's text`

### Task 15: Definitions

**Files:**
- Create: `crates/fidryn-ingest-uscode/src/definitions.rs`, fixtures `conformance/fixtures/uslm/definitions-*.xml`
- Modify: `crates/fidryn-ingest-uscode/src/operative.rs` or `lib.rs` (call the extraction once a unit is built), `samples/uscode/workspace/**` (regenerated)
- Modify: `docs/law-modules.md` (section "Definitions", with the patterns of S10 word for word), `CHANGELOG.md`

**Interfaces:**
- Consumes: units with their notes (Task 9), the definition checks of the one-module check (Task 6).
- Produces: `pub fn extract(unit: &UnitModule, containers: &[(Address, String)]) -> Vec<Definition>`, where `containers` are the unit's containers from the nearest outward with their kinds; the rules are S10 and nothing else.

**Tests** (each fixture is an invented section of title 99; the expected definitions are written out in the test):

| Paragraph | Expected |
| --- | --- |
| `As used in this section, the term “widget”, with respect to a gadget, means a small device.` | term `widget`; `means`; words `As used in this section`; region the unit; basis by the file's flag |
| `The terms “widget” and “gadget” mean small devices.` | two terms; `means`; words unstated; no region; `unresolved` |
| `For purposes of this chapter, the term “widget” includes a gadget.` | `includes`; region the chapter container |
| `The term “widget” does not include a gadget.` | `does_not_include` |
| `The term “widget” has the meaning given such term in ` a link to `/us/usc/t99/s2` | `borrowed`; `borrowed_from` the link's target |
| `The term “widget” has the meaning given such term in the Widget Act.` (no link) | not extracted |
| a chapeau `In this chapter—` with children `“widget” means a small device;` and `“gadget” includes a widget.` | two definitions, each with the words `In this chapter` and the chapter as region |
| the same children under a chapeau `The following rules apply:` | not extracted |
| `In determining the meaning of any Act of Congress, the words “person” and “whoever” include corporations.` | not extracted: the text before `the words` is no reach |
| `For purposes of this Act, the term “widget” means a small device.` | words `For purposes of this Act`; no region; `unresolved` |
| `As used in ` a link to `/us/usc/t99/s2` `, the term “widget” means a small device.` | region the link's target |
| `As used in ` a link to `/us/pl/99/1` `, the term “widget” means a small device.` | words recorded; no region; `unresolved` |
| `In the case of a gadget, the term “widget” means a small device.` | not extracted: after `In` stands neither `this` nor a link |
| `The term “widget” means, and the term “gadget” includes, a device.` | not extracted |
| a paragraph with the sentence form in its second sentence | not extracted |
| a paragraph inside a note, and one inside quoted content | not extracted |

- `basis_follows_the_files_flag`: the same fixture with the positive-law flag on and off gives `enacted` and `restated`.
- `the_references_in_text_note_is_linked`: a `restated` definition in a unit whose editorial note headed "References in Text" contains "This chapter, referred to in" records that note's address.
- `every_extracted_definition_passes_the_checker`: each fixture, ingested and rendered, passes the one-module check.
- On the sample: `cargo xtask uscode-sample --update` adds `definition` blocks; the implementer reads every one against its text (the sample is small) and lists in the report any that looks wrong, with the pattern that produced it.

**Steps:**
- [ ] Tests and fixtures first; implement; regenerate the sample and read its definitions.
- [ ] Reference section and changelog. Gate. Commit: `Extract definitions by the closed patterns and record their reach`

### Task 16: The coverage ledger and its page

**Files:**
- Create: `schemas/coverage-v0.1.json`, `docs/coverage.md`, `samples/uscode/workspace/coverage.json`
- Modify: `crates/fidryn-provision/src/records.rs` (the ledger), `crates/fidryn-driver/src/workspace/report.rs`, `crates/fidryn-cli/src/law.rs` (`coverage`, and `ingest` writes `coverage.json`), `xtask/src/uscode_sample.rs`, `xtask/src/site/guides.rs` and whatever generates `docs/coverage.md`'s table, `conformance/check_json_contracts.py`
- Modify: `docs/law-modules.md` (section "The coverage ledger"), `docs/cli.md`, `CHANGELOG.md`

**Interfaces:**
- Consumes: interfaces and reference outcomes (Task 12), definitions (Task 15).
- Produces: `pub fn coverage(interfaces: &[Interface], index: &AddressIndex, manifest: &Manifest, lock: &Lock) -> Coverage` with the content of S15; `fidryn coverage DIR`; `ingest` writes `coverage.json` after the lock, and the lock does not list it. What only the ingester can count, `not_imported`, the ledger copies from the lock (S11).
- Produces: `docs/coverage.md`, whose table is generated from `samples/uscode/workspace/coverage.json` by `cargo xtask uscode-sample --update` between two marker comments, and checked by `cargo xtask uscode-sample`. Its prose says, by decision O-2: these are the figures of the sample in this repository, the titles it holds, and that the depth words mean imported, addressed and classified. It says that the closed patterns do not find every definition and that the share they miss has not been measured for the sample (design 9.5 wants that estimate beside the figures; Task 22 makes it for the full Code). It does not give figures for the whole Code.
- Produces: the page on the site, as a guide named "Coverage".

**Tests:**
- `the_ledger_counts_a_demo_workspace`: a workspace with known numbers of units by kind and status, provisions, notes by layer, references by layer and outcome, and definitions by basis and kind gives exactly those numbers.
- `the_ledger_is_deterministic_and_matches_its_schema`.
- `the_ledger_never_says_encoded`: no key and no sentence of the page contains the word.
- In `cargo xtask uscode-sample`: the committed `coverage.json` equals `fidryn coverage` of the sample, the page's table equals the file, and the ledger's counts equal `usc_profile.py`'s (step 5 of Task 14 now reads the ledger).

**Steps:**
- [ ] Tests first; implement; regenerate the sample; write the page; regenerate the site.
- [ ] Reference section, command reference, changelog. Gate. Commit: `Add the coverage ledger and show the sample's on the site`

### Task 17: Curated modules: imports, citations and names

**Files:**
- Create: `crates/fidryn-check/src/e2/names.rs`
- Modify: `crates/fidryn-driver/src/workspace/check.rs` (phase two), `crates/fidryn-provision/src/interface.rs` if a field is missing
- Modify: `docs/law-modules.md` (section "Curated modules: imports, citations and names"), `CHANGELOG.md`

**Interfaces:**
- Consumes: interfaces of curated modules with their imports, citations and bindings and the span of each (Task 12).
- Produces: `fidryn_check::e2::names::resolve(module: &Interface, imported: &dyn Fn(&Address) -> Option<&Interface>) -> (Vec<ResolvedBinding>, Vec<Diagnostic>)`, where a `ResolvedBinding` is a binding with its target as an address. The rules are those of S12: a citation is a named address; `binds` takes an address or a name; an unqualified name is a citation of the module itself or one named in an `import "..." { ... }`; `alias.name` is a citation of the module imported `as alias`; an imported module's own imports are not visible through it; two modules may import each other.

**Tests** (a curated workspace with prefix `/zz/notes` and modules `a`, `b`, `c`; `b` declares `citation widget -> "/zz/demo/t1/s8/a"`):
- `a_qualified_name_resolves_through_its_alias`: in `a`, `import "/zz/notes/b" as b` and `binds b.widget`.
- `a_name_brought_into_scope_resolves_unqualified`: `import "/zz/notes/b" { widget }` and `binds widget`.
- `a_local_citation_resolves_unqualified`.
- Each of these is `E200` with the span of the name: `an_unqualified_name_that_is_not_in_scope`; `an_alias_that_no_import_declares`; `a_name_the_imported_module_does_not_declare`, in `alias.name` and in the braces of an import; `one_name_from_two_sources` (a local citation and an imported one of the same name); `an_alias_declared_twice`; `a_name_reached_only_through_another_modules_import` (`c` imports `widget` from `b`; `a` writes `c.widget`).
- `an_import_of_a_module_that_does_not_exist_is_E720`, with the span of its address.
- `two_modules_may_import_each_other`: no diagnostic.

**Steps:**
- [ ] Tests first; implement; reference section and changelog. Gate. Commit: `Resolve the names of citations across curated modules`

### Task 18: Dependencies between workspaces

**Files:**
- Create: `crates/fidryn-driver/src/workspace/deps.rs`
- Create: `samples/uscode/curated/workspace.json` and one module under it that cites provisions of the sample
- Modify: `crates/fidryn-driver/src/workspace/check.rs`, `xtask/src/uscode_sample.rs` (also check the curated sample)
- Modify: `docs/law-modules.md` (section "Depending on another workspace"), `CHANGELOG.md`

**Interfaces:**
- Consumes: the generated sample of Task 14 for the committed curated sample; demo workspaces for the tests.
- Produces: loading of each dependency of a curated workspace: its manifest and lock are read from `path` (relative to the depending manifest; an absolute path is `E730`); the BLAKE3 of its lock and its `release` must equal `lock_digest` and `release` of the entry, else `E740` with a message that names both releases and both digests; its modules are verified against its lock and read through phase one (and the cache of the depending workspace); its prefix must not contain or lie within the prefix of the depending workspace or of another dependency (`E730`); if it has an error of its own, one `E740` says how many and nothing of the depending workspace's references into it is reported as resolved. Its interfaces join the address index.
- Nothing is ever written inside a dependency's directory.

**Tests:**
- `a_curated_workspace_cites_its_dependency_and_checks_clean`: a citation to `/zz/demo/t1/s8/a` has the outcome `Exact`; exit 0.
- `a_different_release_is_E740_and_names_both`: the manifest pinned to release `r1` and its digest, pointed at a copy regenerated as `r2` with one word changed: exit 1, and the message holds `r1`, `r2` and both digests.
- `a_missing_directory_and_a_dependency_without_a_lock_are_E740`.
- `overlapping_prefixes_and_an_absolute_path_are_E730`.
- `a_dependency_with_an_error_is_reported_once`.
- `references_into_a_dependency_get_outcomes`: `UnitOnly` and `NoUnit` are counted in the depending workspace's report.
- `checking_writes_nothing_into_the_dependency`: the listing and the bytes of the dependency's directory are the same before and after.
- In `cargo xtask uscode-sample`: the curated sample checks with no error against the committed generated sample.

**Steps:**
- [ ] Tests first; implement; write the curated sample; reference section and changelog. Gate. Commit: `Let a workspace depend on another at a pinned release`

### Task 19: Bindings and staleness

**Files:**
- Create: `crates/fidryn-provision/src/binding.rs`
- Modify: `crates/fidryn-driver/src/workspace/check.rs`, `crates/fidryn-cli/src/law.rs` (`fidryn text DIR ADDRESS --hashes`)
- Modify: `samples/uscode/curated/` (one binding to a provision of the sample)
- Modify: `docs/law-modules.md` (section "Bindings"), `docs/cli.md`, `CHANGELOG.md`

**Interfaces:**
- Consumes: resolved bindings (Task 17), the dependency's interfaces (Task 18).
- Produces: `pub enum StaleReason { Removed, TextChanged, EnactedNotesChanged }` and `pub fn staleness(address: &Address, provision_hash: Hash32, notes_hash: Hash32, index: &AddressIndex) -> Option<StaleReason>`, with the rule of S13 and its three messages; `W750` in phase two, whose message holds the binding's name, the address, the reason, the release the binding records and the release of the dependency.
- Produces: `fidryn text DIR ADDRESS --hashes`, which prints the two lines a binding needs, `provision_hash "<hex>"` and `notes_hash "<hex>"`.

**Tests** (a demo dependency in two versions, `v1` and `v2`, built in the test; the bindings are written against `v1` and checked against `v2`):
- `a_binding_to_a_changed_provision_is_stale`: `v2` changes one word of `/zz/demo/t1/s8/a`. The binding to that address and the one to `/zz/demo/t1/s8` are stale, "provision text changed"; the one to `/zz/demo/t1/s8/b` is not.
- `a_binding_in_a_unit_whose_enacted_notes_changed_is_stale`: `v2` adds an enacted note under the unit; every binding in the unit is stale, "enacted notes of the unit changed".
- `editorial_changes_make_nothing_stale`: `v2` changes the credit, an editorial note and a footnote.
- `a_binding_to_a_provision_that_is_gone_is_stale`: "provision removed or moved".
- `nothing_is_stale_when_nothing_changed`.
- `staleness_is_a_warning_and_says_what_a_reader_needs`: `W750`, exit 0, and the five things the message holds.
- `a_binding_by_name_is_checked_like_one_by_address`.
- `the_hashes_that_text_prints_make_a_binding_that_is_not_stale`.

**Steps:**
- [ ] Tests first; implement; add the binding to the curated sample; reference section, command reference, changelog. Gate. Commit: `Bind a curated declaration to a provision and report when its text has changed`

### Task 20: Change sets

**Files:**
- Create: `crates/fidryn-provision/src/changes.rs`, `crates/fidryn-driver/src/workspace/changes.rs`, `schemas/change-set-v0.1.json`
- Create: `conformance/usc_compare.py`, `conformance/test_usc_compare.py`, and pairs of fixtures `conformance/fixtures/uslm/pairs/<name>/{old,new}.xml`
- Modify: `crates/fidryn-cli/src/law.rs` (`changes`), `conformance/check_json_contracts.py`
- Modify: `xtask/src/ci.rs` (the step that runs the conformance tests gets `FIDRYN_BINARY`, as `run_schema_probes` in `xtask/src/workspace.rs` gives it to the probes; today that step does not have it)
- Modify: `docs/law-modules.md` (section "Change sets"), `docs/cli.md`, `CHANGELOG.md`

**Interfaces:**
- Consumes: interfaces of two generated workspaces (Task 12), units loaded on demand, `staleness` (Task 19), text records from `uslm_text.py` (Task 2).
- Produces: `fidryn_provision::changes::{ChangeSet, diff_units, pair_moved}` and `fidryn_driver::workspace::changes::change_set(old: &Path, new: &Path, bindings: &[PathBuf], options: &CheckOptions) -> Result<ChangeSet, Vec<Diagnostic>>` with the rules of S15: units are compared by layer hashes first, and only units whose hashes differ are read and compared provision by provision; a removed and an added provision are `moved` by the rule of S15, for which `changes::unnumbered_hash(provision: &Provision) -> Hash32` is the tree hash of S5 computed with the provision's own number absent.
- Produces: `fidryn changes OLD NEW [--bindings DIR]... [--json]`; the JSON form validates against the schema; the text form is the commit message of S15.
- Produces: `conformance/usc_compare.py OLD_ARCHIVE_OR_XML NEW_ARCHIVE_OR_XML`, which reads the text records of the two sides (S15) and nothing else, and prints these sets of addresses:
  - *operative*: from the records of provisions with their `marks` left out: only in the old, only in the new (each reduced to the highest provision), and on both sides with a different record;
  - *enacted notes* and *editorial notes*: the same three sets from the records of notes of each layer;
  - *credits*: the units and containers whose credit record is on one side only or differs;
  - *footnotes*: the units and containers whose footnote records, or whose `marks` anywhere in their provisions and notes, differ;
  - *containers*: only in the old, only in the new, and on both sides with a different record.
- `--against CHANGES.json` exits 1, naming the first address that differs, unless: operative old-only equals `operative.removed` with the `from` of `moved`, new-only equals `added` with the `to` of `moved`, and differing equals `amended_text` with `amended_structure`; the same holds for each note layer against `added`, `removed`, `moved` and `amended`; credits equals `editorial.credits_changed`; footnotes equals `editorial.footnotes_changed`; and the three container sets equal `containers.removed`, `added` and `changed`.

**Tests** (Rust; each is a pair of demo workspaces):
- One test for each kind, asserting the whole change set: a footnote marker added to a provision whose words do not change (`footnotes_changed` only); a container's heading changed (`containers.changed` only); a unit added; a unit removed; a provision added inside a unit, with children (only the highest is listed); a word changed (`amended_text` at that provision and no ancestor); a heading changed, a status added, two children swapped, a column added to a table, each `amended_structure`; a word and a heading changed in one provision (both lists); a section renumbered with its text unchanged (`moved`); a note added, removed, amended, and one whose cited law changed and whose text did not (`moved`); a credit changed; a footnote changed; a member added to a container.
- `a_reissued_title_has_an_empty_change_set`: two workspaces from the same models with different release labels.
- `stale_bindings_are_listed`: with `--bindings`, the stale bindings of Task 19's fixture and their reasons.
- `the_text_form_is_the_golden_commit_message`, and `every_list_is_sorted`.
- `the_change_set_matches_its_schema` (`conformance/`).

**Tests** (Python, and the two together):
- `test_usc_compare.py`: on each fixture pair, the three sets per layer are the expected ones.
- `the_command_and_the_independent_comparison_agree`: for each fixture pair (among them an added marker, a changed credit and a changed container heading), ingest both sides, run `fidryn changes --json`, and `usc_compare.py --against` exits 0. It runs with the conformance tests and fails, it does not skip, when `FIDRYN_BINARY` is not set.

**Steps:**
- [ ] Tests and fixture pairs first; implement the Rust side, then the Python side.
- [ ] Reference section, command reference, changelog. Gate. Commit: `Report what changed between two releases and which bindings went stale`

### Task 21: The full Code, measured

This task is run on this machine, through `lease`, on release 119-142. It changes the repository only where the full release shows a defect; each such change has a test and a fixture that reproduces it with invented text.

**Files:**
- Create: `.agents/docs/reviews/<date>-stage-1-full-code-run.md` (the record)
- Modify: whatever the run shows to be wrong, with tests

**Interfaces:**
- Consumes: every earlier task; the archive of 119-142. The directory `/library/corpora/uscode/119-142/` holds it with a record written by the measurement of 2026-11-16, which is not in the format of S11 and which the fetch tool does not extend: rename that directory to `119-142.measured`, fetch the release again with `tools/fetch_uscode.py 119-142 --all` into a new `/library/corpora/uscode/119-142/`, and confirm that the two archives are the same bytes. The same for `119-138` in Task 22.
- Produces: the generated workspace `/work/projects/law-local/uscode/` in a new local git repository `/work/projects/law-local/` (no remote), and the record.

**Steps, each recorded with its command line, its wall-clock time and its peak memory (`/usr/bin/time -v`):**
- [ ] Ingest: `lease run --cpus 16 --mem 16G --time 1h -- fidryn ingest uscode /library/corpora/uscode/119-142 --out /work/projects/law-local/uscode --jobs 16`. Then once more into a second directory; `diff -r` of the two is empty.
- [ ] Cold check, with an empty cache directory under `/cache/fidryn/`: `fidryn check /work/projects/law-local/uscode --jobs 16 --report ... --unresolved ...`. Bound: 10 minutes, 16 GB (design 9.6, item 9). It must exit 0 (item 1).
- [ ] The same command again, nothing changed. Bound: 30 seconds.
- [ ] Counts (item 2): `usc_profile.py` on the archives against `coverage.json`. The numbers to meet are those of "Measured facts", item F1. For every number that differs, the record says which rule of S16 explains it, with three example addresses. Totals of the two note layers are written beside them.
- [ ] References (item 3): the count of each outcome in each layer, the count by body for `outside`, and for each outcome other than `exact` and `outside` three examples from the unresolved list.
- [ ] Text (item 4): `uslm_text.py` over all archives against `fidryn text --json` of the workspace, compared a file of the release at a time; and `fidryn ingest uscode ... --verify` reports no difference.
- [ ] The largest unit, the deepest nesting, the number of modules, the total size of the workspace and of the cache, and the size of `law.lock.json`.
- [ ] `git init`, add the workspace, and commit it as the first commit of the law repository with the message `<prefix>: release 119-142` (signed, by decision O-5). This commit is replaced by the replay of Task 22.

**When a bound is missed:** measure where the time or memory goes before changing anything, change the one thing the measurement points to, and record the before and after. If the cold check cannot meet ten minutes or 16 GB after two such changes, stop and report the measurements; the design calls these "initial targets to confirm with a first spike", and changing a target is the owner's decision.

**When the ingester refuses a file or the text comparison fails:** it is a case S7 did not anticipate. Reproduce it in an invented fixture, decide by S7's stated principles (no operative word is dropped; nothing is invented; what is not imported is counted), add the fixture and its expected records to `conformance/fixtures/uslm/`, correct both the ingester and `uslm_text.py`, and list the case in the record. A case that needs a new kind of block or run, or a change to S5, stops the task for a ruling.

- [ ] Gate on the repository as changed. Commit: `Correct what the full release of the Code showed` (if anything changed), then the record in a commit of its own: `Record the first full import of the Code`.

### Task 22: Replay of the releases, the real pair, and the definition samples

Run on this machine. Nothing of the full Code enters this repository.

**Files:**
- Create: `tools/replay_uscode.py`, `tools/draw_definition_samples.py`, `conformance/test_replay_order.py`
- Create, in the law repository: `changes/<release>.json` for each release; `encodings/workspace.json` and one curated module with three bindings; `review/definitions/` with the two samples and `estimate.json`
- Modify: `.agents/docs/reviews/<date>-stage-1-full-code-run.md` (the second half of the record)

**Interfaces:**
- Consumes: `fidryn ingest`, `fidryn changes`, `usc_compare.py`, `tools/fetch_uscode.py`.
- Produces: `tools/replay_uscode.py --archives /library/corpora/uscode --repository /work/projects/law-local`, which holds the 27 release points of F9 in that order and for each one: fetches it if it is absent; ingests it into `uscode.next/`; for every release after the first runs `fidryn changes uscode uscode.next`, writes the JSON form to `changes/<release>.json` and keeps the text form; replaces `uscode/` with `uscode.next/`; commits with the text form as the message (`<prefix>: release <first>` for the first). It can be stopped and started again and continues after the last commit.
- Produces: `tools/draw_definition_samples.py --workspace DIR --seed N --out DIR`, which writes `extracted.jsonl` (500 definitions drawn without replacement from all extracted ones: address, terms, kind, words, region, basis, and the paragraph's text) and `near-misses.jsonl` (500 operative paragraphs drawn from those that hold a term in curly quotation marks within 60 characters before one of `mean`, `means`, `include`, `includes`, `shall mean`, `has the meaning` and from which nothing was extracted), each with the seed, the release and the count of the population it was drawn from, and beside each an empty verdict field.

**Steps:**
- [ ] `test_replay_order.py`: the list is the list of F9, entry for entry.
- [ ] Fetch the 27 archives (about 2.9 GB) into `/library/corpora/uscode/<release>/`, one request at a time.
- [ ] Replay, without `--bindings`: a curated workspace is pinned to one release, and judged against the 26 others it would only add false entries to their change sets. Each ingest and each `changes` run goes through `lease` as in Task 21. Design 9.6, item 8 is met when `git log` of the law repository shows 27 commits whose messages are the change sets.
- [ ] The pair of F10. Ingest 119-138 once more into a directory outside the law repository, `/work/projects/law-local-pair/119-138/`.
  - Item 5: `fidryn changes /work/projects/law-local-pair/119-138 /work/projects/law-local/uscode --json`, then `usc_compare.py` on the two archives with `--against` that file exits 0. Record the count of each list.
  - Write the curated workspace `encodings/` in the law repository: pinned to the lock of `uscode/` as it now stands (119-142), with one module of three bindings whose `release` is `119-138` and whose two hashes are what `fidryn text /work/projects/law-local-pair/119-138 ADDRESS --hashes` prints. Choose the three from the change set: one to a provision in `operative.amended_text`, one inside a unit whose only change is in `enacted_notes`, one inside a unit whose only change is in `editorial`.
  - Item 6: `fidryn check encodings` reports the first two stale (`W750`) with their reasons and not the third, and `fidryn changes ... --bindings encodings` lists the same two in `stale_bindings`.
  - Commit `encodings/` to the law repository: `encodings: three bindings written against 119-138`.
- [ ] Item 7: draw the two samples with a recorded seed into `review/definitions/<date>-sample-1/`. The reading is for the person decision O-3 names. Write `review/definitions/estimate.json` beside the ledger: the seed, the release, the sizes of the two populations, and either the verdicts' result (errors found in the first sample; definitions found among the near misses, and the share of the population that implies) or `"status": "drawn, not read"`. If the first sample is read and has an error: correct the patterns (Task 15's tests gain the case), keep the failed sample and its verdicts, and draw a new one with a new seed.
- [ ] Complete the record. Gate. Commit: `Add the replay of releases and the drawing of definition samples`

### Task 23: The integration test, the documents, and release 0.3

**Files:**
- Create: `.github/workflows/law-integration.yml`, `law-repository.pin.json`, `tools/check-law-repository.sh`
- Create: `.agents/docs/reviews/<date>-stage-1-acceptance.md`
- Modify: `xtask/src/site/guides.rs` (the reference becomes a guide, "Law modules", in the group "Write"), `site/` (regenerated)
- Modify: `docs/law-modules.md`, `docs/language.md`, `docs/cli.md`, `docs/implementation-status.md`, `docs/ARCHITECTURE.md`, `docs/README.md`, `docs/releases.md`, `README.md`
- Modify: `Cargo.toml`, `Cargo.lock` (version 0.3.0), `CHANGELOG.md`, `samples/uscode/` (regenerated, because the lock and the manifest record the version)

**Interfaces:**
- Consumes: everything; the records of Tasks 21 and 22.
- Produces: `tools/check-law-repository.sh DIR`: builds the release binary through `lease`, checks the workspaces of the local law repository with it under the limits of the Global Constraints, and fails when the check fails or when the version the repository records (design 4.3: "The law repository records the version of the toolchain it is checked with") is not the binary's.
- Produces: `law-repository.pin.json`, `{"repository": "", "commit": ""}`, and a workflow with two jobs that do nothing while `repository` is empty: one checks the law repository at the pinned commit with the toolchain of the commit under test (design 4.3: "A compiler change that breaks it fails, unless the pin moves together with a matching change on the other side"); one, on a schedule, checks the newest of each against the newest of the other and reports drift. The workflow's only trigger for now is `workflow_dispatch`; a comment at its top says which two lines switch it on when the law repository is published.
- Produces: the acceptance record: for each of the thirteen items of design 9.6, the command lines that were run, where, their output, and "met", "met with a written difference" or "open" with the reason.

**What the documents must say after this task:**
- `docs/law-modules.md`: complete and in reading order: what a law module is; editions; addresses and paths; normalization; provisions, notes and layers; hashes; the syntax, with the three golden modules; workspaces, the manifest and the lock; how a workspace is checked, references and their outcomes; the cache; definitions and their patterns; curated modules, names, dependencies and bindings; change sets; the ingester for the Code, what it reads and what it does not import; the counting rules; limits.
- `docs/implementation-status.md`: a section for edition 2 that says what is parsed, what is checked, and that nothing in an edition-2 module is evaluated; and the limits: a dependency must be a generated workspace and its path relative; tables of contents are not imported; no display citation is derived; definitions are extracted by the closed patterns only.
- `docs/language.md`: one short section that says there are two editions, what the marker is, and where the reference is.
- `docs/cli.md`: every new command and flag (each task has added its own; this task reads the whole page against `fidryn --help`).
- `docs/ARCHITECTURE.md`: the two new crates, the direction rule, the two phases of a workspace check.
- `docs/releases.md`: after raising the version, run `cargo xtask uscode-sample --update`.
- `README.md`: what works now, in one paragraph, with the two commands a reader can run on the sample.
- None of them describes a stage, a plan or a destination.

**Tests:**
- The site check and the documentation check pass with the new guide; every command a document shows runs within the check's limit.
- `cargo run -p xtask --offline -- release-check` prints `release 0.3.0`.
- `cargo xtask uscode-sample` passes after the regeneration.
- The workflow file passes `actionlint` (the "Workflow lint" job of CI runs it; locally, run it if it is installed and say so if it is not).

**Steps:**
- [ ] Write the acceptance record from the records of Tasks 21 and 22; mark what is open.
- [ ] Bring the documents to the state above; regenerate the site.
- [ ] Raise the version to 0.3.0; fold the `[Unreleased]` notes into `## [0.3.0] - <date>`; regenerate the sample.
- [ ] Gate, package build, license check and release check, as at the end of Stage 0.
- [ ] Commit: `Document law modules and label the release 0.3.0`

After this task the controller runs the whole-branch review once, applies one round of fixes for findings that meet the review bar, and reports to the owner. Merging into `dev` and everything after it are the owner's decisions.

## Acceptance map

The thirteen items of design 9.6 and where each is met.

| Item | Met by |
| --- | --- |
| 1. The ingester's workspace is accepted by `fidryn check` in one invocation | Task 21 (full release); Task 14 (sample, in CI) |
| 2. Counts match the independent profile; differences explained; note totals beside them | Task 2 (the rules and the profile), Task 8 (units and provisions on the sample), Task 16 (the ledger), Task 21 (the full release, with the written explanation of any difference) |
| 3. Every unresolved reference has a recorded reason | Task 12 (the closed outcomes), Task 21 (the full list) |
| 4. Rendered text equals the normalized official text for every provision and note; every module verifies against the pinned file | Task 14 (sample, in CI), Task 21 (full release: `uslm_text.py` against `fidryn text --json`, and `ingest --verify`) |
| 5. The change-set command on two real consecutive releases, confirmed independently | Task 20 (fixtures), Task 22 (the real pair, `usc_compare.py --against`) |
| 6. Staleness follows 9.4 on those two releases | Task 19 (fixtures), Task 22 (three bindings on the real pair) |
| 7. The definition samples are drawn, read and recorded | Task 22 draws and prepares them; reading waits for decision O-3 |
| 8. Every release point the publisher offers is replayed as a commit | Task 22 |
| 9. Ten minutes cold, thirty seconds unchanged, 16 GB | Task 21 |
| 10. Existing tests and Stage 0 checks pass | every task's gate |
| 11. The reference documents workspaces, provisions, citations, definitions and bindings; the site shows the ledger | Tasks 3 to 20 (sections), Task 16 (the ledger page), Task 23 |
| 12. A second workspace at a pinned release; a different release is a diagnostic | Task 18 |
| 13. The integration test runs locally on the full workspace and in CI on the sample; the cross-repository job is written | Task 14 (CI), Task 23 (the local script and the workflow) |
