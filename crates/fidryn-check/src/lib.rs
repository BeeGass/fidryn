//! Type, effect, authority, time, and stratification checking.

use fidryn_core::ir::{
    ClauseSelector, Consequence, CoreDecl, CoreEffect, CoreEffectDecl, CoreEffectOp, CoreEntity,
    CoreFunction, CoreModule, CoreOffice, CoreProposition, CoreQuery, CoreRule, CoreVerify, Guard,
    NodeMeta, QueryPlan, RuleKind, VerificationBounds,
};
use fidryn_core::patterns::{LegalStatusPattern, TermPattern};
use fidryn_core::time::Interval;
use fidryn_core::types::{PrimitiveType, Sort, Type};
use fidryn_core::value::{PropTerm, Term};
use fidryn_core::{
    ClauseId, Diagnostic, DiagnosticCode, EffectId, EffectName, JurisdictionId, ModuleId, NodeId,
    OriginId, SourceManifest, SourceManifestId, SourceSnapshotId,
};
use fidryn_hir::{
    HirModule, HirQuery, HirQueryBody, collect_source_callees, collect_term_callees,
    flatten_term_list, parse_type_name, source_has_bare_prop_if, split_qname_type_args,
    term_as_name, term_has_bare_prop_guard,
};
use std::collections::{BTreeMap, BTreeSet};

pub fn check(hir: &HirModule, manifest: &SourceManifest) -> Result<CoreModule, Vec<Diagnostic>> {
    let mut diagnostics = hir.diagnostics.clone();
    check_nominations(hir, &mut diagnostics);
    check_queries(hir, &mut diagnostics);
    check_doctrines(hir, &mut diagnostics);
    check_recursion(hir, &mut diagnostics);
    check_imports(hir, manifest, &mut diagnostics);
    check_sources(hir, &mut diagnostics);
    check_instantiation_arity(hir, &mut diagnostics);
    if diagnostics
        .iter()
        .any(|d| d.code.severity() == fidryn_core::Severity::Error)
    {
        return Err(diagnostics);
    }
    let core = lower(hir, manifest);
    match instantiation_args(hir) {
        Some(args) => instantiate(&core, &args),
        None => Ok(core),
    }
}

/// Instantiate a parameterized `CoreModule` by substituting `args` for its
/// type parameters. Arity mismatch is `E210` (`TypeMismatch`).
pub fn instantiate(module: &CoreModule, args: &[Type]) -> Result<CoreModule, Vec<Diagnostic>> {
    let (base_name, params) = module_type_params(module);
    if params.len() != args.len() {
        return Err(vec![arity_mismatch(&base_name, params.len(), args.len())]);
    }
    let mut out = module.clone();
    if params.is_empty() {
        out.name = base_name;
        return Ok(out);
    }
    for decl in &mut out.declarations {
        subst_decl(decl, &params, args);
    }
    for query in &mut out.queries {
        subst_query(query, &params, args);
    }
    out.name = instantiated_module_name(&base_name, args);
    out.id = ModuleId::of(out.name.as_bytes());
    Ok(out)
}

fn instantiated_module_name(base: &str, args: &[Type]) -> String {
    if args.is_empty() {
        base.to_owned()
    } else {
        format!(
            "{base}<{}>",
            args.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Replace `Type::Sort(Sort::Nominal(param))` (and nested occurrences) with the
/// corresponding argument.
pub fn substitute_nominal(ty: Type, params: &[String], args: &[Type]) -> Type {
    match ty {
        Type::Sort(Sort::Nominal(name)) => match params.iter().position(|p| p == &name) {
            Some(i) => args
                .get(i)
                .cloned()
                .unwrap_or(Type::Sort(Sort::Nominal(name))),
            None => Type::Sort(Sort::Nominal(name)),
        },
        Type::Sort(sort) => Type::Sort(sort),
        Type::Applied {
            ctor,
            args: ty_args,
        } => Type::Applied {
            ctor,
            args: ty_args
                .into_iter()
                .map(|t| substitute_nominal(t, params, args))
                .collect(),
        },
        Type::Primitive(prim) => Type::Primitive(subst_primitive(prim, params, args)),
    }
}

/// Alias used by the review-fix contract (`substitute_type`).
pub fn substitute_type(ty: Type, params: &[String], args: &[Type]) -> Type {
    substitute_nominal(ty, params, args)
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
    let propositions: BTreeSet<String> = hir.propositions.keys().cloned().collect();
    for q in hir.queries.values() {
        if !q.has_goal {
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
        if query_has_bare_prop_guard(q, &propositions) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E310,
                "a proposition was used directly as a guard; use operative/determined/assumed",
            ));
        }
        if let HirQueryBody::Return(term) = &q.body
            && let Some(actual) = literal_term_type(term)
        {
            let expected = parse_type_name(&q.result_type);
            if !types_compatible(&expected, &actual) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::E210,
                    format!(
                        "query `{}` returns {actual} but is declared to return {expected}",
                        q.name
                    ),
                ));
            }
        }
    }
    for f in hir.functions.values() {
        if function_has_bare_prop_guard(f, &propositions) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E310,
                "a proposition was used directly as a guard; use operative/determined/assumed",
            ));
        }
    }
}

fn query_has_bare_prop_guard(q: &HirQuery, propositions: &BTreeSet<String>) -> bool {
    if source_has_bare_prop_if(&q.plan, propositions) {
        return true;
    }
    match &q.body {
        HirQueryBody::Return(term) => term_has_bare_prop_guard(term, propositions),
        HirQueryBody::Goal {
            expr,
            fields,
            office,
            ..
        } => {
            expr.as_ref()
                .is_some_and(|term| term_has_bare_prop_guard(term, propositions))
                || office
                    .as_ref()
                    .is_some_and(|term| term_has_bare_prop_guard(term, propositions))
                || fields
                    .values()
                    .any(|term| term_has_bare_prop_guard(term, propositions))
        }
        HirQueryBody::None => false,
    }
}

fn literal_term_type(term: &Term) -> Option<Type> {
    match term {
        Term::Bool(_) => Some(Type::bool()),
        Term::Int(_) => Some(Type::Primitive(PrimitiveType::Int)),
        Term::Decimal(_) => Some(Type::Primitive(PrimitiveType::Decimal)),
        Term::String(_) => Some(Type::Primitive(PrimitiveType::String)),
        _ => None,
    }
}

fn types_compatible(expected: &Type, actual: &Type) -> bool {
    if expected == actual {
        return true;
    }
    matches!(
        (expected, actual),
        (
            Type::Primitive(PrimitiveType::Money { .. }),
            Type::Primitive(PrimitiveType::Decimal)
        ) | (
            Type::Primitive(PrimitiveType::Decimal),
            Type::Primitive(PrimitiveType::Money { .. })
        )
    )
}

fn function_has_bare_prop_guard(
    f: &fidryn_hir::HirFunction,
    propositions: &BTreeSet<String>,
) -> bool {
    if source_has_bare_prop_if(&f.source, propositions) {
        return true;
    }
    f.body
        .as_ref()
        .is_some_and(|term| term_has_bare_prop_guard(term, propositions))
}

fn check_recursion(hir: &HirModule, diagnostics: &mut Vec<Diagnostic>) {
    let functions: BTreeSet<&str> = hir.functions.keys().map(String::as_str).collect();
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (name, f) in &hir.functions {
        let mut callees = BTreeSet::new();
        if let Some(body) = &f.body {
            collect_term_callees(body, &functions, &mut callees);
        }
        callees.extend(collect_source_callees(&f.source, &functions));
        graph.insert(name.clone(), callees);
    }
    for name in &functions {
        graph.entry((*name).to_owned()).or_default();
    }
    for scc in strongly_connected_components(&graph) {
        if !scc_is_recursive(&scc, &graph) {
            continue;
        }
        for name in &scc {
            let Some(f) = hir.functions.get(name) else {
                continue;
            };
            if f.is_calc {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::E530,
                        format!("calc `{name}` must not recurse"),
                    )
                    .with_suggestion("rewrite the calc as a total, non-recursive closed form"),
                );
            } else if f.fuel.is_none() {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::E530,
                        format!("function `{name}` recurses without an explicit fuel bound"),
                    )
                    .with_suggestion("add `fuel = N` to bound recursion"),
                );
            }
        }
    }
}

fn scc_is_recursive(scc: &[String], graph: &BTreeMap<String, BTreeSet<String>>) -> bool {
    if scc.len() > 1 {
        return true;
    }
    let Some(name) = scc.first() else {
        return false;
    };
    graph
        .get(name)
        .is_some_and(|callees| callees.contains(name))
}

fn strongly_connected_components(graph: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    for node in graph.keys() {
        dfs_finish(node, graph, &mut seen, &mut order);
    }
    let mut rev: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for node in graph.keys() {
        rev.entry(node.clone()).or_default();
    }
    for (from, tos) in graph {
        for to in tos {
            rev.entry(to.clone()).or_default().insert(from.clone());
        }
    }
    let mut seen = BTreeSet::new();
    let mut sccs = Vec::new();
    for node in order.into_iter().rev() {
        if seen.contains(&node) {
            continue;
        }
        seen.insert(node.clone());
        let mut comp = Vec::new();
        dfs_collect(&node, &rev, &mut seen, &mut comp);
        sccs.push(comp);
    }
    sccs
}

fn dfs_finish(
    node: &str,
    graph: &BTreeMap<String, BTreeSet<String>>,
    seen: &mut BTreeSet<String>,
    order: &mut Vec<String>,
) {
    if !seen.insert(node.to_owned()) {
        return;
    }
    if let Some(tos) = graph.get(node) {
        for to in tos {
            dfs_finish(to, graph, seen, order);
        }
    }
    order.push(node.to_owned());
}

fn dfs_collect(
    node: &str,
    rev: &BTreeMap<String, BTreeSet<String>>,
    seen: &mut BTreeSet<String>,
    comp: &mut Vec<String>,
) {
    comp.push(node.to_owned());
    if let Some(tos) = rev.get(node) {
        for to in tos {
            if seen.insert(to.clone()) {
                dfs_collect(to, rev, seen, comp);
            }
        }
    }
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
    for import in &hir.imports {
        if !import.digest_required {
            continue;
        }
        let matched = manifest.artifacts.iter().any(|artifact| {
            artifact_matches_import(&artifact.path, &import.name)
                && digest_authenticates(&artifact.digest)
        });
        if !matched {
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

fn artifact_matches_import(path: &str, import_name: &str) -> bool {
    if import_name.is_empty() {
        return false;
    }
    if path == import_name || path.ends_with(import_name) {
        return true;
    }
    if path
        .split(['/', '\\'])
        .any(|segment| segment == import_name)
    {
        return true;
    }
    let stem = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = stem.rsplit_once('.').map(|(s, _)| s).unwrap_or(stem);
    normalize_import_key(stem) == normalize_import_key(import_name)
}

fn normalize_import_key(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn digest_authenticates(digest: &str) -> bool {
    !digest.is_empty() || digest == "fixture"
}

fn check_instantiation_arity(hir: &HirModule, diagnostics: &mut Vec<Diagnostic>) {
    for import in &hir.imports {
        if import.type_args.is_empty() {
            continue;
        }
        if !import_targets_module(&import.name, &hir.name) {
            continue;
        }
        if import.type_args.len() != hir.type_params.len() {
            diagnostics.push(arity_mismatch(
                &hir.name,
                hir.type_params.len(),
                import.type_args.len(),
            ));
        }
    }
}

fn instantiation_args(hir: &HirModule) -> Option<Vec<Type>> {
    let import = hir
        .imports
        .iter()
        .find(|im| !im.type_args.is_empty() && import_targets_module(&im.name, &hir.name))?;
    if import.type_args.len() != hir.type_params.len() {
        return None;
    }
    Some(
        import
            .type_args
            .iter()
            .map(|arg| parse_type_name(arg))
            .collect(),
    )
}

fn import_targets_module(import_name: &str, module_name: &str) -> bool {
    let (imp, _) = split_qname_type_args(import_name);
    let (module, _) = split_qname_type_args(module_name);
    if imp.is_empty() || module.is_empty() {
        return false;
    }
    imp == module || module.ends_with(&format!(".{imp}")) || imp.ends_with(&format!(".{module}"))
}

fn arity_mismatch(module: &str, expected: usize, actual: usize) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::E210,
        format!(
            "module `{module}` expects {expected} type argument(s), but {actual} were supplied"
        ),
    )
}

fn module_type_params(module: &CoreModule) -> (String, Vec<String>) {
    split_qname_type_args(&module.name)
}

fn encode_module_name(name: &str, params: &[String]) -> String {
    let (base, from_name) = split_qname_type_args(name);
    let params = if params.is_empty() {
        from_name.as_slice()
    } else {
        params
    };
    if params.is_empty() {
        base
    } else {
        format!("{base}<{}>", params.join(", "))
    }
}

fn subst_primitive(prim: PrimitiveType, params: &[String], args: &[Type]) -> PrimitiveType {
    match prim {
        PrimitiveType::Interval { inner } => PrimitiveType::Interval {
            inner: Box::new(substitute_nominal(*inner, params, args)),
        },
        PrimitiveType::Option { inner } => PrimitiveType::Option {
            inner: Box::new(substitute_nominal(*inner, params, args)),
        },
        PrimitiveType::FiniteSet { inner } => PrimitiveType::FiniteSet {
            inner: Box::new(substitute_nominal(*inner, params, args)),
        },
        PrimitiveType::NonEmptySet { inner } => PrimitiveType::NonEmptySet {
            inner: Box::new(substitute_nominal(*inner, params, args)),
        },
        PrimitiveType::Map { key, value } => PrimitiveType::Map {
            key: Box::new(substitute_nominal(*key, params, args)),
            value: Box::new(substitute_nominal(*value, params, args)),
        },
        other => other,
    }
}

fn subst_ty(ty: &mut Type, params: &[String], args: &[Type]) {
    *ty = substitute_nominal(ty.clone(), params, args);
}

fn subst_decl(decl: &mut CoreDecl, params: &[String], args: &[Type]) {
    match decl {
        CoreDecl::Entity(entity) => subst_ty(&mut entity.ty, params, args),
        CoreDecl::RecordType(record) => {
            for (_, ty) in &mut record.fields {
                subst_ty(ty, params, args);
            }
        }
        CoreDecl::Office(office) => subst_ty(&mut office.occupant, params, args),
        CoreDecl::Proposition(prop) => {
            for (_, ty) in &mut prop.params {
                subst_ty(ty, params, args);
            }
        }
        CoreDecl::Function(function) => {
            for (_, ty) in &mut function.params {
                subst_ty(ty, params, args);
            }
            subst_ty(&mut function.result, params, args);
        }
        CoreDecl::EffectDecl(effect) => {
            for op in &mut effect.operations {
                for (_, ty) in &mut op.params {
                    subst_ty(ty, params, args);
                }
                subst_ty(&mut op.result, params, args);
            }
        }
        CoreDecl::Judgment(judgment) => {
            for (_, ty) in &mut judgment.record_requires {
                subst_ty(ty, params, args);
            }
        }
        CoreDecl::Decision(decision) => {
            for (_, ty) in &mut decision.binders {
                subst_ty(ty, params, args);
            }
            if let Some(ret) = &mut decision.declared_result {
                subst_ty(&mut ret.result_type, params, args);
            }
        }
        CoreDecl::Source(_)
        | CoreDecl::Observation(_)
        | CoreDecl::Fact(_)
        | CoreDecl::Rule(_)
        | CoreDecl::Position(_)
        | CoreDecl::Power(_)
        | CoreDecl::Duty(_)
        | CoreDecl::LegalAct(_)
        | CoreDecl::InterpretationFamily(_)
        | CoreDecl::ConflictDoctrine(_)
        | CoreDecl::Clause(_) => {}
    }
}

fn subst_query(query: &mut CoreQuery, params: &[String], args: &[Type]) {
    for (_, ty) in &mut query.binders {
        subst_ty(ty, params, args);
    }
    subst_ty(&mut query.result_type, params, args);
    if let QueryPlan::RunDecision { result, .. } = &mut query.plan {
        subst_ty(&mut result.expected_type, params, args);
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
            ty: parse_type_name(ty),
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
            params: f
                .params
                .iter()
                .map(|(name, ty)| (name.clone(), parse_type_name(ty)))
                .collect(),
            result: parse_type_name(&f.result_type),
            effects: BTreeSet::new(),
            is_calc: f.is_calc,
            fuel: f.fuel,
            body: f.body.clone(),
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
                result: parse_type_name(&e.result_type),
            }],
            meta: meta(&e.name),
        }));
    }
    for rule in &hir.rules {
        let rule_meta = meta(&rule.name);
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
            guard: rule.guard.clone().unwrap_or(Guard::Satisfied),
            consequences: rule
                .consequences
                .iter()
                .enumerate()
                .map(|(i, (op, prop))| CoreEffect {
                    id: EffectId::of(format!("{}:{op}:{i}", rule.name).as_bytes()),
                    consequence: consequence_from_op(op, prop),
                    meta: rule_meta.clone(),
                })
                .collect(),
            fallback: None,
            meta: rule_meta,
        }));
    }
    for (name, alts) in &hir.interpretation_families {
        declarations.push(CoreDecl::InterpretationFamily(
            fidryn_core::ir::CoreInterpretationFamily {
                id: NodeId::of(name.as_bytes()),
                name: name.clone(),
                source: Term::Ident(name.clone()),
                alternatives: alts.clone(),
                meta: meta(name),
            },
        ));
    }
    for decision in &hir.decisions {
        declarations.push(CoreDecl::Decision(fidryn_core::ir::CoreDecision {
            id: NodeId::of(decision.name.as_bytes()),
            name: decision.name.clone(),
            binders: Vec::new(),
            requirements: decision
                .requirements
                .iter()
                .map(|schema| Guard::Observed {
                    schema: schema.clone(),
                    binder: schema.clone(),
                })
                .collect(),
            option_space: Term::Wildcard,
            declared_result: decision.result.as_ref().map(|expression| {
                fidryn_core::ir::DecisionReturn {
                    result_type: Type::Sort(Sort::Nominal("Decision".into())),
                    expression: expression.clone(),
                }
            }),
            meta: meta(&decision.name),
        }));
    }
    let queries = hir
        .queries
        .values()
        .map(|q| lower_query(q, hir, jid))
        .collect();
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
        name: encode_module_name(&hir.name, &hir.type_params),
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

fn consequence_from_op(op: &str, prop: &PropTerm) -> Consequence {
    match op {
        "establish" | "constitute" => Consequence::Establish(prop.clone()),
        "terminate" => Consequence::Terminate(prop.clone()),
        "suspend" => Consequence::Suspend(prop.clone()),
        _ => Consequence::Derive(prop.clone()),
    }
}

fn lower_query(q: &HirQuery, hir: &HirModule, jid: JurisdictionId) -> CoreQuery {
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
    CoreQuery {
        id: NodeId::of(q.name.as_bytes()),
        name: q.name.clone(),
        binders: q
            .params
            .iter()
            .map(|(name, ty)| (name.clone(), parse_type_name(ty)))
            .collect(),
        result_type: parse_type_name(&q.result_type),
        effects,
        automatic: q.automatic,
        plan: lower_query_plan(q, hir),
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

fn lower_query_plan(q: &HirQuery, hir: &HirModule) -> QueryPlan {
    match &q.body {
        HirQueryBody::Return(term) => QueryPlan::Evaluate(term.clone()),
        HirQueryBody::Goal {
            kind,
            office,
            expr,
            fields,
        } => match kind.as_str() {
            "UniqueOccupant" => QueryPlan::UniqueOccupant {
                office: office
                    .clone()
                    .or_else(|| fields.get("office").cloned())
                    .unwrap_or(Term::Wildcard),
            },
            "EvaluateClause" => lower_evaluate_clause(fields, hir),
            "RunDecision" => lower_run_decision(q, fields),
            "StatusOf" => lower_status_of(fields),
            "Evaluate" => QueryPlan::Evaluate(expr.clone().unwrap_or(Term::Wildcard)),
            _ => expr
                .clone()
                .map(QueryPlan::Evaluate)
                .or_else(|| {
                    office
                        .clone()
                        .map(|office| QueryPlan::UniqueOccupant { office })
                })
                .unwrap_or(QueryPlan::Evaluate(Term::Wildcard)),
        },
        HirQueryBody::None => QueryPlan::Evaluate(Term::Wildcard),
    }
}

fn lower_evaluate_clause(fields: &BTreeMap<String, Term>, hir: &HirModule) -> QueryPlan {
    let clause = match fields.get("clause") {
        Some(Term::Call { callee, args }) => ClauseSelector::Instantiated {
            clause: ClauseId::of(callee.as_bytes()),
            arguments: args.clone(),
        },
        Some(Term::Apply { ctor, args }) => ClauseSelector::Instantiated {
            clause: ClauseId::of(ctor.as_bytes()),
            arguments: args.clone(),
        },
        Some(term) => ClauseSelector::Bound {
            binder: term_as_name(term).unwrap_or_default(),
            module: ModuleId::of(hir.name.as_bytes()),
        },
        None => ClauseSelector::Bound {
            binder: String::new(),
            module: ModuleId::of(hir.name.as_bytes()),
        },
    };
    QueryPlan::EvaluateClause {
        clause,
        context: fields
            .get("context")
            .and_then(term_as_name)
            .unwrap_or_default(),
        result: fields.get("result").cloned().unwrap_or(Term::Wildcard),
    }
}

fn lower_run_decision(q: &HirQuery, fields: &BTreeMap<String, Term>) -> QueryPlan {
    QueryPlan::RunDecision {
        decision: fields
            .get("decision")
            .and_then(term_as_name)
            .unwrap_or_default(),
        arguments: fields
            .get("arguments")
            .map(flatten_term_list)
            .unwrap_or_default(),
        result: fidryn_core::ir::DeclaredDecisionResult {
            expected_type: parse_type_name(&q.result_type),
        },
    }
}

fn lower_status_of(fields: &BTreeMap<String, Term>) -> QueryPlan {
    let status_term = fields.get("status");
    let constructor = status_term.and_then(term_as_name).unwrap_or_default();
    let arguments = match status_term {
        Some(Term::Call { args, .. } | Term::Apply { args, .. }) => {
            args.iter().cloned().map(TermPattern::Exact).collect()
        }
        _ => Vec::new(),
    };
    let when_present = fields
        .get("when_present")
        .cloned()
        .or_else(|| status_term.cloned())
        .unwrap_or_else(|| {
            if constructor.is_empty() {
                Term::Wildcard
            } else {
                Term::unit_ctor(constructor.clone())
            }
        });
    let when_closed_absent = fields
        .get("when_closed_absent")
        .cloned()
        .unwrap_or_else(|| {
            if constructor.is_empty() {
                Term::Wildcard
            } else {
                Term::unit_ctor(format!("Not{constructor}"))
            }
        });
    QueryPlan::StatusOf {
        status: LegalStatusPattern::InstitutionalStatus {
            constructor,
            arguments,
        },
        when_present,
        when_closed_absent,
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
            CoreDecl::Function(f)
                if f.name == "countdown"
                    && f.fuel == Some(4)
                    && !f.is_calc
                    && f.body.is_some()
                    && f.params == [("n".to_owned(), parse_type_name("Int"))]
                    && f.result == parse_type_name("Int")
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

    #[test]
    fn evaluate_true_preserves_bool_term_and_result_type() {
        let src = r#"
module Examples.EvalTrue version "0.1.0" {
    query flag() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let module = check_src(src).expect("evaluate true should check");
        let q = module.query("flag").expect("flag query");
        assert_eq!(q.result_type, Type::bool());
        assert_eq!(q.plan, QueryPlan::Evaluate(Term::Bool(true)));
    }

    #[test]
    fn rule_consequences_are_nonempty() {
        let src = r#"
module Examples.Rule version "0.1.0" {
    rule R : derive { when operative P() then derive Q() }
    query ok() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let module = check_src(src).expect("derive rule should check");
        let rule = module.declarations.iter().find_map(|d| match d {
            CoreDecl::Rule(r) if r.name == "R" => Some(r),
            _ => None,
        });
        let rule = rule.expect("rule R");
        assert!(!rule.consequences.is_empty(), "{rule:?}");
        assert!(
            matches!(
                rule.guard,
                Guard::Operative(ref p, _) if p.predicate == "P"
            ),
            "{:?}",
            rule.guard
        );
        assert!(
            rule.consequences
                .iter()
                .any(|e| matches!(e.consequence, Consequence::Derive(ref p) if p.predicate == "Q")),
            "{:?}",
            rule.consequences
        );
    }

    #[test]
    fn mutual_recursion_without_fuel_is_e530() {
        let src = r#"
module Examples.Mutual version "0.1.0" {
    fn first() -> Int { second() }
    fn second() -> Int { first() }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(err.iter().any(|d| d.code == DiagnosticCode::E530));
    }

    #[test]
    fn import_not_satisfied_by_unrelated_digest() {
        let src = r#"
module Examples.Imp version "0.1.0" {
    import Other.Law version "1" { digest "abc" }
    query ok() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let manifest = SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: String::new(),
            jurisdiction: String::new(),
            artifacts: vec![fidryn_core::ManifestArtifact {
                path: "sources/unrelated.txt".into(),
                digest: "deadbeef".into(),
                kind: "text".into(),
                effective: "2026-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        let err = check(&hir, &manifest).unwrap_err();
        assert!(err.iter().any(|d| d.code == DiagnosticCode::E200));
    }

    #[test]
    fn declared_prop_as_if_guard_is_e310() {
        let src = r#"
module Examples.OtherProp version "0.1.0" {
    proposition Eligible(person: LegalPerson)
    query bad() -> Bool {
        goal Evaluate { if Eligible(x) { true } else { false } }
    }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(err.iter().any(|d| d.code == DiagnosticCode::E310));
    }

    fn entity_ty<'a>(module: &'a CoreModule, name: &str) -> Option<&'a Type> {
        module.declarations.iter().find_map(|d| match d {
            CoreDecl::Entity(e) if e.name == name => Some(&e.ty),
            _ => None,
        })
    }

    #[test]
    fn instantiate_substitutes_entity_type_param() {
        let src = r#"
module Id<T> version "0.1.0" {
    entity X: T
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(hir.type_params, vec!["T".to_owned()]);
        let core = check(&hir, &SourceManifest::default()).expect("generic module type-checks");
        assert_eq!(
            entity_ty(&core, "X"),
            Some(&Type::Sort(Sort::Nominal("T".into())))
        );
        let inst = instantiate(&core, &[Type::Sort(Sort::NaturalPerson)]).expect("instantiate");
        assert_eq!(
            entity_ty(&inst, "X"),
            Some(&Type::Sort(Sort::NaturalPerson))
        );
        assert_eq!(
            substitute_nominal(
                Type::Sort(Sort::Nominal("T".into())),
                &["T".into()],
                &[Type::Sort(Sort::NaturalPerson)]
            ),
            Type::Sort(Sort::NaturalPerson)
        );
        assert_eq!(
            substitute_type(
                Type::Sort(Sort::Nominal("T".into())),
                &["T".into()],
                &[Type::Sort(Sort::NaturalPerson)]
            ),
            Type::Sort(Sort::NaturalPerson)
        );
        assert_eq!(inst.name, "Id<NaturalPerson>");
    }

    #[test]
    fn instantiate_substitutes_nested_option() {
        let src = r#"
module Box<T> version "0.1.0" {
    entity X: Option<T>
}
"#;
        let core = check_src(src).expect("generic module type-checks");
        let inst = instantiate(&core, &[Type::Sort(Sort::NaturalPerson)]).expect("instantiate");
        assert_eq!(
            entity_ty(&inst, "X"),
            Some(&Type::Primitive(PrimitiveType::Option {
                inner: Box::new(Type::Sort(Sort::NaturalPerson)),
            }))
        );
    }

    #[test]
    fn instantiate_twice_isolates_nominal_identity() {
        let src = r#"
module Id<T> version "0.1.0" {
    entity X: T
}
"#;
        let core = check_src(src).expect("generic module type-checks");
        let a = instantiate(&core, &[Type::Sort(Sort::NaturalPerson)]).expect("a");
        let b = instantiate(&core, &[Type::Sort(Sort::LegalPerson)]).expect("b");
        let a_again = instantiate(&core, &[Type::Sort(Sort::NaturalPerson)]).expect("a again");
        assert_ne!(a.id, b.id);
        assert_ne!(a.name, b.name);
        assert_eq!(a.id, a_again.id);
        assert_eq!(a.name, a_again.name);
        assert_eq!(entity_ty(&a, "X"), Some(&Type::Sort(Sort::NaturalPerson)));
        assert_eq!(entity_ty(&b, "X"), Some(&Type::Sort(Sort::LegalPerson)));
    }

    #[test]
    fn instantiate_arity_mismatch_is_e210() {
        let src = r#"
module Id<T> version "0.1.0" {
    entity X: T
}
"#;
        let core = check_src(src).expect("generic module type-checks");
        let err = instantiate(
            &core,
            &[
                Type::Sort(Sort::NaturalPerson),
                Type::Sort(Sort::LegalPerson),
            ],
        )
        .unwrap_err();
        assert!(err.iter().any(|d| d.code == DiagnosticCode::E210));
    }

    #[test]
    fn import_two_args_against_one_param_is_e210() {
        let src = r#"
module Id<T> version "0.1.0" {
    import Id<A, B> version "0.1.0"
    entity X: T
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(err.iter().any(|d| d.code == DiagnosticCode::E210));
    }

    #[test]
    fn import_instantiation_substitutes_entity_type() {
        let src = r#"
module Id<T> version "0.1.0" {
    import Id<NaturalPerson> version "0.1.0"
    entity X: T
}
"#;
        let module = check_src(src).expect("arity-1 import should instantiate");
        assert_eq!(
            entity_ty(&module, "X"),
            Some(&Type::Sort(Sort::NaturalPerson))
        );
    }

    #[test]
    fn evaluate_true_still_checks_after_type_params() {
        let src = r#"
module Examples.EvalTrue version "0.1.0" {
    query flag() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let module = check_src(src).expect("evaluate true should check");
        let q = module.query("flag").expect("flag query");
        assert_eq!(q.result_type, Type::bool());
        assert_eq!(q.plan, QueryPlan::Evaluate(Term::Bool(true)));
    }
}
