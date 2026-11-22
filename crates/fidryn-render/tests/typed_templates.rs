use fidryn_core::CoreModule;
use fidryn_render::{
    RenderError, RenderLimits, Template, module_values, render_module, render_values,
    render_values_with_limits,
};
use serde_json::json;

#[test]
fn nested_arrays_preserve_record_fields_order_and_names_with_commas() {
    let values = json!({
        "title": "Appointments",
        "offices": [
            {"name": "Trustee", "people": [{"name": "Doe, Jane"}, {"name": "Smith, John"}]},
            {"name": "Executor", "people": []},
        ]
    });
    let template = "{{title}}: {{#each offices}}{{item.name}}[{{#each item.people}}{{../item.name}}={{item.name}}/{{@root.title}};{{/each}}]{{/each}}";
    assert_eq!(
        render_values(template, &values).unwrap(),
        "Appointments: Trustee[Trustee=Doe, Jane/Appointments;Trustee=Smith, John/Appointments;]Executor[]"
    );
}

#[test]
fn empty_arrays_execute_zero_iterations_and_missing_values_remain_distinct() {
    assert_eq!(
        render_values(
            "A{{#each people}}{{item.absent}}{{/each}}B",
            &json!({"people": []})
        )
        .unwrap(),
        "AB"
    );
    assert_eq!(
        render_values("{{#each people}}{{item}}{{/each}}", &json!({})),
        Err(RenderError::MissingKey("people".into()))
    );
    for value in [json!(null), json!("Doe, Jane"), json!({"name":"Jane"})] {
        assert!(matches!(
            render_values(
                "{{#each people}}{{item}}{{/each}}",
                &json!({"people": value})
            ),
            Err(RenderError::TypeMismatch { .. })
        ));
    }
}

#[test]
fn malformed_templates_fail_even_when_their_collection_is_empty() {
    for template in [
        "{{",
        "}}",
        "{{}}",
        "{{name",
        "{{/each}}",
        "{{#each people}}",
        "{{#each}}",
        "{{#each people}}{{/if}}",
        "{{#if flag}}{{/if}}",
        "{{person..name}}",
        "{{@root..name}}",
        "{{#each people as person}}{{/each}}",
    ] {
        assert!(
            matches!(
                render_values(template, &json!({"people": []})),
                Err(RenderError::MalformedTemplate { .. })
            ),
            "{template}"
        );
    }
}

#[test]
fn scalar_paths_are_typed_and_inserted_values_are_never_reparsed() {
    assert_eq!(
        render_values(
            "{{person.name}} {{n}} {{flag}} {{rows.0}}",
            &json!({
                "person": {"name": "{{missing}}"}, "n": 3, "flag": false, "rows": ["first"]
            })
        )
        .unwrap(),
        "{{missing}} 3 false first"
    );
    assert_eq!(
        render_values("{{person.missing}}", &json!({"person": {}})),
        Err(RenderError::MissingKey("person.missing".into()))
    );
    assert!(matches!(
        render_values("{{person}}", &json!({"person": {}})),
        Err(RenderError::TypeMismatch { .. })
    ));
}

#[test]
fn source_nesting_nodes_output_iterations_and_work_are_bounded() {
    let values = json!({"items": ["abc", "def"], "text": "éé"});
    for (template, limits, resource) in [
        (
            "123",
            RenderLimits {
                max_template_bytes: 2,
                ..Default::default()
            },
            "source bytes",
        ),
        (
            "a{{text}}",
            RenderLimits {
                max_nodes: 1,
                ..Default::default()
            },
            "nodes",
        ),
        (
            "{{text}}",
            RenderLimits {
                max_output_bytes: 3,
                ..Default::default()
            },
            "output bytes",
        ),
        (
            "{{#each items}}{{item}}{{/each}}",
            RenderLimits {
                max_iterations: 1,
                ..Default::default()
            },
            "iterations",
        ),
        (
            "{{#each items}}{{item}}{{/each}}",
            RenderLimits {
                max_steps: 2,
                ..Default::default()
            },
            "steps",
        ),
    ] {
        assert!(
            matches!(render_values_with_limits(template, &values, limits), Err(RenderError::LimitExceeded { resource: actual, .. }) if actual == resource)
        );
    }
    let deep = format!(
        "{}x{}",
        "{{#each items}}".repeat(2000),
        "{{/each}}".repeat(2000)
    );
    assert!(matches!(
        Template::parse(&deep),
        Err(RenderError::LimitExceeded {
            resource: "nesting",
            ..
        })
    ));
}

#[test]
fn parsed_template_can_be_reused_without_leaking_loop_scopes() {
    let template = Template::parse("{{#each names}}[{{item}}]{{/each}}/{{title}}").unwrap();
    assert_eq!(
        template
            .render_values(&json!({"names": ["A", "B"], "title":"one"}))
            .unwrap(),
        "[A][B]/one"
    );
    assert_eq!(
        template
            .render_values(&json!({"names": [], "title":"two"}))
            .unwrap(),
        "/two"
    );
}

fn module() -> CoreModule {
    CoreModule {
        id: fidryn_core::ModuleId::of(b"m"),
        name: "Examples.T".into(),
        version: "0.1.0".into(),
        snapshot: fidryn_core::SourceSnapshotId::of(b"s"),
        manifest: fidryn_core::SourceManifestId::of(b"m"),
        jurisdiction: fidryn_core::JurisdictionId::of(b"j"),
        outside_scope: vec!["Doe, Jane".into(), "tax".into()],
        declarations: vec![],
        nominations: vec![],
        queries: vec![],
        verifications: vec![],
        assertions: vec![],
    }
}

#[test]
fn module_metadata_keeps_collections_typed() {
    let module = module();
    assert_eq!(
        module_values(&module)["outside_scope"],
        json!(["Doe, Jane", "tax"])
    );
    assert_eq!(
        render_module(
            "{{module}}:{{#each outside_scope}}[{{item}}]{{/each}}",
            &module
        )
        .unwrap(),
        "Examples.T:[Doe, Jane][tax]"
    );
    assert_eq!(
        render_module("{{outside_scope_label}}", &module).unwrap(),
        "Doe, Jane, tax"
    );
}
