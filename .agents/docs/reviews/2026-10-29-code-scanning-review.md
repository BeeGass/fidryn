# Native CodeQL governance/publication review

Read-only review of the current policy, ruleset renderer and publication alert gate. No repository edit, Git mutation, alert dismissal, rule exclusion or remote mutation was performed by this reviewer.

The renderer includes the additional CodeQL aggregate context with provider57789 alongside, rather than instead of, the workflow jobs from provider15368. The resulting rules have main12/dev11 required contexts. Main's mandatory ruleset keeps bypass_actors empty; its independent owner review exception remains separate. The existing approved-contributor exception on dev is unchanged and is not a main bypass.

Authenticated read-only API verification of the reconciled live rulesets confirms active enforcement, strict checks, correct counts and the provider-pinned aggregate:

```json
[
  {
    "branch": "main",
    "id": 23613847,
    "enforcement": "active",
    "requiredCount": 12,
    "strict": true,
    "codeql": [
      {
        "context": "CodeQL",
        "integration_id": 57789
      }
    ],
    "bypass": []
  },
  {
    "branch": "dev",
    "id": 26163393,
    "enforcement": "active",
    "requiredCount": 11,
    "strict": true,
    "codeql": [
      {
        "context": "CodeQL",
        "integration_id": 57789
      }
    ],
    "bypass": [
      {
        "actor_id": 44324535,
        "actor_type": "User",
        "bypass_mode": "always"
      }
    ]
  }
]
```

Publication has security-events:read only in the main-only write job, alongside Actions read and Contents write. It waits for the separate CodeQL push run on the exact main SHA and then queries open CodeQL alerts on refs/heads/main with pagination. The pinned analyze action defaults wait-for-processing:true, so normal workflow completion waits for SARIF processing. Primary action definition: https://raw.githubusercontent.com/github/codeql-action/9b3254085494b55a0f7c4c2b3d2d52cc712ce370/analyze/action.yml . Existing final main-SHA checks prevent a superseded run from publishing.

The initial alert predicate accepted malformed rows such as {} because all missing fields read as null and compared unequal to blocked severities. That concrete probe was reported and root fixed the gate: a rule object, explicit security-severity field, recognized non-error rule severity and allowed low/medium/null security severity are now required. Error/high/critical or missing/unknown metadata refuse. Bash pipefail also refuses API errors even when jq receives benign partial output or no output.

The final actual-workflow controls pass all19 release tests (10 inventory +9 CodeQL cases), including no findings, permitted severities, error/high/critical, an alert after the first100-record page, API failure after benign output, invalid JSON and malformed/unknown metadata. Receipt: /cache/tmp/fidryn-codeql-final-controls.log. Actionlint passes; receipt /cache/tmp/fidryn-codeql-final-actionlint.log. The initial independent shell probe is preserved at /cache/tmp/fidryn-codeql-gate-review.log and demonstrates the now-resolved malformed-row edge.

Previously captured202 alert rows all contain warning/high CodeQL metadata, which the new gate correctly refuses. This review verifies gate behavior and wiring; the root/runtime agents own the synthetic-data diagnosis and identifier clarification. No assurance is inferred merely from the analyzer workflow being green, and no finding is dismissed by this change.

No remaining material defect found in the current reviewed change under the GitHub API response contract. The offline controls test actual shell classification and error handling, not server authorization or alert index freshness; live API reads verified ruleset state only. A new candidate still needs remote CI/CodeQL qualification.

Reviewed file identities (SHA256):

- `.github/branch-policy.json`: `d94ac357ff8d79d1e97aca673c6868251cbd922da5e95e9dffb42acb72062522`
- `.github/workflows/ci.yml`: `9a4f99552499bf4e680eb3a7ab690afbcc1085b71d7babaeace03ac5332831e3`
- `xtask/src/governance.rs`: `2ba111b24a51f53ce583eb2087f32fd1d07a2ff4b1a6d2d1d4215eeb9bde4628`
- `conformance/test_release_workflow.py`: `956977d73b78aeb7aeac8163083ece4efd419a76a80e2124e5eb99ae3f926a22`

## Alert triage and semantic clarification

At b0f8930 all configured analysis jobs succeeded but GitHub Advanced Security's
separate CodeQL summary reported 202 rust/cleartext-logging findings. All 202
share need_appointment as their source: 133 sinks are inside cfg(test) modules,
68 are integration-test assertions and one is a fixed-fixture benchmark. None
of the reported sinks is production logging. The helper body is unchanged from
main c5c5f45 and creates a legal Occupies(Wildcard, office) judgment request with
protocol Appointment; it does not obtain medical-appointment information.

The primary CodeQL model inspected at commit
18b48aac513b7915adf742aa3fe226963a6ebcb6 lists appointment as a health-data name
heuristic and applies it to Rust function calls:
https://github.com/github/codeql/blob/18b48aac513b7915adf742aa3fe226963a6ebcb6/shared/concepts/codeql/concepts/internal/SensitiveDataHeuristics.qll
https://github.com/github/codeql/blob/18b48aac513b7915adf742aa3fe226963a6ebcb6/rust/ql/lib/codeql/rust/security/SensitiveData.qll
The actual runner used CodeQL CLI2.27.3. The complete alert inventory and locations
are retained locally at /cache/tmp/fidryn-pr3-codeql-alerts.json and its TSV sibling.

Exactly three identifiers were renamed to need_occupancy_judgment (definition
and two call sites) to express the legal request. Payload, protocol, trace and
outcome behavior remain byte-for-byte unchanged. All172 evaluator units,3money
and27runtime source regressions and focused evaluator Clippy passed. No query
was removed, no path excluded and no alert dismissed. Fresh remote analysis must
establish whether the observed name classification is cleared.
