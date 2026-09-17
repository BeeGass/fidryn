#!/usr/bin/env node
/**
 * Generate static HTML under site/docs/ from repo docs/*.md learner guides.
 * Run from site/: `npm run build-docs` (after `npm install`).
 * Generated HTML is committed so Vercel deploys statically (no npm on deploy).
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
const GITHUB = "https://github.com/BeeGass/fidryn";
const BLOB = `${GITHUB}/blob/main`;

/** Learner guides rendered on-site (slug → source file). */
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

/** Implementer docs: listed on hub, link to GitHub only. */
const IMPLEMENTERS = [
  { file: "ARCHITECTURE.md", label: "Architecture" },
  { file: "implementation-status.md", label: "Implementation status" },
  { file: "OBLIGATIONS.md", label: "Obligations" },
  { file: "INTEGRATION-CONTRACT.md", label: "Integration contract" },
  { file: "INTEGRATION-SUITE.md", label: "Integration suite" },
  { file: "WORKSTREAM-CONTRACT.md", label: "Workstream contract" },
  { file: "REVIEW-FIX-CONTRACT.md", label: "Review-fix contract" },
  { file: "FULL-IMPLEMENTATION.md", label: "Full implementation" },
];

const learnerByFile = new Map(LEARNER.map((p) => [p.file, p]));

function sitePathForSlug(slug) {
  return slug === "index" ? "/docs/" : `/docs/${slug}`;
}

function rewriteHref(href) {
  if (!href || href.startsWith("#") || href.startsWith("mailto:")) {
    return href;
  }

  // linkify may turn bare Foo.md into http://Foo.md — map those back to docs.
  const fakeDoc = href.match(/^https?:\/\/([^\/]+\.(?:md|ebnf|json|fr))(#.*)?$/i);
  if (fakeDoc) {
    return `${BLOB}/docs/${fakeDoc[1]}${fakeDoc[2] || ""}`;
  }

  if (href.startsWith("http://") || href.startsWith("https://")) {
    return href;
  }

  // Strip anchors for lookup; reattach later
  const hashIdx = href.indexOf("#");
  const bare = hashIdx >= 0 ? href.slice(0, hashIdx) : href;
  const hash = hashIdx >= 0 ? href.slice(hashIdx) : "";

  // Repo-relative paths from docs/ (../README.md, ../grammar.ebnf, etc.)
  // Must run before basename learner match so ../README.md ≠ docs hub.
  if (bare.startsWith("../")) {
    const rel = bare.replace(/^\.\.\//, "");
    return `${BLOB}/${rel}${hash}`;
  }

  // Same-directory learner guides only
  const base = path.posix.basename(bare);
  if (!bare.includes("/") && learnerByFile.has(base)) {
    return sitePathForSlug(learnerByFile.get(base).slug) + hash;
  }

  // Same-dir markdown / other implementer docs
  if (bare.endsWith(".md") || bare.endsWith(".ebnf") || bare.endsWith(".json") || bare.endsWith(".fr")) {
    const cleaned = bare.replace(/^\.\//, "");
    if (cleaned.includes("/")) {
      return `${BLOB}/${cleaned}${hash}`;
    }
    return `${BLOB}/docs/${cleaned}${hash}`;
  }

  return href;
}

function makeMd() {
  const md = new MarkdownIt({
    html: false,
    linkify: true,
    typographer: true,
  });

  const defaultLinkOpen =
    md.renderer.rules.link_open ||
    function (tokens, idx, options, env, self) {
      return self.renderToken(tokens, idx, options);
    };

  md.renderer.rules.link_open = function (tokens, idx, options, env, self) {
    const token = tokens[idx];
    const hrefIdx = token.attrIndex("href");
    if (hrefIdx >= 0) {
      token.attrs[hrefIdx][1] = rewriteHref(token.attrs[hrefIdx][1]);
    }
    // External links open in new tab
    const href = hrefIdx >= 0 ? token.attrs[hrefIdx][1] : "";
    if (href.startsWith("http")) {
      token.attrSet("target", "_blank");
      token.attrSet("rel", "noopener noreferrer");
    }
    return defaultLinkOpen(tokens, idx, options, env, self);
  };

  return md;
}

function escapeHtml(s) {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function navHtml(activeSlug) {
  const items = LEARNER.map((p) => {
    const href = sitePathForSlug(p.slug);
    const cls = p.slug === activeSlug ? ' class="active"' : "";
    return `        <a href="${href}"${cls}>${escapeHtml(p.nav)}</a>`;
  }).join("\n");
  return items;
}

function pageShell({ title, activeSlug, bodyHtml, description }) {
  const desc =
    description ||
    "Fidryn documentation — a programming language for legal instruments.";
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
  <meta name="theme-color" content="#120f0c">
  <title>${escapeHtml(title)} — Fidryn</title>
  <meta name="description" content="${escapeHtml(desc)}">
  <link rel="stylesheet" href="/styles.css">
  <link rel="stylesheet" href="/docs.css">
</head>
<body class="docs-body">
  <div class="docs-backdrop" data-docs-close hidden></div>
  <div class="docs-shell">
    <header class="docs-top">
      <a class="mark" href="/">fidryn</a>
      <div class="docs-top-actions">
        <button type="button" class="docs-menu-btn" data-docs-toggle aria-controls="docs-sidebar" aria-expanded="false" aria-label="Open documentation menu">
          <span class="docs-menu-btn-icon" aria-hidden="true"><span></span><span></span><span></span></span>
          <span class="docs-menu-btn-text">Menu</span>
        </button>
        <nav class="nav" aria-label="Site">
          <a href="/docs/">Docs</a>
          <a href="/#install">Install</a>
          <a href="${GITHUB}">GitHub</a>
        </nav>
      </div>
    </header>

    <div class="docs-layout">
      <aside class="docs-sidebar" id="docs-sidebar" aria-label="Documentation">
        <p class="docs-sidebar-label">Guides</p>
        <nav class="docs-side-nav">
${navHtml(activeSlug)}
        </nav>
        <p class="docs-sidebar-label">Also</p>
        <nav class="docs-side-nav">
          <a href="/">Home</a>
          <a href="/#install">Install</a>
          <a href="${GITHUB}" target="_blank" rel="noopener noreferrer">GitHub</a>
          <a href="/llms.txt">llms.txt</a>
        </nav>
      </aside>

      <main class="docs-main prose" id="main">
${bodyHtml}
        <p class="docs-disclaimer"><strong>Research fixture.</strong> Not legal advice, not an operative instrument, and not a complete statement of any jurisdiction&rsquo;s law.</p>
      </main>
    </div>

    <footer class="footer docs-footer">
      <span>Research fixture · Bryan Gass</span>
      <span><a href="https://onlygass.dev">onlygass.dev</a> · <a href="${GITHUB}">source</a> · <a href="/llms.txt">llms.txt</a></span>
    </footer>
  </div>
  <script>
  (function () {
    var body = document.body;
    var btn = document.querySelector("[data-docs-toggle]");
    var backdrop = document.querySelector(".docs-backdrop");
    if (!btn || !backdrop) return;
    backdrop.hidden = false;
    function setOpen(open) {
      body.classList.toggle("docs-nav-open", open);
      btn.setAttribute("aria-expanded", open ? "true" : "false");
      btn.setAttribute("aria-label", open ? "Close documentation menu" : "Open documentation menu");
      body.style.overflow = open ? "hidden" : "";
    }
    btn.addEventListener("click", function () {
      setOpen(!body.classList.contains("docs-nav-open"));
    });
    backdrop.addEventListener("click", function () { setOpen(false); });
    document.addEventListener("keydown", function (e) {
      if (e.key === "Escape") setOpen(false);
    });
    document.getElementById("docs-sidebar").addEventListener("click", function (e) {
      if (e.target.closest("a")) setOpen(false);
    });
  })();
  </script>
</body>
</html>
`;
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

function appendImplementersSection(html) {
  // If the hub markdown already has implementers table, links were rewritten to GitHub.
  // Also ensure a clean card list exists for hub — inject after main content if missing.
  const list = IMPLEMENTERS.map(
    (d) =>
      `  <li><a href="${BLOB}/docs/${d.file}" target="_blank" rel="noopener noreferrer">${escapeHtml(d.label)}</a></li>`
  ).join("\n");
  return (
    html +
    `\n<section class="implementers" id="implementers">\n` +
    `<h2>For implementers</h2>\n` +
    `<p>These live in the repository on GitHub (not rendered on this site):</p>\n` +
    `<ul>\n${list}\n</ul>\n` +
    `</section>\n`
  );
}

function build() {
  if (!fs.existsSync(docsSrc)) {
    console.error(`Docs source not found: ${docsSrc}`);
    process.exit(1);
  }
  fs.mkdirSync(docsOut, { recursive: true });

  const md = makeMd();

  for (const page of LEARNER) {
    const srcPath = path.join(docsSrc, page.file);
    if (!fs.existsSync(srcPath)) {
      console.error(`Missing ${srcPath}`);
      process.exit(1);
    }
    let source = fs.readFileSync(srcPath, "utf8");

    // Hub: drop the "For implementers" markdown table; we inject a GitHub list instead
    // so we don't duplicate and so links are consistent.
    if (page.slug === "index") {
      source = source.replace(/\n## For implementers[\s\S]*$/m, "\n");
    }

    let body = md.render(source);
    if (page.slug === "index") {
      body = appendImplementersSection(body);
    }

    const html = pageShell({
      title: page.title,
      activeSlug: page.slug,
      bodyHtml: body,
      description: firstParagraph(source),
    });

    const outName = page.slug === "index" ? "index.html" : `${page.slug}.html`;
    const outPath = path.join(docsOut, outName);
    fs.writeFileSync(outPath, html);
    console.log(`wrote ${path.relative(siteRoot, outPath)}`);
  }

  console.log("Done. Commit the generated HTML under site/docs/ for static deploy.");
}

build();
