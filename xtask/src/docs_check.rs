//! Checks what the Markdown documents claim: links resolve, complete module
//! samples pass `fidryn check`, and the `fidryn` commands shown run with the
//! exit status the document declares.

use anyhow::{Context, Result, bail};
use fidryn_core::SourceManifest;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const MARKDOWN_ROOTS: [&str; 9] = [
    "README.md",
    "LICENSING.md",
    "CHANGELOG.md",
    "SECURITY.md",
    "docs",
    "examples",
    "packages",
    "conformance",
    "site/README.md",
];

/// Documents whose command blocks are not yet tagged. Task 6 empties it.
pub const COMMAND_RULE_PENDING: &[&str] = &["docs/examples.md"];

/// Tokens that mean the shell would do something the check does not.
const SHELL_OPERATORS: [&str; 7] = [">", ">>", "<", "|", "&&", "||", ";"];

#[derive(Debug, clap::Args)]
pub struct DocsArgs {
    /// Check links and module samples only; do not build or run the binary
    #[arg(long)]
    pub skip_commands: bool,
}

/// One `fidryn` invocation shown in a checked block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocCommand {
    /// 1-based line of the block's opening fence.
    pub line: usize,
    /// The words after `fidryn`.
    pub args: Vec<String>,
    /// The exit status the block declares: 0 for `sh`, N for `sh exit=N`.
    pub exit: i32,
}

/// A fenced code block of a document.
struct Block {
    /// 1-based line of the opening fence.
    line: usize,
    info: String,
    /// Content with `\n` line endings.
    text: String,
}

impl Block {
    fn first_word(&self) -> &str {
        self.info.split_ascii_whitespace().next().unwrap_or("")
    }
}

fn fenced_blocks(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut open: Option<Block> = None;
    for (event, range) in Parser::new(markdown).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                let line = markdown[..range.start].matches('\n').count() + 1;
                open = Some(Block {
                    line,
                    info: info.to_string(),
                    text: String::new(),
                });
            }
            Event::Text(text) => {
                if let Some(block) = open.as_mut() {
                    block.text.push_str(&text.replace("\r\n", "\n"));
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(block) = open.take() {
                    blocks.push(block);
                }
            }
            _ => {}
        }
    }
    blocks
}

fn collect_markdown(dir: &Path, rel: &str, out: &mut Vec<String>) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("read directory {rel}"))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("read directory {rel}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = format!("{rel}/{name}");
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, &child, out)?;
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(child);
        }
    }
    Ok(())
}

pub fn markdown_files(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in MARKDOWN_ROOTS {
        let path = root.join(entry);
        if path.is_dir() {
            collect_markdown(&path, entry, &mut files)?;
        } else if path.is_file() {
            files.push(entry.to_owned());
        }
    }
    files.sort();
    Ok(files)
}

fn has_url_scheme(dest: &str) -> bool {
    let Some((scheme, _)) = dest.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

pub fn broken_links(root: &Path, file: &str, markdown: &str) -> Vec<String> {
    let base = root.join(file).parent().map(Path::to_path_buf);
    let base = base.unwrap_or_else(|| root.to_path_buf());
    let mut problems = Vec::new();
    for event in Parser::new_ext(markdown, Options::ENABLE_TABLES) {
        let (Event::Start(Tag::Link { dest_url, .. }) | Event::Start(Tag::Image { dest_url, .. })) =
            event
        else {
            continue;
        };
        let dest = dest_url.as_ref();
        if dest.is_empty() || dest.starts_with('#') || dest.starts_with('/') || has_url_scheme(dest)
        {
            continue;
        }
        let path = dest.split(['#', '?']).next().unwrap_or("");
        if path.is_empty() {
            continue;
        }
        if !base.join(path).exists() {
            problems.push(format!("{file}: link target `{dest}` does not exist"));
        }
    }
    problems
}

pub fn complete_modules(markdown: &str) -> Vec<(usize, String)> {
    fenced_blocks(markdown)
        .into_iter()
        .filter(|block| matches!(block.info.trim(), "" | "fidryn"))
        .filter(|block| {
            block
                .text
                .lines()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| line.starts_with("module "))
        })
        .map(|block| (block.line, block.text))
        .collect()
}

pub fn module_problems(file: &str, markdown: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for (line, source) in complete_modules(markdown) {
        if let Err(diagnostics) = fidryn_cli::compile_source(&source, &SourceManifest::default()) {
            let shown: Vec<String> = diagnostics.iter().map(ToString::to_string).collect();
            problems.push(format!(
                "{file}:{line}: module does not pass `fidryn check`: {}",
                shown.join("; ")
            ));
        }
    }
    problems
}

/// Split a shell-like line into words. Single or double quotes group a word
/// and are removed; there is no escape processing.
fn split_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => current.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                in_word = true;
            }
            None if c.is_ascii_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            None => {
                current.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(current);
    }
    words
}

/// Logical command lines of a block: lines ending in `\` continue.
fn logical_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut pending = String::new();
    for raw in text.lines() {
        let trimmed = raw.trim_end();
        if let Some(stripped) = trimmed.strip_suffix('\\') {
            pending.push_str(stripped);
            pending.push(' ');
        } else {
            pending.push_str(trimmed);
            lines.push(std::mem::take(&mut pending));
        }
    }
    if !pending.is_empty() {
        lines.push(pending);
    }
    lines
}

/// The words after `fidryn` for each `fidryn` command line of a block.
fn fidryn_invocations(text: &str) -> Vec<Vec<String>> {
    logical_lines(text)
        .iter()
        .map(|line| split_words(line))
        .filter(|words| words.first().is_some_and(|w| w == "fidryn"))
        .map(|words| words[1..].to_vec())
        .collect()
}

fn declared_exit(info: &str) -> i32 {
    info.split_ascii_whitespace()
        .skip(1)
        .find_map(|word| word.strip_prefix("exit=")?.parse().ok())
        .unwrap_or(0)
}

pub fn doc_commands(markdown: &str) -> Vec<DocCommand> {
    let mut commands = Vec::new();
    for block in fenced_blocks(markdown) {
        if block.first_word() != "sh" {
            continue;
        }
        let exit = declared_exit(&block.info);
        for args in fidryn_invocations(&block.text) {
            if args.first().is_some_and(|w| w == "ui") {
                continue;
            }
            commands.push(DocCommand {
                line: block.line,
                args,
                exit,
            });
        }
    }
    commands
}

pub fn command_block_problems(file: &str, markdown: &str) -> Vec<String> {
    if COMMAND_RULE_PENDING.contains(&file) {
        return Vec::new();
    }
    let mut problems = Vec::new();
    for block in fenced_blocks(markdown) {
        match block.first_word() {
            "text" => {}
            "sh" => {
                for args in fidryn_invocations(&block.text) {
                    if let Some(op) = args.iter().find(|w| SHELL_OPERATORS.contains(&w.as_str())) {
                        problems.push(format!(
                            "{file}:{}: `fidryn {}` uses the shell operator `{op}`; the check runs `fidryn` directly, so split the block or tag it `text`",
                            block.line,
                            args.join(" ")
                        ));
                    }
                }
            }
            _ => {
                let shows_command = block
                    .text
                    .lines()
                    .any(|line| line.split_ascii_whitespace().next() == Some("fidryn"));
                if shows_command {
                    problems.push(format!(
                        "{file}:{}: block shows a `fidryn` command; tag it `sh` to have the check run it or `text` to show it unchecked",
                        block.line
                    ));
                }
            }
        }
    }
    problems
}

fn read_document(root: &Path, file: &str) -> Result<String> {
    fs::read_to_string(root.join(file)).with_context(|| format!("read {file}"))
}

pub fn check_static(root: &Path) -> Result<()> {
    let mut problems = Vec::new();
    for file in markdown_files(root)? {
        let markdown = read_document(root, &file)?;
        problems.extend(broken_links(root, &file, &markdown));
        problems.extend(module_problems(&file, &markdown));
        problems.extend(command_block_problems(&file, &markdown));
    }
    if !problems.is_empty() {
        bail!("documentation problems:\n{}", problems.join("\n"));
    }
    Ok(())
}

pub fn check_commands(root: &Path, fidryn: &Path) -> Result<()> {
    let mut problems = Vec::new();
    for file in markdown_files(root)? {
        let markdown = read_document(root, &file)?;
        for command in doc_commands(&markdown) {
            let output = Command::new(fidryn)
                .current_dir(root)
                .args(&command.args)
                .stdin(Stdio::null())
                .output()
                .with_context(|| format!("run {}", fidryn.display()))?;
            let actual = output.status.code().unwrap_or(-1);
            if actual != command.exit {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let first = stderr.lines().next().unwrap_or("");
                problems.push(format!(
                    "{file}:{}: `fidryn {}` exited with status {actual}, expected {}\n    {first}",
                    command.line,
                    command.args.join(" "),
                    command.exit
                ));
            }
        }
    }
    if !problems.is_empty() {
        bail!(
            "documented commands did not behave as shown:\n{}",
            problems.join("\n")
        );
    }
    Ok(())
}

pub fn run(args: DocsArgs) -> Result<()> {
    let root: PathBuf = crate::workspace::workspace_root();
    check_static(&root)?;
    if !args.skip_commands {
        check_commands(&root, &crate::workspace::fidryn_binary()?)?;
    }
    println!("ok");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{module, write};

    fn scratch(name: &str) -> PathBuf {
        crate::test_support::scratch("fidryn-xtask-docs-", name)
    }

    #[test]
    fn markdown_files_are_sorted_with_forward_slashes() {
        let root = scratch("files");
        write(&root, "README.md", "x");
        write(&root, "docs/internal/b.md", "x");
        write(&root, "docs/a.md", "x");
        write(&root, "docs/notes.txt", "x");
        write(&root, "site/README.md", "x");
        write(&root, "site/docs/generated.md", "x");
        assert_eq!(
            markdown_files(&root).unwrap(),
            [
                "README.md",
                "docs/a.md",
                "docs/internal/b.md",
                "site/README.md"
            ]
        );
    }

    #[test]
    fn links_resolve_relative_to_the_file_and_ignore_urls_fragments_and_code() {
        let root = scratch("links");
        write(&root, "docs/a.md", "x");
        write(&root, "examples/trust/x.fr", "x");
        let md = "[ok](a.md) [frag](a.md#part) [dir](../examples/trust/) [up](../docs/a.md) [ref]\n\
                  [web](https://example.com/x.md) [mail](mailto:a@b.c) [anchor](#here) [site](/docs/cli)\n\
                  `[code](missing-in-code.md)`\n\n```\n[fenced](missing-in-fence.md)\n```\n\n\
                  [gone](missing.md) ![img](missing.png) [sub](internal/missing.md#x)\n\n\
                  [ref]: missing-ref.md\n";
        assert_eq!(
            broken_links(&root, "docs/b.md", md),
            [
                "docs/b.md: link target `missing-ref.md` does not exist",
                "docs/b.md: link target `missing.md` does not exist",
                "docs/b.md: link target `missing.png` does not exist",
                "docs/b.md: link target `internal/missing.md#x` does not exist",
            ]
        );
    }

    #[test]
    fn complete_modules_are_fenced_blocks_that_start_with_module_and_crlf_is_normalised() {
        let md = "intro\r\n\r\n```\r\nmodule A version \"1\" {\r\n}\r\n```\r\n\r\n```text\r\nmodule NotChecked version \"1\" {\r\n```\r\n\r\n```\r\nquery q() -> Int { return 7 }\r\n```\r\n";
        assert_eq!(
            complete_modules(md),
            [(3, "module A version \"1\" {\n}\n".to_owned())]
        );
    }

    #[test]
    fn a_module_that_does_not_check_is_reported_with_file_line_and_code() {
        let bad = format!(
            "text\n\n```\n{}```\n",
            module("Docs.Bad", "query q() -> Int { return true }")
        );
        let problems = module_problems("docs/x.md", &bad);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].starts_with("docs/x.md:3: module does not pass `fidryn check`: E210"),
            "{}",
            problems[0]
        );
        let good = format!(
            "```\n{}```\n",
            module("Docs.Ok", "query q() -> Int { return 7 }")
        );
        assert!(module_problems("docs/x.md", &good).is_empty());
    }

    fn words(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn commands_come_from_sh_blocks_with_the_exit_status_they_declare() {
        let md = "```sh\nfidryn check a.fr\nfidryn run b.fr \\\n  --query q \\\n  --arg 'note=two words'\ncargo test\nfidryn ui --no-open\n```\n\n```sh exit=1\nfidryn check broken.fr\n```\n\n```text\nfidryn fmt PATH\n```\n";
        assert_eq!(
            doc_commands(md),
            [
                DocCommand {
                    line: 1,
                    args: words(&["check", "a.fr"]),
                    exit: 0
                },
                DocCommand {
                    line: 1,
                    args: words(&["run", "b.fr", "--query", "q", "--arg", "note=two words"]),
                    exit: 0,
                },
                DocCommand {
                    line: 10,
                    args: words(&["check", "broken.fr"]),
                    exit: 1
                },
            ]
        );
    }

    #[test]
    fn a_block_that_shows_a_command_must_say_whether_it_is_checked() {
        let md = "```\nfidryn check untagged.fr\n```\n\n```sh\nfidryn run a.fr --query q > out.json\n```\n\n```text\nfidryn check PATH\n```\n\n```\nmodule A version \"1\" {\n}\n```\n\n```console\n$ cargo test\n```\n";
        assert_eq!(
            command_block_problems("docs/x.md", md),
            [
                "docs/x.md:1: block shows a `fidryn` command; tag it `sh` to have the check run it or `text` to show it unchecked",
                "docs/x.md:5: `fidryn run a.fr --query q > out.json` uses the shell operator `>`; the check runs `fidryn` directly, so split the block or tag it `text`",
            ]
        );
        assert!(command_block_problems("docs/examples.md", md).is_empty());
    }

    #[test]
    fn a_semicolon_between_commands_is_a_shell_operator() {
        let md = "```sh\nfidryn check a.fr ; fidryn check b.fr\n```\n";
        assert_eq!(
            command_block_problems("docs/x.md", md),
            [
                "docs/x.md:1: `fidryn check a.fr ; fidryn check b.fr` uses the shell operator `;`; the check runs `fidryn` directly, so split the block or tag it `text`"
            ]
        );
    }
}
