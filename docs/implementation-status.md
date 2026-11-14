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
| `query`, `fn`, `calc` | Yes | Yes | Yes | `crates/fidryn-syntax/src/parser.rs:1546`, `:1846` |
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
| `power`, `scenario` | No. Unknown fields are accepted | Nothing | Nothing | `crates/fidryn-syntax/src/parser.rs:882`; `crates/fidryn-hir/src/lib.rs:707`; test `power_and_legal_act_bodies_are_not_parsed_so_unknown_fields_pass` |
| `conflict_doctrine` | No. Unknown fields are accepted. The text must name a declared clause after `defeat`, or the checker reports `E511` | Nothing | Nothing | `crates/fidryn-check/src/lib.rs:2190` |
| `source` | Fields parsed; an unknown field is a parse error | Nothing. The checker requires an `artifact` field (`E540`) | Nothing. The artifact file is never read, whatever the digest | `crates/fidryn-hir/src/lib.rs:376`; `crates/fidryn-check/src/lib.rs:3239`; test `a_source_declaration_does_not_reach_core_and_its_artifact_is_never_read` |
| `import` | Name. Version and digest are re-read from the text | Only a package under `packages/` is linked, and only when a path is checked | Not applicable. `alias` is ignored; with no digest and no package the import does nothing | `crates/fidryn-check/src/lib.rs:2232`, `:2587`; test `an_import_that_names_nothing_is_accepted_and_its_alias_is_ignored` |
| `verify` | Assertions kept | Yes | Closed formulas only: literals, arithmetic, comparisons, Boolean connectives, and quantifiers over a literal set or bounded synthetic entities. An assertion that cannot be decided without a query or a proposition is unresolved, and the property is `unknown` unless another assertion is false (but see [Assertions that are swallowed](#assertions-that-are-swallowed)) | `crates/fidryn-verify/src/property.rs:123`, `:172`, `:206` |

Unknown fields. The parser rejects an unknown field only inside a `source`
block and inside the request of a judgment's `otherwise suspend NeedJudgment
{ ... }` (`crates/fidryn-syntax/src/parser.rs:1515`, `:1213`). In the other
declarations whose body is a list of fields, an unknown field is skipped
without a diagnostic, and so is an unknown line in a `rule`. In a `judgment`
or an `observation`, an unrecognized clause is kept and refused at run time,
as the table says.

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
  and discarded (`crates/fidryn-syntax/src/parser.rs:1657`). Any
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

## Assertions that are swallowed

In a `verify` block, an assertion that ends in an integer literal, such
as `assert 1 + 1 == 2`, swallows the `assert` line that follows it, and
that line swallows the next one if it also ends in an integer literal.
The lines joined this way are decided as one assertion (test
`an_assertion_ending_in_an_integer_literal_swallows_the_next_assertion`),
and the assertion numbers in a counterexample count them as one. While it
looks for a duration unit after the integer, the parser steps over the
line break, so the expression continues into the next line, starting by
applying the integer to the word `assert`
(`crates/fidryn-syntax/src/parser.rs:2682`, `:2239`). That application
cannot be evaluated (`crates/fidryn-verify/src/property.rs:165`), so the
joined assertion is unresolved unless the rest of it settles the result.
`verify` ignores an operand of `and` or `or` when the other operand
settles the result, and it evaluates only the branch of an `if` that is
taken (`crates/fidryn-verify/src/property.rs:140`, `:206`). An `and`,
`or` or `if` in any of the joined lines can settle the result this way.
So `assert 1 + 1 == 2` followed by `assert false` is `unknown`, but each
of these blocks is `proved`, although it holds a false assertion:
`assert true or 1 == 2` followed by `assert false`, and
`assert 1 + 1 == 3` followed by `assert true or true`. A `proved` verdict
is therefore not reliable for a block that has such an assertion. When
both lines are comparisons, as with `assert 1 + 1 == 2` followed by
`assert 2 == 2`, the module is rejected with `E100`. An assertion that
ends in `;`, or in a literal in parentheses such as `(2)`, is not
affected.

In the modules shipped with 0.2, 41 of those that pass the checker
declare a `verify` property, one each. None of those 41 properties has an
assertion that ends in an integer literal with another line after it, so
none is affected, and `verify` reports all 41 `unknown`.

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
| `an_assertion_ending_in_an_integer_literal_swallows_the_next_assertion` | A `verify` block of `assert true or 1 == 2` followed by `assert false` reaches the intermediate representation with one assertion, and `verify` reports it `proved`. |

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
