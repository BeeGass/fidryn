//! Files for search engines and agents.

use super::guides::{GUIDES, SITE, url};

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
