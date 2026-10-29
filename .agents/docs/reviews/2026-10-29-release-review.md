# Release implementation review — resolved state

The final read-only review found no remaining material defect in the current 0.2.0 release path. This receipt covers the precise file bytes listed below; it is not a claim that a GitHub release has been published or that new Windows/macOS packages have passed remote CI.

## Existing candidate failure and LF fix

Both push CI 40966050499 and PR CI 40966292590 at 3c58d74415db6a0e300e900b0a707485f65fdfd2 completed with five successful jobs and only Windows Rust tests failing. Both failures were `fidryn-check::tests::matching_package_links_always_true_so_query_type_checks` (69 passed, 1 failed). The Windows checkout changed authenticated package source LF bytes to CRLF while the nested package lock retained its committed LF digest.

The root-authorized .gitattributes fix sets `* text=auto eol=lf` and `*.woff2 binary`. An actual `git -c core.autocrlf=true checkout-index` into a cache temporary directory preserved both real package sources and both locks byte for byte, with original BLAKE3 digests. No authentication or assertion was weakened. A new Windows CI run remains required. Receipts: `/cache/tmp/fidryn-release-ci-diagnosis.log` and `/cache/tmp/fidryn-release-ci-lf-regression.log`.

## Current release path

- Release metadata reads the pinned PR base SHA or push-before SHA, requires a genuine SemVer precedence increase, and checks all workspace package versions, internal dependency paths/versions, lock entries and the exact dated classified changelog entry. Annotated tags are peeled to commits; build metadata alone does not satisfy an increase.
- Each native platform job tests the workspace, builds the actual rustc host target with release/locked/offline settings, creates four flat assets, verifies extracted full file inventory and SHA256/BLAKE3 digests, and exercises extracted CLI version/check/run. Default packaging refuses dirty source; explicit development packaging is labeled dirty. Git inventory excludes ignored local files. Cleanup requires matching ownership markers and refuses unowned collisions/symlinks.
- Publication only runs for a main push and needs all CI jobs, including the complete platform/package matrix and release metadata. It separately waits for the CodeQL push workflow on the exact main SHA. Write permission is limited to publication; Actions read is supplied for workflow polling.
- The inventory gate requires exactly three distinct native platform families and twelve matching archive/manifest/checksum files, correct clean source/version/SHA metadata, canonical single-record checksum sidecars and actual SHA256 archive verification.
- A stale main run exits explicitly without publication. Missing tags are created at the tested SHA without force; existing tags must resolve to that SHA. Draft creation uses an existing verified tag. Portable `gh release view ... --json apiUrl` resolves drafts, avoiding the published-only REST tag lookup. Draft recovery refuses unexpected assets, uploads the expected set, and checks uploaded/nonempty state. Tag and current main are rechecked before publishing.
- An already-published release is preserved after tag SHA, exact expected asset names and uploaded/nonempty state checks. This is a successful no-op, not remote byte revalidation. docs/releases.md explicitly states that it does not download or rehash remote assets.

## Findings resolved during review

1. Package metadata and publication jq now agree on top-level sourceCommit/dirty fields.
2. Ignored local caches no longer enter packages labeled as clean.
3. Publication no longer depends on the locally unsupported databaseId JSON field.
4. Draft lookup no longer uses REST releases/tags before publication; portable apiUrl is used.
5. Missing actual tags are explicitly created before drafting, so pending draft tags do not break recovery.
6. Exact local and remote asset inventory checks reject missing, unexpected or stale assets.
7. Final tag/current-main checks guard ordinary concurrent changes; stale runs skip before publication.
8. Appended checksum records are rejected. The original gate only checked the first line, then sha256sum read every line; an independent probe appended a valid `../outside` checksum and was accepted. The corrected gate compares both sidecars byte for byte with one canonical record. The same adversarial probe now fails before hashing the outside path.
9. Existing published release documentation now describes the limited no-op verification scope accurately.

## Verification performed by this reviewer

- The final `conformance.test_release_workflow` suite passed all 10 tests. These execute the actual workflow inventory shell with independent mutations: absent/extra archives, repeated platform family, wrong version/SHA/dirty metadata, archive tampering, unrelated checksum paths, appended SHA256 records, extra BLAKE3 records, and host/stem mismatch.
- The appended outside-path probe was independently rerun against the corrected shell and refused with exit 1. Historical accepted probe: `/cache/tmp/fidryn-release-final-checksum-edge.log`. Fixed probe: `/cache/tmp/fidryn-release-final-checksum-edge-fixed.log`.
- Actionlint passed for the final workflow; `/cache/tmp/fidryn-release-final-actionlint.log` is empty.
- The actual publication shell was executed offline with a stateful gh stub in 10 scenarios: stale initial main, already published, missing remote asset, bad remote state, wrong tag, partial draft recovery, fresh tag/draft, unexpected remote asset, main advancing after drafting, and tag drift after upload. All expected exit/mutation assertions passed. Fresh/draft cases reached publication; stale/no-op cases did not publish; refusals never published. Receipt: `/cache/tmp/fidryn-release-final-publication-flow.log`.
- Python test receipt: `/cache/tmp/fidryn-release-final-inventory-tests.log`.

## Qualification limits

The offline gh stub checks actual shell control flow, not GitHub server behavior. The Python inventory suite deliberately uses independent byte fixtures rather than real native archives; extracted executable qualification belongs to the packager and native platform jobs. This reviewer did not invoke live release/tag mutations, build a native package in this final review, or rerun Windows/macOS. A green publication job can mean either actual publication, an explicitly reported stale-run skip, or preservation of an already-published release; the remote release state must be checked when reporting actual publication.

The native workflow cannot make its final ref checks and release PATCH atomic against an authorized actor concurrently rewriting tags/main. Published-release no-op checks do not revalidate remote bytes. These limits are distinct from the tested package inventory and source SHA checks.

This final task made no repository edits, Git changes, or remote mutations. The only repository edit by this reviewer during the preceding release work was the authorized .gitattributes file.

## Reviewed file identities (SHA256)

- `.gitattributes`: `8c4282513ea9832421bc127d9118a365973ce60c269899e2f6327f3972b9aeb7`
- `.github/workflows/ci.yml`: `41bf57dbd7de4a943717888953c99baef15369a83e5feee324d3afc60e3da717`
- `.github/branch-policy.json`: `8791c49efc68557482d7865bce5040f0baf29518f1e7cfc09448ee3bf7e5e45f`
- `xtask/src/main.rs`: `6f6a371c77ff46396dfb43b678ef5e06d0c26d3c897fefbd375df095d1d08c27`
- `xtask/src/package.rs`: `83f1c3cb360c3e2041b21dc9fc7596e061c37732ad17f17802a611071ebb6935`
- `xtask/src/release.rs`: `e8c409e95627b0fb4c3527c2097f695c7bef6e97e05e719c105cb604cbf8bbf8`
- `xtask/tests/release_cli.rs`: `a15f6628daf3e3164b46abeb843b279bbe669453fb2aba3dafff599a888ec7cf`
- `conformance/test_release_workflow.py`: `eae8b8771ac32f91cc26001699e3f92bf2c609bcb922f3da00eeb3500ddf3985`
- `docs/releases.md`: `529871779a80e07eb5f4ddb9d45929d3f92ad8bbc6c6ffea0627bc1cac52f567`
