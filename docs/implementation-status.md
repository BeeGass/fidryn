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
| `query`, `fn`, `calc` | Yes | Yes | Yes | `crates/fidryn-syntax/src/parser.rs:1644`, `:2028` |
| `rule` | Yes | Guard, selection, consequences and `otherwise` fallback. The `from` basis is not checked and not kept. Verbs other than `establish`, `constitute`, `terminate`, `suspend` and `create` become derive or are dropped (see Rule verbs below) | Derive, establish, terminate and suspend. `create` is refused with an engine error when the rule fires | `crates/fidryn-check/src/lib.rs:3634`; `crates/fidryn-eval/src/worklist.rs:248`; test `the_activate_verb_is_lowered_as_derive` (`activate` is a verb the parser accepts and the checker lowers as a derive) |
| `judgment` | Yes. An unrecognized clause is kept as text | Yes | Concurrence policies only. One with an unrecognized clause or a `standard` suspends with a `JudgmentPolicy` request | `crates/fidryn-eval/src/judgment.rs:44`, `:57` |
| `observation` | Yes. An unrecognized clause is kept as text | Yes | Yes. An unrecognized clause is an engine error once a record of the observed schema is on the case | `crates/fidryn-eval/src/judgment.rs:486`, `:533` |
| `type`, `record_type`, `evidence_type`, `entity`, `proposition` | Yes | Yes | Not applicable | `crates/fidryn-check/src/lib.rs:3277` |
| `effect` | Re-read from the declaration text | Name and its first operation, with parameters and result | A call becomes a `needCustom` request to the case | `crates/fidryn-check/src/lib.rs:1806`, `:3370`; `crates/fidryn-eval/src/lib.rs:2346` |
| `duty` | Re-read from the declaration text. Unknown fields are skipped | Bearer, claimant, attachment, content, due | `duty_status` and `duty_step`, on one specialized path | `crates/fidryn-hir/src/body.rs:422`; `crates/fidryn-eval/src/duty.rs` |
| `decision` | Binders are parsed. Record requirements and `returns` are re-read from the declaration text | Binders, record requirements, the first `returns` | Through `RunDecision` | `crates/fidryn-hir/src/body.rs:304`, `:315` |
| `interpretation_family`, `nomination` | By splitting the text on keywords | Alternatives; candidate, office as text, rank. A nomination's `from` basis is dropped | Only by `UniqueOccupant` succession | `crates/fidryn-hir/src/lib.rs:914`, `:925`, `:940`; `crates/fidryn-eval/src/lib.rs:4130` |
| `office` | Name and parameters only | Fixed defaults: occupant type `LegalPerson`, at most one occupant, no competence | Not read at run time | `crates/fidryn-check/src/lib.rs:3342`; test `an_office_keeps_none_of_its_declared_fields` |
| `clause` | Name and parameters only | Body kept as text | Only through `EvaluateClause`, which knows two clause names (see below) | `crates/fidryn-eval/src/lib.rs:4791`, `:4986` |
| `legal_act` | Name and parameters only. Unknown fields are accepted | Nothing. The checker types a call of its name as a `LegalAction` | Nothing | `crates/fidryn-hir/src/lib.rs:663`; test `power_and_legal_act_bodies_are_not_parsed_so_unknown_fields_pass` |
| `power`, `scenario` | No. Unknown fields are accepted | Nothing | Nothing | `crates/fidryn-syntax/src/parser.rs:980`; `crates/fidryn-hir/src/lib.rs:707`; test `power_and_legal_act_bodies_are_not_parsed_so_unknown_fields_pass` |
| `conflict_doctrine` | No. Unknown fields are accepted. The text must name a declared clause after `defeat`, or the checker reports `E511` | Nothing | Nothing | `crates/fidryn-check/src/lib.rs:2190` |
| `source` | Fields parsed; an unknown field is a parse error | Nothing. The checker requires an `artifact` field (`E540`) | Nothing. The artifact file is never read, whatever the digest | `crates/fidryn-hir/src/lib.rs:376`; `crates/fidryn-check/src/lib.rs:3239`; test `a_source_declaration_does_not_reach_core_and_its_artifact_is_never_read` |
| `import` | Name. Version and digest are re-read from the text | Only a package under `packages/` is linked, and only when a path is checked | Not applicable. `alias` is ignored; with no digest and no package the import does nothing | `crates/fidryn-check/src/lib.rs:2232`, `:2587`; test `an_import_that_names_nothing_is_accepted_and_its_alias_is_ignored` |
| `verify` | The block is read as a list of fields, and nine words begin one. The expression after `assert`, and the expression of a field that begins with `for_all` or `exists`, is an assertion, in the order written. A `bounds` field is read. `interpretations`, `judgments`, `choices` and `assuming` fields are recognized and not read. A field that begins with `for`, which is a `for every` line, and an `assert` with no expression are kept as text, in an assertion that cannot be decided. "Where a verify field ends", below, says where a field ends, what becomes of a line the block does not recognize, and which two limits remain | Yes | Closed formulas only: literals, arithmetic, comparisons, Boolean connectives, and quantifiers over a literal set or bounded synthetic entities. An assertion that cannot be decided is unresolved, and the property is `unknown` unless another assertion is false. That holds for an assertion that needs a query or a proposition, and for one kept as text. A block with no assertion is `unknown` | `crates/fidryn-syntax/src/parser.rs:279`, `:1828`; `crates/fidryn-verify/src/property.rs:62`, `:123`, `:172`, `:206`; in `tests/integration/verify_blocks.rs`, tests `two_comparisons_on_consecutive_lines_are_two_assertions`, `a_false_quantifier_line_is_a_counterexample`, `a_top_level_exists_line_is_checked_as_an_assertion`, `an_assuming_line_is_recognized_and_not_read`, `a_for_every_line_leaves_the_property_unknown` and `a_block_with_no_assertion_is_unknown_whatever_its_comments_say` |

Unknown fields. The parser rejects an unknown field only inside a `source`
block and inside the request of a judgment's `otherwise suspend NeedJudgment
{ ... }` (`crates/fidryn-syntax/src/parser.rs:1613`, `:1311`). In the other
declarations whose body is a list of fields, an unknown field is skipped
without a diagnostic, and so is an unknown line in a `rule`. In a `judgment`
or an `observation`, an unrecognized clause is kept and refused at run time,
as the table says. In a `verify` block, a line the block does not recognize
is not skipped unless the field above it goes on into it: it is kept as an
assertion that cannot be decided, or the module is rejected, as the next
paragraphs say.

Where a verify field ends. The tests named here are in
`tests/integration/verify_blocks.rs`, and the parser tests are in
`crates/fidryn-syntax/src/parser.rs`.

- A field ends where its statement ends. An `assert`, `for_all`, `exists`
  or `bounds` field, and a field that is not read, ends earlier where the
  next field begins: it is read or stepped over as if a `;` stood there
  (`crates/fidryn-syntax/src/parser.rs:1952`, `:1973`; parser test
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
  diagnostic it would get before a `;`, which is `E100` "expected
  expression after operator"
  (`an_assertion_cut_off_by_the_next_assertion_is_e100`). An `assert` with
  no expression is an assertion that cannot be decided
  (`an_assert_with_no_expression_leaves_the_property_unknown`).
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
  (`tokens_that_are_not_a_word_below_an_assertion_are_e100`).
- A block with no assertion is `unknown`. Its text is not searched for a
  formula, so a comment is never read as an assertion
  (`crates/fidryn-hir/src/lib.rs:1005`; test
  `a_block_with_no_assertion_is_unknown_whatever_its_comments_say`).
- Two limits remain. What stands inside the parentheses, brackets or
  braces of a field that is not read belongs to that field (test
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
and `constitute` both establish, and the last four all derive. When a rule
has at least one `then` with a recognized verb, a `then` followed by any
other word is dropped without a diagnostic. When it has none, the rule's
text is read again and an unrecognized verb becomes a derive
(`crates/fidryn-hir/src/lib.rs:505`, `crates/fidryn-hir/src/body.rs:389`).

Rule selection. The worklist does not fire a rule that has a `select`
clause. Only `UniqueOccupant` succession reads the selection
(`crates/fidryn-eval/src/worklist.rs:210`).

## Expressions and types

- Integer, decimal and same-currency money arithmetic work. Integer
  division truncates. Adding amounts in two currencies is `E210`
  (`crates/fidryn-check/src/lib.rs:1285`, `crates/fidryn-eval/src/money.rs:7`).
- A duration cannot be added to a time: `start + 30 days` is `E210` (test
  `a_duration_cannot_be_added_to_a_time_in_an_expression`). A date literal
  such as `2026-12-31` has no type in an expression (test
  `a_date_literal_is_untyped_in_an_expression`). Two durations of the same
  kind pass the checker when added, but the evaluator refuses the sum
  (`crates/fidryn-eval/src/lib.rs:3067`).
- The duration units are `days`, `calendar_days`, `counted_days`,
  `working_days`, `hours`, `minutes` and `seconds`. There are no month or
  year units (`crates/fidryn-check/src/lib.rs:1672`). A duty's deadline
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
  `crates/fidryn-hir/src/body.rs:561`).
- A query that declares a result type must end in a typed value, so one
  that ends in `require` is rejected with `E210` (test
  `a_query_that_ends_in_a_require_has_an_untyped_result`).
- In `require determined P using Determine`, the `using` clause is parsed
  and discarded (`crates/fidryn-syntax/src/parser.rs:1755`). Any
  determination of `P` on the case that names a protocol and a decider
  answers it (`crates/fidryn-eval/src/worklist.rs:908`).
- A module that does not pass the checker cannot be run (test
  `a_module_that_fails_the_checker_cannot_be_run` in
  `tests/integration/repaired_examples.rs`).

## Declarations that are lost

A rule whose `from` basis is a bare name, as in `from Basis`, is not
checked, and the declaration that follows it is dropped without a
diagnostic (test
`a_rule_whose_basis_is_a_bare_name_is_not_checked_and_swallows_the_next_declaration`).
The rule then takes the `when` and `then` lines of the last braced block
in its text, which is the dropped declaration's body, and its own are lost
(`crates/fidryn-hir/src/lib.rs:498`, `crates/fidryn-hir/src/body.rs:347`).
So when the dropped declaration is a rule, that rule's guard and
consequence still run, under the earlier rule's name, and what is lost is
the earlier rule's own guard and consequence. When the dropped declaration
is a duty, the duty is gone and the earlier rule has the duty's
`attaches when` line as its guard and no consequence. A basis written as
a dotted name or a call, such as `from Instrument.clause("1.2")`, is not
affected.

Among the modules that pass, this hides one query: `encoding_is_bounded` in
`examples/us-federal/catalog/usc-titles.fr`, which `run` reports as an
unknown query. It also affects these rules and duties:

| Module | Rule whose own `when` and `then` are replaced | Declaration dropped after it |
| --- | --- | --- |
| `examples/us-federal/catalog/usc-titles.fr` | `CatalogHonesty` | query `encoding_is_bounded` |
| `examples/states/arkansas/homestead.fr` | `EligibilityIsNotABoolean` | rule `ForcedSaleProtection`, which runs under `EligibilityIsNotABoolean` |
| `examples/states/arkansas/homestead.fr` | `AnnexedRuralKeepsRuralAcreage` | duty `RespectHomestead` |
| `examples/states/colorado/famli.fr` | `BenefitsWhenCoveredAndQualifying` | duty `RespectFamliLeave` |
| `examples/states/florida/homestead.fr` | `AcreageLimit` | rule `ForcedSaleProtection`, which runs under `AcreageLimit` |
| `examples/states/idaho/noncompete.fr` | `WrittenKeyEmployeePath` | duty `RefrainFromDirectCompetition` |
| `examples/states/kansas/homestead.fr` | `OccupancyConstitutes` | rule `FamilyOccupancyConstitutes`, which runs under `OccupancyConstitutes` |
| `examples/states/kansas/homestead.fr` | `ForcedSaleProtection` | duty `RespectHomestead` |
| `examples/states/louisiana/community-property.fr` | `DefaultCommunity` | rule `EqualShares`, which runs under `DefaultCommunity` |
| `examples/states/michigan/no-fault-pip.fr` | `NoFaultLiability` | duty `PayPIPBenefits` |
| `examples/states/south-dakota/wage-payment.fr` | `MonthlyPayAttaches` | rule `FinalPayOnEmployerSeparation`, which runs under `MonthlyPayAttaches` |
| `examples/states/south-dakota/wage-payment.fr` | `FinalPayOnQuitWithoutTermContract` | duty `PayAllWagesDue` |

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
- Tax: a function named `ordinary_income_tax` with no body runs a built-in
  four-bracket table (10, 12, 22 and 24 percent, with brackets ending at
  11,925, 48,475 and 103,350) (`crates/fidryn-eval/src/lib.rs:3174`,
  `:1515`). Any other function with no body is an engine error.
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
| `a_rule_whose_basis_is_a_bare_name_is_not_checked_and_swallows_the_next_declaration` | A rule written `from Basis` with an undeclared name in its guard passes the checker, and the query after it is missing. |
| `a_claim_inside_the_brackets_of_a_field_that_is_not_read_is_not_read` | In a `verify` block, `assert false` between the braces of an `interpretations` field, and a `for every` line between the parentheses of an `assuming` field, are not read. Each property is `proved`. |
| `an_operand_that_is_not_needed_is_not_examined` | `fidryn verify` evaluates an assertion and does not type-check it. `assert true or no_such_name` is `proved`, and so is `assert true or 1 +` followed by the line `for_all x in {1, 2}: x == 3`, which is the right operand of `+`. |

## Budgets

Every run is bounded. Reaching a run limit stops the run with the engine
error `fuel exhausted`.

- Operations: 100,000 evaluator operations, unless `--budget N` sets another
  limit (`crates/fidryn-eval/src/lib.rs:22`).
- Worklist rounds: the rule worklist runs at most 64 rounds to reach a fixed
  point (`crates/fidryn-eval/src/worklist.rs:9`).
- Depth: evaluation nests at most 256 terms deep
  (`crates/fidryn-eval/src/lib.rs:1028`). The parser rejects an expression
  more than 128 levels deep or of more than 10,000 nodes, and a source file
  over 8 MiB or 262,144 tokens (`crates/fidryn-syntax/src/parser.rs:59`).
- Trace size: a run keeps at most 99,999 evaluation records and the worklist
  at most 100,000. One record holds at most 1 MiB and a run's records at
  most 16 MiB in all (`crates/fidryn-eval/src/lib.rs:1027`,
  `crates/fidryn-eval/src/worklist.rs:290`,
  `crates/fidryn-eval/src/trace.rs:40`).
- `verify` stops after 65,536 steps, a domain of 4,096 values or a depth of
  128, and reports `unknown` (`crates/fidryn-verify/src/property.rs:18`).
