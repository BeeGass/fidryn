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
| `query`, `fn`, `calc` | Yes. A `calc` or an `fn` with no `{ ... }` body is `E100`. A `query` with none is `E430` from the checker. A statement that a query body, a `goal Evaluate` block, the body of a `calc` or an `fn`, or a `transaction` block cannot read is `E100`, wherever it stands in the block | Yes | Yes | `crates/fidryn-syntax/src/parser.rs:2049`, `:2503`, `:2567`, `:2161`; in `tests/integration/declaration_text.rs`, tests `a_function_with_no_body_is_e100_whatever_stands_beside_or_below_it` and `a_query_with_no_body_is_e430_whatever_stands_beside_or_below_it`; in `tests/integration/unreadable_text.rs`, test `a_statement_that_cannot_be_read_is_e100_when_it_is_the_last_of_its_block` |
| `rule` | Yes. The body holds `when`, `then`, `otherwise`, `require`, `select` and `source` statements. Other text in it is `E100`, and so is a `then` or an `otherwise` that none of the nine verbs follows, and a `when`, a `require` or a `source` with no expression after it. A `valid_time` or `record_time` line with an interval is read, and nothing of it is kept. An interval that lacks a bound is `E100` | Guard, selection, consequences and `otherwise` fallback. A consequence whose expression does not have the form of a proposition is `E210`, and so is one that is the name of an entity (see Rule verbs below). The `from` basis is not checked and not kept. Verbs other than `establish`, `constitute`, `terminate`, `suspend` and `create` become derive (see Rule verbs below). Every consequence is given the interval of all time, whatever a `valid_time` or `record_time` line says | Derive, establish, terminate and suspend. `create` is refused with an engine error when the rule fires | `crates/fidryn-syntax/src/parser.rs:2752`, `:2808`, `:2817`, `:2826`; `crates/fidryn-check/src/lib.rs:3291`, `:3653`; `crates/fidryn-eval/src/worklist.rs:248`; in `crates/fidryn-syntax/src/parser.rs`, tests `text_in_a_rule_that_is_no_rule_statement_is_e100_and_only_it_is_stepped_over`, `a_when_a_require_or_a_source_with_nothing_after_it_is_e100`, `an_interval_of_a_rule_that_lacks_a_bound_is_e100` and `a_valid_time_or_record_time_line_of_a_rule_is_read_and_nothing_of_it_is_kept`; in `tests/integration/known_limits.rs`, tests `the_valid_time_and_record_time_lines_of_a_rule_are_not_applied` (the two interval lines), `a_rule_basis_that_names_nothing_is_accepted` (the basis) and `the_activate_verb_is_lowered_as_derive` (`activate` is a verb the parser accepts and the checker lowers as a derive) |
| `judgment` | Yes. An unrecognized clause is kept as text | Yes | Concurrence policies only. One with an unrecognized clause or a `standard` suspends with a `JudgmentPolicy` request | `crates/fidryn-eval/src/judgment.rs:44`, `:57` |
| `observation` | Yes. An unrecognized clause is kept as text | Yes | Yes. An unrecognized clause is an engine error once a record of the observed schema is on the case | `crates/fidryn-eval/src/judgment.rs:486`, `:533` |
| `type`, `record_type`, `evidence_type`, `entity`, `proposition` | Yes | Yes | Not applicable | `crates/fidryn-check/src/lib.rs:3296` |
| `effect` | Re-read from the declaration text | Name and its first operation, with parameters and result | A call becomes a `needCustom` request to the case | `crates/fidryn-check/src/lib.rs:1825`, `:3389`; `crates/fidryn-eval/src/lib.rs:2346` |
| `duty` | Re-read from the declaration text. Unknown fields are skipped | Bearer, claimant, attachment, content, due | `duty_status` and `duty_step`, on one specialized path | `crates/fidryn-hir/src/body.rs:456`; `crates/fidryn-eval/src/duty.rs` |
| `decision` | Binders are parsed. Record requirements and `returns` are re-read from the declaration text | Binders, record requirements, the first `returns` | Through `RunDecision` | `crates/fidryn-hir/src/body.rs:315`, `:326` |
| `interpretation_family`, `nomination` | By splitting the text on keywords | Alternatives; candidate, office as text, rank. A nomination's `from` basis is dropped | Only by `UniqueOccupant` succession | `crates/fidryn-hir/src/lib.rs:935`, `:946`, `:961`; `crates/fidryn-eval/src/lib.rs:4130` |
| `office` | Name and parameters only | Fixed defaults: occupant type `LegalPerson`, at most one occupant, no competence | Not read at run time | `crates/fidryn-check/src/lib.rs:3361`; test `an_office_keeps_none_of_its_declared_fields` |
| `clause` | Name and parameters only | Body kept as text | Only through `EvaluateClause`, which knows two clause names (see below) | `crates/fidryn-eval/src/lib.rs:4791`, `:4986` |
| `legal_act` | Name and parameters only. Unknown fields are accepted | Nothing. The checker types a call of its name as a `LegalAction` | Nothing | `crates/fidryn-hir/src/lib.rs:684`; test `power_and_legal_act_bodies_are_not_parsed_so_unknown_fields_pass` |
| `power`, `scenario` | No. Unknown fields are accepted | Nothing | Nothing | `crates/fidryn-syntax/src/parser.rs:1200`; `crates/fidryn-hir/src/lib.rs:728`; test `power_and_legal_act_bodies_are_not_parsed_so_unknown_fields_pass` |
| `conflict_doctrine` | No. Unknown fields are accepted. The text must name a declared clause after `defeat`, or the checker reports `E511` | Nothing | Nothing | `crates/fidryn-check/src/lib.rs:2209` |
| `source` | Fields parsed; an unknown field is a parse error | Nothing. The checker requires an `artifact` field (`E540`) | Nothing. The artifact file is never read, whatever the digest | `crates/fidryn-hir/src/lib.rs:376`; `crates/fidryn-check/src/lib.rs:3258`; test `a_source_declaration_does_not_reach_core_and_its_artifact_is_never_read` |
| `import` | Name. Version and digest are re-read from the text | Only a package under `packages/` is linked, and only when a path is checked | Not applicable. `alias` is ignored; with no digest and no package the import does nothing | `crates/fidryn-check/src/lib.rs:2251`, `:2606`; test `an_import_that_names_nothing_is_accepted_and_its_alias_is_ignored` |
| `verify` | The block is read as a list of fields, and nine words begin one. The expression after `assert`, and the expression of a field that begins with `for_all` or `exists`, is an assertion, in the order written. A `bounds` field is read for three numbers, and nothing but a `bounds` field sets them. `interpretations`, `judgments`, `choices` and `assuming` fields are recognized and not read. A field that begins with `for`, which is a `for every` line, and an `assert` with no expression are kept as text, in an assertion that cannot be decided. "Where a verify field ends", below, says where a field ends, what becomes of a line the block does not recognize, and which two limits remain | Yes | Closed formulas only: literals, arithmetic, comparisons, Boolean connectives, and quantifiers over a literal set or bounded synthetic entities. An assertion that cannot be decided is unresolved, and the property is `unknown` unless another assertion is false. That holds for an assertion that needs a query or a proposition, and for one kept as text. A block with no assertion is `unknown` | `crates/fidryn-syntax/src/parser.rs:411`, `:2279`; `crates/fidryn-verify/src/property.rs:62`, `:123`, `:172`, `:206`; in `tests/integration/verify_blocks.rs`, tests `two_comparisons_on_consecutive_lines_are_two_assertions`, `a_false_quantifier_line_is_a_counterexample`, `a_top_level_exists_line_is_checked_as_an_assertion`, `an_assuming_line_is_recognized_and_not_read`, `a_for_every_line_leaves_the_property_unknown`, `a_block_with_no_assertion_is_unknown_whatever_its_comments_say` and `the_bounds_come_only_from_a_bounds_field` |

Unknown fields. The parser rejects an unknown field inside a `source`
block and inside the request of a judgment's `otherwise suspend NeedJudgment
{ ... }` (`crates/fidryn-syntax/src/parser.rs:2018`, `:1618`). In a `rule` it
rejects text that is none of the statements the table lists for a rule
(`crates/fidryn-syntax/src/parser.rs:2752`). In the other declarations whose
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
  (`crates/fidryn-syntax/src/parser.rs:2425`, `:2446`; parser test
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
  (`crates/fidryn-hir/src/lib.rs:1026`; test
  `a_block_with_no_assertion_is_unknown_whatever_its_comments_say`).
- The bounds come from a `bounds` field and from nowhere else. The parser
  records the field as the text of its tokens and leaves out every
  comment, wherever it stands: inside the field, after it on its line, or
  on the lines below it (`crates/fidryn-syntax/src/parser.rs:2321`,
  `:2389`; parser test
  `a_bounds_field_is_recorded_as_the_text_of_its_tokens`). The numbers are
  read from those tokens, so no comment sets a number and the contents of
  a string are not read (`crates/fidryn-hir/src/lib.rs:1066`). Each number
  is read at the first place where its word, `persons`, `events` or
  `time_points`, stands as a token. When the next token is an integer, or
  a `:` followed by an integer, that integer is the number. Otherwise the
  number is zero, as it is when the word is not in the field and when the
  integer does not fit in 32 bits (`crates/fidryn-hir/src/lib.rs:1094` and
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
(`crates/fidryn-syntax/src/parser.rs:2679`, `:2687`; test
`a_then_or_an_otherwise_with_no_consequence_is_e100` in that file).

The expression after the verb names a proposition. A consequence whose
expression has another form, such as a number, a string, a sum, a set or
a proposition under `operative`, is rejected with `E210` in a diagnostic
that names the rule and the consequence
(`crates/fidryn-hir/src/lib.rs:519`; test
`a_consequence_that_is_not_a_proposition_is_e210_and_names_the_rule_and_the_consequence`
in that file). So is a consequence that is the name of an entity
(`crates/fidryn-check/src/lib.rs:461`; test
`a_consequence_that_names_an_entity_is_e210` in that file). The test
`a_consequence_that_is_not_a_proposition_is_e210_and_names_the_rule_and_the_consequence`
in `tests/integration/unreadable_text.rs` takes a number, a string and
the name of an entity through the whole compiler. A capitalized name with
no arguments that the module does not declare is a proposition to the
checker (`crates/fidryn-check/src/lib.rs:1236`): `then establish Flag`
passes with no `Flag` declared, and so does `operative Flag()` in a
query.

Rule selection. The worklist does not fire a rule that has a `select`
clause. Only `UniqueOccupant` succession reads the selection
(`crates/fidryn-eval/src/worklist.rs:210`).

## Expressions and types

- Integer, decimal and same-currency money arithmetic work. Integer
  division truncates. Adding amounts in two currencies is `E210`
  (`crates/fidryn-check/src/lib.rs:1304`, `crates/fidryn-eval/src/money.rs:7`).
- A duration cannot be added to a time: `start + 30 days` is `E210` (test
  `a_duration_cannot_be_added_to_a_time_in_an_expression`). A date literal
  such as `2026-12-31` has no type in an expression (test
  `a_date_literal_is_untyped_in_an_expression`). Two durations of the same
  kind pass the checker when added, but the evaluator refuses the sum
  (`crates/fidryn-eval/src/lib.rs:3067`).
- The duration units are `days`, `calendar_days`, `counted_days`,
  `working_days`, `hours`, `minutes` and `seconds`. There are no month or
  year units (`crates/fidryn-check/src/lib.rs:1691`). A duty's deadline
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
  `crates/fidryn-hir/src/body.rs:595`).
- A query that declares a result type must end in a typed value, so one
  that ends in `require` is rejected with `E210` (test
  `a_query_that_ends_in_a_require_has_an_untyped_result`).
- In `require determined P using Determine`, the `using` clause is parsed
  and discarded (`crates/fidryn-syntax/src/parser.rs:2197`). Any
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
| `a_comment_at_the_end_of_a_line_inside_an_effect_is_read_into_its_result_type` | An `effect` is re-read from its text, comments included. With the operation written `request(name: String) -> Bool // asks a clerk`, the result type is the text `Bool // asks a clerk`, and a query that requires the operation is rejected with `E210`. With the comment on a line of its own the module passes the checker. |
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
  (`crates/fidryn-syntax/src/parser.rs:59`, `:1888`, `:3669`; parser
  tests `type_arguments_of_a_call_nested_deeper_than_the_limit_are_e100`
  and `a_chain_of_else_if_longer_than_the_limit_is_e100`). Each `else if`
  of a chain counts as a level.
- Trace size: a run keeps at most 99,999 evaluation records and the worklist
  at most 100,000. One record holds at most 1 MiB and a run's records at
  most 16 MiB in all (`crates/fidryn-eval/src/lib.rs:1027`,
  `crates/fidryn-eval/src/worklist.rs:290`,
  `crates/fidryn-eval/src/trace.rs:40`).
- `verify` stops after 65,536 steps, a domain of 4,096 values or a depth of
  128, and reports `unknown` (`crates/fidryn-verify/src/property.rs:18`).
