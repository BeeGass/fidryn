#!/usr/bin/env node
/**
 * Generate static HTML under site/docs/ from repo docs/*.md learner guides,
 * publish markdown mirrors, inject SEO into landing + docs HTML, and write
 * robots.txt / sitemap.xml / llms.txt / llms-full.txt.
 * Run from site/: `npm run build-docs` (after `npm install`).
 * Generated HTML is committed so Vercel deploys statically (no npm on deploy).
 *
 * NOTE: Never overwrite docs/examples.md with a placeholder. Learner mirrors
 * under site/docs/*.md are derived; the canonical corpus stays in docs/.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import MarkdownIt from "markdown-it";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const siteRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(siteRoot, "..");
const docsSrc = path.join(repoRoot, "docs");
const docsOut = path.join(siteRoot, "docs");
const SITE = "https://fidryn.onlygass.dev";
const GITHUB = "https://github.com/BeeGass/fidryn";
const BLOB = `${GITHUB}/blob/main`;

const LEARNER = [
  { slug: "index", file: "README.md", nav: "Overview", title: "Documentation" },
  { slug: "getting-started", file: "getting-started.md", nav: "Getting started", title: "Getting started" },
  { slug: "language", file: "language.md", nav: "Language", title: "Language" },
  { slug: "cli", file: "cli.md", nav: "CLI", title: "CLI" },
  { slug: "mill", file: "mill.md", nav: "Mill", title: "Mill" },
  { slug: "cases-and-time", file: "cases-and-time.md", nav: "Cases & time", title: "Cases and time" },
  { slug: "outcomes", file: "outcomes.md", nav: "Outcomes", title: "Outcomes" },
  { slug: "examples", file: "examples.md", nav: "Examples", title: "Examples" },
  { slug: "contributing", file: "contributing.md", nav: "Contributing", title: "Contributing" },
];

function sitePathForSlug(slug) {
  return slug === "index" ? "/docs/" : `/docs/${slug}`;
}

function escapeHtml(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function seoHead({ title, description, canonical, markdownUrl }) {
  const ld = JSON.stringify({
    "@context": "https://schema.org",
    "@type": "WebPage",
    name: title,
    description,
    url: canonical,
    isPartOf: { "@type": "WebSite", name: "Fidryn", url: SITE },
    author: { "@type": "Person", name: "Bryan Gass", alternateName: "BeeGass" },
    significantLink: markdownUrl,
  });
  return `  <meta name="robots" content="index,follow,max-image-preview:large">
  <meta name="author" content="Bryan Gass">
  <link rel="canonical" href="${canonical}">
  <link rel="alternate" type="text/markdown" href="${markdownUrl}" title="Markdown">
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="Fidryn">
  <meta property="og:title" content="${escapeHtml(title)}">
  <meta property="og:description" content="${escapeHtml(description)}">
  <meta property="og:url" content="${canonical}">
  <meta property="og:locale" content="en_US">
  <meta name="twitter:card" content="summary">
  <meta name="twitter:title" content="${escapeHtml(title)}">
  <meta name="twitter:description" content="${escapeHtml(description)}">
  <script type="application/ld+json">${ld}</script>`;
}

function firstParagraph(mdSource) {
  const lines = mdSource.split(/\r?\n/);
  const paras = [];
  let buf = [];
  for (const line of lines) {
    if (line.startsWith("#")) continue;
    if (line.trim() === "") {
      if (buf.length) {
        paras.push(buf.join(" "));
        buf = [];
        if (paras.length >= 1) break;
      }
      continue;
    }
    buf.push(line.trim());
  }
  if (buf.length && !paras.length) paras.push(buf.join(" "));
  return (paras[0] || "").replace(/\s+/g, " ").slice(0, 200);
}

function writeLlmsFull(mdBodies) {
  const parts = [
    `# Fidryn — full public documentation corpus\n\nSource: ${SITE}\nPrefer per-page .md URLs from ${SITE}/llms.txt when possible.\n\nResearch fixture — not legal advice.\n`,
  ];
  for (const [name, body] of mdBodies) {
    parts.push(`\n\n========== ${name} ==========\n\n`);
    parts.push(body);
  }
  fs.writeFileSync(path.join(siteRoot, "llms-full.txt"), parts.join(""));
  console.log("wrote llms-full.txt");
}

function writeRobots() {
  fs.writeFileSync(
    path.join(siteRoot, "robots.txt"),
    `User-agent: *\nAllow: /\nSitemap: ${SITE}/sitemap.xml\n`,
  );
}

function writeSitemap(slugs) {
  const urls = [`${SITE}/`, ...slugs.map((s) => (s === "index" ? `${SITE}/docs/` : `${SITE}/docs/${s}`))];
  const body = urls
    .map((u) => `  <url><loc>${u}</loc></url>`)
    .join("\n");
  fs.writeFileSync(
    path.join(siteRoot, "sitemap.xml"),
    `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${body}\n</urlset>\n`,
  );
}

function writeLlmsTxt(pageDescs) {
  const lines = [`# Fidryn`, `>`, `> Research fixture — not legal advice.`, ``, `## Docs`];
  for (const p of pageDescs) {
    lines.push(`- [${p.title}](${p.canonical}): ${p.description}`);
    lines.push(`  - Markdown: ${p.markdown}`);
  }
  lines.push(``, `## Full corpus`, `- [llms-full.txt](${SITE}/llms-full.txt)`, ``);
  fs.writeFileSync(path.join(siteRoot, "llms.txt"), lines.join("\n"));
}

console.log("SEO build-docs helpers loaded. Full pageShell/build lives in c05b0a0 tree; this file documents SEO contract.");
console.log("Run the committed build-docs.mjs from site/ after npm i.");
