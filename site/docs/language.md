---
title: "Language"
description: "Fidryn language map: modules, queries, rules, duties, and what the language will not do."
url: "https://fidryn.onlygass.dev/docs/language"
markdown: "https://fidryn.onlygass.dev/docs/language.md"
author: "Bryan Gass"
---

> Canonical HTML: https://fidryn.onlygass.dev/docs/language
> This markdown mirror is for agents and plain-text readers.

# The Fidryn language

Read this as a map of the constructs you will see in `.fr` files, then
open the fixtures and check them. For build, `check`, and the first
`run`, start with [Getting started](https://fidryn.onlygass.dev/docs/getting-started.md).

Fidryn (FID-rin) is a language for legal instruments: precise where
law is mechanical, explicit where judgment enters. This repository is
the 0.2 research interpreter. Example modules are fixtures. They are
not legal advice, not operative instruments, and not a complete
statement of any jurisdiction's law.

The grammar in `grammar.ebnf` is closed, but the 0.2 parser enforces
that only inside `source` blocks and the request block of a judgment's
`otherwise suspend NeedJudgment`. In most other declarations an unknown
field is skipped without a diagnostic; see
[implementation status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md). Copy from an existing
`.fr` file when you are unsure a spelling parses. The reference is
[`grammar.ebnf`](https://github.com/BeeGass/fidryn/blob/main/grammar.ebnf).


## Surface inventory (status-qualified)

The grammar ([`grammar.ebnf`](https://github.com/BeeGass/fidryn/blob/main/grammar.ebnf)) is broader than this map.
Use the table as a honesty check; open a fixture for the full field list.
Support in 0.2 is **parsed / checked / executed** only where noted — see
[implementation-status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md).

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

The trust fixture also declares a source manifest, resolved relative
to the module file:

```
source_manifest "sources/ma-trust-fixture.manifest.json"
```

A declared path that is missing or malformed is a diagnostic. Modules
that omit a manifest get an empty default. CLI `check` / `run` of a
path can byte-authenticate artifacts against files next to the module.
Pasted source in the [Mill](https://fidryn.onlygass.dev/docs/mill.md) does not.

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
whatever `digest` says ([implementation status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md)).

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
[Mill](https://fidryn.onlygass.dev/docs/mill.md) reads no manifest and no files.


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
[`examples/trust/bryan-revocable-trust.fr`](https://github.com/BeeGass/fidryn/blob/main/examples/trust/bryan-revocable-trust.fr):

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
[implementation status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md)).

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

```
observation OfficialFilingObservation(record: OfficialFilingRecord)
observation EndOfTenancy(record: TenancyRecord)
```

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
([`harbor-robotics.fr`](https://github.com/BeeGass/fidryn/blob/main/examples/massachusetts-llc/harbor-robotics.fr)).

`EvaluateClause` is an additional query plan used by some instruments
(for example [`examples/prenup/ava-noah.fr`](https://github.com/BeeGass/fidryn/blob/main/examples/prenup/ava-noah.fr)):

```
query provision_result() -> ... ! {Observe, Determine, Interpret} {
    goal EvaluateClause { /* clause plan */ }
}
```

The effect row `! {Observe, Determine, Interpret}` is permission, not
a promise that the engine will invent evidence. If the case does not
supply what the effect needs, the outcome is `suspended`. `run` never
fills that request. See [Outcomes](https://fidryn.onlygass.dev/docs/outcomes.md) and
[Cases and time](https://fidryn.onlygass.dev/docs/cases-and-time.md).

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
`requirement failed`). That is why [Getting started](https://fidryn.onlygass.dev/docs/getting-started.md)
runs both. Nested `seq` under `+`, a call, or `if` resumes by skipping
the completed prefix, so a later term does not re-run an earlier
attach.

`require determined P using Determine` asks for a determination of `P`
instead of treating `P` as a Bool. In 0.2 the `using` clause is parsed
and discarded. Any determination of `P` on the case that names a
protocol and a decider answers the requirement. A negative determination
fails it, and with none the query suspends with a request for a judgment
on `P`.

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

Fidryn 0.2 cannot add a duration to a time in an expression: `end +
return_deadline()`, with `end: Time`, is `E210`. A deadline is computed
only by a duty's `due` clause; see [Duties](#duties).

Money is `USD(100.00)`, never an untyped decimal. Durations carry a
calendar kind (`counted_days`, `calendar_days`, `working_days`,
`days`, `hours`, `minutes`, `seconds`). `21 counted_days` and `21 days`
have different types, but a duty's `due` clause counts each of them as
21 days of 24 hours.

`if` / `else` is Boolean control flow. The condition must be a `Bool`,
not a proposition. [`prelude/tax.fr`](https://github.com/BeeGass/fidryn/blob/main/prelude/tax.fr) uses it on
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
([implementation-status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md)); this is not a
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
[CLI](https://fidryn.onlygass.dev/docs/cli.md) `explore` with declared bounds. It does not collapse to
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
`ChildSupportWaiver`; see [implementation status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md).

When the rules' staged effects both add and withdraw the same
proposition, the outcome kind is `normConflict`. No `conflict_doctrine`
is consulted, and list order is never a silent tie-break.

## Duties

A `duty` names bearer, claimant, attachment, content, and due time.
From [`tests/programs/late-payment.fr`](https://github.com/BeeGass/fidryn/blob/main/tests/programs/late-payment.fr):

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
[implementation-status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md)). It does not invent
performance and is not a general legal-duty / transaction engine. Status is one of `Unresolved`,
`Attached`, `Performed`, `Breached`, `Cured`, `Discharged`. A late
perform can remain `Performed` with `breached: true`. Changing
`due 0 counted_days` to `due 15 counted_days` changes when the same
case becomes `Breached`; see `late-payment-extended.fr`.

Declaring a duty does not attach it. Attachment waits on the `when`
guard. A `kind: duty` event on the case does not commit performance
unless an authority grant covers the action, or the event is an
explicit `assumption`. That gate is in
[Cases and time](https://fidryn.onlygass.dev/docs/cases-and-time.md).


## Transactions and quantifiers

`transaction { ... }` blocks exist in the grammar and are exercised by
`tests/programs/transaction-atomic.fr`. Support is **partial** — not a
full atomic commit engine for arbitrary legal acts.

`for_all` / `exists` quantify over **finite declared domains** (and
closures where required). Open domains suspend. Nested / open-world
theorem proving is not implemented. Prefer reading
[implementation-status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md) before treating a
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

Grammar: `ScenarioDecl` in [`grammar.ebnf`](https://github.com/BeeGass/fidryn/blob/main/grammar.ebnf). Fidryn
0.2 does not interpret a `scenario` block: the parser reads its name and
skips its body, and nothing in it reaches evaluation
([implementation status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md)). Neither `run` nor
`run --scenario` reads these blocks.

Scenario evaluation is driven by the case record. Its `assumptions`
rows (`id`, `payload`) are applied when you pass `run --scenario`, and
the report's `executionMode` is then `scenario`. The mill uses scenario
mode when the pasted case has a nonempty `assumptions` list. A default
`run` is operative and ignores `assumptions`. See
[Cases and time](https://fidryn.onlygass.dev/docs/cases-and-time.md) and [CLI](https://fidryn.onlygass.dev/docs/cli.md).

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
assertion that names a query or a proposition is unresolved, and the
property is then `unknown` unless another assertion is false. For those
properties,

```text
cargo run -p fidryn-cli -- verify PATH --property NAME
```

reports `unknown` rather than pretending they are theorems. Fixture
modules often write honesty properties in English-shaped `assert`
lines (`assert automatic pay_due_immediately has empty effect row`).
Treat those as documentation the checker records as a verify item, not
as a solved proof. The empty-effect-row rule for `automatic` is E420
whether or not a `verify` block repeats it.

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

Those three refusals are the language. The [CLI](https://fidryn.onlygass.dev/docs/cli.md) will not
choose a completion either. The [examples](https://fidryn.onlygass.dev/docs/examples.md) show the
refusals in larger fixtures. The [Mill](https://fidryn.onlygass.dev/docs/mill.md) will not live-file.
