//! Page assembly: the landing page, the docs pages, and the 404 page, each
//! a filled `templates/base.html`.

use super::guides::{self, GROUPS, GUIDES, Guide, IMPLEMENTER_DOCS, REPO};
use super::html::{asset_version, esc};
use super::markdown::{Page, TocEntry};
use super::seo::{self, LANDING_DESCRIPTION, LANDING_TITLE, PageKind};
use super::specimen::Run;
use super::templates::fill;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const BASE: &str = include_str!("templates/base.html");
const LANDING: &str = include_str!("templates/landing.html");
const DOC: &str = include_str!("templates/doc.html");
const NOT_FOUND: &str = include_str!("templates/404.html");

const NOT_FOUND_TITLE: &str = "Not found — Fidryn";
const NOT_FOUND_DESCRIPTION: &str = "This page is outside the declared model.";
const CURRENT: &str = " aria-current=\"page\"";

/// Content versions of the hand-written assets, used as `?v=` so the
/// stylesheet and script can be cached for a year.
pub struct Assets {
    /// `asset_version` of `site/assets/fidryn.css`.
    pub css: String,
    /// `asset_version` of `site/assets/fidryn.js`.
    pub js: String,
}

impl Assets {
    /// Hash `site/assets/fidryn.css` and `site/assets/fidryn.js` under `root`.
    pub fn read(root: &Path) -> Result<Self> {
        let version = |name: &str| -> Result<String> {
            let path = root.join("site/assets").join(name);
            let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            Ok(asset_version(&bytes))
        };
        Ok(Self {
            css: version("fidryn.css")?,
            js: version("fidryn.js")?,
        })
    }
}

/// Stamp class and label for an outcome kind as a report spells it.
pub fn stamp(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "determinate" => ("det", "Determinate"),
        "contingent" => ("con", "Contingent"),
        "suspended" => ("sus", "Suspended"),
        "normConflict" => ("nc", "NormConflict"),
        "outsideCompetence" => ("oc", "OutsideCompetence"),
        "inconsistent" => ("inc", "Inconsistent"),
        _ => ("", "Outcome"),
    }
}

fn stamp_html(kind: &str) -> String {
    match stamp(kind) {
        ("", label) => format!("<span class=\"stamp\">{label}</span>"),
        (class, label) => format!("<span class=\"stamp {class}\">{label}</span>"),
    }
}

/// Navigation name of a guide: the overview is "Overview", the rest their title.
fn nav_title(guide: &Guide) -> &'static str {
    if guide.number.is_none() {
        "Overview"
    } else {
        guide.title
    }
}

/// The guides list: the docs sidebar at 720px and up, the drawer on phones.
/// `current` is the slug of the page being rendered, if it is a guide.
pub fn guides_nav(current: Option<&str>) -> String {
    let mut out = String::from(concat!(
        "<nav id=\"drawer\" class=\"guides\" aria-label=\"Guides\">\n",
        "  <div class=\"guides-head\"><p class=\"label\">Contents</p>",
        "<button type=\"button\" class=\"btn sec sm\" data-drawer-close>Close</button></div>\n",
    ));
    for group in GROUPS {
        out.push_str(&format!(
            "  <div class=\"group\">\n    <p class=\"label\">{}</p>\n    <ol>\n",
            esc(group)
        ));
        let members = GUIDES
            .iter()
            .filter(|g| g.group == *group || (*group == GROUPS[0] && g.number.is_none()));
        for g in members {
            let number = g.number.map(|n| format!("&sect;{n}")).unwrap_or_default();
            let aria = if current == Some(g.slug) { CURRENT } else { "" };
            out.push_str(&format!(
                "      <li><a href=\"{}\"{aria}><span class=\"n\">{number}</span><span class=\"t\">{}</span><span class=\"d\">{}</span></a></li>\n",
                guides::url(g.slug),
                esc(nav_title(g)),
                esc(g.blurb),
            ));
        }
        out.push_str("    </ol>\n  </div>\n");
    }
    out.push_str("  <div class=\"group\">\n    <p class=\"label\">Implementers</p>\n    <ol>\n");
    for (title, file, blurb) in IMPLEMENTER_DOCS {
        out.push_str(&format!(
            "      <li><a href=\"{REPO}/blob/main/docs/{file}\"><span class=\"n\" aria-hidden=\"true\">&#8599;</span><span class=\"t\">{}</span><span class=\"d\">{}</span></a></li>\n",
            esc(title),
            esc(blurb),
        ));
    }
    out.push_str("    </ol>\n  </div>\n</nav>");
    out
}

/// The landing page, with the specimen built from `runs`.
pub fn landing(runs: &[Run], assets: &Assets) -> String {
    let main = fill(
        LANDING,
        &[
            ("guides_nav", &guides_nav(None)),
            ("specimen", &specimen(runs)),
            ("contents", &contents()),
        ],
    );
    shell(
        &Shell {
            title: LANDING_TITLE,
            description: LANDING_DESCRIPTION,
            path: "/",
            markdown_path: Some("/index.md"),
            kind: PageKind::Landing,
            body_class: "page-landing",
            crumb: String::new(),
            current_nav: None,
        },
        &main,
        assets,
    )
}

/// A docs page for `guide`, rendered from its markdown `page`.
pub fn doc(guide: &Guide, page: &Page, assets: &Assets) -> String {
    let toc = toc_items(&page.toc);
    let (onpage_inline, onpage) = if page.toc.is_empty() {
        (String::new(), String::new())
    } else {
        (
            format!(
                "<details class=\"onpage-inline\"><summary>On this page</summary><ol>{toc}</ol></details>"
            ),
            format!(
                "<aside class=\"onpage\" aria-label=\"On this page\"><p class=\"label\">On this page</p><ol>{toc}</ol></aside>"
            ),
        )
    };
    let main = fill(
        DOC,
        &[
            ("guides_nav", &guides_nav(Some(guide.slug))),
            ("kicker", &kicker(guide)),
            ("title", &esc(&page.title)),
            ("onpage_inline", &onpage_inline),
            ("body", &page.body),
            ("pager", &pager(guide.slug)),
            ("edit_url", &format!("{REPO}/blob/main/docs/{}", guide.file)),
            ("md_url", &format!("/docs/{}.md", guide.slug)),
            ("onpage", &onpage),
        ],
    );
    let title = format!("{} — Fidryn", guide.title);
    let path = guides::url(guide.slug);
    let markdown_path = format!("/docs/{}.md", guide.slug);
    shell(
        &Shell {
            title: &title,
            description: guide.description,
            path: &path,
            markdown_path: Some(&markdown_path),
            kind: PageKind::Doc,
            body_class: "page-doc",
            crumb: format!(
                "<p class=\"crumb\"><a href=\"/docs/\">Docs</a> <span aria-hidden=\"true\">/</span> {}</p>",
                esc(nav_title(guide))
            ),
            current_nav: Some(if guide.slug == "examples" {
                Nav::Examples
            } else {
                Nav::Docs
            }),
        },
        &main,
        assets,
    )
}

/// The 404 page.
pub fn not_found(assets: &Assets) -> String {
    let main = fill(NOT_FOUND, &[("guides_nav", &guides_nav(None))]);
    shell(
        &Shell {
            title: NOT_FOUND_TITLE,
            description: NOT_FOUND_DESCRIPTION,
            path: "/404",
            markdown_path: None,
            kind: PageKind::NotFound,
            body_class: "page-404",
            crumb: String::new(),
            current_nav: None,
        },
        &main,
        assets,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Docs,
    Examples,
}

/// Everything `base.html` needs besides the main content and the assets.
struct Shell<'a> {
    /// Plain text; escaped when filled.
    title: &'a str,
    /// Plain text; escaped when filled.
    description: &'a str,
    path: &'a str,
    markdown_path: Option<&'a str>,
    kind: PageKind,
    body_class: &'a str,
    /// Markup for the header breadcrumb, empty for none.
    crumb: String,
    current_nav: Option<Nav>,
}

fn shell(page: &Shell<'_>, main: &str, assets: &Assets) -> String {
    let head = seo::head(
        page.title,
        page.description,
        page.path,
        page.markdown_path,
        page.kind,
    );
    let mark = |nav: Nav| {
        if page.current_nav == Some(nav) {
            CURRENT
        } else {
            ""
        }
    };
    fill(
        BASE,
        &[
            ("title", &esc(page.title)),
            ("description", &esc(page.description)),
            ("head", &head),
            ("css_v", &assets.css),
            ("js_v", &assets.js),
            ("body_class", page.body_class),
            ("crumb", &page.crumb),
            ("nav_docs", mark(Nav::Docs)),
            ("nav_examples", mark(Nav::Examples)),
            ("main", main.trim_end()),
        ],
    )
}

/// `§ 6 · Read results` for a numbered guide, `Documentation` for the overview.
fn kicker(guide: &Guide) -> String {
    match guide.number {
        Some(n) => format!("&sect; {n} &middot; {}", esc(guide.group)),
        None => "Documentation".to_owned(),
    }
}

fn toc_items(toc: &[TocEntry]) -> String {
    toc.iter()
        .map(|entry| {
            let number = if entry.number.is_empty() {
                String::new()
            } else {
                format!("<span class=\"n\">{}</span> ", esc(&entry.number))
            };
            format!(
                "<li class=\"lvl-{}\"><a href=\"#{}\">{number}{}</a></li>",
                entry.level,
                esc(&entry.id),
                esc(&entry.text)
            )
        })
        .collect()
}

/// Previous and next guide in reading order: the overview, then §1 to §8.
fn pager(slug: &str) -> String {
    let Some(at) = GUIDES.iter().position(|g| g.slug == slug) else {
        return String::new();
    };
    let link = |class: &str, word: &str, g: &Guide| {
        let label = match g.number {
            Some(n) => format!("{word} &middot; &sect;{n}"),
            None => word.to_owned(),
        };
        format!(
            "<a class=\"{class}\" href=\"{}\"><span class=\"label\">{label}</span><span class=\"t\">{}</span></a>",
            guides::url(g.slug),
            esc(nav_title(g))
        )
    };
    let mut out = String::from("<nav class=\"pager\" aria-label=\"Previous and next guide\">");
    if let Some(prev) = at.checked_sub(1).map(|i| &GUIDES[i]) {
        out.push_str(&link("prev", "Previous", prev));
    }
    if let Some(next) = GUIDES.get(at + 1) {
        out.push_str(&link("next", "Next", next));
    }
    out.push_str("</nav>");
    out
}

/// The landing contents: every numbered guide in reading order.
fn contents() -> String {
    let mut out = String::from("<ol class=\"contents\">\n");
    for g in GUIDES {
        let Some(n) = g.number else { continue };
        out.push_str(&format!(
            "  <li><a href=\"{}\"><span class=\"n\">&sect;{n}</span><span class=\"t\">{}</span><span class=\"lead\" aria-hidden=\"true\"></span><span class=\"d\">{}</span></a></li>\n",
            guides::url(g.slug),
            esc(g.title),
            esc(g.blurb),
        ));
    }
    out.push_str("</ol>");
    out
}

/// The specimen: a tab per run, a panel per run with its three steps, and the
/// dot row the phone layout uses. Without JavaScript every panel shows.
fn specimen(runs: &[Run]) -> String {
    let mut out = String::from(concat!(
        "<div class=\"specimen\" data-specimen>\n",
        "  <div class=\"spec-tabs\" role=\"tablist\" aria-label=\"Real runs\">\n",
    ));
    for (i, run) in runs.iter().enumerate() {
        let (selected, tabindex) = if i == 0 {
            ("true", "")
        } else {
            ("false", " tabindex=\"-1\"")
        };
        out.push_str(&format!(
            "    <button type=\"button\" role=\"tab\" id=\"tab-{id}\" aria-controls=\"run-{id}\" aria-selected=\"{selected}\"{tabindex}>{} {}</button>\n",
            esc(run.label),
            stamp_html(&run.kind),
            id = run.id,
        ));
    }
    out.push_str("  </div>\n  <div class=\"spec-panels\">\n");
    for run in runs {
        out.push_str(&format!(
            "    <section class=\"spec-panel\" id=\"run-{id}\" role=\"tabpanel\" aria-labelledby=\"tab-{id}\">\n",
            id = run.id
        ));
        out.push_str(&format!(
            "      <p class=\"spec-label\">{}</p>\n",
            esc(run.label)
        ));
        out.push_str(&format!(
            "      <p class=\"spec-cmd\"><code>{}</code></p>\n",
            esc(&run.command)
        ));
        out.push_str("      <div class=\"steps\">\n");
        out.push_str(&format!(
            "        <div class=\"step step-source\"><p class=\"step-h\"><span class=\"step-n\">1</span>Source</p>{}</div>\n",
            run.source_html
        ));
        out.push_str(&format!(
            "        <div class=\"step step-case\"><p class=\"step-h\"><span class=\"step-n\">2</span>Case</p>{}</div>\n",
            run.case_html
        ));
        out.push_str(&format!(
            "        <div class=\"step step-outcome\"><p class=\"step-h\"><span class=\"step-n\">3</span>Outcome</p>{}{}</div>\n",
            stamp_html(&run.kind),
            opinion(&run.opinion)
        ));
        out.push_str("      </div>\n    </section>\n");
    }
    out.push_str("  </div>\n  <div class=\"spec-dots\" aria-hidden=\"true\">");
    for i in 0..runs.len() {
        out.push_str(if i == 0 {
            "<i class=\"on\"></i>"
        } else {
            "<i></i>"
        });
    }
    out.push_str(concat!(
        "</div>\n",
        "  <p class=\"spec-foot\">Computed by the Fidryn interpreter when this page was built.</p>\n",
        "</div>",
    ));
    out
}

/// Opinion sentences as a list; a closing `Outside scope: …` sentence is
/// marked as the model boundary.
fn opinion(sentences: &[String]) -> String {
    let last = sentences.len().saturating_sub(1);
    let items: String = sentences
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == last && s.starts_with("Outside scope: ") {
                format!("<li class=\"boundary\">{}</li>", esc(s))
            } else {
                format!("<li>{}</li>", esc(s))
            }
        })
        .collect();
    format!("<ul class=\"opinion\">{items}</ul>")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets() -> Assets {
        Assets {
            css: "0123abcd".to_owned(),
            js: "89abcdef".to_owned(),
        }
    }

    fn guide(slug: &str) -> &'static Guide {
        GUIDES.iter().find(|g| g.slug == slug).expect("guide")
    }

    fn page(with_toc: bool) -> Page {
        let toc = if with_toc {
            vec![
                TocEntry {
                    level: 2,
                    number: "6.1".to_owned(),
                    id: "envelope".to_owned(),
                    text: "Envelope".to_owned(),
                },
                TocEntry {
                    level: 3,
                    number: "6.1.1".to_owned(),
                    id: "as-of".to_owned(),
                    text: "As <of>".to_owned(),
                },
            ]
        } else {
            Vec::new()
        };
        Page {
            title: "Outcomes".to_owned(),
            lead: "Lead.".to_owned(),
            body: "<p>Body text.</p>\n".to_owned(),
            toc,
            sections: Vec::new(),
        }
    }

    fn run(id: &'static str, label: &'static str, kind: &str, opinion: &[&str]) -> Run {
        Run {
            id,
            label,
            command: format!("fidryn run m.fr --query {id} --case <case>.json"),
            source_html: format!("<figure class=\"code\">source {id}</figure>"),
            case_html: format!("<figure class=\"code\">case {id}</figure>"),
            kind: kind.to_owned(),
            opinion: opinion.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn runs() -> Vec<Run> {
        vec![
            run(
                "trust-open",
                "Trust, open eligibility",
                "contingent",
                &[
                    "acting_trustee depends on SuccessorEligibility.",
                    "Outside scope: tax.",
                ],
            ),
            run(
                "trust-court",
                "Trust, court selects I2",
                "determinate",
                &["acting_trustee is Bob."],
            ),
            run(
                "gate-q",
                "require-gate, query q",
                "determinate",
                &["q is 7.", "Outside scope: complete_instruments."],
            ),
            run(
                "gate-r",
                "require-gate, query r",
                "suspended",
                &["Outside scope: first, not last.", "r is suspended."],
            ),
        ]
    }

    /// The text between `start` and the next `end` after it.
    fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
        let from = text
            .find(start)
            .unwrap_or_else(|| panic!("missing {start}"))
            + start.len();
        let to = text[from..]
            .find(end)
            .unwrap_or_else(|| panic!("missing {end}"))
            + from;
        &text[from..to]
    }

    fn all_pages() -> Vec<String> {
        let mut pages = vec![landing(&runs(), &assets()), not_found(&assets())];
        for g in GUIDES {
            pages.push(doc(g, &page(true), &assets()));
            pages.push(doc(g, &page(false), &assets()));
        }
        pages
    }

    #[test]
    fn no_slot_is_left_in_any_page() {
        for html in all_pages() {
            assert!(!html.contains("{{"), "unfilled slot in:\n{html}");
            assert!(html.starts_with("<!doctype html>\n") && html.ends_with("</html>\n"));
        }
    }

    #[test]
    fn guides_nav_marks_only_the_current_guide() {
        for g in GUIDES {
            let nav = guides_nav(Some(g.slug));
            assert_eq!(nav.matches(CURRENT).count(), 1, "{}", g.slug);
            let href = format!("<a href=\"{}\"{CURRENT}>", guides::url(g.slug));
            assert!(nav.contains(&href), "{} not marked:\n{nav}", g.slug);
        }
        assert!(!guides_nav(None).contains("aria-current"));
    }

    #[test]
    fn guides_nav_lists_the_groups_in_order_then_implementers() {
        let nav = guides_nav(None);
        let labels: Vec<&str> = nav
            .match_indices("<p class=\"label\">")
            .map(|(i, m)| between(&nav[i..], m, "</p>"))
            .collect();
        assert_eq!(
            labels,
            [
                "Contents",
                "Start",
                "Write",
                "Run",
                "Read results",
                "Contribute",
                "Implementers"
            ]
        );
        let start = between(&nav, "<p class=\"label\">Start</p>", "</ol>");
        assert!(start.contains(
            "<a href=\"/docs/\"><span class=\"n\"></span><span class=\"t\">Overview</span><span class=\"d\">Where to begin</span></a>"
        ));
        assert!(start.find("/docs/\"").unwrap() < start.find("/docs/getting-started").unwrap());
        assert!(nav.contains(
            "<a href=\"/docs/outcomes\"><span class=\"n\">&sect;6</span><span class=\"t\">Outcomes</span><span class=\"d\">The six kinds and the envelope</span></a>"
        ));
        assert!(nav.contains(
            "<a href=\"https://github.com/BeeGass/fidryn/blob/main/docs/ARCHITECTURE.md\"><span class=\"n\" aria-hidden=\"true\">&#8599;</span><span class=\"t\">Architecture</span>"
        ));
        assert!(nav.starts_with("<nav id=\"drawer\" class=\"guides\" aria-label=\"Guides\">"));
    }

    #[test]
    fn doc_page_nav_marks_one_guide_and_the_right_site_link() {
        for g in GUIDES {
            let html = doc(g, &page(true), &assets());
            let nav = between(&html, "<nav id=\"drawer\"", "</nav>");
            assert_eq!(nav.matches(CURRENT).count(), 1, "{}", g.slug);
            let site_nav = between(&html, "<nav class=\"site-nav\"", "</nav>");
            let (docs, examples) = if g.slug == "examples" {
                ("", CURRENT)
            } else {
                (CURRENT, "")
            };
            assert!(
                site_nav.contains(&format!("<a href=\"/docs/\"{docs}>Docs</a>")),
                "{site_nav}"
            );
            assert!(
                site_nav.contains(&format!(
                    "<a href=\"/docs/examples\"{examples}>Examples</a>"
                )),
                "{site_nav}"
            );
        }
        let landing = landing(&runs(), &assets());
        assert!(!between(&landing, "<nav class=\"site-nav\"", "</nav>").contains("aria-current"));
    }

    #[test]
    fn pager_links_the_neighbors_in_reading_order() {
        assert_eq!(
            pager("index"),
            "<nav class=\"pager\" aria-label=\"Previous and next guide\"><a class=\"next\" href=\"/docs/getting-started\"><span class=\"label\">Next &middot; &sect;1</span><span class=\"t\">Getting started</span></a></nav>"
        );
        assert_eq!(
            pager("getting-started"),
            "<nav class=\"pager\" aria-label=\"Previous and next guide\"><a class=\"prev\" href=\"/docs/\"><span class=\"label\">Previous</span><span class=\"t\">Overview</span></a><a class=\"next\" href=\"/docs/language\"><span class=\"label\">Next &middot; &sect;2</span><span class=\"t\">Language</span></a></nav>"
        );
        assert_eq!(
            pager("outcomes"),
            "<nav class=\"pager\" aria-label=\"Previous and next guide\"><a class=\"prev\" href=\"/docs/mill\"><span class=\"label\">Previous &middot; &sect;5</span><span class=\"t\">Mill</span></a><a class=\"next\" href=\"/docs/examples\"><span class=\"label\">Next &middot; &sect;7</span><span class=\"t\">Examples</span></a></nav>"
        );
        assert_eq!(
            pager("contributing"),
            "<nav class=\"pager\" aria-label=\"Previous and next guide\"><a class=\"prev\" href=\"/docs/examples\"><span class=\"label\">Previous &middot; &sect;7</span><span class=\"t\">Examples</span></a></nav>"
        );
    }

    #[test]
    fn doc_page_has_kicker_title_links_and_body() {
        let html = doc(guide("outcomes"), &page(true), &assets());
        assert!(html.contains("<p class=\"kicker\">&sect; 6 &middot; Read results</p>"));
        assert!(html.contains("<h1 id=\"doc-title\">Outcomes</h1>"));
        assert!(html.contains("<title>Outcomes — Fidryn</title>"));
        assert!(html.contains(
            "<p class=\"crumb\"><a href=\"/docs/\">Docs</a> <span aria-hidden=\"true\">/</span> Outcomes</p>"
        ));
        assert!(html.contains("<body class=\"page-doc\">"));
        assert!(html.contains("<a href=\"https://github.com/BeeGass/fidryn/blob/main/docs/outcomes.md\">Edit this page on GitHub</a>"));
        assert!(html.contains("<a href=\"/docs/outcomes.md\">View as Markdown</a>"));
        assert!(html.contains("<p>Body text.</p>"));
        assert!(
            html.contains("/assets/fidryn.css?v=0123abcd")
                && html.contains("/assets/fidryn.js?v=89abcdef")
        );

        let overview = doc(guide("index"), &page(true), &assets());
        assert!(overview.contains("<p class=\"kicker\">Documentation</p>"));
        assert!(overview.contains("<title>Documentation — Fidryn</title>"));
        assert!(overview.contains("<a href=\"/docs/index.md\">View as Markdown</a>"));
    }

    #[test]
    fn on_this_page_blocks_follow_the_toc() {
        let with = doc(guide("outcomes"), &page(true), &assets());
        let items = "<li class=\"lvl-2\"><a href=\"#envelope\"><span class=\"n\">6.1</span> Envelope</a></li><li class=\"lvl-3\"><a href=\"#as-of\"><span class=\"n\">6.1.1</span> As &lt;of&gt;</a></li>";
        assert!(with.contains(&format!(
            "<details class=\"onpage-inline\"><summary>On this page</summary><ol>{items}</ol></details>"
        )));
        assert!(with.contains(&format!(
            "<aside class=\"onpage\" aria-label=\"On this page\"><p class=\"label\">On this page</p><ol>{items}</ol></aside>"
        )));

        let without = doc(guide("outcomes"), &page(false), &assets());
        assert!(!without.contains("onpage"), "{without}");

        let unnumbered = toc_items(&[TocEntry {
            level: 2,
            number: String::new(),
            id: "for-users".to_owned(),
            text: "For users".to_owned(),
        }]);
        assert_eq!(
            unnumbered,
            "<li class=\"lvl-2\"><a href=\"#for-users\">For users</a></li>"
        );
    }

    #[test]
    fn specimen_has_tabs_panels_stamps_and_dots() {
        let html = specimen(&runs());
        assert!(html.contains(
            "<button type=\"button\" role=\"tab\" id=\"tab-trust-open\" aria-controls=\"run-trust-open\" aria-selected=\"true\">Trust, open eligibility <span class=\"stamp con\">Contingent</span></button>"
        ));
        assert!(html.contains(
            "<button type=\"button\" role=\"tab\" id=\"tab-trust-court\" aria-controls=\"run-trust-court\" aria-selected=\"false\" tabindex=\"-1\">Trust, court selects I2 <span class=\"stamp det\">Determinate</span></button>"
        ));
        assert_eq!(html.matches("role=\"tab\"").count(), 4);
        assert_eq!(html.matches("aria-selected=\"true\"").count(), 1);
        assert_eq!(html.matches("tabindex=\"-1\"").count(), 3);
        assert_eq!(html.matches("<p class=\"spec-label\">").count(), 4);
        assert!(html.contains("<p class=\"spec-label\">Trust, court selects I2</p>"));
        for id in ["trust-open", "trust-court", "gate-q", "gate-r"] {
            assert!(html.contains(&format!(
                "<section class=\"spec-panel\" id=\"run-{id}\" role=\"tabpanel\" aria-labelledby=\"tab-{id}\">"
            )));
            assert!(html.contains(&format!("<figure class=\"code\">source {id}</figure>")));
            assert!(html.contains(&format!("<figure class=\"code\">case {id}</figure>")));
        }
        assert!(html.contains(
            "<p class=\"spec-cmd\"><code>fidryn run m.fr --query gate-r --case &lt;case&gt;.json</code></p>"
        ));
        assert!(html.contains(
            "<p class=\"step-h\"><span class=\"step-n\">3</span>Outcome</p><span class=\"stamp sus\">Suspended</span><ul class=\"opinion\"><li>Outside scope: first, not last.</li><li>r is suspended.</li></ul>"
        ));
        assert!(html.contains(
            "<ul class=\"opinion\"><li>acting_trustee depends on SuccessorEligibility.</li><li class=\"boundary\">Outside scope: tax.</li></ul>"
        ));
        assert!(html.contains("<ul class=\"opinion\"><li>acting_trustee is Bob.</li></ul>"));
        assert!(html.contains(
            "<div class=\"spec-dots\" aria-hidden=\"true\"><i class=\"on\"></i><i></i><i></i><i></i></div>"
        ));
        assert!(html.contains("<p class=\"spec-foot\">Computed by the Fidryn interpreter when this page was built.</p>"));
    }

    #[test]
    fn stamp_maps_every_outcome_kind() {
        assert_eq!(stamp("determinate"), ("det", "Determinate"));
        assert_eq!(stamp("contingent"), ("con", "Contingent"));
        assert_eq!(stamp("suspended"), ("sus", "Suspended"));
        assert_eq!(stamp("normConflict"), ("nc", "NormConflict"));
        assert_eq!(stamp("outsideCompetence"), ("oc", "OutsideCompetence"));
        assert_eq!(stamp("inconsistent"), ("inc", "Inconsistent"));
        assert_eq!(stamp("somethingNew"), ("", "Outcome"));
        assert_eq!(
            stamp_html("somethingNew"),
            "<span class=\"stamp\">Outcome</span>"
        );
    }

    #[test]
    fn landing_page_has_hero_specimen_and_contents() {
        let html = landing(&runs(), &assets());
        assert!(
            html.contains("<title>Fidryn — a programming language for legal instruments</title>")
        );
        assert!(html.contains("<body class=\"page-landing\">"));
        assert!(html.contains("<h1 id=\"hero-title\">No false determinacy.</h1>"));
        assert!(html.contains("<div class=\"specimen\" data-specimen>"));
        let contents = between(&html, "<ol class=\"contents\">", "</ol>");
        let hrefs: Vec<&str> = contents
            .match_indices("href=\"")
            .map(|(i, m)| between(&contents[i..], m, "\""))
            .collect();
        assert_eq!(
            hrefs,
            [
                "/docs/getting-started",
                "/docs/language",
                "/docs/cases-and-time",
                "/docs/cli",
                "/docs/mill",
                "/docs/outcomes",
                "/docs/examples",
                "/docs/contributing",
            ]
        );
        assert!(contents.contains(
            "<li><a href=\"/docs/getting-started\"><span class=\"n\">&sect;1</span><span class=\"t\">Getting started</span><span class=\"lead\" aria-hidden=\"true\"></span><span class=\"d\">Check and run a tiny module</span></a></li>"
        ));
        assert!(
            !between(&html, "<header class=\"site-head\">", "</header>")
                .contains("class=\"crumb\"")
        );
    }

    #[test]
    fn not_found_page_says_no_such_provision() {
        let html = not_found(&assets());
        assert!(html.contains("<title>Not found — Fidryn</title>"));
        assert!(html.contains(
            "<meta name=\"description\" content=\"This page is outside the declared model.\">"
        ));
        assert!(html.contains("<body class=\"page-404\">"));
        assert!(html.contains("<p class=\"nf-mark\" aria-hidden=\"true\">&sect; 404</p>"));
        assert!(html.contains("<h1 id=\"nf-title\">No such provision.</h1>"));
        assert!(html.contains(
            "<p class=\"lede\">This page is outside the declared model. Nothing was invented to fill the gap.</p>"
        ));
        assert!(html.contains("<a class=\"btn pri\" href=\"/\">Back to the start</a>"));
        assert!(html.contains("<a class=\"btn sec\" href=\"/docs/\">Open the docs</a>"));
        assert!(html.contains("<meta name=\"robots\" content=\"noindex\">"));
        assert!(!html.contains("text/markdown"));
    }

    #[test]
    fn assets_read_hashes_the_two_files() {
        let dir = std::env::temp_dir().join(format!("fidryn-assets-{}", std::process::id()));
        fs::create_dir_all(dir.join("site/assets")).unwrap();
        fs::write(dir.join("site/assets/fidryn.css"), "body{}").unwrap();
        fs::write(dir.join("site/assets/fidryn.js"), "\"use strict\";").unwrap();
        let assets = Assets::read(&dir).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(assets.css, asset_version(b"body{}"));
        assert_eq!(assets.js, asset_version(b"\"use strict\";"));
        assert!(Assets::read(&dir).is_err(), "missing files are an error");
    }
}
