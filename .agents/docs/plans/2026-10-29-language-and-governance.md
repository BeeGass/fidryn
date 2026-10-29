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
6. Prepare 0.2.0 with a classified changelog, version/lock consistency checks,
   clean native packages and native-token release publication after main CI and
   exact-commit CodeQL. Include concurrent Time/record changes only after review
   and tests, as explicitly authorized by the user.

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


## 0.2.0 release candidate follow-up

The user explicitly authorized including the concurrent timestamp-comparison and
record-constructor changes after review/testing. The reviewed change passed its
three source-to-CLI regressions and all-target linting, and is committed as
541c4242703eab692fb0b4883db5cf8368bc3d00. It is included in the 0.2.0 changelog.
The workspace, internal dependency versions and lockfile advance to 0.2.0;
instrument/prelude versions and existing interchange schemas remain independent.

The complete canonical local gate passed with 1173 Rust tests including doctests,
12 Python controls, 52 JavaScript tests, all-target/all-feature Clippy, strict
Rustdoc, executable schema probes, site consistency and benchmark compilation
(`/cache/tmp/fidryn-0.2.0-ci.log`). Final checksum-path review then added two
negative controls and required exactly one canonical line in each checksum
sidecar; all14 Python controls, Python lint/format/types and actionlint passed.
Handbook labels were reconciled and the site regenerated; all5 site-output tests,
site consistency and browser site test passed again. These scoped follow-ups
preserve the exact qualification history rather than claiming the earlier full
command covered later documentation/workflow bytes.

Release metadata passed against pinned main base c5c5f4596d2361c78b6de8ac8877f1ce3087acc9.
The packager builds the actual native rustc host, retains clean commit/version/file
identities and both checksum formats, extracts and verifies all packaged files,
and runs the extracted CLI version/check/query smoke test. A dirty development
preview succeeded; a clean committed candidate must still be packaged separately.

Native main protection was reapplied and read back with11 required Actions
contexts and no mandatory-check bypass; dev contribution checks total10. Platform
CI now includes package construction. Publication uses only the native job token,
waits for all CI and exact-main-SHA CodeQL, validates all12 assets across3 platform
families, never moves a version tag and publishes only a complete draft. Existing
published releases retain their assets after tag/name/upload-state checks; that
retry path does not rehash remote asset bytes. A superseded main run skips
publication explicitly. No main promotion, tag creation or published release is
asserted by these local receipts. Remote candidate checks remain required.

Final dependency scan covers267 dependencies and1386 cached advisories with zero
vulnerabilities/warnings. Cargo deny bans/licenses/sources passed with the same
two unused-license-allowance warnings; actionlint and current-tree Gitleaks passed.
The general language/proof/resource obligations in the audit remain open.


## Windows candidate finding and scoped correction

At5e7cd4d, PR CI40969506655 and push CI40969500459 each exposed the same
review-bundle regression on Windows: the exporter rebuilt validated components
as a host PathBuf and emitted backslashes for nested archive paths. Review path
serialization now joins those validated components with `/`; root/parent/UTF-8
and separator validation is retained. The nested-manifest regression asserts all
archive paths, source links and rerun paths, restores its exact layout while
retaining missing inputs, and requires identical invocation/source-bundle
identities after actual Driver rerun. A second regression refuses decoded
manifest references containing platform-dependent separators before host path
interpretation; exact source bytes are retained. Compatible filesystem name/case/
normalization rules remain a documented restoration requirement. All122 CLI tests,
focused CLI Clippy,
formatting and release metadata checks passed after this correction. A fresh
complete remote candidate run is still required; the older failed Windows jobs
are not presented as successful qualification.

The preceding candidate's Ubuntu and macOS package creation, extracted-binary
checks and artifact uploads succeeded in both push and PR runs. PR CodeQL also
passed. Its clean local Linux package was built from5e7cd4d with dirty:false;
that artifact does not qualify the subsequent path correction. The final
committed candidate must be repackaged and checked remotely.


## Final local gate and CI cache correction

The committed review-path candidate 083a63c passed the complete canonical local
`cargo xtask ci --fmt`: 1174 Rust tests, 14 Python controls, 52 JavaScript tests,
all required lint/type/doc/schema/site checks and benchmark compilation. A clean
Linux package from that exact commit passed extraction, file hashes and CLI smoke
checks (`/cache/tmp/fidryn-0.2.0-final-ci.log` and clean-package log).

PR CI 40970378283 then passed Linux and macOS workspace tests but correctly
refused packaging into a cached `target/packages` directory restored without its
hidden ownership marker. CI now creates package output under a fresh runner temp
directory scoped by run id and attempt, with matching artifact-upload globs.
The refusal to overwrite unowned data is preserved. Actionlint and all 14 Python
controls passed after this workflow correction. New exact-candidate remote
qualification and a clean package remain required; previous failed job statuses
are not upgraded to success.
