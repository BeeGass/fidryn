# Packages versus fixtures

`packages/` is package identity: a directory per package, a `.fr` module, and a lock whose `digest` is blake3 of those module bytes.

`examples/` and `tests/` often use `"digest": "fixture"` on source-manifest artifacts. That is a test trust profile. It is never `ByteVerified`.

| | Fixtures | Packages |
| --- | --- | --- |
| Where | `examples/`, `tests/`, `prelude/` | `packages/<name>/` |
| Digest | `"fixture"` (policy) or a hex artifact digest in a source manifest | blake3 of the package `.fr` bytes, stored in `manifest.json` |
| Trust | `TrustProfile::Fixture` or authenticated artifacts next to a module | Path compile may `ByteVerified` when the lock digest matches the bytes |
| Compile | `Driver::check_path` may hash files next to the `.fr` | `Driver::check_path` may resolve `source_root/packages` |
| Mill paste | Empty default manifest, `source_root = None` | Never follows `packages/` on the server |

A path-compiled program may `import Std.Core` (package directory `std`) when `packages/std/manifest.json` names a digest that matches `packages/std/*.fr`. A mismatch is diagnostic `E200`. `Std.Core` may itself `import Logic.True` (`packages/logic/true.fr`); path compile authenticates that nested lock and merges unique declarations (depth cap 8). A cycle or missing nested digest is `E200`.

`Driver::check_source` and mill pasted source do not read `packages/`. In-memory compile without a bundle of observed bytes is not `ByteVerified`; a package hex artifact without bytes is unresolved (`E200`). Nested package linking is path compile only.

This tree is not a package store. There is no network fetch and no lockfile syntax in `.fr` beyond the package manifest digest. Path compile that authenticates a package artifact may merge that module's declarations into the importer; in-memory compile still does not read `packages/`.
