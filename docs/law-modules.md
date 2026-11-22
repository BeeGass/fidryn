# Law modules

A law module is a file of edition 2. It holds provisions of law as data,
with their notes and what the source says of them, each named by an
address and covered by hashes; or, written by hand, it gives names to
provisions and binds names to them. Nothing in a law module is evaluated.
Law modules stand in workspaces: a generated workspace holds what an
importer writes from the archives of one release, and a curated workspace
holds modules written by hand, which may depend on generated workspaces.
`fidryn check` checks one module or a whole workspace, and `fidryn changes`
says what changed between two releases.

This reference gives, in this order: the two editions; the addresses of
modules, provisions and notes, and the file path of a module; the
normalization of text; the model of provisions, notes and layers; their
hashes; the syntax of edition 2, with three modules written in it; what the
checker verifies in one module; workspaces, their manifest and their lock,
and what the checker verifies in one; how a workspace is checked, and the
outcome of each reference; the cache of the check; the definitions that the
ingester extracts and the patterns it reads them by; curated modules, their
names, the dependencies of a curated workspace and its bindings; change
sets; the ingester for the United States Code, what it reads and what it
does not import; what is counted, in the coverage ledger and in the sample
of the Code that the repository holds; and the limits of all of these.

The crate `fidryn-provision` implements the model and the rules over it,
`fidryn-syntax` reads the text of a module, `fidryn-check` lowers it into
the model and checks it, `fidryn-driver` checks a workspace, writes a
generated one and compares two, and `fidryn-ingest-uscode` imports the
Code. In the examples, `/zz/demo` is an invented corpus.

## Editions

A `.fr` file is of one of two editions. A file of the first edition is a
program of the language that [Language](language.md) describes, and
carries no marker. A file of edition 2 is a law module: its first two
significant tokens are the word `edition` and the integer `2`; white
space, line ends and comments before them are skipped. A file whose first
significant token is anything else is a file of the first edition. A file
that begins with the word `edition` followed by anything other than `2` is
refused with `E100`: "unknown edition `3`; this toolchain reads edition 2
after the word `edition`, and files of the first edition carry no marker".

`fidryn_syntax::edition::edition_of` tells the edition of a file, and each
entry point asks it before it reads the file. The command line asks it of
a regular file only: an input that is no regular file, such as a pipe or
`/dev/stdin`, could not be read twice, and every command reads it as a
file of the first edition.

| Entry point | Given a module of edition 2 |
| --- | --- |
| `fidryn check FILE` | the checks of one module, as [below](#what-the-checker-verifies-in-one-module), then `ok` |
| `fidryn check DIR` | the checks of the workspace in the directory, every module of it included, as under [Workspaces](#workspaces) |
| `fmt`, `inputs`, `run`, `explore`, `explain`, `verify`, `diff`, `render`, `review` and `file` | exit 1 with `E100`: "an edition 2 module holds provisions and is not compiled for evaluation; `fidryn check` reads it" |
| `fidryn_cli::compile_source` and `compile_module`, and so the source pasted into the mill of `ui` | the same `E100` |
| the documentation check, `cargo xtask docs` | a fenced block, untagged or tagged `fidryn`, whose first line that is not blank is `edition 2` is a complete module and gets the checks of one module |

## Addresses

An address names a module, a provision or a note. It is `/` followed by one
or more segments separated by `/`:

```text
Address = "/" Segment { "/" Segment }
Segment = one or more of: ASCII letters, ASCII digits, "-", ".", "~" and the en dash U+2013
```

- A segment is never empty, never `.` and never `..`.
- Two addresses are equal when their texts are equal byte for byte:
  `/us/usc/t1/s8/a` and `/us/usc/t1/s8/A` are different addresses.
  Addresses are ordered by the bytes of their text.
- The full stop and the en dash occur in the identifiers of the United
  States Code: `s1395i–3`, `rule71.1`, `s44...46`. No identifier of the Code
  holds `~`. An ingester writes `~`, and only an ingester, to mark what it
  added to an identifier, as in `s1932~2`.
- The first segment names the authority that issues the names below it, the
  second the corpus. Nothing interprets either.
- The parent of an address is the address without its last segment. An
  address of one segment has no parent.
- An address is within another when it equals the other or lies below it at
  a segment boundary: `/us/usc/t1/s8/a` is within `/us/usc/t1/s8`, and
  `/us/usc/t1/s80` is not.

In JSON an address is a string that holds its text. A string that is no
address is refused when it is read.

`Address::parse` refuses a text that is no address and names the fault, with
the segment it lies in, counted from 0:

| Text | Refused because |
| --- | --- |
| the empty text | it is empty |
| `us/usc/t1` | it does not begin with `/` |
| `/us//t1` | segment 1 is empty |
| `/us/usc/` | segment 2 is empty |
| `/us/./t1`, `/us/../t1` | segment 1 is `.` or `..` |
| `/us/usc/t1 s8`, `/us/usc/t1_s8`, `/us/usc/t1%20s8` | segment 2 holds a space, `_` or `%` |

## File paths

A module with address `A` is the file `layout::module_path(A)` under its
workspace's root:

1. Each segment of the address maps to one path segment, character by
   character:

   | Character of the address | In the path |
   | --- | --- |
   | a lower-case ASCII letter, a digit or `~` | kept |
   | an upper-case ASCII letter | `_` followed by its lower-case form |
   | the en dash U+2013 | `-` |
   | the ASCII hyphen | `%2D` |
   | a full stop | kept, except as the first or the last character of a segment and before the letters `fr` that end a segment, where it is `%2E` |

2. If the mapped segment, or its part before its first full stop, compared
   without regard to case, is one of `con`, `prn`, `aux`, `nul`, `com1` to
   `com9` or `lpt1` to `lpt9`, its first character is written as `%`
   followed by the two upper-case hexadecimal digits of its code.
3. The path is the mapped segments joined by `/`, followed by `.fr`. The
   separator is `/` on every host.

| Address | File path |
| --- | --- |
| `/us/usc/t29/s2611` | `us/usc/t29/s2611.fr` |
| `/us/usc/t42/s1395i–3` | `us/usc/t42/s1395i-3.fr` |
| `/us/usc/t2/s44...46` | `us/usc/t2/s44...46.fr` |
| `/us/usc/t5/ptIII/sptE` | `us/usc/t5/pt_i_i_i/spt_e.fr` |
| `/us/usc/t28a/courtRules/Civil/rule6` | `us/usc/t28a/court_rules/_civil/rule6.fr` |
| `/us/usc/t28/s1932~2` | `us/usc/t28/s1932~2.fr` |
| `/zz/demo/a-b` | `zz/demo/a%2Db.fr` |
| `/zz/demo/a.` | `zz/demo/a%2E.fr` |
| `/zz/demo/con` | `zz/demo/%63on.fr` |
| `/zz/demo/con.1` | `zz/demo/%63on.1.fr` |
| `/zz/demo/lpt1` | `zz/demo/%6Cpt1.fr` |
| `/zz/demo/.a` | `zz/demo/%2Ea.fr` |
| `/zz/demo/a.fr/b` | `zz/demo/a%2Efr/b.fr` |
| `/zz/demo/a..fr` | `zz/demo/a.%2Efr.fr` |

No path segment begins with a full stop, and none ends in a full stop or in
`.fr`, so the file of one module never has the name of another module's
directory: `/zz/demo/a.fr/b` is `zz/demo/a%2Efr/b.fr`, beside the file
`zz/demo/a.fr` of the address `/zz/demo/a`.

The mapping is one to one, so two addresses never share a path, also on a
file system that ignores case: `/zz/demo/ch6A` is `zz/demo/ch6_a.fr` and
`/zz/demo/ch6a` is `zz/demo/ch6a.fr`.

`layout::address_of_path` reads a path back as its address. It refuses
every path that the three rules do not write, such as `zz/demo/CH1.fr`,
`zz/demo/a_.fr`, `zz/demo/a%2d.fr`, `zz/demo/a%5F.fr`, `zz/demo/con.fr`,
`zz/demo/.a.fr`, `zz/demo/a.fr/b.fr`, `zz/demo/a.txt` and `zz\demo\a.fr`.
The path `zz/demo/a-b.fr` is the address `/zz/demo/a–b`, with an en dash.

## Normalization

The text of a law module is normalized. `normalize::normalize` normalizes a
text in four steps:

1. Put the text into Unicode Normalization Form C.
2. Replace each of the 25 characters with the Unicode property `White_Space`
   with U+0020: U+0009 to U+000D, U+0020, U+0085, U+00A0, U+1680, U+2000 to
   U+200A, U+2028, U+2029, U+202F, U+205F and U+3000.
3. Replace each run of U+0020 with one.
4. Remove U+0020 at the start and at the end.

Nothing else changes: no case folding, and no change to quotation marks or
dashes. U+200B (zero width space) and U+FEFF are not white space and are
kept. `normalize::is_normalized` tells whether a text is normalized, that
is, whether normalizing it leaves it unchanged.

| Text | Normalized |
| --- | --- |
| two spaces, `a`, two spaces, `b`, one space | `a b` |
| `a`, U+00A0, `b` | `a b` |
| `§`, U+202F, `1` | `§ 1` |
| `Current`, U+2000, `through` | `Current through` |
| `a`, tab, `b`, line feed, `c`, carriage return, line feed, `d` | `a b c d` |
| `e`, U+0301 | `é` (U+00E9) |
| `1`, U+00A0, `000` | `1 000` |
| U+3000, `a`, U+1680 | `a` |
| `a`, U+200B, `b` | unchanged |
| U+FEFF, `a` | unchanged |
| `[sic]` | unchanged |
| `“x” — y` | unchanged |

The unit of normalization is the block: a paragraph, a heading, a number or
a paragraph of a table cell. The text of a block is the words of its runs,
concatenated, and that text is normalized: a run may begin or end with a
space, a block may not.

## Provisions, notes and layers

A module of law has one of three shapes:

- A **unit** holds one tree of provisions, whose top provision has the
  module's address. It also records the other addresses the source gives
  it (`aliases`), the container it stands in (`within`), the member of the
  source archive it was read from (`source`) and whether the source marks it
  as positive law (`positive_law`, which may be unknown), and it holds a
  source credit, notes, footnotes and definitions.
- A **container**, such as a chapter, has a kind, a number, a heading and a
  status of its own and lists its members: the containers and units directly
  below it, in document order. It holds a source credit, notes and
  footnotes, and records `aliases`, `within`, `source` and `positive_law` as
  a unit does.
- A **curated** module is written by hand. It imports modules, names
  addresses (citations) and binds names to provisions (bindings). A binding
  records the release it was made against and two hashes, one of the
  provision and one of the enacted notes of its unit, which tell when it
  is [stale](#bindings).

The crate holds the model in its module `model`.

### Provisions

A provision has an address, a kind, a number, a heading and a status, each
of the last three optional, and its items: text parts and child provisions,
in document order. The address of a child lies below the address of its
parent.

- The kind and the status are identifiers, such as `section` and
  `repealed`. The model gives them no meaning and knows no list of them. A
  provision without a status is in force.
- A number has a value, its machine form, and the text it is shown as,
  which may hold a footnote marker: the value `8` shown as `§ 8.`.
- A text part has a role and at least one block:

| Role | The part holds |
| --- | --- |
| `chapeau` | the words that lead into the children |
| `content` | the text of the provision |
| `continuation` | words that follow children |
| `proviso` | a proviso |

### Texts and blocks

A text is a list of runs:

| Run | Holds |
| --- | --- |
| words | plain words |
| link | the linked words, and the target as the source writes it |
| superscript | superscript words |
| subscript | subscript words |
| footnote marker | the marker as shown, and the number of the footnote it points at |

The plain text of a text is the words of its runs, footnote markers left
out, concatenated; it is the text that is normalized. `Text::from_runs`
merges adjacent runs of words into one and drops every run whose words are
empty. A footnote marker has no words of its own and is kept.

A block is a paragraph, a subheading, a table, quoted content or an image:

- A table is a list of rows. A row belongs to the head, the body or the foot
  of its table and holds cells. A cell is a header cell or not, spans one or
  more columns and one or more rows, and holds blocks. An empty cell holds
  none.
- Quoted content has the origin the source names for it, if any, and items:
  blocks, text parts and levels. A level, such as a quoted section, has a
  kind, an optional number, an optional heading and items of its own, and
  no address.
- An image names the source of its picture. The picture is not part of the
  model.

### Notes, footnotes and definitions

A note has an address, a layer, the group word of the source (`ungrouped`
when there is none), its topics, an optional heading and blocks. The layer
of a note is enacted or editorial. A note belongs to the unit or the
container it is anchored in.

A footnote belongs to its module. The footnotes of a module are numbered
from 1 in document order; each has the marker it shows and its blocks. A
footnote marker names its footnote by that number.

A definition records the provision that holds it (`at`), the terms it
defines, its kind (`means`, `includes`, `does_not_include` or `borrowed`),
the words that state its reach or none when the text states none, the
region the reach covers, its basis (`enacted`, `restated` or `unresolved`),
what a borrowed definition takes its meaning from (`borrowed_from`), and a
note to read with it (`see_note`). The ingester for the Code extracts them
by the rules of [Definitions](#definitions).

### Layers

The text of a module falls into three layers:

| Layer | Holds |
| --- | --- |
| operative | the provisions of a unit; the kind, number, heading, status and members of a container |
| enacted | the enacted notes |
| editorial | the source credit, the editorial notes, the footnotes and the footnote markers |

### In JSON

In JSON the model has the form that `serde` derives: a struct is an object
of its fields, an absent value is `null`, a variant without fields is its
name in snake case (`content`, `does_not_include`), and any other variant
is an object whose one key is its name in snake case. A text is
`{"runs": [...]}`, and a paragraph that holds a link is:

```json
{"para": {"runs": [{"words": "As used in "}, {"ref": {"words": "section 2", "target": "/zz/demo/t1/s2"}}]}}
```

## Hashes

A hash is 32 bytes of BLAKE3, written as 64 lower-case hexadecimal digits.
`Hash32` reads only that form: a text of another length, or one with an
upper-case or any other character, is refused and the fault is named.

Every hash is BLAKE3 over a sequence of frames. A frame is one tag byte,
the length of its value as eight bytes little-endian, and the value. The
first frame of every hash has the tag `0x00` and holds a domain string.
Below, `‖` is concatenation and `F(t, v)` the frame with tag `t` and
value `v`:

| What | Frames |
| --- | --- |
| a text `T` | `F(0x10, UTF-8 of the plain text of T)`, then for each run that is not words, in order: a link `F(0x11, start ‖ end ‖ UTF-8 of its target)`; a superscript `F(0x12, start ‖ end)`; a subscript `F(0x13, start ‖ end)`; a footnote marker nothing. `start` and `end` are byte offsets into the plain text, eight bytes little-endian each, `end` exclusive. |
| an absent text | `F(0x1F, empty)` |
| a block | a paragraph `F(0x20, text)`; a subheading `F(0x21, text)`; a table `F(0x22, table)`; quoted content `F(0x23, quoted)`; an image `F(0x24, UTF-8 of its source)` |
| a table | for each row `F(0x30 + group, cells)`, with the group 0 for the head, 1 for the body and 2 for the foot; a cell is `F(0x34, colspan ‖ rowspan ‖ blocks)`, or `F(0x35, ...)` for a header cell, each span four bytes little-endian |
| quoted content | `F(0x40, UTF-8 of the origin, or empty)`, then each item: a block as above, a part as below, a level `F(0x41, F(0x01, kind) ‖ number ‖ heading ‖ items)` |
| a number | `F(0x02, UTF-8 of the value) ‖ F(0x03, the shown text as a text)`, or `F(0x04, empty)` when there is none |
| a text part | `F(0x50 + role, blocks)`, with the role 0 for `chapeau`, 1 `content`, 2 `continuation` and 3 `proviso` |
| a provision (its tree hash) | BLAKE3 of `F(0x00, "fidryn.provision/v1") ‖ F(0x01, kind) ‖ number ‖ heading ‖ F(0x05, status or empty) ‖ items`, where a text part is as above and a child is `F(0x60, its tree hash)` |
| a note | BLAKE3 of `F(0x00, "fidryn.note/v1") ‖ F(0x70, group) ‖ F(0x71, topics joined by one space) ‖ heading ‖ blocks` |
| a container | BLAKE3 of `F(0x00, "fidryn.container/v1") ‖ F(0x01, kind) ‖ number ‖ heading ‖ F(0x05, status or empty)`, then `F(0x61, address)` for each member in order |
| the enacted layer | BLAKE3 of `F(0x00, "fidryn.layer.enacted/v1")`, then for each enacted note in the byte order of its address `F(0x72, address) ‖ F(0x73, note hash)` |
| the editorial layer | BLAKE3 of `F(0x00, "fidryn.layer.editorial/v1") ‖ the credit as a text, or an absent text`, then the editorial notes as in the enacted layer, then for each footnote in ordinal order `F(0x74, UTF-8 of its marker) ‖ blocks`, then for each footnote marker of the module in document order `F(0x75, UTF-8 of the address it is recorded with) ‖ F(0x76, UTF-8 of the marker as shown) ‖ F(0x77, its footnote number as four bytes little-endian)` |
| the operative layer | of a unit, the tree hash of its top provision; of a container, its container hash |

The address of a provision is not in its tree hash, and the address and
the layer of a note are not in its hash. Definitions are in no hash. A
container's credit is in its editorial layer and not in its container hash.

The document order of the footnote markers of a unit is: the tree of
provisions depth first, taking for each provision its number, its heading
and then its items in order; then the credit; then the notes in the byte
order of their addresses, as the layers take them, the heading of each
before its blocks; then each footnote in ordinal order. For a container it
is its number, its heading, its credit, its notes in the same order, and
its footnotes. A marker in a provision is recorded with that
provision's address, and a marker in a note, its quoted content included,
with the note's address. A marker in a credit, in a footnote, or in a
container's own number or heading is recorded with the module's address.

What follows from these rules:

- A change to any word, number, heading, status, link target or link
  extent, superscript, subscript, table cell or span of a provision changes
  its tree hash and the tree hash of every provision above it, and of no
  other provision.
- A footnote, a footnote marker, and the provision a marker stands in
  change the editorial layer and nothing else. Where in its provision a
  marker stands is in no hash.
- The notes and their footnote markers are taken in the byte order of the
  notes' addresses, so the order in which a module lists its notes changes
  no layer hash. Moving a note to a new address changes the hash of its
  layer and not the note's own hash.

`hash::hash_provision`, `hash::hash_note`, `hash::hash_container`,
`hash::layer_hashes_of_unit` and `hash::layer_hashes_of_container` compute
them. `hash::tree_hashes` gives the tree hash of every provision of a tree
in document order, hashing each provision once. A second implementation in
Python, `conformance/hash_vectors.py`, computes the same hashes of the
invented models in `conformance/fixtures/hash/vectors.json`, and the tests
of both hold their values to the digests recorded there.

## Syntax

The tokens are those of the lexer of the first edition. Words such as
`module` and `content` are identifiers that the parser compares by their
text. A string is a JSON string literal. Line ends are not significant, and
`//` begins a comment that runs to the end of its line.

```text
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

A link `[words -> target]` holds the linked words and the target as the
source writes it; `sup` and `sub` hold superscript and subscript words;
`mark "1" 2` is a footnote marker shown as `1` that names footnote 2. A
`num` holds the value of a number and the text it is shown as.

The parser, `fidryn_syntax::e2::parse`, enforces the grammar and stops at
its first error, an `E100` at the token it is about:

- Every block has the fields shown, in the order shown. A word that is no
  field of its block is "unknown field `<word>` in `<block>`"; a field that
  stands out of its order, twice, or not at all is named as such.
- A word that begins no declaration is "unknown declaration `<word>`".
- Text after the closing brace of the module is an error.
- A source holds at most 64 MiB and 16,777,216 tokens, and its blocks nest
  at most 64 deep: "module exceeds resource limits".

A module has one of three shapes. A **unit** has exactly one `provision` at
the top, a **container module** exactly one `container`, and a **curated**
module neither. `alias`, `within`, `source`, `positive_law`, `credit`,
`note`, `footnote`, `definition` and `layers` belong to units and
containers; `import`, `citation` and `binding` belong to curated modules.
`within`, `source`, `positive_law`, `credit` and `layers` stand at most
once, and a unit or a container has a `source` and `layers`. A container
holds no provision, so a definition in a container module names no
provision of its module. The declarations may stand in any order.

**The canonical form** is what `fidryn_provision::render::render` writes,
and generated modules are always in it:

- UTF-8, `\n` line ends, a final line end, two spaces of indentation for
  each open brace, no blank line, no trailing space.
- Line 1 is `edition 2`, line 2 is `module "<address>" {`.
- The declarations in this order: each `alias`, `within`, `source`,
  `positive_law`, the `provision` or the `container`, `credit`, the notes
  in document order, the footnotes by ordinal, the definitions in the
  document order of the provisions they name and then by their first term,
  and `layers`. A curated module has its imports, then its citations, then
  its bindings, each in the order of its list.
- One field to a line. A text is written on one line, its runs separated by
  one space.
- A string is written with `\\` for a backslash, `\"` for a quotation mark,
  `\u00XX` with lower-case hexadecimal digits for a character below U+0020
  or U+007F, and every other character as itself.
- A part or a cell whose content is exactly one paragraph is written in the
  short form (`content "..."`), otherwise in the brace form with one block
  to a line. A cell with no block is written `td ""`, and that spelling is
  read back as a cell with no block. The shown form of a number that has
  none is written `""` and read back as the empty text. Besides these two,
  an empty string stands only where the source itself may give an empty
  value: the value of a number, the shown form of a footnote and of a
  marker, the source of an image, and the target of a link. Anywhere else
  an empty string is refused: as the words of a run, as a term, as stated
  reach words, as `borrowed_from`, as `source`, as the release of a
  binding, as a topic, and as the origin of quoted content.
- `span <colspan> <rowspan>` is written only when one of the two is not 1.
- Every `hash` and the three values of `layers` are computed by the rules
  of [Hashes](#hashes) from what is written above them.

`fidryn_check::e2::lower` reads a parsed module back into the model:
lowering the parse of what `render` writes gives the model back. A unit,
with invented text, in the canonical form:

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
      hash "ec085e67853dd9ced4dafa9836e747dc158ae35ed76f628d9b1eacbd6937be77"
    }
    provision "/zz/demo/t1/s8/b" subsection {
      num "b" "(b)"
      heading "Sizes"
      content {
        p "A widget comes in the sizes of this table." mark "1" 1
        table {
          head {
            th "Size"
            th "Width"
          }
          body {
            td "small"
            td "1 inch"
          }
          body {
            td "large"
            td ""
          }
          foot {
            td span 2 1 "Every width is invented."
          }
        }
      }
      hash "f36533507953a8ee2c81864d9db0cda382347b2322c0df95de608eef5c5c62a3"
    }
    hash "699c00e650aa1e79b5d4fa480684eb557f327ebdb41a043553535a12ab189a31"
  }
  credit "(Added by an invented act.)"
  note "/zz/demo/t1/s8/note/editorialNotes/nolaw/codification" editorial {
    group editorialNotes
    topic "codification"
    heading "Codification"
    p "This invented section was enacted as part of an invented code."
    hash "c3b231e97013f057f0ca7072b0aec4cde6432fa8567fe131b4065c60eefb864e"
  }
  footnote 1 "1" {
    p "So in the invented original."
  }
  definition "/zz/demo/t1/s8/a" {
    term "widget"
    kind means
    words "As used in section 2 of this title"
    region "/zz/demo/t1/s2"
    basis enacted
  }
  layers {
    operative "699c00e650aa1e79b5d4fa480684eb557f327ebdb41a043553535a12ab189a31"
    enacted "a396bac23ba81b30cb34dbf808cf06deeaeb4b87e010b66646faadc6accfc6ff"
    editorial "ddcc1297ec38d082f41d0dc581020c81cbb1c1ede2e221dd58b8000528b4cc40"
  }
}
```

This module is
[`unit.fr`](../crates/fidryn-check/tests/fixtures/e2/unit.fr). Beside it,
[`container.fr`](../crates/fidryn-check/tests/fixtures/e2/container.fr) is
a container module and
[`curated.fr`](../crates/fidryn-check/tests/fixtures/e2/curated.fr) a
curated module that binds two provisions of the unit. The tests of
`fidryn-check` hold the three files to what `render` writes, and the
documentation check checks the module above.

## What the checker verifies in one module

`fidryn check FILE` and the documentation check run
`fidryn_check::e2::check_source` on a module of edition 2: the marker, the
grammar, lowering into the model (`lower`), and the checks over the model
(`check_module`). Every diagnostic carries the span of the text it is
about. `fidryn check` prints each as `FILE: CODE Title: message` and exits
1, or prints `ok`.

| Check | Code |
| --- | --- |
| The module's address, and every address written in it, is an address | `E700` |
| The module has one of the three shapes, every declaration belongs to its shape, a declaration that stands once stands once, and a unit or a container has a `source` and `layers` | `E100` |
| A unit's top provision has the module's address, and a container's address is the module's; every child provision's address is below its parent's; every note's address is below the module's; no address of a provision or a note is declared twice | `E700` |
| Every run but a footnote marker has words, except in `td ""` and in the shown form `""` of a number ("a run has no words"); the text of every block is normalized ("text is not normalized"); a topic is one word, neither empty nor holding white space; no term, stated reach words, `borrowed_from`, `source`, release of a binding or origin of quoted content is an empty string | `E710` |
| Every `hash`, every value of `layers` and both hashes of a binding are 64 lower-case hexadecimal digits | `E710` |
| Every `hash` and every value of `layers` equals the hash computed from what the module writes | `E710` |
| The footnotes are numbered from 1, without a gap, in the order they are written; every footnote marker names a footnote of the module | `E710` |
| A definition names a provision of the unit; each term stands in that provision's text; the reach words, when stated, stand in its text or in the chapeau of one of its ancestors; the kind is `means`, `includes`, `does_not_include` or `borrowed` and the basis `enacted`, `restated` or `unresolved`; a `borrowed` definition has `borrowed_from` | `E710` |
| In a curated module, no name is declared twice, its citations and its bindings sharing one set of names | `E200` |

An address is below another when it lies within it and is not the same
address. The text of a provision is the plain text of each paragraph and
subheading of its text parts, those in its tables and its quoted content
included, and none of its children's; a term stands in it when one of
those texts holds the term. Every text holds the empty string, so an empty
term or empty reach words would pass the checks of a definition without
checking anything. The topics of a note and the origin of quoted content
have rules of their own because the hash of a note joins its topics with
one space and the hash of quoted content takes an absent origin as an
empty one: without them, two different modules could have one hash.

An address or a name declared twice is reported at its later declaration,
with the earlier one as a related span. A footnote is reported at its
number when the number is not one more than that of the footnote written
before it, the first being 1.

A text that is not normalized is reported at the run where it first
differs from its normalized form. A hash that does not match is reported
at the hash, with the hash computed from the module. A change to one word
of a provision is therefore reported at the `hash` of that provision, at
the `hash` of every provision above it, and at the `operative` value of
`layers`, since each of them covers the word. When lowering finds an
error, the module is not lowered and the checks over the model do not
run: a module whose text is not normalized is not also checked for its
hashes.

## Workspaces

A workspace is a directory with a manifest, `workspace.json`, at its top.
Every `.fr` file in it is a module of edition 2 whose address is below the
manifest's `prefix` and which stands at the [file path](#file-paths) of its
address. An entry whose name begins with a dot, whether a file, a
directory or a symbolic link, is not the workspace's: the check does not
read it, nor anything in it, unless it is told to keep its
[cache](#the-cache) there, and the writer leaves it alone, so a
repository's `.git`, a `.gitignore` or an editor's lock link may stand in a
workspace. No symbolic link is followed, and one whose name does not begin
with a dot is an error.

```json
{
  "schema": "fidryn.workspace/v0.1",
  "name": "demo",
  "kind": "generated",
  "prefix": "/zz/demo",
  "release": "42-7",
  "toolchain": "0.3.0",
  "dependencies": []
}
```

- `kind` is `generated` or `curated`. A generated workspace is written by
  an importer from the archives of one release: its manifest has that
  `release`, and it has no dependencies. A curated workspace is written by
  hand: its manifest has no `release` key.
- Each dependency of a curated workspace names a generated workspace with
  its `name`, its directory `path`, relative to the manifest's directory
  with `/` between its segments, its `release`, and `lock_digest`, the
  BLAKE3 of the bytes of its lock
  ([Depending on another workspace](#depending-on-another-workspace)).
- `toolchain` is the version of the toolchain that whoever wrote the
  manifest recorded; the importer records its own. No check writes it: a
  check compares it with its own version and reports a difference as
  `W760`, a warning.

A generated workspace has a lock, `law.lock.json`, beside its manifest; a
curated workspace has none. The lock records the archives the workspace
was made from, what the importer counted, and the BLAKE3 of the bytes of
every module file by its path:

```json
{
  "schema": "fidryn.law-lock/v0.1",
  "prefix": "/zz/demo",
  "release": "42-7",
  "generator": {
    "name": "demo-writer",
    "version": "0.3.0"
  },
  "artifacts": [
    {
      "file": "demo01@42-7.zip",
      "url": "https://example.invalid/releases/42/7/demo01@42-7.zip",
      "retrieved_at": "2026-11-16T22:14:03Z",
      "bytes": 42241,
      "blake3": "<64 hexadecimal digits>",
      "sha256": "<64 hexadecimal digits>",
      "members": [
        {
          "name": "demo01.xml",
          "bytes": 2048,
          "blake3": "<64 hexadecimal digits>"
        }
      ]
    }
  ],
  "addresses": {
    "aliases": 0,
    "disambiguated": 0,
    "assigned": 0
  },
  "not_imported": {
    "tables_of_contents": 0,
    "footnotes_in_tables_of_contents": 0,
    "links_in_tables_of_contents": 0,
    "release_notes": 0,
    "links_without_words": 0,
    "note_group_headings": 0
  },
  "modules": {
    "zz/demo/t1.fr": "<64 hexadecimal digits>",
    "zz/demo/t1/ch1.fr": "<64 hexadecimal digits>"
  }
}
```

`url`, `retrieved_at`, `bytes`, `blake3` and `sha256` of an artifact are
those of the archive in the retrieval record, `retrieval.json`, which
stands beside the archives of one release and is written when they are
downloaded:

```json
{
  "schema": "fidryn.retrieval/v0.1",
  "release": "42-7",
  "archives": [
    {
      "file": "demo01@42-7.zip",
      "url": "https://example.invalid/releases/42/7/demo01@42-7.zip",
      "retrieved_at": "2026-11-16T22:14:03Z",
      "bytes": 42241,
      "sha256": "<64 hexadecimal digits>",
      "blake3": "<64 hexadecimal digits>"
    }
  ]
}
```

`fidryn_provision::workspace` holds the three files as data. Each is
written with its keys in the order shown, two spaces of indentation, one
member to a line, an empty object or array written `{}` or `[]`, the
`modules` of a lock sorted by path, and a final line end. Each is read
back, and refused with the place named, when it has a field it does not
know or lacks one it needs, when its `schema` is another, when a digest is
not 64 lower-case hexadecimal digits, and when its JSON value is not the
value its data writes back, such as an object written as the array of its
values. [`workspace-v0.1.json`](../schemas/workspace-v0.1.json),
[`law-lock-v0.1.json`](../schemas/law-lock-v0.1.json) and
[`retrieval-v0.1.json`](../schemas/retrieval-v0.1.json) in `schemas/` are
their JSON Schemas.

### Writing a generated workspace

An importer writes a generated workspace with
`fidryn_driver::workspace::GeneratedWriter`:

- `create(out, manifest)` takes the manifest of a generated workspace with a
  release and no dependencies, and a directory `out` that does not exist,
  holds nothing but entries whose name begins with a dot (the working
  directory of a new repository, with its `.git`, is such a one), or holds
  a generated workspace of the same prefix. Such a directory may also hold
  its [coverage ledger](#the-coverage-ledger), `coverage.json`, at its top,
  which the run writes again, and anywhere any entry whose name begins with
  a dot, which the run leaves as they are. Each of its `.fr` files must be
  the output of an earlier run: a module of edition 2 whose address lies
  below the prefix and which stands at the path of its address. `create`
  reads the head of each, the first significant tokens up to the brace
  after the module's address, which must lie in its first 64 KiB, before
  it writes anything. Any other
  directory, a curated workspace, a generated workspace of another prefix,
  one that holds a symbolic link of another name, and one that holds a
  `.fr` file of the first edition, a module of another prefix or a module
  at another path among them, is refused, with every such `.fr` file named,
  and nothing in it is changed. `create` writes `workspace.json` at once, through a temporary
  file like every file of the run, and clears the temporary files that an
  earlier run which was stopped left behind. One entry whose name begins
  with a dot is the writer's own and is not left alone: `.fidryn-write` at
  the top of the workspace, the directory of the temporary files of a run,
  which `create` clears and `finish` removes.
- `write(module)` writes a unit or a container whose address is below the
  prefix, in the canonical form, to its path, through a temporary file
  that is renamed, and keeps the module's [interface](#interfaces), built
  from the module as it was given. It may be called from several threads
  at once, and it fails when the run has already written a module to that
  path, so that no module replaces another.
- `finish(generator, artifacts, addresses, not_imported)` removes every
  module of an earlier run, found by `create`, that the run did not write,
  and each directory that leaves empty, then writes `law.lock.json`, then,
  last, `coverage.json`, the
  [coverage ledger](#the-coverage-ledger) counted from the interfaces of
  the modules it wrote, the manifest and the lock, and returns the lock.
  Nothing is read again to count the ledger: neither the text of the
  modules nor what the importer read.
- `compare(out, manifest)` makes a writer that writes nothing. It compares
  the manifest with `workspace.json` at once, `write` compares each module
  with the file at its path, and `finish` compares the lock with
  `law.lock.json` and the ledger with `coverage.json`. `finish` counts as
  extra every `.fr` file of `out` that the run did not write and every
  entry that `create` refuses: a file other than a module, the manifest,
  the lock and the ledger at the top, and a symbolic link or any other
  entry that is neither a file nor a directory. A `.fr` file that `create`
  refuses for its head is extra where the run writes nothing, and
  different where it writes a module. The two follow one rule, so
  they cannot disagree. Only the entries whose name begins with a dot are
  not looked at. `finish` returns the lock when
  nothing differs, and then `create` accepts the directory and a run of the
  same modules changes none of its bytes; otherwise it returns
  `WriteError::Differs`, which lists every path that is `missing`, `extra`
  or `different`, in the byte order of the paths. A directory that does
  not exist lacks every file and is not made.

What a run writes depends only on its modules, its manifest and what
`finish` is given: neither the order of the calls nor the number of
threads changes a byte. A run that is stopped before `finish`, by an error
or because it was killed, leaves a generated workspace of the same prefix
whose lock is missing or is that of the run before it. `fidryn check`
reports it with `E730`, and the next run accepts the directory and, when it
finishes, puts it right. A run stopped after the lock and before the
ledger leaves a workspace that checks clean, since no check reads the
ledger, and whose ledger is missing or is that of the run before it;
`compare` reports it as `missing coverage.json` or
`different coverage.json`, and the next run puts it right.

### What the checker verifies in a workspace

`fidryn check DIR` runs `fidryn_driver::workspace::check_workspace`. It
reads each module file by itself: its bytes, its edition and, for a module
of edition 2, the checks of
[one module](#what-the-checker-verifies-in-one-module). Then it checks what
holds across the files, from the manifest, the lock, the BLAKE3 of each
file's bytes and the [interface](#interfaces) of each module that lowered,
as [How a workspace is checked](#how-a-workspace-is-checked) describes, and
gives every reference its [outcome](#references).

| Check | Code |
| --- | --- |
| The directory has a `workspace.json`, a regular file of at most 64 MiB, and it is a manifest | `E730` |
| A generated workspace has a `release`, no dependencies and a lock, a regular file of at most 64 MiB, whose prefix and release are the manifest's; a curated workspace has neither a `release` nor a lock | `E730` |
| In a generated workspace, every module file is listed in the lock with the BLAKE3 of its bytes, and every file the lock lists is present | `E730` |
| Every `.fr` file is a module of edition 2; every entry of the directory whose name does not begin with a dot is a file or a directory, and none is a symbolic link | `E730` |
| Every module's address is below the prefix, and the module stands at the path of its address | `E730` |
| A unit's or a container's `within` names a container of the workspace that lists it as a `member`, and every `member` of a container is a module whose `within` is that container | `E720` |
| Every `import` of a curated module names a module of the workspace or of a dependency, by its address | `E720` |
| The names of every curated module resolve, as [Curated modules: imports, citations and names](#curated-modules-imports-citations-and-names) describes | `E200` |
| No two modules declare one address, as the address of a module, of a provision or of a note | `E700` |
| Each dependency of a curated workspace has a relative `path` and a name that no dependency before it has, and its prefix neither is, contains nor lies within the prefix of the workspace or of a dependency before it that passed its checks | `E730` |
| Each dependency is a generated workspace with a lock, the BLAKE3 of the bytes of its lock and the release of its lock are the `lock_digest` and the `release` of its entry, and it has no error of its own | `E740` |
| No binding of a curated module is stale, as [Bindings](#bindings) describes | `W750`, a warning |
| The manifest's `toolchain` is the version of the running toolchain | `W760`, a warning |

When the directory has no manifest that can be read, that is the one
diagnostic, and nothing else of the directory is read. A module file that
does not lower, a module that does not stand at the path of its address,
and a file the lock lists that is missing, take no part in the checks of
`within`, members and imports, and none of their addresses is one that a
reference reaches: their own diagnostics say what is wrong, and a
container that lists them, or a module that imports them, is not reported
for it.

Every diagnostic names its file by its path in the workspace. One about a
file as a whole, such as a module file the lock does not list or a file of
the first edition, has no span, and so has one about a dependency, which
names the manifest, `workspace.json`. One about where a module stands is at its
address; one about a `within` is at the string after `within`, one about a
member at the string after that `member`, one about an import at the
string after `import`, one about a name at that name, and one about a
stale binding at the binding's name. An address that
two modules declare is reported in the file of each module after the first,
by path, that declares it, naming the file of the first: at the module's
address when the address is the module's own, and otherwise about the file
as a whole, since the interface of a module holds no range of the address
of a provision or a note. The addresses of a module that passes the checks
of one module lie at or below its own, so two such modules that declare one
address lie one within the other, as a unit `/zz/demo/t1/s8/a` lies within
a unit `/zz/demo/t1/s8` that has a subsection `/zz/demo/t1/s8/a`; the file
of the outer one comes first by path. The diagnostics are sorted by the
path of their file and then by position, a diagnostic about the file as a
whole first.

`fidryn check DIR` prints each diagnostic as
`DIR/PATH: CODE Title: message`, then one summary line, and then `ok` when
there is no error. It exits 1 when there is an error, and 0 otherwise, a
warning included. The
invented demo workspace under `conformance/fixtures/workspace/demo` is
what the writer writes from one title, one chapter and three sections;
the tests of `fidryn-driver` hold it to that. Checked with `--no-cache`,
which keeps no [cache](#the-cache) and so writes nothing:

```sh
fidryn check conformance/fixtures/workspace/demo --no-cache
```

```text
modules: 5; from cache: 0; errors: 0; warnings: 0
ok
```

`modules` counts the module files read, every `.fr` file of the workspace,
and `from cache` how many of them were read from the cache rather than
checked again. The module files of a dependency are counted in neither.

## How a workspace is checked

The check of a workspace runs in two phases.

1. **Phase one**, for each module file by itself, of the workspace and of
   each of its [dependencies](#depending-on-another-workspace) that passes
   the checks of its entry, its manifest and its lock: the BLAKE3
   of its bytes, its edition, the grammar, lowering, the checks of
   [one module](#what-the-checker-verifies-in-one-module), and the
   module's [interface](#interfaces). What it gives of a file depends on
   the file's bytes alone, so it is kept in the [cache](#the-cache), and a
   file whose bytes were checked before is not checked again.
2. **Phase two**, with the result of every module in memory: the BLAKE3 of
   each file against the lock of a generated workspace, and where each
   module stands, below the prefix and at the path of its address; and the
   check of each dependency by itself. Then, from the manifest and the
   interfaces alone, the manifest's toolchain, the `within` of every unit
   and container and the members of every container, the imports and the
   names of every curated module, the [staleness](#bindings) of every
   binding, the addresses that two modules declare, and the
   [outcome](#references) of every reference and of every citation. Phase
   two reads no module again.
   `fidryn_driver::workspace::check_interfaces` is that last part as a
   function of the interfaces of the workspace, what is known of each of
   its dependencies (the name, the release, the prefix and the interfaces
   of one that passed its checks, the prefix, when it is known, of one that
   failed), the manifest and the toolchain's version.

Phase one runs on `--jobs` worker threads, by default as many as the
system reports as available, each module file being read and checked on
one of them; the module files of each dependency are read on the same
workers, one dependency after another. Of phase two, the outcomes of the
references and the citations are found on as many threads, the modules
shared out among them; the rest of it, the comparison with the lock, where
each module stands, the checks of each dependency, the manifest, `within`
and members, imports and names, the staleness of the bindings, and the
index of addresses the outcomes are looked up in, runs on one thread. The diagnostics, the summary line, the check
report and the unresolved list are the same, byte for byte, for every
number of workers: each module's result is collected by itself, and the
results are sorted by the path of their file and then by position before
anything is printed or written.

### Interfaces

The interface of a module, which `fidryn_provision::interface_of` builds
from the lowered module and the byte ranges of its declarations, holds all
that phase two and the [coverage ledger](#the-coverage-ledger) read of the
module, and all that a [change set](#change-sets) reads of a module whose
hashes did not change:

- its address, its shape, its aliases, `within`, `source` and
  `positive_law`; for a container its kind, status and members;
- each provision's address, kind, status and tree hash, and whether it
  holds an image; each note's address, layer, group and hash, and whether
  it holds an image; the number of footnotes; the three layer hashes;
- each reference's layer, the address of the provision or note it stands
  in, and its target;
- each definition's provision, terms, kind and basis;
- for a curated module, its imports, citations and bindings;
- the byte range of the module's address, of the address after `within`,
  after each `alias` and after each `member`, and of each `import`,
  `citation` and `binding` declaration, with the ranges of its parts: of an
  import its address, its alias and each name in its braces, of a citation
  its name, and of a binding its name and what `binds` names, the alias and
  the name of `alias.name` apart. Phase two reports what it finds at these
  ranges.

A provision holds an image when one of its own text parts does, its
children's not counted; a note holds one when one of its blocks does,
tables and quoted content included.

An interface is written as one line of JSON,
`{"schema":"fidryn.interface/v0.1","interface":{...}}`, with these keys in
this order:

| Key | Holds |
| --- | --- |
| `address`, `address_range` | the module's address, and the range `{"start": s, "end": e}` of bytes of its source where the address stands |
| `shape` | `unit`, `container` or `curated` |
| `aliases`, `within` | each alias, and `within` or `null`, as `{"item": address, "range": ...}` |
| `source`, `positive_law` | as the module declares them; `null` in a curated module |
| `container` | of a container module, its `kind`, `status` and `members`, each member with its range; otherwise `null` |
| `words` | each kind and status of a provision and each group of a note, once, in the order in which they first occur |
| `provisions` | each provision of a unit, in document order, each before its children, the unit's top provision first: `at`, `kind` and `status` as places in `words`, `hash`, its tree hash, and `image` |
| `notes` | each note: `at`, `layer`, `group` as a place in `words`, `hash` and `image` |
| `footnotes` | the number of footnotes |
| `layers` | the three layer hashes, or `null` in a curated module |
| `references` | each reference, in document order: `layer`, `from` and `target` |
| `definitions` | each definition: `at`, the provision that holds it, `terms`, `kind` and `basis` |
| `imports`, `citations`, `bindings` | of a curated module, each as `{"item": ..., "range": ...}`, where `range` holds `whole`, the range of the declaration, and the ranges of its parts: of an import `module`, `alias`, or `null` for an import with braces, and `names`; of a citation `name`; of a binding `name`, `qualifier`, the alias of `alias.name` or `null`, and `target`, the address or the name after `binds` and any alias |

`at` is an address relative to the module's: empty for the module's own
address, and otherwise the segments below it without their first `/`, so
that `a/1` in the module `/zz/demo/t1/s8` is `/zz/demo/t1/s8/a/1`. An
address that does not lie below the module's, which only a module with an
`E700` has, is written whole. `from` is `"module"`, `{"provision": n}` or
`{"note": n}`, the place of what the reference stands in, counted from 0
in `provisions` or in `notes`. Each place in `words` is counted from 0.
`Interface::from_json` refuses a text of another `schema`, a key it does
not know or a missing one, the key of an optional field included, a place
that names nothing, an `at` that names no address, parts that the shape of
the module does not have, the range of a part that a declaration does not
have, such as the range of an alias of an import with braces, and a
declaration that lacks the range of a part it has, such as an import with
an alias and no range of it. It reads only what `to_json` writes: a
text whose JSON value is not the value that the interface read from it
writes back, such as one with an object written as an array of its values,
is refused, and the place where the two differ is named.

### The check report and the unresolved list

`fidryn check DIR --report FILE` writes the check report,
`fidryn.check-report/v0.1`, whose JSON Schema is
[`check-report-v0.1.json`](../schemas/check-report-v0.1.json): the number
of module files of the workspace read, of those read from a cache, of
errors and of warnings; for each layer the number of its references with
each outcome; under `citations`, the number of the citations of its
curated modules with each outcome; and the number of references of
outcome `outside` by the body of law they name. Every layer and every
outcome is written, in the order shown, and the bodies are sorted. The
report holds no path, so it is the same on every host. This is the report
of an invented workspace that has a reference of every outcome and no
curated module, committed under `conformance/fixtures/check-report/`:

```json
{
  "schema": "fidryn.check-report/v0.1",
  "modules": 8,
  "from_cache": 0,
  "errors": 0,
  "warnings": 0,
  "references": {
    "operative": {
      "exact": 3,
      "target-status": 2,
      "ambiguous": 2,
      "unit-only": 2,
      "range-form": 2,
      "no-unit": 3,
      "outside": 2,
      "malformed": 2
    },
    "enacted": {
      "exact": 1,
      "target-status": 0,
      "ambiguous": 0,
      "unit-only": 0,
      "range-form": 0,
      "no-unit": 1,
      "outside": 0,
      "malformed": 0
    },
    "editorial": {
      "exact": 0,
      "target-status": 0,
      "ambiguous": 0,
      "unit-only": 1,
      "range-form": 0,
      "no-unit": 1,
      "outside": 2,
      "malformed": 0
    }
  },
  "citations": {
    "exact": 0,
    "target-status": 0,
    "ambiguous": 0,
    "unit-only": 0,
    "range-form": 0,
    "no-unit": 0,
    "outside": 0,
    "malformed": 0
  },
  "outside_by_body": {
    "/yy/stat": 2,
    "/zz/other": 2
  }
}
```

`fidryn check DIR --unresolved FILE` writes the unresolved list: every
reference and every citation whose outcome is not `exact`, one object of
compact JSON to a line. The line of a reference has the keys `from`, the
address the reference stands in, `layer`, `target` and `outcome`, in that
order; the line of a citation has `from`, the address of its module,
`citation`, its name, `target` and `outcome`. The lines are sorted by the
bytes of `from`, a reference before a citation of the same `from`, and
then by the bytes of `target`, and the lines of the citations of one
module and one target by the citation's name. Its first lines for the same
workspace:

```text
{"from":"/zz/demo/t1/ch1","layer":"editorial","target":"/yy/stat/116/926","outcome":"outside"}
{"from":"/zz/demo/t1/s6","layer":"operative","target":"/zz/demo/t1/s1/z","outcome":"unit-only"}
{"from":"/zz/demo/t1/s6","layer":"operative","target":"/zz/demo/t1/s2","outcome":"target-status"}
```

Both files are written whether or not the check finds an error, and
`fidryn check` exits 1 when one of them cannot be written.

## References

A reference is a link in a text of a module, `Run::Ref`: the linked words
and the target as the source writes it. A reference needs no import. Its
layer is that of what it stands in, and so is the address it stands in:

| The link stands in | Layer | Address it stands in |
| --- | --- | --- |
| a provision: its number, its heading and its text parts, tables and quoted content included | `operative` | the provision's |
| the number or the heading of a container | `operative` | the module's |
| a note: its heading and its blocks, quoted content included | the note's own, `enacted` or `editorial` | the note's |
| a source credit | `editorial` | the module's |
| a footnote | `editorial` | the module's |

The check of a workspace gives every reference of every module that stands
at the path of its address exactly one outcome, the first of this list
that applies. A module that does not stand there, and a module file that
does not lower, are reported, and their references are left out of the
outcomes and the counts. A beginning of a target is the target cut after
any of its segments, the target itself included: the beginnings of
`/zz/demo/t1/s8/a` are `/zz`, `/zz/demo`, `/zz/demo/t1`, `/zz/demo/t1/s8`
and `/zz/demo/t1/s8/a`. A known address is the address of
a module, a provision or a note of the workspace or of a dependency, or an
alias of such a module; when an address of one is an alias of another, the
address wins. An address that two modules declare is an error (`E700`),
and a reference to it reaches what the module whose file comes first by
path declares.

| Outcome | When |
| --- | --- |
| `malformed` | the target is not an [address](#addresses) |
| `ambiguous` | a beginning `B` of the target is the address of a module, a provision or a note, and so is `B~2`: the source gave one identifier to more than one element, and the reference cannot say which it means |
| `exact` | the target is a known address, and neither what it names nor the unit it lies in has a status |
| `target-status` | as `exact`, but what the target names, or the unit it lies in, has a status, which is recorded: its own, or else its unit's |
| `unit-only` | the longest beginning of the target that is a known address is a unit, or lies in one: a provision, a note of a unit, an alias of a unit |
| `range-form` | the segment of the target after its base holds a dash, the en dash or `-`, and the base followed by that segment cut before its first dash is a known address. The base is the longest beginning that is a known address or, when there is none, the prefix the target lies below. The target is a range of units or a dashed number that is no unit, and the reference cannot say which |
| `no-unit` | the target lies below the prefix of the workspace or of a dependency |
| `outside` | the target lies below no such prefix; it is recorded with its first two segments, the body of law it names |

A target lies below a prefix when it is the prefix itself or continues it
at a segment boundary: in a workspace with the prefix `/zz/demo`, a
reference to the corpus as a whole, `/zz/demo`, is `no-unit`, and
`/zz/demox/t1` lies below no prefix. One of the last two rows always
applies, so no reference is left without an outcome. Of a workspace with
the prefix `/zz/demo`, the units `/zz/demo/t1/s1` and
`/zz/demo/t1/s5`, each with a subsection `a`, `/zz/demo/t1/s2` (status
`repealed`), `/zz/demo/t1/s3` (status `omitted`, alias `/zz/demo/t1/s4`)
and `/zz/demo/t1/s5~2`, and the containers `/zz/demo/t1` and
`/zz/demo/t1/ch1`:

| Target | Outcome |
| --- | --- |
| `/zz/demo/t1/s1/a`, `/zz/demo/t1/s1`, `/zz/demo/t1/ch1` | `exact` |
| `/zz/demo/t1/s2` | `target-status`, `repealed` |
| `/zz/demo/t1/s4` | `target-status`, `omitted`, through the alias |
| `/zz/demo/t1/s5`, `/zz/demo/t1/s5/a` | `ambiguous` |
| `/zz/demo/t1/s1/z`, `/zz/demo/t1/s2/a` | `unit-only` |
| `/zz/demo/t1/s1–9`, `/zz/demo/t1/s1–9/b` | `range-form` |
| `/zz/demo/t1/s9`, `/zz/demo/t9/s1`, `/zz/demo/t1/s8–9` | `no-unit` |
| `/zz/other/x`, `/yy/stat/116/926` | `outside`, with the bodies `/zz/other` and `/yy/stat` |
| `not an address`, `/zz/demo/t1/s1/a ` with a space at its end | `malformed` |

An outcome is never a diagnostic: a reference that reaches nothing does
not fail the check. The check report counts the references of each layer
by outcome, and those of outcome `outside` by body; the unresolved list
names each one whose outcome is not `exact`. The target of every citation
of a curated module gets its outcome by the same rules; the report counts
the citations by outcome, and the unresolved list names each one whose
outcome is not `exact`. A reference or a citation whose target lies where
a dependency that failed its checks leaves nothing known is left out of
both, as [Depending on another workspace](#depending-on-another-workspace)
describes.
`fidryn_provision::AddressIndex` gives the outcome of a target, and also
the tree hash of the provision at an address and the enacted-layer hash of
the unit an address lies in, an alias of a unit standing for the unit,
which the staleness of a [binding](#bindings) compares. A lookup of it
costs the same however many addresses it holds, and a target has few
segments, so the outcomes of all the references of a workspace cost time
in proportion to their number.

## The cache

What phase one gives of a module file depends on the file's bytes alone,
so it does not change while the file does not. The check keeps it in a
cache on disk, and a check that finds there the entry of a file's bytes
takes phase one's result from it, instead of lowering and checking the
module again. Phase two runs in full every time. A check of a workspace
in which nothing changed reads the bytes of each module file, to find its
entry, and the entry, and does none of the rest of phase one.

An entry holds the result of phase one of one file: the BLAKE3 of its
bytes, which phase two compares with the lock; its diagnostics, with
their spans, none of which names the file; and the module's
[interface](#interfaces), or none when the module does not lower. Nothing
that depends on the path of the file, on the lock, on the manifest or on
another module is in it, so two files with the same bytes, in one
workspace or in two, share one entry.

The key of an entry is the BLAKE3 of four frames, built as the
[hashes](#hashes) are:

```text
F(0x00, "fidryn.cache/v1") ‖ F(0x01, UTF-8 of the toolchain's version) ‖ F(0x02, UTF-8 of the identity of the executable) ‖ F(0x03, the file's bytes)
```

The toolchain's version is the one `fidryn --version` prints. The identity
of the executable is the BLAKE3 of the file of the running `fidryn`, as 64
hexadecimal digits: two builds of one version can differ in what phase one
does, and an entry that another build wrote is never read. When that
identity cannot be read, the check keeps no cache, and `fidryn check` says
so once on stderr. A file larger than the largest source of edition 2 is
not kept; the check refuses it in any case.

The entry of a key is the file `v1/<the first two digits of the key>/<key>.json`
in the cache's directory. That directory is the one `--cache-dir` names;
else the one the environment variable `FIDRYN_CACHE_DIR` names, when it is
set and not empty; else `fidryn` in the cache directory of the user,
outside every workspace: on Windows the directory `LOCALAPPDATA` names,
and on other systems the one `XDG_CACHE_HOME` names, else `.cache` in
`HOME`, each taken only when it is an absolute path. When none of them
names a directory, the check keeps no cache, as with `--no-cache`, and
`fidryn check` says so once on stderr. `--no-cache` keeps no cache: the
check then reads and checks every module file and writes no entry. Unless
the cache is named inside it, the check writes nothing into the workspace,
and it reads nothing there but the workspace's own files: a
`.fidryn-cache` or any other entry whose name begins with a dot is not
read, and a symbolic link of any name is not followed.

One cache serves every workspace of a user. An entry is the result of the
bytes of one file, whatever workspace holds the file, so the workspaces
that hold a file of the same bytes share its entry, and the first check of
a workspace may read some of its modules from the cache. The module files
of the dependencies of a curated workspace are read through the same
cache, and the check keeps no cache when the cache's directory and the
directory of a dependency overlap, as
[Depending on another workspace](#depending-on-another-workspace)
describes.

A check trusts its cache. An entry that is what the writer writes is
taken as the result of the bytes it names, and its key is no secret: each
of its frames can be known, and the identity of an executable that is
published can be computed by anyone. Whoever can write in the cache's
directory can therefore make a check pass a module whose hashes do not
match its text, leave a module out without a diagnostic, or hide a stale
binding. A cache belongs where only people one trusts can write, which is
why the default is a directory of the user. A cache named inside the
workspace it serves, with `--cache-dir` or `FIDRYN_CACHE_DIR`, is the
user's choice and is used. Its entries then go with the workspace when it
is copied or committed, and a check of a copy that came from someone else,
with that cache named, takes as results whatever they put there. Its
entries are also files of the workspace, and unless its name begins with a
dot, the writer of a generated workspace refuses the directory for them.

An entry is one line of compact JSON with these keys, in this order:
`schema`, which is `fidryn.cache/v1`; `key`; `digest`, the BLAKE3 of the
file's bytes; `diagnostics`, each as `fidryn_core::Diagnostic` writes it,
with `code`, `message`, `primary_span`, `related_spans` and `suggestion`;
and `interface`, the object that the JSON of an interface holds under
`interface`, or `null`.

Each worker of phase one looks up the entry of the file it reads by the
entry's path; the cache's directory is never listed. An entry is read back
only when it is what the writer writes: its JSON value is the value that
what is read from it writes back, its key is that of its name, its digest
is that of the file's bytes, and its interface names only what it holds,
as `Interface::from_json` requires. Any other entry, one cut short, one
that is not JSON or is JSON of another shape, or one copied from another
key, is ignored: the module is checked again and its entry written again.
An entry that cannot be read is missing in the same way.

No read of a check sees an entry that the same check wrote: the entries of
the files whose entries were missing are written once every file has been
read, one for each key. So two module files of the same bytes in one
workspace are both checked when the cache holds neither, and the count
`from cache` is the same for any number of workers. An entry is written to
a temporary name in its directory and then renamed, so that a reader never
sees half of one, and two checks at once, of one workspace or of two,
which may write the same entry, leave a valid cache; a check may read an
entry that another check wrote while it ran. A failure to write the
cache, such as a full disk or a directory that cannot be made, does not
fail the check: no further entry is written, and `fidryn check` says once
on stderr that the cache could not be written, naming the cache's
directory and the error, and goes on.

The diagnostics, the summary line, the check report and the unresolved
list of a check that reads from the cache are byte for byte those of a
check without one, but for the count `from cache`, as long as nothing but
a check wrote the cache. A first and a second check of a copy of the demo
workspace, with a cache that held none of its files:

```text
modules: 5; from cache: 0; errors: 0; warnings: 0
ok
modules: 5; from cache: 5; errors: 0; warnings: 0
ok
```

## Definitions

The [ingester for the United States Code](#the-ingester-for-the-united-states-code)
extracts definitions from the
provisions of every unit it imports, once the unit's provisions and notes
are read, and writes each in the unit's module as a `definition` block
([Syntax](#syntax)): the provision that holds it, its terms, its kind, the
words that state its reach, the region the reach covers, its basis, and,
where the rules give them, what it borrows its meaning from and a note to
read with it. A container has no definitions. The rules read definitions
written in a closed list of forms and in no other: they do not find every
definition of the Code, and a provision whose text matches none of the
forms is imported as a plain provision, with no definition. A definition
the rules do not find is missing from the module; a definition they find
records its terms, its kind and its reach words as the text has them.

### Where definitions are read

A definition is extracted from the first block of a `chapeau` or `content`
part of a provision of a unit, when that block is a paragraph and its text
matches one of the patterns below, and from nothing else: not from a note,
not from quoted content, and not from a later block of a part. The text of
a paragraph is its plain text: the words of its runs, in order, without its
footnote markers. `Q` is a term in curly quotation marks, `“` and `”`, with
at least one character and neither mark inside it; the term is recorded
without the marks. Wherever these rules speak of a quotation mark, they
mean one of these two characters and no other: a straight `"`, the single
marks `‘` and `’`, and every other quotation mark are characters like any
other. Where these rules say "in any case", the letters `A` to
`Z` are compared as `a` to `z`, and no other character is changed. Where
they speak of words that a text holds, they mean whole words: a word is
bounded by the start or the end of the text or by a character that is not
one of the letters `A` to `Z` and `a` to `z`.

### Verbs

| Kind | Verb forms |
| --- | --- |
| `means` | `means`, `mean`, `shall mean` |
| `includes` | `includes`, `include`, `shall include` |
| `does_not_include` | `does not include`, `do not include`, `shall not include` |
| `borrowed` | `has the meaning given`, `have the meaning given`, `has the meanings given`, `have the meanings given`, `has the same meaning as`, `have the same meaning as`, `shall have the meaning given` |

A verb form stands where its words are followed by the end of the text or
by a character that is not one of the letters `A` to `Z` and `a` to `z`;
where a rule looks for a verb form inside other text, the character before
it is not such a letter either.

### Patterns

1. *Sentence form.* The paragraph begins
   `[Reach ","] ("the"|"The") ("term"|"terms"|"word"|"words"|"phrase") Q { ("," | "and" | "or" | ", and" | ", or") Q } ["," Aside ","] Verb ...`.
   `Aside` is text with no quotation mark and no verb form of the table in
   it.
2. *List form.* The paragraph begins
   `Q { ("," | "and" | "or" | ", and" | ", or") Q } Verb ...`, and it has
   an inherited reach (below).

One space stands between two parts of a pattern, and a comma stands
directly after the part before it: `the term “widget”, with respect to a
gadget, means`, `the terms “widget” and “gadget” mean`. The kind of the
definition is the kind of its verb form.

### The reach

`Reach` is `Opening Object`. `Opening` is one of `As used in`, `In`,
`For purposes of`, `For the purposes of`, `For the purpose of`,
`For purpose of` and `When used in`. `Object` is the text from the opening
to the end of the reach: it is not empty, it holds no quotation mark, and
it holds none of the words `term`, `terms`, `word`, `words`, `phrase`,
`phrases`, `definition`, `definitions`, `following`, `meaning` and
`meanings`, in any case. After `In`, the `Object` must begin with `this` and
a space, or be a single link; a paragraph that opens with `In` and anything
else ("In the case of an individual, the term ...") states a condition, not
a reach. In the sentence form the reach ends at the comma that stands before
`the term` (or `The`, `terms`, `word`, `words`, `phrase`), and no quotation
mark stands before that comma. A paragraph whose text before those words is
anything other than a `Reach` is not extracted.

A single link is an object whose words are exactly the words of one link
of the paragraph.

### A reach chapeau

The chapeau of a provision is a *reach chapeau* when it is one paragraph
whose text is `Reach [[","] LeadIn] Close`, and nothing more. `LeadIn` is
one of `the term`, `the terms`, `the following definitions apply` and
`the following definitions shall apply`. `Close` is a dash (U+2014 or `-`)
or a colon, with at most one space before it. The lead-in and the close are
no part of the reach. `In this section:`, `As used in this chapter, the
term—` and `In this section, the following definitions apply:` are reach
chapeaus; `The following rules apply:`, `Except as otherwise provided, in
this chapter:` and `In this chapter, the following terms have the following
meanings:` are not.

### The reach of a paragraph

A paragraph of the sentence form that begins with a `Reach` has that reach.
Any other paragraph looks to the nearest ancestor of its provision that has
a chapeau. When that chapeau is a reach chapeau, the paragraph has its
reach, an *inherited* reach. When it is any other chapeau, the paragraph is
not extracted: such a chapeau may state a reach in words these rules do not
read. When no ancestor has a chapeau, a paragraph of the sentence form has
no reach, and a paragraph of the list form is not extracted. The reach
words are recorded exactly as the text has them, from the opening to the
end of the object; a definition with no reach has the words `unstated`.

### What is not extracted

A paragraph that matches a pattern is still not extracted when one of these
holds:

- *It defines a second term.* After the `Verb`, the text holds a `Q` that
  is followed by a space and a verb form, or by a comma, a space, text with
  no quotation mark, a comma, a space and a verb form.
- *Its aside speaks of a reach.* The `Aside` begins with `in` and a space,
  or holds `as used in`, `when used in`, `for purposes of`,
  `for the purposes of`, `for the purpose of` or `for purpose of`, in any
  case.
- *A reach follows its verb.* The `Verb` is followed by a comma, a space,
  one of the openings in any case, and a space.
- *It has no reach, and speaks of one.* It has the words `unstated`, and
  its text holds one of the six phrases of the second case, in any case.

A borrowed definition with no link after its verb is not extracted either
(below).

### The region

When the `Object` is `this`, a space and one of `title`, `subtitle`,
`chapter`, `subchapter`, `part`, `subpart`, `division`, `section`,
`subsection`, `paragraph`, `subparagraph`, `clause`, and then either
nothing more or a comma, a space and text that begins `unless the context`,
the region is the nearest ancestor whose kind is that word, counted from
the provision whose text holds the reach (that provision included, then its
ancestors, then its unit's containers in order, the nearest first). When
the `Object` is a single link whose target is an address below the prefix
of the corpus, the region is that target. Otherwise there is no region: a
link to a public law or to the Statutes at Large names no region of the
Code, and an object that says more than these forms allow ("this chapter
and chapter 5", "paragraph (1)") is not resolved.

### The basis

| Region | Unit | Basis |
| --- | --- | --- |
| none | any | `unresolved` |
| an address | its file declares its title positive law (`positive_law true`) | `enacted` |
| an address | any other | `restated` |

### Borrowed definitions

A definition of kind `borrowed` records as `borrowed_from` the target of the
first link that begins at or after the end of its verb form. If there is
none, the paragraph is not extracted, and so it is when that link has an
empty target.

### The note

When the basis is `restated` and the unit has an editorial note whose
heading is "References in Text", one of whose own paragraphs (not one
inside a table or quoted content) holds, in any case, the words that name
the region (`this` and the kind word, or the words of the link) followed by
`, referred to in`, the definition records the address of the first such
note as `see_note`.

### What the values mean

- `unstated`: neither the paragraph nor a chapeau above it states a reach
  in the forms these rules read. A reach stated somewhere else, for example
  in another subsection of the same section, is not read, so a definition
  whose words are `unstated` may still have a reach that the text states
  elsewhere.
- `unresolved`: the rules found no region. That includes reach words that
  name a part of the Code in a form the rules do not resolve, such as "this
  chapter and chapter 5", "subparagraph (A) of this paragraph" or, of a
  court rule, "this Rule 3021", a reach
  that names a law outside the Code, such as "this Act" or a link to a
  public law, and a reach whose kind word names neither the provision that
  states the reach nor any provision or container above it. For an
  inherited reach, the provision that states it is the provision of the
  chapeau.
- `restated`: the file does not declare its title positive law (its flag is
  `no`, or it has none), so the words of the provision are those of the
  editors of the Code, and the reach the law enacted may differ from the
  one they state: a "this chapter" of the Code may stand for "this Act" or
  "this title" of the law the editors classified there. `see_note`, when a
  definition has one, is the address of the first editorial note headed
  "References in Text" one of whose own paragraphs holds, in any case, the
  words that name the region followed by `, referred to in`. Such a note
  often says what the enacted words were; the rules do not check that it
  does.

An invented example: the chapeau of a section of an invented title that is
positive law, and the definitions extracted from its paragraphs.

```text
§ 2. Definitions
In this section:
  (1) Widget.—The term “widget” means a small device.
  (2) “gadget” includes a widget.
  (3) The term “sprocket” has the meaning given such term in section 5 of this title.

definition ".../s2/1"  term "widget"    kind means     words "In this section"  region ".../s2"  basis enacted
definition ".../s2/2"  term "gadget"    kind includes  words "In this section"  region ".../s2"  basis enacted
definition ".../s2/3"  term "sprocket"  kind borrowed  words "In this section"  region ".../s2"  basis enacted
                       borrowed_from the target of the link "section 5 of this title"
```

Of the definitions of a module, which [the canonical form](#syntax) writes
in the order of their provisions and then of their first term, two of one
provision with one first term keep the order of their parts. The checker
holds each to the rules of
[What the checker verifies in one module](#what-the-checker-verifies-in-one-module).

### A second implementation of the rules

`conformance/usc_definitions.py` implements the rules of this section a
second time. It is written from their text, on the independent reading of
the publisher's XML that `conformance/uslm_text.py` prints
([The official XML of the Code](../conformance/README.md#the-official-xml-of-the-code)),
and shares no code with the importer. It writes the definitions it finds in
a release, one JSON object to a line, and with `--against DIR` compares
them with those that the `definition` blocks of a workspace hold.

The check of [the sample](#the-sample), which `cargo xtask ci` runs, holds
the importer's definitions of the sample to it: the two must find the same
definitions in the sample's archives, field for field. That shows that two
implementations of the rules, written apart, read them alike on that text:
a definition that one of them finds, misses or records otherwise by a
mistake of its own fails the check. It does not show that the rules find
every definition the text holds. Both follow the same closed list of
forms, so a definition written in another form is missed by both, and the
comparison cannot see it.

## Curated modules: imports, citations and names

A curated module is written by a person. It gives names to addresses,
binds names to provisions, and imports other modules to use the names they
give. Of three modules of a curated workspace with the prefix `/zz/notes`,
the first cites a subsection:

```fidryn
edition 2
module "/zz/notes/b" {
  citation widget -> "/zz/demo/t1/s8/a"
}
```

The second imports it under an alias, imports a name from a third, cites a
subsection of its own, and binds what the first cites:

```fidryn
edition 2
module "/zz/notes/a" {
  import "/zz/notes/b" as b
  import "/zz/notes/c" { gadget }
  citation sizes -> "/zz/demo/t1/s8/b"
  binding widget_binding {
    binds b.widget
    release "demo-1"
    provision_hash "ec085e67853dd9ced4dafa9836e747dc158ae35ed76f628d9b1eacbd6937be77"
    notes_hash "a396bac23ba81b30cb34dbf808cf06deeaeb4b87e010b66646faadc6accfc6ff"
    pending
  }
}
```

- A **citation**, `citation NAME -> "ADDRESS"`, gives the name `NAME` to
  the address.
- An **import** names a module by its address. Written
  `import "ADDRESS" as ALIAS`, it makes `ALIAS.NAME` name the citation
  `NAME` of that module; written `import "ADDRESS" { NAME, ... }`, it
  brings each citation named in the braces into scope under its own name.
- A **binding** binds what `binds` names: an address, written as a string;
  a name without an alias, which is a citation of the module itself or one
  that an import brings into scope; or `ALIAS.NAME`, a citation of the
  module imported as `ALIAS`. `fidryn_check::e2::names::resolve` gives each
  binding that resolves with the address it binds: `widget_binding` above
  binds `/zz/demo/t1/s8/a`.

The names of a module are one set: the names of its citations and of its
bindings, the aliases of its imports, and the names its imports bring into
scope. A name enters it once: in the module `/zz/notes/a` above, a
citation named `b` or `gadget`, or a second import `as b`, would be an
error.

Only an imported module's own citations are visible through it, and
nothing it imports itself: when `/zz/notes/c` imports `widget` from
`/zz/notes/b`, `c.widget` names nothing, and a module that wants `widget`
imports `/zz/notes/b`. An import therefore leads no further than the
module it names, and two modules may import each other.

The check of a workspace resolves the names of each curated module in
phase two, from the interfaces of the module and of the modules it imports
alone:

| Check | Code |
| --- | --- |
| An `import` names a module of the workspace or of a dependency, by its address | `E720`, at the address |
| A name enters the set of names of its module once | `E200`, where it enters again, with the place where it entered first as a related span |
| Each name in the braces of an `import` is a citation of the module it imports | `E200`, at the name |
| A name after `binds` without an alias is a citation of the module or a name that an import brings into scope, and neither the alias of an import nor the name of a binding | `E200`, at the name |
| In `ALIAS.NAME` after `binds`, `ALIAS` is the alias of an import, and `NAME` a citation of the module it imports | `E200`, at the alias or at the name |

Two citations or bindings of one name are an error of the
[check of one module](#what-the-checker-verifies-in-one-module), and the
check of the workspace does not report them again. Nor does it report a
binding whose name cannot resolve for a reason it reports elsewhere: a
name that enters the set twice, a name in braces that the imported module
does not cite, or an import of a module that is not there. An import of a
module file that does not lower, or of a module that does not stand at the
path of its address, is not reported at all: that file's own diagnostics
say what is wrong. The address that a citation names gets an
[outcome](#references), from the modules of the workspace and of its
[dependencies](#depending-on-another-workspace), and an outcome is never a
diagnostic. What a binding binds is compared with what it was written
against, as [Bindings](#bindings) describes.

## Depending on another workspace

A curated workspace may depend on generated workspaces, so that what its
modules import and cite below their prefixes is looked up in their
modules. Each entry of its manifest's `dependencies` names one:

```json
{
  "schema": "fidryn.workspace/v0.1",
  "name": "notes",
  "kind": "curated",
  "prefix": "/zz/notes",
  "toolchain": "0.3.0",
  "dependencies": [
    {
      "name": "demo",
      "path": "../demo",
      "release": "42-7",
      "lock_digest": "<64 hexadecimal digits>"
    }
  ]
}
```

- `name` names the dependency in diagnostics; no two entries have one.
- `path` is its directory, relative to the directory of the manifest, with
  `/` between its segments; an absolute path is refused.
- `release` and `lock_digest` pin it: the release its lock records, and the
  BLAKE3 of the bytes of its `law.lock.json`, as 64 lower-case hexadecimal
  digits. Any change to the lock, such as a new release of the dependency,
  gives another digest, so the entry names the very release the curated
  workspace was written against.

The check of the curated workspace takes the entries in their order, and
an entry that fails one check is not checked further:

| Check | Code |
| --- | --- |
| The `path` is relative | `E730` |
| No entry before it has its `name` | `E730`, naming the paths of both |
| Its directory holds a generated workspace with a lock | `E740`, saying what it holds instead: no directory, a directory without `workspace.json`, a `workspace.json` that is no manifest, a curated workspace, a generated workspace without a lock, or a lock that cannot be read as one |
| The BLAKE3 of the bytes of its lock and the release of its lock are the `lock_digest` and the `release` of the entry | `E740`, naming both releases and both digests |
| Its prefix neither is, contains nor lies within the prefix of the workspace or of a dependency before it that passed these checks | `E730`, naming the other prefix |
| It has no error of its own | `E740`, saying how many it has |

The last check is the check of the dependency by itself, as
`fidryn check` of its directory runs it: by the rules of any workspace, so
that an entry whose name begins with a dot is not read and no link is
followed, its files are compared with its lock, and each of its module
files goes through phase one, on the worker threads of the check and
through its cache. Its errors, such as a module file that differs from
the lock, a module that does not check or a file of the first edition,
are not printed one by one: one `E740` says how many there are, and
`fidryn check` of the dependency's directory shows them. Its warnings,
such as a `toolchain` other than the running one, are not reported. A
generated workspace has no dependencies of its own, so no chain of them is
followed. Every diagnostic about a dependency names the manifest,
`workspace.json`, and has no span.

A dependency that passes every check joins phase two: an import may name
one of its modules, and a reference or a citation whose target lies below
its prefix gets the outcome that its modules give. A target below no
prefix of the workspace or of a dependency is `outside`. Of the curated
module

```fidryn
edition 2
module "/zz/notes/a" {
  import "/zz/demo/t1/s8" as s8
  citation widget -> "/zz/demo/t1/s8/a"
  citation part -> "/zz/demo/t1/s8/z"
}
```

in a workspace that depends on a generated workspace of the prefix
`/zz/demo` whose unit `/zz/demo/t1/s8` has a subsection `a`, the import
names a module of the dependency, `widget` is `exact`, and `part` is
`unit-only`, the one line of the unresolved list:

```text
{"from":"/zz/notes/a","citation":"part","target":"/zz/demo/t1/s8/z","outcome":"unit-only"}
```

The example in the repository is the curated sample,
`samples/uscode/curated/` ([The sample](#the-sample)): a workspace of the
prefix `/zz/sample` that depends on the generated sample of the Code at
`../workspace`, pinned to its release and to the digest of its lock, whose
one module cites sections, deeper provisions and a note of it, each with
the outcome `exact`, and binds one of the sections.

A dependency that fails a check gives phase two no interface, only its
prefix when the check read the manifest of its directory; the manifest of
an entry refused for its `path` or its `name` is not read. What lies below
the prefix of a dependency that failed, and below no prefix of the
workspace or of a dependency that passed, is then not known; and when a
dependency failed whose prefix is not known (no directory, no manifest, a
manifest that cannot be read, or an entry refused for its `path` or its
`name`), nothing is known below no prefix of the workspace or of a
dependency that passed, since the prefix could be any of it. A reference
or a citation whose target is an address that lies where nothing is known
is left out of the counts of the check report and of the unresolved list,
an import of a module there is not reported, and a binding of an address
there is not judged, since the one diagnostic of the dependency is the
report; a target that is no address is `malformed` all the same. The
check exits 1.

But for one layout that [Limits](#limits) names, the check writes nothing
inside the directory of a dependency, with any cache or with none: the
entries of the module files of a dependency go into the cache of the
check of the workspace, under the keys of their bytes as every entry is,
so that the next check reads them from there. When the cache's directory
and the directory of a dependency that the manifest names, whatever the
checks of the entry find, are the same, or one lies inside the other, the
check keeps no cache at all. The cache's directory lies inside the
dependency's when it is named inside it, such as a `.fidryn-cache` of the
dependency, or inside a workspace that stands in a directory of its
dependency; the dependency's lies inside the cache's when it stands where
the cache writes its entries, at `v1` in the cache's directory, or
anywhere else below it. The check then reads and writes no entry, says so
once on stderr, as
`the cache could not be used, and the check went on without it: DIR: ...`
with the cache's directory, the dependency and which holds which, and goes
on. Its diagnostics, its report and its exit status are then those of a
check with `--no-cache`. The paths are compared with every link and every
`..` resolved, and of a directory that does not exist yet, the part that
exists is resolved and the rest taken as written; a cache's directory that
does not exist yet lies inside a dependency when the part of it that exists
does. Only the two directories themselves are compared: a symbolic link
below the cache's directory that leads into a dependency is followed when
an entry is written through it.

The summary line and the check report are about the workspace that is
checked. `modules` and `from cache` count its own module files, and the
module files of a dependency are counted nowhere; `errors` and `warnings`
count its own diagnostics, among them the one of each dependency that
failed.

## Bindings

A binding of a curated module records what a person read when they wrote
it: the provision it binds, the release of the dependency they read it in,
and two hashes of what they read. Of a curated workspace that depends on
the demo workspace, `conformance/fixtures/workspace/demo` of the release
`42-7`:

```fidryn
edition 2
module "/zz/notes/a" {
  citation sizes -> "/zz/demo/t1/s2"
  binding sizes_binding {
    binds sizes
    release "42-7"
    provision_hash "238c20bcd64758ed0b7f1e2eed5758d78ff4abda212539e8b9e7a07b7301686f"
    notes_hash "a396bac23ba81b30cb34dbf808cf06deeaeb4b87e010b66646faadc6accfc6ff"
    pending
  }
}
```

- What `binds` names is an address of a dependency, written out or reached
  through a citation, as
  [Curated modules](#curated-modules-imports-citations-and-names)
  describes; `sizes_binding` binds `/zz/demo/t1/s2`. An alias of a unit
  stands for the unit, as it does for a [reference](#references), and an
  address wins over an alias.
- `release` is the release of the dependency the binding was written
  against.
- `provision_hash` is the [tree hash](#hashes) of the provision: of its
  number, heading, status and text and of the tree hash of each child, so
  that a change to any word of it or below it changes it.
- `notes_hash` is the hash of the [enacted layer](#layers) of the unit the
  provision lies in, which holds its enacted notes.

Both hashes are 64 lower-case hexadecimal digits, which the check of one
module requires (`E710`). The check of a workspace compares them, in
phase two and from the interfaces alone, with what the dependency holds
now. A binding is **stale**, for the first of these reasons that holds:

| Reason | When |
| --- | --- |
| provision removed or moved | the address is no provision of a dependency: the address of a note, of a container or of nothing in the dependency whose prefix it lies below, an alias of a container, or an address below the prefix of no dependency of the workspace, such as one of the workspace's own |
| provision text changed | the tree hash of the provision at the address differs from `provision_hash` |
| enacted notes of the unit changed | the hash of the enacted layer of its unit differs from `notes_hash` |

A binding whose provision changed in a word is stale, and so is a binding
of the section that holds it; a binding of a sibling is not. A new
enacted note of a unit makes every binding of a provision of that unit
stale. Nothing else makes a binding stale:

- Editorial matter, the source credit, the editorial notes and the
  footnotes and their markers, is in neither hash, so a change to it makes
  nothing stale.
- A note set out under a container enters the enacted layer of no unit.
- The release a binding records is compared with nothing. It is shown in
  the warning beside the release of the dependency, so that a reader sees
  against which release the binding was written; against a dependency of
  another release in which neither hash changed, the binding is not stale.
- A dependency that fails a check makes no binding stale: the bindings of
  what lies below its prefix are not judged, and its one diagnostic is the
  report.

A stale binding is `W750 StaleBinding`, a warning, at the binding's name:
it tells a person to read the provision again. It never changes the exit
status and is counted in `warnings` of the summary line and of the check
report. Its message names the binding, the address, the reason, the
release the binding records and the release of the dependency, and says
what to do. Once the curated workspace above, in `notes/`, is pinned to a
release `42-8` of its dependency in which a word of the section changed,
`fidryn check notes` prints:

```text
notes/zz/notes/a.fr: W750 StaleBinding: the binding `sizes_binding` of `/zz/demo/t1/s2` is stale (provision text changed) in the release `42-8` of the dependency `demo`, and was written against the release `42-7`; read the provision again, and write into the binding the hashes that `fidryn text` prints for it with `--hashes`
```

For `enacted notes of the unit changed` it says to read the provision and
the enacted notes of its unit again; for `provision removed or moved`, to
find where the provision now stands and bind its address; and for an
address below no dependency, that the address lies below no dependency of
the workspace, and to bind a provision of a dependency.

A binding is written with what
[`fidryn text DIR ADDRESS --hashes`](cli.md#text) prints: read the
provision with `fidryn text DIR ADDRESS`, then print its two lines and
write them into the binding after its `release`, the release of the
dependency's manifest:

```sh
fidryn text conformance/fixtures/workspace/demo /zz/demo/t1/s2
fidryn text conformance/fixtures/workspace/demo /zz/demo/t1/s2 --hashes
```

```text
provision_hash "238c20bcd64758ed0b7f1e2eed5758d78ff4abda212539e8b9e7a07b7301686f"
notes_hash "a396bac23ba81b30cb34dbf808cf06deeaeb4b87e010b66646faadc6accfc6ff"
```

The command takes the hashes from the [interfaces](#interfaces) of the
modules, where the check takes them, so a binding written from its output
is not stale against the workspace it was printed from. It prints the
hashes of a provision, or of the unit an alias names, and refuses any
other address, saying what it is. `fidryn_provision::staleness` gives the
reason of a binding against an index of addresses, and
`fidryn_provision::bound_hashes` the hashes a binding records; both name
no corpus.

The curated sample, `samples/uscode/curated/` ([The sample](#the-sample)),
binds one section of the generated sample by the name of its citation,
with the hashes that `fidryn text samples/uscode/workspace /us/usc/t45/s151 --hashes`
prints and the release of the sample, and the check of the sample fails
when that binding is stale.

## Change sets

A **change set** says what changed between two releases of one corpus:
which provisions, notes and containers were added, removed, moved or
amended between two generated workspaces of one prefix, each the import of
one release, by layer, and which bindings of given curated workspaces are
stale against the newer one. [`fidryn changes OLD_DIR NEW_DIR`](cli.md#changes)
prints it as JSON, `fidryn.change-set/v0.1`, whose JSON Schema is
[`change-set-v0.1.json`](../schemas/change-set-v0.1.json), or as the text
of a commit message, under which a release enters a repository of the
imported law.

Each of the two workspaces is read as `fidryn check` reads it
([How a workspace is checked](#how-a-workspace-is-checked)), through phase
one and the cache; a workspace that has an error ends the command with its
diagnostics, and no change set is made. Then the two are compared from
their [interfaces](#interfaces). A unit whose three layer hashes and the
addresses of whose provisions are equal on both sides is not read again,
nor is a container whose hashes are equal. The notes, the containers, and
the units on one side only are compared from the interfaces alone. Only a
unit whose operative or editorial layer or whose provisions differ, a
container whose editorial layer differs, and a unit on one side whose top
provision has the kind and the status of a provision on the other side
only, so that it could have moved, are opened, and their provisions,
credits and footnotes compared. A module file that is opened is held to
the BLAKE3 of the bytes its check accepted, which the lock records: a file
that changed after the check read it ends the command with an `E730` that
names it, and enters no change set.

### The lists

Every list holds addresses, and every list is sorted by the bytes of its
addresses; a moved entry is `{"from": ..., "to": ...}`, sorted by `from`
and then by `to`.

| List | What puts an address into it |
| --- | --- |
| `operative.added` | a provision on the new side only whose parent is on both sides, or the top provision of a unit on the new side only |
| `operative.removed` | the same, of the old side |
| `operative.amended_text` | a provision on both sides whose own words differ |
| `operative.amended_structure` | a provision on both sides whose own structure differs |
| `operative.moved` | a removed and an added provision that are one (below) |
| `enacted_notes.added`, `removed` | an enacted note left unpaired once like-named notes are paired (below), at an address at which the other side has no unpaired note |
| `enacted_notes.amended` | an address at which each side has an enacted note left unpaired once like-named notes are paired, the two of different [note hashes](#hashes) |
| `enacted_notes.moved` | a like-named note paired with one at another address, or a removed and an added enacted note of equal note hashes (below) |
| `editorial.added`, `removed`, `amended`, `moved` | the same, of the editorial notes |
| `editorial.credits_changed` | a unit or a container on both sides whose source credit was added, removed, or changed in its words, its links, its superscripts or its subscripts; a footnote marker in a credit counts for `footnotes_changed` |
| `editorial.footnotes_changed` | a unit or a container on both sides whose footnotes or footnote markers differ in any way: a footnote added, removed or changed, or a marker added, removed, standing in another provision or note, showing another mark or pointing at another footnote |
| `containers.added`, `removed` | a container on one side only |
| `containers.changed` | a container on both sides whose kind, number, heading, status, aliases or members differ |
| `stale_bindings` | a binding of a curated workspace given with `--bindings` that is stale against the new side (below) |

A provision is listed for what is its own, and at one place. Its **own
words** are, in order, the text of each paragraph and subheading of its
own text parts, those in the cells of its tables included, with its links,
superscripts and subscripts, each quoted content whole, and the source of
each image: a link added to words, or taken from them, amends the text. Its
**own structure** is its kind, its number, its heading and its status; its
text parts and children in their order, each text part by its role and by
the kind of each of its blocks, a paragraph, a subheading, a table, quoted
content or an image, and each child by its address; and the shape of its
tables: the group of each row, and the header, the spans and the kinds of
the blocks of each cell. A paragraph that becomes a subheading with the
same words amends the structure, and not the text. A provision may be in
both lists. A provision whose tree hash
changed only because something below it changed is in neither: a word
changed in a subsection lists the subsection, and not its section. A
subsection added to a section is `added`, and the section is
`amended_structure`, since the addresses of its children changed; the
children of the subsection are not listed. A column added to a table
changes its shape, and its words when its cells hold any.

A footnote marker is no word of the text it stands in, so a marker that is
added, removed or renumbered changes `editorial.footnotes_changed` of its
unit or container and nothing in `operative`. Of the top provision of a
unit, its aliases, the container it is within, its source and its
positive-law flag are not compared: a unit that leaves one container for
another lists the two containers, whose members changed, and not the unit.
The credits and the footnotes of a module on one side only are not listed,
since the module is.

### Moved provisions and notes

`added` and `removed` are first reduced to their highest provisions. Among
the entries that remain, a removed and an added provision are one `moved`
entry, and are taken out of the other two lists, when their tree hashes
are equal once each one's own number is left out: the tree hash of
[Hashes](#hashes) computed with the frame of no number, `F(0x04, empty)`,
in place of the provision's number, and the tree hash of each child as it
is (`fidryn_provision::changes::unnumbered_hash`). A section renumbered and
otherwise unchanged has moved, and it carries its children with it: they
are not listed. Each removed provision, in the order of the addresses, is
paired with the first added one, in the order of the addresses, that has
an equal hash and is not paired yet. So the provisions on the old side
only, reduced to their highest, are exactly `removed` together with the
`from` of `moved`, and those on the new side only `added` with the `to`.

Notes are compared by their note hashes, which hold neither the address
nor the layer of a note, in two steps. Like-named notes are paired first.
Two notes of one unit or container are of one **family** when their
addresses differ only in the ordinal that marks like-named notes: a last
segment that is a whole number of 2 or more, written in ASCII digits with
no leading zero, after an address that is itself the address of a note of
the module on that side. Within one module, one layer and one family, an
old and a new note of equal hashes are paired, each old note, in the order
of the addresses, with the first new one, in the order of the addresses,
that has an equal hash and is not paired yet. A pair at one address is
unchanged and is listed nowhere; a pair at two addresses is one `moved`
entry. So a note inserted before two like-named notes is `added` at the
address of the first, and the two, whose text did not change, have moved,
each to the ordinal after its own. The pairing stays within a family,
since many notes are the same words in different units: a note amended to
the words of a note of another unit is `amended`, and has not moved there.

The notes that remain are then compared by address: one at an address that
a remaining note on the other side has is `amended`, and one at an address
that none has is `removed` or `added`. A removed and an added note are one
`moved` entry when their note hashes are equal, paired in the same way as
provisions across the workspace: a note whose address changed and whose
text did not has moved, as the notes of a renumbered section do, and so
does a note whose address names another law while its text is the same.

A note whose text changed has no note of equal hash on the other side, so
it is compared by address, with whatever unpaired note stands at its old
address on the other side. When one does, the address is `amended`, and
what the old note is compared with may be another note: of two like-named
notes before which a note is inserted, an amended first is `amended` at the
first address, where it is compared with the inserted note, and its new
text is `added` at the second. When a paired note took its old address, the
changed note is `removed` there, and its new text is `added` at its new
address: an amended second, after an unchanged first that moved to its
address, is `removed` at the second address and `added` at the third. In
neither case can a change set tell that the note at the new address is the
old one amended, since a note has no identity but its address and its
text.

### Stale bindings

With `--bindings DIR`, each binding of the curated workspace in `DIR` is
judged against the new side by the rule of [Bindings](#bindings),
whatever release that workspace is pinned to and whether or not its pin is
satisfied: the change set answers which bindings would be stale if the
pin moved to the new release. The modules of `DIR` are read by phase one
alone, and an error there ends the command; the names of its curated
modules are resolved against its own modules and those of the new side, a
binding whose name does not resolve is left out, and so is one whose
address lies below another prefix than the new side's. A directory given
twice, the same once its path is resolved, is read once. Each entry names
the curated workspace by the `name` of its manifest, the module that
declares the binding, the binding, the address it binds and the reason; the
entries are sorted by workspace, module and binding, and an entry that two
copies of one curated workspace both give is listed once.

### The two forms

`--json` prints the change set as pretty JSON with two spaces and a final
line end, every key present and every list as above, empty lists
included:

```json
{
  "schema": "fidryn.change-set/v0.1",
  "prefix": "/zz/demo",
  "old": { "release": "v1", "lock_digest": "..." },
  "new": { "release": "v2", "lock_digest": "..." },
  "operative": { "added": [], "removed": [], "amended_text": [], "amended_structure": [], "moved": [] },
  "enacted_notes": { "added": [], "removed": [], "amended": [], "moved": [] },
  "editorial": { "added": [], "removed": [], "amended": [], "moved": [], "credits_changed": [], "footnotes_changed": [] },
  "containers": { "added": [], "removed": [], "changed": [] },
  "stale_bindings": [ { "workspace": "notes", "module": "/zz/notes/a", "binding": "first", "address": "/zz/demo/t1/s8/a", "reason": "provision text changed" } ]
}
```

`lock_digest` is the BLAKE3 of the bytes of the lock of each side.
Without `--json` the command prints the text form, a commit message: a
first line `<prefix>: <old release> to <new release>`; a blank line; one
line of counts for each of the four groups, `operative`, `enacted_notes`,
`editorial` and `containers`; then each list that is not empty, after a
blank line, under its name as its key path and a colon, one entry to a
line: an address, a moved entry as `<from> to <to>`, and a stale binding
as `<address>: <binding> of <module> in <workspace> (<reason>)`. No line
begins with `#`, none ends in a space, and the text ends with one line
end. Between two invented workspaces in which one word of `s8/a` changed,
`s8/b` was renumbered `s8/c` and an enacted note was added to `s3`, with
a curated workspace `notes` whose module `/zz/notes/a` binds `s1`, `s4`,
an alias of `s3`, `s8`, `s8/a` and `s8/b` with the hashes of the older:

```text
/zz/demo: v1 to v2

operative: 0 added, 0 removed, 1 amended_text, 1 amended_structure, 1 moved
enacted_notes: 1 added, 0 removed, 0 amended, 0 moved
editorial: 0 added, 0 removed, 0 amended, 0 moved, 0 credits_changed, 0 footnotes_changed
containers: 0 added, 0 removed, 0 changed

operative.amended_text:
/zz/demo/t1/s8/a

operative.amended_structure:
/zz/demo/t1/s8

operative.moved:
/zz/demo/t1/s8/b to /zz/demo/t1/s8/c

enacted_notes.added:
/zz/demo/t1/s3/note/demoNotes/nolaw/amendment

stale_bindings:
/zz/demo/t1/s4: aliased of /zz/notes/a in notes (enacted notes of the unit changed)
/zz/demo/t1/s8/a: first of /zz/notes/a in notes (provision text changed)
/zz/demo/t1/s8/b: second of /zz/notes/a in notes (provision removed or moved)
/zz/demo/t1/s8: section of /zz/notes/a in notes (provision text changed)
```

Both forms are the same bytes on every run and for every number of
workers. `conformance/usc_compare.py` compares two releases of the
publisher's XML independently, from their text records alone. With
`--against` it checks, for each group, that the addresses on one side only
and the addresses that differ are the unions of the lists of the change
set: the provisions only in the old are `operative.removed` with the
`from` of `operative.moved`, those only in the new `operative.added` with
the `to`, and those that differ `amended_text` with `amended_structure`;
the notes of each layer likewise against `added`, `removed`, `moved` and
`amended`, once it has paired like-named notes as above, by the equality of
their records but for their addresses and their footnote markers, which no
note hash holds, so that a note paired with one at
another address is only in the old at its old address and only in the new
at its new one; the units and containers whose credit or footnotes differ
`credits_changed` and `footnotes_changed`; and the containers `removed`,
`added` and `changed`. So an address in `amended_text` where
`amended_structure` was due, or a wrong pairing of moved provisions, is
not something it can see; the tests of `fidryn-driver` hold each list on
its own. It compares neither `prefix`, nor the two releases and their lock
digests, nor `stale_bindings`. The conformance tests run it on invented
pairs of releases beside the command.

## The ingester for the United States Code

`fidryn-ingest-uscode` imports the United States Code from the official
XML release point published by the Office of the Law Revision Counsel. It
reads the archives of one release, each verified against its retrieval
record, into the units and containers of the model, with the definitions
of each unit that [Definitions](#definitions) describes, which
[`fidryn ingest uscode`](cli.md#ingest) writes as a generated workspace
with the prefix `/us/usc`. It is the one crate that knows the Code's
titles, the publisher's elements and their status values; the command
line names the Code only to choose it, and the checker and the writer of a
workspace know none of them. Its rules follow four principles: no
operative word is dropped; nothing is invented; an element the rules do
not name is refused, not skipped; and what is by rule not imported is
counted.

### What it reads

A release is a directory that holds the publisher's zip archives and,
beside them, nothing but their [retrieval record](#workspaces),
`retrieval.json`. Before it reads a document, the ingester:

- reads the record, and refuses a directory that has none; the record is
  opened only when it is a regular file, so that a symbolic link, a
  directory, a pipe or a device in its place is refused before it is
  opened, and it is read through a bound of 1 MiB, past which it is refused;
- recomputes the size, the SHA-256 and the BLAKE3 of every archive, and
  refuses one that differs from its record, the files the record does not
  list, named in the order of the bytes of their names, and an archive it
  lists that the directory does not hold;
- refuses a member whose name is not `usc`, two digits, an optional `a` or
  `A`, and `.xml`, a member larger than 512 MiB, whatever its header
  declares, and a release in which two archives hold a member of the same
  name.

Each member holds one title, or the appendix of one: `usc05A.xml` holds
the title the publisher designates `5a`. Nothing is written beside the
archives.

A document is refused when:

- it has a document type declaration or an entity declaration;
- its root is not `uscDoc` in the namespace
  `http://xml.house.gov/schemas/uslm/1.0`, or its schema location names a
  file other than `USLM-1.0.15.xsd`;
- its `docPublicationName` is not `Online@` followed by the release of the
  retrieval record, where each en dash (U+2013) of the name is read as a
  hyphen and no other character as another;
- it has more than one property `is-positive-law`, or one that is neither
  `yes` nor `no`;
- an element stands where its class does not allow it, or is in no class
  below; or text stands where only elements may;
- a `ref` has neither an `href` nor the class `footnoteRef`, or has both,
  so that it would be at once a link and a footnote marker; a link, a
  superscript or a subscript stands inside another, or a footnote marker
  inside one; a footnote marker holds an element; a marker names no
  footnote of its module, or two footnotes of one module have one `id`;
  or a footnote holds a `signature`, a `subheading` or a `heading`;
- normalization composes one character out of characters on both sides of
  the boundary of a link, a superscript, a subscript or a marker;
- an identifier holds `~`, or gives no [address](#addresses);
- a level has a second `num` or a second `heading`;
- a span of a table cell is not a whole number written in ASCII digits;
- a `br`, an `img` or a column holds content, or an `img` has an attribute
  other than `src`;
- a `signature` holds anything but `name` and `role`, a footnote that
  stands directly in it included;
- a unit or a container has a second source credit, or a source credit
  stands anywhere but directly in one;
- a cross heading has no topic, or one that is not one word of ASCII
  letters and digits beginning with a letter; or a topic of a note holds a
  white-space character other than U+0020, at which topics are split;
- the law a note cites first is a reference of fewer than four segments,
  or one whose second, third or fourth segment holds a character that an
  address segment cannot hold;
- an element stands more than 64 deep in its module, or a container more
  than 64 deep among the containers of its file: the element of a unit or a
  container stands at depth 1 of its module and each element inside it one
  deeper, except a unit or a container that is a member of a container,
  which begins a module of its own, and the top container stands at depth 1
  among the containers. 64 is the deepest nesting of a module that the
  grammar of edition 2 reads, and a module holds no block deeper than the
  elements it is read from, so the check never refuses a module the
  ingester writes for its nesting; release 119-142 nests at most 17 deep in
  a module;
- the document holds more than 8,388,608 nodes (2^23: elements, runs of
  text, comments and processing instructions, and not attributes), three
  times the 2,703,416 nodes of `usc42.xml` of release 119-142, its largest
  member;
- a `status` is not one of the 19 values of the publisher's schema:
  `proposed`, `withdrawn`, `cancelled`, `pending`, `operational`,
  `suspended`, `renumbered`, `repealed`, `expired`, `terminated`,
  `hadItsEffect`, `omitted`, `notAdopted`, `transferred`, `redesignated`,
  `reserved`, `vacant`, `crossReference`, `unknown`.

A refusal names the archive, the member, the element, the line it starts
on, and the identifier of the element or of its nearest ancestor that has
one.

The flag of every module of a document, its `positive_law`, is the
document's property `is-positive-law`: `true` for `yes`, `false` for `no`.
A document may have no such property, and then its modules have no flag:
they declare no `positive_law`, and the text record of each of its units
has `"positive_law":null`.

### The classes of elements

| Class | Elements |
| --- | --- |
| Document | `uscDoc`; `meta`, read for the release name and the positive-law flag alone; `main`. The top container of a file is the `title` in `main`, or the `appendix` directly under `uscDoc` |
| Container | `title`, `appendix`, `subtitle`, `chapter`, `subchapter`, `part`, `subpart`, `division`, `subdivision`, `article`, `courtRules`, `compiledAct`, `reorganizationPlans`; and a `level` with no unit above it and no text part or `p` among its children |
| Unit | `section`, `courtRule`, `reorganizationPlan`; and a `level` with no unit above it and a text part or a `p` among its children |
| Provision inside a unit | any element of the two classes above, and `subsection`, `paragraph`, `subparagraph`, `clause`, `subclause`, `item`, `subitem`, `subsubitem` and `level`, inside a unit. Its kind is its tag |
| Label | `num` and `heading` of a level |
| Text part | `chapeau`, `content`, `continuation`, `proviso` |
| Block | `p` and `xhtml:p`, read alike; `xhtml:table`; `quotedContent`; `img`; in a note, and in quoted content inside one, `signature`; in a note also any `subheading`, any `heading` other than the note's own (the first `heading` that is a child of the note) at any depth inside it, whether or not the note has a heading of its own, and a text part directly in the note; in quoted content also `note`, and a `heading` or `subheading` directly in it |
| Inline, kept as structure | `ref` with an `href` (a link); `ref` of the class `footnoteRef` (a footnote marker); `sup` and `xhtml:sup`; `sub` and `xhtml:sub` |
| Inline, words kept and markup dropped | `date`, `i`, `b`, `inline`, `xhtml:span`, `xhtml:b`, `xhtml:i`, `xhtml:a`, `name`, `role`; `br` is one space |
| Table structure | `xhtml:thead`, `xhtml:tbody`, `xhtml:tfoot`, `xhtml:tr`, `xhtml:th`, `xhtml:td`; `xhtml:colgroup` and `xhtml:col`, which hold no text, are dropped |
| Note | `notes`, `note`, `sourceCredit` |
| Not imported, counted | `toc` and everything in it; release notes; the label of a note group |

The unit of a text is the outermost unit that holds it: a section that
stands inside another section is a provision of kind `section` of the
outer one. A footnote (a `note` of the type `footnote`) is taken out of
the text where it stands, without closing its paragraph, and becomes a
footnote of its module, numbered in document order; its marker is a run of
the text that gives no words. An `img` is imported as the source its `src`
names, normalized, and empty when it has none; the picture is not in the
archive. Quoted content has the `origin` it names as the source writes it,
and none when the attribute is absent or empty.

A `num` with neither a `value` nor text is absent, and one with a `value`
and no text keeps the value with an empty shown form; a `heading` with no
text is absent, and so is a source credit with no text. A number, a heading
or a credit that holds a footnote marker is never absent: it keeps the
marker, with an empty text when it has no other. A text part that stands
directly in a note, where the publisher sets out a law as loose paragraphs,
gives its blocks to the note, in their place among the note's other
blocks, and its role word (`chapeau`, `content`, `continuation` or
`proviso`) is not kept. Each of these rules decides what a hash holds.

### Notes and their layers

Each `note` in a `notes` element is a note, except a cross heading
(`role="crossHeading"`), which opens a group named by its topic and is not
itself a note. So is each `note` that stands directly in a level and is
neither a footnote nor a release note. A note belongs to the unit or the
container it is anchored in. Its topics are its `topic` attribute split at
U+0020 and no other character, with the empty members dropped: an empty
attribute, or one of spaces, gives no topic, and two spaces between two
words give two topics. Its group is the topic of the nearest cross
heading before it in its `notes` element, and `ungrouped` when there is
none or the note stands directly in a level:

| Group | Layer |
| --- | --- |
| `editorialNotes` | editorial |
| `ungrouped`, with one of the topics `removalDescription` and `historicalAndRevision` | editorial |
| any other: `statutoryNotes`, `execDoc`, `courtRules`, any other `ungrouped` note, and any group word a release introduces | enacted |

Its address is the address of its anchor followed by
`/note/<group>/<law>/<heading>`:

- `<law>` is the `origin` of the note's first quoted content when it
  begins `/us/pl/`, `/us/act/` or `/us/stat/`; otherwise the `href` of the
  first link that begins `/us/pl/` or `/us/act/`, taking the note's
  heading and then its blocks in order, where a link in a footnote and a
  link with no words do not count. Of that reference the first four
  segments are kept and the last three joined by full stops:
  `pl.999.2`, `act.2099-07-30.ch9`, `stat.999.10`. A note with neither has
  `nolaw`.
- `<heading>` is the text of the note's heading with the letters `A` to
  `Z` put in lower case and every run of characters other than ASCII
  letters and digits written `-`, without `-` at either end, cut to 60
  characters and again without `-` at its end.
- Of a note with no heading, or whose heading leaves nothing by that
  rule, `<heading>` is `noheading` when it has a law, and, when it has
  neither, `h.` followed by the first 16 hexadecimal digits of its hash.
- When several notes of one anchor agree in all three parts, the second in
  document order gets `/2`, the third `/3`.

### How it gives addresses

An address is the publisher's identifier except for the later holders of
a repeated identifier, the elements without one, and what stands inside
either:

1. **A unit or a container with an identifier.** An identifier that holds
   a space is a list: its first member is the identifier used, and the
   other members are aliases. An identifier whose last segment holds three
   full stops in a row stands for a range, as `s44...46` does: it is used
   as it is, and its aliases are its two ends, the identifier cut before
   the full stops and the identifier with its last segment replaced by
   that segment's leading lower-case letters followed by what stands after
   the full stops. When the two ends differ only in a whole number at their
   end, the second not smaller than the first and at most 999 above it,
   every number between them gives an alias as well; a member of a range
   whose number is followed by a letter gets no alias. The address is the
   identifier, unless a unit or a container before it in the release,
   taking the members in the order of their names and the elements in
   document order, already has it; then it is the identifier followed by
   `~2`, by `~3` for the next, and so on.
2. **A unit or a container without an identifier.** The address of the
   container it stands in, or the prefix for the top element of a file,
   followed by `~<tag>.<n>`, where `n` counts from 1 the elements of that
   tag without an identifier in that place, in document order.
3. **A provision inside a unit.** The address of its parent followed by
   the last segment of its identifier, of its first member if it lists
   several, and by `~2`, `~3` when a sibling before it already has that
   address; without an identifier, followed by `~<tag>.<n>` as in rule 2.
4. An alias that is the address of a unit, a container or a provision is
   dropped, and so is one the element already has.

The count of rule 1, and that of rule 2 for the top element of a file,
run over every member of the release that the archive directory holds,
in the order of their names, whichever titles are written: an address
depends neither on the titles chosen nor on the number of workers. The
lock's `addresses` counts the elements whose address got `~2`, `~3` by
rule 1 or 3 (`disambiguated`), the elements whose own segment is
`~<tag>.<n>` (`assigned`), and the aliases kept (`aliases`). What stands
inside such an element is not counted again.

### What it does not import

Some of what a document holds is by rule not imported, and the lock's
`not_imported` counts it:

| Not imported | Counted as |
| --- | --- |
| a table of contents, `toc`, and everything in it | `tables_of_contents`; and the footnotes and links in it, as `footnotes_in_tables_of_contents` and `links_in_tables_of_contents` |
| a release note: a note directly under a `title` or an `appendix` whose text begins `Current through ` | `release_notes` |
| a link that has no words, or white space alone | `links_without_words` |
| the label of a note group, the heading of a cross heading | `note_group_headings` |

What a cross heading holds after its heading is imported, as a note of the
group it opens.

### What the command writes

`fidryn ingest uscode ARCHIVE_DIR --out DIR` writes the workspace with
`GeneratedWriter` ([Writing a generated workspace](#writing-a-generated-workspace)):
the manifest, of the name `uscode`, the kind `generated`, the prefix
`/us/usc`, the release of the record and the running toolchain's version;
a module for each unit and container of the titles chosen, every title by
default; the lock, whose generator is `fidryn-ingest-uscode` at the
same version, whose artifacts are the archives and members whose modules
the run wrote, as the record gives them, and whose counts are those above;
and last the [coverage ledger](#the-coverage-ledger) of the workspace.
The numbering of addresses still runs over every member of the archive
directory, as [How it gives addresses](#how-it-gives-addresses) says.

```text
fidryn ingest uscode ARCHIVE_DIR --out DIR
ingested: U units, C containers from A archives (release R)
```

The bytes it writes depend on the archives, on the titles chosen and on
the toolchain's version, which the manifest and the lock record, and on
nothing else. No time is written but the retrieval times the record
gives, and no path but the paths of the files inside the workspace;
neither the host, its user, its time zone, the working directory nor the
number of workers changes a byte. Two documents that differ only in their
element ids, their publication name, the creation time of their metadata
and their release note give the same modules, and locks that differ only
in `release` and `artifacts`.

## The coverage ledger

A generated workspace holds at its top its coverage ledger,
`coverage.json`, `fidryn.coverage/v0.1`: what the workspace holds, in
counts. The [writer](#writing-a-generated-workspace) writes it last, after
the lock, through a temporary file like every file of a run, and the lock
does not list it. [`fidryn coverage DIR`](cli.md#coverage) prints it,
counted again from the files of the workspace as they stand; the two are
the same, byte for byte, while no file is changed. No check reads it. Its
JSON Schema is [`coverage-v0.1.json`](../schemas/coverage-v0.1.json).

`fidryn_provision::records::coverage` counts it from the
[interfaces](#interfaces) of the modules, the index of their addresses
below the prefix, which gives each reference its [outcome](#references) as
the check of the workspace does, the manifest and the lock, and from
nothing else: it reads no module's source and nothing outside the
workspace. A unit and a container are counted, with their notes,
footnotes, references and definitions; a curated module is counted
nowhere. The keys, in their order:

| Key | Counts | From |
| --- | --- | --- |
| `schema` | `"fidryn.coverage/v0.1"` | |
| `prefix` | the prefix of the workspace | the manifest |
| `release` | the release of the workspace | the lock |
| `totals.units` | the unit modules | the interfaces of the shape `unit` |
| `totals.containers` | the container modules | the interfaces of the shape `container` |
| `totals.provisions` | every provision of every unit, the unit's own top provision included | the provisions of the interfaces of units |
| `totals.units_by_kind` | the units by the kind of their top provision | the kind of the first provision of a unit's interface |
| `totals.units_by_status` | the units by the status of their top provision, `in-force` for one with none | its status |
| `totals.containers_by_kind` | the containers by their kind | the kind of the container |
| `totals.addresses` | `aliases`, `disambiguated` and `assigned`: the aliases the importer kept, and the elements whose address it disambiguated or whose segment it assigned | the lock's `addresses`, copied |
| `totals.notes` | the notes of the units and the containers, `enacted` and `editorial`, by their layer | the layer of each note |
| `totals.notes_by_group` | the same notes by their group | the group of each note |
| `totals.footnotes` | the footnotes of the units and the containers | the number of footnotes of each interface |
| `totals.with_images` | the provisions and the notes that hold an image, whose picture is not in the archive the text was read from | the `image` of each provision and note |
| `totals.references` | the references of each layer, `operative`, `enacted` and `editorial`, by outcome, each of the eight outcomes for each layer, as the check report counts them | each reference, its outcome looked up in the index |
| `totals.outside_by_body` | the references of the outcome `outside`, of every layer, by the body of law they name | the same |
| `totals.definitions` | the definitions extracted, by [basis](#the-basis): `enacted`, `restated` and `unresolved` | the definitions of the interfaces of units |
| `totals.definitions_by_kind` | the definitions extracted, by kind: `means`, `includes`, `does_not_include`, `borrowed` | the same |
| `totals.not_imported` | what the importer counted and did not import, by the six names of the lock | the lock's `not_imported`, copied |
| `by_source` | one row for each source file of a unit or a container, sorted by the bytes of `source`: `source`; `positive_law`, `true`, `false`, or `null` for a file with no flag; and the `units` of the file, their `provisions` and their `definitions` | the `source` and `positive_law` of each interface |

The ledger is written with two spaces of indentation, one member to a
line, an empty object written `{}`, and a final line end. The keys of a
map whose keys are words of the workspace, a kind, a status, a group, a
body of law or a kind of definition, are sorted by their bytes, and a word
is listed only when something is counted under it; the outcomes stand in
the order of [References](#references). A file of an appendix is a row of
its own, as every other source is, and nothing merges its counts into
those of the title it accompanies. The flag of a row is that of the module
of the file whose address comes first in the byte order of addresses,
whatever the order in which the modules are read; every module of a file
the importer reads has the file's flag, so they agree, and when they do not
that module's flag is the row's. Nothing of the host, the clock or the
number of workers reaches the ledger, and an estimate of the definitions
the rules miss is not in it: the ledger holds only what is counted from the
workspace.

The ledger of the invented demo workspace is
[`conformance/fixtures/workspace/demo/coverage.json`](../conformance/fixtures/workspace/demo/coverage.json),
and that of the sample of the Code
[`samples/uscode/workspace/coverage.json`](../samples/uscode/workspace/coverage.json),
whose figures [Coverage](coverage.md) shows.

## The sample

The repository holds a sample of the Code under `samples/uscode/`
([its README](../samples/uscode/README.md)): the publisher's archives of
three whole titles of the release `119-142` of the official XML release
point published by the Office of the Law Revision Counsel, Title 1, Title
11 Appendix and Title 45, and what is imported from them.

- `archives/` holds the archives, exactly as served, and their retrieval
  record, `retrieval.json`.
- `profile.json` holds the counts of the archives that
  `conformance/usc_profile.py` prints, by the counting rules of
  [usc-counting-rules.md](../conformance/usc-counting-rules.md).
- `workspace/` holds the generated workspace that
  `fidryn ingest uscode samples/uscode/archives --out DIR` writes: its
  manifest, its lock, its [coverage ledger](#the-coverage-ledger), and a
  module for each of the 563 units and 72 containers of the three titles,
  638 files in all. It is written by `cargo xtask uscode-sample --update`
  and never by hand. The page [Coverage](coverage.md) shows the figures of
  its ledger.
- `curated/` holds a curated workspace written by hand that
  [depends](#depending-on-another-workspace) on `workspace/`, at
  `../workspace`, pinned to its release and to the BLAKE3 of its lock. Its
  one module gives names to provisions of the sample, a whole section, a
  subsection and deeper provisions, a rule of the appendix title and a
  note, and [binds](#bindings) one section, with the release of the sample
  and the two hashes that `fidryn text --hashes` prints for it; it holds no
  text of the Code. `fidryn check samples/uscode/curated --no-cache` checks
  it with its dependency.

`cargo xtask uscode-sample`, which `cargo xtask ci` runs, holds the sample
to nine things, in this order, and fails with a message that names the
first difference when one of them does not hold:

1. every archive has the size, the SHA-256 and the BLAKE3 that its record
   gives, and the record names every archive and no other;
2. an ingest of the archives into a temporary directory writes, byte for
   byte, every directory and file of the committed workspace, its ledger
   among them, and nothing else;
3. an ingest on one worker writes the same bytes as an ingest on eight;
4. `fidryn check samples/uscode/workspace --no-cache` finds no error, and
   `fidryn coverage samples/uscode/workspace` prints the committed
   `coverage.json`, byte for byte;
5. `profile.json` is, byte for byte, what the script prints for the
   archives; its counts of references by layer and outcome, which the
   script works out by the rules of [References](#references) on its own,
   are those of the check report; and every count that the profile's
   `import` and the ledger's `totals` both hold is the same in both, from
   the units and the provisions to the notes, the images and what was not
   imported. Only the ledger counts definitions, `definitions` and
   `definitions_by_kind`, since that script extracts none, and the
   profile's `prefix` and `release` stand at the top of the ledger, where
   they are compared;
6. the text of every provision, container, note, source credit and
   footnote that `fidryn text samples/uscode/workspace --json` reads back
   from the modules equals, line for line, what `conformance/uslm_text.py`
   reads from the publisher's XML by the [rules of the reading](#the-ingester-for-the-united-states-code),
   which it implements on its own, sharing no code with the ingester;
7. the definitions that `conformance/usc_definitions.py` finds in the
   archives, by its own implementation of the [rules](#definitions), are
   those that the `definition` blocks of the committed workspace hold,
   field for field
   ([A second implementation of the rules](#a-second-implementation-of-the-rules));
   the script names the first definition that differs;
8. the check of `curated/` with its dependency finds no error and no
   warning, a pin that is no longer the lock's (`E740`) among the errors
   and a stale binding (`W750`) among the warnings, counts at least one
   citation, and gives every citation of it the outcome `exact`; the step
   names the file and the first thing that is wrong, and the curated
   sample when it counts no citation;
9. the table of [Coverage](coverage.md), between the markers
   `<!-- coverage-table:begin -->` and `<!-- coverage-table:end -->`, is
   the one the committed ledger gives.

The `fidryn` commands run in the process of the step, as the binary runs
them. The step makes its temporary directories under the system's
directory for temporary files and removes them whether it passes or fails,
and its checks keep no cache, so it leaves the repository as it found it:
only `--update` writes, the workspace, then, in the curated manifest,
the release and the lock digest of its dependency and its `toolchain`,
and then the table of `docs/coverage.md` between its markers, and nothing
else; it never touches the curated module. The ledger is not listed in
the lock, so writing it moves no pin. When `--update` changes the table of
`docs/coverage.md`, `cargo xtask site` is run after it to render the page
again, since `cargo xtask ci` fails while the site is not what the
documents give.

```text
cargo xtask uscode-sample
cargo xtask uscode-sample --update
```

## Limits

Each of these is true of the code at this version. They say what the code
does and does not do where a reader of the sections above could expect
more.

### Workspaces and the cache

- A dependency of a curated workspace is a generated workspace, named by
  a path relative to the directory of the manifest. A curated workspace
  cannot depend on another curated workspace (`E740`), and a dependency
  named by an absolute path is refused (`E730`).
- A command reads a manifest, a lock and an entry of the cache only from a
  regular file of at most 64 MiB (67,108,864 bytes), more than nine times
  the lock of the import of the whole Code. A symbolic link, a named pipe,
  a device or a directory in the place of a manifest or a lock is refused
  before anything is opened, as the check refuses a symbolic link, and a
  larger file is refused, with a sentence that gives the bound, without
  being read; such an entry of the cache is a miss, and an entry that would
  be larger is not written.
- A check trusts its cache: an entry written by hand under the key of a
  module's bytes is taken as that module's result. A cache belongs where
  only people one trusts can write, and the default is a directory of the
  user, outside every workspace ([The cache](#the-cache)).
- The check compares the cache's directory with the directory of each
  dependency, and not what lies below the cache's directory. When a
  symbolic link there, such as a `v1` that leads into a dependency, is
  followed, the check writes its entries inside the dependency.
- The writer of a generated workspace tells an earlier output from another
  `.fr` file by its head alone. A module of the prefix at its own path
  whose head lies past its first 64 KiB, after comments, is refused,
  though the check would read it; and a module that a person changed below
  its head is an earlier output, which a run writes again or removes.
- A check that is killed while it writes an entry of the cache leaves the
  temporary file of the entry, whose name begins with a dot and ends in
  `.tmp`, in the cache. Nothing reads it, and nothing removes it.
- A module file name such as `s1932~2.fr` has the shape of a Windows 8.3
  short name. On a volume that gives files such short names, it could be
  the short name of another file of the same directory.

### Diagnostics

- `fidryn check` prints a diagnostic of a module of edition 2 as
  `FILE: CODE Title: message`, with no line and no column: the span the
  diagnostic is about is recorded and not printed. A message that names
  what it is about, such as the provision whose hash does not match, says
  where it is; "text is not normalized" does not.
- `E740` has the title `PinMismatch` also when a dependency fails for
  another reason than its pin: no directory, no manifest, a curated
  workspace, no lock, or errors of its own.

### Modules written by hand

- A module's definitions, its aliases, `within`, `source` and
  `positive_law` are in no hash of the module ([Hashes](#hashes)), and the
  check of a definition holds its form and not its region, its basis or
  its note
  ([What the checker verifies in one module](#what-the-checker-verifies-in-one-module)).
  In a generated workspace the lock holds the BLAKE3 of the bytes of every
  module file, so a module edited by hand there fails the check (`E730`)
  unless the lock is written again with it, and then the digest of the
  lock changes, which a curated workspace that depends on it reports
  (`E740`). A module checked by itself, and a module of a curated
  workspace, which has no lock, pass the check with a definition whose
  region, basis or note was edited by hand.
- `fidryn text` prints the strings of a module as they are. Every control
  character that is no white space passes normalization and the check:
  U+0000 to U+0008, U+000E to U+001F, U+007F, and U+0080 to U+009F but
  U+0085. Among them is the escape, U+001B, that begins the sequences a
  terminal acts on, and `fidryn text` writes such a character to the
  terminal as it is. A module written by hand can hold any of them. The
  importer writes none below U+0020, since XML cannot hold one, and keeps
  U+007F and the C1 controls but U+0085 where the publisher's XML holds
  them.

### Definitions

- The rules read definitions in a closed list of forms and in no other
  ([Definitions](#definitions)). They do not find every definition, and
  the share of the definitions they miss has not been measured.
- `unstated` is given when neither the paragraph nor a chapeau above it
  states a reach. A reach stated in a sibling provision, such as "In this
  section" as subsection (a) with the definitions in subsection (b), is
  not seen.
- An exception written inside the text of a definition, such as "means a
  widget, except in section 5", does not narrow its region: the region is
  that of the reach alone.
- The importer and `conformance/usc_definitions.py`
  ([A second implementation of the rules](#a-second-implementation-of-the-rules))
  record two rare forms differently. A release that holds one shows the
  difference only when the two are compared on it, as the check of the
  sample compares them for the sample:
  - a reach whose object begins with `this` and a space but names no kind
    of [The region](#the-region), and is a single link to an address below
    the prefix of the corpus, as a link whose words are `this Act` would
    be: the script records the link's target as the region, as the rules
    say, and the importer records no region;
  - a restated definition whose unit has two notes that
    [The note](#the-note) could point at, one at a provision and one at a
    provision below it, where the higher provision's note stands first in
    the document: the importer points at that note, and the script at the
    lower provision's, since the independent reading gives the notes of a
    provision after those of the provisions below it.

### Change sets

- The credit and the footnotes of a unit or a container that is on one
  side only, and of a unit that moved, are in no list.
- Where a footnote marker stands inside its provision is in no hash, and
  so in no list: a marker that moves within one provision changes nothing
  that a change set lists.
- A superscript that the source does not mark as a footnote marker is
  text, even when it shows the number of a footnote. A provision that
  gained only such a superscript is in `operative.amended_text`, and a
  binding of it, or of a provision above it, goes stale. The cost is a
  false alarm, never a missed change.
- A change of the aliases of a unit, of the container a unit or a container
  stands in, or of the positive-law flag of a file, is in no list; a unit
  that moves to another container lists the two containers, whose members
  changed. A title enacted as positive law between two releases, with no
  other change, gives a change set in which every list is empty, and the
  bases of its definitions, which follow the flag, are in no list either:
  definitions are in no hash.
- Like-named notes are paired by equal note hashes alone, and a note has no
  identity but its address and its text. A note of a family whose text
  changed while another was inserted before it is compared with whatever
  unpaired note stands at its old address on the other side: that address
  is `amended` when one does, though what it compares may be the old note
  and the inserted one, and the note is `removed` there when a paired note
  took the address; its new text is `added` at its new address. No change
  set says that the note at the new address is the old one amended. Units
  and containers whose addresses `~2`, `~3` mark as later holders of one
  identifier are not paired this way: they are compared by address.
- The addresses of a family are taken in the order of their bytes on both
  sides, so `…/10` comes before `…/2`: in a family of ten or more
  like-named notes, notes of equal text pair in that order, and one may be
  listed as moved from `/2` to `/11` where pairing by the order of the
  ordinals would leave it in place.
- `conformance/usc_compare.py --against` checks the unions of the lists,
  as [Change sets](#change-sets) says. It cannot see a provision in the
  wrong one of two lists whose union it checks, or a wrong pairing of a
  move, and it compares neither the prefix, the two releases nor
  `stale_bindings`. The tests of `fidryn-driver` hold each list on its
  own.

### The ingester and the independent reading

- The ingester and `conformance/uslm_text.py` read nine forms of the
  publisher's XML differently. No release of 2026 holds any of them.
  - On five of them one of the two refuses the document, so a release
    that holds one is refused by that reading:
    - a `continuation` or a `proviso` inside a `content` that stands
      directly in a note, which the ingester reads as blocks of the note
      and the independent reading refuses;
    - a note inside a paragraph or a text part of quoted content, rather
      than directly in the quoted content or in one of its levels, which
      the ingester refuses and the independent reading reads;
    - white space inside a `br`, an `img` or a column, which the ingester
      refuses as content and the independent reading takes for none;
    - an identifier of a unit or a container whose list has an empty
      member, as two spaces or a space at either end make, and an
      identifier of a provision that begins with a space, which the
      ingester refuses as no address and the independent reading reads
      without the empty member;
    - a `class` attribute of a `ref` whose words are separated by a
      white-space character that is not ASCII. The ingester splits the
      attribute at ASCII white space only, and the independent reading at
      any, so one of them finds the class `footnoteRef` where the other
      does not, and one of them refuses the `ref`: as having neither an
      `href` nor that class, or as having both.
  - On four of them both read the document, and read it differently:
    - a `num` or a `heading` among the words of a level of quoted content,
      where the independent reading ends the open paragraph and the
      ingester does not;
    - a footnote inside a footnote, which the ingester counts as a footnote
      of its own and the independent reading leaves out;
    - a table of contents inside another, which the ingester counts twice
      among the tables of contents of the lock and the independent reading
      once; no text differs;
    - a footnote inside the release note of a title or an appendix, which
      the ingester keeps as a footnote of that container and the
      independent reading leaves out with the note.

    A release that holds one of these four is imported and checked without
    a word. The difference shows only when the two readings are compared on
    that release: what `conformance/uslm_text.py` prints for its archives
    against what `fidryn text DIR --json` prints for its workspace, and, for
    a table of contents, the counts of `conformance/usc_profile.py` against
    the ledger. The check of [the sample](#the-sample) makes both
    comparisons for the sample; neither an import nor the replay of releases
    makes them.
- The independent reading refuses the nesting the ingester refuses, by the
  same rule, but has no bound on the number of nodes of a document: it
  reads a document the ingester refuses for its size, given the memory,
  about 3 GB for a member of 64 MiB of empty elements.
- A table of contents is not imported. The lock counts it
  ([What it does not import](#what-it-does-not-import)).
- No display citation is derived. The address is the one citation a module
  records, and no form such as "1 U.S.C. § 8(b)" is made from it.

### Fetching

- `tools/fetch_uscode.py` follows a redirect only to the host it asked,
  over `https`, and refuses any other, naming where it pointed. The
  retrieval record names the address the tool asked for, not the one a
  redirect led to.
- It takes at most `--max-bytes` bytes in one response, 536,870,912
  (512 MiB) unless that says otherwise, a bound with room above the
  largest archive the publisher offers, the archive of every title, and it
  refuses a larger response as soon as it has read one byte more.
