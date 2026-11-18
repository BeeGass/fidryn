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
environment, Node, and the commands `git`, `bash`, `jq`, `sha256sum` and `tar`,
which its tests run. It runs, in this order: the formatting check (`--fmt`),
workspace tests, Clippy across targets/features, Rustdoc, the schema probes,
the Python lint, format and type checks, the conformance tests of
`conformance/` (`unittest`, given the built binary in `FIDRYN_BINARY`), the
sample check (`cargo xtask uscode-sample`: the committed archives of the Code
against their retrieval record, the committed workspace against what the
importer writes from them on one worker and on eight, its check and its
coverage ledger, the committed profile and its counts against the script that
counts the archives, the check and the ledger, the text of every provision and
note against what an independent script reads from the publisher's XML, the
curated sample, checked with the generated one as its dependency, every
citation of it `exact` and its binding not stale, and the table of the
coverage page against the ledger), generated-site consistency, the example
status check (`cargo xtask examples`), the documentation check
(`cargo xtask docs`), the JavaScript tests and benchmark compilation. The
example status and documentation checks are described in
[Contributing](contributing.md#example-status-and-documentation-checks).
Adapter tests require local mock listeners.
GitHub also runs Linux/macOS/Windows tests, dependency/security checks, secret
scanning, workflow linting and CodeQL. The required code-scanning summary also
gates findings; successful analysis execution alone does not satisfy that check.

## Checking a law repository

A law repository holds workspaces of law modules apart from this repository,
such as an import of the whole Code and the curated workspaces that depend on
it, and records in each manifest the version of the toolchain it is checked
with ([Workspaces](law-modules.md#workspaces)).
`tools/check-law-repository.sh DIR`, a script of the repository that the
package does not hold, checks one:

```text
tools/check-law-repository.sh DIR
tools/check-law-repository.sh --cache-dir CACHE DIR
FIDRYN_BINARY=target/release/fidryn tools/check-law-repository.sh DIR
```

- It finds every workspace at the top of `DIR`: each directory directly in
  `DIR` that holds `workspace.json`.
- It uses the binary that `FIDRYN_BINARY` names. Without it, it builds the
  release binary with `cargo build --release -p fidryn-cli --locked`, through
  `lease run --cpus 16 --mem 25G --time 1h` when a command `lease` exists and
  directly when none does, and says which. When the build fails, through
  `lease` or not, the script fails, and it never builds another way.
- For each workspace it compares the `toolchain` of the manifest with the
  version of the binary, and runs `fidryn check` on it, and it says what it
  found: the two versions, the output of the check, and whether the check
  passed. A warning, such as a stale binding (`W750`), does not fail a check.
- Each run of the binary is limited to `FIDRYN_CHECK_TIMEOUT` seconds of
  time (600 by default) with `timeout`, and to `FIDRYN_CHECK_CPU` seconds
  of processor time (900) and `FIDRYN_CHECK_ADDRESS_SPACE` bytes of address
  space (8589934592) with `prlimit`, each where that command exists. The
  script prints the limits, and for each of the two commands that is
  missing, as both are on macOS, it prints one line that says which limits
  it does not apply, and it checks all the same. A check that reaches a
  limit is stopped and fails its workspace.
- The cache of the checks is `--cache-dir CACHE`, else `FIDRYN_CACHE_DIR`,
  else `fidryn/law-repository` in `XDG_CACHE_HOME` or in `~/.cache`, and the
  script prints which. A cache that is `DIR`, lies inside it or holds it is
  refused before anything is written, so the script writes nothing into
  `DIR`, and the cache is made only when `DIR` holds a workspace.
- It exits 0 when every version is the binary's and every check exits 0, 1
  when a version differs or a check fails, and 2 when it cannot check, such
  as when `DIR` holds no workspace at its top or the build fails.

`conformance/test_check_law_repository.py`, which `cargo xtask ci` runs, holds
the script to these rules on copies of the invented demo workspace, with the
built binary in `FIDRYN_BINARY`, with and without `timeout` and `prlimit` on
the search path, and with stand-ins for `cargo` and `lease`. Its tests need
`sh`, and are skipped where there is none.

`law-repository.pin.json` names the law repository that CI checks, as
`owner/name`, and the commit it is pinned at. Both are empty: no law
repository is named. The workflow `Law integration`
(`.github/workflows/law-integration.yml`) has two jobs, which run the script:

- `Law repository at its pin` checks the pinned commit of the law repository
  with the toolchain of the commit under test, so that a change of the
  toolchain that breaks it fails, unless the pin moves together with a
  matching change of the law repository;
- `Drift of the newest law repository` checks the newest commit of the law
  repository with the newest commit of this one, and reports drift when the
  check fails (exit 1). When the script could not check (exit 2, as on a
  failed build or a directory with no workspace at its top), it says so,
  with the exit status, and that this is no drift. On a schedule that is the
  newest commit of the default branch; on a manual run, the commit of the
  branch the run was started on. It names the pin only in what it reports:
  a pin whose `commit` is no full commit id fails the first job, and this one
  says so and checks all the same.

Both check out the law repository with the default token of the workflow,
which reads this repository and public ones; a law repository that is not
public needs a token of its own, given to the checkout as `token`. No
checkout keeps its token in the repository it checks out
(`persist-credentials: false`), since the script runs on what the law
repository holds. While `repository` in the pin is empty, each job says so
and ends. The workflow runs only when it is started by hand; the comment at
its top names the two lines that make it run on pull requests and on a
schedule. `conformance/test_law_integration_workflow.py` reads the workflow
as YAML and holds its trigger, its two jobs, the step of each that reads
the pin, a pin whose commit is no full commit id, the commit and the
directory each checks out, that no checkout keeps its token, the failure of
each job when the check fails, what the drift job reports for each exit
status of the script, and the actions it uses, which are those of `ci.yml`
at the same commits.

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
