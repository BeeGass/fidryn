//! Files for search engines and agents.

use super::guides::{GUIDES, SITE, url};
use super::html::esc;

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
