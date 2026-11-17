# Changelog

Release versions follow Semantic Versioning. Source instrument versions and
interchange schema versions are maintained independently of the interpreter.

## [Unreleased]

### Added

- Law modules of edition 2, which hold provisions of law as data. A file
  whose first two words are `edition 2` is read by a parser of its own,
  lowered into the model of `fidryn-provision`, and checked as one module by
  `fidryn check`: its shape, its addresses, the normalization of its text,
  its hashes, its footnote markers and its definitions. Every other command,
  and `fidryn_cli::compile_source` and `compile_module`, refuses such a file
  with `E100`. A file without the marker is read as before.
- `fidryn_provision::render::render`, which writes a module in its
  canonical form with every hash computed, and `hash::tree_hashes`, the tree
  hash of every provision of a tree in one pass.
- The documentation check reads a block that begins with `edition 2` as a
  complete module and checks it.
- Workspaces of edition 2 modules. `fidryn check DIR` checks a directory
  with a manifest, `workspace.json`, at its top: every module by itself,
  then the manifest, the lock `law.lock.json` of a generated workspace,
  where each module stands, the agreement of the module files with the
  lock (`E730`), the containers and their members (`E720`), and the
  toolchain's version (`W760`, a warning); it prints a summary line.
  `fidryn_driver::workspace::GeneratedWriter` writes a generated workspace.
  The manifest, the lock and the retrieval record refuse a file whose JSON
  value is not the value they write back, and have the JSON Schemas
  `schemas/workspace-v0.1.json`, `law-lock-v0.1.json` and
  `retrieval-v0.1.json`.

## [0.2.0] - 2026-11-16

### Changed

- Breaking API and input changes: typed query arguments, nominal entities,
  collections, money currencies and clause references are checked consistently.
  Invalid or undeclared inputs now fail explicitly. Case, source-manifest and
  package-lock versions and semantic field names are validated strictly.
- Operative CLI runs apply hypotheses only with `--scenario`. Reports use
  `fidryn.evaluation-report/v0.2`, retaining invocation, source and executable
  identities, actual derivation graphs, and checked exploration evidence.
- Compiler and Core APIs preserve requirements, structured collections, named
  arguments, observation validators, judgment policies and selection metadata.
- Generic selection, full court-finding and clause execution, position
  materialization and the complete module calculus remain explicit open
  obligations. Unsupported paths return refusals or open outcomes.
- Under the stricter checker, 54 of the 114 modules under `examples/`,
  `packages/`, `prelude/` and `tests/programs/` pass `fidryn check`.
  Thirty-five example modules and seven case records were repaired with
  typing-only edits, and documents mark the rest as drafts where they name
  them by path.
- The implementation status and language guides state what the 0.2
  interpreter interprets and what it does not.
- Internal review ledgers moved under `docs/internal/`.

### Added

- Source-defined concurrence judgments and typed observations with scoped,
  temporal evidence validation; grounded parameterized duty instances.
- Configurable shared evaluation budgets, continuation accounting, parser/input
  limits, bounded completion search and retained trace limits.
- Retained font and crate license texts under `LICENSES/`, and locked
  dependency notice inventories in release archives.
- Typed query input schemas, editor diagnostic ranges, structured document
  rendering and portable review bundles with retained inputs and rerun metadata.
- Native GitHub protections, mandatory test/security checks, release metadata
  validation, distributable native packages and release publication from main.
- A recorded check status for every example module, enforced in CI, with
  the counts stated in the README and example guides.
- A documentation check that follows links, compiles complete module
  samples and runs the `fidryn` commands that documents show in blocks
  tagged `sh`.
- Tests that pin known limits of the interpreter, cited by the
  implementation status guide.
- A security policy.
- `LICENSING.md`, which states what the MIT License covers in this
  repository, how third-party material is handled, and that contributions
  are accepted under the same license.

### Fixed

- Timestamp ordering and named record-constructor calls through nested functions.
- Canonical portable review-bundle paths on Windows, including nested and missing
  source observations.
- Consistent generated-site diagnostic paths across hosts.
- Requirements and assertions discarded by parsing or lowering; untyped Boolean
  and result escapes; literal overflow and string/collection corruption.
- Circular rule support, order-dependent conflicts, shared continuation frames,
  staged duty reads and transaction progress that survived rollback.
- Unrelated, duplicated, stale or future evidence and unscoped acceptance or
  authority affecting institutional results.
- Inconsistent completion admission, assignment identity collisions, inaccurate
  coverage and replay claims that did not agree with the actual typed result.
- Compilation trust depending on cache order; incomplete dependency observations;
  source path escapes, alias conflicts and non-private capture directories.
- Guides no longer rely on a POSIX-only temporary path.
- The examples guide shows only commands that behave as stated.
- A line that does not begin with a continuation is no longer joined to the
  line before it when that line ends in an integer literal, in `operative P`
  or `determined P`, or in an `if` without `else`. `1` followed by a line
  that begins with `days` is no longer the duration `1 days`. An `if` whose
  condition is followed by the end of the statement is rejected with `E100`,
  and the next line is no longer read as its branch.
- A blank line or a comment line does not end a statement before a line
  that begins with `and`, `or`, `in`, `.`, `,`, `->`, `=>`, `:`, `=`, `|`
  or `<`, so a condition can carry a comment between its lines. With the
  lines `when operative A()`, a comment line and `and operative B()`, a
  rule fired before when `A()` alone held. Before a line that begins with
  any other continuation, a blank line or a comment line now ends the
  statement where the parser tests for its end, whatever the line above it
  ends in: `if c > 0`, a comment line and then `{ 1 }` is rejected with
  `E100`, and so is `nomination Alice`, a blank line and then
  `for TrusteeOf(BRT) rank 1`. Both passed the checker before, and the
  block was read as the branch of the `if`.
- `fidryn verify` reads a `verify` block as a list of fields. With the
  exceptions that "The fields of a verify block" in `docs/language.md`
  lists: a field ends where the next one begins, so an `assert` line is its
  own assertion even when the line above it is incomplete; a line that
  begins with `for_all` or `exists` is checked as an assertion; and a
  `for every` line, an `assert` with no expression and a line the block
  does not recognize are kept as assertions that cannot be decided, which
  leaves the property `unknown` unless another assertion is false. Before,
  an `assert` after the first one of a block was not checked, and neither
  was a quantifier line below an `assert`: `assert true` followed by the
  line `assert false`, or by the line `for_all x in {1, 2}: x == 3`, was
  `proved` and is a counterexample. `asert false` followed by the line
  `assert true` was `proved` and is `unknown`, and `assert true or 1 == 2`
  followed by the line `assert false` was `unknown` and is a
  counterexample.
- A `verify` block with no assertion is `unknown`, whatever its comments
  say. The text of such a block was searched for the word `assert` before,
  and a comment that held it was read as the assertion. The bounds of a
  block are read only from the tokens of its `bounds` field. A comment no
  longer sets them, wherever it stands, and neither do the contents of a
  string or another field that holds the word `bounds`. Each number is read
  at the first place where its word, `persons`, `events` or `time_points`,
  stands as a token in the field: it is the integer token that follows the
  word directly or after one `:`. Otherwise it is zero, as it is when the
  integer does not fit in 32 bits. So `persons: 2.5` gives zero, and so
  does `bounds persons x persons 2`.
- A name directly before a `{` is no longer applied to the block where the
  block belongs to the construct around it: in the condition of an `if`,
  in the domain of a quantifier, and in the header of a rule. A rule whose
  `from` basis is a bare name, as in `from Basis`, keeps its own `when` and
  `then` lines and no longer removes the declaration after it.
  `if a < b { 1 } else { 2 }` tests `a < b` and keeps its branch; it passed
  the checker before and failed when run. A record constructor written with
  its name needs parentheses in those places, as in
  `if (R { x: 1 }) == r { 1 } else { 2 }`.
- A declaration such as `proposition P` with no parentheses or `entity E`
  with no type, a header keyword such as `jurisdiction` with no value, and
  a `query`, `calc` or `fn` with no body each took the declaration after it
  as part of itself, with no diagnostic from the parser. That declaration
  is now read. `proposition P` and `entity E` written so pass the checker
  as before. `jurisdiction`, `source_snapshot`, `source_manifest`,
  `effective_at`, `recorded_at` or `outside_scope` is rejected with `E100`
  when its statement ends with the keyword, and the value is not read from
  the next line. A `calc` or `fn` with no body used the body
  of the declaration it took: `calc f(a: Int) -> Int` followed by
  `query q() -> Int { return 7 }` passed the checker, and `f(1)` was 7. A
  `calc` or `fn` with no `{ ... }` body is now rejected with `E100`, and a
  `query` with no body is rejected with `E430`. Inside a body the same
  holds for a field name in a `source` block whose statement ends with it:
  it no longer takes the line below it. The `source` field `effective`
  still does: it is a prefix word, so its line goes on into the next one.
  `record`, `otherwise`, `on` or `review` alone on its line in a
  `judgment` does not take the line below it either.
- Nothing but its own `{ ... }` block is read as the body of a `query`,
  `calc` or `fn`. A comment was read as one, beside the header, below it or
  between its lines: the lines `query Q()`, `// { return 7 }` and `-> Int`
  passed the checker and answered 7, and are now rejected with `E430`. So
  was the effect row of a query with no body:
  `query Q() -> LegalPerson ! {Acme}`, where `Acme` is an entity, answered
  `Acme`, and is now rejected with `E430`. And the header itself was read
  as the body of a `calc` or `fn` that had none, so
  `calc f(a: Int) -> Int fuel = 100` at the end of a module passed the
  checker, and a query that called `f` failed when run.
- A brace inside a comment or a string is not a brace where the body of a
  declaration is read from its text. Inside a rule, the comment line
  `// } { when false`, or a string that holds those characters, gave the
  rule the guard `false`, and it never fired.
- A declaration keyword that takes a name, with nothing after it on its
  line, took the first word of the line below as its name: `proposition`
  above the line `proposition Witness()` declared a proposition named
  `proposition`, and `Witness` was lost with no diagnostic. It is now
  rejected with `E100`. So is a header that stops at `:`, `->` or `=`
  above another declaration: `entity Pat :` followed by a `query` line
  read the `query` line as the type of `Pat`, and the query was lost. A
  type on the next line that is no declaration keyword is read as before.
  A name that ends in `.` and type arguments that `<` opens and no `>`
  closes are rejected with `E100` too: `entity N .` above a `query` line
  named the entity `N.query`, and `entity E : Map <` above one passed the
  checker, and each lost the query. The header of a `record_type`, an
  `evidence_type`, an `observation` or a `judgment` that stops at `:` or
  `->` above another declaration passed the checker too and lost the
  declaration below it. It is now rejected with `E100`, and the
  declarations below it are read. One line below a header that ends in
  `:` is not a declaration: under the header of a `for_all` or an
  `exists`, a line that begins with `for_all` or `exists` is the body of
  the quantifier, as before.
- The header of a declaration ends where the next declaration begins.
  Outside the parentheses, brackets and braces that a declaration has
  opened and not closed, a line that begins with a declaration keyword is
  not read as part of that declaration, whatever the line above it ends
  in. The body of a quantifier, in the note above, is the one exception.
  A rule whose `from` basis ended in `not` or in a `.`, for example, was
  not rejected: with the lines `rule R : derive from Basis.` and
  `clause C { then establish Witness() }` above a query `q`, a module
  passed the checker, and `fidryn run` reported `q` as an unknown query.
  An observation whose `from` source ended in `+` above a `verify` block
  passed the checker too, and `fidryn verify` did not know the property
  of the block. Such a header is now rejected with `E100`, and the
  declarations below it are read. The header of a judgment, an effect row
  that `!` begins and type arguments that `<` opens and no `>` closes end
  in the same way: each is rejected with `E100` when it is not finished
  above a declaration, and that declaration is read.
- A parenthesis or a square bracket among the type arguments of a type
  that the parser reads, such as the type of a parameter of a `calc`, the
  type of an entity or the type of a field of a `record_type`, is rejected
  with `E100`. `calc f(x: Int<(>) -> Int { return 1 }` passed the checker.
- A declaration keyword that takes a name and has something else after it
  is rejected with `E100`. `power { holder A }`, `proposition (x: Int)`
  and `entity : NaturalPerson` passed the checker, each as a declaration
  with no name.
- A token that begins no field inside the braces of a type variant or of
  an `otherwise suspend NeedJudgment` request is rejected with `E100`, and
  the declarations below it are read. `type T = A { ) }` passed the
  checker, and so did a judgment whose `otherwise suspend NeedJudgment`
  request held `{ ) }`. A `type` declaration that has `{` where its `=`
  belongs, as in `type N {` above another declaration, is rejected with
  `E100` as before, and the declarations below it are now read.
- In the body of a rule, text that is no rule statement is rejected with
  `E100`, and so is a `then` or an `otherwise` that no consequence
  follows. Such text was stepped over without a diagnostic: with `whne`
  written for `when`, a rule lost its guard and fired whatever the case
  held. A `when`, a `require` or a `source` with no expression after it is
  rejected with `E100` as well. It had no diagnostic: with
  `when // operative A()` as the last line of a rule, the condition was in
  a comment, and the rule fired whatever the case held. A `valid_time` or
  `record_time` line with an interval is still accepted and still not
  applied: the consequences of a rule hold at every time, which the
  implementation status guide now says and a test pins. The interval needs
  both of its bounds: `valid_time [, +inf)` is rejected with `E100`.
- A consequence of a rule whose expression is a number, a string, a sum, a
  set or a proposition under `operative` is rejected with `E210`, in a
  diagnostic that names the rule and the consequence. `then establish 5`
  passed the checker, and the rule established nothing. A consequence that
  is the name of an entity whose type is neither `Prop` nor an alias of
  `Prop` is rejected with `E210` as well: `then establish Acme`, with
  `entity Acme : LegalPerson`, passed the checker too.
- A consequence after `create` is checked as it is after the other eight
  verbs, unless its name is `Duty`. The checker looked at nothing after
  `create`, so `then create Acme(1)`, where `Acme` is an entity, and
  `then create 2026-01-01` passed it. Each is now rejected with `E210`, as
  it is after `establish`. `Duty` after `create` is now checked for its
  fields `bearer`, `claimant` and `content`, and not as a proposition:
  `then create Duty(bearer = Acme, content = C())` passed the checker, and
  is rejected with `E210`, "Duty creation requires `claimant`".
- A statement that a query body, a `goal Evaluate` block, the body of a
  `calc` or an `fn`, or a `transaction` block cannot read is rejected with
  `E100`, wherever it stands in the block. In a goal it was stepped over
  without a diagnostic, and the statement below it with it: a goal that
  held the lines `true`, `return` and `require false` answered `true`. In
  the other blocks it had no diagnostic when it was the last statement: a
  query whose body held the lines `true` and `return` answered `true`, and
  a `calc` whose body held the lines `7` and `return` returned 7.
- `require ... using` with no protocol name in the statement of `using` is
  rejected with `E100`. A `using` at the end of a line took the first word
  of the next line as the name: `require true using` followed by the lines
  `require false` and `true` answered `true`.
- The `observation` sample of the language guide is a complete
  declaration. The two header lines it showed before are rejected with
  `E100`.
- A comment at the end of a line is no longer kept as part of the header
  value, the type or the `source` field value before it.
  `jurisdiction Test // where` gave the jurisdiction the value
  `Test // where`. `entity Pat : NaturalPerson // the tenant` gave `Pat`
  the type `NaturalPerson // the tenant`, and a query that returned `Pat`
  as a `NaturalPerson` was rejected with `E210`. In an `effect`,
  `request(name: String) -> Bool // asks a clerk` gave the operation the
  result type `Bool // asks a clerk`, and a query that required the effect
  was rejected with `E210`. The result type is now `Bool`.
- A comment is never read as code. Some parts of a declaration are read
  back out of its text, and they are now read with every comment taken as
  white space. Before, a comment could set them. In an `effect`, a comment
  line above the operation, such as
  `// asks a clerk (see the rules) -> Clerk`, left the operation with no
  parameters, and a query that called it with an argument was rejected
  with `E210`. `entity Pat :` with a comment after the colon and the type
  on the next line gave `Pat` a type that held the comment. Two
  nominations for one office, each with a comment that held `rank 9`, were
  rejected with `E410` as a tie at rank 9. A `conflict_doctrine` that named
  its clause only in a comment, as in `// then defeat Residual`, passed the
  checker, and is now rejected with `E511`. A `source` whose only
  `artifact` stood in a comment passed the checker, and is now rejected
  with `E540`. And a comment in a query that held
  `for_all x in People: true` declared the property `for_all:x:0`, which
  `fidryn verify` no longer knows.
- A bracket or a word inside a string literal is not read as structure
  where a declaration is read back out of its text. An `effect` with the
  citation `from "Stat. 12(a"` in its header lost the parameters of its
  operation, and one with a string that held `->` below its operation took
  its result type from the string; a query that required either effect was
  rejected with `E210`. A `conflict_doctrine` that named a clause only
  inside a string, as in `note "we defeat Residual here"`, passed the
  checker, and is now rejected with `E511`. A `source` with no `artifact`
  field passed the checker when only a string in another field held that
  word, with more text after it in the string, as in
  `citation "the artifact of 1990"`, and is now rejected with `E540`. A
  string in a query that held `for_all w in People: true` declared a
  property; it declares none now.
- Where a part of a declaration is read back out of its text by a word,
  such as the `rank` of a nomination, the word is found only as a word of
  its own, and not inside a longer name. The rank of
  `nomination Frank for TrusteeOf(BRT) rank 2` was read from the `rank` in
  `Frank`, as 0. With `nomination Bob for TrusteeOf(BRT) rank 1` beside it,
  on a case in which the office needs a successor and both candidates are
  eligible and have accepted it, a query whose goal is `UniqueOccupant`
  for that office answered `Frank`, and it answers `Bob` now. With `Bob`
  at rank 0 instead, the module was rejected with `E410`, as a tie at that
  rank. The query answered in the same way for a candidate named
  `Franklin`, `rank_holder` or `ranked` at rank 2. A `source` with a
  longer name that holds the word `artifact`, such as `artifact_registry`,
  `my_artifact` or `artifacts`, or with such a name as the value of
  another field, as in `kind artifact_set`, passed the checker with no
  `artifact` field, and is now rejected with `E540`. Beside `for`,
  `alternative`, `defines` and `as`, where the grammar puts white space, a
  tab now serves as well as a space, and so does a line end where the
  parser does not end the statement at it; beside `rank` and `artifact`
  both already did. A nomination with a tab before or after `for`, or
  with `for` at the start of a line that no space begins, counted for
  every office, whatever office it named: with `Alice` so nominated for
  `GuardianOf(BRT)` at rank 1 and `Bob` nominated for `TrusteeOf(BRT)` at
  rank 2, the query above answered `Alice`, and it answers `Bob` now. In
  the `interpretation_family` that makes the candidates eligible, an
  `alternative` with a tab after the word was lost, and one whose name
  ends in `alternative`, such as `both_alternative`, lost its definitions:
  the query was `suspended` for want of them. A `defines` with a tab after
  the word, or with `as` at the start of a line that no space begins, was
  lost as well: when it made the candidate of rank 1 eligible, the query
  answered with the candidate of rank 2. In both of these cases the query
  now answers with the candidate of rank 1.
- A definition in an alternative of an `interpretation_family` whose
  expression is no proposition is rejected with `E210`, in a diagnostic
  that names the family and quotes the definition, such as
  "`defines 5 as established` in interpretation family `Reading` requires
  a proposition". Such a definition, for example
  `defines 1 + 2 as established`,
  `defines not Eligible(Alice) as not_established` or
  `defines Alice == Alice as established`, was dropped without a
  diagnostic, and the module passed the checker. The expression is all
  the text between `defines` and its `as`, and a definition with more
  than a proposition there is rejected in the same way:
  `defines Eligible(Alice) 5 as established` passed the checker, and a
  definition of `Eligible(Bob, TrusteeOf(BRT))` with
  `Eligible(Alice, TrusteeOf(BRT))` after it made `Bob` eligible. A
  definition with no `as established` or `as not_established` after its
  expression, such as `defines Eligible(Alice)` or
  `defines Eligible(Alice) as suspended`, was dropped in the same way, and
  is now rejected with `E100`. So is one
  marked with a longer word: `as established_later` was read as
  `as established`, and `as not_established_yet` as `as not_established`.
- The definitions of an `interpretation_family` are read from the blocks
  of its alternatives: an alternative is its name and the block that
  follows it, and its definitions are the ones inside that block. A
  `defines` that stands in the family outside the block of every
  alternative is rejected with `E100`, in a diagnostic that quotes it,
  names the family and says that it stands outside the block of every
  alternative. One below the `}` of an alternative was taken into that
  alternative, and one above the first alternative was dropped. In a
  module with `Bob` nominated at rank 1 and `Alice` at rank 2 for the
  office `TrusteeOf(BRT)`, only `Alice` made eligible in the block of
  `alternative Both`, and a query whose goal is `UniqueOccupant` for that
  office, on a case that selects `Both`, the query answered `Bob` with
  `defines Eligible(Bob, TrusteeOf(BRT)) as established` below that
  block, and `Alice` with it above the block. A `defines` that `(`
  follows directly is found:
  `defines(Eligible(Bob, TrusteeOf(BRT))) as established` in that block
  was dropped, and the query answered `Alice`; it answers `Bob` now. An
  `alternative` with no name, such as `alternative {`, and a definition
  with more than one status are rejected with `E100`; the first passed
  the checker, and `as not_established as established` was read as
  `as not_established`.
- A `duty` whose `content` cannot be read is rejected with `E100`, in the
  diagnostic a duty now has for a `bearer`, `claimant`, `attaches` or
  `due` that cannot be read: "duty `Keep` contains an invalid or
  over-budget attachment/deadline expression". `content [a, b)` passed
  the checker, and the duty had no content; so did a content of 5,000
  nested brackets, `content days`, `content a +` and a `content` with
  nothing after it. `[a, b)` passed the checker in the other four fields
  as well.
- A `duty` field or the `returns` of a `decision` that holds an item its
  reader cannot read, inside the arguments of a call, the braces of a
  set, a record or a block, or a `transaction` block, is rejected with
  `E100`, in the diagnostic that the duty or the decision has for an
  expression that cannot be read. `content Pay([a, b))`,
  `content { [a, b) }` and `due f([a, b))` passed the checker, and so did
  a `decision` with `returns { 1, [a, b) }`. This reader reads less than
  the parser: an interval, as in `Pay([1, 2))`, a duration word with no
  number before it, as in `Pay(days)`, and `()` are read in a query or a
  rule, and are rejected in a duty or a decision.
- A word that begins a field of a `duty`, such as `content` or `due`,
  begins one only outside the parentheses, brackets and braces of a field
  that the reader of the duty does not know; inside them it is part of
  that field. A duty with
  `discharge_by Delivered(due 30 counted_days after invoice_date)` and no
  `due` field took that deadline: once the duty had attached on a case
  whose invoice date was in January, `duty_status` gave `Breached` in
  September. It gives `Attached` now.
- A rule with no `{ ... }` body is rejected with `E100`, and so is a
  `verify` whose name is not followed by `{`:
  `verify Checked extra { assert false }` passed the checker before.
- An `if` or an `else` with no branch is rejected with `E100`. The line
  below an `else` that ends its statement is no longer read as its branch,
  so `assert if true { true } else` followed by the line
  `for_all x in {1, 2}: x == 3` is rejected; it passed the checker before.
- A word that cannot begin an expression, such as `return` or `when`, is no
  longer kept as a name when it begins an item in the braces of an
  expression that the parser reads, such as the branch of an `if` in the
  body of a `calc`. `if c > 0 { return 1 } else { return 2 }` there is
  rejected with `E100`; it passed the checker before and failed when run.
  The text of a `duty`, which the parser steps over and the lowering
  reads, still takes such a word for a name: `content { return Pay(Acme) }`
  passes the checker, with the name `return` in the content.
- The type arguments of a call nested 80,000 deep ended the process with
  a stack overflow, and so did an `if` with 20,000 `else if` links in the
  body of a `calc`. Type arguments of a call nested more than 128 deep
  are now rejected with `E100`, as those in the type of a parameter are.
  In the body of a `calc`, each `else if` of a chain counts as a level of
  the expression, and a chain that passes the limit of 128 levels is
  rejected with `E100` however long it is.
- Deeply nested text that the parser steps over, and that is read later
  from the text, no longer ends the process and no longer loses a part
  without a diagnostic. A `duty` whose `content` was an `if` with 30,000
  `else if` links ended `fidryn check` with a stack overflow, and so did an
  `effect` whose operation had a result type nested 5,000 deep. The duty is
  now rejected with `E100`, and so is one with 127 links, which passed the
  checker before; a chain of 126 links is the longest that is read. The
  effect is rejected with `E100`, "type nesting exceeds the limit of 128",
  which is the limit and the message the parser has for a type it reads.
  That holds wherever a type is read from the text of a declaration: one
  of 128 levels of `<` is accepted there, and one of 129 is rejected. A
  result type nested 200 deep in an `effect` passed the checker before. A
  `decision` whose `returns` held a part nested deeper than its reader
  goes, such as an element of a set under 63 pairs of parentheses, passed
  the checker with that part left out, and an `interpretation_family` lost
  a definition that held such a part. Each is now rejected with `E100`.
- `fidryn check` reads a long set literal of comparisons such as
  `{x < y, x < y}` faster, and such a literal no longer uses the stack up.
  A `calc` that returns 3,000 of them took about five seconds to check; it
  now takes about a third of a second. One of 65,000 ended the process
  with a stack overflow; it is now rejected with `E100`.
- Where a type is missing after the `:` of a parameter of a `calc`, an
  `fn`, a `query` or a `rule`, or after the `->` of the result of a
  `calc`, an `fn` or a `query`, the message is "expected a type", as it
  is after the `:` of an entity; it read "expected type" there. A
  parameter with no type of a `proposition`, an `office`, a `judgment`, a
  `decision`, a `duty`, a `clause`, a `legal_act` or an `observation`,
  the result of a `decision` with no type, and a field with no type of a
  `record_type` or an `evidence_type` passed the checker before, and are
  now rejected with `E100`, "expected a type".

### Security

- Required code-scanning findings checks and a publication guard for unresolved
  errors or high/critical findings, in addition to analysis execution checks.

- Updated vulnerable or unsound pinned dependencies and added locked dependency,
  secret-history/current-tree, workflow and CodeQL checks.
- Main's mandatory checks have no bypass actors; the owner's review exception
  remains separate. Source capture and workflow-content trust limits are
  documented with the remaining language obligations.
