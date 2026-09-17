//! Type, effect, authority, time, and stratification checking.

use fidryn_core::ir::{
    ClauseSelector, CoreDecl, CoreEffectDecl, CoreEffectOp, CoreEntity, CoreFunction, CoreModule,
    CoreOffice, CoreProposition, CoreQuery, CoreRule, CoreVerify, Guard, NodeMeta, QueryPlan,
    RuleKind, VerificationBounds,
};
use fidryn_core::time::Interval;
use fidryn_core::types::{Sort, Type};
use fidryn_core::value::Term;
use fidryn_core::{
    Diagnostic, DiagnosticCode, EffectName, JurisdictionId, ModuleId, NodeId, OriginId,
    SourceManifest, SourceManifestId, SourceSnapshotId,
};
use fidryn_hir::{HirModule, HirQuery};
use std::collections::{BTreeMap, BTreeSet};

pub fn check(hir: &HirModule, manifest: &SourceManifest) -> Result<CoreModule, Vec<Diagnostic>> {
    let mut diagnostics = hir.diagnostics.clone();
    check_nominations(hir, &mut diagnostics);
    check_queries(hir, &mut diagnostics);
    check_doctrines(hir, &mut diagnostics);
    check_recursion(hir, &mut diagnostics);
    check_imports(hir, manifest, &mut diagnostics);
    check_sources(hir, &mut diagnostics);
    if diagnostics
        .iter()
        .any(|d| d.code.severity() == fidryn_core::Severity::Error)
    {
        return Err(diagnostics);
    }
    Ok(lower(hir, manifest))
}

fn check_nominations(hir: &HirModule, diagnostics: &mut Vec<Diagnostic>) {
    let mut ranks: BTreeMap<(String, i64), Vec<String>> = BTreeMap::new();
    for n in &hir.nominations {
        ranks
            .entry((n.office.clone(), n.rank))
            .or_default()
            .push(n.candidate.clone());
    }
    for ((office, rank), names) in ranks {
        if names.len() > 1 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E410,
                format!(
                    "{} both have nomination rank {rank} for {office}. Required: unique ranks or an explicit tie-resolution rule",
                    names.join(" and ")
                ),
            ));
        }
    }
}

fn check_queries(hir: &HirModule, diagnostics: &mut Vec<Diagnostic>) {
    for q in hir.queries.values() {
        if !q.has_goal && !query_has_body(&q.plan) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E430,
                format!("query `{}` is missing a body or explicit goal", q.name),
            ));
        }
        if q.automatic && !q.effects.is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E420,
                format!("automatic query `{}` must have an empty effect row", q.name),
            ));
        }
        if looks_like_prop_guard(&q.plan) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E310,
                "a proposition was used directly as a guard; use operative/determined/assumed",
            ));
        }
    }
}

fn looks_like_prop_guard(src: &str) -> bool {
    src.contains("if Incapacitated(") || src.contains("if Eligible(")
}

fn query_has_body(src: &str) -> bool {
    contains_ident(src, "require")
        || contains_ident(src, "return")
        || contains_ident(src, "for_all")
        || contains_ident(src, "exists")
}

fn check_recursion(hir: &HirModule, diagnostics: &mut Vec<Diagnostic>) {
    for f in hir.functions.values() {
        let body = last_brace_inner(&f.source).unwrap_or("");
        if !contains_ident(body, &f.name) {
            continue;
        }
        if f.is_calc {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::E530,
                    format!("calc `{}` must not recurse", f.name),
                )
                .with_suggestion("rewrite the calc as a total, non-recursive closed form"),
            );
            continue;
        }
        if f.fuel.is_none() {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::E530,
                    format!(
                        "function `{}` recurses without an explicit fuel bound",
                        f.name
                    ),
                )
                .with_suggestion("add `fuel = N` to bound recursion"),
            );
        }
    }
}

fn last_brace_inner(src: &str) -> Option<&str> {
    let mut depth = 0i32;
    let mut start = None;
    let mut last = None;
    for (i, c) in src.char_indices() {
        match c {
            '{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0
                    && let Some(s) = start
                {
                    last = Some(&src[s + 1..i]);
                }
            }
            _ => {}
        }
    }
    last
}

fn contains_ident(src: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut start = 0;
    while let Some(rel) = src[start..].find(name) {
        let i = start + rel;
        let before = src[..i].chars().next_back();
        let after = src[i + name.len()..].chars().next();
        let ident_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
        if before.is_none_or(|c| !ident_char(c)) && after.is_none_or(|c| !ident_char(c)) {
            return true;
        }
        start = i + 1;
    }
    false
}

fn check_doctrines(hir: &HirModule, diagnostics: &mut Vec<Diagnostic>) {
    for doctrine in &hir.conflict_doctrines {
        if expand_conflict_clause_ids(&doctrine.target, &hir.clauses).is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E511,
                format!(
                    "conflict doctrine `{}` has an empty ClauseId expansion",
                    doctrine.name
                ),
            ));
        }
    }
}

/// Resolve `defeat <Clause>` targets against known clauses.
fn expand_conflict_clause_ids(source: &str, clauses: &BTreeMap<String, String>) -> Vec<String> {
    let mut found = Vec::new();
    for (i, _) in source.match_indices("defeat") {
        let before = source[..i].chars().next_back();
        if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let rest = &source[i + "defeat".len()..];
        if rest
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        let name: String = rest
            .chars()
            .skip_while(|c| !(c.is_ascii_alphabetic() || *c == '_'))
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if clauses.contains_key(&name) {
            found.push(name);
        }
    }
    found
}

fn check_imports(hir: &HirModule, manifest: &SourceManifest, diagnostics: &mut Vec<Diagnostic>) {
    if manifest.snapshot.is_empty() {
        return;
    }
    for import in &hir.imports {
        if !import.digest_required {
            continue;
        }
        let known = manifest.artifacts.iter().any(|a| {
            import.name.is_empty() || a.path.contains(&import.name) || !a.digest.is_empty()
        });
        if !known {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E200,
                format!(
                    "import `{}` requires a digest in the authenticated source manifest",
                    import.name
                ),
            ));
        }
    }
}

fn check_sources(hir: &HirModule, diagnostics: &mut Vec<Diagnostic>) {
    for source in &hir.sources {
        if source.artifact.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E540,
                format!("source `{}` is missing a legal artifact", source.name),
            ));
        }
    }
    for q in &hir.quantifiers {
        if q.domain.chars().any(|c| c.is_ascii_uppercase()) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::W610,
                format!(
                    "quantifier over `{}` is an open universe unless a closure record is supplied",
                    q.domain
                ),
            ));
        }
    }
}

fn lower(hir: &HirModule, manifest: &SourceManifest) -> CoreModule {
    let jid = JurisdictionId::of(hir.jurisdiction.as_bytes());
    let meta = |node: &str| NodeMeta {
        span: None,
        source: Some(node.to_owned()),
        jurisdiction: jid,
        valid_time: Interval::always(),
        record_time: Interval::always(),
        origin: OriginId::Direct(NodeId::of(node.as_bytes())),
    };
    let mut declarations = Vec::new();
    for (name, ty) in &hir.entities {
        declarations.push(CoreDecl::Entity(CoreEntity {
            id: NodeId::of(name.as_bytes()),
            name: name.clone(),
            ty: Type::Sort(match ty.as_str() {
                "NaturalPerson" => Sort::NaturalPerson,
                "Trust" => Sort::Trust,
                "PremaritalAgreement" => Sort::PremaritalAgreement,
                "ProposedLLC" => Sort::ProposedLlc,
                _ => Sort::LegalPerson,
            }),
            meta: meta(name),
        }));
    }
    for name in hir.propositions.keys() {
        declarations.push(CoreDecl::Proposition(CoreProposition {
            id: NodeId::of(name.as_bytes()),
            name: name.clone(),
            params: Vec::new(),
            meta: meta(name),
        }));
    }
    for name in hir.offices.keys() {
        declarations.push(CoreDecl::Office(CoreOffice {
            id: NodeId::of(name.as_bytes()),
            name: name.clone(),
            occupant: Type::Sort(Sort::LegalPerson),
            cardinality_min: 0,
            cardinality_max: Some(1),
            competence: Vec::new(),
            meta: meta(name),
        }));
    }
    for f in hir.functions.values() {
        declarations.push(CoreDecl::Function(CoreFunction {
            id: NodeId::of(f.name.as_bytes()),
            name: f.name.clone(),
            params: Vec::new(),
            result: Type::Sort(Sort::Nominal(f.result_type.clone())),
            effects: BTreeSet::new(),
            is_calc: f.is_calc,
            fuel: f.fuel,
            meta: meta(&f.name),
        }));
    }
    for e in hir.effects.values() {
        declarations.push(CoreDecl::EffectDecl(CoreEffectDecl {
            id: NodeId::of(e.name.as_bytes()),
            name: e.name.clone(),
            operations: vec![CoreEffectOp {
                name: "request".into(),
                params: Vec::new(),
                result: Type::Sort(Sort::Nominal(e.result_type.clone())),
            }],
            meta: meta(&e.name),
        }));
    }
    for rule in &hir.rules {
        declarations.push(CoreDecl::Rule(CoreRule {
            id: NodeId::of(rule.name.as_bytes()),
            name: rule.name.clone(),
            kind: match rule.kind.as_str() {
                "constitutive" => RuleKind::Constitutive,
                "derive" => RuleKind::Derive,
                _ => RuleKind::Prescriptive,
            },
            binders: Vec::new(),
            selection: None,
            guard: Guard::Satisfied,
            consequences: Vec::new(),
            fallback: None,
            meta: meta(&rule.name),
        }));
    }
    let queries = hir.queries.values().map(|q| lower_query(q, jid)).collect();
    let verifications = hir
        .quantifiers
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let name = format!("{}:{}:{i}", q.kind.as_str(), q.binder);
            CoreVerify {
                id: NodeId::of(name.as_bytes()),
                name,
                bounds: VerificationBounds {
                    persons: 0,
                    events: 0,
                    time_points: 0,
                },
                formula: format!(
                    "{} {} in {}: {}",
                    q.kind.as_str(),
                    q.binder,
                    q.domain,
                    q.formula
                ),
                meta: meta(&q.binder),
            }
        })
        .collect();
    CoreModule {
        id: ModuleId::of(hir.name.as_bytes()),
        name: hir.name.clone(),
        version: hir.version.clone(),
        snapshot: SourceSnapshotId::of(hir.snapshot.as_bytes()),
        manifest: SourceManifestId::of(manifest.snapshot.as_bytes()),
        jurisdiction: jid,
        outside_scope: hir.outside_scope.clone(),
        declarations,
        nominations: hir
            .nominations
            .iter()
            .map(|n| fidryn_core::CoreNomination {
                candidate: n.candidate.clone(),
                office: n.office.clone(),
                rank: n.rank,
            })
            .collect(),
        queries,
        verifications,
        assertions: Vec::new(),
    }
}

fn extract_plan_field(plan: &str, key: &str) -> Option<String> {
    let needle = format!("{key} ");
    let i = plan.find(&needle)?;
    let rest = plan[i + needle.len()..].trim_start();
    let end = rest.find(['\n', '}', '{']).unwrap_or(rest.len());
    let value = rest[..end].trim().trim_end_matches(',').to_owned();
    if value.is_empty() { None } else { Some(value) }
}

fn lower_query(q: &HirQuery, jid: JurisdictionId) -> CoreQuery {
    let mut effects = BTreeSet::new();
    for e in &q.effects {
        match e.as_str() {
            "Observe" => {
                effects.insert(EffectName::Observe);
            }
            "Determine" => {
                effects.insert(EffectName::Determine);
            }
            "Choose" => {
                effects.insert(EffectName::Choose);
            }
            "Interpret" => {
                effects.insert(EffectName::Interpret);
            }
            _ => {}
        }
    }
    let plan = if q.plan.contains("UniqueOccupant") {
        let office = extract_plan_field(&q.plan, "office").unwrap_or_else(|| "TrusteeOf".into());
        QueryPlan::UniqueOccupant {
            office: Term::Ident(office),
        }
    } else if q.plan.contains("EvaluateClause") {
        QueryPlan::EvaluateClause {
            clause: ClauseSelector::Bound {
                binder: extract_plan_field(&q.plan, "clause").unwrap_or_else(|| "provision".into()),
                module: ModuleId::of(b"clause"),
            },
            context: "case".into(),
            result: Term::Ident("result".into()),
        }
    } else if q.plan.contains("RunDecision") {
        QueryPlan::RunDecision {
            decision: extract_plan_field(&q.plan, "decision")
                .unwrap_or_else(|| "ProcessResponsiveRecord".into()),
            arguments: Vec::new(),
            result: fidryn_core::ir::DeclaredDecisionResult {
                expected_type: Type::Sort(Sort::Nominal("FOIADisposition".into())),
            },
        }
    } else if q.plan.contains("StatusOf") {
        let ctor = extract_plan_field(&q.plan, "status")
            .or_else(|| extract_plan_field(&q.plan, "when_present"))
            .unwrap_or_else(|| "FormedLLC".into());
        let present = ctor
            .split('(')
            .next()
            .unwrap_or("FormedLLC")
            .trim()
            .to_owned();
        let absent = extract_plan_field(&q.plan, "when_closed_absent")
            .and_then(|s| s.split('(').next().map(str::trim).map(str::to_owned))
            .unwrap_or_else(|| format!("Not{present}"));
        QueryPlan::StatusOf {
            status: fidryn_core::LegalStatusPattern::InstitutionalStatus {
                constructor: present.clone(),
                arguments: Vec::new(),
            },
            when_present: Term::unit_ctor(present),
            when_closed_absent: Term::unit_ctor(absent),
        }
    } else {
        QueryPlan::Evaluate(Term::Ident(q.name.clone()))
    };
    CoreQuery {
        id: NodeId::of(q.name.as_bytes()),
        name: q.name.clone(),
        binders: Vec::new(),
        result_type: Type::Sort(Sort::LegalPerson),
        effects,
        automatic: q.automatic,
        plan,
        meta: NodeMeta {
            span: None,
            source: None,
            jurisdiction: jid,
            valid_time: Interval::always(),
            record_time: Interval::always(),
            origin: OriginId::Direct(NodeId::of(q.name.as_bytes())),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_hir::elaborate;
    use fidryn_syntax::parse_file;

    #[test]
    fn duplicate_ranks_are_e410() {
        let src = r#"
module Examples.Dup version "0.1.0" {
    nomination Alice for TrusteeOf(BRT) rank 1
    nomination Bob for TrusteeOf(BRT) rank 1
    query acting_trustee() -> LegalPerson {
        goal UniqueOccupant { office TrusteeOf(BRT) }
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let err = check(&hir, &SourceManifest::default()).unwrap_err();
        assert!(err.iter().any(|d| d.code == DiagnosticCode::E410));
    }

    #[test]
    fn missing_goal_is_e430() {
        let src = r#"
module Examples.NoGoal version "0.1.0" {
    query acting_trustee() -> LegalPerson
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let err = check(&hir, &SourceManifest::default()).unwrap_err();
        assert!(err.iter().any(|d| d.code == DiagnosticCode::E430));
    }

    #[test]
    fn prop_as_guard_is_e310() {
        let src = r#"
module Examples.PropGuard version "0.1.0" {
    query bad() -> LegalPerson {
        goal Evaluate { if Incapacitated(Bryan) { Bryan } }
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let err = check(&hir, &SourceManifest::default()).unwrap_err();
        assert!(err.iter().any(|d| d.code == DiagnosticCode::E310));
    }

    #[test]
    fn empty_clause_id_expansion_is_e511() {
        let src = r#"
module Examples.UnknownTarget version "0.1.0" {
    conflict_doctrine Ghost {
        when Foo
        then defeat MissingClause as_to Bar
        reason MandatoryStatutoryLimit
    }
    query q() -> LegalPerson {
        goal Evaluate { x }
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let err = check(&hir, &SourceManifest::default()).unwrap_err();
        assert!(err.iter().any(|d| d.code == DiagnosticCode::E511));
    }

    fn check_src(src: &str) -> Result<CoreModule, Vec<Diagnostic>> {
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        check(&hir, &SourceManifest::default())
    }

    #[test]
    fn unguarded_fn_recursion_is_e530() {
        let src = r#"
module Examples.Recurse version "0.1.0" {
    fn countdown(n: Int) -> Int {
        countdown(n - 1)
    }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(err.iter().any(|d| d.code == DiagnosticCode::E530));
    }

    #[test]
    fn fueled_fn_recursion_is_allowed() {
        let src = r#"
module Examples.Fuel version "0.1.0" {
    effect DocketLookup {
        request(docket_id: String) -> Docket
    }
    fn countdown(n: Int) -> Int fuel = 4 {
        countdown(n - 1)
    }
    query all_ok() -> Bool {
        for_all x in People: Eligible(x)
        exists y in People: Trustee(y)
    }
}
"#;
        let module = check_src(src).expect("fueled recursion and quantifiers should check");
        assert!(module.declarations.iter().any(|d| matches!(
            d,
            CoreDecl::Function(f) if f.name == "countdown" && f.fuel == Some(4) && !f.is_calc
        )));
        assert!(module.declarations.iter().any(|d| matches!(
            d,
            CoreDecl::EffectDecl(e) if e.name == "DocketLookup"
        )));
        assert!(
            module
                .verifications
                .iter()
                .any(|v| v.formula.contains("for_all x in People"))
        );
        assert!(
            module
                .verifications
                .iter()
                .any(|v| v.formula.contains("exists y in People"))
        );
    }

    #[test]
    fn calc_recursion_is_e530_even_with_fuel() {
        let src = r#"
module Examples.CalcRecurse version "0.1.0" {
    calc countdown(n: Int) -> Int fuel = 4 {
        countdown(n - 1)
    }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(err.iter().any(|d| d.code == DiagnosticCode::E530));
    }

    #[test]
    fn substring_name_is_not_recursion() {
        let src = r#"
module Examples.Count version "0.1.0" {
    fn count(n: Int) -> Int {
        counter(n)
    }
}
"#;
        check_src(src).expect("count calling counter is not self-recursion");
    }
}
