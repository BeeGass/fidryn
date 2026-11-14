# Changelog

Release versions follow Semantic Versioning. Source instrument versions and
interchange schema versions are maintained independently of the interpreter.

## [Unreleased]

## [0.2.1] - 2026-11-14

### Changed

- The project is licensed under the MIT License again. `LICENSE` holds the
  MIT text and every workspace package declares `MIT`.
- Thirty-four example modules and seven case records were repaired with
  typing-only edits. 54 of the 114 modules under `examples/`, `packages/`,
  `prelude/` and `tests/programs/` now pass `fidryn check`. Documents mark
  the rest as drafts where they name them by path.
- The implementation status and language guides state what the 0.2
  interpreter interprets and what it does not.
- Internal review ledgers moved under `docs/internal/`.

### Added

- A recorded check status for every example module, enforced in CI, with
  the counts stated in the README and example guides.
- A documentation check that follows links, compiles complete module
  samples and runs the `fidryn` commands that documents show in blocks
  tagged `sh`.
- Tests that pin known limits of the interpreter, cited by the
  implementation status guide.
- A security policy.

### Fixed

- The site footer shows the workspace version.
- Guides no longer rely on a POSIX-only temporary path.
- The examples guide shows only commands that behave as stated.
- A line that does not begin with a continuation is no longer joined to the
  line before it when that line ends in an integer literal, in `operative P`
  or `determined P`, or in an `if` without `else`. `1` followed by a line
  that begins with `days` is no longer the duration `1 days`. An `if` whose
  condition is followed by the end of the statement is rejected with `E100`,
  and the next line is no longer read as its branch.
- A blank line or a comment line no longer cuts a statement short before a
  line that begins with `and`, `or`, `in`, `.`, `,`, `->`, `=>`, `:`, `=`,
  `|` or `<`, so a condition can carry a comment between its lines. After a
  line that ends in an integer literal, in `operative P` or in
  `determined P`, a blank line or a comment line now ends the statement
  before a line that begins with any other continuation, as it already did
  after other lines: `if c > 0`, a comment line and then `{ 1 }` is rejected
  with `E100`.
- `fidryn verify` reads a `verify` block as a list of fields. With the
  exceptions that "The fields of a verify block" in `docs/language.md`
  lists: a field ends where the next one begins, so an `assert` line is its
  own assertion even when the line above it is incomplete; a line that
  begins with `for_all` or `exists` is checked as an assertion; and a
  `for every` line, an `assert` with no expression and a line the block
  does not recognize are kept as assertions that cannot be decided, which
  leaves the property `unknown` unless another assertion is false. Two
  blocks that were `proved`: `assert true or 1 == 2` followed by the line
  `assert false` is a counterexample, and `assert true or 1 +` followed by
  the line `assert false` is rejected with `E100`.
- A `verify` block with no assertion is `unknown`, whatever its comments
  say. The text of such a block was searched for the word `assert` before,
  and a comment that held it was read as the assertion. The bounds of a
  block are read only from the tokens of its `bounds` field. A comment no
  longer sets them, wherever it stands, and neither do the contents of a
  string or another field that holds the word `bounds`. A number is an
  integer token that follows its word directly or after one `:`, so
  `persons: 2.5` gives zero.

## [0.2.0] - 2026-10-29

### Changed

- Current original-project licensing uses the supplied proprietary terms. Prior
  project license text and third-party notices remain retained under their own
  terms; Rust and Python metadata point to the root license file.

- Breaking API and input changes: typed query arguments, nominal entities,
  collections, money currencies and clause references are checked consistently.
  Invalid or undeclared inputs now fail explicitly. Case, source-manifest and
  package-lock versions and semantic field names are validated strictly.
- Operative CLI runs apply hypotheses only with `--scenario`. Reports use
  `fidryn.evaluation-report/v0.2`, retaining invocation, source and executable
  identities, actual derivation graphs, and checked exploration evidence.
- Compiler and Core APIs preserve requirements, structured collections, named
  arguments, observation validators, judgment policies and selection metadata.
- Generic selection, full court-finding and clause execution, position
  materialization and the complete module calculus remain explicit open
  obligations. Unsupported paths return refusals or open outcomes.

### Added

- Source-defined concurrence judgments and typed observations with scoped,
  temporal evidence validation; grounded parameterized duty instances.
- Configurable shared evaluation budgets, continuation accounting, parser/input
  limits, bounded completion search and retained trace limits.
- Retained project/font license texts and locked dependency notice inventories in
  release archives, with narrowly scoped first-party license hash checks.
- Typed query input schemas, editor diagnostic ranges, structured document
  rendering and portable review bundles with retained inputs and rerun metadata.
- Native GitHub protections, mandatory test/security checks, release metadata
  validation, distributable native packages and release publication from main.

### Fixed

- Timestamp ordering and named record-constructor calls through nested functions.
- Canonical portable review-bundle paths on Windows, including nested and missing
  source observations.
- Consistent generated-site diagnostic paths across hosts.
- Requirements and assertions discarded by parsing or lowering; untyped Boolean
  and result escapes; literal overflow and string/collection corruption.
- Circular rule support, order-dependent conflicts, shared continuation frames,
  staged duty reads and transaction progress that survived rollback.
- Unrelated, duplicated, stale or future evidence and unscoped acceptance or
  authority affecting institutional results.
- Inconsistent completion admission, assignment identity collisions, inaccurate
  coverage and replay claims that did not agree with the actual typed result.
- Compilation trust depending on cache order; incomplete dependency observations;
  source path escapes, alias conflicts and non-private capture directories.

### Security

- Required code-scanning findings checks and a publication guard for unresolved
  errors or high/critical findings, in addition to analysis execution checks.

- Updated vulnerable or unsound pinned dependencies and added locked dependency,
  secret-history/current-tree, workflow and CodeQL checks.
- Main's mandatory checks have no bypass actors; the owner's review exception
  remains separate. Source capture and workflow-content trust limits are
  documented with the remaining language obligations.
