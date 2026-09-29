//! The learner guides published on the site, in reading order.

pub const SITE: &str = "https://fidryn.onlygass.dev";
pub const REPO: &str = "https://github.com/BeeGass/fidryn";

/// Sidebar groups, in order. Every numbered guide belongs to one.
pub const GROUPS: &[&str] = &["Start", "Write", "Run", "Read results", "Contribute"];

pub struct Guide {
    /// URL slug; `index` is the docs overview at `/docs/`.
    pub slug: &'static str,
    /// Source file under `docs/`.
    pub file: &'static str,
    /// Page title and navigation label.
    pub title: &'static str,
    /// Section number, `None` for the overview.
    pub number: Option<u32>,
    /// Sidebar group; empty for the overview.
    pub group: &'static str,
    /// One line under the title in the sidebar and the landing contents.
    pub blurb: &'static str,
    /// Meta description.
    pub description: &'static str,
}

pub const GUIDES: &[Guide] = &[
    Guide {
        slug: "index",
        file: "README.md",
        title: "Documentation",
        number: None,
        group: "",
        blurb: "Where to begin",
        description: "Fidryn documentation hub — learner guides for the programming language for legal instruments.",
    },
    Guide {
        slug: "getting-started",
        file: "getting-started.md",
        title: "Getting started",
        number: Some(1),
        group: "Start",
        blurb: "Check and run a tiny module",
        description: "Build, check, and run a tiny Fidryn (.fr) program in about an hour.",
    },
    Guide {
        slug: "language",
        file: "language.md",
        title: "Language",
        number: Some(2),
        group: "Write",
        blurb: "Modules, queries, rules, duties",
        description: "Fidryn language map: modules, queries, rules, duties, and what the language will not do.",
    },
    Guide {
        slug: "cases-and-time",
        file: "cases-and-time.md",
        title: "Cases and time",
        number: Some(3),
        group: "Write",
        blurb: "Records, valid-at, known-at",
        description: "Case records, admissible completions, valid-at and known-at in Fidryn.",
    },
    Guide {
        slug: "cli",
        file: "cli.md",
        title: "CLI",
        number: Some(4),
        group: "Run",
        blurb: "Every subcommand and flag",
        description: "Every fidryn CLI subcommand and flag the reference binary accepts.",
    },
    Guide {
        slug: "mill",
        file: "mill.md",
        title: "Mill",
        number: Some(5),
        group: "Run",
        blurb: "The localhost UI",
        description: "Localhost Fidryn mill UI on 127.0.0.1 — checks modules; does not live-file.",
    },
    Guide {
        slug: "outcomes",
        file: "outcomes.md",
        title: "Outcomes",
        number: Some(6),
        group: "Read results",
        blurb: "The six kinds and the envelope",
        description: "Determinate, Suspended, Contingent, and the rest of the Fidryn outcome envelope.",
    },
    Guide {
        slug: "examples",
        file: "examples.md",
        title: "Examples",
        number: Some(7),
        group: "Read results",
        blurb: "Trust, tax, fifty states",
        description: "Trust, tax, federal slices, and the fifty-state corpus map for Fidryn.",
    },
    Guide {
        slug: "contributing",
        file: "contributing.md",
        title: "Contributing",
        number: Some(8),
        group: "Contribute",
        blurb: "Toolchain, tests, commits",
        description: "Toolchain, tests, and how to work on the Fidryn reference interpreter.",
    },
];

/// Implementer documents, linked to GitHub from the sidebar: (title, file under docs/, blurb).
pub const IMPLEMENTER_DOCS: &[(&str, &str, &str)] = &[
    (
        "Architecture",
        "ARCHITECTURE.md",
        "Pipeline and crate contract",
    ),
    (
        "Implementation status",
        "implementation-status.md",
        "Evidence-backed capability matrix",
    ),
    ("Obligations", "OBLIGATIONS.md", "Review obligations"),
    (
        "Integration contract",
        "INTEGRATION-CONTRACT.md",
        "How the machinery is wired",
    ),
];

/// Site path of a guide: `/docs/` for the overview, `/docs/{slug}` otherwise.
pub fn url(slug: &str) -> String {
    if slug == "index" {
        "/docs/".to_owned()
    } else {
        format!("/docs/{slug}")
    }
}

/// The guide rendered from `file` (a name under `docs/`), if it is published.
pub fn by_file(file: &str) -> Option<&'static Guide> {
    GUIDES.iter().find(|g| g.file == file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::workspace_root;

    #[test]
    fn overview_comes_first_and_is_unnumbered() {
        let first = &GUIDES[0];
        assert_eq!(
            (first.slug, first.file, first.number, first.group),
            ("index", "README.md", None, "")
        );
        assert!(
            GUIDES[1..]
                .iter()
                .all(|g| g.number.is_some() && !g.group.is_empty())
        );
    }

    #[test]
    fn numbered_guides_run_one_to_eight_in_order() {
        let numbers: Vec<u32> = GUIDES.iter().filter_map(|g| g.number).collect();
        assert_eq!(numbers, (1..=8).collect::<Vec<u32>>());
    }

    #[test]
    fn groups_are_contiguous_and_in_sidebar_order() {
        let mut runs: Vec<&str> = Vec::new();
        for guide in &GUIDES[1..] {
            if runs.last() != Some(&guide.group) {
                runs.push(guide.group);
            }
        }
        assert_eq!(runs, GROUPS);
    }

    #[test]
    fn every_source_file_exists_under_docs() {
        let docs = workspace_root().join("docs");
        let files = GUIDES
            .iter()
            .map(|g| g.file)
            .chain(IMPLEMENTER_DOCS.iter().map(|d| d.1));
        for file in files {
            assert!(docs.join(file).is_file(), "docs/{file} does not exist");
        }
    }

    #[test]
    fn slugs_and_files_are_unique_and_every_guide_has_copy() {
        for (i, a) in GUIDES.iter().enumerate() {
            assert!(
                !a.title.is_empty() && !a.blurb.is_empty() && !a.description.is_empty(),
                "{}",
                a.slug
            );
            for b in &GUIDES[i + 1..] {
                assert_ne!(a.slug, b.slug);
                assert_ne!(a.file, b.file);
            }
        }
    }

    #[test]
    fn url_maps_the_overview_to_the_docs_root() {
        assert_eq!(url("index"), "/docs/");
        assert_eq!(url("cases-and-time"), "/docs/cases-and-time");
    }

    #[test]
    fn by_file_finds_published_guides_only() {
        assert_eq!(by_file("outcomes.md").map(|g| g.slug), Some("outcomes"));
        assert_eq!(by_file("README.md").map(|g| g.slug), Some("index"));
        assert!(by_file("ARCHITECTURE.md").is_none());
        assert!(by_file("docs/cli.md").is_none());
    }
}
