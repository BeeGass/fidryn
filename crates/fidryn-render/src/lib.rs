//! Constrained templates. Missing keys fail closed.
//!
//! Interpolation is not semantic certification. A path segment named
//! `certified` does not approve a template. Approval is an explicit
//! digest plus `CertifiedTemplate::approved`.

use fidryn_core::{CoreModule, Value};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RenderError {
    #[error("missing template key `{0}`")]
    MissingKey(String),
    #[error("template `{0}` is not certified for semantics-preserving claims")]
    Uncertified(String),
    #[error("template digest mismatch")]
    DigestMismatch,
}

/// Explicit approval record. A directory named `certified` is not this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertifiedTemplate<'a> {
    pub name: &'a str,
    pub source: &'a str,
    pub digest_hex: &'a str,
    pub approved: bool,
}

pub fn template_digest(source: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"fidryn-template");
    hasher.update(&[0xff]);
    hasher.update(source.as_bytes());
    hasher.finalize().to_hex().to_string()
}

/// Interpolate only. The result is not a certified legal claim.
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
        if let Some(each_key) = each_block_key(key) {
            let Some(list) = vars.get(each_key) else {
                return Err(RenderError::MissingKey(each_key.to_owned()));
            };
            let body_and_rest = &after[end + 2..];
            let Some((inner, after_block)) = split_each_body(body_and_rest) else {
                return Err(RenderError::MissingKey("/each".into()));
            };
            for item in list.split(", ") {
                let mut item_vars = vars.clone();
                item_vars.insert("item".into(), item.to_owned());
                out.push_str(&render(inner, &item_vars)?);
            }
            rest = after_block;
            continue;
        }
        let Some(value) = vars.get(key) else {
            return Err(RenderError::MissingKey(key.to_owned()));
        };
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

fn each_block_key(tag: &str) -> Option<&str> {
    let rest = tag.strip_prefix("#each")?;
    if rest.is_empty() {
        return Some("");
    }
    if rest.starts_with(|c: char| c.is_whitespace()) {
        return Some(rest.trim());
    }
    None
}

fn split_each_body(src: &str) -> Option<(&str, &str)> {
    let mut rest = src;
    let mut offset = 0;
    while let Some(rel) = rest.find("{{") {
        let after = &rest[rel + 2..];
        let end = after.find("}}")?;
        if after[..end].trim() == "/each" {
            let close_start = offset + rel;
            let close_end = close_start + 2 + end + 2;
            return Some((&src[..close_start], &src[close_end..]));
        }
        let consumed = rel + 2 + end + 2;
        offset += consumed;
        rest = &rest[consumed..];
    }
    None
}

/// Interpolate only if the digest matches and `approved` is set.
/// Path names such as `templates/certified/` do not satisfy this.
pub fn render_certified(
    spec: CertifiedTemplate<'_>,
    vars: &BTreeMap<String, String>,
) -> Result<String, RenderError> {
    if !spec.approved {
        return Err(RenderError::Uncertified(spec.name.to_owned()));
    }
    if template_digest(spec.source) != spec.digest_hex {
        return Err(RenderError::DigestMismatch);
    }
    render(spec.source, vars)
}

/// Interpolate Core module fields. Missing keys fail closed.
/// Not certified unless [`render_certified`] is used.
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
            nominations: vec![],
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

    #[test]
    fn each_block_repeats_comma_separated_items() {
        let mut vars = BTreeMap::new();
        vars.insert("outside_scope".into(), "tax, creditor_priority".into());
        vars.insert("module".into(), "Examples.T".into());
        assert_eq!(
            render(
                "{{module}}: {{#each outside_scope}}[{{item}}]{{/each}}",
                &vars
            )
            .unwrap(),
            "Examples.T: [tax][creditor_priority]"
        );
        assert!(matches!(
            render("{{#each missing}}{{item}}{{/each}}", &vars),
            Err(RenderError::MissingKey(k)) if k == "missing"
        ));
        assert!(matches!(
            render("{{#each outside_scope}}{{item}}", &vars),
            Err(RenderError::MissingKey(k)) if k == "/each"
        ));
    }

    #[test]
    fn path_named_certified_is_not_approval() {
        let mut vars = BTreeMap::new();
        vars.insert("module".into(), "Examples.T".into());
        let source = "Module: {{module}}";
        let spec = CertifiedTemplate {
            name: "templates/certified/instrument-outline.txt",
            source,
            digest_hex: "deadbeef",
            approved: false,
        };
        assert!(matches!(
            render_certified(spec, &vars),
            Err(RenderError::Uncertified(_))
        ));
        let spec = CertifiedTemplate {
            name: "templates/certified/instrument-outline.txt",
            source,
            digest_hex: "deadbeef",
            approved: true,
        };
        assert!(matches!(
            render_certified(spec, &vars),
            Err(RenderError::DigestMismatch)
        ));
        let digest = template_digest(source);
        let spec = CertifiedTemplate {
            name: "instrument-outline",
            source,
            digest_hex: digest.as_str(),
            approved: true,
        };
        assert_eq!(render_certified(spec, &vars).unwrap(), "Module: Examples.T");
    }
}
