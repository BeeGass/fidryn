# Outcomes

This guide explains the six honest results a Fidryn query can return.
Fidryn is a research interpreter. An outcome is not a court order, not
legal advice, and not a filing.

The six-kind result is a `fidryn.outcome/v0.1` object, defined by
[`schemas/outcome-v0.1.json`](../schemas/outcome-v0.1.json). Field names
are camelCase. Trust, coverage, execution mode, and assumptions that
cannot fit that schema live on the evaluation-report envelope
([`fidryn.evaluation-report/v0.1`](../schemas/evaluation-report-v0.1.json));
they are not extra keys on the outcome object (`additionalProperties`
is false).

See [CLI](cli.md), the [language grammar](../grammar.ebnf), and
[Cases and time](cases-and-time.md).

## Envelope

CLI `run` / `explore` and mill `/api/run` / `/api/explore` emit
`fidryn.evaluation-report/v0.1`. The six-kind legal result is nested as
`outcomeDocument`. That nested object is:

| Field | Meaning |
| --- | --- |
| `schema` | `fidryn.outcome/v0.1`. |
| `module` | Compiled module identity, `Name@version`. |
| `sourceSnapshot` | Content identity of the compiled source snapshot, not the case's `module` string. |
| `query` | Query name you asked. |
| `asOf.validTime` | `--valid-at`. |
| `asOf.recordTime` | `--known-at`. |
| `modelBoundary.outsideScope` | Stable unique union of module `outside_scope` and case `outsideScope`. |
| `modelBoundary.admissibleCompletions` | The case's declared completion space when nonempty. |
| `outcome` | One of the six kinds below. |

`modelBoundary` is part of the answer. An omitted interpretation cannot
silently shrink the model after the fact: the envelope repeats the space
you declared. Read it before you treat a `determinate` value as "the"
result.

Unknown queries, exhausted fuel, and unsupported operations are engine
failures (exit 1). They are never rewritten as `inconsistent`.

## Evaluation report envelope

Qualifications that the outcome schema cannot carry live on
`fidryn.evaluation-report/v0.1`. `fidryn_trace::render_report` emits
that object. `outcomeDocument` is the `fidryn.outcome/v0.1` projection
above; that schema is unchanged.

| Field | Meaning |
| --- | --- |
| `schema` | `fidryn.evaluation-report/v0.1`. |
| `executionMode` | `operative` or `scenario`. Operative `evaluate` does not apply `case.assumptions`. |
| `assumptions` | Overlay hypotheses copied onto a scenario report. Empty for operative runs. |
| `sourceTrust` | How the compiled source was authenticated. In-memory / mill pasted compile is `unauthenticated`. A path compile may be `fixture` or `byteVerified`. `fixture` is not byte-verified. |
| `verificationMethod` | `none` (no covering verification; a claims-digest binding is not covering), `structural` (shape only), or `finiteReplay` (each declared branch re-evaluated). Only `finiteReplay` is covering. |
| `coverage` | Optional `CoverageWitness`. Shape completeness is not covering meaning. |
| `outcomeDocument` | The `fidryn.outcome/v0.1` object from the previous section. |

Do not drop trust or coverage to keep an old outcome printer happy.
If a qualification cannot fit `fidryn.outcome/v0.1`, it belongs here.
A scenario report keeps `executionMode: scenario` and a nonempty
`assumptions` array on this envelope; it must not be reduced to an
outcome-only `fidryn.outcome/v0.1` export.

## Outcome kinds

`outcome` is tagged by `kind`. Every kind includes `trace`, a 32-character
hex `TraceId`. `fidryn explain TRACE_ID` loads `TRACE_ID.json` when that
file exists; otherwise it prints the hashed id. JSON traces always
include a `nodes` array.

Values inside an outcome are tagged runtime values
(`{"kind":"bool","data":true}`, `{"kind":"entity","data":"Bob"}`,
`{"kind":"unit"}`). They are not the looser case-JSON literals. A
proposition is `{"kind":"prop",...}`; it is not a boolean.

### Determinate

```json
{
  "kind": "determinate",
  "value": { "kind": "entity", "data": "Bob" },
  "trace": "0123456789abcdef0123456789abcdef",
  "convergenceCertificate": null,
  "ignoredOpenIssues": []
}
```

**When.** The query has one answer that does not depend on any still-open
admissible completion, or a competent authority has already determined
the issue in this context. `Determinate` additionally requires a
**nonempty** exhaustive completion set. An empty declared domain is
`inconsistent` or `suspended`, never a vacuous `determinate`.

**Fields.**

- `value` (required). The answer. Compared with full value equality, not
  with display labels. `Bob` the entity is not `"Bob"` the string.
- `trace`. Provenance id for this evaluation.
- `convergenceCertificate`. Optional 32-hex completion-proof id. Present
  when a covering certificate was bound. A raw id cannot be deserialized
  back into a live certificate; the kernel has to verify it.
- `ignoredOpenIssues`. Open requests that were set aside. Nonempty only
  together with a **covering** certificate. A claims-digest binder is
  not enough; the constructor refuses `determinate` with ignored issues
  and a non-covering certificate.

**What it is not.** Listing one alternative first is not determination.
Hashing the open issues is not covering. Shape-complete `examined ==
total` with empty or fabricated `branches` is not covering.

### Contingent

```json
{
  "kind": "contingent",
  "alternatives": {
    "i:SuccessorEligibility=I1": { "kind": "entity", "data": "Alice" },
    "i:SuccessorEligibility=I2": { "kind": "entity", "data": "Bob" }
  },
  "pivots": [],
  "trace": "0123456789abcdef0123456789abcdef"
}
```

**When.** At least two still-admissible total answers disagree, or a
determinate branch is mixed with a nested contingent. Explore of the
trust fixture with two certificates and **no** recorded interpretation
is the usual example: I1 yields Alice, I2 yields Bob.

**Fields.**

- `alternatives` (required). Map from assignment identity (the full
  binding, not a list index) to the answer in that world.
- `pivots` (required). Open requests that split the worlds
  (`needInterpretation`, `needChoice`, `needEvidence`, and so on).
- `trace`.

Aggregation compares full `Value`s. Two worlds that print the same label
but differ in kind stay contingent. A determinate world plus a
contingent world is contingent, not determinate.

### Suspended

```json
{
  "kind": "suspended",
  "requests": [
    {
      "kind": "needInterpretation",
      "source": "Instrument.clause(\"4.4\")",
      "family": "SuccessorEligibility"
    }
  ],
  "trace": "0123456789abcdef0123456789abcdef"
}
```

**When.** The evaluator needs a legal operation that the case does not
discharge, and there is no covering certificate that would let those
open branches be ignored. `run` never invents the missing completion.
Open branches without a covering certificate stay `suspended`.

Typical causes: missing evidence whose `observedAt` is at or before
known-at; a judgment with no matching determination; a choice with no
recorded decision; an interpretation family with no recorded
alternative; a unique-occupant query with no occupancy; a closed-world
status asked without a closure record; an incomplete explore that ran
out of budget.

**Fields.**

- `requests` (required). The outstanding `OpenRequest` set.
- `trace`.

Request kinds you will see:

| `kind` | Meaning |
| --- | --- |
| `needEvidence` | `issue` plus `schema`. Observe matches that schema and known-at. |
| `needJudgment` | `issue` (a proposition term) plus `protocol`. Determine matches protocol and issue. |
| `needChoice` | `protocol` plus `options`. |
| `needInterpretation` | `source` plus `family`. |
| `needApplicableLaw` | `issue` plus `candidates`. A unique candidate may resume; a tie suspends. |
| `needConflict` | `graph` plus `doctrines`. A unique applicable doctrine may resume; list order is never a tie-break. |
| `needCustom` | `effect` plus `payload` (for example an incomplete explore). |

### NormConflict

```json
{
  "kind": "normConflict",
  "doctrines": [ { "name": "LaterInTime" } ],
  "trace": "0123456789abcdef0123456789abcdef"
}
```

**When.** Staged effects disagree and the conflict oracle cannot pick
exactly one applicable doctrine. Zero applicable doctrines, or two or
more incompatible ones, stay unresolved. Silent preference for the
first name in a list is forbidden.

**Fields.**

- `doctrines` (required). The conflict doctrines involved (`name`,
  `guard`, `defeats`, `as_to`, `reason`, and identity metadata).
- `trace`.

If a unique doctrine applies, evaluation resumes instead of returning
this kind. Recorded resolutions on the case, when present, also resume.

### OutsideCompetence

```json
{
  "kind": "outsideCompetence",
  "request": {
    "kind": "needInterpretation",
    "source": "Instrument.clause(\"4.4\")",
    "family": "SuccessorEligibility"
  },
  "reason": "recorded interpretation is not in the admissible set",
  "trace": "0123456789abcdef0123456789abcdef"
}
```

**When.** The case asks the interpreter to do something outside the
declared model or the module's competence. A recorded interpretation or
choice that is not in `admissibleCompletions` is the usual case. An
explored doctrine or source outside the candidate set is another.

**Fields.**

- `request` (required). The operation that could not be discharged.
- `reason` (required). Why it is outside competence, not merely open.
- `trace`.

`established: false` from a competent protocol is a negative
determination, not this kind.

### Inconsistent

```json
{
  "kind": "inconsistent",
  "core": ["empty completion set"],
  "trace": "0123456789abcdef0123456789abcdef"
}
```

**When.** The admitted model cannot be satisfied. Examples: a named
interpretation family whose declared list is empty; every explored
branch unsatisfiable; two established occupants of an office that must
be unique.

**Fields.**

- `core` (required). Human-readable inconsistency lines. Explore of an
  empty declared domain uses `empty completion set`.
- `trace`.

Engine errors are not this kind. `unknown query q` exits without an
outcome document.

## Rules to internalize

### Determinate needs nonempty exhaustive agreement or a competent determination

A legal computation may return one determinate result only when that
result is invariant across every still-admissible resolution, or when a
competent authority has already made a determination that is operative
in the relevant context. The completion set those worlds are drawn from
must be nonempty and exhaustive. Vacuity (`Family: []`) is not
agreement. `run` does not choose a completion to manufacture agreement.

### Open branches without a covering certificate stay Suspended

An open `needInterpretation`, `needEvidence`, or similar request is not
dropped because the rest of the query looks settled. Ignoring open
issues on a `determinate` result requires `ignoredOpenIssues` **and** a
covering `convergenceCertificate`. Without that pair, the outcome is
`suspended`.

### A claims digest is not covering proof

`CheckedCertificate::verified` binds a proof id to a hash of program,
snapshot, case, query, clocks, constraints, and answer. Matching that
digest does not discharge open constraints and does not set
`covering: true`.

Covering requires `verified_covering` plus a complete
`CoverageWitness`: `incomplete == false`, `examined == total`,
`examined > 0`, matching answer, and a `branches` list whose meaning
checks. Shape completeness is not enough.
`fidryn-kernel::accept_covering_eval` re-evaluates each
`CoverageWitness.branches` entry (`BranchClaim { bindings, answer }`)
against the claimed program and query. It rejects fabricated
evaluations, duplicate worlds, incomplete or mismatched counts, and
digest-only certificates. The kernel does not search and does not
invent worlds. `accept_covering` (shape only) is not that check.

### explore versus run

`run` evaluates the given case with a `CaseFile` handler. Missing
completions suspend. It never fills I1 or I2 for you.

`explore` searches the declared `admissibleCompletions` (and any
`--bounds` merge). Recorded interpretations and decisions constrain the
search; they are not overwritten. Divergent total answers become
`contingent`. Convergent nonempty answers can become `determinate`. An
empty named domain becomes `inconsistent`.

The two commands need not return the same category. With two
certificates and an open `SuccessorEligibility` domain,
`run ... --query acting_trustee` suspends (`needInterpretation`) while
`explore` is `contingent` between Alice and Bob. With
[`court-selects-i2.json`](../examples/trust/cases/court-selects-i2.json),
both answer Bob, because I2 is already on the case and inside the
declared domain. Treat agreement as a fact about that record, not as a
reason to skip reading `kind`.

### modelBoundary is on the envelope

Do not read `outcome.value` and ignore `modelBoundary`.
`outsideScope` is the union of module and case exclusions; a case that
lists only `tax` does not erase the module's `creditor_priority`.
`admissibleCompletions` on the envelope is the space the result is
relative to. If you omitted a family, the result is silent about that
family; it is not a proof that the family does not matter.

### Prop is not Bool

`Prop` is a first-class sort. There is no conversion from a proposition
term to `bool`. `Incapacitated(Bryan, Administer(BRT))` is not `true`.
Guards use `operative`, `assumed`, `determined`, or `necessarily`. A
tagged `{ "kind": "bool", "data": false }` is boolean; a tagged
`{ "kind": "prop", "data": { "predicate": "Incapacitated", "arguments": [] } }`
is not. Case JSON that needs both field names `kind` and `data` for a
user record must use `{ "kind": "record", "data": { ... } }`; see
[Cases and time](cases-and-time.md).

## Reading a result

1. Confirm the CLI/mill report `schema` is
   `fidryn.evaluation-report/v0.1`, then open nested `outcomeDocument`
   (`schema` `fidryn.outcome/v0.1`). Read `executionMode`,
   `sourceTrust`, and `assumptions` on the report before treating the
   answer as operative.
2. Read `modelBoundary` and `asOf` before `outcome.kind`.
3. Branch on `kind`. Only `determinate` has `value` as *the* answer.
4. If `kind` is `determinate` and `ignoredOpenIssues` is nonempty, demand
   a covering certificate, not a hex string you minted yourself.
5. If `kind` is `suspended` or `contingent`, the work is in `requests` /
   `pivots` and `alternatives`, not in guessing a default.
6. Do not promote a `run` suspension into the `explore` contingent
   answer, or the reverse, without a new evaluation.

The modules under `examples/` and the records under `tests/` are
fixtures for this interpreter. They are not legal advice.
