# The Fidryn language

Read this as a map of the constructs you will see in `.fr` files, then
open the fixtures and check them. For build, `check`, and the first
`run`, start with [Getting started](getting-started.md).

Fidryn (FID-rin) is a language for legal instruments: precise where
law is mechanical, explicit where judgment enters. This repository is
the 0.2 research interpreter. Example modules are fixtures. They are
not legal advice, not operative instruments, and not a complete
statement of any jurisdiction's law.

The grammar in `grammar.ebnf` is closed, but the 0.2 parser enforces
that only inside `source` blocks, the bodies of rules and the request
block of a judgment's `otherwise suspend NeedJudgment`. In most other
declarations an unknown field is skipped without a diagnostic; see
[implementation status](implementation-status.md). Copy from an existing
`.fr` file when you are unsure a spelling parses. The reference is
[`grammar.ebnf`](../grammar.ebnf).


## Surface inventory (status-qualified)

The grammar ([`grammar.ebnf`](../grammar.ebnf)) is broader than this map.
Use the table as a honesty check; open a fixture for the full field list.
Support in 0.2 is **parsed / checked / executed** only where noted — see
[implementation-status](implementation-status.md).

| Construct | Fixture example | Status note |
| --- | --- | --- |
| `import` (+ `digest` / `alias`) | `examples/trust/bryan-revocable-trust.fr` | Parsed. `alias` is accepted and ignored. An import with no digest and no matching package does nothing. |
| `record_type` / `evidence_type` | FOIA / trust fixtures | Parsed/checked; used as named evidence schemas |
| `observation` | LLC / state homestead modules | Parsed/checked; Observe matches schema + known-at |
| `scenario` | all 50 `examples/states/` modules | Name read; body not interpreted. Scenario runs use the case record's `assumptions`, not these blocks |
| `Evaluate` / `UniqueOccupant` / `RunDecision` / `StatusOf` | trust, late-payment, LLC | Executed query plans |
| `EvaluateClause` | `examples/prenup/ava-noah.fr` | Executed clause-plan queries |
| `power` / `legal_act` | trust, FOIA, prenup | Name and parameters read; body not interpreted, so unknown fields pass. A `legal_act` name types as a `LegalAction` in calls. Neither reaches evaluation |
| `clause` | prenup, minimum-wage | Name and parameters read; body kept as text. At run time only `EvaluateClause` uses it, and the evaluator knows two clause names |
| `conflict_doctrine` | prenup, minimum-wage | Body not interpreted; the text must name a declared clause after `defeat` (E511). Never consulted when staged effects disagree |
| `transaction` | `tests/programs/transaction-atomic.fr` | Partial; not a full commit engine |
| `for_all` / `exists` | status + grammar | Finite declared domains only; open domains suspend; no open-world proofs |
| `fn` | `packages/logic/true.fr` | Parsed, checked and executed; a query can call a function |
| Module type parameters | status | Type-name substitution only; no cross-file value-parameter calculus |

## Module header

Every file is one module. The header names the instrument, the dated
slice it encodes, and what it refuses to claim.

```
module Programs.LatePayment version "0.1.0" {
    jurisdiction Test
    source_snapshot "2026-09-17-late-payment"
    effective_at 2026-09-17
    recorded_at 2026-09-17T12:00:00Z

    outside_scope { complete_instruments }

    // queries, duties, and interpretation families follow
}
```

`jurisdiction` is a name, not a choice of engine. `source_snapshot` is
the dated slice this file encodes. `effective_at` is a date.
`recorded_at` is a date-time (`Z` or a numeric offset).
`outside_scope` is a set of named exclusions; those names reappear on
the outcome's `modelBoundary`.

A comment begins with `//` and runs to the end of its line. There is no
block comment: `/* ... */` is read as ordinary tokens, not skipped. A
brace inside a comment or a string is not a brace: it neither opens nor
closes the body of a declaration. The text kept for a declaration holds
the comments inside it, and a part that is read from that text can take
one in: in an `effect`, a comment after the result type of an operation
is read into the type, and a query that uses the operation is rejected
with `E210` (see [implementation-status](implementation-status.md)).

The trust fixture also declares a source manifest, resolved relative
to the module file:

```
source_manifest "sources/ma-trust-fixture.manifest.json"
```

A declared path that is missing or malformed is a diagnostic. Modules
that omit a manifest get an empty default. CLI `check` / `run` of a
path can byte-authenticate artifacts against files next to the module.
Pasted source in the [Mill](mill.md) does not.

## Sources and digests

A `source { }` block names an artifact the module is encoding.

```
source FinalPayStatute {
    kind statute
    authority CaliforniaLegislature
    citation "Cal. Lab. Code §§ 201-203"
    artifact "sources/ca-lab-201-203.txt"
    digest fixture
    effective [2026-09-17, +inf)
}
```

In 0.2 a `source` block is a record for the reader. The parser rejects
an unknown field in it, and the checker requires an `artifact` field
(`E540`). The checker also types the source's name as a `String`, so a
query that returns `FinalPayStatute` passes `check`, but `run` fails
with `unbound identifier`. Nothing else reads the block: it does not
reach evaluation, and the file named by `artifact` is never read,
whatever `digest` says ([implementation status](implementation-status.md)).

Digests are checked in the source manifest instead. That is the JSON
file the module names with `source_manifest`, or, when it names none,
`sources/manifest.json` or the only `*.manifest.json` in `sources/` next
to the module. Each manifest artifact has a `path` and a `digest`.
`"digest": "fixture"` is a fixture trust profile, not a hash of the file
and not byte verification. A 32- or 64-digit hex digest is blake3 of the
artifact bytes (64 hex: the full digest; 32 hex: the first 16 bytes).
`check` and `run` of a `.fr` path read the file at `path`, relative to
the module directory or to `--source-root`, and compare. A missing file
or a mismatch leaves the artifact unauthenticated, and an import that
requires that digest fails with E200. Pasted source in the
[Mill](mill.md) reads no manifest and no files.


## Imports

Modules may import another module by name and version. An import that
carries a `digest` must find that digest on an authenticated artifact of
the source manifest above, or the checker reports E200. An `alias` line
in an import is accepted and ignored in 0.2.

```
import MA.TrustLaw.Fixture version "2026-08-23"
import Agency.FOIARegulations version "fixture-2026-08-23" {
    digest fixture
}
```

In an import, `digest fixture` asks for a manifest artifact whose digest
is `fixture`. That is a trust profile, not byte verification. A hex
digest in an import asks for a manifest artifact with that digest whose
file `check` or `run` of a path has read and matched. With no matching
manifest artifact the import is E200. Mill paste loads no manifest, so
there an import with a digest is always E200.

## Entities, propositions, and offices

Name the parties and the propositions you will later test. A
proposition is not a Boolean.

```
entity Payer : NaturalPerson
entity Payee : NaturalPerson

proposition InvoiceIssued(person: NaturalPerson)
proposition Eligible(person: LegalPerson, office: Office)
```

An `office` is a position someone can occupy. From
[`examples/trust/bryan-revocable-trust.fr`](../examples/trust/bryan-revocable-trust.fr):

```
office TrusteeOf(trust: Trust) occupied_by LegalPerson {
    cardinality 0..1
    acquired_by Effective(AcceptOffice)
    suspended_by OperativeIncapacity
    lost_by {Death, EffectiveResignation, EffectiveRemoval}
    competence {AdministerTrust, DecideTrustDistributions}
}
```

Fidryn 0.2 reads only an office's name and parameters. The rest of the
declaration, `occupied_by` and every field in the body, is discarded:
every office reaches the intermediate representation with occupant type
`LegalPerson`, at most one occupant and no competence, and nothing reads
the declaration at run time (test
`an_office_keeps_none_of_its_declared_fields`; see
[implementation status](implementation-status.md)).

`Occupies(Bryan, TrusteeOf(BRT))` is a proposition about that office,
not a `Bool`.


## Record and evidence types

Name structured payloads the case and Observe paths will carry.

```
record_type FOIARequest {
    // fields as in examples/foia/foia-request.fr
}

evidence_type PhysicianCertificate {
    // fields as in examples/trust/bryan-revocable-trust.fr
}
```

These declarations are part of the closed grammar. Copy field shapes from
an existing fixture rather than inventing spellings.

## Observations

An `observation` names an evidence-shaped fact the evaluator may Observe.
Its header binds one record and, after `establishes`, gives the
proposition that the record establishes. Its body holds the conditions
the record must meet. This one is from
[`examples/tax/federal-tax.fr`](../examples/tax/federal-tax.fr):

```
observation DomesticFormationRecord(record: BeneficialOwnershipReport)
establishes DomesticCompany(record.company)
    from FinCENBOIFinalRule.section("domestic-entity")
{
    require record.company == AcmeLLC
}
```

A header with no `establishes` or with no `{ ... }` body is rejected with
`E100`.

Observe still requires a matching case evidence row with `observedAt` at
or before `--known-at`. Declaring an observation does not invent the
record.

## Queries, goals, and effects

A query is the thing you `run`. The body is `return` / `require`
statements or an explicit `goal`. Fixtures use `Evaluate`,
`UniqueOccupant`, `RunDecision`, and `StatusOf`.

```
query due() -> Money<USD> {
    goal Evaluate { USD(100.00) }
}

query acting_trustee() -> LegalPerson ! {Observe, Determine, Interpret} {
    goal UniqueOccupant { office TrusteeOf(BRT) }
}

query paid_on_time() -> Bool ! {Observe} {
    goal RunDecision { decision TimelyPayment }
}

query entity_status() -> EntityStatus ! {Observe, Determine} {
    goal StatusOf {
        status FormedLLC(HarborRobotics)
        when_present FormedLLC(HarborRobotics)
        when_closed_absent NotFormedLLC(HarborRobotics)
    }
}
```

`Evaluate` computes a term. `UniqueOccupant` asks who occupies an
office. `RunDecision` runs a named `decision` against the case.
`StatusOf` reports a legal status when present, or the closed-absent
form when the domain is closed
([`harbor-robotics.fr`](../examples/massachusetts-llc/harbor-robotics.fr)).

`EvaluateClause` is an additional query plan used by some instruments
(for example [`examples/prenup/ava-noah.fr`](../examples/prenup/ava-noah.fr)):

```
query provision_result() -> ... ! {Observe, Determine, Interpret} {
    goal EvaluateClause {
        // clause plan
    }
}
```

The effect row `! {Observe, Determine, Interpret}` is permission, not
a promise that the engine will invent evidence. If the case does not
supply what the effect needs, the outcome is `suspended`. `run` never
fills that request. See [Outcomes](outcomes.md) and
[Cases and time](cases-and-time.md).

## Automatic versus judgment

Mark a query `automatic` only when the answer is mechanical and the
effect row is empty.

```
query automatic pay_due_immediately(discharged: Bool) -> Bool {
    return discharged
}

query waiting_time_penalty() -> Bool ! {Determine} {
    goal Evaluate {
        require determined WillfulFailureToPay(Employer)
            using Determine
        return true
    }
}
```

The waiting-time query answers `true` once the case records a
determination that establishes `WillfulFailureToPay(Employer)`. It
suspends otherwise, including when the determination is negative (tests
`an_established_determination_makes_the_judgment_query_true` and
`a_negative_determination_suspends_and_never_answers_false` in
`tests/integration/repaired_examples.rs`).

An automatic query that reaches `Determine`, `Observe`, or `Interpret`
is E420. Willful failure is a judgment, not a Bool you compute in the
module. The waiting-time query is allowed to suspend.

A `judgment` declaration names who decides a proposition, what record
it needs, and what happens on established / rejected / otherwise. The
trust fixture's incapacity judgment requires two distinct physician
certificates and otherwise suspends `NeedJudgment`. The language will
not collapse "the physicians have not spoken" into `false`.

## Require, return, and sequential evaluation

Statements in a block run left to right. Several statements lower to
a `seq`; the last value is the result. `require` is a gate: `true`
continues, `false` suspends and does not return the later value.

```
query q() -> Int { require true; return 7 }
query r() -> Int { require false; return 7 }
```

`q` is determinate `7`. `r` is `suspended` (`needCustom` /
`requirement failed`). That is why [Getting started](getting-started.md)
runs both. Nested `seq` under `+`, a call, or `if` resumes by skipping
the completed prefix, so a later term does not re-run an earlier
attach.

`require determined P using Determine` asks for a determination of `P`
instead of treating `P` as a Bool. In 0.2 the `using` clause is parsed
and discarded. Any determination of `P` on the case that names a
protocol and a decider answers the requirement. A negative determination
fails it, and with none the query suspends with a request for a judgment
on `P`. The protocol name stands in the statement of `using`: a `using`
that ends its statement, with no name after it, is rejected with `E100`,
and the line below it is not read as the name.

A statement that a query body, a `goal Evaluate` block, the body of a
`calc` or an `fn`, or a `transaction` block cannot read is rejected with
`E100`, wherever it stands in the block. A `return` with no expression is
one.

## Where a statement ends

A statement ends at a `;` or at the end of its line. It goes on into the
next line in two cases.

- The line ends in a token that cannot end a statement. The tokens that
  can are a name or word, a literal, `)`, `]`, `}`, `*` and `>`. A line
  that ends in one of the prefix words `not`, `operative`, `effective`,
  `established`, `determined`, `always`, `active`, `every`, `unique` and
  `closed` goes on as well.
- The next line begins with a continuation: one of the words `and`, `or`,
  `in`, `where`, `then`, `else`, `from`, `decides`, `occupied_by`,
  `establishes`, `using`, `for`, `rank`, `as`, `to`, `version` and `fuel`,
  or one of the tokens `.`, `,`, `->`, `=>`, `:`, `=`, `|`, `<`, `!`, `{`,
  `(` and `[`.

A blank line has no token, and neither has a line that holds only a
comment. The first case holds across such lines. The second holds across
them only for a line that begins with `and`, `or`, `in`, `.`, `,`, `->`,
`=>`, `:`, `=`, `|` or `<`. No statement begins with one of those, so a
condition can carry a comment between its lines:

```
rule BothConditions : derive from "cite" {
    when operative A
        // the second condition
        and operative B
    then derive C
}
```

The guard of that rule is `operative A and operative B`. Each of the
other continuations, such as `then`, `for` or `(`, can begin a statement
of its own, so below a blank line or a comment line it does not continue
the statement above. That holds where the parser tests whether a
statement has ended, as it does after the condition of an `if` and after
the word `else`: the block of a branch does not stand below a blank line
or a comment line. In some places where it expects the next part of a form
it does not test, and reads on across blank and comment lines: an `else`
after the block of an `if`, for example, and the `{ ... }` block that
follows the domain of a quantifier or the word `transaction`.

Parentheses, brackets and braces do not change the rule: a line break
inside them ends the expression before it in the same cases. `(1`
followed by the line `+ 2)` is rejected, because `+` is not a
continuation. Write `(1 +` and then `2)`.

Text the parser steps over without reading it, such as a `verify` field
it does not read or a clause it keeps as text, ends by the same rule with
three differences. A line break inside its parentheses, brackets or
braces does not end it. A prefix word at the end of a line does not carry
it into the next line. And it ends after the `}` that closes its
outermost brace, unless a continuation follows.

A declaration does not go on into the declaration below it. Outside the
parentheses, brackets and braces that a declaration has opened and not
closed, a line that begins with a declaration keyword is no part of that
declaration, whatever the line above it ends in. Blank lines and comment
lines between the two change nothing. There is one exception: below the
header of a `for_all` or an `exists` that ends in `:`, a line that begins
with `for_all` or `exists` is the body of the quantifier.

So the first case above does not carry a header into such a line. A
header that leaves no bracket open and whose line ends in `,`, `.`, `:`,
`!`, `->`, `=>`, `=`, `==`, `!=`, `<`, `<=`, `>=`, `+`, `-`, `/`, `|` or
`..` is not finished above such a line, and the module is rejected with
`E100`: `rule R : derive from Basis +` above the line `clause C {` is
rejected, and `clause C` is a declaration with its own block. That holds
wherever in a header the token stands: after an operator that lacks its
operand, after a name that ends in `.`, after the `!` of an effect row,
among type arguments that `<` opens and no `>` closes, and after the `:`,
`->` or `=` that a type follows. A prefix word at the end of the `from`
basis of a rule, or of an expression in the header of a judgment or an
observation, lacks its operand in the same way: the module is rejected
with `E100`, and the declaration below is read. A type on the next line
that is no declaration keyword is read: `entity Pat :` followed by the
line `NaturalPerson` declares `Pat`. Inside a bracket that the
declaration has left open, the lines below can still be read as what the
bracket holds: below `proposition P(`, the line `clause C {` is read as a
parameter, and the module is rejected with no clause declared.

A keyword that takes a name, such as `entity`, `rule` or `query`, is
rejected with `E100` when its statement ends with it: the name is not
read from the next line. It is rejected as well when the token after it
is not a name, as in `power { holder A }` or `entity : NaturalPerson`.
The six header keywords, `jurisdiction`, `source_snapshot`,
`source_manifest`, `effective_at`, `recorded_at` and `outside_scope`, are
rejected with `E100` in the same way when the statement ends with the
keyword: the value is not read from the next line.

## Calc, conditionals, money, and durations

`calc` is a total, effect-free function. It must not recurse (E530).

```
calc return_deadline() -> Duration<counted_days> {
    return 21 counted_days
}

calc federal_hourly_floor() -> Money<USD> {
    return USD(7.25)
}
```

A `calc` or an `fn` needs its `{ ... }` body. A header with none is
rejected with `E100`.

Fidryn 0.2 cannot add a duration to a time in an expression: `end +
return_deadline()`, with `end: Time`, is `E210`. A deadline is computed
only by a duty's `due` clause; see [Duties](#duties).

Money is `USD(100.00)`, never an untyped decimal. Durations carry a
calendar kind (`counted_days`, `calendar_days`, `working_days`,
`days`, `hours`, `minutes`, `seconds`). `21 counted_days` and `21 days`
have different types, but a duty's `due` clause counts each of them as
21 days of 24 hours.

`if` / `else` is Boolean control flow. The condition must be a `Bool`,
not a proposition. [`prelude/tax.fr`](../prelude/tax.fr) uses it on
amounts:

```
if amount <= 11925.00 {
    amount * 0.10
} else {
    if amount <= 48475.00 {
        11925.00 * 0.10 + (amount - 11925.00) * 0.12
    } else {
        if amount <= 103350.00 {
            11925.00 * 0.10 + (48475.00 - 11925.00) * 0.12 + (amount - 48475.00) * 0.22
        } else {
            17651.00 + (amount - 103350.00) * 0.24
        }
    }
}
```

The `{` after the condition opens the branch. In the condition of an
`if`, in the domain of a quantifier, and in the header of a rule, a
judgment or an observation, a name followed by `{` is therefore the name
alone and is not applied to the block: `if flag { 1 } else { 2 }` tests
`flag`. A record constructor written with its name needs parentheses
there, as in `if (R { x: 1 }) == r { 1 } else { 2 }`.

An `if` or an `else` needs its branch in the same statement. With none,
the module is rejected with `E100`.

This is a rejected guard (E310):

```
query bad() -> LegalPerson {
    goal Evaluate { if Incapacitated(Bryan) { Bryan } }
}
```

Use `when operative P` on a rule, or `require determined P using
Determine` in a query. `Prop` has no conversion to `Bool`.

## Rules

A `rule` fires when a guard holds, then derives or establishes a
proposition. The current staged worklist is **bounded and specialized**
([implementation-status](implementation-status.md)); this is not a
general rule engine, and prescriptive duties / full transaction commit
are not that worklist. The kind is `derive`, `constitutive`, or `prescriptive`.

```
rule InitialTrustee : constitutive
    from Instrument.clause("1.2")
{
    when operative TrustInstrumentExecuted(BRT) in TrustCreation(BRT)
    then establish Occupies(Bryan, TrusteeOf(BRT))
}
```

The `from` basis can also be a bare name, as in `from Instrument`: the `{`
after it opens the rule's body. A rule needs that body. A rule header
with no `{ ... }` after it is rejected with `E100`.

The body holds `when`, `then`, `otherwise`, `require`, `select` and
`source` statements. Other text in it is rejected with `E100`, and so is
a `then` or an `otherwise` that no consequence follows, and a `when`, a
`require` or a `source` with no expression after it. A consequence names
a proposition: `then establish 5` is rejected with `E210`, and so is a
consequence that is the name of an entity whose type is neither `Prop`
nor an alias of `Prop`. Two more lines are accepted:
`valid_time` and `record_time`, each followed by an interval with both of
its bounds, such as `[2026-08-14, +inf)`. Fidryn 0.2 reads them and does
not apply them: the consequences of the rule hold at every time, whatever
the interval says (see
[implementation-status](implementation-status.md#known-limits-pinned-by-tests)).

`when operative P` tests legal status. `in TrustCreation(BRT)` names the
legal context of that test. The guard still turns on the proposition
`TrustInstrumentExecuted(BRT)`. It is not a proposition named `in`.
`when determined P` tests for a determination of `P` already on the case.
`then derive` and `then establish` are the usual consequences. Guards
must use `operative`, `determined`, `assumed`, or `necessarily` — not a
bare proposition. The successor rule in the trust fixture also
`select unique candidate from nominations_for(...)` with
`require unique nomination_rank`. A tie is not a silent pick.

## Interpretation families

When the instrument still admits more than one reading, declare the
family and what each alternative establishes.

```
interpretation_family DeadlineMeaning {
    alternative Strict {
        defines GracePeriod(0) as established
        defines StrictDeadline() as established
        defines ExtendedDeadline() as not_established
    }
    alternative Extended {
        defines GracePeriod(15) as established
        defines StrictDeadline() as not_established
        defines ExtendedDeadline() as established
    }
}
```

Each `defines P as established` or `defines P as not_established` is
a claim in that world. The case lists still-admissible alternatives
under `admissibleCompletions.interpretations`. `run` does not pick
`Strict` because it is written first. If the query needs a reading
and the case has not selected one, the outcome is `suspended`
(`needInterpretation`).

The trust fixture binds a family to a clause and names a selector
(`explicit_instrument_definition or controlling_interpretation or
request Interpret`). That is how a court selection on the case becomes
operative. It is not a default vote.

## Nominations

A nomination is a ranked candidate for an office.

```
nomination Alice for TrusteeOf(BRT) rank 1
    from Instrument.clause("4.4.a")

nomination Bob for TrusteeOf(BRT) rank 2
    from Instrument.clause("4.4.b")
```

Rank `1` is higher than rank `2`. Duplicate ranks for the same office
are E410. Rank is not occupancy: `UniqueOccupant` still needs an
occupancy record, eligibility, and any required acceptance. Open
eligibility stays `suspended`, or becomes `contingent` under
[CLI](cli.md) `explore` with declared bounds. It does not collapse to
the first name on the list.


## Powers, legal acts, clauses, and conflict doctrines

These constructs appear in fixtures (trust, FOIA, prenup, minimum wage).

```
power SettlorAmendmentPower { ... }
legal_act AcceptOffice(candidate: LegalPerson, office: Office) { ... }
conflict_doctrine ChildSupportCannotBeAdverselyAffected ...
```

Fidryn 0.2 does not interpret the bodies of `power`, `legal_act`,
`conflict_doctrine` or `scenario` declarations, so an unknown field in
one of them passes without a diagnostic. A `clause` body is kept as text.
The evaluator knows two clause names, `SpousalSupportWaiver` and
`ChildSupportWaiver`; see [implementation status](implementation-status.md).

When the rules' staged effects both add and withdraw the same
proposition, the outcome kind is `normConflict`. No `conflict_doctrine`
is consulted, and list order is never a silent tie-break.

## Duties

A `duty` names bearer, claimant, attachment, content, and due time.
From [`tests/programs/late-payment.fr`](../tests/programs/late-payment.fr):

```
duty PayInvoice {
    bearer Payer
    claimant Payee
    attaches when operative InvoiceIssued(Payer)
    content USD(100.00)
    due 0 counted_days after invoice_date
}

query obligation_status() -> DutyState {
    goal Evaluate {
        duty_status(PayInvoice)
    }
}
```

`duty_status(Name)` looks up that duty declaration plus the case on the
**implemented specialized duty path** (see late-payment fixtures and
[implementation-status](implementation-status.md)). It does not invent
performance and is not a general legal-duty / transaction engine. Status is one of `Unresolved`,
`Attached`, `Performed`, `Breached`, `Cured`, `Discharged`. A late
perform can remain `Performed` with `breached: true`. Changing
`due 0 counted_days` to `due 15 counted_days` changes when the same
case becomes `Breached`; see `late-payment-extended.fr`.

Declaring a duty does not attach it. Attachment waits on the `when`
guard. A `kind: duty` event on the case does not commit performance
unless an authority grant covers the action, or the event is an
explicit `assumption`. That gate is in
[Cases and time](cases-and-time.md).


## Transactions and quantifiers

`transaction { ... }` blocks exist in the grammar and are exercised by
`tests/programs/transaction-atomic.fr`. Support is **partial** — not a
full atomic commit engine for arbitrary legal acts.

`for_all` / `exists` quantify over **finite declared domains** (and
closures where required). The body follows the domain after a `:` or as a
block: `for_all x in Scores: x > 0` and `for_all x in Scores { x > 0 }`
are the same quantifier. Open domains suspend. Nested / open-world
theorem proving is not implemented. Prefer reading
[implementation-status](implementation-status.md) before treating a
quantified `verify` as a proved theorem.

## Scenarios (in-module)

A `scenario` block writes down example hypotheses in fixtures that do
not ship companion JSON case files (for example under
`examples/states/`).

```
scenario InsideCityHalfAcre {
    assume OccupiesAsResidence(Owner, Home)
    assume InsideMunicipality(Home)
    assume within_urban_acreage(0.5)
    at 2026-09-17T12:00:00-04:00
}
```

Grammar: `ScenarioDecl` in [`grammar.ebnf`](../grammar.ebnf). Fidryn
0.2 does not interpret a `scenario` block: the parser reads its name and
skips its body, and nothing in it reaches evaluation
([implementation status](implementation-status.md)). Neither `run` nor
`run --scenario` reads these blocks.

Scenario evaluation is driven by the case record. Its `assumptions`
rows (`id`, `payload`) are applied when you pass `run --scenario`, and
the report's `executionMode` is then `scenario`. The mill uses scenario
mode when the pasted case has a nonempty `assumptions` list. A default
`run` is operative and ignores `assumptions`. See
[Cases and time](cases-and-time.md) and [CLI](cli.md).

## Verify

A `verify` block is a named property made of `assert` lines. Fidryn 0.2
decides closed formulas only: literals, arithmetic, comparisons, Boolean
connectives, and quantifiers over a literal set or bounded synthetic
entities.

```
verify Arithmetic {
    assert true
    assert 1 + 1 == 2
}
```

Both assertions hold, so the property is proved. A false assertion, such
as `assert false` or `assert 1 + 1 == 3`, is a counterexample. An
assertion that cannot be decided without a query or a proposition is
unresolved, and the property is then `unknown` unless another assertion
is false. For those properties,

```text
cargo run -p fidryn-cli -- verify PATH --property NAME
```

reports `unknown` rather than pretending they are theorems. Fixture
modules often write honesty properties in English-shaped `assert`
lines (`assert automatic pay_due_immediately has empty effect row`).
Treat those as documentation the checker records as a verify item, not
as a solved proof. The empty-effect-row rule for `automatic` is E420
whether or not a `verify` block repeats it.

### The fields of a verify block

A verify block is a list of fields. Nine words begin a field:

| First word | What `verify` does with the field |
| --- | --- |
| `assert` | The expression after it is an assertion. |
| `for_all`, `exists` | The expression the word begins is an assertion, checked in the same way. |
| `bounds` | It is read for three numbers, the synthetic persons, events and time points, and for nothing else. |
| `interpretations`, `judgments`, `choices`, `assuming` | The field is recognized and not read. |
| `for` | A `for every` line is not evaluated. It is kept as an assertion that cannot be decided. |

An assertion that cannot be decided leaves the property `unknown` unless
another assertion is false. A block with no assertion is `unknown` too,
whatever its comments say: a comment is never read as an assertion.

The bounds come from a `bounds` field and from nowhere else. Each of the
three numbers is read at the first place where its word, `persons`,
`events` or `time_points`, stands as a token in the field. When the next
token is an integer, or a `:` followed by an integer, that integer is the
number. Otherwise the number is zero, as it is when the word is not in the
field. So `bounds persons x persons 2` gives zero persons, and so do
`persons: 2.5`, which is not an integer, and an integer that does not fit
in 32 bits.

No comment sets a number, wherever it stands: inside the field, after it
on its line, or on the lines below it. The contents of a string are not
read either. Another field that holds the word `bounds` does not set the
bounds, and a block with no `bounds` field has zero of each. When a block
has several `bounds` fields, the last one counts. The test
`the_bounds_come_only_from_a_bounds_field` in
`tests/integration/verify_blocks.rs` pins each of these rules.

A field ends where its statement ends; see
[Where a statement ends](#where-a-statement-ends). An `assert`, `for_all`,
`exists` or `bounds` field, and a field that is not read, ends earlier
where the next field begins, as if a `;` stood there:

- The word `assert` begins a field wherever it stands, in the middle of a
  line too.
- A line that begins with one of the other eight words begins a field. In
  the middle of a line those words do not cut a field short:
  `assuming fair for every x in {1, 2}: x == 3` is one `assuming` field,
  and none of it is read.
- There is one exception. Below an `assert`, `for_all` or `exists` field,
  a line that begins with `for_all` or `exists` does not end that field.
  It continues the line above when the statement goes on into it, and is
  a field of its own when the statement has ended. This is one assertion,
  and it is proved:

  ```
  verify EveryNumberHasAnEqual {
      for_all x in {1, 2}:
          exists y in {1, 2}: x == y
  }
  ```

- Nothing inside the parentheses, brackets or braces of a field begins a
  field.

A `for every` line, and a line the block does not recognize, have no
early end. The parser steps over such a line as text it does not read,
together with every line that text goes on into, an `assert` line
included, and the whole is one assertion that cannot be decided.

An expression that is complete where the next field begins is an
assertion. One that is cut off there gets the diagnostic it would get
before a `;`: `assert true or 1 +` followed by the line `assert false` is
rejected with `E100`, "expected expression after operator", and
`assert if true { true } else` followed by the line `assert true` is
rejected with `E100`, "expected `{` after `else`". Where the parser has no
diagnostic before a `;`, there is none here either: `not` with nothing
after it gives no expression. An `assert` with no expression, `assert not`
included, is an assertion that cannot be decided.

Text that follows the expression of an `assert`, `for_all` or `exists`
field is read as the next field when it begins with one of the nine
words. That is rare on one line, because a word that directly follows an
expression is normally taken into the expression as an operand:
`assert true assuming fair` is one assertion, and it cannot be decided.
Before a `:` or an `=` a word is not an operand, so
`assert true assuming: x` is the assertion `true` followed by an
`assuming` field.

Text that begins with none of the nine words, such as a misspelled
`assert`, is not dropped either. When the field above goes on into it, it
is part of that field. Otherwise it is read as a line of its own and kept
as an assertion that cannot be decided, with one difference after an
`assert`: the `assert` and the text that follows its expression, on the
same line or below it, are together one assertion that cannot be decided,
and the module is rejected with `E100` when that text begins with
something other than a word.

Two limits remain.

- What stands inside the parentheses, brackets or braces of a `bounds`
  field, or of a field that is not read, belongs to that field, and so
  does a line that begins with none of the nine words when the field goes
  on into it. None of it is read as an assertion: an `assert false`
  between the braces of an `interpretations` field or of a `bounds` field
  is not read.
- `verify` evaluates an assertion; it does not type-check it. A true
  operand of `or` and a false operand of `and` settle the result whatever
  the other operand is, and the branch an `if` does not take is not
  examined. So `assert true or no_such_name` is proved, and so is
  `assert true or 1 +` followed by the line
  `for_all x in {1, 2}: x == 3`, where the second line is the right
  operand of `+`.

## What the language will not do

Fidryn will not hide judgment in a `Bool`. A proposition is a `Prop`.
`if Incapacitated(Bryan)` is E310. Write `operative`, `determined`,
`assumed`, or `necessarily`, or request `Determine`.

Fidryn will not invent a completion. `run` evaluates the case you
supplied. Missing interpretation, evidence, or choice yields
`suspended`. An empty admissible set is `inconsistent` or
`suspended`, never a vacuous determinate answer. `explore` searches
only the finite domains you declared.

Fidryn will not treat `Prop` as `Bool`. `defines Eligible(Alice,
TrusteeOf(BRT)) as not_established` is not `false`. It is a reading of
a proposition in one named alternative.

Those three refusals are the language. The [CLI](cli.md) will not
choose a completion either. The [examples](examples.md) show the
refusals in larger fixtures. The [Mill](mill.md) will not live-file.
