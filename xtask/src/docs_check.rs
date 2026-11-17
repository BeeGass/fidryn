//! Checks what the Markdown documents claim: links resolve, complete module
//! samples pass `fidryn check`, and the `fidryn` commands shown run with the
//! exit status the document declares.

use anyhow::{Context, Result, bail};
use fidryn_core::SourceManifest;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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

/// Tokens that mean the shell would do something the check does not.
const SHELL_OPERATORS: [&str; 7] = [">", ">>", "<", "|", "&&", "||", ";"];

/// Words that run the CLI from a checkout; what follows them are its arguments.
const CARGO_RUN_PREFIX: [&str; 5] = ["cargo", "run", "-p", "fidryn-cli", "--"];

/// How long one documented command may run before the check stops it.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

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
    /// The words after `fidryn`, or after `cargo run -p fidryn-cli --`.
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

/// What a block's tag asks of the check.
#[derive(Debug, PartialEq, Eq)]
enum BlockTag {
    /// `sh` or `sh exit=N`: run its commands and expect this exit status.
    Sh(i32),
    /// A first word of `sh` followed by anything other than `exit=N`.
    MalformedSh,
    /// `text`: show its commands unchecked.
    Text,
    Other,
}

impl Block {
    fn tag(&self) -> BlockTag {
        let words: Vec<&str> = self.info.split_ascii_whitespace().collect();
        match words.as_slice() {
            ["sh"] => BlockTag::Sh(0),
            ["sh", second] => match second.strip_prefix("exit=").map(str::parse) {
                Some(Ok(exit)) => BlockTag::Sh(exit),
                _ => BlockTag::MalformedSh,
            },
            ["sh", ..] => BlockTag::MalformedSh,
            ["text", ..] => BlockTag::Text,
            _ => BlockTag::Other,
        }
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

/// The first line of `text` that is not blank.
fn first_line(text: &str) -> Option<&str> {
    text.lines().find(|line| !line.trim().is_empty())
}

/// Whether the first line of `source` that is not blank is the marker of
/// edition 2.
fn is_edition_2(source: &str) -> bool {
    first_line(source).is_some_and(|line| line.trim() == "edition 2")
}

/// The untagged and `fidryn` blocks that are complete modules: those whose
/// first line that is not blank begins a module of the first edition or is
/// the marker of edition 2.
pub fn complete_modules(markdown: &str) -> Vec<(usize, String)> {
    fenced_blocks(markdown)
        .into_iter()
        .filter(|block| matches!(block.info.trim(), "" | "fidryn"))
        .filter(|block| {
            first_line(&block.text).is_some_and(|line| line.starts_with("module "))
                || is_edition_2(&block.text)
        })
        .map(|block| (block.line, block.text))
        .collect()
}

/// A module of edition 2 gets the checks of one module; any other is
/// compiled as `fidryn check` compiles it.
pub fn module_problems(file: &str, markdown: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for (line, source) in complete_modules(markdown) {
        let checked = if is_edition_2(&source) {
            fidryn_check::e2::check_source(&source).map(|_| ())
        } else {
            fidryn_cli::compile_source(&source, &SourceManifest::default()).map(|_| ())
        };
        if let Err(diagnostics) = checked {
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

/// The arguments of a `fidryn` command line: the words after `fidryn`, or
/// after `cargo run -p fidryn-cli --`. `None` for any other line.
fn command_args(words: &[String]) -> Option<Vec<String>> {
    if words.first().is_some_and(|w| w == "fidryn") {
        return Some(words[1..].to_vec());
    }
    let prefix = CARGO_RUN_PREFIX.len();
    let runs_the_cli = words.len() >= prefix
        && words
            .iter()
            .zip(CARGO_RUN_PREFIX)
            .all(|(word, expected)| word == expected);
    runs_the_cli.then(|| words[prefix..].to_vec())
}

/// The arguments of each `fidryn` command line of a block.
fn fidryn_invocations(text: &str) -> Vec<Vec<String>> {
    logical_lines(text)
        .iter()
        .filter_map(|line| command_args(&split_words(line)))
        .collect()
}

/// A command with `ui` as any of its words is taken to start the mill, which
/// runs until it is stopped, so the check does not run it.
fn starts_the_server(args: &[String]) -> bool {
    args.iter().any(|word| word == "ui")
}

pub fn doc_commands(markdown: &str) -> Vec<DocCommand> {
    let mut commands = Vec::new();
    for block in fenced_blocks(markdown) {
        let BlockTag::Sh(exit) = block.tag() else {
            continue;
        };
        for args in fidryn_invocations(&block.text) {
            if starts_the_server(&args) {
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
    let mut problems = Vec::new();
    for block in fenced_blocks(markdown) {
        match block.tag() {
            BlockTag::Text => {}
            BlockTag::Sh(_) => {
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
            BlockTag::MalformedSh => problems.push(format!(
                "{file}:{}: block tag `{}` is not `sh` or `sh exit=N` with N an integer",
                block.line,
                block.info.trim()
            )),
            BlockTag::Other => {
                if !fidryn_invocations(&block.text).is_empty() {
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

/// How a documented command ended.
enum Ran {
    Exited {
        status: i32,
        stderr: String,
    },
    /// Still running when the timeout expired; it was killed.
    TimedOut,
}

/// Runs `command` with no stdin and its stdout discarded, killing it if it
/// has not exited within `timeout`.
fn run_with_timeout(mut command: Command, timeout: Duration) -> std::io::Result<Ran> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut pipe = child.stderr.take().expect("stderr is piped");
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            child.wait()?;
            break None;
        }
        thread::sleep(Duration::from_millis(5));
    };
    let stderr = reader.join().unwrap_or_default();
    Ok(match status {
        Some(status) => Ran::Exited {
            status: status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        },
        None => Ran::TimedOut,
    })
}

pub fn check_commands(root: &Path, fidryn: &Path) -> Result<()> {
    let mut problems = Vec::new();
    for file in markdown_files(root)? {
        let markdown = read_document(root, &file)?;
        for command in doc_commands(&markdown) {
            let mut process = Command::new(fidryn);
            process.current_dir(root).args(&command.args);
            let ran = run_with_timeout(process, COMMAND_TIMEOUT)
                .with_context(|| format!("run {}", fidryn.display()))?;
            match ran {
                Ran::Exited { status, .. } if status == command.exit => {}
                Ran::Exited { status, stderr } => {
                    let first = stderr.lines().next().unwrap_or("");
                    problems.push(format!(
                        "{file}:{}: `fidryn {}` exited with status {status}, expected {}\n    {first}",
                        command.line,
                        command.args.join(" "),
                        command.exit
                    ));
                }
                Ran::TimedOut => problems.push(format!(
                    "{file}:{}: `fidryn {}` did not exit within {} seconds and was stopped",
                    command.line,
                    command.args.join(" "),
                    COMMAND_TIMEOUT.as_secs()
                )),
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
    fn complete_modules_are_fenced_blocks_that_start_with_module_and_crlf_is_normalized() {
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

    #[test]
    fn the_documentation_check_reads_edition_2_blocks() {
        let unit = include_str!("../../crates/fidryn-check/tests/fixtures/e2/unit.fr");
        let changed = unit.replacen("a small device", "a small devise", 1);
        assert_ne!(changed, unit);
        let md = format!(
            "An invented module of edition 2:\n\n```fidryn\n{unit}```\n\n\
             The same, with one letter changed:\n\n```fidryn\n\n{changed}```\n"
        );
        assert_eq!(complete_modules(&md).len(), 2);
        let second = md[..md.rfind("```fidryn").expect("a second block")]
            .matches('\n')
            .count()
            + 1;
        let problems = module_problems("docs/x.md", &md);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].starts_with(&format!(
                "docs/x.md:{second}: module does not pass `fidryn check`: E710"
            )),
            "{}",
            problems[0]
        );
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

    #[test]
    fn a_command_with_ui_as_any_word_is_the_server_and_is_not_run() {
        let md = "```sh\nfidryn --source-root . ui\nfidryn ui --no-open\ncargo run -p fidryn-cli -- ui --port 8751\nfidryn check a.fr\n```\n";
        assert_eq!(
            doc_commands(md),
            [DocCommand {
                line: 1,
                args: words(&["check", "a.fr"]),
                exit: 0
            }]
        );
    }

    #[test]
    fn cargo_run_of_the_cli_is_a_fidryn_command() {
        let md = "```sh\ncargo run -p fidryn-cli -- check a.fr\ncargo run -p fidryn-cli -- run b.fr \\\n    --query q\ncargo run -p fidryn-cli --\ncargo run -p other -- check c.fr\ncargo test --workspace --locked\n```\n\n```sh exit=1\ncargo run -p fidryn-cli -- check broken.fr\n```\n";
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
                    args: words(&["run", "b.fr", "--query", "q"]),
                    exit: 0
                },
                DocCommand {
                    line: 1,
                    args: Vec::new(),
                    exit: 0
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
    fn a_cargo_run_command_must_be_tagged_and_must_not_use_shell_operators() {
        let md = "```\ncargo run -p fidryn-cli -- check untagged.fr\n```\n\n```sh\ncargo run -p fidryn-cli -- run a.fr --query q > out.json\n```\n\n```text\ncargo run -p fidryn-cli -- <subcommand> ...\n```\n\n```\ncargo run -p other -- check x.fr\n```\n";
        assert_eq!(
            command_block_problems("docs/x.md", md),
            [
                "docs/x.md:1: block shows a `fidryn` command; tag it `sh` to have the check run it or `text` to show it unchecked",
                "docs/x.md:5: `fidryn run a.fr --query q > out.json` uses the shell operator `>`; the check runs `fidryn` directly, so split the block or tag it `text`",
            ]
        );
    }

    #[test]
    fn an_sh_tag_other_than_sh_or_sh_exit_n_is_reported() {
        let md = "```sh exit=abc\nfidryn check a.fr\n```\n\n```sh exit 1\nfidryn check b.fr\n```\n\n```sh exit=1 more\n```\n\n```sh exit=1\nfidryn check c.fr\n```\n\n```sh\nfidryn check d.fr\n```\n\n```shell\ncargo test\n```\n";
        assert_eq!(
            command_block_problems("docs/x.md", md),
            [
                "docs/x.md:1: block tag `sh exit=abc` is not `sh` or `sh exit=N` with N an integer",
                "docs/x.md:5: block tag `sh exit 1` is not `sh` or `sh exit=N` with N an integer",
                "docs/x.md:9: block tag `sh exit=1 more` is not `sh` or `sh exit=N` with N an integer",
            ]
        );
    }
}
