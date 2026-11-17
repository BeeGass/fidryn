//! The files a check of a workspace writes on request: the check report,
//! `fidryn.check-report/v0.1`, and the list of the references whose outcome
//! is not `exact`.

use super::check::WorkspaceReport;
use super::index::LAYERS;
use fidryn_provision::{Layer, Outcome};
use serde_json::{Map, Value};

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

impl WorkspaceReport {
    /// The check report: `schema`, the counts `modules`, `from_cache`,
    /// `errors` and `warnings`, `references`, with an object for each layer
    /// that counts each outcome, in the order of [`Outcome::NAMES`], and
    /// `outside_by_body`, with the bodies of law sorted. It is written with
    /// two spaces of indentation and a final line end, and holds no path, so
    /// it is the same on every host.
    pub fn to_json(&self) -> String {
        let mut references = Map::new();
        for layer in LAYERS {
            let counts = self.references.get(&layer);
            let mut outcomes = Map::new();
            for name in Outcome::NAMES {
                let count = counts.and_then(|counts| counts.get(name)).copied();
                outcomes.insert(name.to_owned(), Value::from(count.unwrap_or(0)));
            }
            references.insert(layer_name(layer).to_owned(), Value::Object(outcomes));
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
        report.insert("outside_by_body".to_owned(), Value::Object(outside));
        let mut text = serde_json::to_string_pretty(&Value::Object(report))
            .expect("a report of strings and numbers is written");
        text.push('\n');
        text
    }

    /// The unresolved list: for every reference whose outcome is not
    /// `exact`, in the order of [`WorkspaceReport::unresolved`], one line of
    /// compact JSON with the keys `from`, `layer`, `target` and `outcome`,
    /// in that order, each line with its line end.
    pub fn unresolved_lines(&self) -> String {
        let mut text = String::new();
        for unresolved in &self.unresolved {
            let mut line = Map::new();
            line.insert("from".to_owned(), Value::from(unresolved.from.as_str()));
            line.insert(
                "layer".to_owned(),
                Value::from(layer_name(unresolved.layer)),
            );
            line.insert("target".to_owned(), Value::from(unresolved.target.as_str()));
            line.insert("outcome".to_owned(), Value::from(unresolved.outcome.name()));
            text.push_str(
                &serde_json::to_string(&Value::Object(line)).expect("a line of strings is written"),
            );
            text.push('\n');
        }
        text
    }
}
