//! Guide markdown to page HTML. The first `h1` becomes the title; `h2`–`h6`
//! get GitHub-style ids and a `#` anchor; `h2`/`h3` get section numbers and
//! "On this page" entries; tables scroll inside a wrapper; code is framed
//! and highlighted; links are rewritten for the site.

use super::highlight::{self, Keywords};
use super::html::esc;
use super::links::{self, LinkStyle};
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd, html,
};
use std::collections::HashMap;
use std::mem;

/// One "On this page" entry: an `h2` or `h3`. Text fields are plain text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TocEntry {
    /// 2 or 3.
    pub level: u8,
    /// `6.1` or `6.1.2`; empty when the heading is unnumbered.
    pub number: String,
    pub id: String,
    pub text: String,
}

/// A searchable part of a page: one per `h2`/`h3`. Text fields are plain text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub number: String,
    pub id: String,
    pub heading: String,
    /// The first 200 characters of the section's text.
    pub text: String,
}

/// A rendered guide. `body` is HTML; every other text field is plain text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// The first `h1`, which is not part of `body`.
    pub title: String,
    /// The first 200 characters of the text before the first `h2`/`h3`.
    pub lead: String,
    pub body: String,
    pub toc: Vec<TocEntry>,
    pub sections: Vec<Section>,
}

/// Opening tag of the region every table scrolls inside.
const TABLE_WRAP: &str =
    "<div class=\"table-wrap\" tabindex=\"0\" role=\"region\" aria-label=\"Table\">";

/// Length of `Page::lead` and `Section::text`, in characters.
const SUMMARY_CHARS: usize = 200;

/// Render one guide. `guide_number` numbers its `h2`/`h3` headings (`6.1`,
/// `6.1.2`); `None`, for the overview, leaves them unnumbered. An `h3`
/// before any `h2` is never numbered.
pub fn render(md: &str, guide_number: Option<u32>, kw: &Keywords) -> Page {
    let mut parser = Parser::new_ext(
        md,
        Options::ENABLE_TABLES | Options::ENABLE_HEADING_ATTRIBUTES,
    );
    let mut page = Outline {
        guide_number,
        ..Outline::default()
    };
    let mut out = Vec::new();
    while let Some(event) = parser.next() {
        match event {
            Event::Start(Tag::Heading { level, id, .. }) => {
                let inner: Vec<Event<'_>> = parser
                    .by_ref()
                    .take_while(|e| !matches!(e, Event::End(TagEnd::Heading(_))))
                    .collect();
                let text = collapse(&inline_text(&inner));
                if level == HeadingLevel::H1 && page.title.is_none() {
                    page.title = Some(text);
                } else {
                    let (id, number) = page.heading(level, id, text);
                    push_heading(&mut out, level, &id, &number, inner);
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let code = code_text(&mut parser);
                let info = match &kind {
                    CodeBlockKind::Fenced(info) => info.as_ref(),
                    CodeBlockKind::Indented => "",
                };
                let lang = highlight::sniff(info, &code, kw);
                let frame = highlight::code_frame(lang, &code, kw, None);
                out.push(Event::Html(format!("{frame}\n").into()));
                page.text.push_str(&code);
                page.text.push(' ');
            }
            Event::Start(Tag::Table(_)) => {
                out.push(Event::Html(TABLE_WRAP.into()));
                out.push(event);
            }
            Event::End(TagEnd::Table) => {
                out.push(event);
                out.push(Event::Html("</div>\n".into()));
            }
            event => {
                page.read(&event);
                out.push(site_link(event));
            }
        }
    }
    page.close_part();
    let mut body = String::with_capacity(md.len() * 2);
    html::push_html(&mut body, out.into_iter());
    Page {
        title: page.title.unwrap_or_default(),
        lead: page.lead,
        body,
        toc: page.toc,
        sections: page.sections,
    }
}

/// GitHub-style heading id: lowercase, each space becomes `-`, and every
/// character other than a letter, digit, `-`, or `_` is dropped.
pub fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// What `render` learns about a page besides its HTML.
#[derive(Default)]
struct Outline {
    guide_number: Option<u32>,
    title: Option<String>,
    ids: Ids,
    h2: u32,
    h3: u32,
    toc: Vec<TocEntry>,
    sections: Vec<Section>,
    lead: String,
    /// Plain text read since the last `h2`/`h3`, or since the start.
    text: String,
}

impl Outline {
    /// Give a heading its id and number. An `h2`/`h3` also starts a section
    /// and a TOC entry; a deeper heading's text belongs to the current part.
    fn heading(
        &mut self,
        level: HeadingLevel,
        explicit_id: Option<CowStr<'_>>,
        text: String,
    ) -> (String, String) {
        let base = explicit_id.map_or_else(|| slug(&text), CowStr::into_string);
        let id = self.ids.unique(if base.is_empty() {
            "section".to_owned()
        } else {
            base
        });
        let number = match level {
            HeadingLevel::H2 => {
                self.h2 += 1;
                self.h3 = 0;
                self.guide_number.map(|n| format!("{n}.{}", self.h2))
            }
            HeadingLevel::H3 if self.h2 > 0 => {
                self.h3 += 1;
                self.guide_number
                    .map(|n| format!("{n}.{}.{}", self.h2, self.h3))
            }
            _ => None,
        }
        .unwrap_or_default();
        if matches!(level, HeadingLevel::H2 | HeadingLevel::H3) {
            self.close_part();
            self.toc.push(TocEntry {
                level: level as u8,
                number: number.clone(),
                id: id.clone(),
                text: text.clone(),
            });
            self.sections.push(Section {
                number: number.clone(),
                id: id.clone(),
                heading: text,
                text: String::new(),
            });
        } else {
            self.text.push_str(&text);
            self.text.push(' ');
        }
        (id, number)
    }

    /// Keep the text a reader sees, with a space wherever a block or line ends.
    fn read(&mut self, event: &Event<'_>) {
        match event {
            Event::Text(chunk) | Event::Code(chunk) => self.text.push_str(chunk),
            Event::SoftBreak
            | Event::HardBreak
            | Event::Rule
            | Event::Start(Tag::Item | Tag::List(_))
            | Event::End(
                TagEnd::Paragraph | TagEnd::Item | TagEnd::TableCell | TagEnd::BlockQuote(_),
            ) => self.text.push(' '),
            _ => {}
        }
    }

    /// The text read so far becomes the lead, or the last section's text.
    fn close_part(&mut self) {
        let done = summary(&mem::take(&mut self.text));
        match self.sections.last_mut() {
            Some(section) => section.text = done,
            None => self.lead = done,
        }
    }
}

/// Heading ids handed out so far, de-duplicated the way GitHub does it:
/// the second `x` becomes `x-1`, the third `x-2`.
#[derive(Default)]
struct Ids(HashMap<String, usize>);

impl Ids {
    fn unique(&mut self, base: String) -> String {
        let mut id = base.clone();
        while self.0.contains_key(&id) {
            let count = self.0.get_mut(&base).expect("the base id is taken first");
            *count += 1;
            id = format!("{base}-{count}");
        }
        self.0.insert(id.clone(), 0);
        id
    }
}

/// `<h2 id="…"><span class="hn">6.1</span> Text <a class="anchor" …>#</a></h2>`.
fn push_heading<'a>(
    out: &mut Vec<Event<'a>>,
    level: HeadingLevel,
    id: &str,
    number: &str,
    inner: Vec<Event<'a>>,
) {
    let id = esc(id);
    let mut open = format!("<{level} id=\"{id}\">");
    if !number.is_empty() {
        open.push_str(&format!("<span class=\"hn\">{number}</span> "));
    }
    out.push(Event::Html(open.into()));
    out.extend(inner.into_iter().map(site_link));
    out.push(Event::Html(
        format!(
            " <a class=\"anchor\" href=\"#{id}\" aria-label=\"Link to this section\">#</a></{level}>\n"
        )
        .into(),
    ));
}

/// The text of the code block whose start tag was just read.
fn code_text(parser: &mut Parser<'_>) -> String {
    let mut code = String::new();
    for event in parser.by_ref() {
        match event {
            Event::Text(chunk) => code.push_str(&chunk),
            Event::End(TagEnd::CodeBlock) => break,
            _ => {}
        }
    }
    code
}

/// A link event with its target rewritten for the site.
fn site_link(event: Event<'_>) -> Event<'_> {
    match event {
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) if link_type != LinkType::Email => {
            let dest_url = links::rewrite(&dest_url, LinkStyle::Html).into();
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            })
        }
        other => other,
    }
}

/// The plain text of inline events, such as a heading's content.
fn inline_text(events: &[Event<'_>]) -> String {
    let mut text = String::new();
    for event in events {
        match event {
            Event::Text(chunk) | Event::Code(chunk) => text.push_str(chunk),
            Event::SoftBreak | Event::HardBreak => text.push(' '),
            _ => {}
        }
    }
    text
}

/// `text` with every run of whitespace turned into one space, trimmed.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first 200 characters of `text` after collapsing whitespace, cut at a
/// character boundary, without a trailing space.
fn summary(text: &str) -> String {
    let collapsed = collapse(text);
    match collapsed.char_indices().nth(SUMMARY_CHARS) {
        Some((cut, _)) => collapsed[..cut].trim_end().to_owned(),
        None => collapsed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::guides::GUIDES;
    use crate::workspace::workspace_root;
    use std::collections::HashSet;
    use std::fs;

    fn keywords() -> Keywords {
        Keywords::load(&workspace_root()).expect("read grammar.ebnf")
    }

    fn page(md: &str, guide_number: Option<u32>) -> Page {
        render(md, guide_number, &keywords())
    }

    /// Every value of `attr="…"` in `html`, in order.
    fn attrs<'a>(html: &'a str, attr: &str) -> Vec<&'a str> {
        html.split(&format!(" {attr}=\""))
            .skip(1)
            .map(|rest| &rest[..rest.find('"').expect("closing quote")])
            .collect()
    }

    #[test]
    fn the_first_h1_is_the_title_and_not_in_the_body() {
        let p = page(
            "# Outcomes\n\nThis guide explains.\n\n## Envelope\n\nText.\n",
            Some(6),
        );
        assert_eq!(p.title, "Outcomes");
        assert!(
            !p.body.contains("<h1") && !p.body.contains("Outcomes"),
            "{}",
            p.body
        );
        assert!(
            p.body
                .starts_with("<p>This guide explains.</p>\n<h2 id=\"envelope\">"),
            "{}",
            p.body
        );
    }

    #[test]
    fn numbered_guides_number_h2_and_h3_and_anchor_every_heading() {
        let p = page(
            "# T\n\n## Envelope\n\n### Fields\n\n#### Detail\n\n## Outcome kinds\n\n### Determinate\n",
            Some(6),
        );
        let body = &p.body;
        for expected in [
            "<h2 id=\"envelope\"><span class=\"hn\">6.1</span> Envelope <a class=\"anchor\" href=\"#envelope\" aria-label=\"Link to this section\">#</a></h2>\n",
            "<h3 id=\"fields\"><span class=\"hn\">6.1.1</span> Fields <a class=\"anchor\" href=\"#fields\" aria-label=\"Link to this section\">#</a></h3>\n",
            "<h4 id=\"detail\">Detail <a class=\"anchor\" href=\"#detail\" aria-label=\"Link to this section\">#</a></h4>\n",
            "<h2 id=\"outcome-kinds\"><span class=\"hn\">6.2</span> Outcome kinds <a class=\"anchor\"",
            "<h3 id=\"determinate\"><span class=\"hn\">6.2.1</span> Determinate <a class=\"anchor\"",
        ] {
            assert!(body.contains(expected), "missing {expected} in {body}");
        }
    }

    #[test]
    fn the_overview_is_unnumbered() {
        let p = page(
            "# Fidryn documentation\n\n## For users\n\n### Guides\n",
            None,
        );
        assert!(p.body.contains(
            "<h2 id=\"for-users\">For users <a class=\"anchor\" href=\"#for-users\" aria-label=\"Link to this section\">#</a></h2>"
        ));
        assert!(
            p.body
                .contains("<h3 id=\"guides\">Guides <a class=\"anchor\"")
        );
        assert!(!p.body.contains("class=\"hn\""));
        assert!(p.toc.iter().all(|entry| entry.number.is_empty()));
        assert!(p.sections.iter().all(|section| section.number.is_empty()));
    }

    #[test]
    fn an_h3_before_any_h2_is_unnumbered() {
        let p = page("# T\n\n### Early\n\n## First\n\n### Later\n", Some(2));
        let numbers: Vec<(&str, &str)> = p
            .toc
            .iter()
            .map(|e| (e.id.as_str(), e.number.as_str()))
            .collect();
        assert_eq!(
            numbers,
            [("early", ""), ("first", "2.1"), ("later", "2.1.1")]
        );
        assert!(
            p.body
                .contains("<h3 id=\"early\">Early <a class=\"anchor\""),
            "{}",
            p.body
        );
    }

    #[test]
    fn toc_lists_h2_and_h3_in_order() {
        let p = page(
            "# T\n\n## Envelope\n\n### Fields\n\n#### Deep\n\n## Kinds\n",
            Some(6),
        );
        let entry = |level, number: &str, id: &str, text: &str| TocEntry {
            level,
            number: number.to_owned(),
            id: id.to_owned(),
            text: text.to_owned(),
        };
        assert_eq!(
            p.toc,
            [
                entry(2, "6.1", "envelope", "Envelope"),
                entry(3, "6.1.1", "fields", "Fields"),
                entry(2, "6.2", "kinds", "Kinds"),
            ]
        );
    }

    #[test]
    fn slugs_follow_github() {
        for (text, id) in [
            (
                "Surface inventory (status-qualified)",
                "surface-inventory-status-qualified",
            ),
            (
                "POST /api/run and POST /api/explore",
                "post-apirun-and-post-apiexplore",
            ),
            (
                "Delaware, Illinois, Massachusetts, New York, Texas, UCC, everyday",
                "delaware-illinois-massachusetts-new-york-texas-ucc-everyday",
            ),
            (
                "Annotated example: court selects I2",
                "annotated-example-court-selects-i2",
            ),
            ("NormConflict", "normconflict"),
            ("snake_case and kebab-case", "snake_case-and-kebab-case"),
            ("Café § 4 — naïve", "café--4--naïve"),
        ] {
            assert_eq!(slug(text), id, "{text}");
        }
    }

    #[test]
    fn duplicate_headings_get_numbered_suffixes() {
        let p = page(
            "# T\n\n## Fields\n\n## Fields\n\n## Fields 1\n\n## Fields\n\n## §\n",
            None,
        );
        let ids: Vec<&str> = p.toc.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            ["fields", "fields-1", "fields-1-1", "fields-2", "section"]
        );
    }

    #[test]
    fn inline_code_in_a_heading_keeps_its_markup_and_a_plain_id() {
        let p = page("# CLI\n\n## `run`\n", Some(4));
        assert!(
            p.body.contains(
                "<h2 id=\"run\"><span class=\"hn\">4.1</span> <code>run</code> <a class=\"anchor\" href=\"#run\" aria-label=\"Link to this section\">#</a></h2>"
            ),
            "{}",
            p.body
        );
        assert_eq!(p.toc[0].text, "run");
        assert_eq!(p.sections[0].heading, "run");
    }

    #[test]
    fn an_explicit_heading_id_replaces_the_slug() {
        let p = page("# T\n\n## Envelope {#env}\n", Some(6));
        assert_eq!(p.toc[0].id, "env");
        assert_eq!(p.toc[0].text, "Envelope");
    }

    #[test]
    fn every_table_is_wrapped_in_a_scrolling_region() {
        let md = "# T\n\nIntro.\n\n| Field | Meaning |\n| --- | --- |\n| `schema` | a long cell |\n\n- item\n\n  | a | b |\n  | - | - |\n  | 1 | 2 |\n";
        let body = page(md, Some(7)).body;
        assert_eq!(body.matches("<table>").count(), 2, "{body}");
        assert_eq!(
            body.matches(&format!("{TABLE_WRAP}<table>")).count(),
            2,
            "{body}"
        );
        assert_eq!(body.matches("</table>\n</div>\n").count(), 2, "{body}");
    }

    #[test]
    fn fenced_and_indented_code_become_highlighted_frames() {
        let md = "# T\n\n```json\n{\"a\": 1}\n```\n\n```\ncargo test --offline\n```\n\n```rust\nfn main() {}\n```\n\nIndented:\n\n    module X version \"1\" {}\n";
        let body = page(md, None).body;
        for expected in [
            "<figure class=\"code\" data-lang=\"json\"><figcaption><span>json</span>",
            "<pre><code><span class=\"tk-pu\">{</span><span class=\"tk-ty\">&quot;a&quot;</span>",
            "<figure class=\"code\" data-lang=\"shell\"><figcaption><span>shell</span>",
            "<span class=\"tk-kw\">cargo</span>",
            "<figure class=\"code\" data-lang=\"text\"><figcaption><span>text</span><button type=\"button\" class=\"copy\" data-copy hidden>Copy</button></figcaption><pre><code>fn main() {}</code></pre></figure>\n",
            "<figure class=\"code\" data-lang=\"fr\"><figcaption><span>.fr</span>",
            "<span class=\"tk-kw\">module</span>",
        ] {
            assert!(body.contains(expected), "missing {expected} in {body}");
        }
        assert_eq!(body.matches("<pre").count(), 4);
        assert_eq!(body.matches("<figure class=\"code\"").count(), 4);
    }

    #[test]
    fn links_are_rewritten_for_the_site() {
        let md = "# T\n\n[CLI](cli.md#file), [docs](README.md), [arch](ARCHITECTURE.md), [grammar](../grammar.ebnf), [programs](../tests/programs/), [web](https://example.com/x), [here](#here), <https://fidryn.onlygass.dev/>.\n\n## See [Outcomes](outcomes.md)\n";
        let body = page(md, None).body;
        assert_eq!(
            attrs(&body, "href"),
            [
                "/docs/cli#file",
                "/docs/",
                "https://github.com/BeeGass/fidryn/blob/main/docs/ARCHITECTURE.md",
                "https://github.com/BeeGass/fidryn/blob/main/grammar.ebnf",
                "https://github.com/BeeGass/fidryn/tree/main/tests/programs/",
                "https://example.com/x",
                "#here",
                "https://fidryn.onlygass.dev/",
                "/docs/outcomes",
                "#see-outcomes",
            ]
        );
    }

    #[test]
    fn blockquotes_stay_blockquotes() {
        let body = page("# T\n\n> **What it is not.** A court order.\n", None).body;
        assert_eq!(
            body,
            "<blockquote>\n<p><strong>What it is not.</strong> A court order.</p>\n</blockquote>\n"
        );
    }

    #[test]
    fn lead_and_section_text_are_collapsed_plain_text() {
        let md = "# T\n\nThe `outcome`   is\ntagged by **kind**.\n\n## One\n\nFirst  line\nsecond line.\n\n#### Deep\n\n```\ncargo test\n```\n\n| a | b |\n| - | - |\n| c | d |\n\n## Two\n";
        let p = page(md, Some(6));
        assert_eq!(p.lead, "The outcome is tagged by kind.");
        assert_eq!(p.sections[0].heading, "One");
        assert_eq!(p.sections[0].number, "6.1");
        assert_eq!(
            p.sections[0].text,
            "First line second line. Deep cargo test a b c d"
        );
        assert_eq!(p.sections[1].text, "");
    }

    #[test]
    fn summaries_stop_at_200_characters_on_a_char_boundary() {
        let long = format!("{} {}", "é".repeat(150), "x".repeat(100));
        let p = page(&format!("# T\n\n{long}\n"), None);
        assert_eq!(p.lead.chars().count(), 200);
        assert_eq!(p.lead, format!("{} {}", "é".repeat(150), "x".repeat(49)));
        let p = page(&format!("# T\n\n{} tail\n", "a".repeat(199)), None);
        assert_eq!(p.lead, "a".repeat(199), "no trailing space");
    }

    #[test]
    fn every_guide_renders_with_unique_ids_and_resolving_links() {
        let kw = keywords();
        let docs = workspace_root().join("docs");
        for guide in GUIDES {
            let md = fs::read_to_string(docs.join(guide.file)).expect("read guide");
            let p = render(&md, guide.number, &kw);
            let file = guide.file;
            assert_eq!(
                md.lines().next(),
                Some(format!("# {}", p.title).as_str()),
                "{file} must open with its # title"
            );
            assert!(!p.lead.is_empty(), "{file} has no lead");
            let ids = attrs(&p.body, "id");
            let unique: HashSet<&str> = ids.iter().copied().collect();
            assert_eq!(unique.len(), ids.len(), "{file} repeats an id");
            assert_eq!(p.sections.len(), p.toc.len());
            for entry in &p.toc {
                assert!(unique.contains(entry.id.as_str()), "{file}: {}", entry.id);
            }
            for href in attrs(&p.body, "href") {
                match href.strip_prefix('#') {
                    Some(anchor) => assert!(unique.contains(anchor), "{file}: #{anchor}"),
                    None => assert!(
                        href.starts_with('/')
                            || href.starts_with("https://")
                            || href.starts_with("http://"),
                        "{file}: relative link {href}"
                    ),
                }
            }
            let tables = p.body.matches("<table>").count();
            assert_eq!(
                p.body.matches(&format!("{TABLE_WRAP}<table>")).count(),
                tables,
                "{file}"
            );
            assert_eq!(
                p.body.matches("</table>\n</div>\n").count(),
                tables,
                "{file}"
            );
            let frames = p.body.matches("<figure class=\"code\"").count();
            assert_eq!(p.body.matches("<pre").count(), frames, "{file}");
        }
    }

    #[test]
    fn the_outcomes_guide_has_the_anchors_the_landing_page_links_to() {
        let md = fs::read_to_string(workspace_root().join("docs/outcomes.md")).expect("read guide");
        let body = render(&md, Some(6), &keywords()).body;
        let ids = attrs(&body, "id");
        for id in [
            "determinate",
            "contingent",
            "suspended",
            "normconflict",
            "outsidecompetence",
            "inconsistent",
        ] {
            assert!(ids.contains(&id), "missing #{id}");
        }
    }
}
