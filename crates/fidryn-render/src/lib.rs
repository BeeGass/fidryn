//! Parsed, bounded templates over typed values. Missing keys fail closed.
//!
//! Interpolation is not semantic certification. Template approval is an
//! explicit digest plus `CertifiedTemplate::approved`.

use fidryn_core::{CoreModule, Value};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RenderError {
    #[error("missing template key `{0}`")]
    MissingKey(String),
    #[error("malformed template at byte {offset}: {message}")]
    MalformedTemplate { offset: usize, message: String },
    #[error("template key `{path}` requires {expected}, found {actual}")]
    TypeMismatch {
        path: String,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("template {resource} limit exceeded ({limit})")]
    LimitExceeded {
        resource: &'static str,
        limit: usize,
    },
    #[error("template `{0}` is not certified for semantics-preserving claims")]
    Uncertified(String),
    #[error("template digest mismatch")]
    DigestMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderLimits {
    pub max_template_bytes: usize,
    pub max_nesting: usize,
    pub max_nodes: usize,
    pub max_iterations: usize,
    pub max_steps: usize,
    pub max_output_bytes: usize,
}

impl Default for RenderLimits {
    fn default() -> Self {
        Self {
            max_template_bytes: 1_048_576,
            max_nesting: 32,
            max_nodes: 100_000,
            max_iterations: 10_000,
            max_steps: 1_000_000,
            max_output_bytes: 8_388_608,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Text(String),
    Interpolate(ValuePath),
    Each { path: ValuePath, body: Vec<Node> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ValuePath {
    source: String,
    parents: usize,
    root: bool,
    segments: Vec<String>,
}

impl ValuePath {
    fn parse(source: &str, offset: usize) -> Result<Self, RenderError> {
        let mut rest = source;
        let mut parents = 0;
        while let Some(tail) = rest.strip_prefix("../") {
            parents += 1;
            rest = tail;
        }
        let root = rest == "@root" || rest.starts_with("@root.");
        if root {
            if parents > 0 {
                return Err(malformed(offset, "root paths cannot have parent prefixes"));
            }
            rest = rest.strip_prefix("@root").unwrap();
            rest = rest.strip_prefix('.').unwrap_or(rest);
        }
        let segments = if rest.is_empty() && root {
            Vec::new()
        } else {
            rest.split('.').map(str::to_owned).collect::<Vec<_>>()
        };
        if segments.iter().any(|segment| {
            segment.is_empty()
                || !segment
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        }) || (segments.is_empty() && !root)
        {
            return Err(malformed(offset, "expected a field path"));
        }
        Ok(Self {
            source: source.into(),
            parents,
            root,
            segments,
        })
    }
}

/// A template parsed once before any values are interpolated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    nodes: Vec<Node>,
    limits: RenderLimits,
}

impl Template {
    pub fn parse(source: &str) -> Result<Self, RenderError> {
        Self::parse_with_limits(source, RenderLimits::default())
    }

    pub fn parse_with_limits(source: &str, limits: RenderLimits) -> Result<Self, RenderError> {
        if source.len() > limits.max_template_bytes {
            return Err(limit("source bytes", limits.max_template_bytes));
        }
        let mut parser = TemplateParser {
            source,
            offset: 0,
            node_count: 0,
            limits,
        };
        let nodes = parser.parse_nodes(0, false)?;
        Ok(Self { nodes, limits })
    }

    pub fn render_values(&self, values: &JsonValue) -> Result<String, RenderError> {
        let mut renderer = Renderer {
            root: values,
            output: String::new(),
            iterations: 0,
            steps: 0,
            limits: self.limits,
        };
        renderer.render_nodes(&self.nodes, &mut Vec::new())?;
        Ok(renderer.output)
    }
}

struct TemplateParser<'a> {
    source: &'a str,
    offset: usize,
    node_count: usize,
    limits: RenderLimits,
}

impl TemplateParser<'_> {
    fn push(&mut self, nodes: &mut Vec<Node>, node: Node) -> Result<(), RenderError> {
        if self.node_count >= self.limits.max_nodes {
            return Err(limit("nodes", self.limits.max_nodes));
        }
        self.node_count += 1;
        nodes.push(node);
        Ok(())
    }

    fn parse_nodes(&mut self, depth: usize, in_each: bool) -> Result<Vec<Node>, RenderError> {
        // Keep the recursive parser bounded even if a caller raises its limit.
        let nesting_limit = self.limits.max_nesting.min(128);
        if depth > nesting_limit {
            return Err(limit("nesting", nesting_limit));
        }
        let mut nodes = Vec::new();
        while self.offset < self.source.len() {
            let rest = &self.source[self.offset..];
            let next_open = rest.find("{{");
            if let Some(close) = rest.find("}}")
                && next_open.is_none_or(|open| close < open)
            {
                return Err(malformed(self.offset + close, "unexpected `}}`"));
            }
            let Some(start) = next_open else {
                self.push(&mut nodes, Node::Text(rest.into()))?;
                self.offset = self.source.len();
                break;
            };
            if start > 0 {
                self.push(&mut nodes, Node::Text(rest[..start].into()))?;
            }
            self.offset += start;
            let tag_offset = self.offset;
            let after = &self.source[self.offset + 2..];
            let end = after
                .find("}}")
                .ok_or_else(|| malformed(tag_offset, "unclosed tag"))?;
            let tag = after[..end].trim();
            self.offset += 2 + end + 2;
            if tag == "/each" {
                if !in_each {
                    return Err(malformed(tag_offset, "unexpected `/each`"));
                }
                return Ok(nodes);
            }
            if let Some(path) = tag.strip_prefix("#each")
                && path.starts_with(char::is_whitespace)
            {
                let path = ValuePath::parse(path.trim(), tag_offset)?;
                let body = self.parse_nodes(depth + 1, true)?;
                self.push(&mut nodes, Node::Each { path, body })?;
            } else if tag.starts_with(['#', '/']) {
                return Err(malformed(tag_offset, "unsupported or malformed block tag"));
            } else {
                let path = ValuePath::parse(tag, tag_offset)?;
                self.push(&mut nodes, Node::Interpolate(path))?;
            }
        }
        if in_each {
            Err(malformed(self.offset, "unclosed `each` block"))
        } else {
            Ok(nodes)
        }
    }
}

struct Renderer<'a> {
    root: &'a JsonValue,
    output: String,
    iterations: usize,
    steps: usize,
    limits: RenderLimits,
}

impl<'a> Renderer<'a> {
    fn append(&mut self, text: &str) -> Result<(), RenderError> {
        if self
            .output
            .len()
            .checked_add(text.len())
            .is_none_or(|size| size > self.limits.max_output_bytes)
        {
            return Err(limit("output bytes", self.limits.max_output_bytes));
        }
        self.output.push_str(text);
        Ok(())
    }

    fn resolve(
        &self,
        path: &ValuePath,
        scopes: &[&'a JsonValue],
    ) -> Result<&'a JsonValue, RenderError> {
        let missing = || RenderError::MissingKey(path.source.clone());
        let scope = if path.root {
            self.root
        } else if path.parents > scopes.len() {
            return Err(missing());
        } else {
            scopes
                .len()
                .checked_sub(path.parents + 1)
                .and_then(|index| scopes.get(index))
                .copied()
                .unwrap_or(self.root)
        };
        let mut segments = path.segments.iter();
        let mut value = if path.root {
            scope
        } else if let Some(first) = segments.next() {
            if first == "item" || first == "this" {
                scope
            } else {
                scope
                    .get(first)
                    .or_else(|| self.root.get(first))
                    .ok_or_else(missing)?
            }
        } else {
            scope
        };
        for segment in segments {
            value = match value {
                JsonValue::Array(items) => segment
                    .parse::<usize>()
                    .ok()
                    .and_then(|index| items.get(index)),
                JsonValue::Object(fields) => fields.get(segment),
                _ => None,
            }
            .ok_or_else(missing)?;
        }
        Ok(value)
    }

    fn render_nodes(
        &mut self,
        nodes: &[Node],
        scopes: &mut Vec<&'a JsonValue>,
    ) -> Result<(), RenderError> {
        for node in nodes {
            if self.steps >= self.limits.max_steps {
                return Err(limit("steps", self.limits.max_steps));
            }
            self.steps += 1;
            match node {
                Node::Text(text) => self.append(text)?,
                Node::Interpolate(path) => match self.resolve(path, scopes)? {
                    JsonValue::String(text) => self.append(text)?,
                    JsonValue::Bool(value) => self.append(if *value { "true" } else { "false" })?,
                    JsonValue::Number(value) => self.append(&value.to_string())?,
                    value => {
                        return Err(RenderError::TypeMismatch {
                            path: path.source.clone(),
                            expected: "a string, number, or Boolean",
                            actual: value_kind(value),
                        });
                    }
                },
                Node::Each { path, body } => {
                    let value = self.resolve(path, scopes)?;
                    let items = value.as_array().ok_or_else(|| RenderError::TypeMismatch {
                        path: path.source.clone(),
                        expected: "an array",
                        actual: value_kind(value),
                    })?;
                    for item in items {
                        if self.iterations >= self.limits.max_iterations {
                            return Err(limit("iterations", self.limits.max_iterations));
                        }
                        self.iterations += 1;
                        scopes.push(item);
                        let result = self.render_nodes(body, scopes);
                        scopes.pop();
                        result?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn malformed(offset: usize, message: &str) -> RenderError {
    RenderError::MalformedTemplate {
        offset,
        message: message.into(),
    }
}

fn limit(resource: &'static str, limit: usize) -> RenderError {
    RenderError::LimitExceeded { resource, limit }
}

fn value_kind(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "a Boolean",
        JsonValue::Number(_) => "a number",
        JsonValue::String(_) => "a string",
        JsonValue::Array(_) => "an array",
        JsonValue::Object(_) => "an object",
    }
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

/// Interpolate strings. Iteration requires [`render_values`] with an array.
pub fn render(template: &str, vars: &BTreeMap<String, String>) -> Result<String, RenderError> {
    let values = JsonValue::Object(
        vars.iter()
            .map(|(key, value)| (key.clone(), JsonValue::String(value.clone())))
            .collect(),
    );
    render_values(template, &values)
}

/// Render scalar paths and nested `each` blocks over JSON arrays of typed values.
/// `item` addresses the current element, `../item` its parent, and `@root` the root.
pub fn render_values(template: &str, values: &JsonValue) -> Result<String, RenderError> {
    Template::parse(template)?.render_values(values)
}

pub fn render_values_with_limits(
    template: &str,
    values: &JsonValue,
    limits: RenderLimits,
) -> Result<String, RenderError> {
    Template::parse_with_limits(template, limits)?.render_values(values)
}

fn require_approval(spec: &CertifiedTemplate<'_>) -> Result<(), RenderError> {
    if !spec.approved {
        return Err(RenderError::Uncertified(spec.name.to_owned()));
    }
    if template_digest(spec.source) != spec.digest_hex {
        return Err(RenderError::DigestMismatch);
    }
    Ok(())
}

/// Interpolate only if the digest matches and `approved` is set.
pub fn render_certified(
    spec: CertifiedTemplate<'_>,
    vars: &BTreeMap<String, String>,
) -> Result<String, RenderError> {
    require_approval(&spec)?;
    render(spec.source, vars)
}

pub fn render_certified_values(
    spec: CertifiedTemplate<'_>,
    values: &JsonValue,
) -> Result<String, RenderError> {
    require_approval(&spec)?;
    render_values(spec.source, values)
}

/// Render Core module metadata. Approval still requires [`render_certified_values`].
pub fn render_module(template: &str, module: &CoreModule) -> Result<String, RenderError> {
    render_values(template, &module_values(module))
}

pub fn module_values(module: &CoreModule) -> JsonValue {
    serde_json::json!({
        "module": module.name,
        "version": module.version,
        "outside_scope": module.outside_scope,
        "outside_scope_label": module.outside_scope.join(", "),
    })
}

pub fn module_vars(module: &CoreModule) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("module".into(), module.name.clone()),
        ("version".into(), module.version.clone()),
        ("outside_scope".into(), module.outside_scope.join(", ")),
        (
            "outside_scope_label".into(),
            module.outside_scope.join(", "),
        ),
    ])
}

pub fn value_label(value: &Value) -> String {
    value.display_label()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_interpolate_and_missing_keys_fail_closed() {
        let vars = BTreeMap::from([("name".into(), "Bryan".into())]);
        assert_eq!(
            render("Trustee: {{name}}", &vars).unwrap(),
            "Trustee: Bryan"
        );
        assert_eq!(
            render("{{missing}}", &vars),
            Err(RenderError::MissingKey("missing".into()))
        );
        assert!(matches!(
            render("{{#each name}}{{item}}{{/each}}", &vars),
            Err(RenderError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn path_names_never_approve_a_template() {
        let values = serde_json::json!({"name": "Bryan"});
        let source = "Trustee: {{name}}";
        let mut spec = CertifiedTemplate {
            name: "templates/certified/instrument-outline.txt",
            source,
            digest_hex: "deadbeef",
            approved: false,
        };
        assert!(matches!(
            render_certified_values(spec.clone(), &values),
            Err(RenderError::Uncertified(_))
        ));
        spec.approved = true;
        assert_eq!(
            render_certified_values(spec.clone(), &values),
            Err(RenderError::DigestMismatch)
        );
        let digest = template_digest(source);
        spec.digest_hex = &digest;
        assert_eq!(
            render_certified_values(spec, &values).unwrap(),
            "Trustee: Bryan"
        );
    }
}
