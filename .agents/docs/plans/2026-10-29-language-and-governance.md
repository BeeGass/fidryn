# Fidryn implementation plan

Authority: user-approved integrated plan, 2026-10-29.

## Complete destination

Preserve original work on dev; restrict main to owner-authorized, passing
same-repository dev PR promotion through native GitHub rulesets and required
Actions checks. Approved contributors
may directly push dev; other contributions need independent review. BeeGass
id 44324535 has the explicit owner exception to review, never to mandatory CI.

Build structured/resolved syntax and HIR, uniform sequencing and continuation
frames, typed input/evidence/authority admission, grounded truth maintenance,
consistent bitemporal institutional state, one validated completion model,
sound verification and invocation-bound kernel qualification, resource safety,
authenticated dependency closure, complete reports/traces/diffs, typed document
generation, professional editor/input/review workflows, and all existing language
capability obligations. Full details and counterexamples are in
../reviews/2026-10-29-language-audit.md and human contracts in docs/.

## Ordered execution

1. Recover local main commits to dev without rewriting remote main.
2. Implement/review/test CI and native ruleset reconciliation. Keep owner review
   exceptions separate from mandatory main checks. Validate dev-to-main PR
   identity in the required Main source policy workflow; document GitHub checks
   being commit-scoped. No dedicated App or private key provisioning is required.
   User explicitly chose native GitHub capabilities over the earlier controller.
3. Add source-path counterexamples and correct frontend, runtime/admission,
   worklist/state and proof boundaries in dependency order.
4. Integrate artifact-bound source trust, checked v0.2 invocations, actual trace
   artifacts, semantic diffs, typed rendering, diagnostics/input/editor APIs,
   source-linked prose and portable attorney-review bundles.
5. Reconcile status/obligation docs and generated site; run complete local gate,
   remote CI matrix/security/CodeQL, policy checks and promotion refusal tests.

## Acceptance laws

Preservation across parse/lower/fmt/replay; type preservation; identity isolation;
permutation invariance except declared order; grounded justification; temporal
consistency; shared model admission; proof agreement; qualification preservation;
bounded failure and atomic transactions. Existing green tests alone close none
of these general obligations.

## Completion accounting

Keep implementation, focused checks, full qualification, external provisioning,
professional/legal assessment and promotion receipts separate. Do not reduce the
capability destination or mark missing evidence complete. Commits remain focused;
reports identify the exact tree/executable/configuration they tested.

## Current verified governance receipts

Original local main commits f918bc4 and eb072a2 are preserved on remote dev at
eb072a296b987e3f350b0c4348c6755d95540e4b; remote main remains c5c5f45. Local
main matches remote main and recovery/accidental-main-20261029 retains the old tip.
Native rulesets applied and returned active: Protect main 23613847, Main independent
review 26163390, Protect dev 23613849, Dev contributions 26163393. Main mandatory
checks have no bypass actors; BeeGass user 44324535 bypasses only the independent
review ruleset for PRs. Check provider GitHub Actions 15368 verified against main.
New checks must still run successfully on the final candidate before promotion.

## Local qualification receipt

The final production source passed `cargo xtask ci --fmt` with locked offline
Rust dependencies and the locked Python environment:1139 Rust tests including
doctests, workspace all-target/all-feature Clippy with warnings denied, strict
Rustdoc, four executable schema probes and their four negative-control unit
tests, Python lint/format/types, required browser tests, generated-site consistency
and benchmark compilation. Human handbook reconciliation afterwards was checked
by the site tests and `cargo xtask site --check`; generated pages were updated.

Independent security checks passed actionlint1.7.12, Gitleaks8.30.1 on all patch
history including merges/all refs and current files, cargo audit (259 dependencies,
1386 cached advisories; database dated2026-10-29, zero vulnerabilities/warnings),
and cargo deny bans/licenses/sources (two unused-license-allowance warnings).
The workflow now also scans merge patches/all fetched refs and the current tree.

Source commits:d577cfe(language/runtime/proof),60ce599(artifacts/reports/rendering/
review),93d40dd(native governance/qualification). Each is GPG signed. Full remote
Linux/macOS/Windows, Security and CodeQL checks remain required on the final PR
candidate. These local receipts do not assert remote execution or closure of the
open general-language obligations in the tracked review ledger.
