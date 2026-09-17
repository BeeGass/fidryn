# Workstream contract (2026-09-17 sections 2–5)

Shared types and acceptance. Crate owners implement against this file.
Do not invent a second meaning of Determinate. Do not weaken
no-false-determinacy. Do not treat a claims digest as covering proof.
Do not treat digest `fixture` as byte-verified.

Environment: PATH `/opt/homebrew/bin:$HOME/.cargo/bin:/usr/bin:/bin`,
DEVELOPER_DIR=/Library/Developer/CommandLineTools. Rust 1.98 / edition 2024.

## Trust profiles

```text
Fixture            synthetic/test input; not source authentication
ByteVerified       expected digest equals hash of artifact bytes
PolicyAccepted     explicit trust policy named in the manifest
Unauthenticated    no digest, or digest without bytes
```

## Certificates

`CheckedCertificate::verified` remains a claims-digest binder.

Ignoring open issues requires `verified_covering` with a
`CoverageWitness { examined, total, incomplete: false, answer }` where
`examined == total`, `examined > 0`, `answer` equals the claimed answer.
`Outcome::determinate` with nonempty `ignored_open_issues` must carry a
covering certificate (`cert.is_covering()`).

A trusted evaluator may establish covering by exhaustive finite search.
A hash of the open issues is not covering.

## Requirements

`seq` and `require` are Core operations (`Term::Apply` ctors).

| Program | Result |
| --- | --- |
| `require true; return 7` | Determinate 7 |
| `require false; return 7` | requirement-failure (`Suspended` NeedCustom require); 7 does not run |
| `require unresolved; return 7` | Suspended with that issue; 7 does not run |

False require is a failed guard for this invocation, not a compile error
and not Determinate false unless a later declared result says so.

## Tax builtin

The only missing-body arithmetic helper is the explicit builtin name
`ordinary_income_tax`. Any other function with no body is
`EngineError::Unsupported`. Name substring `tax` is not a builtin.

## Records vs tagged values

Runtime tags stay the Value serde kinds. An explicit record wrapper is:

```json
{"kind":"record","data":{ ... fields ... }}
```

`{"kind":"bool","data":false}` is a Boolean. A user record that needs
those field names must use `kind: record`.

## Streaming search

`fidryn-solve` yields assignments lazily. The completion cap applies
while generating, not after a full product. Budget exhaustion is
incomplete coverage, never Determinate.

## Duties

Status machine: Attached → Performed | Breached | Unresolved;
Breached → Cured | Discharged; late Perform keeps breach history.
Unauthorized or unresolved transitions commit nothing.

## Authority

Grant: action, scope, context, interval, source. A transition names the
required action; the response must carry a grant covering that action
at the record time. Missing/expired grant suspends or fails closed.
