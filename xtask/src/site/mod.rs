//! `cargo xtask site`: render the public site under `site/` from `docs/*.md`.

mod guides;
mod highlight;
mod html;
mod links;
mod markdown;
mod pages;
mod search;
mod seo;
mod specimen;
mod templates;

use crate::workspace::workspace_root;
use anyhow::{Context, Result, bail};
use clap::Args;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct SiteArgs {
    /// Fail if the committed site differs from a fresh render instead of writing it
    #[arg(long)]
    check: bool,
}

/// One generated file, relative to `site/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutFile {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

impl OutFile {
    pub fn text(path: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            bytes: text.into().into_bytes(),
        }
    }
}

pub fn run(args: SiteArgs) -> Result<()> {
    let root = workspace_root();
    if args.check {
        return check(&root);
    }
    let files = build(&root)?;
    write_files(&root.join("site"), &files)?;
    eprintln!("wrote {} files under site/", files.len());
    Ok(())
}

/// Fail when the committed site differs from a fresh render.
pub fn check(root: &Path) -> Result<()> {
    let files = build(root)?;
    let stale = stale_files(&root.join("site"), &files)?;
    if !stale.is_empty() {
        bail!(
            "site/ is out of date; run `cargo xtask site`:\n  {}",
            stale.join("\n  ")
        );
    }
    eprintln!("site/ is up to date ({} generated files)", files.len());
    Ok(())
}

/// Render every generated file. Reads inputs under `root`; writes nothing.
pub fn build(root: &Path) -> Result<Vec<OutFile>> {
    let kw = highlight::Keywords::load(root)?;
    let assets = pages::Assets::read(root)?;
    let runs = specimen::runs(root, &kw)?;
    let mut sources = Vec::with_capacity(guides::GUIDES.len());
    for guide in guides::GUIDES {
        let path = root.join("docs").join(guide.file);
        let md = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let page = markdown::render(&md, guide.number, &kw);
        sources.push((guide, md, page));
    }

    let mut files = vec![
        OutFile::text("index.html", pages::landing(&runs, &assets)),
        OutFile::text("index.md", seo::landing_markdown()),
        OutFile::text("404.html", pages::not_found(&assets)),
    ];
    let mut mirrors = Vec::with_capacity(sources.len());
    for (guide, md, page) in &sources {
        let mirror = seo::mirror(guide, md);
        files.push(OutFile::text(
            format!("docs/{}.html", guide.slug),
            pages::doc(guide, page, &assets),
        ));
        files.push(OutFile::text(
            format!("docs/{}.md", guide.slug),
            mirror.clone(),
        ));
        mirrors.push((*guide, mirror));
    }
    let indexed: Vec<_> = sources
        .iter()
        .map(|(guide, _, page)| (*guide, page))
        .collect();
    files.extend([
        OutFile::text("search-index.json", search::index(&indexed)),
        OutFile::text("sitemap.xml", seo::sitemap()),
        OutFile::text("robots.txt", seo::robots()),
        OutFile::text("llms.txt", seo::llms_txt()),
        OutFile::text("llms-full.txt", seo::llms_full(&mirrors)),
    ]);
    Ok(files)
}

/// Generated files that are missing or differ on disk, then files under
/// `site/docs/` that the generator no longer produces (sorted).
pub fn stale_files(site: &Path, files: &[OutFile]) -> Result<Vec<String>> {
    let mut stale = Vec::new();
    for file in files {
        match fs::read(site.join(&file.path)) {
            Ok(bytes) if bytes == file.bytes => {}
            Ok(_) => stale.push(format!("{} differs", file.path.display())),
            Err(_) => stale.push(format!("{} is missing", file.path.display())),
        }
    }
    let expected: BTreeSet<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    let docs = site.join("docs");
    if docs.is_dir() {
        let mut extra = Vec::new();
        for entry in fs::read_dir(&docs).with_context(|| format!("read {}", docs.display()))? {
            let rel = Path::new("docs").join(entry?.file_name());
            if !expected.contains(&rel) {
                extra.push(format!("{} is not generated; delete it", rel.display()));
            }
        }
        extra.sort();
        stale.extend(extra);
    }
    Ok(stale)
}

/// Write `files` under `site`, creating directories as needed.
pub fn write_files(site: &Path, files: &[OutFile]) -> Result<()> {
    for file in files {
        let path = site.join(&file.path);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        }
        fs::write(&path, &file.bytes).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory under the system temp dir, private to one test.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fidryn-xtask-site-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn stale_files_lists_missing_differing_then_extra_docs_files() {
        let site = scratch("stale");
        fs::create_dir_all(site.join("docs")).unwrap();
        fs::create_dir_all(site.join("assets")).unwrap();
        fs::write(site.join("robots.txt"), "same").unwrap();
        fs::write(site.join("docs/cli.html"), "old").unwrap();
        fs::write(site.join("docs/zeta.md"), "stray").unwrap();
        fs::write(site.join("docs/alpha.html"), "stray").unwrap();
        fs::write(site.join("assets/fidryn.css"), "hand-written").unwrap();
        fs::write(site.join("README.md"), "hand-written").unwrap();
        let files = [
            OutFile::text("docs/cli.html", "new"),
            OutFile::text("robots.txt", "same"),
            OutFile::text("sitemap.xml", "<urlset/>"),
        ];
        let stale = stale_files(&site, &files).unwrap();
        assert_eq!(
            stale,
            [
                "docs/cli.html differs",
                "sitemap.xml is missing",
                "docs/alpha.html is not generated; delete it",
                "docs/zeta.md is not generated; delete it",
            ]
        );
        fs::remove_dir_all(&site).unwrap();
    }

    #[test]
    fn stale_files_without_a_docs_dir_reports_only_generated_paths() {
        let site = scratch("no-docs");
        let files = [
            OutFile::text("docs/index.html", "x"),
            OutFile::text("robots.txt", "r"),
        ];
        let stale = stale_files(&site, &files).unwrap();
        assert_eq!(
            stale,
            ["docs/index.html is missing", "robots.txt is missing"]
        );
        fs::remove_dir_all(&site).unwrap();
    }

    #[test]
    fn write_then_check_is_clean() {
        let site = scratch("write-check");
        fs::create_dir_all(site.join("docs")).unwrap();
        fs::write(site.join("docs/cli.html"), "old").unwrap();
        let files = [
            OutFile::text("index.html", "<!doctype html>\n"),
            OutFile::text("docs/cli.html", "new"),
            OutFile {
                path: PathBuf::from("docs/data.bin"),
                bytes: vec![0, 159, 146, 150],
            },
        ];
        write_files(&site, &files).unwrap();
        assert!(stale_files(&site, &files).unwrap().is_empty());
        assert_eq!(
            fs::read_to_string(site.join("docs/cli.html")).unwrap(),
            "new"
        );
        assert_eq!(
            fs::read(site.join("docs/data.bin")).unwrap(),
            [0, 159, 146, 150]
        );
        write_files(&site, &files).unwrap();
        assert!(stale_files(&site, &files).unwrap().is_empty());
        fs::remove_dir_all(&site).unwrap();
    }
}

#[cfg(test)]
mod build_tests {
    use super::*;

    #[test]
    fn build_writes_exactly_the_site_files() {
        let files = build(&workspace_root()).expect("build");
        let mut got: Vec<String> = files.iter().map(|f| f.path.display().to_string()).collect();
        got.sort();
        let mut want: Vec<String> = [
            "index.html",
            "index.md",
            "404.html",
            "search-index.json",
            "sitemap.xml",
            "robots.txt",
            "llms.txt",
            "llms-full.txt",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        for guide in guides::GUIDES {
            want.push(format!("docs/{}.html", guide.slug));
            want.push(format!("docs/{}.md", guide.slug));
        }
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn two_builds_are_byte_identical() {
        let root = workspace_root();
        assert!(build(&root).expect("first build") == build(&root).expect("second build"));
    }

    #[test]
    fn every_mirror_is_in_llms_full_and_every_page_has_a_mirror_link() {
        let files = build(&workspace_root()).expect("build");
        let text = |path: &str| {
            let file = files
                .iter()
                .find(|f| f.path == Path::new(path))
                .unwrap_or_else(|| panic!("{path}"));
            String::from_utf8(file.bytes.clone()).expect("utf-8")
        };
        let full = text("llms-full.txt");
        for guide in guides::GUIDES {
            let mirror = text(&format!("docs/{}.md", guide.slug));
            assert!(mirror.starts_with("---\ntitle: "), "{}", guide.slug);
            assert_eq!(
                mirror.matches("\n> Canonical HTML: ").count(),
                1,
                "{}",
                guide.slug
            );
            assert!(
                full.contains(&mirror),
                "{} mirror missing from llms-full.txt",
                guide.slug
            );
            let html = text(&format!("docs/{}.html", guide.slug));
            assert!(html.contains(&format!(
                "<a href=\"/docs/{}.md\">View as Markdown</a>",
                guide.slug
            )));
        }
    }
}
