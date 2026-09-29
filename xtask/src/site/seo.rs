//! Files for search engines and agents.

use super::guides::{GUIDES, Guide, SITE, url};
use super::html::esc;
use super::links::rewrite_markdown_links;

/// `robots.txt`: allow everything and point at the sitemap and the llms maps.
pub fn robots() -> String {
    format!(
        "User-agent: *\nAllow: /\n\nSitemap: {SITE}/sitemap.xml\n\n# LLM / agent maps\n# {SITE}/llms.txt\n# {SITE}/llms-full.txt\n"
    )
}

/// `sitemap.xml`: the landing page, then every guide in reading order. No
/// `lastmod`, so the file only changes when the guide list does.
pub fn sitemap() -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    push_url(&mut out, "/", "1.0");
    for guide in GUIDES {
        let priority = if guide.number.is_some() { "0.8" } else { "0.9" };
        push_url(&mut out, &url(guide.slug), priority);
    }
    out.push_str("</urlset>\n");
    out
}

fn push_url(out: &mut String, path: &str, priority: &str) {
    out.push_str(&format!(
        "  <url>\n    <loc>{SITE}{path}</loc>\n    <changefreq>weekly</changefreq>\n    <priority>{priority}</priority>\n  </url>\n"
    ));
}

/// `<title>` of the landing page.
pub const LANDING_TITLE: &str = "Fidryn — a programming language for legal instruments";

/// Meta description of the landing page, also the summary in `llms.txt`.
pub const LANDING_DESCRIPTION: &str = "Fidryn (FID-rin) is a programming language for legal instruments: precise where law is mechanical, explicit where judgment enters, and incapable of hiding authority inside a Boolean. Research fixture — not legal advice.";

/// Which structured data and robots policy a page gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKind {
    Landing,
    Doc,
    NotFound,
}

/// SEO tags for `<head>`, one per line: robots, author, canonical, the
/// markdown alternate (when `markdown_path` is given), Open Graph, Twitter,
/// and JSON-LD. `path` and `markdown_path` are site paths such as
/// `/docs/cli` and `/docs/cli.md`; `title` and `description` are plain text.
pub fn head(
    title: &str,
    description: &str,
    path: &str,
    markdown_path: Option<&str>,
    kind: PageKind,
) -> String {
    let url = format!("{SITE}{path}");
    let markdown_url = markdown_path.map(|p| format!("{SITE}{p}"));
    let robots = match kind {
        PageKind::NotFound => "noindex",
        PageKind::Landing | PageKind::Doc => "index,follow,max-image-preview:large",
    };
    let (t, d) = (esc(title), esc(description));
    let mut lines = vec![
        format!("<meta name=\"robots\" content=\"{robots}\">"),
        "<meta name=\"author\" content=\"Bryan Gass\">".to_owned(),
        format!("<link rel=\"canonical\" href=\"{}\">", esc(&url)),
    ];
    if let Some(md) = &markdown_url {
        lines.push(format!(
            "<link rel=\"alternate\" type=\"text/markdown\" href=\"{}\" title=\"Markdown\">",
            esc(md)
        ));
    }
    lines.extend([
        "<meta property=\"og:type\" content=\"website\">".to_owned(),
        "<meta property=\"og:site_name\" content=\"Fidryn\">".to_owned(),
        format!("<meta property=\"og:title\" content=\"{t}\">"),
        format!("<meta property=\"og:description\" content=\"{d}\">"),
        format!("<meta property=\"og:url\" content=\"{}\">", esc(&url)),
        "<meta property=\"og:locale\" content=\"en_US\">".to_owned(),
        "<meta name=\"twitter:card\" content=\"summary\">".to_owned(),
        format!("<meta name=\"twitter:title\" content=\"{t}\">"),
        format!("<meta name=\"twitter:description\" content=\"{d}\">"),
    ]);
    let data = match kind {
        PageKind::Landing => serde_json::json!({
            "@context": "https://schema.org",
            "@type": "SoftwareApplication",
            "name": "Fidryn",
            "applicationCategory": "DeveloperApplication",
            "operatingSystem": "Linux, macOS, Windows",
            "programmingLanguage": "Fidryn",
            "url": SITE,
            "downloadUrl": "https://github.com/BeeGass/fidryn",
            "author": {"@type": "Person", "name": "Bryan Gass", "alternateName": "BeeGass", "url": "https://onlygass.dev"},
            "description": description,
            "license": "https://github.com/BeeGass/fidryn/blob/main/LICENSE",
        }),
        PageKind::Doc | PageKind::NotFound => {
            let mut page = serde_json::json!({
                "@context": "https://schema.org",
                "@type": "WebPage",
                "name": title,
                "description": description,
                "url": url,
                "isPartOf": {"@type": "WebSite", "name": "Fidryn", "url": SITE},
                "author": {"@type": "Person", "name": "Bryan Gass", "alternateName": "BeeGass"},
            });
            if let Some(md) = markdown_url {
                page["significantLink"] = serde_json::Value::String(md);
            }
            page
        }
    };
    // `</` inside a script element would end it early; `<\/` is the same JSON.
    let json = data.to_string().replace("</", "<\\/");
    lines.push(format!(
        "<script type=\"application/ld+json\">{json}</script>"
    ));
    lines.join("\n")
}

/// The markdown mirror of a guide (`site/docs/{slug}.md`): front matter, a
/// note naming the canonical HTML page, then the source with every link
/// made absolute.
pub fn mirror(guide: &Guide, md: &str) -> String {
    let html = format!("{SITE}{}", url(guide.slug));
    format!(
        "{}> Canonical HTML: {html}\n> This markdown mirror is for agents and plain-text readers.\n\n{}\n",
        front_matter(
            guide.title,
            guide.description,
            &html,
            &format!("{SITE}/docs/{}.md", guide.slug)
        ),
        rewrite_markdown_links(md).trim_end()
    )
}

/// `site/index.md`: the landing page as markdown.
pub fn landing_markdown() -> String {
    let mut docs = String::new();
    for g in GUIDES {
        let (label, file) = if g.number.is_some() {
            (g.title, format!("{}.md", g.slug))
        } else {
            ("Docs hub", "docs/index.md".to_owned())
        };
        docs.push_str(&format!(
            "- [{label}]({SITE}{}) · [{file}]({SITE}/docs/{}.md)\n",
            url(g.slug),
            g.slug
        ));
    }
    format!(
        concat!(
            "{front}",
            "# Fidryn (FID-rin)\n\n",
            "Fidryn is a **programming language for legal instruments**: precise where law is mechanical, explicit where judgment enters, and incapable of hiding authority, discretion, or ambiguity inside a Boolean.\n\n",
            "**Research fixture.** Not legal advice, not an operative instrument, and not a complete statement of any jurisdiction's law.\n\n",
            "## What it is\n\n",
            "- Source files use the `.fr` extension.\n",
            "- You write modules, queries, and duties; the reference interpreter checks them and evaluates queries against case records.\n",
            "- It never invents a completion when the model still has open branches.\n",
            "- Determinate results only when invariant across every still-admissible resolution — or a competent authority has already determined them.\n\n",
            "## Install\n\n",
            "```bash\ncargo install --git https://github.com/BeeGass/fidryn --locked fidryn-cli\n```\n\n",
            "Requires Rust 1.98+. Local mill: `fidryn ui --no-open` (loopback only, default `127.0.0.1:8751`). This public site does **not** expose live filing or the mill API.\n\n",
            "## Documentation\n\n",
            "{docs}\n",
            "## Agent maps\n\n",
            "- [llms.txt]({site}/llms.txt)\n",
            "- [llms-full.txt]({site}/llms-full.txt)\n",
            "- [sitemap.xml]({site}/sitemap.xml)\n\n",
            "## Source\n\n",
            "https://github.com/BeeGass/fidryn\n",
        ),
        front = front_matter(
            LANDING_TITLE,
            LANDING_DESCRIPTION,
            &format!("{SITE}/"),
            &format!("{SITE}/index.md")
        ),
        docs = docs,
        site = SITE,
    )
}

/// `site/llms.txt`: the curated map of public pages for agents.
pub fn llms_txt() -> String {
    let mut guides = String::new();
    for g in GUIDES.iter().filter(|g| g.number.is_some()) {
        guides.push_str(&format!(
            "- [{}]({SITE}/docs/{}.md): {}\n  - HTML: {SITE}{}\n",
            g.title,
            g.slug,
            g.description,
            url(g.slug)
        ));
    }
    format!(
        concat!(
            "# Fidryn\n\n",
            "> {description}\n\n",
            "This file follows the llms.txt convention: a curated map of public pages, with clean markdown mirrors for agents.\n",
            "Human-facing HTML is unchanged; prefer `text/markdown` URLs below when you need the full text.\n\n",
            "Site: {site}\n",
            "Full corpus: {site}/llms-full.txt\n",
            "Source: https://github.com/BeeGass/fidryn\n\n",
            "## Primary pages\n\n",
            "- [Home]({site}/): Overview — programming language for legal instruments\n",
            "  - Markdown: {site}/index.md\n",
            "- [Docs hub]({site}/docs/): Learner documentation index\n",
            "  - Markdown: {site}/docs/index.md\n\n",
            "## Learner guides\n\n",
            "{guides}\n",
            "## Notes for agents\n\n",
            "- Research fixture — not legal advice.\n",
            "- Per-page markdown mirrors use the `.md` suffix and `Content-Type: text/markdown`.\n",
            "- HTML pages advertise the mirror via `rel=alternate` / `type=text/markdown`.\n",
            "- The local mill (`fidryn ui`) binds loopback only and is not exposed on this site.\n",
        ),
        description = LANDING_DESCRIPTION,
        site = SITE,
        guides = guides,
    )
}

/// `site/llms-full.txt`: a header, the guide index, then every mirror in
/// full, each under a banner naming its URL.
pub fn llms_full(mirrors: &[(&Guide, String)]) -> String {
    let mut out = format!(
        concat!(
            "# Fidryn — full public documentation corpus\n\n",
            "Source: {site}\n",
            "Prefer per-page .md URLs from {site}/llms.txt when possible.\n\n",
            "Research fixture — not legal advice.\n\n",
            "---\n\n",
            "## Guide index\n\n",
            "| Guide | HTML | Markdown |\n",
            "| --- | --- | --- |\n",
            "| Home | {site}/ | {site}/index.md |\n",
        ),
        site = SITE
    );
    for (g, _) in mirrors {
        out.push_str(&format!(
            "| {} | {SITE}{} | {SITE}/docs/{}.md |\n",
            g.title,
            url(g.slug),
            g.slug
        ));
    }
    for (g, mirror) in mirrors {
        out.push_str(&format!(
            "\n========== {SITE}/docs/{}.md ==========\n\n{mirror}",
            g.slug
        ));
    }
    out
}

/// YAML front matter in the fixed key order the mirrors have always used.
fn front_matter(title: &str, description: &str, url: &str, markdown: &str) -> String {
    let quote = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    format!(
        "---\ntitle: {}\ndescription: {}\nurl: {}\nmarkdown: {}\nauthor: \"Bryan Gass\"\n---\n\n",
        quote(title),
        quote(description),
        quote(url),
        quote(markdown)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robots_is_exact() {
        assert_eq!(
            robots(),
            "User-agent: *\nAllow: /\n\nSitemap: https://fidryn.onlygass.dev/sitemap.xml\n\n# LLM / agent maps\n# https://fidryn.onlygass.dev/llms.txt\n# https://fidryn.onlygass.dev/llms-full.txt\n"
        );
    }

    #[test]
    fn sitemap_lists_the_landing_page_then_every_guide_in_order() {
        let xml = sitemap();
        let locs: Vec<&str> = xml
            .split("<loc>")
            .skip(1)
            .map(|rest| &rest[..rest.find("</loc>").expect("closed <loc>")])
            .collect();
        assert_eq!(
            locs,
            [
                "https://fidryn.onlygass.dev/",
                "https://fidryn.onlygass.dev/docs/",
                "https://fidryn.onlygass.dev/docs/getting-started",
                "https://fidryn.onlygass.dev/docs/language",
                "https://fidryn.onlygass.dev/docs/cases-and-time",
                "https://fidryn.onlygass.dev/docs/cli",
                "https://fidryn.onlygass.dev/docs/mill",
                "https://fidryn.onlygass.dev/docs/outcomes",
                "https://fidryn.onlygass.dev/docs/examples",
                "https://fidryn.onlygass.dev/docs/contributing",
            ]
        );
    }

    #[test]
    fn sitemap_is_a_complete_urlset_without_dates() {
        let xml = sitemap();
        assert!(xml.starts_with(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n  <url>\n    <loc>https://fidryn.onlygass.dev/</loc>\n    <changefreq>weekly</changefreq>\n    <priority>1.0</priority>\n  </url>\n"
        ));
        assert!(xml.ends_with("  </url>\n</urlset>\n"));
        assert_eq!(xml.matches("<url>").count(), 10);
        assert!(!xml.contains("lastmod"));
    }
}

#[cfg(test)]
mod head_tests {
    use super::*;

    fn json_ld(head: &str) -> serde_json::Value {
        let start = head
            .find("<script type=\"application/ld+json\">")
            .expect("JSON-LD")
            + "<script type=\"application/ld+json\">".len();
        let end = head[start..].find("</script>").expect("script closes") + start;
        serde_json::from_str(&head[start..end]).expect("JSON-LD parses")
    }

    #[test]
    fn doc_head_has_canonical_alternate_social_tags_and_web_page_data() {
        let head = head(
            "Outcomes — Fidryn",
            "Determinate & the rest.",
            "/docs/outcomes",
            Some("/docs/outcomes.md"),
            PageKind::Doc,
        );
        for line in [
            "<meta name=\"robots\" content=\"index,follow,max-image-preview:large\">",
            "<meta name=\"author\" content=\"Bryan Gass\">",
            "<link rel=\"canonical\" href=\"https://fidryn.onlygass.dev/docs/outcomes\">",
            "<link rel=\"alternate\" type=\"text/markdown\" href=\"https://fidryn.onlygass.dev/docs/outcomes.md\" title=\"Markdown\">",
            "<meta property=\"og:type\" content=\"website\">",
            "<meta property=\"og:title\" content=\"Outcomes — Fidryn\">",
            "<meta property=\"og:description\" content=\"Determinate &amp; the rest.\">",
            "<meta property=\"og:url\" content=\"https://fidryn.onlygass.dev/docs/outcomes\">",
            "<meta name=\"twitter:card\" content=\"summary\">",
            "<meta name=\"twitter:title\" content=\"Outcomes — Fidryn\">",
            "<meta name=\"twitter:description\" content=\"Determinate &amp; the rest.\">",
        ] {
            assert!(head.lines().any(|l| l == line), "missing {line}\n{head}");
        }
        let data = json_ld(&head);
        assert_eq!(data["@type"], "WebPage");
        assert_eq!(data["name"], "Outcomes — Fidryn");
        assert_eq!(data["description"], "Determinate & the rest.");
        assert_eq!(data["url"], "https://fidryn.onlygass.dev/docs/outcomes");
        assert_eq!(
            data["significantLink"],
            "https://fidryn.onlygass.dev/docs/outcomes.md"
        );
        assert!(!head.ends_with('\n'));
    }

    #[test]
    fn landing_head_describes_the_software() {
        let head = head(
            LANDING_TITLE,
            LANDING_DESCRIPTION,
            "/",
            Some("/index.md"),
            PageKind::Landing,
        );
        assert!(head.contains("<link rel=\"canonical\" href=\"https://fidryn.onlygass.dev/\">"));
        assert!(head.contains("href=\"https://fidryn.onlygass.dev/index.md\""));
        let data = json_ld(&head);
        assert_eq!(data["@type"], "SoftwareApplication");
        assert_eq!(data["name"], "Fidryn");
        assert_eq!(data["description"], LANDING_DESCRIPTION);
        assert_eq!(data["downloadUrl"], "https://github.com/BeeGass/fidryn");
    }

    #[test]
    fn not_found_head_is_noindex_without_alternate() {
        let head = head(
            "Not found — Fidryn",
            "Gone.",
            "/404",
            None,
            PageKind::NotFound,
        );
        assert!(head.contains("<meta name=\"robots\" content=\"noindex\">"));
        assert!(!head.contains("index,follow"));
        assert!(!head.contains("text/markdown"));
        assert!(head.contains("<link rel=\"canonical\" href=\"https://fidryn.onlygass.dev/404\">"));
        let data = json_ld(&head);
        assert_eq!(data["@type"], "WebPage");
        assert!(data.get("significantLink").is_none());
    }

    #[test]
    fn json_ld_cannot_close_its_script_element() {
        let head = head("A </script><b>", "</p>", "/docs/x", None, PageKind::Doc);
        let script = &head[head.find("<script").unwrap()..];
        assert_eq!(
            script.matches("</").count(),
            1,
            "only the real closing tag: {script}"
        );
        assert!(script.contains("A <\\/script><b>"));
        assert_eq!(json_ld(&head)["name"], "A </script><b>");
        assert!(
            head.contains("<meta property=\"og:title\" content=\"A &lt;/script&gt;&lt;b&gt;\">")
        );
    }
}

#[cfg(test)]
mod corpus_tests {
    use super::*;

    fn guide(slug: &str) -> &'static Guide {
        GUIDES.iter().find(|g| g.slug == slug).expect("guide")
    }

    /// Every `](target)` link target in markdown text.
    fn link_targets(md: &str) -> Vec<&str> {
        md.match_indices("](")
            .filter_map(|(i, _)| {
                let rest = &md[i + 2..];
                rest.find(')').map(|end| &rest[..end])
            })
            .collect()
    }

    #[test]
    fn mirror_has_front_matter_note_and_absolute_links() {
        let md = "# Outcomes\n\nSee [CLI](cli.md), [the kinds](#outcome-kinds), [the overview](README.md), and [the schema](../schemas/outcome-v0.1.json).\n";
        let text = mirror(guide("outcomes"), md);
        assert!(text.starts_with(concat!(
            "---\n",
            "title: \"Outcomes\"\n",
            "description: \"Determinate, Suspended, Contingent, and the rest of the Fidryn outcome envelope.\"\n",
            "url: \"https://fidryn.onlygass.dev/docs/outcomes\"\n",
            "markdown: \"https://fidryn.onlygass.dev/docs/outcomes.md\"\n",
            "author: \"Bryan Gass\"\n",
            "---\n",
            "\n",
            "> Canonical HTML: https://fidryn.onlygass.dev/docs/outcomes\n",
            "> This markdown mirror is for agents and plain-text readers.\n",
            "\n",
            "# Outcomes\n",
        )));
        assert!(text.ends_with(".json).\n") && !text.ends_with("\n\n"));
        let targets = link_targets(&text);
        assert_eq!(targets.len(), 4, "{targets:?}");
        for target in targets {
            assert!(
                target.starts_with("https://") || target.starts_with('#'),
                "relative link left in the mirror: {target}"
            );
        }
        assert!(
            text.contains(
                "](https://github.com/BeeGass/fidryn/blob/main/schemas/outcome-v0.1.json)"
            )
        );

        let overview = mirror(guide("index"), "# Fidryn documentation\n");
        assert!(overview.contains("url: \"https://fidryn.onlygass.dev/docs/\"\n"));
        assert!(overview.contains("markdown: \"https://fidryn.onlygass.dev/docs/index.md\"\n"));
    }

    #[test]
    fn landing_markdown_keeps_its_sections_and_lists_every_guide() {
        let md = landing_markdown();
        assert!(md.starts_with(
            "---\ntitle: \"Fidryn — a programming language for legal instruments\"\n"
        ));
        assert!(md.contains("url: \"https://fidryn.onlygass.dev/\"\nmarkdown: \"https://fidryn.onlygass.dev/index.md\"\n"));
        for heading in [
            "# Fidryn (FID-rin)",
            "## What it is",
            "## Install",
            "## Documentation",
            "## Agent maps",
            "## Source",
        ] {
            assert!(md.contains(&format!("\n{heading}\n")), "missing {heading}");
        }
        assert!(md.contains(
            "- [Docs hub](https://fidryn.onlygass.dev/docs/) · [docs/index.md](https://fidryn.onlygass.dev/docs/index.md)\n"
        ));
        assert!(md.contains(
            "- [Cases and time](https://fidryn.onlygass.dev/docs/cases-and-time) · [cases-and-time.md](https://fidryn.onlygass.dev/docs/cases-and-time.md)\n"
        ));
        let listed = md.matches("](https://fidryn.onlygass.dev/docs/").count();
        assert_eq!(listed, 2 * GUIDES.len());
        assert!(md.ends_with("## Source\n\nhttps://github.com/BeeGass/fidryn\n"));
    }

    #[test]
    fn llms_txt_lists_every_guide_with_html_and_markdown_urls() {
        let txt = llms_txt();
        assert!(txt.starts_with(&format!("# Fidryn\n\n> {LANDING_DESCRIPTION}\n\n")));
        assert!(txt.contains(&format!(
            "- [Docs hub]({SITE}/docs/): Learner documentation index\n  - Markdown: {SITE}/docs/index.md\n"
        )));
        let mut last = 0;
        for g in GUIDES.iter().filter(|g| g.number.is_some()) {
            let entry = format!(
                "- [{}]({SITE}/docs/{}.md): {}\n  - HTML: {SITE}/docs/{}\n",
                g.title, g.slug, g.description, g.slug
            );
            let at = txt
                .find(&entry)
                .unwrap_or_else(|| panic!("missing {}:\n{txt}", g.slug));
            assert!(at > last, "{} out of order", g.slug);
            last = at;
        }
        assert!(txt.ends_with("is not exposed on this site.\n"));
    }

    #[test]
    fn llms_full_contains_the_index_and_every_mirror() {
        let mirrors: Vec<(&Guide, String)> = GUIDES
            .iter()
            .map(|g| {
                (
                    g,
                    mirror(g, &format!("# {}\n\nBody of {}.\n", g.title, g.slug)),
                )
            })
            .collect();
        let full = llms_full(&mirrors);
        assert!(full.starts_with("# Fidryn — full public documentation corpus\n\n"));
        assert!(full.contains(&format!("| Home | {SITE}/ | {SITE}/index.md |\n")));
        for (g, text) in &mirrors {
            assert!(full.contains(&format!(
                "| {} | {SITE}{} | {SITE}/docs/{}.md |\n",
                g.title,
                url(g.slug),
                g.slug
            )));
            assert!(
                full.contains(&format!(
                    "\n========== {SITE}/docs/{}.md ==========\n\n{text}",
                    g.slug
                )),
                "{} mirror missing",
                g.slug
            );
        }
        assert!(full.ends_with("Body of contributing.\n"));
    }
}
