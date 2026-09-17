# Cases and time

This guide explains how to supply a case record to Fidryn. Fidryn is a
research interpreter for legal instruments. It is not legal advice, not an
operative filing system, and not a statement of any jurisdiction's law.
The JSON you write is a fixture for evaluation, not a court record.

A case is a `fidryn.case-record/v0.1` document. The interchange schema is
[`schemas/case-record-v0.1.json`](../schemas/case-record-v0.1.json). Wire
names are camelCase except where a nested type keeps its Rust field names
(determinations use `recorded_at`).

Required properties are `schema` and `admissibleCompletions`. Everything
else may be omitted and defaults to empty. The published schema lists
`schema`, `module`, `facts`, `evidence`, `determinations`,
`interpretations`, `decisions`, `closures`, `outsideScope`, and
`admissibleCompletions`. The interpreter also reads an `events` array
when it is present.

## Supplying a case

Pass a JSON file to `fidryn run` or `fidryn explore`:

```
fidryn run PATH --query NAME --case RECORD.json --valid-at TIME --known-at TIME
fidryn explore PATH --query NAME --case RECORD.json --bounds BOUNDS.json \
    --valid-at TIME --known-at TIME
```

`TIME` is ISO 8601 / RFC 3339 with a `Z` suffix or a numeric offset
(`2033-01-01T00:00:00Z`, `2033-01-01T00:00:00+00:00`). Equal instants
canonicalize to UTC. `run` never chooses a missing completion. `--arg
KEY=VALUE` writes `case.facts["KEY"]`. `explore --bounds` merges extra
declared domains into `admissibleCompletions`.

See [CLI](../README.md#cli), the [language grammar](../grammar.ebnf), and
[Outcomes](outcomes.md).

## Record shape

| Field | Meaning |
| --- | --- |
| `schema` | Must be the string `fidryn.case-record/v0.1`. |
| `module` | Optional module identity (`Name@version`). The outcome envelope uses the compiled module, not this string, as `sourceSnapshot`. |
| `facts` | Named values already treated as given. Bare JSON booleans, integers, and strings are accepted. |
| `evidence` | Observed records. Each item has `schema`, `value`, and `observedAt`. |
| `determinations` | Competent judgments already on file. Nested fields are `issue`, `protocol`, `established`, `decider`, and optional `recorded_at`. |
| `interpretations` | Selected alternative per interpretation family (`"SuccessorEligibility": "I2"`). |
| `decisions` | Selected option per choice protocol. |
| `closures` | Closed-world flags (`domain`, `closed`). Absence is not a determinate negative unless the domain is closed. |
| `admissibleCompletions` | The declared finite model. This is the model boundary, not a hint. |
| `outsideScope` | Named exclusions. The outcome envelope unions these with the module's `outside_scope`. |
| `events` | Optional ledger rows (`kind`, `validTime`, `recordTime`, `payload`). |

`admissibleCompletions` has three maps:

```json
"admissibleCompletions": {
  "interpretations": {
    "SuccessorEligibility": ["I1", "I2"]
  },
  "evidence": {
    "SecondConcurringCertificate": {
      "responses": ["absent", "present_prospective"],
      "effectOnValidTime": "unchanged_if_after_query_valid_time"
    }
  },
  "choices": {
    "DistributionProtocol": ["hold", "distribute"]
  }
}
```

`interpretations` maps a family name to the alternatives the instrument
still admits. `evidence` maps a schema name to a finite response list
and an optional `effectOnValidTime`. `choices` maps a protocol name to
the options a decision-maker may still pick.

That object is the model. An omitted family is not silently filled in
later. An empty declared list for a named family is not a vacuous
determinate world: explore treats it as an empty completion set
(`inconsistent`), even if `interpretations` already records a selection.
A recorded alternative that is not in the declared list is outside
competence, not a new world.

## Valid time and record time

Fidryn is bitemporal. Every evaluation has two clocks.

`--valid-at` is valid time: the instant on the legal timeline you are
asking about (when occupancy, a deadline, or a status is supposed to
hold). `--known-at` is record time: the instant of the file you are
willing to treat as known. Occupancy and similar ledgers must contain
the valid instant and must have been recorded no later than record time.

These clocks are independent. You can ask what the office was on
2026-08-23 using only records known on that day, or you can ask the same
valid-time question using a later known-at that includes subsequent
filings.

The outcome envelope repeats the pair as `asOf.validTime` and
`asOf.recordTime`.

## Evidence after known-at does not count

Each evidence item carries `observedAt`. Observe matches schema, the
issue's subject when the value designates one, and `observedAt <=
--known-at`. A certificate dated after known-at is invisible to that
query. It does not become a determinate negative; the query still
suspends if the remaining known record is incomplete.

The same cutoff applies to duty and institutional events: `recordTime`
after known-at is not admitted.

Determinations match **protocol and issue**. `established: false` is a
negative determination from a competent authority, not
`outsideCompetence`. A determination for Alice does not discharge a
judgment about Bob.

## Completions are the model boundary

`run` evaluates the case you gave. If an interpretation, choice, or
judgment is required and missing, the outcome is `suspended` with a
`needInterpretation`, `needChoice`, or `needJudgment` request. `run`
does not pick I1 because it is listed first.

`explore` searches the **declared** completions. Recorded selections
constrain that search: a case that already records
`SuccessorEligibility: I2` is not re-opened onto I1. Declared evidence
responses such as `absent` versus `present_prospective` are hypothetical
worlds for explore, not facts invented by `run`.

Three different empty-looking objects are not the same:

- `"admissibleCompletions": {}` declares no open families. A query that
  does not need an interpretation can still be determinate.
- `"interpretations": {}` on the case means no alternative has been
  selected yet. If the query needs one, `run` suspends.
- `"admissibleCompletions": { "interpretations": { "Family": [] } }`
  declares a family whose admissible set is empty. That is not a
  determinate answer by vacuity. Explore reports `inconsistent` with
  core `empty completion set`. A recorded selection cannot reopen that
  empty domain.

Interpretations you record must be members of the declared domain for
that family. `I2` is lawful only when `I2` appears in
`admissibleCompletions.interpretations.SuccessorEligibility`. An
off-list selection is `outsideCompetence` (`recorded interpretation is
not in the admissible set` / `inadmissible interpretation`). The same
rule applies to `decisions` versus `choices`.

## Money and structured values

Case JSON accepts bare literals for booleans, integers, and strings.
Decimals and other runtime values must be tagged so they do not alias
those literals.

```json
{ "kind": "decimal", "data": "1.25" }
```

is money-as-decimal. The bare string `"1.25"` is a string. A JSON
number that is not an integer may decode as decimal, but do not rely on
that for amounts you care about: write the tagged form.

```json
{ "kind": "bool", "data": false }
```

is the boolean false.

```json
{ "kind": "record", "data": { "kind": "bool", "data": false } }
```

is a user record whose fields happen to be named `kind` and `data`.
Without the `record` wrapper, `{"kind":"bool","data":false}` is always
boolean, so you cannot store a complaint-shaped object that uses those
field names as a tagged bool. Unknown `kind` tags (for example
`{"kind":"Complaint","id":"1"}`) decode as a map, not as a bool.

Other tagged forms you may need:

```json
{ "kind": "entity", "data": "Alice" }
{ "kind": "instant", "data": "2026-08-23T12:00:00Z" }
{ "kind": "prop", "data": { "predicate": "Incapacitated", "arguments": [] } }
```

A bare `"Alice"` is a string, not an entity. A proposition is never a
boolean; see [Outcomes](outcomes.md).

## Authority grants and duty events

Institutional events do not commit performance just because they appear
in JSON. Events with `kind` `duty`, `authority`, or `institutional` are
admitted only when an `AuthorityGrant` covers the action at
`recordTime`. Assumptions are explicit: `kind` `assumption` skips that
gate.

Grants may be evidence:

```json
{
  "schema": "AuthorityGrant",
  "value": { "action": "perform" },
  "observedAt": "2026-01-01T00:00:00Z"
}
```

or a fact named `authority_grants`. A `duty` event whose payload is
`Performed` / `action: "perform"` without a covering grant is dropped
from the duty machine. It does not turn `duty_status` into Performed.

`kind` `evidence` rows are copied onto the record ledger at
`recordTime`. Other documented kinds include `correction` and
`retraction`. Events are append-only in the interpreter; they do not
rewrite earlier rows.

A typical duty event:

```json
{
  "kind": "duty",
  "validTime": { "start": "NegInf", "end": "PosInf" },
  "recordTime": "2026-01-01T00:00:00Z",
  "payload": {
    "kind": "ctor",
    "data": { "name": "Performed", "fields": {} }
  }
}
```

`validTime` is an interval (`start` / `end` are `NegInf`, `PosInf`,
`Inclusive` of an instant, or `Exclusive` of an instant). `recordTime`
is a single RFC 3339 instant.

## Annotated example: court selects I2

The fixture
[`examples/trust/cases/court-selects-i2.json`](../examples/trust/cases/court-selects-i2.json)
is a complete case for `Examples.BryanRevocableTrust`. The module
declares interpretation family `SuccessorEligibility` with alternatives
I1 (Alice and Bob both eligible) and I2 (only Bob eligible). Two
physician certificates and an occupancy record are on file. A court (or
other controlling selector) has already chosen I2.

```json
{
  "schema": "fidryn.case-record/v0.1",
  "module": "Examples.BryanRevocableTrust@0.1.0",
  "facts": {
    "alice_accepted": true,
    "bob_accepted": true
  },
  "evidence": [
    {
      "schema": "OccupancyRecord",
      "value": "Bryan",
      "observedAt": "2026-08-23T12:00:00Z"
    },
    {
      "schema": "PhysicianCertificate",
      "value": "certificate-1",
      "observedAt": "2026-08-23T12:00:00Z"
    },
    {
      "schema": "PhysicianCertificate",
      "value": "certificate-2",
      "observedAt": "2026-08-23T12:00:00Z"
    }
  ],
  "interpretations": {
    "SuccessorEligibility": "I2"
  },
  "admissibleCompletions": {
    "interpretations": {
      "SuccessorEligibility": ["I1", "I2"]
    }
  }
}
```

What each part is doing:

- `schema` identifies the interchange format. Reject anything else.
- `module` names the fixture this record is meant to accompany. Compile
  `examples/trust/bryan-revocable-trust.fr`; do not treat this string as
  a source-snapshot digest.
- `facts.alice_accepted` and `facts.bob_accepted` record that both
  nominees have accepted office. They are booleans, not propositions.
- `OccupancyRecord` with value `Bryan` is who occupies `TrusteeOf(BRT)`
  at the observation instant. Unique-occupant queries need this (or an
  equivalent ledger row). Two occupants for one office are
  `inconsistent`.
- Two `PhysicianCertificate` rows, both observed at
  `2026-08-23T12:00:00Z`, are the concurring record the instrument's
  incapacity judgment expects. If `--known-at` is earlier than that
  instant, neither certificate counts.
- `interpretations.SuccessorEligibility` is `I2`, which is a member of
  the declared domain `["I1", "I2"]`. That selection is already on the
  case, so `run` does not suspend for `needInterpretation`, and
  `explore` does not also try I1.
- `admissibleCompletions` is nonempty. The still-admissible worlds are
  exactly those two named alternatives, of which one has been chosen.
  Contrast
  [`two-certificates-open-eligibility.json`](../examples/trust/cases/two-certificates-open-eligibility.json),
  which has the same certificates and domain but no recorded
  interpretation: `run` stays suspended, `explore` is contingent between
  Alice and Bob.

Against `acting_trustee` with `--valid-at` / `--known-at` at or after
the observation times, both `run` and `explore` answer Bob. That
agreement is not a general guarantee; see [Outcomes](outcomes.md).

## Checklist

1. Set `"schema": "fidryn.case-record/v0.1"`.
2. Declare every still-open family, evidence schema, and choice protocol
   under `admissibleCompletions`. Leave a family off the object only if
   it is not part of this model.
3. Put recorded interpretations and decisions in those declared lists.
4. Timestamp evidence with `observedAt` and choose `--known-at` so that
   later records do not leak into an earlier file.
5. Tag decimals. Do not store amounts as bare strings.
6. Wrap user objects that need fields named `kind` and `data` as
   `{ "kind": "record", "data": { ... } }`.
7. Do not expect a `duty` event to perform an obligation unless an
   `AuthorityGrant` covers the action, or the event is an explicit
   `assumption`.
8. Treat `outsideScope` as part of the answer. The envelope will repeat
   it on `modelBoundary`.
