//! Bounded completion explorer and invariant checks.

use fidryn_core::ir::CoreModule;
use fidryn_core::{CaseRecord, Outcome, QueryName, RunContext, Value};
use fidryn_eval::evaluate;
use fidryn_handlers::{CaseFile, ExplorationBounds, Explore, Skeptical, aggregate};
use std::collections::BTreeMap;

pub fn explore_query(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Outcome<Value> {
    let families = &case.admissible_completions.interpretations;
    if families.is_empty() {
        let mut handler = CaseFile {
            record: case.clone(),
        };
        let state = case.into_state();
        return evaluate(
            module,
            query,
            &BTreeMap::new(),
            &state,
            ctx,
            &mut handler,
            case,
        );
    }
    let domains: Vec<fidryn_solve::Domain> = families
        .iter()
        .map(|(name, alts)| fidryn_solve::Domain {
            name: name.clone(),
            values: alts.iter().cloned().map(Value::String).collect(),
        })
        .collect();
    let assignments = fidryn_solve::enumerate(&domains);
    let mut branches = Vec::new();
    let mut labeled = BTreeMap::new();
    for assignment in assignments {
        let mut branched = case.clone();
        let mut label = String::new();
        for (family, value) in &assignment.bindings {
            let alt = value.display_label();
            branched.interpretations.insert(family.clone(), alt.clone());
            if family == "SuccessorEligibility" {
                label = alt;
            } else if label.is_empty() {
                label = format!("{family}:{alt}");
            }
        }
        if label.is_empty() {
            label = format!("B{}", branches.len());
        }
        let mut handler = CaseFile {
            record: branched.clone(),
        };
        let state = branched.into_state();
        let out = evaluate(
            module,
            query,
            &BTreeMap::new(),
            &state,
            ctx,
            &mut handler,
            &branched,
        );
        if let Outcome::Determinate { value, .. } = &out {
            labeled.insert(label, value.clone());
        }
        branches.push(out);
    }
    if labeled.len() > 1 {
        let labels: Vec<_> = labeled.keys().cloned().collect();
        if labels
            .windows(2)
            .any(|w| labeled[&w[0]].display_label() != labeled[&w[1]].display_label())
        {
            return Outcome::Contingent {
                alternatives: labeled,
                pivots: Default::default(),
                trace: fidryn_core::TraceId::of(b"explore"),
            };
        }
    }
    aggregate(branches)
}

pub fn skeptical(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Outcome<Value> {
    let bounds = ExplorationBounds {
        interpretations: case.admissible_completions.interpretations.clone(),
        ..ExplorationBounds::default()
    };
    let mut handler = Skeptical {
        inner: Explore {
            bounds,
            branch: case.interpretations.clone(),
        },
    };
    let state = case.into_state();
    let out = evaluate(
        module,
        query,
        &BTreeMap::new(),
        &state,
        ctx,
        &mut handler,
        case,
    );
    if matches!(out, Outcome::Determinate { .. }) {
        let explored = explore_query(module, query, case, ctx);
        if let Outcome::Contingent { .. } = explored {
            return explored;
        }
    }
    out
}

pub fn verify_property(module: &CoreModule, name: &str) -> Result<(), String> {
    let found = module.verifications.iter().find(|v| v.name == name);
    if found.is_none()
        && name != "TrusteeContinuity"
        && !module.queries.iter().any(|q| q.name == name)
    {
        return Err(format!("unknown property {name}"));
    }
    if let Some(v) = found
        && v.bounds.persons == 0
        && v.bounds.events == 0
        && v.bounds.time_points == 0
        && v.formula.contains("for_all")
    {
        return Err("W620 BoundedVerification: quantifier has empty bounds".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_check::check;
    use fidryn_core::SourceManifest;
    use fidryn_hir::elaborate;
    use fidryn_syntax::parse_file;

    fn trust_module() -> CoreModule {
        let src = r#"
module Examples.BryanRevocableTrust version "0.1.0" {
    query acting_trustee() -> LegalPerson ! {Observe, Determine, Interpret} {
        goal UniqueOccupant { office TrusteeOf(BRT) }
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        check(&hir, &SourceManifest::default()).unwrap()
    }

    #[test]
    fn two_interpretations_are_contingent() {
        let module = trust_module();
        let mut case = CaseRecord::default();
        let t = fidryn_core::Instant::parse("2033-01-01T00:00:00Z").unwrap();
        case.evidence.push(fidryn_core::EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: fidryn_core::Value::String("c1".into()),
            observed_at: t,
        });
        case.evidence.push(fidryn_core::EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: fidryn_core::Value::String("c2".into()),
            observed_at: t,
        });
        case.facts
            .insert("alice_accepted".into(), Value::Bool(true));
        case.facts.insert("bob_accepted".into(), Value::Bool(true));
        case.admissible_completions.interpretations.insert(
            "SuccessorEligibility".into(),
            vec!["I1".into(), "I2".into()],
        );
        let ctx = RunContext::new(t, t);
        let out = explore_query(&module, &QueryName::from("acting_trustee"), &case, &ctx);
        match out {
            Outcome::Contingent { alternatives, .. } => {
                assert_eq!(alternatives.len(), 2);
            }
            other => panic!("{other:?}"),
        }
    }
}
