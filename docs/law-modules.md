# Law modules

This reference gives the rules by which a module of law, and each provision
and note in it, is named, where the file of a module stands in its
workspace, how the text of a module is normalized, what a module holds, how
its provisions, notes and layers are hashed, how a module is written, what
the checker verifies of one module, how modules form a workspace and what
the checker verifies of one, how a workspace is checked, and what outcome
each reference has. The crate `fidryn-provision` implements the model and
the rules over it, `fidryn-syntax` reads the text of a module,
`fidryn-check` lowers it into the model and checks it, and `fidryn-driver`
checks a workspace and writes a generated one. In the examples, `/zz/demo`
is an invented corpus.

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
  provision and one of the notes.

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
note to read with it (`see_note`). Definitions are in no hash.

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

## Edition 2

A law module is a file of edition 2. Its first two significant tokens are
the word `edition` and the integer `2`; white space, line ends and comments
before them are skipped. A file whose first significant token is anything
else is a file of the first edition, which carries no marker, and is read as
before. A file that begins with the word `edition` followed by anything
other than `2` is refused with `E100`: "unknown edition `3`; this toolchain
reads edition 2 after the word `edition`, and files of the first edition
carry no marker".

A module of edition 2 holds provisions of law as data, and nothing in it is
evaluated. `fidryn_syntax::edition::edition_of` tells the edition of a
file, and each entry point asks it before it reads the file:

| Entry point | Given a module of edition 2 |
| --- | --- |
| `fidryn check FILE` | the checks of one module, as [below](#what-the-checker-verifies-in-one-module), then `ok` |
| `fidryn check DIR` | the checks of the workspace in the directory, every module of it included, as under [Workspaces](#workspaces) |
| `fmt`, `inputs`, `run`, `explore`, `explain`, `verify`, `diff`, `render`, `review` and `file` | exit 1 with `E100`: "an edition 2 module holds provisions and is not compiled for evaluation; `fidryn check` reads it" |
| `fidryn_cli::compile_source` and `compile_module`, and so the source pasted into the mill of `ui` | the same `E100` |
| the documentation check, `cargo xtask docs` | a fenced block, untagged or tagged `fidryn`, whose first line that is not blank is `edition 2` is a complete module and gets the checks of one module |

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
| A definition names a provision of the unit; each term stands in that provision's text; the reach words, when stated, stand in its text or in the chapeau of its parent; the kind is `means`, `includes`, `does_not_include` or `borrowed` and the basis `enacted`, `restated` or `unresolved`; a `borrowed` definition has `borrowed_from` | `E710` |
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
read it, nor anything in it, and the writer leaves it alone, so a
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
  "toolchain": "0.2.0",
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
  BLAKE3 of the bytes of its lock.
- `toolchain` is the version of the toolchain the workspace was last
  checked with.

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
    "version": "0.2.0"
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
  `coverage.json` at its top, and anywhere any entry whose name begins with
  a dot, which the run leaves as they are. Any other directory, a curated
  workspace, a generated workspace of another prefix and one that holds a
  symbolic link of another name among them, is refused, and nothing in it
  is changed. `create` writes `workspace.json` at once, through a temporary
  file like every file of the run, and clears the temporary files that an
  earlier run which was stopped left behind. One entry whose name begins
  with a dot is the writer's own and is not left alone: `.fidryn-write` at
  the top of the workspace, the directory of the temporary files of a run,
  which `create` clears and `finish` removes.
- `write(module)` writes a unit or a container whose address is below the
  prefix, in the canonical form, to its path, through a temporary file
  that is renamed. It may be called from several threads at once, and it
  fails when the run has already written a module to that path, so that
  no module replaces another.
- `finish(generator, artifacts, addresses, not_imported)` removes every
  `.fr` file the run did not write, and each directory that leaves empty,
  then writes `law.lock.json` last, and returns the lock.

What a run writes depends only on its modules, its manifest and what
`finish` is given: neither the order of the calls nor the number of
threads changes a byte. A run that is stopped before `finish`, by an error
or because it was killed, leaves a generated workspace of the same prefix
whose lock is missing or is that of the run before it. `fidryn check`
reports it with `E730`, and the next run accepts the directory and, when it
finishes, puts it right.

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
| The directory has a `workspace.json`, and it is a manifest | `E730` |
| A generated workspace has a `release`, no dependencies and a lock, whose prefix and release are the manifest's; a curated workspace has neither a `release` nor a lock | `E730` |
| In a generated workspace, every module file is listed in the lock with the BLAKE3 of its bytes, and every file the lock lists is present | `E730` |
| Every `.fr` file is a module of edition 2; every entry of the directory whose name does not begin with a dot is a file or a directory, and none is a symbolic link | `E730` |
| Every module's address is below the prefix, and the module stands at the path of its address | `E730` |
| A unit's or a container's `within` names a container of the workspace that lists it as a `member`, and every `member` of a container is a module whose `within` is that container | `E720` |
| The manifest's `toolchain` is the version of the running toolchain | `W760`, a warning |

When the directory has no manifest that can be read, that is the one
diagnostic, and nothing else of the directory is read. A module file that
does not lower, a module that does not stand at the path of its address,
and a file the lock lists that is missing, take no part in the checks of
`within` and members, and none of their addresses is one that a reference
reaches: their own diagnostics say what is wrong, and a container that
lists them is not reported for it.

Every diagnostic names its file by its path in the workspace. One about a
file as a whole, such as a module file the lock does not list or a file of
the first edition, has no span. One about where a module stands is at its
address; one about a `within` is at the string after `within`, and one
about a member at the string after that `member`. The diagnostics are
sorted by the path of their file and then by position, a diagnostic about
the file as a whole first.

`fidryn check DIR` prints each diagnostic as
`DIR/PATH: CODE Title: message`, then one summary line, and then `ok` when
there is no error. It exits 1 when there is an error, and 0 otherwise, a
warning included. The
invented demo workspace under `conformance/fixtures/workspace/demo` is
what the writer writes from one title, one chapter and three sections;
the tests of `fidryn-driver` hold it to that. Checked with `--no-cache`,
which keeps no [cache](#the-cache) and so writes nothing into the
repository:

```sh
fidryn check conformance/fixtures/workspace/demo --no-cache
```

```text
modules: 5; from cache: 0; errors: 0; warnings: 0
ok
```

`modules` counts the module files read, every `.fr` file of the workspace,
and `from cache` how many of them were read from the cache rather than
checked again.

## How a workspace is checked

The check of a workspace runs in two phases.

1. **Phase one**, for each module file by itself: the BLAKE3 of its bytes,
   its edition, the grammar, lowering, the checks of
   [one module](#what-the-checker-verifies-in-one-module), and the
   module's [interface](#interfaces). What it gives of a file depends on
   the file's bytes alone, so it is kept in the [cache](#the-cache), and a
   file whose bytes were checked before is not checked again.
2. **Phase two**, with the result of every module in memory: the BLAKE3 of
   each file against the lock of a generated workspace, and where each
   module stands, below the prefix and at the path of its address. Then,
   from the manifest and the interfaces alone, the manifest's toolchain,
   the `within` of every unit and container and the members of every
   container, and the [outcome](#references) of every reference. Phase two
   reads no module again. `fidryn_driver::workspace::check_interfaces` is
   that last part as a function of the interfaces of the workspace and of
   its dependencies, the manifest and the toolchain's version.

Phase one runs on `--jobs` worker threads, by default as many as the
system reports as available, each module file being read and checked on
one of them. Of phase two, the outcomes of the references are found on as
many threads, the modules shared out among them; the rest of it, the
comparison with the lock, where each module stands, the manifest, `within`
and members, and the index of addresses the outcomes are looked up in,
runs on one thread. The diagnostics, the summary line, the check
report and the unresolved list are the same, byte for byte, for every
number of workers: each module's result is collected by itself, and the
results are sorted by the path of their file and then by position before
anything is printed or written.

### Interfaces

The interface of a module, which `fidryn_provision::interface_of` builds
from the lowered module and the byte ranges of its declarations, holds all
that phase two reads of the module:

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
  `citation` and `binding` declaration, at which phase two reports what
  it finds.

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
| `imports`, `citations`, `bindings` | of a curated module, each with its range |

`at` is an address relative to the module's: empty for the module's own
address, and otherwise the segments below it without their first `/`, so
that `a/1` in the module `/zz/demo/t1/s8` is `/zz/demo/t1/s8/a/1`. An
address that does not lie below the module's, which only a module with an
`E700` has, is written whole. `from` is `"module"`, `{"provision": n}` or
`{"note": n}`, the place of what the reference stands in, counted from 0
in `provisions` or in `notes`. Each place in `words` is counted from 0.
`Interface::from_json` refuses a text of another `schema`, a key it does
not know or a missing one, the key of an optional field included, a place
that names nothing, an `at` that names no address, and parts that the
shape of the module does not have. It reads only what `to_json` writes: a
text whose JSON value is not the value that the interface read from it
writes back, such as one with an object written as an array of its values,
is refused, and the place where the two differ is named.

### The check report and the unresolved list

`fidryn check DIR --report FILE` writes the check report,
`fidryn.check-report/v0.1`, whose JSON Schema is
[`check-report-v0.1.json`](../schemas/check-report-v0.1.json): the number
of module files read, of those read from a cache, of errors and of
warnings; for each layer the number of its references with each outcome;
and the number of references of outcome `outside` by the body of law they
name. Every layer and every outcome is written, in the order shown, and
the bodies are sorted. The report holds no path, so it is the same on
every host. This is the report of an invented workspace that has a
reference of every outcome, committed under
`conformance/fixtures/check-report/`:

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
  "outside_by_body": {
    "/yy/stat": 2,
    "/zz/other": 2
  }
}
```

`fidryn check DIR --unresolved FILE` writes the unresolved list: every
reference whose outcome is not `exact`, one object of compact JSON to a
line with the keys `from`, the address the reference stands in, `layer`,
`target` and `outcome`, in that order, sorted by the bytes of `from` and
then of `target`. Its first lines for the same workspace:

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
address wins.

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
names each one whose outcome is not `exact`.
`fidryn_provision::AddressIndex` gives the outcome of a target, and also
the tree hash of the provision at an address and the enacted-layer hash of
the unit an address lies in. A lookup of it costs the same however many
addresses it holds, and a target has few segments, so the outcomes of all
the references of a workspace cost time in proportion to their number.

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
set and not empty; else `.fidryn-cache` inside the workspace. `--no-cache`
keeps no cache: the check then reads and checks every module file and
writes no entry. The name `.fidryn-cache` begins with a dot, so the check
does not read it as part of the workspace and the writer of a generated
workspace leaves it alone; the check writes nothing else into the
workspace.

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
sees half of one, and two checks at once, which may write the same entry,
leave a valid cache. A failure to write the cache, such as a full disk or
a directory that cannot be made, does not fail the check: no further entry
is written, and `fidryn check` says once on stderr that the cache could not
be written, naming the cache's directory and the error, and goes on.

The diagnostics, the summary line, the check report and the unresolved
list of a check that reads from the cache are byte for byte those of a
check without one, but for the count `from cache`. A first and a second
check of a copy of the demo workspace:

```text
modules: 5; from cache: 0; errors: 0; warnings: 0
ok
modules: 5; from cache: 5; errors: 0; warnings: 0
ok
```
