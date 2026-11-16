//! A keyword in the text of a declaration is read only as a word of its own.
//!
//! The lowering reads the `for` and the `rank` of a nomination, the
//! `artifact` of a source and the definitions of an interpretation family
//! back out of the text of the declaration. A name that holds one of those
//! words, such as a candidate named `Frank`, gave a wrong answer with no
//! diagnostic. Each module here is made in the test.

use fidryn_cli::compile_source;
use fidryn_core::{
    CaseRecord, CoreModule, DiagnosticCode, Instant, Outcome, QueryName, RunContext,
    SourceManifest, Value,
};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use std::collections::BTreeMap;

const AT: &str = "2026-09-17T12:00:00Z";

/// A module with two candidates for one office, both eligible, and the
/// query `acting_trustee`, which asks who holds the office after its
/// occupant: the candidate of the lowest rank.
fn succession(first: &str, second: &str, nominations: &str) -> String {
    format!(
        "module Probe.Succession version \"0.1.0\" {{
    entity {first} : NaturalPerson
    entity {second} : NaturalPerson
    entity Pat : NaturalPerson
    entity BRT : Trust
    proposition Incapacitated(person: NaturalPerson, office: Office)
    proposition Eligible(person: NaturalPerson, office: Office)
    interpretation_family SuccessorEligibility {{
        alternative Both {{
            defines Eligible({first}, TrusteeOf(BRT)) as established
            defines Eligible({second}, TrusteeOf(BRT)) as established
        }}
    }}
{nominations}
    query acting_trustee() -> LegalPerson ! {{Observe, Determine, Interpret}} {{
        goal UniqueOccupant {{ office TrusteeOf(BRT) }}
    }}
}}
"
    )
}

/// The case: `Pat` occupies the office and is incapacitated, each candidate
/// has accepted it, and the alternative `Both` is selected.
fn case(candidates: [&str; 2]) -> CaseRecord {
    let mut facts = serde_json::json!({ "Incapacitated(Pat,TrusteeOf(BRT))": true });
    for candidate in candidates {
        facts[format!("Accepted({candidate},TrusteeOf(BRT))")] = serde_json::json!(true);
    }
    serde_json::from_value(serde_json::json!({
        "schema": "fidryn.case-record/v0.1",
        "facts": facts,
        "evidence": [{
            "schema": "OccupancyRecord",
            "value": { "person": "Pat", "office": "TrusteeOf(BRT)" },
            "observedAt": AT
        }],
        "interpretations": { "SuccessorEligibility": "Both" },
        "admissibleCompletions": {
            "interpretations": { "SuccessorEligibility": ["Both"] },
            "evidence": {},
            "choices": {}
        }
    }))
    .expect("the case record")
}

fn messages(source: &str) -> Vec<(DiagnosticCode, String)> {
    match compile_source(source, &SourceManifest::default()) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message))
            .collect(),
    }
}

/// Who holds the office, by the query `acting_trustee` against the case.
fn acting_trustee(first: &str, second: &str, nominations: &str) -> String {
    let source = succession(first, second, nominations);
    let module: CoreModule = compile_source(&source, &SourceManifest::default())
        .unwrap_or_else(|diagnostics| panic!("{nominations}: {diagnostics:?}"));
    let at = Instant::parse(AT).expect("timestamp");
    let case = case([first, second]);
    let state = case.into_state();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(at),
    };
    let outcome = evaluate(
        &module,
        &QueryName::from("acting_trustee"),
        &BTreeMap::new(),
        &state,
        &RunContext::new(at, at),
        &mut handler,
        &case,
    )
    .expect("the query runs");
    match outcome {
        Outcome::Determinate {
            value: Value::Entity(name),
            ..
        } => name,
        other => panic!("{nominations}: {other:?}"),
    }
}

#[test]
fn a_candidate_whose_name_holds_the_word_rank_is_ranked_as_written() {
    // The rank of `Frank` was read as 0, so `Frank` held the office ahead
    // of `Bob`, whose rank is 1.
    assert_eq!(
        acting_trustee(
            "Frank",
            "Bob",
            "    nomination Frank for TrusteeOf(BRT) rank 2\n    nomination Bob for TrusteeOf(BRT) rank 1"
        ),
        "Bob"
    );
    assert_eq!(
        acting_trustee(
            "Frank",
            "Bob",
            "    nomination Frank for TrusteeOf(BRT) rank 1\n    nomination Bob for TrusteeOf(BRT) rank 2"
        ),
        "Frank"
    );
    // With `Bob` at rank 0 the module was rejected with `E410`, as a tie
    // between two candidates of rank 0.
    let nominations = "    nomination Frank for TrusteeOf(BRT) rank 5\n    nomination Bob for TrusteeOf(BRT) rank 0";
    assert_eq!(messages(&succession("Frank", "Bob", nominations)), []);
    assert_eq!(acting_trustee("Frank", "Bob", nominations), "Bob");
    // Other names that hold the word, and a name that holds `for`.
    for name in ["Franklin", "rank_holder", "ranked", "Platform"] {
        assert_eq!(
            acting_trustee(
                name,
                "Bob",
                &format!(
                    "    nomination {name} for TrusteeOf(BRT) rank 2\n    nomination Bob for TrusteeOf(BRT) rank 1"
                )
            ),
            "Bob",
            "{name}"
        );
    }
}

#[test]
fn a_nomination_written_with_tabs_or_line_ends_is_ranked_as_written() {
    // The office of `Alice` was read as empty, so her nomination was for no
    // office and `Bob` held it, though `Alice` has the lower rank.
    for alice in [
        "    nomination\tAlice\tfor\tTrusteeOf(BRT)\trank\t1",
        "    nomination Alice\nfor TrusteeOf(BRT)\nrank 1",
        "    nomination Alice for TrusteeOf(BRT)\nrank 1",
    ] {
        assert_eq!(
            acting_trustee(
                "Alice",
                "Bob",
                &format!("{alice}\n    nomination Bob for TrusteeOf(BRT) rank 2")
            ),
            "Alice",
            "{alice:?}"
        );
    }
}

#[test]
fn a_source_whose_name_holds_the_word_artifact_needs_its_artifact_field() {
    // The artifact was cut out of the text after the name, so a source with
    // no `artifact` field passed the checker.
    let source = |fields: &str| {
        format!(
            "module Probe.Source version \"0.1.0\" {{\n    source artifact_registry {{\n        kind statute\n        citation \"Stat. 12\"\n{fields}    }}\n    query q() -> Int {{ return 7 }}\n}}\n"
        )
    };
    assert_eq!(
        messages(&source("")),
        [(
            DiagnosticCode::E540,
            "source `artifact_registry` is missing a legal artifact".to_owned()
        )]
    );
    assert_eq!(messages(&source("        artifact \"files/a.txt\"\n")), []);
}

#[test]
fn a_definition_that_is_no_proposition_or_has_no_status_is_rejected() {
    // Both passed the checker, and the alternative defined nothing.
    let family = |definition: &str| {
        format!(
            "module Probe.Family version \"0.1.0\" {{\n    entity Alice : NaturalPerson\n    proposition Eligible(person: NaturalPerson)\n    interpretation_family Reading {{\n        alternative Wide {{\n            {definition}\n        }}\n    }}\n    query q() -> Int {{ return 7 }}\n}}\n"
        )
    };
    assert_eq!(
        messages(&family("defines not Eligible(Alice) as not_established")),
        [(
            DiagnosticCode::E210,
            "`defines not Eligible(Alice) as not_established` in interpretation family `Reading` requires a proposition".to_owned()
        )]
    );
    assert_eq!(
        messages(&family("defines Eligible(Alice)")),
        [(
            DiagnosticCode::E100,
            "`defines Eligible(Alice)` in interpretation family `Reading` has no `as established` or `as not_established` after its expression".to_owned()
        )]
    );
    assert_eq!(
        messages(&family("defines Eligible(Alice) as established")),
        []
    );
}
