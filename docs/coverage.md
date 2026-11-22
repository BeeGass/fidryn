# Coverage

The figures on this page are those of the sample of the United States Code
committed in this repository, under `samples/uscode/`, and of nothing else.
The sample holds three whole titles of the release `119-142` of the
official XML release point published by the Office of the Law Revision
Counsel: Title 1, Title 11 Appendix and Title 45. The table below is
generated from the coverage ledger of the sample's workspace,
[`samples/uscode/workspace/coverage.json`](../samples/uscode/workspace/coverage.json),
which [`fidryn coverage`](cli.md#coverage) prints and
[The coverage ledger](law-modules.md#the-coverage-ledger) describes key by
key.

## What the words mean

The ledger says how far the provisions of the sample are taken in three
words, and in no others.

- **Imported.** The official text of the provision is in a module, read
  from a pinned file, the publisher's archive, whose size and digests the
  importer verifies against the retrieval record that says where and when
  it was fetched. The lock of the workspace records the digests of each
  archive and of each module file, and the check of the workspace verifies
  every module file against them.
- **Addressed.** The provision has a typed address, and its place in the
  hierarchy, its status and the references in its text are resolved and
  checked. Every provision imported is addressed. Each reference has an
  outcome, and the table gives the references of each layer by outcome, so
  that a reader sees how many of them reach their target.
- **Classified.** What kind of provision it is, is recorded. At this version
  the one kind that is recorded is a definition, and the ledger gives the
  number of definitions extracted, by basis and by kind; a definition with
  a condition, or with source words in the place of a link, is counted as
  any other. It gives no count of classified provisions, since one
  provision can hold more than one definition.

## The basis of a definition

Each definition extracted has one of three bases:

- `enacted`: its reach names a region of the Code, and the file of its unit
  declares its title positive law, so the words that state the reach are
  those the law enacted.
- `restated`: its reach names a region of the Code, and the file of its
  unit does not declare its title positive law, so the words are those of
  the editors of the Code, and the reach the law enacted may differ from the
  one they state.
- `unresolved`: the rules found no region of the Code for it, for one of
  four causes: it states no reach that they read; its reach is a link to a
  law outside the Code, such as a public law; its reach words are in a form
  the rules do not resolve, whatever they name: a part of the Code, as "this
  chapter and chapter 5" and, of a court rule, "this Rule 3021" do, or a law
  outside it, as "this Act" does, unless those words are those of one link
  to an address of the Code, whose target is then the region; or
  its reach is in a form they resolve, "this" and the name of a kind of
  provision or container, but neither the provision that states the reach
  nor any provision or container above it is of that kind, as with "As
  used in this subtitle" in a section that stands in no subtitle.

The rules read definitions written in a closed list of forms
([Definitions](law-modules.md#definitions)), and they do not find every
definition that the text holds. The forms they leave out are listed, with
their counts measured on the whole of release point 119-142, in
[What stays out](law-modules.md#d16-what-stays-out); the share they miss
has not been measured for the sample.

## Counted and not imported

Some of what the publisher's files hold is by rule not imported. The
importer counts it in the lock of the workspace under these six names, and
the ledger copies the counts:

- `tables_of_contents`: the tables of contents, whose entries repeat the
  headings of the units they list;
- `footnotes_in_tables_of_contents`: the footnotes inside them;
- `links_in_tables_of_contents`: the links inside them;
- `release_notes`: the notes that say which release a file is current
  through;
- `links_without_words`: the links that have no words, or white space
  alone;
- `note_group_headings`: the cross headings that open the groups of notes,
  whose labels are not imported; what a cross heading holds after its label
  is imported, as a note of its group.

## The figures

<!-- coverage-table:begin -->

| Counted | Key | Figure |
| --- | --- | --- |
| The release | `release` | `119-142` |
| The prefix of every address | `prefix` | `/us/usc` |
| Units imported | `totals.units` | 563 |
| Units, by kind | `totals.units_by_kind` | 229 `courtRule`, 1 `level`, 333 `section` |
| Units, by status (`in-force` for a unit with none) | `totals.units_by_status` | 482 `in-force`, 9 `omitted`, 68 `repealed`, 4 `transferred` |
| Containers imported | `totals.containers` | 72 |
| Containers, by kind | `totals.containers_by_kind` | 1 `appendix`, 25 `chapter`, 1 `courtRules`, 1 `level`, 13 `part`, 29 `subchapter`, 2 `title` |
| Provisions imported and addressed, the top provision of each unit included | `totals.provisions` | 4,857 |
| Addresses: aliases kept, and addresses disambiguated and assigned by the importer | `totals.addresses` | 229 `aliases`, 0 `disambiguated`, 3 `assigned` |
| Notes imported, by layer | `totals.notes` | 1,498 `enacted`, 593 `editorial` |
| Notes, by group | `totals.notes_by_group` | 1 `courtRules`, 303 `editorialNotes`, 9 `execDoc`, 539 `statutoryNotes`, 1,239 `ungrouped` |
| Footnotes imported | `totals.footnotes` | 107 |
| Provisions and notes that hold an image, whose picture is not in the archive | `totals.with_images` | 1 |
| References in operative text, by outcome | `totals.references.operative` | 644 `exact`, 14 `target-status`, 0 `ambiguous`, 11 `unit-only`, 1 `range-form`, 273 `no-unit`, 192 `outside`, 0 `malformed` |
| References in enacted notes, by outcome | `totals.references.enacted` | 332 `exact`, 18 `target-status`, 0 `ambiguous`, 14 `unit-only`, 0 `range-form`, 364 `no-unit`, 1,454 `outside`, 0 `malformed` |
| References in editorial matter, by outcome | `totals.references.editorial` | 275 `exact`, 47 `target-status`, 0 `ambiguous`, 14 `unit-only`, 1 `range-form`, 259 `no-unit`, 4,919 `outside`, 0 `malformed` |
| References of the outcome `outside`, by the body of law they name | `totals.outside_by_body` | 739 `/us/act`, 3,310 `/us/pl`, 2,516 `/us/stat` |
| Definitions extracted, by basis | `totals.definitions` | 8 `enacted`, 116 `restated`, 66 `unresolved` |
| Definitions extracted, by kind | `totals.definitions_by_kind` | 7 `borrowed`, 8 `does_not_include`, 31 `includes`, 144 `means` |
| Counted and not imported | `totals.not_imported` | 19 `tables_of_contents`, 4 `footnotes_in_tables_of_contents`, 341 `links_in_tables_of_contents`, 3 `release_notes`, 0 `links_without_words`, 358 `note_group_headings` |
| The file `usc01.xml` (positive law) | `by_source.0` | 39 units, 129 provisions, 11 definitions |
| The file `usc11a.xml` (not positive law) | `by_source.1` | 230 units, 2,362 provisions, 17 definitions |
| The file `usc45.xml` (not positive law) | `by_source.2` | 294 units, 2,366 provisions, 162 definitions |

<!-- coverage-table:end -->

`cargo xtask uscode-sample --update` writes this table from the ledger, and
`cargo xtask uscode-sample`, which `cargo xtask ci` runs, fails when the
table is not what the ledger gives.
