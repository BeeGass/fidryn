//! The landing specimen: four real runs, evaluated when the site is built
//! through the same library path as `fidryn run`.

use super::highlight::{self, Keywords, Lang};
use anyhow::{Context, Result, anyhow, bail};
use fidryn_cli::{CaseInput, RunRequest, run_report_text};
use fidryn_syntax::{TokenKind, lex};
use std::fs;
use std::path::Path;

/// One specimen run, ready to lay out.
pub struct Run {
    /// Tab and panel id suffix: `trust-open`, `trust-court`, `gate-q`, `gate-r`.
    pub id: &'static str,
    /// Tab label.
    pub label: &'static str,
    /// The equivalent `fidryn run …` command line, with repo-relative paths.
    pub command: String,
    /// The module, or an excerpt of it, with the file's real line numbers.
    pub source_html: String,
    /// The exact case JSON that was evaluated.
    pub case_html: String,
    /// Outcome kind as the report spells it, for example `contingent`.
    pub kind: String,
    /// The report in sentences, worded as the mill words them.
    pub opinion: Vec<String>,
}

const TRUST: &str = "examples/trust/bryan-revocable-trust.fr";
const GATE: &str = "tests/programs/require-gate.fr";

/// The empty case record, pretty-printed exactly as the mill's samples serve it.
const EMPTY_CASE: &str =
    "{\n  \"schema\": \"fidryn.case-record/v0.1\",\n  \"admissibleCompletions\": {}\n}\n";

/// Where the Getting started guide writes the empty case record.
const EMPTY_CASE_PATH: &str = "/tmp/fidryn-empty-case.json";

/// First lines of the two trust blocks the specimen shows.
const TRUST_EXCERPT: &[&str] = &[
    "interpretation_family SuccessorEligibility",
    "query acting_trustee",
];

struct Spec {
    id: &'static str,
    label: &'static str,
    module: &'static str,
    /// First lines of the blocks to show; empty shows the whole module.
    excerpt: &'static [&'static str],
    /// Case file under the repository root; `None` is the empty case record.
    case: Option<&'static str>,
    query: &'static str,
    /// Used for both `--valid-at` and `--known-at`.
    at: &'static str,
}

const SPECS: &[Spec] = &[
    Spec {
        id: "trust-open",
        label: "Trust, open eligibility",
        module: TRUST,
        excerpt: TRUST_EXCERPT,
        case: Some("examples/trust/cases/two-certificates-open-eligibility.json"),
        query: "acting_trustee",
        at: "2034-03-01T09:00:00Z",
    },
    Spec {
        id: "trust-court",
        label: "Trust, court selects I2",
        module: TRUST,
        excerpt: TRUST_EXCERPT,
        case: Some("examples/trust/cases/court-selects-i2.json"),
        query: "acting_trustee",
        at: "2034-03-01T09:00:00Z",
    },
    Spec {
        id: "gate-q",
        label: "require-gate, query q",
        module: GATE,
        excerpt: &[],
        case: None,
        query: "q",
        at: "2026-09-17T12:00:00Z",
    },
    Spec {
        id: "gate-r",
        label: "require-gate, query r",
        module: GATE,
        excerpt: &[],
        case: None,
        query: "r",
        at: "2026-09-17T12:00:00Z",
    },
];

/// Evaluate the four specimen runs, in tab order.
pub fn runs(root: &Path, kw: &Keywords) -> Result<Vec<Run>> {
    SPECS.iter().map(|spec| run(root, kw, spec)).collect()
}

fn run(root: &Path, kw: &Keywords, spec: &Spec) -> Result<Run> {
    let module_path = root.join(spec.module);
    let src = fs::read_to_string(&module_path)
        .with_context(|| format!("read {}", module_path.display()))?;
    let segments = if spec.excerpt.is_empty() {
        vec![(1, src.trim_end().to_owned())]
    } else {
        let blocks = excerpt(&src, spec.excerpt);
        if blocks.len() != spec.excerpt.len() {
            bail!(
                "{}: expected {} blocks starting with {:?}, found {}",
                spec.module,
                spec.excerpt.len(),
                spec.excerpt,
                blocks.len()
            );
        }
        blocks
    };
    let (case_text, case_caption) = match spec.case {
        Some(rel) => {
            let path = root.join(rel);
            let text =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            (text, rel)
        }
        None => (EMPTY_CASE.to_owned(), "empty case record"),
    };
    let command = format!(
        "fidryn run {} --query {} --case {} --valid-at {at} --known-at {at}",
        spec.module,
        spec.query,
        spec.case.unwrap_or(EMPTY_CASE_PATH),
        at = spec.at,
    );
    let report_text = run_report_text(&RunRequest {
        path: &module_path,
        query: spec.query,
        case: CaseInput::Json(&case_text),
        valid_at: spec.at,
        known_at: spec.at,
        args: &[],
        scenario: false,
    })
    .map_err(|err| anyhow!("specimen `{command}` failed: {err}"))?;
    let report: serde_json::Value = serde_json::from_str(&report_text)
        .with_context(|| format!("specimen `{command}` printed invalid JSON"))?;
    let kind = report["outcomeDocument"]["outcome"]["kind"]
        .as_str()
        .ok_or_else(|| anyhow!("specimen `{command}`: the report has no outcome kind"))?
        .to_owned();
    Ok(Run {
        id: spec.id,
        label: spec.label,
        command,
        source_html: highlight::numbered_frame(Lang::Fr, &segments, kw, spec.module),
        case_html: highlight::code_frame(Lang::Json, &case_text, kw, Some(case_caption)),
        kind,
        opinion: fidryn_cli::opinion::sentences(&report),
    })
}

/// Blocks of `src` whose first line, trimmed, starts with each prefix in
/// `starts`, in that order. A block runs from that line through the line of
/// the brace that closes it, and comes with the 1-based number of its first
/// line. Braces are counted on lexer tokens, so braces in strings and
/// comments do not count, and a brace group followed directly by `{` (the
/// effect set in `query q() -> T ! {Observe} {`) does not end the block. A
/// prefix with no matching line, or whose block never closes, yields nothing;
/// callers compare the count.
pub fn excerpt(src: &str, starts: &[&str]) -> Vec<(usize, String)> {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut offsets = Vec::with_capacity(lines.len());
    let mut at = 0;
    for line in &lines {
        offsets.push(at);
        at += line.len() + 1;
    }
    let tokens: Vec<_> = lex(src)
        .into_iter()
        .filter(|t| {
            !matches!(
                t.kind,
                TokenKind::Whitespace
                    | TokenKind::Newline
                    | TokenKind::Comment
                    | TokenKind::DocComment
            )
        })
        .collect();
    let line_of = |pos: usize| offsets.partition_point(|&o| o <= pos) - 1;
    let mut out = Vec::new();
    for prefix in starts {
        let Some(first) = lines
            .iter()
            .position(|l| l.trim_start().starts_with(prefix))
        else {
            continue;
        };
        let from = tokens.partition_point(|t| (t.start as usize) < offsets[first]);
        let mut depth = 0usize;
        let mut last = None;
        for (i, token) in tokens.iter().enumerate().skip(from) {
            match token.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace if depth == 0 => break,
                TokenKind::RBrace => {
                    depth -= 1;
                    let reopens = tokens
                        .get(i + 1)
                        .is_some_and(|t| t.kind == TokenKind::LBrace);
                    if depth == 0 && !reopens {
                        last = Some(line_of(token.start as usize));
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(last) = last {
            let text: Vec<&str> = lines[first..=last]
                .iter()
                .map(|l| l.trim_end_matches('\r'))
                .collect();
            out.push((first + 1, text.join("\n")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::workspace_root;

    fn keywords() -> Keywords {
        Keywords::load(&workspace_root()).expect("grammar.ebnf")
    }

    /// Text inside the frame's `<pre><code>`, with highlight spans removed and
    /// entities decoded.
    fn frame_text(frame: &str) -> String {
        let start = frame.find("<code>").expect("frame has <code>") + "<code>".len();
        let end = frame.rfind("</code>").expect("frame has </code>");
        let mut text = String::new();
        let mut rest = &frame[start..end];
        while let Some(open) = rest.find('<') {
            text.push_str(&rest[..open]);
            let close = rest[open..].find('>').expect("tag closes") + open;
            rest = &rest[close + 1..];
        }
        text.push_str(rest);
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&")
    }

    #[test]
    fn excerpt_keeps_the_real_line_numbers_of_the_trust_blocks() {
        let src = fs::read_to_string(workspace_root().join(TRUST)).unwrap();
        let blocks = excerpt(&src, TRUST_EXCERPT);
        assert_eq!(blocks.len(), 2, "{blocks:?}");

        let (first, family) = &blocks[0];
        assert_eq!(*first, 47);
        assert!(
            family.starts_with("    interpretation_family SuccessorEligibility\n"),
            "{family}"
        );
        assert!(
            family.ends_with("          or request Interpret\n    }"),
            "{family}"
        );
        assert_eq!(family.lines().count(), 17, "lines 47 through 63");

        let (first, query) = &blocks[1];
        assert_eq!(*first, 205);
        assert!(query.starts_with("    query acting_trustee()\n"), "{query}");
        assert!(
            query.ends_with("            office TrusteeOf(BRT)\n        }\n    }"),
            "{query}"
        );
        assert_eq!(query.lines().count(), 8, "lines 205 through 212");

        let file: Vec<&str> = src.lines().collect();
        for (first, text) in &blocks {
            for (i, line) in text.lines().enumerate() {
                assert_eq!(line, file[first - 1 + i], "line {}", first + i);
            }
        }
    }

    #[test]
    fn excerpt_counts_braces_on_tokens_not_characters() {
        let src = concat!(
            "module M version \"1\" {\n",
            "    query a() -> Text ! {Observe}\n",
            "    {\n",
            "        // a } in a comment\n",
            "        return \"}\"\n",
            "    }\n",
            "    query b() -> Int { return 1 }\n",
            "}\n",
        );
        let a = "    query a() -> Text ! {Observe}\n    {\n        // a } in a comment\n        return \"}\"\n    }";
        assert_eq!(
            excerpt(src, &["query a", "query b"]),
            vec![
                (2, a.to_owned()),
                (7, "    query b() -> Int { return 1 }".to_owned())
            ]
        );
    }

    #[test]
    fn excerpt_skips_missing_and_unclosed_blocks() {
        let src = "module M version \"1\" {\n    query a() -> Int {\n        return 1\n";
        assert!(excerpt(src, &["query a"]).is_empty(), "unclosed block");
        assert!(excerpt(src, &["query zz"]).is_empty(), "missing prefix");
    }

    #[test]
    fn runs_are_the_four_specimen_runs_with_real_outcomes() {
        let runs = runs(&workspace_root(), &keywords()).expect("specimen runs");
        let got: Vec<(&str, &str, &str, &str)> = runs
            .iter()
            .map(|r| (r.id, r.label, r.kind.as_str(), r.opinion[0].as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (
                    "trust-open",
                    "Trust, open eligibility",
                    "contingent",
                    "acting_trustee depends on SuccessorEligibility."
                ),
                (
                    "trust-court",
                    "Trust, court selects I2",
                    "determinate",
                    "acting_trustee is Bob."
                ),
                ("gate-q", "require-gate, query q", "determinate", "q is 7."),
                (
                    "gate-r",
                    "require-gate, query r",
                    "suspended",
                    "r is suspended."
                ),
            ]
        );
        let open = &runs[0].opinion;
        assert!(
            open.contains(&"Under I1 it is Alice.".to_owned()),
            "{open:?}"
        );
        assert!(open.contains(&"Under I2 it is Bob.".to_owned()), "{open:?}");
        assert_eq!(
            open.last().map(String::as_str),
            Some(
                "Outside scope: tax, creditor_priority, real_property_recording, complete_Massachusetts_trust_law."
            )
        );
        assert_eq!(
            runs[3].opinion.last().map(String::as_str),
            Some("Outside scope: complete_instruments.")
        );
    }

    #[test]
    fn commands_are_the_equivalent_cli_lines() {
        let runs = runs(&workspace_root(), &keywords()).expect("specimen runs");
        let commands: Vec<&str> = runs.iter().map(|r| r.command.as_str()).collect();
        assert_eq!(
            commands,
            vec![
                "fidryn run examples/trust/bryan-revocable-trust.fr --query acting_trustee --case examples/trust/cases/two-certificates-open-eligibility.json --valid-at 2034-03-01T09:00:00Z --known-at 2034-03-01T09:00:00Z",
                "fidryn run examples/trust/bryan-revocable-trust.fr --query acting_trustee --case examples/trust/cases/court-selects-i2.json --valid-at 2034-03-01T09:00:00Z --known-at 2034-03-01T09:00:00Z",
                "fidryn run tests/programs/require-gate.fr --query q --case /tmp/fidryn-empty-case.json --valid-at 2026-09-17T12:00:00Z --known-at 2026-09-17T12:00:00Z",
                "fidryn run tests/programs/require-gate.fr --query r --case /tmp/fidryn-empty-case.json --valid-at 2026-09-17T12:00:00Z --known-at 2026-09-17T12:00:00Z",
            ]
        );
    }

    #[test]
    fn frames_show_the_numbered_source_and_the_evaluated_case() {
        let root = workspace_root();
        let runs = runs(&root, &keywords()).expect("specimen runs");

        let trust = &runs[0].source_html;
        assert!(trust.contains(&format!("<span>{TRUST}</span>")), "{trust}");
        for n in ["47", "63", "205", "212"] {
            assert!(
                trust.contains(&format!("data-n=\"{n}\"")),
                "line {n} missing"
            );
        }
        for n in ["46", "64", "204", "213"] {
            assert!(
                !trust.contains(&format!("data-n=\"{n}\"")),
                "line {n} should be cut"
            );
        }
        assert!(
            trust.contains("class=\"line gap\""),
            "gap between the two blocks"
        );

        let gate = &runs[2].source_html;
        assert!(
            gate.contains("data-n=\"1\"") && gate.contains("data-n=\"11\""),
            "{gate}"
        );
        assert!(!gate.contains("class=\"line gap\""), "{gate}");

        let case_file = fs::read_to_string(
            root.join("examples/trust/cases/two-certificates-open-eligibility.json"),
        )
        .unwrap();
        assert!(
            runs[0].case_html.contains(
                "<span>examples/trust/cases/two-certificates-open-eligibility.json</span>"
            )
        );
        assert_eq!(
            frame_text(&runs[0].case_html),
            case_file.trim_end_matches('\n')
        );
        assert!(runs[2].case_html.contains("<span>empty case record</span>"));
        assert_eq!(
            frame_text(&runs[2].case_html),
            EMPTY_CASE.trim_end_matches('\n')
        );
    }
}
