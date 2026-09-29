# 0001: Generate the site with a Rust xtask

Date: 2026-09-29. Status: Accepted.

## Context

The learner docs under `site/docs/` came from a Node script
(`site/scripts/build-docs.mjs`, markdown-it). The script had been
reduced to a stub, so the committed HTML was patched by hand and
drifted from `docs/*.md`. The project wants to stay on the Rust stack,
with no Node build step, and the site and the mill were being
redesigned together.

## Decision

`cargo xtask site` renders the whole site (landing page, docs, markdown
mirrors, search index, SEO files) from `docs/*.md`. It uses
pulldown-cmark, the `fidryn-syntax` lexer for `.fr` highlighting, and
`fidryn-cli` to evaluate the landing specimen, so the page shows real
interpreter output. `--check` fails when the committed output is
stale, and `cargo xtask ci` runs it. The design system
(`site/assets/fidryn.css`) and the site script
(`site/assets/fidryn.js`) are hand-written; the mill embeds the
stylesheet and has its own script, `web/mill.js`.

## Consequences

- A docs change needs `cargo xtask site` and a commit of the
  regenerated files; CI catches a forgotten regeneration.
- A change in evaluation semantics makes the site stale until it is
  regenerated, which is intended.
- `xtask` now depends on `fidryn-cli` (a heavier build) and
  `pulldown-cmark`.
- Node is optional: it only runs `node --test` over the JavaScript
  helpers.

Alternatives considered: a separate `fidryn-site` crate (no second
consumer, so no gain) and askama templates (new proc-macro
dependencies for four templates).
