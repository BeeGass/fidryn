//! Constrained templates. Missing keys fail closed.

use fidryn_core::{CoreModule, Value};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RenderError {
    #[error("missing template key `{0}`")]
    MissingKey(String),
    #[error("template `{0}` is not certified for semantics-preserving claims")]
    Uncertified(String),
}

pub fn render(template: &str, vars: &BTreeMap<String, String>) -> Result<String, RenderError> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str("{{");
            rest = after;
            continue;
        };
        let key = after[..end].trim();
        let Some(value) = vars.get(key) else {
            return Err(RenderError::MissingKey(key.to_owned()));
        };
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Interpolate Core module fields. Missing keys fail closed.
pub fn render_module(template: &str, module: &CoreModule) -> Result<String, RenderError> {
    render(template, &module_vars(module))
}

pub fn module_vars(module: &CoreModule) -> BTreeMap<String, String> {
    let mut vars = BTreeMap::new();
    vars.insert("module".into(), module.name.clone());
    vars.insert("version".into(), module.version.clone());
    vars.insert("outside_scope".into(), module.outside_scope.join(", "));
    vars
}

pub fn value_label(value: &Value) -> String {
    value.display_label()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_and_fails_closed() {
        let mut vars = BTreeMap::new();
        vars.insert("name".into(), "Bryan".into());
        assert_eq!(
            render("Trustee: {{name}}", &vars).unwrap(),
            "Trustee: Bryan"
        );
        assert!(matches!(
            render("{{missing}}", &vars),
            Err(RenderError::MissingKey(_))
        ));
    }

    fn sample_module() -> CoreModule {
        CoreModule {
            id: fidryn_core::ModuleId::of(b"m"),
            name: "Examples.T".into(),
            version: "0.1.0".into(),
            snapshot: fidryn_core::SourceSnapshotId::of(b"s"),
            manifest: fidryn_core::SourceManifestId::of(b"m"),
            jurisdiction: fidryn_core::JurisdictionId::of(b"j"),
            outside_scope: vec!["tax".into(), "creditor_priority".into()],
            declarations: vec![],
            queries: vec![],
            verifications: vec![],
            assertions: vec![],
        }
    }

    #[test]
    fn render_module_interpolates_core_fields() {
        let module = sample_module();
        let text = render_module(
            "{{module}}@{{version}}\noutside: {{outside_scope}}\n",
            &module,
        )
        .unwrap();
        assert_eq!(text, "Examples.T@0.1.0\noutside: tax, creditor_priority\n");
        assert!(matches!(
            render_module("{{unknown}}", &module),
            Err(RenderError::MissingKey(k)) if k == "unknown"
        ));
    }
}
