//! Plain sentences for an evaluation report, shared by the mill and the site.
//!
//! Every sentence is a fixed template filled from report fields. Nothing is
//! inferred beyond the data: a field the templates do not name is left out,
//! and an outcome kind without a template gets one sentence naming the kind.

use serde_json::{Map, Value};

/// Sentences describing `report`, a `fidryn.evaluation-report/v0.1` document.
///
/// The outcome's sentences come first; `Outside scope: …` is always last
/// when the model boundary names anything.
pub fn sentences(report: &Value) -> Vec<String> {
    let doc = &report["outcomeDocument"];
    let query = doc["query"].as_str().unwrap_or("The query");
    let mut out = if doc["outcome"].is_object() {
        outcome_sentences(query, &doc["outcome"])
    } else {
        vec!["The report has no outcome.".to_owned()]
    };
    let outside = texts(&doc["modelBoundary"]["outsideScope"]);
    if !outside.is_empty() {
        out.push(format!("Outside scope: {}.", outside.join(", ")));
    }
    out
}

/// A runtime value (`{"kind", "data"}`) as short plain text. Shapes this
/// function does not know are shown as compact JSON.
pub fn value_text(value: &Value) -> String {
    tagged_value_text(value).unwrap_or_else(|| value.to_string())
}

/// An open request (`{"kind", …}`) as a noun phrase, for example
/// `evidence matching PaymentRecord`. A request without a kind is shown as
/// compact JSON.
pub fn request_text(request: &Value) -> String {
    let Some(kind) = request["kind"].as_str() else {
        return request.to_string();
    };
    match kind {
        "needCustom" => {
            let effect = text(&request["effect"]);
            match request["payload"].as_str() {
                Some(payload) => format!("{payload} ({effect})"),
                None => effect,
            }
        }
        "needEvidence" => format!("evidence matching {}", text(&request["schema"])),
        "needInterpretation" => {
            let family = text(&request["family"]);
            match request["source"].as_str() {
                Some(source) if !source.is_empty() => {
                    format!("an interpretation of {family} under {source}")
                }
                _ => format!("an interpretation of {family}"),
            }
        }
        "needJudgment" => format!("a determination under {}", text(&request["protocol"])),
        "needChoice" => format!(
            "a decision under {}{}",
            text(&request["protocol"]),
            among(&request["options"])
        ),
        "needApplicableLaw" => format!("applicable law{}", among(&request["candidates"])),
        "needConflict" => {
            let names = names(&request["doctrines"]);
            if names.is_empty() {
                "one applicable conflict doctrine".to_owned()
            } else {
                format!(
                    "one applicable conflict doctrine among {}",
                    names.join(", ")
                )
            }
        }
        other => other.to_owned(),
    }
}

fn outcome_sentences(query: &str, outcome: &Value) -> Vec<String> {
    match outcome["kind"].as_str() {
        Some("determinate") => determinate(query, outcome),
        Some("contingent") => contingent(query, outcome),
        Some("suspended") => suspended(query, outcome),
        Some("normConflict") => norm_conflict(query, outcome),
        Some("outsideCompetence") => outside_competence(query, outcome),
        Some("inconsistent") => inconsistent(query, outcome),
        _ => vec![format!(
            "{query} returned an outcome of kind {}.",
            text(&outcome["kind"])
        )],
    }
}

fn determinate(query: &str, outcome: &Value) -> Vec<String> {
    let mut out = vec![format!("{query} is {}.", value_text(&outcome["value"]))];
    if let Some(id) = outcome["convergenceCertificate"].as_str() {
        out.push(format!("Covered by convergence certificate {id}."));
    }
    match outcome["ignoredOpenIssues"].as_array().map_or(0, Vec::len) {
        0 => {}
        1 => out.push("1 open issue was set aside under that certificate.".to_owned()),
        n => out.push(format!(
            "{n} open issues were set aside under that certificate."
        )),
    }
    out
}

fn contingent(query: &str, outcome: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut families: Vec<&str> = Vec::new();
    for pivot in outcome["pivots"].as_array().into_iter().flatten() {
        let name = ["family", "protocol", "kind"]
            .into_iter()
            .find_map(|key| pivot[key].as_str());
        if let Some(name) = name
            && !families.contains(&name)
        {
            families.push(name);
        }
    }
    if !families.is_empty() {
        out.push(format!("{query} depends on {}.", families.join(" and ")));
    }
    if let Some(alternatives) = outcome["alternatives"].as_object() {
        for (key, value) in sorted_entries(alternatives) {
            out.push(format!(
                "Under {} it is {}.",
                completion_text(key),
                value_text(value)
            ));
        }
    }
    out.push("No single answer is determinate across the admissible completions.".to_owned());
    out
}

fn suspended(query: &str, outcome: &Value) -> Vec<String> {
    let requests: Vec<String> = outcome["requests"]
        .as_array()
        .into_iter()
        .flatten()
        .map(request_text)
        .collect();
    let mut out = vec![format!("{query} is suspended.")];
    match requests.as_slice() {
        [] => {}
        [one] => out.push(format!("Outstanding request: {one}.")),
        many => out.push(format!("Outstanding requests: {}.", many.join("; "))),
    }
    out
}

fn norm_conflict(query: &str, outcome: &Value) -> Vec<String> {
    let names = names(&outcome["doctrines"]);
    if names.is_empty() {
        vec![format!("{query} ended in a norm conflict.")]
    } else {
        vec![format!(
            "{query} ended in a norm conflict: no single applicable doctrine among {}.",
            names.join(", ")
        )]
    }
}

fn outside_competence(query: &str, outcome: &Value) -> Vec<String> {
    let mut out = vec![match outcome["reason"].as_str() {
        Some(reason) if !reason.is_empty() => {
            format!("{query} is outside the module's competence: {reason}.")
        }
        _ => format!("{query} is outside the module's competence."),
    }];
    if !outcome["request"].is_null() {
        out.push(format!("Request: {}.", request_text(&outcome["request"])));
    }
    out
}

fn inconsistent(query: &str, outcome: &Value) -> Vec<String> {
    let mut out = vec![format!(
        "{query} has no consistent answer: the admitted model cannot be satisfied."
    )];
    let core = texts(&outcome["core"]);
    if !core.is_empty() {
        out.push(format!("Unsatisfiable core: {}.", core.join(", ")));
    }
    out
}

/// `I1` for a run key; `SuccessorEligibility = I1` for the explore key
/// `i:SuccessorEligibility=I1`. Each binding of a multi-binding key loses
/// its `x:` prefix, and the bindings are joined with `, `.
fn completion_text(key: &str) -> String {
    key.split(',')
        .map(|binding| {
            let bytes = binding.as_bytes();
            let rest = if bytes.len() > 1 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                &binding[2..]
            } else {
                binding
            };
            rest.replace('=', " = ")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn tagged_value_text(value: &Value) -> Option<String> {
    let data = &value["data"];
    let shown = match value["kind"].as_str()? {
        "int" | "decimal" | "bool" | "instant" | "duration" | "entity" => text(data),
        "string" => format!("\"{}\"", text(data)),
        "unit" => "unit".to_owned(),
        "ctor" => ctor_text(data)?,
        "set" => {
            let items: Vec<String> = data.as_array()?.iter().map(value_text).collect();
            format!("{{{}}}", items.join(", "))
        }
        "map" => {
            let entries: Vec<String> = sorted_entries(data.as_object()?)
                .into_iter()
                .map(|(key, item)| format!("{key}: {}", value_text(item)))
                .collect();
            format!("{{{}}}", entries.join(", "))
        }
        "option" if data.is_null() => "none".to_owned(),
        "option" => value_text(data),
        _ => return None,
    };
    Some(shown)
}

/// `Name`, `Name(v0, v1)` for positional fields (`_0`, `_1`, …), or
/// `Name(k: v, …)` for named fields.
fn ctor_text(data: &Value) -> Option<String> {
    let name = data["name"].as_str()?;
    let fields = match data["fields"].as_object() {
        Some(fields) if !fields.is_empty() => fields,
        _ => return Some(name.to_owned()),
    };
    let args: Vec<String> = if fields.keys().all(|key| key.starts_with('_')) {
        let mut entries: Vec<(&String, &Value)> = fields.iter().collect();
        entries.sort_by_key(|(key, _)| (key[1..].parse::<u64>().ok(), key.as_str()));
        entries
            .into_iter()
            .map(|(_, item)| value_text(item))
            .collect()
    } else {
        sorted_entries(fields)
            .into_iter()
            .map(|(key, item)| format!("{key}: {}", value_text(item)))
            .collect()
    };
    Some(format!("{name}({})", args.join(", ")))
}

/// ` among a, b` when `list` is a nonempty array of strings, else nothing.
fn among(list: &Value) -> String {
    match list.as_array() {
        Some(items) if !items.is_empty() && items.iter().all(Value::is_string) => {
            format!(" among {}", texts(list).join(", "))
        }
        _ => String::new(),
    }
}

/// Doctrine names: string items, or the `name` of object items.
fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().or_else(|| item["name"].as_str()))
        .map(str::to_owned)
        .collect()
}

/// Every item of an array as text; empty for anything else.
fn texts(list: &Value) -> Vec<String> {
    list.as_array().into_iter().flatten().map(text).collect()
}

/// A string as itself; any other JSON as compact JSON.
fn text(value: &Value) -> String {
    match value.as_str() {
        Some(string) => string.to_owned(),
        None => value.to_string(),
    }
}

fn sorted_entries(map: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut entries: Vec<(&String, &Value)> = map.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TRUST_SCOPE: [&str; 4] = [
        "tax",
        "creditor_priority",
        "real_property_recording",
        "complete_Massachusetts_trust_law",
    ];

    /// An evaluation report for `acting_trustee` with `outcome` and the
    /// given `outsideScope`.
    fn report_with(outcome: Value, outside_scope: &[&str]) -> Value {
        json!({
            "schema": "fidryn.evaluation-report/v0.1",
            "executionMode": "operative",
            "sourceTrust": "unauthenticated",
            "verificationMethod": "none",
            "assumptions": [],
            "coverage": null,
            "outcomeDocument": {
                "schema": "fidryn.outcome/v0.1",
                "module": "Examples.BryanRevocableTrust@0.1.0",
                "sourceSnapshot": "2026-08-23-ma-trust-fixture",
                "query": "acting_trustee",
                "asOf": {
                    "validTime": "2034-03-01T09:00:00Z",
                    "recordTime": "2034-03-01T09:00:00Z"
                },
                "modelBoundary": {
                    "outsideScope": outside_scope,
                    "admissibleCompletions": {}
                },
                "outcome": outcome
            }
        })
    }

    fn report(outcome: Value) -> Value {
        report_with(outcome, &[])
    }

    fn entity(name: &str) -> Value {
        json!({"kind": "entity", "data": name})
    }

    #[test]
    fn determinate_names_the_value() {
        let outcome = json!({
            "kind": "determinate",
            "value": entity("Bob"),
            "trace": "0".repeat(32),
            "ignoredOpenIssues": []
        });
        assert_eq!(sentences(&report(outcome)), ["acting_trustee is Bob."]);
    }

    #[test]
    fn determinate_with_a_null_certificate_adds_nothing() {
        let outcome = json!({
            "kind": "determinate",
            "value": {"kind": "int", "data": 7},
            "trace": "0".repeat(32),
            "convergenceCertificate": null,
            "ignoredOpenIssues": []
        });
        assert_eq!(sentences(&report(outcome)), ["acting_trustee is 7."]);
    }

    #[test]
    fn determinate_certificate_and_one_ignored_issue() {
        let outcome = json!({
            "kind": "determinate",
            "value": entity("Bob"),
            "trace": "0".repeat(32),
            "convergenceCertificate": "ab".repeat(16),
            "ignoredOpenIssues": [
                {"kind": "needInterpretation", "source": "Instrument", "family": "SuccessorEligibility"}
            ]
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee is Bob.".to_owned(),
                format!("Covered by convergence certificate {}.", "ab".repeat(16)),
                "1 open issue was set aside under that certificate.".to_owned(),
            ]
        );
    }

    #[test]
    fn determinate_certificate_and_several_ignored_issues() {
        let outcome = json!({
            "kind": "determinate",
            "value": entity("Bob"),
            "trace": "0".repeat(32),
            "convergenceCertificate": "cd".repeat(16),
            "ignoredOpenIssues": [
                {"kind": "needInterpretation", "source": "Instrument", "family": "SuccessorEligibility"},
                {"kind": "needEvidence", "issue": {}, "schema": "SecondConcurringCertificate"}
            ]
        });
        assert_eq!(
            sentences(&report(outcome))[2],
            "2 open issues were set aside under that certificate."
        );
    }

    #[test]
    fn contingent_with_run_keys() {
        let outcome = json!({
            "kind": "contingent",
            "alternatives": {"I2": entity("Bob"), "I1": entity("Alice")},
            "pivots": [
                {"kind": "needInterpretation", "source": "Instrument.clause(\"4.4\")", "family": "SuccessorEligibility"}
            ],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report_with(outcome, &TRUST_SCOPE)),
            [
                "acting_trustee depends on SuccessorEligibility.",
                "Under I1 it is Alice.",
                "Under I2 it is Bob.",
                "No single answer is determinate across the admissible completions.",
                "Outside scope: tax, creditor_priority, real_property_recording, complete_Massachusetts_trust_law.",
            ]
        );
    }

    #[test]
    fn contingent_with_explore_keys() {
        let outcome = json!({
            "kind": "contingent",
            "alternatives": {
                "i:SuccessorEligibility=I1": entity("Alice"),
                "i:SuccessorEligibility=I2": entity("Bob")
            },
            "pivots": [{"kind": "needInterpretation", "source": "Instrument", "family": "SuccessorEligibility"}],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee depends on SuccessorEligibility.",
                "Under SuccessorEligibility = I1 it is Alice.",
                "Under SuccessorEligibility = I2 it is Bob.",
                "No single answer is determinate across the admissible completions.",
            ]
        );
    }

    #[test]
    fn contingent_with_multi_binding_explore_keys() {
        let outcome = json!({
            "kind": "contingent",
            "alternatives": {
                "i:SuccessorEligibility=I1,e:SecondConcurringCertificate=absent": entity("Alice")
            },
            "pivots": [],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "Under SuccessorEligibility = I1, SecondConcurringCertificate = absent it is Alice.",
                "No single answer is determinate across the admissible completions.",
            ]
        );
    }

    #[test]
    fn contingent_pivot_names_fall_back_and_drop_duplicates() {
        let outcome = json!({
            "kind": "contingent",
            "alternatives": {},
            "pivots": [
                {"kind": "needInterpretation", "source": "A", "family": "SuccessorEligibility"},
                {"kind": "needJudgment", "issue": {}, "protocol": "CourtCapacityDetermination"},
                {"kind": "needInterpretation", "source": "B", "family": "SuccessorEligibility"},
                {"kind": "needEvidence", "issue": {}}
            ],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome))[0],
            "acting_trustee depends on SuccessorEligibility and CourtCapacityDetermination and needEvidence."
        );
    }

    #[test]
    fn contingent_without_pivots_skips_the_dependency_sentence() {
        let outcome = json!({
            "kind": "contingent",
            "alternatives": {"I1": entity("Alice")},
            "pivots": [],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "Under I1 it is Alice.",
                "No single answer is determinate across the admissible completions.",
            ]
        );
    }

    #[test]
    fn suspended_with_one_request() {
        let outcome = json!({
            "kind": "suspended",
            "requests": [{"kind": "needCustom", "effect": "require", "payload": "requirement failed"}],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee is suspended.",
                "Outstanding request: requirement failed (require).",
            ]
        );
    }

    #[test]
    fn suspended_with_several_requests() {
        let outcome = json!({
            "kind": "suspended",
            "requests": [
                {"kind": "needEvidence", "issue": {}, "schema": "PhysicianCertificate"},
                {"kind": "needJudgment", "issue": {}, "protocol": "CourtCapacityDetermination"}
            ],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee is suspended.",
                "Outstanding requests: evidence matching PhysicianCertificate; a determination under CourtCapacityDetermination.",
            ]
        );
    }

    #[test]
    fn norm_conflict_names_the_doctrines() {
        let outcome = json!({
            "kind": "normConflict",
            "doctrines": [{"name": "LexSpecialis"}, {"name": "LexPosterior"}],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee ended in a norm conflict: no single applicable doctrine among LexSpecialis, LexPosterior."
            ]
        );
        let bare = json!({"kind": "normConflict", "doctrines": [], "trace": "0".repeat(32)});
        assert_eq!(
            sentences(&report(bare)),
            ["acting_trustee ended in a norm conflict."]
        );
    }

    #[test]
    fn outside_competence_gives_reason_and_request() {
        let outcome = json!({
            "kind": "outsideCompetence",
            "request": {"kind": "needApplicableLaw", "issue": "situs", "candidates": ["Massachusetts", "New York"]},
            "reason": "the situs of the land is not modeled",
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee is outside the module's competence: the situs of the land is not modeled.",
                "Request: applicable law among Massachusetts, New York.",
            ]
        );
    }

    #[test]
    fn inconsistent_lists_the_core() {
        let outcome = json!({
            "kind": "inconsistent",
            "core": ["Alive(Bryan)", "Dead(Bryan)"],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee has no consistent answer: the admitted model cannot be satisfied.",
                "Unsatisfiable core: Alive(Bryan), Dead(Bryan).",
            ]
        );
        let bare = json!({"kind": "inconsistent", "core": [], "trace": "0".repeat(32)});
        assert_eq!(
            sentences(&report(bare)),
            ["acting_trustee has no consistent answer: the admitted model cannot be satisfied."]
        );
    }

    #[test]
    fn unknown_kind_is_named_and_nothing_more() {
        let outcome = json!({"kind": "vacated", "order": "remand", "trace": "0".repeat(32)});
        assert_eq!(
            sentences(&report(outcome)),
            ["acting_trustee returned an outcome of kind vacated."]
        );
    }

    #[test]
    fn report_without_an_outcome() {
        assert_eq!(
            sentences(&report(Value::Null)),
            ["The report has no outcome."]
        );
        assert_eq!(sentences(&json!({})), ["The report has no outcome."]);
        assert_eq!(
            sentences(&report_with(Value::Null, &["tax"])),
            ["The report has no outcome.", "Outside scope: tax."]
        );
    }

    #[test]
    fn outside_scope_is_last_only_when_nonempty() {
        let outcome = json!({
            "kind": "determinate",
            "value": {"kind": "int", "data": 7},
            "trace": "0".repeat(32),
            "ignoredOpenIssues": []
        });
        assert_eq!(
            sentences(&report_with(outcome.clone(), &["complete_instruments"])),
            [
                "acting_trustee is 7.",
                "Outside scope: complete_instruments.",
            ]
        );
        assert_eq!(sentences(&report(outcome)), ["acting_trustee is 7."]);
    }

    #[test]
    fn value_text_covers_every_value_kind() {
        let cases = [
            (json!({"kind": "int", "data": 7}), "7"),
            (json!({"kind": "decimal", "data": "100.00"}), "100.00"),
            (json!({"kind": "bool", "data": true}), "true"),
            (
                json!({"kind": "instant", "data": "2034-03-01T09:00:00Z"}),
                "2034-03-01T09:00:00Z",
            ),
            (
                json!({"kind": "duration", "data": {"amount": 30, "kind": "counted_days"}}),
                r#"{"amount":30,"kind":"counted_days"}"#,
            ),
            (json!({"kind": "string", "data": "Alice"}), "\"Alice\""),
            (json!({"kind": "entity", "data": "Alice"}), "Alice"),
            (json!({"kind": "unit"}), "unit"),
            (
                json!({"kind": "ctor", "data": {"name": "Performed", "fields": {}}}),
                "Performed",
            ),
            (
                json!({"kind": "ctor", "data": {"name": "USD", "fields": {"_0": {"kind": "decimal", "data": "100.00"}}}}),
                "USD(100.00)",
            ),
            (
                json!({"kind": "ctor", "data": {"name": "Payment", "fields": {
                    "payer": entity("Alice"),
                    "amount": {"kind": "decimal", "data": "100.00"}
                }}}),
                "Payment(amount: 100.00, payer: Alice)",
            ),
            (
                json!({"kind": "set", "data": [entity("Alice"), entity("Bob")]}),
                "{Alice, Bob}",
            ),
            (json!({"kind": "set", "data": []}), "{}"),
            (
                json!({"kind": "map", "data": {"status": {"kind": "string", "data": "Due"}, "amount": {"kind": "int", "data": 3}}}),
                "{amount: 3, status: \"Due\"}",
            ),
            (json!({"kind": "option", "data": null}), "none"),
            (json!({"kind": "option", "data": entity("Bob")}), "Bob"),
        ];
        for (value, expected) in cases {
            assert_eq!(value_text(&value), expected, "{value}");
        }
    }

    #[test]
    fn value_text_orders_positional_fields_by_number() {
        let fields: Map<String, Value> = (0..11)
            .map(|i| (format!("_{i}"), json!({"kind": "int", "data": i})))
            .collect();
        let value = json!({"kind": "ctor", "data": {"name": "Row", "fields": fields}});
        assert_eq!(value_text(&value), "Row(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10)");
    }

    #[test]
    fn value_text_shows_unknown_shapes_as_compact_json() {
        let cases = [
            json!({"kind": "prop", "data": {"predicate": "Alive", "arguments": []}}),
            json!({"kind": "clauseRef", "data": {"module": "M", "clause": "4.4", "arguments": [], "digest": "00"}}),
            json!({"kind": "tensor", "data": [1, 2]}),
            json!({"kind": "set", "data": "not a list"}),
            json!({"kind": "ctor", "data": {"fields": {}}}),
            json!({"name": "no kind"}),
            json!(7),
            json!("bare"),
            Value::Null,
        ];
        for value in cases {
            assert_eq!(value_text(&value), value.to_string(), "{value}");
        }
    }

    #[test]
    fn request_text_covers_every_request_kind() {
        let cases = [
            (
                json!({"kind": "needCustom", "effect": "require", "payload": "requirement failed"}),
                "requirement failed (require)",
            ),
            (
                json!({"kind": "needCustom", "effect": "require", "payload": {"code": 3}}),
                "require",
            ),
            (
                json!({"kind": "needEvidence", "issue": {}, "schema": "PaymentRecord"}),
                "evidence matching PaymentRecord",
            ),
            (
                json!({"kind": "needInterpretation", "source": "Instrument", "family": "SuccessorEligibility"}),
                "an interpretation of SuccessorEligibility under Instrument",
            ),
            (
                json!({"kind": "needInterpretation", "family": "SuccessorEligibility"}),
                "an interpretation of SuccessorEligibility",
            ),
            (
                json!({"kind": "needJudgment", "issue": {}, "protocol": "CourtCapacityDetermination"}),
                "a determination under CourtCapacityDetermination",
            ),
            (
                json!({"kind": "needChoice", "protocol": "TrusteeDistributionDecision", "options": ["pay", "hold"]}),
                "a decision under TrusteeDistributionDecision among pay, hold",
            ),
            (
                json!({"kind": "needChoice", "protocol": "TrusteeDistributionDecision", "options": [1, 2]}),
                "a decision under TrusteeDistributionDecision",
            ),
            (
                json!({"kind": "needApplicableLaw", "issue": "situs", "candidates": ["Massachusetts", "New York"]}),
                "applicable law among Massachusetts, New York",
            ),
            (
                json!({"kind": "needApplicableLaw", "issue": "situs", "candidates": []}),
                "applicable law",
            ),
            (
                json!({"kind": "needConflict", "graph": [], "doctrines": ["LexSpecialis", "LexPosterior"]}),
                "one applicable conflict doctrine among LexSpecialis, LexPosterior",
            ),
            (
                json!({"kind": "needConflict", "graph": [], "doctrines": []}),
                "one applicable conflict doctrine",
            ),
            (json!({"kind": "needOracle", "question": "?"}), "needOracle"),
            (json!({"issue": "no kind"}), r#"{"issue":"no kind"}"#),
        ];
        for (request, expected) in cases {
            assert_eq!(request_text(&request), expected, "{request}");
        }
    }
}
