# Implementation status

A `.fr` file is not an executable specification. Presence under `examples/`
or `prelude/` does not mean the interpreter implements the feature.

A feature counts as implemented only when syntax, name resolution, checking,
evaluation, uncertainty, provenance, and tests agree.

This page says what the 0.2 interpreter does today. Each statement below
cites a test or a code location in this version. Tests are in
`tests/integration/known_limits.rs` unless another file is named. The review
records from earlier passes are listed in the
[internal ledgers](internal/README.md); several describe code that has since
changed.

## Declarations

"Body parsed into structure" says whether the parser reads a declaration's
body into fields. "Reaches the intermediate representation" says what the
checker passes on to the evaluator. "Executed" says what the evaluator does
with it.

| Kind | Body parsed into structure | Reaches the intermediate representation | Executed | Evidence |
| --- | --- | --- | --- | --- |
| `query`, `fn`, `calc` | Yes. A `calc` or an `fn` with no `{ ... }` body is `E100`. A `query` with none is `E430` from the checker. A statement that a query body, a `goal Evaluate` block, the body of a `calc` or an `fn`, or a `transaction` block cannot read is `E100`, wherever it stands in the block | Yes | Yes | `crates/fidryn-syntax/src/parser.rs:2251`, `:2705`, `:2769`, `:2363`; in `tests/integration/declaration_text.rs`, tests `a_function_with_no_body_is_e100_whatever_stands_beside_or_below_it` and `a_query_with_no_body_is_e430_whatever_stands_beside_or_below_it`; in `tests/integration/unreadable_text.rs`, test `a_statement_that_cannot_be_read_is_e100_when_it_is_the_last_of_its_block` |
| `rule` | Yes. The body holds `when`, `then`, `otherwise`, `require`, `select` and `source` statements. Other text in it is `E100`, and so is a `then` or an `otherwise` that none of the nine verbs follows, and a `when`, a `require` or a `source` with no expression after it. A `valid_time` or `record_time` line with an interval is read, and nothing of it is kept. An interval that lacks a bound is `E100` | Guard, selection, consequences and `otherwise` fallback. The expression of a consequence is checked as a proposition: a number, a string, a date or the name of an entity whose type is neither `Prop` nor an alias of `Prop`, for example, is `E210` after each of the nine verbs. After `create`, `Duty` with arguments is checked for its three fields instead (see Rule verbs below). The `from` basis is not checked and not kept. Verbs other than `establish`, `constitute`, `terminate`, `suspend` and `create` become derive (see Rule verbs below). Every consequence is given the interval of all time, whatever a `valid_time` or `record_time` line says | Derive, establish, terminate and suspend. `create` is refused with an engine error when the rule fires | `crates/fidryn-syntax/src/parser.rs:2954`, `:3010`, `:3019`, `:3028`; `crates/fidryn-check/src/lib.rs:3454`, `:3816`; `crates/fidryn-eval/src/worklist.rs:248`; in `crates/fidryn-syntax/src/parser.rs`, tests `text_in_a_rule_that_is_no_rule_statement_is_e100_and_only_it_is_stepped_over`, `a_when_a_require_or_a_source_with_nothing_after_it_is_e100`, `an_interval_of_a_rule_that_lacks_a_bound_is_e100` and `a_valid_time_or_record_time_line_of_a_rule_is_read_and_nothing_of_it_is_kept`; in `tests/integration/known_limits.rs`, tests `the_valid_time_and_record_time_lines_of_a_rule_are_not_applied` (the two interval lines), `a_rule_basis_that_names_nothing_is_accepted` (the basis) and `the_activate_verb_is_lowered_as_derive` (`activate` is a verb the parser accepts and the checker lowers as a derive) |
| `judgment` | Yes. An unrecognized clause is kept as text | Yes | Concurrence policies only. One with an unrecognized clause or a `standard` suspends with a `JudgmentPolicy` request | `crates/fidryn-eval/src/judgment.rs:44`, `:57` |
| `observation` | Yes. An unrecognized clause is kept as text | Yes | Yes. An unrecognized clause is an engine error once a record of the observed schema is on the case | `crates/fidryn-eval/src/judgment.rs:486`, `:533` |
| `type`, `record_type`, `evidence_type`, `entity`, `proposition` | Yes | Yes | Not applicable | `crates/fidryn-check/src/lib.rs:3459` |
| `effect` | Re-read from the declaration text | Name and its first operation, with parameters and result | A call becomes a `needCustom` request to the case | `crates/fidryn-check/src/lib.rs:1980`, `:3552`; `crates/fidryn-eval/src/lib.rs:2346` |
| `duty` | Re-read from the declaration text. A `bearer`, `claimant`, `attaches`, `content` or `due` whose expression cannot be read is `E100`. Unknown fields are skipped, but each of those five words is taken for its field wherever it stands as a name in the body, also inside an unknown field: `discharge_by Delivered(due)` is `E100` | Bearer, claimant, attachment, content, due | `duty_status` and `duty_step`, on one specialized path | `crates/fidryn-hir/src/body.rs:469`; `crates/fidryn-eval/src/duty.rs`; in `crates/fidryn-hir/src/lib.rs`, test `a_duty_whose_content_cannot_be_read_is_e100` |
| `decision` | Binders are parsed. Record requirements and `returns` are re-read from the declaration text | Binders, record requirements, the first `returns` | Through `RunDecision` | `crates/fidryn-hir/src/body.rs:327`, `:339` |
| `interpretation_family`, `nomination` | By splitting the text on keywords. A keyword is found only as a word of its own, not inside a longer name. Where white space stands beside `for`, `rank`, `alternative`, `defines` or `as`, a tab serves as a space, and so does a line end where the parser does not end the statement at it | Alternatives and their definitions; candidate, office as text, rank. A definition whose expression is no proposition is `E210`, and one with no `as established` or `as not_established` after its expression is `E100`. A nomination's `from` basis is dropped | Only by `UniqueOccupant` succession | `crates/fidryn-hir/src/lib.rs:1076`, `:1092`, `:1131`; `crates/fidryn-eval/src/lib.rs:4130`; tests in `crates/fidryn-hir/src/word_tests.rs`; in `crates/fidryn-hir/src/lib.rs`, tests `a_definition_that_is_no_proposition_is_e210_and_names_the_family_and_the_definition` and `a_definition_with_no_status_after_its_expression_is_e100`; in `tests/integration/whole_words.rs`, tests `a_candidate_whose_name_holds_the_word_rank_is_ranked_as_written` and `a_nomination_written_with_tabs_or_line_ends_is_ranked_as_written` |
| `office` | Name and parameters only | Fixed defaults: occupant type `LegalPerson`, at most one occupant, no competence | Not read at run time | `crates/fidryn-check/src/lib.rs:3524`; test `an_office_keeps_none_of_its_declared_fields` |
| `clause` | Name and parameters only | Body kept as text | Only through `EvaluateClause`, which knows two clause names (see below) | `crates/fidryn-eval/src/lib.rs:4791`, `:4986` |
| `legal_act` | Name and parameters only. Unknown fields are accepted | Nothing. The checker types a call of its name as a `LegalAction` | Nothing | `crates/fidryn-hir/src/lib.rs:724`; test `power_and_legal_act_bodies_are_not_parsed_so_unknown_fields_pass` |
| `power`, `scenario` | No. Unknown fields are accepted | Nothing | Nothing | `crates/fidryn-syntax/src/parser.rs:1392`; `crates/fidryn-hir/src/lib.rs:768`; test `power_and_legal_act_bodies_are_not_parsed_so_unknown_fields_pass` |
| `conflict_doctrine` | No. Unknown fields are accepted. The text must name a declared clause after `defeat`, or the checker reports `E511` | Nothing | Nothing | `crates/fidryn-check/src/lib.rs:2365` |
| `source` | Fields parsed; an unknown field is a parse error | Nothing. The checker requires an `artifact` field (`E540`) | Nothing. The artifact file is never read, whatever the digest | `crates/fidryn-hir/src/lib.rs:380`; `crates/fidryn-check/src/lib.rs:3419`; test `a_source_declaration_does_not_reach_core_and_its_artifact_is_never_read` |
| `import` | Name. Version and digest are re-read from the text | Only a package under `packages/` is linked, and only when a path is checked | Not applicable. `alias` is ignored; with no digest and no package the import does nothing | `crates/fidryn-check/src/lib.rs:2412`, `:2767`; test `an_import_that_names_nothing_is_accepted_and_its_alias_is_ignored` |
| `verify` | The block is read as a list of fields, and nine words begin one. The expression after `assert`, and the expression of a field that begins with `for_all` or `exists`, is an assertion, in the order written. A `bounds` field is read for three numbers, and nothing but a `bounds` field sets them. `interpretations`, `judgments`, `choices` and `assuming` fields are recognized and not read. A field that begins with `for`, which is a `for every` line, and an `assert` with no expression are kept as text, in an assertion that cannot be decided. "Where a verify field ends", below, says where a field ends, what becomes of a line the block does not recognize, and which two limits remain | Yes. A `for_all` or an `exists` written in a `verify` block, in a query or as a declaration of its own is a property of its own as well, named after its kind, its variable and its number among the quantifiers written in those places, counted from 0 in the order of the text: `for_all:x:0` | Closed formulas only: literals, arithmetic, comparisons, Boolean connectives, and quantifiers over a literal set or bounded synthetic entities. An assertion that cannot be decided is unresolved, and the property is `unknown` unless another assertion is false. That holds for an assertion that needs a query or a proposition, and for one kept as text. A block with no assertion is `unknown`. A property made from a quantifier has no bounds, whatever the block around it declares, and is `unknown` | `crates/fidryn-syntax/src/parser.rs:549`, `:2481`; `crates/fidryn-verify/src/property.rs:62`, `:123`, `:172`, `:206`; in `tests/integration/verify_blocks.rs`, tests `two_comparisons_on_consecutive_lines_are_two_assertions`, `a_false_quantifier_line_is_a_counterexample`, `a_top_level_exists_line_is_checked_as_an_assertion`, `an_assuming_line_is_recognized_and_not_read`, `a_for_every_line_leaves_the_property_unknown`, `a_block_with_no_assertion_is_unknown_whatever_its_comments_say` and `the_bounds_come_only_from_a_bounds_field`; `crates/fidryn-check/src/lib.rs:3768`; in `tests/integration/comments.rs`, test `a_quantifier_written_in_a_query_or_a_verify_block_makes_a_property_named_after_it` |

Unknown fields. The parser rejects an unknown field inside a `source`
block and inside the request of a judgment's `otherwise suspend NeedJudgment
{ ... }` (`crates/fidryn-syntax/src/parser.rs:2220`, `:1810`). In a `rule` it
rejects text that is none of the statements the table lists for a rule
(`crates/fidryn-syntax/src/parser.rs:2954`). In the other declarations whose
body is a list of fields, an unknown field is skipped without a diagnostic.
In a `judgment` or an `observation`, an unrecognized clause is kept and
refused at run time, as the table says. In a `verify` block, a line the
block does not recognize is not skipped unless the field above it goes on
into it: it is kept as an assertion that cannot be decided, or the module
is rejected, as the next paragraphs say.

Where a verify field ends. The tests named here are in
`tests/integration/verify_blocks.rs`, and the parser tests are in
`crates/fidryn-syntax/src/parser.rs`.

- A field ends where its statement ends. An `assert`, `for_all`, `exists`
  or `bounds` field, and a field that is not read, ends earlier where the
  next field begins: it is read or stepped over as if a `;` stood there
  (`crates/fidryn-syntax/src/parser.rs:2627`, `:2648`; parser test
  `a_verify_field_is_read_as_if_a_semicolon_stood_where_the_next_field_begins`).
- The word `assert` begins a field wherever it stands
  (`two_assertions_on_one_line_are_two_assertions`,
  `an_assertion_on_the_line_of_a_field_that_is_not_read_is_an_assertion`).
- A line that begins with one of the other eight words begins a field
  (`a_field_that_is_not_read_ends_where_the_next_field_begins`,
  `a_for_every_line_leaves_the_property_unknown_below_any_other_line`,
  `a_false_assertion_above_a_for_every_line_is_a_counterexample`,
  `a_line_that_begins_with_for_is_kept_below_an_assertion`; parser test
  `an_assertion_cut_off_where_the_next_field_begins_is_e100_there`). In
  the middle of a line those words do not cut a field short (parser test
  `a_verify_field_ends_where_the_next_field_begins`).
- There is one exception. Below an `assert`, `for_all` or `exists` field, a
  line that begins with `for_all` or `exists` does not end that field. It
  continues the line above when the statement goes on into it, and is a
  field of its own when the statement has ended
  (`a_quantifier_line_goes_on_into_a_quantifier_line_and_ends_at_an_assert_line`,
  `a_false_quantifier_line_is_a_counterexample`).
- A `for every` line and a line the block does not recognize have no early
  end. The parser steps over such a line as text it does not read,
  together with every line that text goes on into, an `assert` line
  included, and the whole is one assertion that cannot be decided (parser
  test `a_line_verify_does_not_decide_takes_with_it_what_continues_it`).
- Nothing inside the parentheses, brackets or braces of a field begins a
  field (parser test
  `nothing_inside_the_brackets_of_a_verify_field_begins_a_field`).
- An expression that is cut off where the next field begins gets the
  diagnostic it would get before a `;`. After an operator that stands
  between two operands, such as `+` or `==`, that is `E100` "expected
  expression after operator"
  (`an_assertion_cut_off_by_the_next_assertion_is_e100`). After an `else`
  that ends its statement it is `E100` "expected `{` after `else`",
  whatever the line below holds
  (`an_else_with_no_branch_is_e100_whatever_the_line_below_it_holds`).
  Where the parser has no diagnostic before a `;`, there is none here
  either: `not` with nothing after it gives no expression. An `assert`
  with no expression, `assert not` included, is an assertion that cannot
  be decided (`an_assert_with_no_expression_leaves_the_property_unknown`).
- Text that follows the expression of an `assert`, `for_all` or `exists`
  field is read as the next field when it begins with one of the nine
  words. On one line that happens only where the word is not taken into
  the expression as an operand, as before a `:`: `assert true assuming: x`
  is the assertion `true` and an `assuming` field, and
  `assert true assuming fair` is one assertion that cannot be decided
  (`a_field_word_after_an_expression_is_an_operand_unless_a_colon_follows`;
  parser test
  `a_field_word_after_a_complete_verify_expression_begins_the_next_field`).
- Text that begins with none of the nine words is part of the field above
  when that field goes on into it. Otherwise it is read as a line of its
  own and kept as an assertion that cannot be decided
  (`a_misspelled_assert_line_before_an_assertion_leaves_the_property_unknown`).
  After an `assert` there is one difference: the `assert` and the text
  that follows its expression are together one assertion that cannot be
  decided
  (`a_misspelled_assert_line_after_an_assertion_leaves_the_property_unknown`),
  and the module is rejected with `E100` when that text begins with
  something other than a word
  (`tokens_that_are_not_a_word_below_an_assertion_are_e100`). A duration
  unit such as `days` is a word here
  (`a_unit_word_below_an_assertion_is_text_like_any_other_word`; parser
  test `text_after_the_expression_of_an_assert_joins_it_or_is_e100`).
- A block with no assertion is `unknown`. Its text is not searched for a
  formula, so a comment is never read as an assertion
  (`crates/fidryn-hir/src/lib.rs:1249`; test
  `a_block_with_no_assertion_is_unknown_whatever_its_comments_say`).
- The bounds come from a `bounds` field and from nowhere else. The parser
  records the field as the text of its tokens and leaves out every
  comment, wherever it stands: inside the field, after it on its line, or
  on the lines below it (`crates/fidryn-syntax/src/parser.rs:2523`,
  `:2591`; parser test
  `a_bounds_field_is_recorded_as_the_text_of_its_tokens`). The numbers are
  read from those tokens, so no comment sets a number and the contents of
  a string are not read (`crates/fidryn-hir/src/lib.rs:1289`). Each number
  is read at the first place where its word, `persons`, `events` or
  `time_points`, stands as a token. When the next token is an integer, or
  a `:` followed by an integer, that integer is the number. Otherwise the
  number is zero, as it is when the word is not in the field and when the
  integer does not fit in 32 bits (`crates/fidryn-hir/src/lib.rs:1317` and
  the unit test
  `the_bounds_of_a_verify_block_come_only_from_a_bounds_field` in that
  file). Another field that holds the word `bounds` does not set the
  bounds, and a block with no `bounds` field has zero of each. The test
  `the_bounds_come_only_from_a_bounds_field` has a row for each of these
  rules.
- Two limits remain. What stands inside the parentheses, brackets or
  braces of a `bounds` field, or of a field that is not read, belongs to
  that field and is not read as an assertion (test
  `a_claim_inside_the_brackets_of_a_field_that_is_not_read_is_not_read`
  in `tests/integration/known_limits.rs`), and so does a line that begins
  with none of the nine words when the field goes on into it
  (`a_line_a_field_that_is_not_read_goes_on_into_is_not_read`). And
  `verify` evaluates an assertion without type-checking it: a true operand
  of `or` and a false operand of `and` settle the result whatever the
  other operand is, and the branch an `if` does not take is not examined
  (test `an_operand_that_is_not_needed_is_not_examined` in
  `tests/integration/known_limits.rs`).

Rule verbs. The parser recognizes nine verbs after `then`: `establish`,
`constitute`, `terminate`, `suspend`, `create`, `derive`, `activate`,
`defeat` and `make` (`crates/fidryn-syntax/src/parser.rs:46`). `establish`
and `constitute` both establish, and the last four all derive. A `then` or
an `otherwise` that none of these verbs follows is rejected with `E100`
(`crates/fidryn-syntax/src/parser.rs:2881`, `:2889`; test
`a_then_or_an_otherwise_with_no_consequence_is_e100` in that file).

The expression after the verb names a proposition. A consequence is
checked alike after each of the nine verbs, with one exception: the name
`Duty` after `create` (below). The lowering takes the expression as a
name with its arguments. A consequence whose expression has another form,
such as a number, a string, a sum, a set or a proposition under
`operative`, is rejected with `E210` in a diagnostic that names the rule
and the consequence (`crates/fidryn-hir/src/lib.rs:515`; test
`a_consequence_that_is_not_a_proposition_is_e210_and_names_the_rule_and_the_consequence`
in that file). The checker then requires the name, applied to its
arguments, to have the type `Prop`, by the check it makes of the operand
of `operative` (`crates/fidryn-check/src/lib.rs:598`). With
`proposition Exempt(who: LegalPerson)` and `entity Acme : LegalPerson`,
`Exempt(Acme)` has that type. A date, a duration, an interval, a
quantifier and `Acme(1)` do not have it. Each of these is rejected with
`E210` after each of the nine verbs, in a diagnostic of that check that
begins "`operative` in `R` requires a proposition" for a rule `R`: it
names the rule and not the consequence.
A consequence that is the name of an entity with no arguments, where the
type of the entity is neither `Prop` nor an alias of `Prop`, is rejected
with `E210` in a diagnostic that names the rule and the consequence
(`crates/fidryn-check/src/lib.rs:545`; test
`a_consequence_that_names_an_entity_is_e210` in that file). The name of
an entity whose type is `Prop` or an alias of `Prop` is not rejected by
that test, and the check above does not look for it among the entities,
so the name decides: `Pr` passes, and `pr`, `ABC`, `Final` and `If` are
`E210`. The test
`a_consequence_that_is_not_a_proposition_is_e210_and_names_the_rule_and_the_consequence`
in `tests/integration/unreadable_text.rs` takes a number, a string and
the name of an entity through the whole compiler.

After `create`, `Duty` with arguments is not checked as a proposition:
the checker requires the fields `bearer`, `claimant` and `content`
(`crates/fidryn-check/src/lib.rs:562`). `then create Duty(bearer = Acme,
claimant = Acme, content = Exempt(Acme))` passes, and `then create
Duty(1)` is `E210` three times, once for each field. In a module that
does not declare the name `Duty`, `then create Duty` and `then create
Duty()` are `E210` in the same way. Two tests take consequences through
each of the nine verbs, these four forms of `Duty` among them:
`create_is_checked_as_the_other_eight_verbs_are` in
`crates/fidryn-check/src/lib.rs`, and
`a_consequence_is_checked_alike_after_each_of_the_nine_verbs` in
`tests/integration/unreadable_text.rs`, which goes through the whole
compiler.

A capitalized name with no arguments is a proposition to the checker when
nothing declares it and the checker has no other reading of it
(`crates/fidryn-check/src/lib.rs:1321`): `then establish Flag` passes
with no `Flag` declared, and so does `operative Flag()` in a query. Every
module has the types and the record types that `prelude/prelude.fr`
declares (`crates/fidryn-hir/src/lib.rs:279`, `:280`), so the name of a
variant or of a record type of that file, such as `Final`, is declared in
every module: `then establish Final` is `E210` in a module that declares
no `Final` of its own. The checker has other readings of some names, and
those are no propositions either: for example `ABC`, which it reads as a
currency (`crates/fidryn-check/src/lib.rs:1218`), and `If` and `IF`,
which it reads as it reads `if` (test
`a_capitalized_name_the_checker_reads_otherwise_is_no_proposition` in
that file).

Rule selection. The worklist does not fire a rule that has a `select`
clause. Only `UniqueOccupant` succession reads the selection
(`crates/fidryn-eval/src/worklist.rs:210`).

## Expressions and types

- Integer, decimal and same-currency money arithmetic work. Integer
  division truncates. Adding amounts in two currencies is `E210`
  (`crates/fidryn-check/src/lib.rs:1389`, `crates/fidryn-eval/src/money.rs:7`).
- A duration cannot be added to a time: `start + 30 days` is `E210` (test
  `a_duration_cannot_be_added_to_a_time_in_an_expression`). A date literal
  such as `2026-12-31` has no type in an expression (test
  `a_date_literal_is_untyped_in_an_expression`). Two durations of the same
  kind pass the checker when added, but the evaluator refuses the sum
  (`crates/fidryn-eval/src/lib.rs:3067`).
- The duration units are `days`, `calendar_days`, `counted_days`,
  `working_days`, `hours`, `minutes` and `seconds`. There are no month or
  year units (`crates/fidryn-check/src/lib.rs:1776`). A duty's deadline
  counts `days`, `calendar_days` and `counted_days` alike, as 24 hours each.
  `working_days` needs a closed `working_days` closure and a
  `working_days_holidays` fact on the case (`crates/fidryn-eval/src/duty.rs:858`).
- Every instant is converted to UTC when it is read. The offset is not kept
  (`crates/fidryn-core/src/time.rs:20`).
- There are no lists, no aggregation, no string operations and no matching
  on variants. Strings can be compared for equality. An ordered comparison
  of two strings passes the checker and fails at run time
  (`crates/fidryn-core/src/types.rs:93`, `crates/fidryn-eval/src/lib.rs:3132`).
- A misspelled type name creates a new sort without a diagnostic (test
  `a_misspelled_type_name_becomes_a_new_sort_without_a_diagnostic`;
  `crates/fidryn-hir/src/body.rs:680`).
- A query that declares a result type must end in a typed value, so one
  that ends in `require` is rejected with `E210` (test
  `a_query_that_ends_in_a_require_has_an_untyped_result`).
- In `require determined P using Determine`, the `using` clause is parsed
  and discarded (`crates/fidryn-syntax/src/parser.rs:2399`). Any
  determination of `P` on the case that names a protocol and a decider
  answers it (`crates/fidryn-eval/src/worklist.rs:908`). A `using` with no
  protocol name in its statement is `E100` (test
  `using_at_the_end_of_a_line_is_e100_and_does_not_take_the_statement_below_it`
  in `crates/fidryn-syntax/src/parser.rs`).
- A module that does not pass the checker cannot be run (test
  `a_module_that_fails_the_checker_cannot_be_run` in
  `tests/integration/repaired_examples.rs`).

## Names the evaluator knows

Some behavior is attached to particular names. A program that uses
another name for the same idea does not get this behavior.

- Office succession (`UniqueOccupant`): a `judgment` that decides
  `Incapacitated` turns on the declared succession path
  (`crates/fidryn-eval/src/lib.rs:3613`). The successor rule's selection
  must test `operative Eligible(candidate, office)`
  (`crates/fidryn-eval/src/lib.rs:4088`) over `nominations_for(office)`
  ordered by `nomination_rank` (`:4075`, `:4118`). Acceptance is an
  `AcceptOffice` evidence record (`:4458`). The current occupant comes from
  `OccupancyRecord` evidence (`:5221`).
- `StatusOf`: only `OfficialFilingRecord` evidence counts as a filing
  (`crates/fidryn-eval/src/lib.rs:4504`). `FormedLLC` is the status name used
  when neither `status` nor `when_present` names one (`:4548`).
- `EvaluateClause`: `SpousalSupportWaiver` asks the case for a
  `PrenupEnforceability` judgment (`crates/fidryn-eval/src/lib.rs:4791`).
  `ChildSupportWaiver` is always prevented by
  `ChildSupportCannotBeAdverselyAffected` (`:4986`). Any other clause
  suspends with a `PrenupEnforceability` request that the case cannot
  answer.
- Duties: payment is performance only as a `PaymentRecord` evidence record
  (`crates/fidryn-eval/src/duty.rs:701`). When `due` names no anchor, the
  deadline counts from the case fact `invoice_date`, then `due_at`
  (`crates/fidryn-eval/src/duty.rs:944`).
- Tax: a function named `ordinary_income_tax` whose body is empty, written
  `{ }`, runs a built-in four-bracket table (10, 12, 22 and 24 percent, with
  brackets ending at 11,925, 48,475 and 103,350)
  (`crates/fidryn-eval/src/lib.rs:3174`, `:1515`). Any other function with
  an empty body is an engine error when it is called. A function with no
  `{ ... }` at all is rejected with `E100`, as the first table says.
- `EligibleForOpenTexturedCredit` always asks the case for an
  `OpenTexturedCredit` judgment (`crates/fidryn-eval/src/lib.rs:1129`).
- A term named `judgment` or `Adjudicated` answers from a
  `JudgmentOnTheMerits` determination and the `judgment_entry` fact
  (`crates/fidryn-eval/src/lib.rs:1124`, `:3205`).
- A term named `ResolveNormConflict` or `SelectApplicableLaw` runs a
  built-in conflict or law-selection request
  (`crates/fidryn-eval/src/lib.rs:1078`, `:1082`).

## Known limits pinned by tests

| Test | What it shows |
| --- | --- |
| `a_duration_cannot_be_added_to_a_time_in_an_expression` | `start + 30 days`, with `start: Time`, is rejected with `E210`. |
| `a_date_literal_is_untyped_in_an_expression` | `d <= 2026-12-31`, with `d: Date`, is rejected with `E210`, because the date literal has no type. |
| `the_activate_verb_is_lowered_as_derive` | `then activate Flagged(Acme)`: the parser accepts `activate` as a verb, and the checker lowers it as a derive consequence. |
| `an_import_that_names_nothing_is_accepted_and_its_alias_is_ignored` | An import of a module that does not exist, with an `alias`, passes the checker. |
| `power_and_legal_act_bodies_are_not_parsed_so_unknown_fields_pass` | A `power` and a `legal_act` with made-up fields pass the checker, and neither reaches the intermediate representation. |
| `a_source_declaration_does_not_reach_core_and_its_artifact_is_never_read` | A `source` block that names an artifact file that does not exist passes the checker, and no source declaration reaches the intermediate representation. |
| `an_office_keeps_none_of_its_declared_fields` | An office declared with `cardinality 3..9` and a competence reaches the intermediate representation with cardinality 0 to 1 and no competence. |
| `a_misspelled_type_name_becomes_a_new_sort_without_a_diagnostic` | A parameter of type `NaturalPersn` passes the checker. |
| `a_query_that_ends_in_a_require_has_an_untyped_result` | A query declared `-> Bool` whose goal ends in `require determined ...` is rejected with `E210`. |
| `a_rule_basis_that_names_nothing_is_accepted` | A rule written `from NoSuchSource`, where the module declares nothing of that name, passes the checker, and the name does not reach the intermediate representation. |
| `the_valid_time_and_record_time_lines_of_a_rule_are_not_applied` | A rule establishes a proposition, and below the consequence it says `valid_time [2026-08-14, +inf)` and `record_time [2026-08-14T00:00:00-04:00, +inf)`. A query that tests the proposition is determinate `true` on 2026-09-01, inside both intervals, and it is just as `true` on 2026-08-01 and on 2020-01-01, before they begin. |
| `a_claim_inside_the_brackets_of_a_field_that_is_not_read_is_not_read` | In a `verify` block, `assert false` between the braces of an `interpretations` field, a `for every` line between the parentheses of an `assuming` field, and `assert false` between the braces or the parentheses of a `bounds` field are not read. Each property is `proved`. |
| `an_operand_that_is_not_needed_is_not_examined` | `fidryn verify` evaluates an assertion and does not type-check it. `assert true or no_such_name` is `proved`, and so is `assert true or 1 +` followed by the line `for_all x in {1, 2}: x == 3`, which is the right operand of `+`. So are `assert true` followed by a blank line and the line `or asert false`, and the three lines `assuming fair ) +`, `assert true or not` and that `for_all` line, where the `for_all` line is the operand of `not`. |

## Budgets

Every run is bounded. Reaching a run limit stops the run with the engine
error `fuel exhausted`.

- Operations: 100,000 evaluator operations, unless `--budget N` sets another
  limit (`crates/fidryn-eval/src/lib.rs:22`).
- Worklist rounds: the rule worklist runs at most 64 rounds to reach a fixed
  point (`crates/fidryn-eval/src/worklist.rs:9`).
- Depth: evaluation nests at most 256 terms deep
  (`crates/fidryn-eval/src/lib.rs:1028`). The parser rejects an expression
  more than 128 levels deep or of more than 10,000 nodes, type arguments
  nested more than 128 deep, in a type that it reads and in a call, and a
  source file over 8 MiB or 262,144 tokens
  (`crates/fidryn-syntax/src/parser.rs:59`, `:2080`, `:3871`; parser
  tests `type_arguments_of_a_call_nested_deeper_than_the_limit_are_e100`
  and `a_chain_of_else_if_longer_than_the_limit_is_e100`). Each `else if`
  of a chain counts as a level.
- Text read after the parser: the parser steps over the body of a `duty`,
  a `decision` and an `interpretation_family`. The lowering reads the
  `bearer`, `claimant`, `attaches`, `content` and `due` of a duty, the
  `returns` of a decision and the definitions of a family from that text,
  with a reader of 128 levels. Parentheses, a call, a set, a record and an
  `if` each take two levels, so 63 of them are read one inside another. A
  `not` takes one, and a chain of `else if` is read up to 126 links. A
  declaration with a deeper expression in one of those places is rejected
  with `E100` (`crates/fidryn-hir/src/body.rs:1541`, `:1650`, `:1814`; in
  `crates/fidryn-hir/src/depth_tests.rs`, tests
  `an_expression_is_read_whole_or_its_declaration_is_rejected` and
  `a_chain_of_else_if_is_read_126_links_long_and_no_longer`). A type that is
  read from the text of a declaration, such as the result or a parameter
  of the operation of an `effect`, has the parser's limit: more than 128
  levels of `<` are `E100`, "type nesting exceeds the limit of 128"
  (`crates/fidryn-hir/src/body.rs:568`; `crates/fidryn-check/src/lib.rs:185`;
  in `crates/fidryn-check/src/depth_tests.rs`, test
  `a_type_read_from_text_nests_128_deep_and_no_deeper`).
- Trace size: a run keeps at most 99,999 evaluation records and the worklist
  at most 100,000. One record holds at most 1 MiB and a run's records at
  most 16 MiB in all (`crates/fidryn-eval/src/lib.rs:1027`,
  `crates/fidryn-eval/src/worklist.rs:290`,
  `crates/fidryn-eval/src/trace.rs:40`).
- `verify` stops after 65,536 steps, a domain of 4,096 values or a depth of
  128, and reports `unknown` (`crates/fidryn-verify/src/property.rs:18`).
