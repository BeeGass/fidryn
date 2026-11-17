//! The files a check of a workspace writes on request: the check report,
//! `fidryn.check-report/v0.1`, and the list of the references and the
//! citations whose outcome is not `exact`.

use super::check::WorkspaceReport;
use super::index::{LAYERS, Unresolved, UnresolvedCitation};
use fidryn_provision::{Layer, Outcome};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The `schema` of a check report.
pub const CHECK_REPORT_SCHEMA: &str = "fidryn.check-report/v0.1";

/// The name of `layer` in the files of a check.
fn layer_name(layer: Layer) -> &'static str {
    match layer {
        Layer::Operative => "operative",
        Layer::Enacted => "enacted",
        Layer::Editorial => "editorial",
    }
}

/// An object that counts each outcome of `counts`, in the order of
/// [`Outcome::NAMES`], an outcome that `counts` lacks counted 0.
fn by_outcome(counts: Option<&BTreeMap<String, u64>>) -> Value {
    let mut outcomes = Map::new();
    for name in Outcome::NAMES {
        let count = counts.and_then(|counts| counts.get(name)).copied();
        outcomes.insert(name.to_owned(), Value::from(count.unwrap_or(0)));
    }
    Value::Object(outcomes)
}

/// The line of the unresolved list of `unresolved`, without its line end.
fn reference_line(unresolved: &Unresolved) -> String {
    let mut line = Map::new();
    line.insert("from".to_owned(), Value::from(unresolved.from.as_str()));
    line.insert(
        "layer".to_owned(),
        Value::from(layer_name(unresolved.layer)),
    );
    line.insert("target".to_owned(), Value::from(unresolved.target.as_str()));
    line.insert("outcome".to_owned(), Value::from(unresolved.outcome.name()));
    serde_json::to_string(&Value::Object(line)).expect("a line of strings is written")
}

/// The line of the unresolved list of `unresolved`, without its line end.
fn citation_line(unresolved: &UnresolvedCitation) -> String {
    let mut line = Map::new();
    line.insert("from".to_owned(), Value::from(unresolved.from.as_str()));
    line.insert(
        "citation".to_owned(),
        Value::from(unresolved.citation.as_str()),
    );
    line.insert("target".to_owned(), Value::from(unresolved.target.as_str()));
    line.insert("outcome".to_owned(), Value::from(unresolved.outcome.name()));
    serde_json::to_string(&Value::Object(line)).expect("a line of strings is written")
}

impl WorkspaceReport {
    /// The check report: `schema`, the counts `modules`, `from_cache`,
    /// `errors` and `warnings`, `references`, with an object for each layer
    /// that counts each outcome, in the order of [`Outcome::NAMES`],
    /// `citations`, an object that counts the citations with each outcome in
    /// the same order, and `outside_by_body`, with the bodies of law sorted.
    /// It is written with two spaces of indentation and a final line end,
    /// and holds no path, so it is the same on every host.
    pub fn to_json(&self) -> String {
        let mut references = Map::new();
        for layer in LAYERS {
            references.insert(
                layer_name(layer).to_owned(),
                by_outcome(self.references.get(&layer)),
            );
        }
        let outside: Map<String, Value> = self
            .outside_by_body
            .iter()
            .map(|(body, count)| (body.clone(), Value::from(*count)))
            .collect();
        let mut report = Map::new();
        report.insert("schema".to_owned(), Value::from(CHECK_REPORT_SCHEMA));
        report.insert("modules".to_owned(), Value::from(self.modules));
        report.insert("from_cache".to_owned(), Value::from(self.from_cache));
        report.insert("errors".to_owned(), Value::from(self.errors()));
        report.insert("warnings".to_owned(), Value::from(self.warnings()));
        report.insert("references".to_owned(), Value::Object(references));
        report.insert("citations".to_owned(), by_outcome(Some(&self.citations)));
        report.insert("outside_by_body".to_owned(), Value::Object(outside));
        let mut text = serde_json::to_string_pretty(&Value::Object(report))
            .expect("a report of strings and numbers is written");
        text.push('\n');
        text
    }

    /// The unresolved list: one line of compact JSON, with its line end,
    /// for every reference and every citation whose outcome is not `exact`.
    /// The line of a reference has the keys `from`, `layer`, `target` and
    /// `outcome`, in that order, and that of a citation `from`, the address
    /// of its module, `citation`, its name, `target` and `outcome`. The
    /// lines are sorted by `from`, a reference before a citation of the same
    /// `from`, and then as [`WorkspaceReport::unresolved`] and
    /// [`WorkspaceReport::unresolved_citations`] are.
    pub fn unresolved_lines(&self) -> String {
        let mut text = String::new();
        let mut references = self.unresolved.iter().peekable();
        let mut citations = self.unresolved_citations.iter().peekable();
        loop {
            let reference_first = match (references.peek(), citations.peek()) {
                (None, None) => break,
                (Some(reference), Some(citation)) => reference.from <= citation.from,
                (reference, _) => reference.is_some(),
            };
            let line = if reference_first {
                references.next().map(reference_line)
            } else {
                citations.next().map(citation_line)
            };
            text.push_str(&line.expect("a line was peeked"));
            text.push('\n');
        }
        text
    }
}
