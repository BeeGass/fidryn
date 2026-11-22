# Crate license documents

These directories retain exact upstream license and attribution documents for
the named crate versions. They supplement published crate archives whose file
selection omitted those documents. They do not change the upstream terms or the
license of original Fidryn material.

Each `provenance.json` binds the crate name, version and Cargo registry source to
an immutable upstream commit. Every retained document records its upstream URL
at that commit and the SHA-256 of its exact bytes. Package generation verifies
those fields and hashes before using a fallback. A different version or source
requires its own reviewed documents.

| Crate | Revision evidence | Retained documents |
| --- | --- | --- |
| `anes 0.1.6` | Published `.cargo_vcs_info.json` pins [378ac7461ecac29fde9e10df7b08359d1315a9ad](https://github.com/qwandor/anes-rs/tree/378ac7461ecac29fde9e10df7b08359d1315a9ad). The upstream [`anes/Cargo.toml`](https://github.com/qwandor/anes-rs/blob/378ac7461ecac29fde9e10df7b08359d1315a9ad/anes/Cargo.toml) exactly matches the published `Cargo.toml.orig`. The published repository URL, `zrzka/anes-rs`, redirects to `qwandor/anes-rs`. | `LICENSE-APACHE`, `LICENSE-MIT`, `README.md` |
| `la-arena 0.3.1` | Upstream release commit [a0467c546c1e412cd1b35257f8d5ae08f79072d6](https://github.com/rust-lang/rust-analyzer/commit/a0467c546c1e412cd1b35257f8d5ae08f79072d6), titled `Release la-arena 0.3.1`. Its [`lib/la-arena/Cargo.toml`](https://github.com/rust-lang/rust-analyzer/blob/a0467c546c1e412cd1b35257f8d5ae08f79072d6/lib/la-arena/Cargo.toml) exactly matches the published `Cargo.toml.orig`. | `LICENSE-APACHE`, `LICENSE-MIT` |
| `r-efi 5.3.0` | Published `.cargo_vcs_info.json` pins [97b55bed1c2c91dcbf787674849f05337ff80b33](https://github.com/r-efi/r-efi/tree/97b55bed1c2c91dcbf787674849f05337ff80b33). Its [`Cargo.toml`](https://github.com/r-efi/r-efi/blob/97b55bed1c2c91dcbf787674849f05337ff80b33/Cargo.toml) exactly matches the published `Cargo.toml.orig`. | `AUTHORS` |
| `winapi-i686-pc-windows-gnu 0.4.0` | Upstream release commit [9497609ef44cc9bcd16cd2411c0ee6ccaf5483aa](https://github.com/retep998/winapi-rs/commit/9497609ef44cc9bcd16cd2411c0ee6ccaf5483aa), titled `Publish 0.3.4` for the root `winapi` crate. Its [`i686/Cargo.toml`](https://github.com/retep998/winapi-rs/blob/9497609ef44cc9bcd16cd2411c0ee6ccaf5483aa/i686/Cargo.toml) declares the import-library crate version `0.4.0` and exactly matches the published `Cargo.toml.orig`. | `LICENSE-APACHE`, `LICENSE-MIT` |
| `winapi-x86_64-pc-windows-gnu 0.4.0` | The same [9497609ef44cc9bcd16cd2411c0ee6ccaf5483aa](https://github.com/retep998/winapi-rs/commit/9497609ef44cc9bcd16cd2411c0ee6ccaf5483aa) release commit has [`x86_64/Cargo.toml`](https://github.com/retep998/winapi-rs/blob/9497609ef44cc9bcd16cd2411c0ee6ccaf5483aa/x86_64/Cargo.toml) declaring version `0.4.0` and exactly matching the published `Cargo.toml.orig`. | `LICENSE-APACHE`, `LICENSE-MIT` |

`r-efi` already ships `AUTHORS` in its published crate. That file includes its MIT
permission text, alternative-license notices, copyright notices and author list.
The retained copy here provides pinned provenance; normal package generation
uses the published copy directly. The other four crates need the repository
fallbacks above.

The ANES README is also retained because its license section contains the
upstream contributor attribution statement. No copyright names were added to
upstream license files that omit a named copyright line.

All documents were retrieved through the primary repositories at the recorded
commits and preserved without modifying names, attribution or license text.
