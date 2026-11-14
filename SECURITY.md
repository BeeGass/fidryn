# Security policy

## Reporting a vulnerability

Use GitHub's private reporting: on the repository page open the
**Security** tab and choose **Report a vulnerability**. Please do not open
a public issue for a suspected vulnerability.

Include the version (`fidryn --version`), the platform, and the smallest
module, case record or command that shows the problem.

## What is in scope

- The `fidryn` binary and the crates in this repository.
- The local web interface started by `fidryn ui`. It binds to `127.0.0.1`
  only and is not meant to be exposed to a network.
- Handling of untrusted input: module source, case records, source
  manifests, templates and review bundles.

## What is not a vulnerability

- A legal conclusion you disagree with. Modules in this repository are
  research fixtures, not legal advice.
- Live filing. It is off unless both `--live` and
  `FIDRYN_ALLOW_LIVE_FILING=1` are set by the operator.

## Supported versions

Only the latest release receives fixes.
