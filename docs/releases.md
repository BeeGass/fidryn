# Releases and packages

The interpreter's workspace version is the release version. Current project terms
and retained prior/third-party grants are documented in
[Licensing](../LICENSING.md). Source instruments,
prelude modules and interchange schemas retain their own version histories.
The 0.2.0 release therefore continues to load the repository's 0.1.0 instrument
fixtures and emits checked evaluation reports using schema v0.2.

## Prepare a release

1. Increase `[workspace.package].version` and the internal workspace dependency
   versions in `Cargo.toml`, then update `Cargo.lock`.
2. Add a dated `## [VERSION] - YYYY-MM-DD` entry to `CHANGELOG.md` with classified
   release notes. Document breaking behavior and remaining limits.
3. Run `cargo xtask release-check` and `cargo xtask ci --fmt`.
4. Build a native package with `cargo xtask package` from a clean checkout.
5. Promote through the same-repository `dev` to `main` PR after every required
   check passes. `Release metadata` requires a genuine version increase over the
   PR's pinned base, including agreement across workspace packages, internal
   dependencies and the lockfile.

`cargo xtask release-notes` prints only the current version's changelog body.
Existing version tags must resolve to the same commit for an idempotent rerun;
tags are never forced or moved by the release workflow.

## Package contents

`cargo xtask package` builds the native Rust host target with the pinned
toolchain, locked dependencies and release profile. Run `cargo fetch --locked`
first to make the resolved dependency sources and their notices available to the
offline packager. Each platform produces:

- `fidryn-VERSION-HOST.tar.gz`, or `.zip` for Windows.
- A matching `.manifest.json` containing the version, source commit, dirty flag,
  actual host target, binary digests and complete file inventory.
- SHA-256 and BLAKE3 checksum sidecars for the archive.

The archive contains the executable, `LICENSE`, `LICENSING.md`, retained notices
under `LICENSES/`, README, changelog, documentation, schemas, templates and a
self-contained smoke fixture. `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES/`
retain resolved Cargo dependency notices from the locked workspace graph. This
inventory includes a superset of the binary dependencies; it does not infer an
exact binary linkage map. The packager extracts its
own output, checks file hashes and inventory, then runs the extracted executable
with `--version`, `check` and `run` before publishing the four local files.
It refuses dirty sources by default. `--allow-dirty` explicitly produces a
development package labeled `dirty: true`; CI publication rejects such packages.
`--output-dir PATH` selects the output directory. Existing unowned data is refused.

## Native GitHub publication

Linux, macOS and Windows test jobs build and verify packages and upload them as
artifacts in the same CI run. Main's publication job waits for the complete CI
gate and the separate CodeQL run on the exact main SHA. It then refuses unresolved
CodeQL errors or high/critical security findings on main. Its write token is limited
to the publication job; no dedicated App or private key is required.

Publication checks the three platform families, exact archive/manifest/checksum
inventory, clean source/version/SHA metadata and SHA-256 checksums. It creates a
missing version tag at the tested commit, prepares a draft release, uploads all
expected files, rechecks the tag and current main, and then publishes. An existing
published release is preserved after checking its tag commit, expected asset names
and uploaded, nonempty states; this retry path does not download or rehash its
remote asset bytes. A matching draft can be retried;
unexpected assets refuse publication. A superseded main run skips publication,
retaining its tested CI artifacts or prepared draft for inspection.

Releases identify their source and executable bytes. They do not establish legal
approval or close the general-language obligations in the implementation audit.

First-party custom license checks in `deny.toml` bind exact workspace crate
versions and the license-file hash. Version or license-text changes must update
those entries; the third-party license allowlist remains separate.
