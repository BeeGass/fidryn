//! `web/index.html` is served under the mill's Content-Security-Policy
//! (`default-src 'self'`), so it may not carry inline script, inline style,
//! or handler attributes. It must also keep the strings the router tests
//! look for and every element id `web/mill.js` reads.

use std::collections::BTreeMap;
use std::path::PathBuf;

fn page() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/index.html");
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

/// One start tag: lowercase name and attributes in source order
/// (`None` for a bare attribute such as `hidden`).
struct Tag {
    name: String,
    attrs: Vec<(String, Option<String>)>,
}

impl Tag {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_deref().unwrap_or(""))
    }
}

/// Every start tag in `html`, skipping comments, end tags, and the doctype.
/// Quoted attribute values may hold spaces, `>`, and `=`.
fn start_tags(html: &str) -> Vec<Tag> {
    let bytes = html.as_bytes();
    let mut tags = Vec::new();
    let mut i = 0;
    while let Some(offset) = html[i..].find('<') {
        let at = i + offset;
        if html[at..].starts_with("<!--") {
            i = html[at..]
                .find("-->")
                .map_or(html.len(), |end| at + end + 3);
            continue;
        }
        if !bytes.get(at + 1).is_some_and(u8::is_ascii_alphabetic) {
            i = at + 1;
            continue;
        }
        let mut j = at + 1;
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
            j += 1;
        }
        let name = html[at + 1..j].to_ascii_lowercase();
        let mut attrs = Vec::new();
        loop {
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j >= bytes.len() {
                break;
            }
            if bytes[j] == b'>' {
                j += 1;
                break;
            }
            if bytes[j] == b'/' {
                j += 1;
                continue;
            }
            let key_start = j;
            while j < bytes.len()
                && !bytes[j].is_ascii_whitespace()
                && !matches!(bytes[j], b'=' | b'>' | b'/')
            {
                j += 1;
            }
            let key = html[key_start..j].to_ascii_lowercase();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let mut value = None;
            if j < bytes.len() && bytes[j] == b'=' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && matches!(bytes[j], b'"' | b'\'') {
                    let quote = bytes[j];
                    let value_start = j + 1;
                    j = value_start;
                    while j < bytes.len() && bytes[j] != quote {
                        j += 1;
                    }
                    value = Some(html[value_start..j].to_owned());
                    j += 1;
                } else {
                    let value_start = j;
                    while j < bytes.len() && !bytes[j].is_ascii_whitespace() && bytes[j] != b'>' {
                        j += 1;
                    }
                    value = Some(html[value_start..j].to_owned());
                }
            }
            attrs.push((key, value));
        }
        tags.push(Tag { name, attrs });
        i = j;
    }
    tags
}

#[test]
fn page_keeps_the_strings_the_mill_is_known_by() {
    let html = page();
    for needle in [
        "<title>fidryn mill</title>",
        "<h1 class=\"sr-only\">fidryn mill</h1>",
        "localhost 127.0.0.1",
        "Live filing is not available from the UI",
        ">Check<",
        ">Run<",
        ">Explore<",
        ">Render<",
    ] {
        assert!(html.contains(needle), "web/index.html lacks {needle:?}");
    }
}

#[test]
fn head_loads_the_shared_styles_the_favicon_and_mill_js() {
    let html = page();
    let head_end = html.find("</head>").expect("web/index.html has a </head>");
    for needle in [
        "<link rel=\"stylesheet\" href=\"/assets/fidryn.css\">",
        "<link rel=\"stylesheet\" href=\"/assets/mill.css\">",
        "<link rel=\"icon\" href=\"/favicon.svg\" type=\"image/svg+xml\">",
        "<script src=\"/assets/mill.js\"></script>",
    ] {
        let at = html
            .find(needle)
            .unwrap_or_else(|| panic!("web/index.html lacks {needle:?}"));
        assert!(at < head_end, "{needle:?} must be inside <head>");
    }
}

#[test]
fn page_has_no_inline_script_style_or_handlers() {
    let html = page();
    assert!(
        !html.contains("<style"),
        "inline <style> is blocked by the CSP"
    );
    for tag in start_tags(&html) {
        for (key, _) in &tag.attrs {
            assert_ne!(key, "style", "<{}> has a style attribute", tag.name);
            assert!(
                !key.starts_with("on"),
                "<{}> has a handler attribute {key}",
                tag.name
            );
        }
        if tag.name == "script" {
            assert!(tag.attr("src").is_some(), "every <script> must load a file");
        }
    }
    let mut rest = html.as_str();
    while let Some(at) = rest.find("<script") {
        let after = &rest[at..];
        let close = after.find('>').expect("<script> tag is closed");
        assert!(
            after[close + 1..].starts_with("</script>"),
            "a <script> element has an inline body"
        );
        rest = &after[close + 1..];
    }
}

#[test]
fn page_has_every_element_id_mill_js_reads_once() {
    let tags = start_tags(&page());
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    for tag in &tags {
        if let Some(id) = tag.attr("id") {
            *ids.entry(id.to_owned()).or_default() += 1;
        }
    }
    for (id, count) in &ids {
        assert_eq!(*count, 1, "id {id:?} appears {count} times");
    }
    for id in [
        "health",
        "samples",
        "confirm",
        "confirm-replace",
        "confirm-cancel",
        "history",
        "buffers",
        "buffer-status",
        "editor-wrap",
        "gutter",
        "hl",
        "editor",
        "diag-pop",
        "query",
        "query-names",
        "validAt",
        "validAt-err",
        "knownAt",
        "knownAt-err",
        "check",
        "run",
        "explore",
        "render",
        "views",
        "result-body",
    ] {
        assert!(ids.contains_key(id), "web/index.html lacks id {id:?}");
    }
    let editor = tags
        .iter()
        .find(|t| t.attr("id") == Some("editor"))
        .expect("#editor");
    assert_eq!(editor.name, "textarea");
    let names = tags
        .iter()
        .find(|t| t.attr("id") == Some("query-names"))
        .expect("#query-names");
    assert_eq!(names.name, "datalist");
}

#[test]
fn buffer_tabs_and_view_buttons_carry_their_data_attributes() {
    let tags = start_tags(&page());
    let buffers: Vec<&Tag> = tags
        .iter()
        .filter(|t| t.attr("data-buffer").is_some())
        .collect();
    let names: Vec<&str> = buffers
        .iter()
        .filter_map(|t| t.attr("data-buffer"))
        .collect();
    assert_eq!(names, ["module", "case", "template"]);
    for tab in &buffers {
        assert_eq!(tab.name, "button");
        assert_eq!(tab.attr("role"), Some("tab"));
        assert!(
            tab.attr("aria-selected").is_some(),
            "buffer tab without aria-selected"
        );
    }
    let views: Vec<&str> = tags.iter().filter_map(|t| t.attr("data-view")).collect();
    assert_eq!(views, ["opinion", "table", "json"]);
}

#[test]
fn page_reuses_the_shared_design_classes() {
    let tags = start_tags(&page());
    let classes: Vec<&str> = tags
        .iter()
        .filter_map(|t| t.attr("class"))
        .flat_map(str::split_whitespace)
        .collect();
    for class in [
        "site-head",
        "brand",
        "mono",
        "wordmark",
        "label",
        "btn",
        "pri",
        "sec",
        "sm",
        "actions",
        "theme-toggle",
        "skip",
        "sr-only",
    ] {
        assert!(
            classes.contains(&class),
            "web/index.html does not use .{class}"
        );
    }
    for class in &classes {
        let shared = [
            "site-head",
            "brand",
            "mono",
            "wordmark",
            "label",
            "btn",
            "pri",
            "sec",
            "sm",
            "actions",
            "theme-toggle",
            "skip",
            "sr-only",
            "page-mill",
        ];
        assert!(
            shared.contains(class) || class.starts_with("mill-"),
            "class {class:?} is neither shared nor prefixed mill-"
        );
    }
    let toggle = tags
        .iter()
        .find(|t| {
            t.attr("class")
                .is_some_and(|c| c.split_whitespace().any(|c| c == "theme-toggle"))
        })
        .expect(".theme-toggle");
    assert!(toggle.attr("data-theme-toggle").is_some());
    assert!(
        toggle.attr("hidden").is_some(),
        "the theme toggle ships hidden until mill.js runs"
    );
}
