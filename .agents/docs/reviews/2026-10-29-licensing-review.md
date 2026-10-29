# 0.2.0 licensing content and notice implementation audit

Scope: concrete repository text retention, metadata consistency, existing asset notices and native package contents. This is not an opinion about ownership, enforceability, permissions under a particular grant or legal sufficiency. Root owns the licensing decision, documentation, metadata, font records and final release qualification. This reviewer changed only the newly delegated `xtask/src/licenses.rs` implementation during this task; no Git mutation or remote mutation occurred.

## Prior project text

The committed original MIT LICENSE is 1067 bytes with SHA256 `a1211fd38230055cf76a2a282da537b3e0d6f87d5cd0ee2ad454c30eecea16c9`. `LICENSES/prior-project-LICENSE.txt` preserves it byte for byte. The new root LICENSE also ends with the exact original MIT bytes, once, under its retained-prior-text heading. No original notice was removed or rewritten by this change.

A precise scan for repository licensing claims in README/docs/site found no stale whole-project MIT claim. MIT/Apache references in retained prior texts, dependencies and notices describe those materials rather than a new blanket project grant. Current Cargo metadata uses `license-file`, and Python metadata refers to LICENSE; frontend separately owns actual metadata/UV/Cargo-deny admission receipts.

## Existing non-Cargo assets

Four WOFF2 fonts are embedded by `crates/fidryn-cli/src/ui.rs` and served on the static site. The previous `site/fonts/LICENSE.md` contained only family/OFL labels. FreeType SFNT inspection identified the exact shipped metadata (receipt `/cache/tmp/fidryn-license-font-metadata.json`):

- `site/fonts/fraunces.woff2`: Version 1.000;[b76b70a41]; Copyright 2020 The Fraunces Project Authors (github.com/undercasetype/Fraunces); SHA256 `48282a415ec22e31beaf0a0666e6fae0c8cbddcd0b1f6e729f27c3ade8a64e43`.
- `site/fonts/plex-mono-400.woff2`: Version 2.3; Copyright 2017 IBM Corp. All rights reserved.; SHA256 `c36f509c0a8f9f85f29cb44bc8701d8a9e0b14c499e77a884f789ead7093a7ac`.
- `site/fonts/plex-mono-500.woff2`: Version 2.3; Copyright 2017 IBM Corp. All rights reserved.; SHA256 `a76f53ca6612e7b3828eec2311098675b7f9849ae4169a8bcef6302aec02a6c0`.
- `site/fonts/plex-sans.woff2`: Version 3.201; Copyright 2019 IBM Corp. All rights reserved.; SHA256 `056e4e2459f57a0033c8c9c844ff19d6e42ac8602027803d4345823bcc939818`.

Root has retained complete upstream OFL1.1 texts under LICENSES/fonts, added the actual embedded copyright notices, recorded shipped font versions/file hashes, and expanded site/fonts/LICENSE.md with the terms. Different dates in current upstream headings and embedded font metadata are disclosed and both notices preserved. Upstream primary texts inspected: Google Fonts `ofl/fraunces/OFL.txt`, `ofl/ibmplexsans/OFL.txt`, and `ofl/ibmplexmono/OFL.txt`. Other static asset files are the local CSS/JS and F-monogram SVG; no additional existing third-party notice header or vendored code bundle was found in this scan. That is an inventory observation, not proof of all assets' authorship.

Runtime expanded native package inclusion to LICENSE, LICENSING.md, LICENSES and the font notices. Project/third-party texts are retained separately rather than attributing dependencies or font software to the new original-material terms.

## Dependency notice collector

Implemented only `xtask/src/licenses.rs`, public API `copy_third_party_notices(payload: &Path) -> anyhow::Result<()>`.

- Reads full Cargo metadata with --locked --offline --all-features and excludes workspace member IDs. The output describes the locked workspace graph superset, including development and other-target dependencies; it does not claim exact binary linkage or legal approval.
- Inventories actual recursive license/notice/copyright/copying/authorship documents without copying implementation source files. Explicit license_file is required when declared, confined to the crate, checked for symlinks and normalized to a crate-relative slash path for the generated index.
- Copies exact bytes into THIRD_PARTY_LICENSES/name-version and emits THIRD_PARTY_NOTICES.md with crate versions, source/license metadata, origin and document paths. Documents/index are included in the packager's existing full manifest hashing and extraction checks.
- Missing source licensing documents require an exact name/version/Cargo-source fallback. Pinned repository commit, source URL, file SHA256 and exact fallback inventory are checked; missing, changed or unverified files refuse packaging. Provenance JSON is copied unchanged. Explicitly pinned README attribution can accompany actual notice documents; automatic source scanning does not copy every README.
- Published r-efi5.3.0 already contains AUTHORS with its actual grants and notices, so it is collected directly. The genuinely absent published notice documents are anes0.1.6, la-arena0.3.1 and both winapi GNU0.4.0 import-library crates. Frontend retained verified upstream source/provenance for those exact versions, plus redundant r-efi provenance collateral. New versions/sources do not inherit older fallbacks.
- Limits: metadata16MiB, graph4096packages, walk500000entries/depth32, documents16384, individual document2MiB and aggregate copied documents64MiB. Symlinks/special files, out-of-crate explicit paths, empty or NUL-bearing documents, incompatible outputs and ambiguous name-version directories refuse. Outputs are created exclusively; existing package ownership guards remain in package.rs.

## Validation and integrated result

Focused helper tests: **10 passed**, covering recursive exact-byte copies, explicit custom paths, source-file exclusion, AUTHORS, absolute Windows-prefix path normalization (metadata is queried only after real normal components), unavailable notices, verified fallback/provenance, README attribution, changed hash/source/unverified extras, path escape/symlinks, resource limits and existing output preservation. Receipt: `/cache/tmp/fidryn-licenses-tests.log`.

Scoped all-target/all-feature xtask Clippy with -D warnings passed. Receipt: `/cache/tmp/fidryn-licenses-clippy.log`. Only licenses.rs was formatted/edited by this reviewer.

Root independently reported the actual integrated licensed native preview passing: 530 manifest files, 496 licensing files, 250 dependency entries and four verified upstream fallbacks, with no local cache path leakage in the generated index. Root also verified prior MIT/font/root-doc retention. These are parent-owned integrated receipts; this reviewer did not run a duplicate native build. Full canonical CI/new-head platform qualification remains root's responsibility.

No further concrete content or package omission was found after those updates. The collector is frozen for final qualification. Offline hash/provenance checking trusts the reviewed repository fallback records and resolved Cargo/toolchain inputs; it does not perform fresh network authentication or establish legal approval. Fonts, other non-Cargo assets and toolchain/runtime components are explicitly outside the Cargo graph inventory and retain their separate notice scope.

## Reviewed file identities (SHA256)

- `xtask/src/licenses.rs`: `a392fb88ddb7f03c15bc6368cc42b33eed1b81b8a0878741747bc710db96a87e`
- `LICENSE`: `b7279d8c109c5c3a41e8fb50e4d124545e53110dba78902ff7862ce5c834ef0f`
- `LICENSING.md`: `a93d85322a9f4214c8991228babb80497615b3731c135f176f5e0dc2c691a932`
- `LICENSES/prior-project-LICENSE.txt`: `a1211fd38230055cf76a2a282da537b3e0d6f87d5cd0ee2ad454c30eecea16c9`
- `site/fonts/LICENSE.md`: `03815590a0c9d5058f40dd30c6a67725cf7a154cea5b19d23ddae0fc826ce94b`
- `LICENSES/fonts/README.md`: `c551ae638f15081c4f33b64677ed367f2e737e45fd17bc43d8f79a0525ff56b0`
