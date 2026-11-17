# Law modules

This reference gives the rules by which a module of law, and each provision
and note in it, is named, where the file of a module stands in its
workspace, how the text of a module is normalized, what a module holds, how
its provisions, notes and layers are hashed, how a module is written, what
the checker verifies of one module, and how modules form a workspace and
what the checker verifies of one. The crate `fidryn-provision` implements
the model and the rules over it, `fidryn-syntax` reads the text of a
module, `fidryn-check` lowers it into the model and checks it, and
`fidryn-driver` checks a workspace and writes a generated one. In the
examples, `/zz/demo` is an invented corpus.

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
  earlier run which was stopped left behind.
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
holds across the files, from the manifest and a summary of each module
that lowered: its path, its address, its shape, `within`, its members and
its aliases.

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
does not lower, and a file the lock lists that is missing, take no part in
the checks of `within` and members: their own diagnostics say what is
wrong, and a container that lists them is not reported for it.

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
the tests of `fidryn-driver` hold it to that:

```sh
fidryn check conformance/fixtures/workspace/demo
```

```text
modules: 5; from cache: 0; errors: 0; warnings: 0
ok
```

`modules` counts the module files read, every `.fr` file of the workspace,
and `from cache` how many of them were read from a cache; the check keeps
none, so it is 0.
