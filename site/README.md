# fidryn.onlygass.dev

Static site for Fidryn: language homepage plus on-site learner docs.

## Deploy

- Host: Vercel (or any static host)
- Project root directory: `site`
- Production domain: `fidryn.onlygass.dev`
- DNS: CNAME `fidryn` → `cname.vercel-dns.com` (or the value Vercel shows) on the Squarespace zone for `onlygass.dev`
- Framework: none (`vercel.json` sets `framework` / install / build to null). Deploy is static HTML/CSS/fonts — no `npm install` on Vercel.
- Generated docs HTML under `site/docs/` is **committed**; regenerating is a local maintainer step.

## Contents

| Path | Purpose |
| --- | --- |
| `/` | Language homepage (what / why / learn / install / `.fr` teaser) |
| `/docs/` | Docs hub (learner guides + implementer links to GitHub) |
| `/docs/getting-started` etc. | Rendered learner guides (`cleanUrls`) |
| `/fonts/*` | Fraunces + IBM Plex (self-hosted) |

Learner guides are built from `docs/*.md` in the repo. Implementer docs (`ARCHITECTURE.md`, `OBLIGATIONS.md`, contracts, status matrix, …) stay on GitHub and are listed from `/docs/`.

The localhost mill (`fidryn ui` / `web/`) stays on 127.0.0.1. Do not publish live filing or the mill API on this host.

## Regenerate docs

From `site/` (Node 18+):

```bash
npm install
npm run build-docs
```

This reads `../docs/*.md` and writes HTML into `site/docs/`. Commit the updated HTML with any markdown edits so production stays static.

Do not commit `site/_deploy_files.json` if it appears locally.
