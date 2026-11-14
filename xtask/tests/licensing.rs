//! The license text, the package metadata and the documents agree.

use std::fs;
use std::path::{Path, PathBuf};

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
