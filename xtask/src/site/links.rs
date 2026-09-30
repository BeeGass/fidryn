//! Link targets for rendered pages and markdown mirrors. Links to learner
//! guides stay on the site; every other repository path goes to GitHub.

use super::guides::{self, REPO, SITE};
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};

/// Where a rewritten link will be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkStyle {
    /// Rendered pages: guides become site paths such as `/docs/cli#file`.
    Html,
    /// Markdown mirrors: guides become absolute `.md` URLs.
    Markdown,
}

/// The target of a link written in `docs/*.md`. Guides stay on the site
/// (`README.md` is the overview at `/docs/`); other files under `docs/` and
/// `../` repository paths become GitHub `blob/main` URLs, or `tree/main` for
/// directories (a trailing `/`). Fragments are kept. Absolute URLs, `/…`,
/// `#…`, and `mailto:` targets are returned unchanged.
pub fn rewrite(href: &str, style: LinkStyle) -> String {
    if href.is_empty() || href.starts_with(['#', '/']) || has_scheme(href) {
        return href.to_owned();
    }
    let (path, fragment) = href.split_at(href.find('#').unwrap_or(href.len()));
    let repo_path = match path.strip_prefix("../") {
        Some(outside_docs) => outside_docs.to_owned(),
        None => {
            let file = path.strip_prefix("./").unwrap_or(path);
            if let Some(guide) = guides::by_file(file) {
                return match style {
                    LinkStyle::Html => format!("{}{fragment}", guides::url(guide.slug)),
                    LinkStyle::Markdown => format!("{SITE}/docs/{}.md{fragment}", guide.slug),
                };
            }
            format!("docs/{file}")
        }
    };
    let kind = if repo_path.is_empty() || repo_path.ends_with('/') {
        "tree"
    } else {
        "blob"
    };
    format!("{REPO}/{kind}/main/{repo_path}{fragment}")
}

/// `md` with the target of every inline link rewritten with
/// [`LinkStyle::Markdown`]. Everything else, code included, is kept byte for
/// byte.
pub fn rewrite_markdown_links(md: &str) -> String {
    let mut out = String::with_capacity(md.len() + 1024);
    let mut copied = 0;
    for (event, range) in Parser::new_ext(md, Options::ENABLE_TABLES).into_offset_iter() {
        let Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url,
            ..
        }) = event
        else {
            continue;
        };
        let Some(open) = md[range.clone()].rfind("](") else {
            continue;
        };
        let mut start = range.start + open + 2;
        if md[start..].starts_with('<') {
            start += 1;
        }
        if start < copied || !md[start..].starts_with(dest_url.as_ref()) {
            continue;
        }
        out.push_str(&md[copied..start]);
        out.push_str(&rewrite(&dest_url, LinkStyle::Markdown));
        copied = start + dest_url.len();
    }
    out.push_str(&md[copied..]);
    out
}

/// Whether `href` starts with a URL scheme such as `https:` or `mailto:`.
fn has_scheme(href: &str) -> bool {
    href.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::guides::GUIDES;
    use crate::workspace::workspace_root;
    use std::fs;

    const STYLES: [LinkStyle; 2] = [LinkStyle::Html, LinkStyle::Markdown];

    #[test]
    fn guide_links_stay_on_the_site() {
        assert_eq!(rewrite("cli.md", LinkStyle::Html), "/docs/cli");
        assert_eq!(rewrite("cli.md#file", LinkStyle::Html), "/docs/cli#file");
        assert_eq!(
            rewrite("./outcomes.md#determinate", LinkStyle::Html),
            "/docs/outcomes#determinate"
        );
        assert_eq!(rewrite("README.md", LinkStyle::Html), "/docs/");
    }

    #[test]
    fn guide_links_in_mirrors_are_absolute_markdown_urls() {
        assert_eq!(
            rewrite("cli.md", LinkStyle::Markdown),
            "https://fidryn.onlygass.dev/docs/cli.md"
        );
        assert_eq!(
            rewrite("cases-and-time.md#checklist", LinkStyle::Markdown),
            "https://fidryn.onlygass.dev/docs/cases-and-time.md#checklist"
        );
        assert_eq!(
            rewrite("README.md", LinkStyle::Markdown),
            "https://fidryn.onlygass.dev/docs/index.md"
        );
    }

    #[test]
    fn other_docs_go_to_github() {
        for style in STYLES {
            assert_eq!(
                rewrite("ARCHITECTURE.md", style),
                "https://github.com/BeeGass/fidryn/blob/main/docs/ARCHITECTURE.md"
            );
            assert_eq!(
                rewrite("implementation-status.md#trust", style),
                "https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md#trust"
            );
        }
    }

    #[test]
    fn repository_paths_go_to_blob_or_tree() {
        for style in STYLES {
            assert_eq!(
                rewrite("../grammar.ebnf", style),
                "https://github.com/BeeGass/fidryn/blob/main/grammar.ebnf"
            );
            assert_eq!(
                rewrite("../README.md#install", style),
                "https://github.com/BeeGass/fidryn/blob/main/README.md#install"
            );
            assert_eq!(
                rewrite("../tests/programs/", style),
                "https://github.com/BeeGass/fidryn/tree/main/tests/programs/"
            );
        }
    }

    #[test]
    fn absolute_and_local_links_are_unchanged() {
        for href in [
            "https://fidryn.onlygass.dev/",
            "http://127.0.0.1:8751",
            "mailto:someone@example.com",
            "/docs/cli",
            "#file",
            "",
        ] {
            for style in STYLES {
                assert_eq!(rewrite(href, style), href);
            }
        }
    }

    #[test]
    fn mirrors_rewrite_inline_link_targets_and_nothing_else() {
        let md = concat!(
            "See [CLI](cli.md#run) and the\n",
            "[language\ngrammar](../grammar.ebnf \"Grammar\"), [top](#top), <https://example.com>.\n\n",
            "Code: `[x](cli.md)`\n\n",
            "```\n[y](cli.md)\n```\n\n",
            "| a | [b](mill.md) |\n| --- | --- |\n"
        );
        assert_eq!(
            rewrite_markdown_links(md),
            concat!(
                "See [CLI](https://fidryn.onlygass.dev/docs/cli.md#run) and the\n",
                "[language\ngrammar](https://github.com/BeeGass/fidryn/blob/main/grammar.ebnf \"Grammar\"), [top](#top), <https://example.com>.\n\n",
                "Code: `[x](cli.md)`\n\n",
                "```\n[y](cli.md)\n```\n\n",
                "| a | [b](https://fidryn.onlygass.dev/docs/mill.md) |\n| --- | --- |\n"
            )
        );
    }

    /// The text a reader sees: every text and code event, in order.
    fn visible_text(md: &str) -> String {
        Parser::new_ext(md, Options::ENABLE_TABLES)
            .filter_map(|event| match event {
                Event::Text(text) | Event::Code(text) => Some(text.into_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn real_guide_mirrors_link_only_to_absolute_urls() {
        let docs = workspace_root().join("docs");
        for guide in GUIDES {
            let md = fs::read_to_string(docs.join(guide.file)).expect("read guide");
            let mirror = rewrite_markdown_links(&md);
            for event in Parser::new_ext(&mirror, Options::ENABLE_TABLES) {
                if let Event::Start(Tag::Link { dest_url, .. }) = event {
                    assert!(
                        dest_url.starts_with("https://") || dest_url.starts_with('#'),
                        "{}: {dest_url}",
                        guide.file
                    );
                }
            }
            assert_eq!(visible_text(&mirror), visible_text(&md), "{}", guide.file);
        }
    }
}
