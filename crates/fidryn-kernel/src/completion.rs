//! Declared completion membership for covering replay.
//!
//! Coverage is exact assignment membership in the Cartesian product of
//! declared interpretation, choice, and evidence domains, not raw map
//! cardinality. Undeclared keys are not worlds.

use fidryn_core::{CaseRecord, CoverageWitness, EvidenceItem, Instant, RunContext, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const INTERPRETATION_NS: &str = "i:";
pub const EVIDENCE_NS: &str = "e:";
pub const CHOICE_NS: &str = "c:";

/// One declared completion dimension after recorded-selection narrowing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionSlot {
    pub name: String,
    pub values: Vec<Value>,
}

/// Checked finite completion space for a case record.
///
/// Slots are declared interpretation (`i:`), choice (`c:`), and evidence
/// (`e:`) domains only. Zero slots is one empty assignment. A declared
/// empty domain admits no assignments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedCompletionModel {
    slots: Vec<CompletionSlot>,
}

impl ValidatedCompletionModel {
    /// Build the admitted space from declared domains and recorded selections.
    pub fn from_case(case: &CaseRecord) -> Result<Self, String> {
        let mut slots = Vec::new();
        let completions = &case.admissible_completions;
        for (family, alternatives) in &completions.interpretations {
            if family.is_empty() {
                return Err("declared interpretation family must be nonempty".into());
            }
            slots.push(CompletionSlot {
                name: format!("{INTERPRETATION_NS}{family}"),
                values: recorded_or_declared(case.interpretations.get(family), alternatives),
            });
        }
        for (schema, domain) in &completions.evidence {
            if schema.is_empty() {
                return Err("declared evidence schema must be nonempty".into());
            }
            slots.push(CompletionSlot {
                name: format!("{EVIDENCE_NS}{schema}"),
                values: unique_domain_values(
                    domain
                        .responses
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            });
        }
        for (protocol, options) in &completions.choices {
            if protocol.is_empty() {
                return Err("declared choice protocol must be nonempty".into());
            }
            slots.push(CompletionSlot {
                name: format!("{CHOICE_NS}{protocol}"),
                values: recorded_or_declared(case.decisions.get(protocol), options),
            });
        }
        Ok(Self { slots })
    }

    pub fn slots(&self) -> &[CompletionSlot] {
        &self.slots
    }

    /// Cartesian size. Zero slots is 1. Any empty slot is 0.
    pub fn product_size(&self) -> Result<usize, String> {
        if self.slots.is_empty() {
            return Ok(1);
        }
        self.slots
            .iter()
            .map(|slot| slot.values.len())
            .try_fold(1usize, |acc, size| acc.checked_mul(size))
            .ok_or_else(|| "declared admissible completion product overflows usize".to_string())
    }

    /// Project and admit each witness branch as an exact assignment.
    pub fn admit_witness<'a>(
        &self,
        witness: &'a CoverageWitness,
    ) -> Result<Vec<&'a BTreeMap<String, Value>>, String> {
        let expected = self.product_size()?;
        if expected == 0 {
            return Err("declared empty completion domain admits no assignments".into());
        }
        if witness.branches.len() != expected
            || witness.total != expected
            || witness.examined != expected
        {
            return Err(format!(
                "coverage witness total {} / branches {} does not match declared completion product {expected}",
                witness.total,
                witness.branches.len()
            ));
        }
        let slot_names: BTreeSet<&str> = self.slots.iter().map(|slot| slot.name.as_str()).collect();
        let mut admitted: Vec<&BTreeMap<String, Value>> =
            Vec::with_capacity(witness.branches.len());
        let mut unique: BTreeSet<String> = BTreeSet::new();
        for branch in &witness.branches {
            let canonical = self.admit_assignment(&branch.bindings, &slot_names)?;
            let identity = fidryn_core::canonical_json(&canonical).map_err(|e| e.to_string())?;
            if !unique.insert(identity) {
                return Err("duplicate worlds in coverage witness".into());
            }
            admitted.push(&branch.bindings);
        }
        Ok(admitted)
    }

    fn admit_assignment(
        &self,
        bindings: &BTreeMap<String, Value>,
        slot_names: &BTreeSet<&str>,
    ) -> Result<BTreeMap<String, Value>, String> {
        for key in bindings.keys() {
            if !slot_names.contains(key.as_str()) {
                return Err(format!(
                    "coverage witness has undeclared completion key `{key}`"
                ));
            }
        }
        let mut canonical = BTreeMap::new();
        for slot in &self.slots {
            let Some(raw) = bindings.get(&slot.name) else {
                return Err(format!(
                    "coverage witness is missing declared slot `{}`",
                    slot.name
                ));
            };
            let value = canonical_slot_value(raw);
            if !slot
                .values
                .iter()
                .any(|admitted| canonical_slot_value(admitted) == value)
            {
                return Err(format!(
                    "coverage witness value {value:?} is outside domain of `{}`",
                    slot.name
                ));
            }
            canonical.insert(slot.name.clone(), value);
        }
        Ok(canonical)
    }

    /// Restricted overlay: admitted slots only. Never mutates fixed facts
    /// or overwrites recorded interpretations/choices.
    pub fn overlay(
        &self,
        base: &CaseRecord,
        bindings: &BTreeMap<String, Value>,
        ctx: &RunContext,
    ) -> Result<CaseRecord, String> {
        let slot_names: BTreeSet<&str> = self.slots.iter().map(|slot| slot.name.as_str()).collect();
        let assignment = self.admit_assignment(bindings, &slot_names)?;
        let mut case = base.clone();
        for (key, value) in &assignment {
            let text = binding_label(value);
            if let Some(family) = key.strip_prefix(INTERPRETATION_NS) {
                if !case.interpretations.contains_key(family) {
                    case.interpretations.insert(family.to_owned(), text);
                }
            } else if let Some(protocol) = key.strip_prefix(CHOICE_NS) {
                if !case.decisions.contains_key(protocol) {
                    case.decisions.insert(protocol.to_owned(), text);
                }
            } else if let Some(schema) = key.strip_prefix(EVIDENCE_NS) {
                apply_evidence_response(&mut case, base, schema, &text, ctx);
            } else {
                return Err(format!("undeclared completion slot `{key}`"));
            }
        }
        Ok(case)
    }
}

fn recorded_or_declared(recorded: Option<&String>, declared: &[String]) -> Vec<Value> {
    if declared.is_empty() {
        return Vec::new();
    }
    if let Some(value) = recorded {
        if declared.iter().any(|item| item == value) {
            return vec![Value::String(value.clone())];
        }
        return Vec::new();
    }
    unique_domain_values(declared.iter().cloned().map(Value::String).collect())
}

fn unique_domain_values(values: Vec<Value>) -> Vec<Value> {
    let mut unique = Vec::new();
    for value in values {
        let canonical = canonical_slot_value(&value);
        if !unique.iter().any(|prior| prior == &canonical) {
            unique.push(canonical);
        }
    }
    unique
}

fn canonical_slot_value(value: &Value) -> Value {
    match value {
        Value::String(text) | Value::Entity(text) => Value::String(text.clone()),
        other => other.clone(),
    }
}

fn binding_label(value: &Value) -> String {
    match value {
        Value::String(s) | Value::Entity(s) => s.clone(),
        other => other.display_label(),
    }
}

fn apply_evidence_response(
    branched: &mut CaseRecord,
    original: &CaseRecord,
    schema: &str,
    response: &str,
    ctx: &RunContext,
) {
    if is_absent_response(response) {
        return;
    }
    if original.evidence.iter().any(|item| item.schema == schema) {
        return;
    }
    let effect = original
        .admissible_completions
        .evidence
        .get(schema)
        .and_then(|domain| domain.effect_on_valid_time.as_deref());
    branched.evidence.push(EvidenceItem {
        schema: schema.to_owned(),
        value: Value::String(response.to_owned()),
        observed_at: evidence_observed_at(response, effect, ctx),
    });
}

fn is_absent_response(response: &str) -> bool {
    matches!(response, "absent" | "none" | "missing" | "not_present" | "")
}

fn evidence_observed_at(response: &str, effect: Option<&str>, ctx: &RunContext) -> Instant {
    let prospective = response.contains("prospective")
        || effect.is_some_and(|text| text.contains("after") || text.contains("unchanged"));
    if prospective {
        Instant::parse("2099-01-01T00:00:00Z").unwrap_or(ctx.record_time)
    } else {
        ctx.record_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::{BranchClaim, CompletionDomain};

    fn witness_from(maps: Vec<BTreeMap<String, Value>>) -> CoverageWitness {
        let n = maps.len();
        CoverageWitness {
            examined: n,
            total: n,
            incomplete: false,
            answer: Value::Bool(true),
            branches: maps
                .into_iter()
                .map(|bindings| BranchClaim {
                    bindings,
                    answer: Value::Bool(true),
                })
                .collect(),
        }
    }

    fn i_assignment(label: &str) -> BTreeMap<String, Value> {
        BTreeMap::from([("i:I".into(), Value::String(label.into()))])
    }

    #[test]
    fn zero_variables_is_one_empty_assignment() {
        let model = ValidatedCompletionModel::from_case(&CaseRecord::default()).unwrap();
        assert_eq!(model.product_size().unwrap(), 1);
        model
            .admit_witness(&witness_from(vec![BTreeMap::new()]))
            .expect("empty assignment");
        let err = model
            .admit_witness(&witness_from(vec![BTreeMap::from([(
                "noise".into(),
                Value::Int(0),
            )])]))
            .expect_err("undeclared key");
        assert!(err.contains("undeclared"), "{err}");
    }

    #[test]
    fn empty_declared_domain_admits_no_assignments() {
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("I".into(), Vec::new());
        let model = ValidatedCompletionModel::from_case(&case).unwrap();
        assert_eq!(model.product_size().unwrap(), 0);
        let err = model
            .admit_witness(&witness_from(vec![i_assignment("A")]))
            .expect_err("empty domain");
        assert!(
            err.contains("empty") || err.contains("no assignments"),
            "{err}"
        );
    }

    #[test]
    fn noise_maps_are_not_declared_worlds() {
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("I".into(), vec!["A".into(), "B".into()]);
        let model = ValidatedCompletionModel::from_case(&case).unwrap();
        let err = model
            .admit_witness(&witness_from(vec![
                BTreeMap::from([("noise".into(), Value::Int(0))]),
                BTreeMap::from([("noise".into(), Value::Int(1))]),
            ]))
            .expect_err("noise");
        assert!(
            err.contains("undeclared") || err.contains("missing"),
            "{err}"
        );
    }

    #[test]
    fn extra_fixed_key_is_rejected() {
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("I".into(), vec!["A".into(), "B".into()]);
        let model = ValidatedCompletionModel::from_case(&case).unwrap();
        let err = model
            .admit_witness(&witness_from(vec![
                BTreeMap::from([
                    ("i:I".into(), Value::String("A".into())),
                    ("fixed".into(), Value::Bool(true)),
                ]),
                BTreeMap::from([
                    ("i:I".into(), Value::String("B".into())),
                    ("fixed".into(), Value::Bool(true)),
                ]),
            ]))
            .expect_err("fixed");
        assert!(err.contains("undeclared"), "{err}");
    }

    #[test]
    fn out_of_domain_value_is_rejected() {
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("I".into(), vec!["A".into(), "B".into()]);
        let model = ValidatedCompletionModel::from_case(&case).unwrap();
        let err = model
            .admit_witness(&witness_from(vec![i_assignment("A"), i_assignment("Z")]))
            .expect_err("out of domain");
        assert!(err.contains("outside domain"), "{err}");
    }

    #[test]
    fn recorded_selection_narrows_the_product() {
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("I".into(), vec!["A".into(), "B".into()]);
        case.interpretations.insert("I".into(), "A".into());
        let model = ValidatedCompletionModel::from_case(&case).unwrap();
        assert_eq!(model.product_size().unwrap(), 1);
        model
            .admit_witness(&witness_from(vec![i_assignment("A")]))
            .expect("recorded A");
        let err = model
            .admit_witness(&witness_from(vec![i_assignment("A"), i_assignment("B")]))
            .expect_err("B is not admitted once A is recorded");
        assert!(err.contains("product") || err.contains("domain"), "{err}");
    }

    #[test]
    fn duplicate_declared_values_are_unique() {
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("I".into(), vec!["A".into(), "A".into()]);
        let model = ValidatedCompletionModel::from_case(&case).unwrap();
        assert_eq!(model.product_size().unwrap(), 1);
        model
            .admit_witness(&witness_from(vec![i_assignment("A")]))
            .expect("unique A");
    }

    #[test]
    fn overlay_does_not_mutate_facts_or_recorded_selections() {
        let mut case = CaseRecord::default();
        case.facts.insert("fixed".into(), Value::Bool(false));
        case.interpretations.insert("I".into(), "A".into());
        case.admissible_completions
            .interpretations
            .insert("I".into(), vec!["A".into()]);
        let model = ValidatedCompletionModel::from_case(&case).unwrap();
        let ctx = RunContext::new(
            Instant::parse("2033-01-01T00:00:00Z").unwrap(),
            Instant::parse("2033-01-01T00:00:00Z").unwrap(),
        );
        let overlaid = model
            .overlay(&case, &i_assignment("A"), &ctx)
            .expect("overlay");
        assert_eq!(overlaid.facts.get("fixed"), Some(&Value::Bool(false)));
        assert_eq!(overlaid.interpretations.get("I"), Some(&"A".to_string()));
        assert!(!overlaid.facts.contains_key("i:I"));
    }

    #[test]
    fn overlay_does_not_insert_undeclared_fact_keys() {
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("I".into(), vec!["A".into(), "B".into()]);
        let model = ValidatedCompletionModel::from_case(&case).unwrap();
        let ctx = RunContext::new(
            Instant::parse("2033-01-01T00:00:00Z").unwrap(),
            Instant::parse("2033-01-01T00:00:00Z").unwrap(),
        );
        let mut bindings = i_assignment("A");
        bindings.insert("fixed".into(), Value::Bool(true));
        let err = model.overlay(&case, &bindings, &ctx).expect_err("fixed");
        assert!(err.contains("undeclared"), "{err}");
        assert!(!case.facts.contains_key("fixed"));
    }

    #[test]
    fn evidence_slot_is_a_declared_domain() {
        let mut case = CaseRecord::default();
        case.admissible_completions.evidence.insert(
            "Filing".into(),
            CompletionDomain {
                responses: vec!["present".into(), "absent".into()],
                effect_on_valid_time: None,
            },
        );
        let model = ValidatedCompletionModel::from_case(&case).unwrap();
        assert_eq!(model.product_size().unwrap(), 2);
        model
            .admit_witness(&witness_from(vec![
                BTreeMap::from([("e:Filing".into(), Value::String("present".into()))]),
                BTreeMap::from([("e:Filing".into(), Value::String("absent".into()))]),
            ]))
            .expect("evidence product");
    }
}
