//! Read-only release version and changelog admission.

use crate::workspace::{cargo_bin, workspace_root};
use anyhow::{Context, Result, bail, ensure};
use cargo_metadata::{Metadata, MetadataCommand};
use clap::Args;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use semver::Version;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use toml_edit::{DocumentMut, Item};

#[derive(Debug, Args)]
pub struct ReleaseCheckArgs {
    /// Require a version increase over this full 40- or 64-digit commit SHA.
    #[arg(long)]
    pub base: Option<String>,
}

#[derive(Debug, Args)]
pub struct ReleaseNotesArgs {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseMetadata {
    pub version: Version,
    pub date: String,
    /// Exact Markdown bytes between the current release heading and the next H2.
    pub notes: String,
}

pub fn run_check(args: ReleaseCheckArgs) -> Result<()> {
    let release = release_check(&workspace_root(), args.base.as_deref())?;
    println!(
        "release {} ({}): workspace versions and changelog verified",
        release.version, release.date
    );
    Ok(())
}

pub fn run_notes(_args: ReleaseNotesArgs) -> Result<()> {
    let notes = release_notes(&workspace_root())?;
    std::io::stdout()
        .lock()
        .write_all(notes.as_bytes())
        .context("cannot write release notes")
}

pub fn release_notes(root: &Path) -> Result<String> {
    Ok(release_check(root, None)?.notes)
}

/// Check local manifests, lockfile, changelog and, optionally, the fetched base/tag state.
pub fn release_check(root: &Path, base: Option<&str>) -> Result<ReleaseMetadata> {
    if let Some(base) = base {
        validate_base(base)?;
    }
    let manifest = read_toml(&root.join("Cargo.toml"))?;
    let declared_version = workspace_version(&manifest)?;
    let metadata = MetadataCommand::new()
        .cargo_path(cargo_bin())
        .current_dir(root)
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .other_options(["--offline".to_owned(), "--locked".to_owned()])
        .verbose(false)
        .exec()
        .context("read-only cargo metadata --no-deps --offline --locked failed")?;
    let cli = metadata
        .workspace_packages()
        .into_iter()
        .find(|package| package.name == "fidryn-cli")
        .context("fidryn-cli is not a workspace member")?;
    ensure!(
        declared_version == cli.version,
        "workspace version {declared_version} does not match fidryn-cli {}",
        cli.version
    );
    check_workspace_versions(root, &manifest, &metadata, &cli.version)?;
    let changelog = fs::read_to_string(root.join("CHANGELOG.md"))
        .context("CHANGELOG.md is required for a release")?;
    let release = extract_entry(&changelog, &cli.version)?;
    if let Some(base) = base {
        let previous = git(root, &["show", &format!("{base}:Cargo.toml")]).with_context(|| {
            format!("cannot read base {base}:Cargo.toml; fetch the base commit")
        })?;
        let previous: DocumentMut = previous
            .parse()
            .context("base Cargo.toml is not valid TOML")?;
        let previous = workspace_version(&previous)?;
        ensure!(
            release.version.cmp_precedence(&previous).is_gt(),
            "release version {} must strictly increase over base version {previous}",
            release.version
        );
        check_existing_tag(root, &release.version)?;
    }
    Ok(release)
}

fn validate_base(base: &str) -> Result<()> {
    ensure!(
        matches!(base.len(), 40 | 64) && base.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "--base must be a full 40- or 64-digit hexadecimal commit SHA"
    );
    Ok(())
}

fn read_toml(path: &Path) -> Result<DocumentMut> {
    fs::read_to_string(path)
        .with_context(|| format!("cannot read {}", path.display()))?
        .parse()
        .with_context(|| format!("invalid TOML in {}", path.display()))
}

fn at<'a>(document: &'a DocumentMut, keys: &[&str]) -> Option<&'a Item> {
    let (first, rest) = keys.split_first()?;
    let mut item = document.get(first)?;
    for key in rest {
        item = item.get(key)?;
    }
    Some(item)
}

fn workspace_version(document: &DocumentMut) -> Result<Version> {
    let version = at(document, &["workspace", "package", "version"])
        .and_then(Item::as_str)
        .context("Cargo.toml must declare workspace.package.version as a semantic version")?;
    Version::parse(version).context("workspace.package.version is not a semantic version")
}

fn check_workspace_versions(
    root: &Path,
    manifest: &DocumentMut,
    metadata: &Metadata,
    version: &Version,
) -> Result<()> {
    let mut members = BTreeMap::new();
    for package in metadata.workspace_packages() {
        ensure!(
            &package.version == version,
            "workspace package `{}` has version {}, expected {version}",
            package.name,
            package.version
        );
        let directory = package
            .manifest_path
            .parent()
            .context("workspace package manifest has no directory")?
            .as_std_path()
            .canonicalize()?;
        members.insert(package.name.to_string(), directory);
    }
    if let Some(dependencies) = at(manifest, &["workspace", "dependencies"]) {
        check_dependency_table(dependencies, root, manifest, &members, version)?;
    }
    for package in metadata.workspace_packages() {
        let path = package.manifest_path.as_std_path();
        let document = read_toml(path)?;
        for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(dependencies) = document.get(table) {
                check_dependency_table(
                    dependencies,
                    path.parent().context("manifest has no parent")?,
                    manifest,
                    &members,
                    version,
                )?;
            }
        }
        if let Some(targets) = document.get("target").and_then(Item::as_table_like) {
            for (_, target) in targets.iter() {
                for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
                    if let Some(dependencies) = target.get(table) {
                        check_dependency_table(
                            dependencies,
                            path.parent().context("manifest has no parent")?,
                            manifest,
                            &members,
                            version,
                        )?;
                    }
                }
            }
        }
    }
    let lock = read_toml(&root.join("Cargo.lock"))?;
    let packages = lock
        .get("package")
        .and_then(Item::as_array_of_tables)
        .context("Cargo.lock has no package entries")?;
    for name in members.keys() {
        let versions = packages
            .iter()
            .filter(|package| {
                package.get("name").and_then(Item::as_str) == Some(name.as_str())
                    && package.get("source").is_none()
            })
            .map(|package| package.get("version").and_then(Item::as_str))
            .collect::<Vec<_>>();
        let expected = version.to_string();
        ensure!(
            versions == [Some(expected.as_str())],
            "Cargo.lock must contain exactly one `{name}` package at version {version}"
        );
    }
    Ok(())
}

fn check_dependency_table(
    dependencies: &Item,
    directory: &Path,
    manifest: &DocumentMut,
    members: &BTreeMap<String, PathBuf>,
    version: &Version,
) -> Result<()> {
    let dependencies = dependencies
        .as_table_like()
        .context("dependency section must be a table")?;
    for (key, dependency) in dependencies.iter() {
        if dependency.get("workspace").and_then(Item::as_bool) == Some(true) {
            let inherited =
                at(manifest, &["workspace", "dependencies", key]).with_context(|| {
                    format!("dependency `{key}` is not declared in workspace.dependencies")
                })?;
            let name = inherited
                .get("package")
                .and_then(Item::as_str)
                .unwrap_or(key);
            if members.contains_key(name) {
                ensure!(
                    dependency.get("version").is_none() && dependency.get("path").is_none(),
                    "inherited internal dependency `{key}` cannot override its version or path"
                );
            }
            continue;
        }
        let name = dependency
            .get("package")
            .and_then(Item::as_str)
            .unwrap_or(key);
        let Some(expected_path) = members.get(name) else {
            continue;
        };
        let declared = dependency
            .as_str()
            .or_else(|| dependency.get("version").and_then(Item::as_str))
            .with_context(|| {
                format!("internal dependency `{key}` must declare version {version}")
            })?;
        ensure!(
            declared.strip_prefix('=').unwrap_or(declared).trim() == version.to_string(),
            "internal dependency `{key}` declares version `{declared}`, expected `{version}`"
        );
        let path = dependency
            .get("path")
            .and_then(Item::as_str)
            .with_context(|| format!("internal dependency `{key}` must use its workspace path"))?;
        let actual_path = directory
            .join(path)
            .canonicalize()
            .with_context(|| format!("cannot resolve internal dependency `{key}` path"))?;
        ensure!(
            &actual_path == expected_path,
            "internal dependency `{key}` does not point at workspace package `{name}`"
        );
    }
    Ok(())
}

fn git_output(root: &Path, arguments: &[&str]) -> Result<Output> {
    Command::new("git")
        .current_dir(root)
        .args(arguments)
        .output()
        .context("cannot execute git")
}

fn git(root: &Path, arguments: &[&str]) -> Result<String> {
    let output = git_output(root, arguments)?;
    ensure!(
        output.status.success(),
        "git {} failed: {}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    String::from_utf8(output.stdout).context("git output is not UTF-8")
}

fn check_existing_tag(root: &Path, version: &Version) -> Result<()> {
    let reference = format!("refs/tags/v{version}");
    let existing = git_output(root, &["show-ref", "--verify", "--quiet", &reference])?;
    if existing.status.code() == Some(1) {
        return Ok(());
    }
    ensure!(
        existing.status.success(),
        "cannot inspect release tag `{reference}`"
    );
    let tag = git(
        root,
        &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
    )?;
    let head = git(root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    ensure!(
        tag.trim() == head.trim(),
        "release tag `v{version}` already exists at another commit; increase the version"
    );
    Ok(())
}

struct Heading {
    start: usize,
    end: usize,
}

fn extract_entry(changelog: &str, version: &Version) -> Result<ReleaseMetadata> {
    let headings = Parser::new(changelog)
        .into_offset_iter()
        .filter_map(|(event, range)| {
            matches!(
                event,
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H2,
                    ..
                })
            )
            .then_some(Heading {
                start: range.start,
                end: range.end,
            })
        })
        .collect::<Vec<_>>();
    let prefix = format!("## [{version}]");
    let matches = headings
        .iter()
        .enumerate()
        .filter(|(_, heading)| {
            let line_start = changelog[..heading.start]
                .rfind('\n')
                .map_or(0, |index| index + 1);
            line_start == heading.start
                && changelog[heading.start..heading.end]
                    .trim()
                    .starts_with(&prefix)
        })
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1,
        "CHANGELOG.md must contain exactly one dated `## [{version}] - YYYY-MM-DD` entry"
    );
    let (index, heading) = matches[0];
    let line = changelog[heading.start..heading.end].trim();
    let date = line
        .strip_prefix(&format!("{prefix} - "))
        .with_context(|| {
            format!("release {version} heading must be `## [{version}] - YYYY-MM-DD`")
        })?;
    validate_date(date)?;
    let end = headings
        .get(index + 1)
        .map_or(changelog.len(), |next| next.start);
    let body = &changelog[heading.end..end];
    validate_classified_notes(body)?;
    Ok(ReleaseMetadata {
        version: version.clone(),
        date: date.into(),
        notes: body.into(),
    })
}

fn validate_date(date: &str) -> Result<()> {
    ensure!(
        date.len() == 10
            && date.as_bytes()[4] == b'-'
            && date.as_bytes()[7] == b'-'
            && date
                .bytes()
                .enumerate()
                .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit()),
        "release date must have YYYY-MM-DD format"
    );
    let year = date[..4].parse::<u32>()?;
    let month = date[5..7].parse::<u32>()?;
    let day = date[8..].parse::<u32>()?;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let maximum = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    ensure!(
        year > 0 && day > 0 && day <= maximum,
        "release date `{date}` is not a valid calendar date"
    );
    Ok(())
}

fn validate_classified_notes(notes: &str) -> Result<()> {
    const CLASSES: &[&str] = &[
        "Added",
        "Changed",
        "Deprecated",
        "Removed",
        "Fixed",
        "Security",
    ];
    let mut heading_level = None;
    let mut heading = String::new();
    let mut current: Option<(String, bool)> = None;
    let mut classes = BTreeSet::new();
    for event in Parser::new(notes) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading_level = Some(level);
                heading.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if heading_level == Some(HeadingLevel::H3) {
                    if let Some((name, nonempty)) = current.take() {
                        ensure!(nonempty, "release category `{name}` has no notes");
                    }
                    ensure!(
                        CLASSES.contains(&heading.trim()),
                        "unknown release category `{heading}`; use {}",
                        CLASSES.join(", ")
                    );
                    ensure!(
                        classes.insert(heading.trim().to_owned()),
                        "duplicate release category `{heading}`"
                    );
                    current = Some((heading.trim().into(), false));
                } else {
                    ensure!(
                        current.is_some(),
                        "release notes must be classified under ### Added/Changed/Deprecated/Removed/Fixed/Security"
                    );
                }
                heading_level = None;
            }
            Event::Text(text) | Event::Code(text) if heading_level.is_some() => {
                heading.push_str(&text)
            }
            Event::Text(text) | Event::Code(text) if !text.trim().is_empty() => {
                let Some((_, nonempty)) = current.as_mut() else {
                    bail!(
                        "release notes must be classified under ### Added/Changed/Deprecated/Removed/Fixed/Security"
                    );
                };
                *nonempty = true;
            }
            _ => {}
        }
    }
    let (name, nonempty) =
        current.context("release entry must contain nonempty classified notes")?;
    ensure!(nonempty, "release category `{name}` has no notes");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn version(value: &str) -> Version {
        Version::parse(value).unwrap()
    }

    #[test]
    fn base_requires_a_full_hexadecimal_commit_id() {
        for value in ["a".repeat(40), "b".repeat(64), "ABCDEF".repeat(10) + "ABCD"] {
            validate_base(&value).unwrap();
        }
        for value in [
            "",
            "HEAD",
            "--help",
            "abc123",
            &"a".repeat(41),
            &"g".repeat(40),
        ] {
            assert!(validate_base(value).is_err(), "{value}");
        }
        let error =
            release_check(Path::new("missing-release-workspace"), Some("HEAD")).unwrap_err();
        assert!(error.to_string().contains("40- or 64-digit"));
    }

    #[test]
    fn notes_extract_only_the_exact_current_entry_body() {
        let body = "\n### Added\n\n- Retain **exact** Markdown.\n\n```markdown\n## [0.0.0] - 2000-01-01\n```\n\n";
        let changelog = format!(
            "# Changelog\n\n## [Unreleased]\n\n### Fixed\n\n- Future work.\n\n## [1.2.0] - 2024-02-29\n{body}## [1.1.0] - 2024-01-01\n\n### Removed\n\n- Previous release.\n"
        );
        let release = extract_entry(&changelog, &version("1.2.0")).unwrap();
        assert_eq!(release.notes, body);
        assert_eq!(release.date, "2024-02-29");
        assert!(!release.notes.contains("Future work"));
        assert!(!release.notes.contains("Previous release"));
    }

    #[test]
    fn missing_duplicate_and_unreleased_only_entries_are_refused() {
        for changelog in [
            "# Changelog\n\n## [Unreleased]\n\n### Added\n\n- Future release.\n",
            "## [1.1.0] - 2024-01-01\n\n### Fixed\n\n- Old release.\n",
            "## [1.2.0] - 2024-01-01\n\n### Fixed\n\n- First.\n\n## [1.2.0] - 2024-01-02\n\n### Fixed\n\n- Second.\n",
            "```markdown\n## [1.2.0] - 2024-01-01\n\n### Fixed\n- Example.\n```\n",
        ] {
            assert!(extract_entry(changelog, &version("1.2.0")).is_err());
        }
    }

    #[test]
    fn release_dates_are_real_calendar_dates() {
        for date in ["2024-02-29", "2000-02-29", "2026-10-29"] {
            validate_date(date).unwrap();
        }
        for date in [
            "2023-02-29",
            "1900-02-29",
            "2026-04-31",
            "2026-00-10",
            "2026-12-00",
            "0000-01-01",
            "2026-1-1",
            "2026-10-29 trailing",
        ] {
            assert!(validate_date(date).is_err(), "{date}");
        }
    }

    #[test]
    fn release_notes_need_classified_substantive_content() {
        for body in [
            "",
            "\n",
            "- Unclassified note.\n",
            "### Added\n\n",
            "### Notes\n\n- Unclassified.\n",
            "### Added\n\n<!-- later -->\n",
            "### Added\n\n#### Placeholder\n",
            "### Added\n\n### Fixed\n\n- Fixed.\n",
            "### Added\n\n- One.\n\n### Added\n\n- Two.\n",
        ] {
            assert!(validate_classified_notes(body).is_err(), "{body:?}");
        }
        validate_classified_notes(
            "### Added\n\n- Feature.\n\n### Security\n\n- Dependency update.\n",
        )
        .unwrap();
    }

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(release: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "fidryn-release-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("fidryn-core/src")).unwrap();
            fs::create_dir_all(path.join("fidryn-cli/src")).unwrap();
            fs::create_dir_all(path.join("empty-hooks")).unwrap();
            let fixture = Self(path);
            fixture.write(
                "fidryn-core/Cargo.toml",
                "[package]\nname = \"fidryn-core\"\nversion.workspace = true\nedition = \"2024\"\n",
            );
            fixture.write(
                "fidryn-core/src/lib.rs",
                "pub fn available() -> bool {true}\n",
            );
            fixture.write("fidryn-cli/Cargo.toml","[package]\nname = \"fidryn-cli\"\nversion.workspace = true\nedition = \"2024\"\n[dependencies]\nfidryn-core.workspace = true\n");
            fixture.write(
                "fidryn-cli/src/lib.rs",
                "pub fn available() -> bool {fidryn_core::available()}\n",
            );
            fixture.set_version(release);
            fixture
        }

        fn write(&self, path: &str, contents: &str) {
            fs::write(self.0.join(path), contents).unwrap();
        }

        fn set_version(&self, release: &str) {
            self.write("Cargo.toml",&format!("[workspace]\nresolver = \"2\"\nmembers = [\"fidryn-core\", \"fidryn-cli\"]\n[workspace.package]\nversion = \"{release}\"\n[workspace.dependencies]\nfidryn-core = {{path = \"fidryn-core\", version = \"{release}\"}}\n"));
            self.write("CHANGELOG.md",&format!("# Changelog\n\n## [{release}] - 2026-10-29\n\n### Changed\n\n- Updated the release fixture.\n"));
            self.lock();
        }

        fn lock(&self) {
            let output = Command::new(cargo_bin())
                .current_dir(&self.0)
                .args(["generate-lockfile", "--offline"])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        fn git(&self, args: &[&str]) -> String {
            let output = Command::new("git")
                .current_dir(&self.0)
                .args([
                    "-c",
                    "user.name=Release Tests",
                    "-c",
                    "user.email=release-tests@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "tag.gpgsign=false",
                ])
                .arg("-c")
                .arg(format!(
                    "core.hooksPath={}",
                    self.0.join("empty-hooks").display()
                ))
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap()
        }

        fn commit(&self) -> String {
            self.git(&["add", "."]);
            self.git(&["commit", "-q", "-m", "Release fixture"]);
            self.git(&["rev-parse", "HEAD"]).trim().into()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn coherent_workspace_and_notes_check_without_git_or_writes() {
        let fixture = Fixture::new("1.2.0");
        let before = fs::read(fixture.0.join("Cargo.lock")).unwrap();
        let release = release_check(&fixture.0, None).unwrap();
        assert_eq!(release.version, version("1.2.0"));
        assert_eq!(release_notes(&fixture.0).unwrap(), release.notes);
        assert_eq!(fs::read(fixture.0.join("Cargo.lock")).unwrap(), before);
        assert!(!fixture.0.join(".git").exists());
    }

    #[test]
    fn metadata_cannot_substitute_for_missing_or_empty_release_notes() {
        let fixture = Fixture::new("1.2.0");
        fs::remove_file(fixture.0.join("CHANGELOG.md")).unwrap();
        assert!(
            release_check(&fixture.0, None)
                .unwrap_err()
                .to_string()
                .contains("CHANGELOG.md")
        );
        fixture.write("CHANGELOG.md", "## [1.2.0] - 2026-10-29\n\n### Added\n");
        assert!(release_check(&fixture.0, None).is_err());
    }

    #[test]
    fn workspace_version_is_required_without_falling_back_to_metadata() {
        let fixture = Fixture::new("1.2.0");
        let manifest = fs::read_to_string(fixture.0.join("Cargo.toml")).unwrap();
        fixture.write(
            "Cargo.toml",
            &manifest.replace(
                "[workspace.package]\nversion = \"1.2.0\"\n",
                "[workspace.package]\n",
            ),
        );
        let error = release_check(&fixture.0, None).unwrap_err();
        assert!(error.to_string().contains("workspace.package.version"));
    }

    #[test]
    fn workspace_and_member_package_versions_must_match_cli() {
        let fixture = Fixture::new("1.2.0");
        fixture.write("fidryn-cli/Cargo.toml","[package]\nname = \"fidryn-cli\"\nversion = \"1.3.0\"\nedition = \"2024\"\n[dependencies]\nfidryn-core.workspace = true\n");
        fixture.lock();
        assert!(
            release_check(&fixture.0, None)
                .unwrap_err()
                .to_string()
                .contains("does not match fidryn-cli")
        );
    }

    #[test]
    fn broad_or_missing_internal_dependency_versions_are_refused() {
        let fixture = Fixture::new("1.2.0");
        for dependency in [
            "{path=\"fidryn-core\",version=\"*\"}",
            "{path=\"fidryn-core\"}",
        ] {
            let root = fs::read_to_string(fixture.0.join("Cargo.toml")).unwrap();
            let prefix = root.split("[workspace.dependencies]").next().unwrap();
            fixture.write(
                "Cargo.toml",
                &format!("{prefix}[workspace.dependencies]\nfidryn-core = {dependency}\n"),
            );
            fixture.lock();
            let error = release_check(&fixture.0, None).unwrap_err();
            assert!(
                error.to_string().contains("internal dependency"),
                "{error:#}"
            );
        }
    }

    #[test]
    fn renamed_target_specific_internal_dependencies_are_checked() {
        let fixture = Fixture::new("1.2.0");
        fixture.write("fidryn-cli/Cargo.toml","[package]\nname=\"fidryn-cli\"\nversion.workspace=true\nedition=\"2024\"\n[target.'cfg(any())'.build-dependencies]\ncore_alias={package=\"fidryn-core\",path=\"../fidryn-core\",version=\"*\"}\n");
        fixture.lock();
        let error = release_check(&fixture.0, None).unwrap_err();
        assert!(error.to_string().contains("core_alias"), "{error:#}");
    }

    #[test]
    fn stale_lockfile_version_is_refused_without_rewriting() {
        let fixture = Fixture::new("1.2.0");
        let lock = fs::read_to_string(fixture.0.join("Cargo.lock"))
            .unwrap()
            .replace("version = \"1.2.0\"", "version = \"1.1.0\"");
        fixture.write("Cargo.lock", &lock);
        assert!(release_check(&fixture.0, None).is_err());
        assert_eq!(
            fs::read_to_string(fixture.0.join("Cargo.lock")).unwrap(),
            lock
        );
    }

    #[test]
    fn base_requires_a_strict_version_increase() {
        let fixture = Fixture::new("1.2.0");
        fixture.git(&["init", "-q"]);
        let base = fixture.commit();
        assert!(
            release_check(&fixture.0, Some(&base))
                .unwrap_err()
                .to_string()
                .contains("strictly increase")
        );
        fixture.set_version("1.3.0");
        assert_eq!(
            release_check(&fixture.0, Some(&base)).unwrap().version,
            version("1.3.0")
        );
        fixture.set_version("1.1.0");
        assert!(release_check(&fixture.0, Some(&base)).is_err());
    }

    #[test]
    fn build_metadata_only_does_not_increase_release_precedence() {
        let fixture = Fixture::new("1.2.0+first");
        fixture.git(&["init", "-q"]);
        let base = fixture.commit();
        fixture.set_version("1.2.0+second");
        assert!(
            release_check(&fixture.0, Some(&base))
                .unwrap_err()
                .to_string()
                .contains("strictly increase")
        );
    }

    #[test]
    fn stale_version_tags_refuse_but_annotated_head_tags_allow_reruns() {
        let fixture = Fixture::new("1.2.0");
        fixture.git(&["init", "-q"]);
        let base = fixture.commit();
        fixture.set_version("1.3.0");
        fixture.commit();
        fixture.git(&["tag", "v1.3.0", &base]);
        assert!(
            release_check(&fixture.0, Some(&base))
                .unwrap_err()
                .to_string()
                .contains("another commit")
        );
        fixture.git(&["tag", "-d", "v1.3.0"]);
        fixture.git(&["tag", "-a", "v1.3.0", "-m", "Release fixture"]);
        release_check(&fixture.0, Some(&base)).unwrap();
    }
}
