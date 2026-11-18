# Releases and packages

The interpreter's workspace version is the release version. The project is under the MIT License; [Licensing](../LICENSING.md) describes it and the third-party notices. Source instruments,
prelude modules and interchange schemas retain their own version histories.
Release 0.3 therefore still loads the repository's 0.1.0 instrument
fixtures and emits checked evaluation reports using schema v0.2.

## Prepare a release

1. Increase `[workspace.package].version` and the internal workspace dependency
   versions in `Cargo.toml`, then update `Cargo.lock` with
   `cargo update --workspace --offline`, which changes the entries of the
   workspace's own packages and no dependency.
2. Add a dated `## [VERSION] - YYYY-MM-DD` entry to `CHANGELOG.md` with classified
   release notes. Document breaking behavior and remaining limits. The notes
   gathered under `## [Unreleased]` move below the new heading, and the
   `## [Unreleased]` heading stays above it, empty.
3. Regenerate the files that record the version, with the build of the new
   version, in this order:
   1. `cargo xtask uscode-sample --update` writes the sample's generated
      workspace, whose manifest records the version as `toolchain` and whose
      lock records it as `generator.version`, and then the `toolchain` of
      `samples/uscode/curated/workspace.json` and its pin, the `lock_digest`
      of the lock that changed.
   2. The demo workspace under `conformance/fixtures/workspace/` records the
      version in the same two places. The test
      `the_committed_demo_workspace_is_what_the_writer_writes` of
      `fidryn-driver` (`cargo test -p fidryn-driver --lib`) fails while it
      differs and names the directory that holds what the writer writes;
      copy the files that differ from there.
   3. The change set under `conformance/fixtures/change-set/` records the
      `lock_digest` of the two workspaces it compares, whose locks record the
      version. The test `the_committed_change_set_is_what_the_comparison_writes`
      of `fidryn-driver` fails while it differs and names the directory that
      holds what the comparison writes; copy `change-set.json` from there.
   4. The examples of a manifest and a lock in
      [Law modules](law-modules.md) show the version; edit them.
   5. When the major or the minor version changes, the documents that name
      the release series, such as `Fidryn 0.3` in `README.md`, the guides
      under `docs/` and `grammar.ebnf`, name the new one. The changelog's
      entries of earlier releases keep theirs.
   6. Last, `cargo xtask site` renders the site from the documents; the
      footer of every page shows the version.

   In the generated files, only the lines that carry the version differ, and
   the digests of the locks that changed: the pin of the curated manifest and
   the two of the change set. No module file, coverage ledger, profile or text
   record changes.
4. Run `cargo xtask release-check` and `cargo xtask ci --fmt`.
5. Build a native package with `cargo xtask package` from a clean checkout,
   and check the dependencies with
   `cargo deny --offline check licenses bans sources`.
6. Promote through the same-repository `dev` to `main` PR after every required
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

`deny.toml` allows a fixed list of licenses for dependencies. Workspace crates declare `MIT`, which is on that list, so a release needs no license entries of its own.
