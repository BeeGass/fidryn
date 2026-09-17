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
`run`, start with [Getting started](getting-started.md).

Fidryn (FID-rin) is a language for legal instruments: precise where
law is mechanical, explicit where judgment enters. This repository is
the v0.1 research interpreter. Example modules are fixtures. They are
not legal advice, not operative instruments, and not a complete
statement of any jurisdiction's law.

The grammar is closed: unknown body fields are parse errors. Copy from
an existing `.fr` file when you are unsure a spelling parses. The
reference is [`grammar.ebnf`](../grammar.ebnf).

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

`digest fixture` is a fixture profile, not a hash of the file. It is
not byte-verified authentication. A 32- or 64-digit hex digest is
blake3 of the artifact bytes (64 hex: the full digest; 32 hex: the
first 16 bytes). `check` of a `.fr` path hashes the file at `artifact`
relative to the module directory. A missing file or a mismatch is
unauthenticated; digest-required imports fail with E200. Hex without
readable bytes is not a fixture profile. The source-manifest JSON uses
the same `"digest": "fixture"` versus hex distinction.

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

`Occupies(Bryan, TrusteeOf(BRT))` is a proposition about that office,
not a `Bool`.

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
    }
}
```

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

`require determined P using Determine` requests a competent
determination instead of treating `P` as a Bool.

## Calc, conditionals, money, and durations

`calc` is a total, effect-free function. It must not recurse (E530).

```
calc return_deadline() -> Duration<counted_days> {
    return 21 counted_days
}

calc federal_hourly_floor() -> Money<USD> {
    return USD(7.25)
}

query automatic deadline_running(end: Time, as_of: Time) -> Bool {
    return as_of <= end + return_deadline()
}
```

Money is `USD(100.00)`, never an untyped decimal. Durations carry a
calendar kind (`counted_days`, `calendar_days`, `working_days`,
`days`, `hours`, `minutes`, `seconds`). `21 counted_days` is not
`21 days`.

`if` / `else` is Boolean control flow. The condition must be a `Bool`,
not a proposition. [`prelude/tax.fr`](../prelude/tax.fr) uses it on
amounts.

## Rules

A `rule` fires when a guard holds, then derives or establishes a
proposition. The kind is `derive`, `constitutive`, or `prescriptive`.

```
rule InitialTrustee : constitutive
    from Instrument.clause("1.2")
{
    when operative TrustInstrumentExecuted(BRT) in TrustCreation(BRT)
    then establish Occupies(Bryan, TrusteeOf(BRT))
}
```

`when operative P` tests legal status. `when determined P` tests a
competent determination already on the case. `then derive` and
`then establish` are the usual consequences. Guards must use
`operative`, `determined`, `assumed`, or `necessarily` — not a bare
proposition.

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

`run` does not pick `Strict` because it is written first. If the query
needs a reading and the case has not selected one, the outcome is
`suspended` (`needInterpretation`).

## Nominations

```
nomination Alice for TrusteeOf(BRT) rank 1
    from Instrument.clause("4.4.a")

nomination Bob for TrusteeOf(BRT) rank 2
    from Instrument.clause("4.4.b")
```

Rank `1` is higher than rank `2`. Duplicate ranks for the same office
are E410. Rank is not occupancy.

## Duties

```
duty PayInvoice {
    bearer Payer
    claimant Payee
    attaches when operative InvoiceIssued(Payer)
    content USD(100.00)
    due 0 counted_days after invoice_date
}
```

`duty_status(Name)` looks up that duty declaration plus the case. It
does not invent performance.

## Scenarios (in-module)

```
scenario InsideCityHalfAcre {
    assume OccupiesAsResidence(Owner, Home)
    assume InsideMunicipality(Home)
    assume within_urban_acreage(0.5)
    at 2026-09-17T12:00:00-04:00
}
```

## Verify

```
verify Trivial {
    assert true
}
```

## What the language will not do

Fidryn will not hide judgment in a `Bool`. Fidryn will not invent a
completion. Fidryn will not treat `Prop` as `Bool`. Those three
refusals are the language.
