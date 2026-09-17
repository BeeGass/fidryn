# fidryn.onlygass.dev

Static landing site for Fidryn.

## Deploy

- Host: Vercel (or any static host)
- Project root directory: `site`
- Production domain: `fidryn.onlygass.dev`
- DNS: CNAME `fidryn` → `cname.vercel-dns.com` (or the value Vercel shows) on the Squarespace zone for `onlygass.dev`

## Contents

| Path | Purpose |
| --- | --- |
| `/` | Product / install landing |

The localhost mill (`fidryn ui` / `web/`) stays on 127.0.0.1. Do not publish live filing or the mill API on this host.
