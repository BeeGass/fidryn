//! A comment means what the same amount of white space means.
//!
//! The lowering and the checker read parts of a declaration back out of its
//! text. No reader of that text sees a comment, and none finds structure
//! inside a string literal. The first four tests take that through every
//! module that `examples/status.json` records as passing. "Compiles to"
//! means what `fidryn check` does with the text: the parser, the lowering
//! and the checker, down to the intermediate representation.
//!
//! A comment at the end of a line changes nothing. A line that holds only a
//! comment is a blank line, and a blank line is not nothing: it ends a
//! statement where "Where a statement ends" in `docs/language.md` says so.
//! So a module with a comment line put in is compared with the same module
//! with an empty line put in at that place, and not with the module as it
//! is written.
//!
//! The intermediate representation keeps some text as it is written, for
//! display: the text of a `clause`, the formula of a `verify` block, a
//! clause of a `judgment` or an `observation` that is kept as text, and a
//! line of a `verify` block that is kept as an assertion that cannot be
//! decided. That text holds the comments inside it. It is compared with the
//! comment and all white space taken out. Everything else is compared as it
//! is, and so is the answer of `fidryn verify` for every property.

use fidryn_cli::{compile_source, load_manifest};
use fidryn_core::{
    CaseRecord, CoreDecl, CoreModule, DiagnosticCode, Instant, Outcome, QueryName, RunContext,
    SourceManifest, Term, Value,
};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use fidryn_verify::verify_property;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A comment that holds what a reader of raw text trips over: brackets that
/// close and open, a quote, an arrow, and the words the readers search for.
const COMMENT: &str = "// } ] ) { [ ( \" when true then derive Wrong() assert false bounds { persons: 9 } returns 9 defeat Wrong -> Wrong for_all x in People: true";

/// Above every how-manieth line the fourth test puts a line in. One is
/// every line.
const STRIDE: usize = 1;

type Compiled = Result<CoreModule, Vec<(DiagnosticCode, String)>>;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn messages(diagnostics: Vec<fidryn_core::Diagnostic>) -> Vec<(DiagnosticCode, String)> {
    diagnostics
        .into_iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.message))
        .collect()
}

/// The modules `examples/status.json` records as passing: path, text and
/// source manifest.
fn passing_modules() -> Vec<(String, String, SourceManifest)> {
    let status = std::fs::read_to_string(root().join("examples/status.json")).expect("status");
    let status: serde_json::Value = serde_json::from_str(&status).expect("status is JSON");
    let modules: Vec<_> = status["files"]
        .as_array()
        .expect("files")
        .iter()
        .filter(|file| file["check"] == "ok")
        .map(|file| {
            let path = file["path"].as_str().expect("path").to_owned();
            let full = root().join(&path);
            let source = std::fs::read_to_string(&full).expect("module text");
            let manifest = load_manifest(&full, &source).expect("source manifest");
            (path, source, manifest)
        })
        .collect();
    assert!(modules.len() > 40, "{} passing modules", modules.len());
    modules
}

fn compiled(source: &str, manifest: &SourceManifest) -> Compiled {
    compile_source(source, manifest).map_err(messages)
}

/// Text that is kept for display, without the comment and without white
/// space.
fn bare(text: &str) -> String {
    text.replace(COMMENT, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The compiled module with each text it keeps for display made bare.
fn display_text_made_bare(mut module: CoreModule) -> CoreModule {
    for declaration in &mut module.declarations {
        match declaration {
            CoreDecl::Clause(clause) => clause.source = bare(&clause.source),
            CoreDecl::Judgment(judgment) => {
                for clause in &mut judgment.unsupported_clauses {
                    *clause = bare(clause);
                }
            }
            CoreDecl::Observation(observation) => {
                for clause in &mut observation.unsupported_clauses {
                    *clause = bare(clause);
                }
            }
            _ => {}
        }
    }
    for verification in &mut module.verifications {
        verification.formula = bare(&verification.formula);
        for assertion in &mut verification.assertions {
            if let Term::Apply { ctor, args } = assertion
                && ctor == "$unsupported_assertion"
                && let [Term::String(text)] = args.as_mut_slice()
            {
                *text = bare(text);
            }
        }
    }
    module
}

/// The names of the properties of the module, in its order.
fn property_names(module: &CoreModule) -> Vec<&str> {
    module
        .verifications
        .iter()
        .map(|property| property.name.as_str())
        .collect()
}

/// The answer of `fidryn verify` for every property of the module.
fn verdicts(module: &CoreModule) -> Vec<String> {
    module
        .verifications
        .iter()
        .map(|property| verify_property(module, &property.name).to_string())
        .collect()
}

/// The Debug text of two values around the first place where it differs.
fn where_they_differ<T: std::fmt::Debug>(left: &T, right: &T) -> String {
    let (left, right) = (format!("{left:?}"), format!("{right:?}"));
    let at = left
        .bytes()
        .zip(right.bytes())
        .position(|(l, r)| l != r)
        .unwrap_or(left.len().min(right.len()));
    let around = |text: &str| -> String {
        text.char_indices()
            .filter(|(index, _)| (at.saturating_sub(60)..at + 100).contains(index))
            .map(|(_, c)| c)
            .collect()
    };
    format!("...{}... | ...{}...", around(&left), around(&right))
}

/// What differs between two compiled modules, part by part.
fn module_difference(left: &CoreModule, right: &CoreModule) -> Option<String> {
    let (left_verdicts, right_verdicts) = (verdicts(left), verdicts(right));
    let (left, right) = (
        display_text_made_bare(left.clone()),
        display_text_made_bare(right.clone()),
    );
    if left == right && left_verdicts == right_verdicts {
        return None;
    }
    let mut found = Vec::new();
    let (left_names, right_names) = (property_names(&left), property_names(&right));
    if left_names != right_names {
        found.push(format!("properties {left_names:?} | {right_names:?}"));
    }
    if left.declarations.len() != right.declarations.len() {
        found.push(format!(
            "{} declarations | {}",
            left.declarations.len(),
            right.declarations.len()
        ));
    }
    for (a, b) in left.declarations.iter().zip(&right.declarations) {
        if a != b {
            found.push(format!("declaration {}", where_they_differ(a, b)));
        }
    }
    for (a, b) in left.queries.iter().zip(&right.queries) {
        if a != b {
            found.push(format!("query {}", where_they_differ(a, b)));
        }
    }
    for (a, b) in left.verifications.iter().zip(&right.verifications) {
        if a != b {
            found.push(format!("property {}", where_they_differ(a, b)));
        }
    }
    if left_verdicts != right_verdicts {
        found.push(format!(
            "verdicts {}",
            where_they_differ(&left_verdicts, &right_verdicts)
        ));
    }
    if found.is_empty() {
        found.push(format!("module {}", where_they_differ(&left, &right)));
    }
    let more = found.len().saturating_sub(3);
    found.truncate(3);
    if more > 0 {
        found.push(format!("and {more} more"));
    }
    Some(format!("the modules differ: {}", found.join("; ")))
}

/// What differs between what two texts compile to, when anything does.
fn difference(left: &Compiled, right: &Compiled) -> Option<String> {
    match (left, right) {
        (Ok(left), Ok(right)) => module_difference(left, right),
        (Err(left), Err(right)) if left == right => None,
        (Err(left), Err(right)) => {
            let only_left: Vec<_> = left.iter().filter(|d| !right.contains(d)).take(2).collect();
            let only_right: Vec<_> = right.iter().filter(|d| !left.contains(d)).take(2).collect();
            Some(format!(
                "the diagnostics differ: {only_left:?} | {only_right:?}"
            ))
        }
        (Ok(_), Err(diagnostics)) => Some(format!(
            "the first compiles and the second is rejected: {:?}",
            &diagnostics[..diagnostics.len().min(2)]
        )),
        (Err(diagnostics), Ok(_)) => Some(format!(
            "the first is rejected and the second compiles: {:?}",
            &diagnostics[..diagnostics.len().min(2)]
        )),
    }
}

fn joined(lines: impl Iterator<Item = String>) -> String {
    let mut text = lines.collect::<Vec<_>>().join("\n");
    text.push('\n');
    text
}

/// The text with `line` put in above each of its lines whose index
/// `above` accepts.
fn with_lines_put_in(source: &str, line: &str, above: impl Fn(usize) -> bool) -> String {
    joined(source.lines().enumerate().flat_map(|(index, text)| {
        let put_in = above(index).then(|| line.to_owned());
        put_in.into_iter().chain([text.to_owned()])
    }))
}

fn assert_none(differences: &[String], what: &str) {
    assert!(
        differences.is_empty(),
        "{} of the passing modules differ ({what}):\n{}",
        differences.len(),
        differences.join("\n")
    );
}

#[test]
fn a_comment_at_the_end_of_every_line_changes_nothing() {
    let mut differences = Vec::new();
    for (path, source, manifest) in passing_modules() {
        let original = compiled(&source, &manifest);
        assert!(original.is_ok(), "{path}: {original:?}");
        // A string cannot span lines, so every line can take a comment.
        let commented = joined(source.lines().map(|line| format!("{line} {COMMENT}")));
        if let Some(found) = difference(&original, &compiled(&commented, &manifest)) {
            differences.push(format!("{path}: {found}"));
        }
    }
    assert_none(&differences, "a comment at the end of every line");
}

#[test]
fn a_comment_line_in_place_of_every_blank_line_changes_nothing() {
    let mut differences = Vec::new();
    for (path, source, manifest) in passing_modules() {
        let original = compiled(&source, &manifest);
        let commented = joined(source.lines().map(|line| {
            if line.trim().is_empty() {
                COMMENT.to_owned()
            } else {
                line.to_owned()
            }
        }));
        if let Some(found) = difference(&original, &compiled(&commented, &manifest)) {
            differences.push(format!("{path}: {found}"));
        }
    }
    assert_none(&differences, "a comment line in place of every blank line");
}

#[test]
fn a_comment_line_above_every_line_is_an_empty_line_above_every_line() {
    let mut differences = Vec::new();
    let mut twins_that_compile = 0;
    let modules = passing_modules();
    for (path, source, manifest) in &modules {
        let twin = compiled(&with_lines_put_in(source, "", |_| true), manifest);
        twins_that_compile += usize::from(twin.is_ok());
        let commented = compiled(&with_lines_put_in(source, COMMENT, |_| true), manifest);
        if let Some(found) = difference(&twin, &commented) {
            differences.push(format!("{path}: {found}"));
        }
    }
    println!(
        "{twins_that_compile} of {} twins with an empty line above every line compile",
        modules.len()
    );
    assert_none(&differences, "a comment line above every line");
}

#[test]
fn a_comment_line_above_one_line_is_an_empty_line_above_that_line() {
    // With an empty line above every line most modules are rejected, and
    // the test above then compares two lists of diagnostics. So for those
    // modules a line is put in above one line at a time.
    let mut differences = Vec::new();
    let mut places = 0;
    for (path, source, manifest) in passing_modules() {
        if compiled(&with_lines_put_in(&source, "", |_| true), &manifest).is_ok() {
            continue;
        }
        for line in (0..source.lines().count()).step_by(STRIDE) {
            places += 1;
            let twin = compiled(&with_lines_put_in(&source, "", |at| at == line), &manifest);
            let commented = compiled(
                &with_lines_put_in(&source, COMMENT, |at| at == line),
                &manifest,
            );
            if let Some(found) = difference(&twin, &commented) {
                differences.push(format!("{path}, above line {}: {found}", line + 1));
            }
        }
    }
    println!("a line was put in at {places} places");
    assert_none(&differences, "a comment line above one line");
}

// What the tests below compile, and how they run a query.

fn module(name: &str, declarations: &str) -> String {
    format!(
        "module {name} version \"0.1.0\" {{\n    jurisdiction Test\n    source_snapshot \"comments\"\n    effective_at 2026-01-01\n    recorded_at 2026-01-01T00:00:00Z\n    outside_scope {{ everything_else }}\n{declarations}\n}}\n"
    )
}

fn compile(declarations: &str) -> Compiled {
    compiled(
        &module("Comments.Text", declarations),
        &SourceManifest::default(),
    )
}

/// The outcome of a query against a case record that holds nothing.
fn run(module: &CoreModule, query: &str) -> Outcome<Value> {
    let at = Instant::parse("2026-09-17T12:00:00Z").expect("timestamp");
    let case = CaseRecord::default();
    let state = case.into_state();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(at),
    };
    evaluate(
        module,
        &QueryName::from(query),
        &BTreeMap::new(),
        &state,
        &RunContext::new(at, at),
        &mut handler,
        &case,
    )
    .expect("the query runs")
}

fn effect_operation(module: &CoreModule) -> (String, usize, String) {
    module
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            CoreDecl::EffectDecl(effect) => {
                let operation = &effect.operations[0];
                Some((
                    operation.name.clone(),
                    operation.params.len(),
                    operation.result.to_string(),
                ))
            }
            _ => None,
        })
        .expect("an effect")
}

/// A module with an effect whose block holds these lines, and a query that
/// requires the effect.
fn reviewed(header: &str, lines: &str) -> String {
    format!(
        "    effect Review{header} {{\n{lines}\n    }}\n    query q() -> Bool ! {{Review}} {{\n        require Review(\"x\")\n        return true\n    }}"
    )
}

#[test]
fn a_comment_in_an_effect_is_not_read_into_its_operation() {
    let plain = compile(&reviewed("", "        request(name: String) -> Bool"))
        .expect("the effect compiles");
    assert_eq!(
        effect_operation(&plain),
        ("request".to_owned(), 1, "Bool".to_owned())
    );
    // The query asks the case for the review.
    let asked = run(&plain, "q");
    assert!(
        matches!(&asked, Outcome::Suspended { requests, .. } if requests.len() == 1),
        "{asked:?}"
    );
    for lines in [
        // At the end of the line of the operation: the comment was read
        // into the result type, and the query was rejected with `E210`.
        "        request(name: String) -> Bool // asks a clerk",
        // On the first line of the block: the operation lost its name and
        // its parameter.
        "        // asks a clerk (see the rules) -> Clerk\n        request(name: String) -> Bool",
        "        request(name: String) // (the name: Wrong)\n            -> Bool // -> Wrong",
    ] {
        let commented = compile(&reviewed("", lines)).unwrap_or_else(|diagnostics| {
            panic!("{lines}: {diagnostics:?}");
        });
        assert_eq!(commented, plain, "{lines}");
        assert_eq!(
            format!("{:?}", run(&commented, "q")),
            format!("{asked:?}"),
            "{lines}"
        );
    }
    // The first word of the block names the operation, whatever the
    // comment above it says.
    let named = compile(&reviewed(
        "",
        "        // request(nothing: Wrong) -> Wrong\n        lookup(name: String) -> Bool",
    ))
    .expect("the effect compiles");
    assert_eq!(
        effect_operation(&named),
        ("lookup".to_owned(), 1, "Bool".to_owned())
    );
}

#[test]
fn a_bracket_or_an_arrow_inside_a_string_does_not_move_a_block_or_a_clause() {
    // A citation with a parenthesis that nothing closes, and an arrow in a
    // string below the operation.
    let cited = compile(&reviewed(
        " from \"Stat. § 12(a\"",
        "        request(name: String) -> Bool\n        note \"a -> Wrong ) }\"",
    ))
    .expect("the effect compiles");
    assert_eq!(
        effect_operation(&cited),
        ("request".to_owned(), 1, "Bool".to_owned())
    );
    // An artifact whose name holds a brace, below a citation that holds
    // the word `artifact`.
    compile(
        "    source Statute {\n        kind statute\n        citation \"the artifact \\\"wrong\\\" of 12(a\"\n        artifact \"files/a}1.txt\"\n        digest fixture\n    }\n    query q() -> Int { return 7 }",
    )
    .expect("the source has its artifact");
    // The citation alone is no artifact.
    assert_eq!(
        compile(
            "    source Statute {\n        kind statute\n        citation \"the artifact \\\"wrong\\\" of 12(a\"\n    }\n    query q() -> Int { return 7 }",
        ),
        Err(vec![(
            DiagnosticCode::E540,
            "source `Statute` is missing a legal artifact".to_owned()
        )])
    );
}

#[test]
fn a_quantifier_in_a_comment_or_a_string_makes_no_property() {
    let commented = compile(
        "    query q() -> Bool {\n        // for_all x in People: true\n        return true // exists y in People: true\n    }\n    verify Stated {\n        // exists z in People { true }\n        assert true\n    }\n    query r() -> String {\n        return \"for_all w in People: true\"\n    }",
    )
    .expect("the module compiles");
    assert_eq!(property_names(&commented), ["Stated"]);
    assert_eq!(
        verify_property(&commented, "for_all:x:0").to_string(),
        "unknown property for_all:x:0"
    );
}

#[test]
fn a_quantifier_written_in_a_query_or_a_verify_block_makes_a_property_named_after_it() {
    let module = compile(
        "    entity Pat : NaturalPerson\n    query q() -> Bool {\n        return for_all x in {Pat}: true // every one\n    }\n    verify Stated {\n        exists y in {1, 2}: y == 2\n    }\n    for_all z in {1, 2}: z > 0",
    )
    .expect("the module compiles");
    // The block, then one property for each quantifier written in a query,
    // in a verify block or as a declaration of its own: its kind, its
    // variable and its number among them, in the order of the text.
    assert_eq!(
        property_names(&module),
        ["Stated", "for_all:x:0", "exists:y:1", "for_all:z:2"]
    );
    // `fidryn verify` proves the block and decides none of the other
    // three.
    assert_eq!(
        verify_property(&module, "Stated").to_string(),
        "proved Stated (persons=0, events=0, time_points=0)"
    );
    for name in ["for_all:x:0", "exists:y:1", "for_all:z:2"] {
        assert_eq!(
            verify_property(&module, name).to_string(),
            format!(
                "unknown {name}: no supported structured verification assertion (persons=0, events=0, time_points=0)"
            )
        );
    }
    // Over a sort the property is named in the same way, and it is not
    // decided either: such a property has no bounds, whatever the block
    // around it declares. A quantifier in a `calc` is no property and is
    // not counted.
    let module = compile(
        "    calc every() -> Bool {\n        return for_all w in {1, 2}: w > 0\n    }\n    verify Bounded {\n        for_all x in People: true\n        bounds { persons: 2, events: 0, time_points: 0 }\n    }",
    )
    .expect("the module compiles");
    assert_eq!(property_names(&module), ["Bounded", "for_all:x:0"]);
    assert_eq!(
        verify_property(&module, "Bounded").to_string(),
        "proved Bounded (persons=2, events=0, time_points=0)"
    );
    assert_eq!(
        verify_property(&module, "for_all:x:0").to_string(),
        "unknown for_all:x:0: W620 BoundedVerification: quantifier has empty bounds (persons=0, events=0, time_points=0)"
    );
}
