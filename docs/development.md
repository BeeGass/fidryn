# Development and promotion

Development takes place on `dev`. BeeGass (GitHub account id `44324535`) is the
initial approved contributor. GitHub authenticates pushes using the account's
SSH credentials or access token; commit signatures independently attest origin.
Approved contributors may push normally to `dev` before CI passes. Other
contributors use PRs with independent approval and passing required checks.
Force pushes and deletion are blocked for both long-lived branches.

`main` requires a PR, successful CI, an up-to-date branch and resolved review
conversations. A separate review ruleset requires one approval, with a BeeGass-only
PR exception. This permits the owner to merge their own PR without waiving tests.
The broad administrator bypass is removed. Critical files have CODEOWNERS.

## Qualification

```sh
cargo xtask ci --fmt
```

The canonical gate requires the pinned Rust toolchain, uv with its locked Python
environment, and Node. It runs workspace tests, Clippy across targets/features,
Rustdoc, schema assertions and controls, Python/JavaScript checks, generated-site
consistency and benchmark compilation. Adapter tests require local mock listeners.
GitHub also runs Linux/macOS/Windows tests, dependency/security checks, secret
scanning, workflow linting and CodeQL.

## Promote through GitHub

1. Push changes to `dev` or merge an independently reviewed contribution into it.
2. Open a PR from `BeeGass/fidryn:dev` to `BeeGass/fidryn:main`.
3. Wait for every required check, resolve conversations, and review the change.
4. Merge through GitHub using a merge commit. BeeGass may use the review exception.
5. Fast-forward `dev` to the resulting main merge commit before the next promotion.

The required `Main source policy` workflow validates both repository identities
and the `dev`/`main` branch names. It needs no App or private key and executes no
candidate code. GitHub checks are commit-scoped; this is the standard native
workflow policy, not an atomic source-branch restriction for multiple PRs sharing
one commit. Use the documented dev-to-main PR path.

## Maintain protections

The reviewed account ids/check names are in `.github/branch-policy.json`.
Render the native rules before applying them:

```sh
cargo xtask governance
cargo xtask governance --apply
```

The command reconciles GitHub rulesets and reads them back. Only BeeGass may apply
changes. Owner review exceptions and approved-contributor direct dev pushes live
in separate rulesets from main's mandatory CI and structural protection.
GitHub settings remain directly inspectable and editable by the repository owner.

## Reproducible evaluation

Normal `run` is operative. Add `--scenario` to apply disclosed hypotheses.
`--budget N` limits actual evaluator operations; the invocation hash includes
that budget. Reports use `fidryn.evaluation-report/v0.2` and retain program/case
identities, clocks, mode, engine semantics, source bundle identity and actual
derivation nodes. Save the report JSON and pass it to `fidryn explain` after
the process exits. An isolated hash has no saved derivation to reconstruct.

`--source-root PATH` selects the filesystem capability root for declared
artifacts and packages. By default it is the module's directory. Shared source
layouts require an explicit enclosing root. Pasted mill source has no filesystem
capability. `fixture` source policy is distinct from observed byte verification.

Query arguments accept JSON literals and tagged case values through `--arg
KEY=VALUE`; unquoted names remain strings. For example, `--arg count=7` supplies
an integer and `--arg ready=true` supplies a Boolean. Input types are checked
against the declared query parameters before evaluation.

## Inputs, documents and review

`fidryn inputs MODULE --query QUERY` describes the query's declared arguments as
JSON Schema. Record fields, aliases, constructors, available nominal entities,
observation records and external review requests come from the checked module.
Unsupported representations produce an error. The schema describes the argument
object, not the whole case record. The mill exposes this through `/api/inputs`;
check diagnostics also retain source paths, byte spans and UTF-16 editor ranges
when the source text is available.

`fidryn render MODULE --template TEMPLATE --data DATA.json` renders structured
JSON under `data`, alongside typed module metadata. `each` iterates arrays and
preserves record fields and names containing punctuation. Missing keys and
scalar/list mismatches are errors. Ordinary document rendering does not establish
semantic certification or legal validity.

`fidryn review MODULE --query QUERY --case CASE.json --valid-at TIME --known-at
TIME > review.json` produces a portable JSON review bundle. It includes the exact
case/root/dependency bytes, checked v0.2 report and actual trace, source-linked
declarations, source observations and rerun argument vector. Its normalized
semantic manifest is explicitly labeled; original manifest formatting is not
reconstructed. Optional `--template` retains reviewer material. The review status
remains `not_assessed`, separate from compilation, byte checks and execution.

Archive paths use `/` independently of the exporting host. Restoring exact source
bytes requires a filesystem that can represent the recorded names; case folding,
Unicode normalization and reserved-name rules can differ between filesystems.
The exporter does not rename retained source references to accommodate another
filesystem, and the bundle does not provide a general archive importer.

Source capture assumes a trusted local filesystem while files are observed.
Static path and symlink checks do not provide descriptor-based isolation against
hostile concurrent path replacement. Compilation uses exclusively created private
snapshot directories (0700, with 0600 files on Unix). Duplicate normalized artifact
paths and aliases are rejected before observations can disagree.

The native policy trusts workflow definitions accepted into the repository.
Required checks bind the GitHub Actions provider and check names; they do not
independently authenticate a workflow's contents. Review changes to `.github/`
and protection policy as security-sensitive owner decisions. CODEOWNERS supplies
the normal review path; the owner remains responsible when using their exception.

CLI/mill reports and review bundles also record `engineArtifact`: a streaming
BLAKE3 digest, byte count, package/semantics versions, architecture and OS of the
executable. Linux hashes the running process inode; other platforms label the
current executable file capture. This identifies the tested artifact without
claiming that the archive distributes it or that rebuilding reproduces its bytes.

## Versioned main promotion

Every main promotion must increase the workspace version and include that
version's dated changelog entry. The required `Release metadata` check compares
the pinned base commit, checks internal dependency and lockfile consistency, and
rejects a reused version. All three platform jobs also build, extract and smoke
test their native packages.

A successful push to main publishes through GitHub releases after the full CI
gate and CodeQL on that exact main commit pass. Packages carry the source commit,
version, native target and hashes. A superseded main run skips publication.
See [Releases](releases.md) for artifact contents and retry behavior.
