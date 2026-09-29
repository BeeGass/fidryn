//! `search-index.json`: one entry per guide page, then one per `h2`/`h3`
//! section, in guide order. The site script fetches it on first focus of the
//! search field.

use super::guides::{self, Guide};
use super::markdown::Page;
use serde_json::{Value, json};

/// Longest snippet, in characters.
const SNIPPET_CHARS: usize = 300;

/// The search index as compact JSON with a final newline.
pub fn index(pages: &[(&Guide, &Page)]) -> String {
    let mut entries = Vec::new();
    for (guide, page) in pages {
        let url = guides::url(guide.slug);
        entries.push(json!({
            "u": url,
            "n": guide.number.map(|n| format!("§{n}")).unwrap_or_default(),
            "h": page.title,
            "p": guide.title,
            "t": snippet(&page.lead),
        }));
        for section in &page.sections {
            entries.push(json!({
                "u": format!("{url}#{}", section.id),
                "n": section.number,
                "h": section.heading,
                "p": guide.title,
                "t": snippet(&section.text),
            }));
        }
    }
    let mut out = Value::Array(entries).to_string();
    out.push('\n');
    out
}

/// The first 300 characters of `text` with whitespace collapsed, cut at a
/// character boundary, with no trailing space.
fn snippet(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = collapsed.chars().take(SNIPPET_CHARS).collect();
    cut.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::highlight::Keywords;
    use crate::site::markdown::{self, Section};
    use crate::workspace::workspace_root;
    use std::fs;

    fn guide(slug: &str) -> &'static Guide {
        guides::GUIDES
            .iter()
            .find(|g| g.slug == slug)
            .expect("guide")
    }

    fn entries(json: &str) -> Vec<serde_json::Map<String, Value>> {
        let value: Value = serde_json::from_str(json).expect("index parses");
        value
            .as_array()
            .expect("array")
            .iter()
            .map(|e| e.as_object().expect("object").clone())
            .collect()
    }

    #[test]
    fn page_then_sections_in_order_with_numbers() {
        let page = Page {
            title: "Outcomes".to_owned(),
            lead: "This guide explains\n  the six honest results.".to_owned(),
            body: String::new(),
            toc: Vec::new(),
            sections: vec![
                Section {
                    number: "6.3".to_owned(),
                    id: "outcome-kinds".to_owned(),
                    heading: "Outcome kinds".to_owned(),
                    text: "outcome is tagged by kind.".to_owned(),
                },
                Section {
                    number: "6.3.1".to_owned(),
                    id: "determinate".to_owned(),
                    heading: "Determinate".to_owned(),
                    text: "One answer.".to_owned(),
                },
            ],
        };
        let overview = Page {
            title: "Fidryn documentation".to_owned(),
            lead: "Start here.".to_owned(),
            body: String::new(),
            toc: Vec::new(),
            sections: vec![Section {
                number: String::new(),
                id: "for-users".to_owned(),
                heading: "For users".to_owned(),
                text: "Guides.".to_owned(),
            }],
        };
        let json = index(&[(guide("index"), &overview), (guide("outcomes"), &page)]);
        assert!(
            json.ends_with("]\n") && !json.contains("\n "),
            "compact: {json}"
        );
        assert_eq!(
            json,
            concat!(
                r#"[{"u":"/docs/","n":"","h":"Fidryn documentation","p":"Documentation","t":"Start here."},"#,
                r##"{"u":"/docs/#for-users","n":"","h":"For users","p":"Documentation","t":"Guides."},"##,
                r#"{"u":"/docs/outcomes","n":"§6","h":"Outcomes","p":"Outcomes","t":"This guide explains the six honest results."},"#,
                r##"{"u":"/docs/outcomes#outcome-kinds","n":"6.3","h":"Outcome kinds","p":"Outcomes","t":"outcome is tagged by kind."},"##,
                r##"{"u":"/docs/outcomes#determinate","n":"6.3.1","h":"Determinate","p":"Outcomes","t":"One answer."}]"##,
                "\n"
            )
        );
    }

    #[test]
    fn snippets_are_collapsed_and_cut_at_300_characters() {
        let long = "§ word ".repeat(60);
        let cut = snippet(&long);
        assert!(cut.chars().count() <= 300, "{}", cut.chars().count());
        assert!(!cut.ends_with(' ') && !cut.contains("  "));
        assert!(long.starts_with(&cut));
        assert_eq!(snippet("  a \n\t b  "), "a b");
        assert_eq!(snippet(&"é".repeat(350)), "é".repeat(300));
    }

    #[test]
    fn real_guides_index_every_section_with_short_snippets() {
        let root = workspace_root();
        let kw = Keywords::load(&root).unwrap();
        let rendered: Vec<(&Guide, Page)> = guides::GUIDES
            .iter()
            .map(|g| {
                let md = fs::read_to_string(root.join("docs").join(g.file)).unwrap();
                (g, markdown::render(&md, g.number, &kw))
            })
            .collect();
        let pages: Vec<(&Guide, &Page)> = rendered.iter().map(|(g, p)| (*g, p)).collect();
        let entries = entries(&index(&pages));
        let sections: usize = rendered.iter().map(|(_, p)| p.sections.len()).sum();
        assert_eq!(entries.len(), guides::GUIDES.len() + sections);
        let kinds = entries
            .iter()
            .find(|e| e["u"] == "/docs/outcomes#outcome-kinds")
            .expect("outcome kinds section");
        assert_eq!(kinds["n"], "6.3");
        assert_eq!(kinds["h"], "Outcome kinds");
        assert_eq!(kinds["p"], "Outcomes");
        let page = entries
            .iter()
            .find(|e| e["u"] == "/docs/outcomes")
            .expect("outcomes page");
        assert_eq!(page["n"], "§6");
        for entry in &entries {
            let t = entry["t"].as_str().unwrap();
            assert!(t.chars().count() <= 300, "{t}");
            assert!(!t.ends_with(' ') && !t.contains('\n'), "{t:?}");
            assert_eq!(entry.len(), 5, "{entry:?}");
        }
    }
}
