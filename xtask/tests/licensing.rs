//! The license text, the package metadata and the documents agree.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MIT: &str = "MIT License

Copyright (c) 2026 Bryan Gass

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_owned()
}

#[test]
fn the_license_file_is_the_mit_text() {
    let text = fs::read_to_string(root().join("LICENSE")).expect("LICENSE");
    assert_eq!(text.replace("\r\n", "\n"), MIT);
}

#[test]
fn every_workspace_package_declares_mit() {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(root().join("Cargo.toml"))
        .no_deps()
        .exec()
        .expect("cargo metadata");
    let packages = metadata.workspace_packages();
    assert!(packages.len() >= 17, "only {} packages", packages.len());
    for package in packages {
        assert_eq!(package.license.as_deref(), Some("MIT"), "{}", package.name);
        assert!(package.license_file.is_none(), "{}", package.name);
    }
}

#[test]
fn nothing_still_states_the_proprietary_terms() {
    for file in [
        "LICENSE",
        "LICENSING.md",
        "README.md",
        "deny.toml",
        "docs/README.md",
        "docs/contributing.md",
        "docs/releases.md",
    ] {
        let text = fs::read_to_string(root().join(file)).expect(file);
        for needle in [
            "roprietary",
            "PROPRIETARY",
            "LicenseRef-",
            "All Rights Reserved",
            "freectrl",
        ] {
            assert!(!text.contains(needle), "{file} still contains {needle}");
        }
    }
    assert!(!root().join("LICENSES/prior-project-LICENSE.txt").exists());
}

/// The document's text with every run of whitespace, line breaks included,
/// collapsed to one space, so a phrase matches however the paragraph wraps.
fn words(file: &str) -> String {
    let text = fs::read_to_string(root().join(file)).expect(file);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn contributions_are_accepted_under_the_project_license() {
    let licensing = words("LICENSING.md");
    let contributing = words("docs/contributing.md");
    for (file, text) in [
        ("LICENSING.md", &licensing),
        ("docs/contributing.md", &contributing),
    ] {
        assert!(
            !text.contains("rights agreement"),
            "{file} still asks for a rights agreement"
        );
    }
    assert!(
        licensing.contains("Contributions are accepted under the MIT License"),
        "LICENSING.md does not state the contribution terms"
    );
}

#[test]
fn every_source_fixture_labels_itself() {
    let output = Command::new("git")
        .current_dir(root())
        .args(["ls-files", "-z", "--", ":(glob)examples/**/sources/**"])
        .output()
        .expect("run git ls-files");
    assert!(
        output.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let listing = String::from_utf8(output.stdout).expect("UTF-8 paths");
    let mut checked = 0;
    for file in listing.split('\0').filter(|file| !file.is_empty()) {
        if Path::new(file).extension().is_some_and(|ext| ext == "json") {
            continue;
        }
        let text = fs::read_to_string(root().join(file)).expect(file);
        let text = text.to_lowercase();
        assert!(
            text.contains("fixture") || text.contains("not an official edition"),
            "{file} does not label itself as a fixture or as not an official edition"
        );
        checked += 1;
    }
    assert!(checked > 0, "no source fixture found under examples/");
}
