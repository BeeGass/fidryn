//! A rule keeps its own guard and consequences whatever its `from` basis
//! is, and the declarations after it are kept.
//!
//! The basis here is a bare name, as in `from Basis`: a name directly
//! before the `{` that opens the rule's body.

use fidryn_cli::compile_source;
use fidryn_core::ir::{Consequence, Guard};
use fidryn_core::{
    CaseRecord, CoreDecl, CoreModule, CoreRule, DiagnosticCode, Instant, Outcome, QueryName,
    RunContext, SourceManifest, Value,
};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use std::collections::BTreeMap;

fn source(body: &str) -> String {
    format!(
        "module Rules.Basis version \"0.1.0\" {{\n    jurisdiction Test\n    source_snapshot \"rules\"\n    effective_at 2026-01-01\n    recorded_at 2026-01-01T00:00:00Z\n    outside_scope {{ everything_else }}\n{body}\n}}\n"
    )
}

/// A rule with the basis `from Basis` whose guard tests `P` of
/// `guard_argument`, followed by the queries `a` and `b`.
fn rule_then_two_queries(guard_argument: &str) -> String {
    source(&format!(
        "    proposition P(name: String)\n    proposition Q(name: String)\n    rule R : constitutive from Basis {{\n        when operative P({guard_argument})\n        then derive Q(\"x\")\n    }}\n    query a() -> Int {{ return 1 }}\n    query b() -> Int {{ return 2 }}"
    ))
}

fn compile(source: &str) -> CoreModule {
    compile_source(source, &SourceManifest::default()).expect("the module compiles")
}

fn rule<'a>(module: &'a CoreModule, name: &str) -> &'a CoreRule {
    module
        .declarations
        .iter()
        .find_map(|decl| match decl {
            CoreDecl::Rule(rule) if rule.name == name => Some(rule),
            _ => None,
        })
        .unwrap_or_else(|| panic!("rule {name}"))
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

#[test]
fn the_guard_of_a_rule_with_a_bare_name_basis_is_checked() {
    let diagnostics = compile_source(
        &rule_then_two_queries("undeclared"),
        &SourceManifest::default(),
    )
    .expect_err("an undeclared name in the guard is rejected");
    let read: Vec<(DiagnosticCode, &str)> = diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
        .collect();
    assert_eq!(
        read,
        [(
            DiagnosticCode::E210,
            "argument `name` of `P` could not be typed as String"
        )]
    );
}

#[test]
fn the_declarations_after_a_rule_with_a_bare_name_basis_are_kept() {
    let module = compile(&rule_then_two_queries("\"x\""));
    let names: Vec<&str> = module
        .queries
        .iter()
        .map(|query| query.name.as_str())
        .collect();
    assert_eq!(names, ["a", "b"]);
    // The guard and the consequence are the rule's own.
    let rule = rule(&module, "R");
    assert!(
        matches!(&rule.guard, Guard::Operative(prop, _) if prop.predicate == "P"),
        "{:?}",
        rule.guard
    );
    assert!(
        matches!(
            &rule.consequences[0].consequence,
            Consequence::Derive(prop) if prop.predicate == "Q"
        ),
        "{:?}",
        rule.consequences
    );
}

#[test]
fn a_rule_with_a_bare_name_basis_fires_with_its_own_consequence() {
    let module = compile(&source(
        "    proposition Own()\n    proposition Other()\n    rule First : derive from Basis {\n        when true\n        then derive Own()\n    }\n    rule Second : derive {\n        when false\n        then derive Other()\n    }\n    query own() -> Bool { goal Evaluate { operative Own() } }\n    query other() -> Bool { goal Evaluate { operative Other() } }",
    ));
    // `First` derives `Own`, which is its own consequence.
    let own = run(&module, "own");
    assert!(
        matches!(
            own,
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            }
        ),
        "{own:?}"
    );
    // `Second` keeps its guard, which is false, so nothing derives `Other`.
    let other = run(&module, "other");
    assert!(matches!(other, Outcome::Suspended { .. }), "{other:?}");
}
