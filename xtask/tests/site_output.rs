//! Integrity of the committed site under `site/`: every internal link and
//! anchor resolves, every page carries its metadata, and a fresh render
//! matches what is committed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ORIGIN: &str = "https://fidryn.onlygass.dev";

/// Generated text files whose absolute site URLs must resolve.
const TEXT_FILES: &[&str] = &[
    "index.md",
    "llms.txt",
    "llms-full.txt",
    "sitemap.xml",
    "robots.txt",
];

fn site() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("site")
}

/// Paths of every file under `dir`, relative to it, with `/` separators.
fn files_under(dir: &Path, rel: &str, out: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        if entry.file_type().unwrap().is_dir() {
            files_under(&entry.path(), &path, out);
        } else {
            out.push(path);
        }
    }
}

/// The site's HTML pages, keyed by path relative to `site/`.
fn pages() -> BTreeMap<String, String> {
    let mut all = Vec::new();
    files_under(&site(), "", &mut all);
    all.into_iter()
        .filter(|p| p.ends_with(".html"))
        .map(|p| {
            let html = fs::read_to_string(site().join(&p)).unwrap();
            (p, html)
        })
        .collect()
}

/// Values of every `name="…"` attribute in `html`, entity `&amp;` decoded.
fn attrs(html: &str, name: &str) -> Vec<String> {
    let needle = format!(" {name}=\"");
    html.match_indices(&needle)
        .map(|(i, _)| {
            let rest = &html[i + needle.len()..];
            rest[..rest.find('"').expect("attribute closes")].replace("&amp;", "&")
        })
        .collect()
}

/// The file under `site/` that serves a site path, with Vercel's `cleanUrls`.
fn resolve(path: &str) -> Option<String> {
    let rel = path.trim_start_matches('/');
    let candidates = if rel.is_empty() || rel.ends_with('/') {
        vec![format!("{rel}index.html")]
    } else {
        vec![rel.to_owned(), format!("{rel}.html")]
    };
    candidates.into_iter().find(|c| site().join(c).is_file())
}

/// Check one link found in `from`. Returns a problem, if any.
fn check_link(from: &str, href: &str, ids: &BTreeMap<String, BTreeSet<String>>) -> Option<String> {
    let local = if let Some(rest) = href.strip_prefix(ORIGIN) {
        if rest.is_empty() {
            "/".to_owned()
        } else {
            rest.to_owned()
        }
    } else if (href.starts_with('/') && !href.starts_with("//")) || href.starts_with('#') {
        href.to_owned()
    } else if href.contains("://") || href.starts_with("mailto:") {
        return None;
    } else {
        return Some(format!("{from}: relative link {href}"));
    };
    let (path, fragment) = match local.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (local.as_str(), None),
    };
    let path = path.split('?').next().unwrap_or_default();
    let target = if path.is_empty() {
        from.to_owned()
    } else {
        match resolve(path) {
            Some(t) => t,
            None => return Some(format!("{from}: {href} does not resolve to a file")),
        }
    };
    match (fragment, ids.get(&target)) {
        (Some(""), _) => Some(format!("{from}: empty fragment in {href}")),
        (Some(id), Some(set)) if !set.contains(id) => {
            Some(format!("{from}: {href} has no id=\"{id}\" in {target}"))
        }
        // A fragment into a markdown or text file cannot be checked.
        _ => None,
    }
}

#[test]
fn every_internal_link_and_anchor_resolves() {
    let pages = pages();
    assert!(
        pages.contains_key("index.html") && pages.contains_key("404.html"),
        "{:?}",
        pages.keys()
    );
    let ids: BTreeMap<String, BTreeSet<String>> = pages
        .iter()
        .map(|(p, html)| (p.clone(), attrs(html, "id").into_iter().collect()))
        .collect();
    let mut problems = Vec::new();
    let mut checked = 0;
    for (page, html) in &pages {
        for href in attrs(html, "href").into_iter().chain(attrs(html, "src")) {
            checked += 1;
            problems.extend(check_link(page, &href, &ids));
        }
    }
    for name in TEXT_FILES {
        let text = fs::read_to_string(site().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        for (i, _) in text.match_indices(ORIGIN) {
            let url: String = text[i..]
                .chars()
                .take_while(|c| !c.is_whitespace() && !matches!(c, ')' | '<' | '"' | '|'))
                .collect();
            checked += 1;
            problems.extend(check_link(name, &url, &ids));
        }
    }
    let index = fs::read_to_string(site().join("search-index.json")).expect("search-index.json");
    let entries: serde_json::Value = serde_json::from_str(&index).expect("search index parses");
    for entry in entries.as_array().expect("array") {
        checked += 1;
        problems.extend(check_link(
            "search-index.json",
            entry["u"].as_str().expect("u"),
            &ids,
        ));
    }
    assert!(
        checked > 500,
        "only {checked} links found; is the site generated?"
    );
    assert!(
        problems.is_empty(),
        "{} broken links:\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn every_page_has_title_description_and_canonical() {
    for (page, html) in pages() {
        let title = html
            .split_once("<title>")
            .and_then(|(_, rest)| rest.split_once("</title>"))
            .map(|(t, _)| t.trim().to_owned());
        assert!(title.is_some_and(|t| !t.is_empty()), "{page}: no title");
        let description = html
            .split_once("<meta name=\"description\" content=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(d, _)| d.to_owned());
        assert!(
            description.is_some_and(|d| !d.is_empty()),
            "{page}: no description"
        );
        assert_eq!(
            html.matches("<link rel=\"canonical\" href=\"https://fidryn.onlygass.dev/")
                .count(),
            1,
            "{page}: needs exactly one canonical link"
        );
    }
}

#[test]
fn ids_are_unique_on_every_page() {
    // A heading slug that collides with a template id (`main`, `drawer`)
    // would break its anchor and the control that points at it.
    for (page, html) in pages() {
        let mut seen = BTreeSet::new();
        for id in attrs(&html, "id") {
            assert!(seen.insert(id.clone()), "{page}: duplicate id=\"{id}\"");
        }
    }
}

#[test]
fn no_template_slot_is_left_outside_code() {
    for (page, html) in pages() {
        // Guides may show `{{module}}` in code; only markup outside code counts.
        let mut rest = html.as_str();
        let mut outside = String::new();
        while let Some(start) = rest.find("<code") {
            outside.push_str(&rest[..start]);
            let end = rest[start..]
                .find("</code>")
                .map(|e| start + e + "</code>".len());
            rest = end.map_or("", |e| &rest[e..]);
        }
        outside.push_str(rest);
        assert!(!outside.contains("{{"), "{page}: unfilled template slot");
    }
}

#[test]
fn generator_check_passes_on_the_committed_site() {
    let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["site", "--check"])
        .output()
        .expect("spawn xtask site --check");
    assert!(
        out.status.success(),
        "xtask site --check failed; run `cargo xtask site`\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
