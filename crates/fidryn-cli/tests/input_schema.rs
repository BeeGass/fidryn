use fidryn_cli::compile_source;
use fidryn_cli::inputs::{diagnostics_data, query_input_schema};
use fidryn_core::{Diagnostic, DiagnosticCode, SourceManifest, Span, Value};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

fn compile(body: &str) -> fidryn_core::CoreModule {
    compile_source(
        &format!("module Inputs version \"1\" {{ {body} }}"),
        &SourceManifest::default(),
    )
    .unwrap()
}

fn assert_references_exist(value: &Json, root: &Json) {
    match value {
        Json::Object(fields) => {
            if let Some(reference) = fields.get("$ref").and_then(Json::as_str) {
                assert!(
                    root.pointer(reference.strip_prefix('#').unwrap()).is_some(),
                    "{reference}"
                );
            }
            for value in fields.values() {
                assert_references_exist(value, root);
            }
        }
        Json::Array(values) => {
            for value in values {
                assert_references_exist(value, root);
            }
        }
        _ => {}
    }
}

#[test]
fn input_schema_derives_aliases_sums_records_entities_and_recursive_refs() {
    let module = compile(
        r#"
        entity Alice: NaturalPerson
        type Count = Int
        type Event = Started | Measured(Count)
        record_type Node { name: String, next: Option<Node> }
        proposition Read()
        observation ReadNode(node: Node) establishes Read() {require true}
        query q(person: NaturalPerson, count: Count, event: Event, nodes: FiniteSet<Node>, amount: Money<USD>) -> Bool {return true}
    "#,
    );
    let schema = query_input_schema(&module, "q").unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["count"]["x-fidryn-type"], "Count");
    assert_eq!(
        schema["properties"]["person"]["anyOf"][0]["enum"],
        json!(["Alice"])
    );
    assert_eq!(
        schema["x-fidryn-program-digest"],
        module.program_digest().unwrap().to_string()
    );
    assert_eq!(schema["x-fidryn-observations"][0]["name"], "ReadNode");
    assert_eq!(
        schema["x-fidryn-observations"][0]["validators"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_references_exist(&schema, &schema);
    assert!(
        schema["$defs"]
            .as_object()
            .unwrap()
            .keys()
            .any(|key| key.contains("Node"))
    );
    let count = Value::from_case_json(json!(7)).unwrap();
    let count_type = &module.query("q").unwrap().binders[1].1;
    assert!(fidryn_eval::runtime_value_matches(
        &module, &count, count_type
    ));
}

#[test]
fn generic_constructor_schemas_bind_payload_types() {
    let module =
        compile("type Box<T> = Boxed(T) | Empty; query q(value: Box<Int>) -> Bool {return true}");
    let schema = query_input_schema(&module, "q").unwrap();
    assert_references_exist(&schema, &schema);
    let value = Value::from_case_json(
        json!({"kind":"ctor","data":{"name":"Boxed","fields":{"_0":{"kind":"int","data":3}}}}),
    )
    .unwrap();
    assert!(fidryn_eval::runtime_value_matches(
        &module,
        &value,
        &module.query("q").unwrap().binders[0].1
    ));
    let value=Value::from_case_json(json!({"kind":"ctor","data":{"name":"Boxed","fields":{"_0":{"kind":"string","data":"wrong"}}}})).unwrap();
    assert!(!fidryn_eval::runtime_value_matches(
        &module,
        &value,
        &module.query("q").unwrap().binders[0].1
    ));
}

#[test]
fn unknown_input_types_and_queries_have_explicit_errors() {
    let module = compile("query q(value: Unresolved) -> Bool {return true}");
    assert!(
        query_input_schema(&module, "q")
            .unwrap_err()
            .contains("unsupported input type")
    );
    assert!(
        query_input_schema(&module, "missing")
            .unwrap_err()
            .contains("unknown query")
    );
    let module = compile("query q(person: NaturalPerson) -> Bool {return true}");
    let schema = query_input_schema(&module, "q").unwrap();
    assert_eq!(
        schema["properties"]["person"]["anyOf"][0]["allOf"],
        json!([false])
    );
    assert!(schema["properties"]["person"]["anyOf"][0]["x-fidryn-unavailable"].is_string());
}

#[test]
fn diagnostic_locations_use_the_owning_file_and_utf16_columns() {
    let source = "let x = \"😀\";\nclaim";
    let start = source.find('😀').unwrap() as u32;
    let mut diagnostic = Diagnostic::new(DiagnosticCode::E210, "wrong type")
        .with_source("/workspace/module.fr")
        .with_span(Span {
            start,
            end: start + 4,
        });
    diagnostic.related_spans.push(Span {
        start: start + 1,
        end: start + 2,
    });
    let data = diagnostics_data(
        &[diagnostic],
        &BTreeMap::from([("/workspace/module.fr".into(), source.into())]),
    );
    let primary = &data["diagnostics"][0]["primary"];
    assert_eq!(primary["range"]["start"], json!({"line":0,"character":9}));
    assert_eq!(primary["range"]["end"], json!({"line":0,"character":11}));
    assert_eq!(
        data["diagnostics"][0]["related"][0]["location_available"],
        false
    );
}

#[test]
fn missing_source_never_gets_an_invented_editor_range() {
    let diagnostic = Diagnostic::new(DiagnosticCode::E100, "invalid")
        .with_source("/package/import.fr")
        .with_span(Span { start: 0, end: 1 });
    let data = diagnostics_data(
        &[diagnostic],
        &BTreeMap::from([("/workspace/module.fr".into(), "unrelated".into())]),
    );
    assert_eq!(data["diagnostics"][0]["source_file"], "/package/import.fr");
    assert_eq!(data["diagnostics"][0]["primary"]["range"], Json::Null);
    assert_eq!(
        data["diagnostics"][0]["primary"]["location_available"],
        false
    );
}
