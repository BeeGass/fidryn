//! Type, effect, authority, time, and stratification checking.

use fidryn_core::ir::{
    ClauseSelector, Consequence, CoreDecl, CoreDuty, CoreEffect, CoreEffectDecl, CoreEffectOp,
    CoreEntity, CoreFunction, CoreModule, CoreOffice, CoreProposition, CoreQuery, CoreRule,
    CoreVerify, Guard, NodeMeta, QueryPlan, RuleKind, VerificationBounds,
};
use fidryn_core::patterns::{LegalStatusPattern, TermPattern};
use fidryn_core::time::Interval;
use fidryn_core::types::{PrimitiveType, Sort, Type};
use fidryn_core::value::{BinOp, PropTerm, Term};
use fidryn_core::{
    ClauseId, Diagnostic, DiagnosticCode, EffectId, EffectName, JurisdictionId, ManifestArtifact,
    ModuleId, NodeId, OriginId, SourceManifest, SourceManifestId, SourceSnapshotId,
};
use fidryn_hir::{
    HirFunction, HirImport, HirModule, HirQuery, HirQueryBody, collect_source_callees,
    collect_term_callees, flatten_term_list, last_brace_inner, parse_type_name,
    source_has_bare_prop_if, split_qname_type_args, term_as_name, term_has_bare_prop_guard,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub use fidryn_core::TrustProfile;

fn permits_import(profile: TrustProfile) -> bool {
    matches!(
        profile,
        TrustProfile::Fixture | TrustProfile::ByteVerified | TrustProfile::PolicyAccepted
    )
}

/// Classify `digest` without reading artifact bytes.
///
/// `"fixture"` is [`TrustProfile::Fixture`], not byte-verified. A 32- or
/// 64-digit hex string with no bytes is [`TrustProfile::Unauthenticated`].
pub fn digest_authenticates(digest: &str) -> TrustProfile {
    authenticate_digest(digest, None)
}

/// Check `hir` against `manifest` without reading artifact files.
///
/// `"fixture"` still authenticates. A hex digest without file bytes does not.
pub fn check(hir: &HirModule, manifest: &SourceManifest) -> Result<CoreModule, Vec<Diagnostic>> {
    check_with_sources(hir, manifest, None)
}

/// Aggregate source-integrity for `manifest` from actual artifact reads.
///
/// [`TrustProfile::ByteVerified`] only when every plausible hex digest was
/// read under `source_root` and matched blake3, and at least one such hex
/// artifact exists. `"fixture"` is [`TrustProfile::Fixture`] and is never
/// ByteVerified. `source_root = None` never ByteVerified. A missing or
/// unread hex artifact is [`TrustProfile::Unauthenticated`]. This profile
/// is source-integrity only, not legal applicability or issuer authenticity.
pub fn source_integrity(manifest: &SourceManifest, source_root: Option<&Path>) -> TrustProfile {
    let mut any_fixture = false;
    let mut hex_count = 0usize;
    let mut hex_verified = 0usize;
    for artifact in &manifest.artifacts {
        let digest = artifact.digest.trim();
        if digest.eq_ignore_ascii_case("fixture") {
            any_fixture = true;
            continue;
        }
        if !is_plausible_hex_digest(digest) {
            continue;
        }
        hex_count += 1;
        if authenticate_artifact(artifact, source_root) == TrustProfile::ByteVerified {
            hex_verified += 1;
        }
    }
    if any_fixture {
        TrustProfile::Fixture
    } else if hex_count > 0 && hex_verified == hex_count {
        TrustProfile::ByteVerified
    } else {
        TrustProfile::Unauthenticated
    }
}

/// Check `hir` against `manifest`, hashing artifact files under `source_root`.
///
/// Hex digests of length 32 or 64 authenticate when they match blake3 of the
/// file at `artifact.path` relative to `source_root` (64 hex: full digest; 32
/// hex: first 16 bytes). A missing file or mismatch is E200 for
/// digest-required imports. `source_root = None` never byte-verifies.
pub fn check_with_sources(
    hir: &HirModule,
    manifest: &SourceManifest,
    source_root: Option<&Path>,
) -> Result<CoreModule, Vec<Diagnostic>> {
    let mut diagnostics = hir.diagnostics.clone();
    check_nominations(hir, &mut diagnostics);
    check_queries(hir, &mut diagnostics);
    check_doctrines(hir, &mut diagnostics);
    check_recursion(hir, &mut diagnostics);
    check_imports(hir, manifest, source_root, &mut diagnostics);
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
    let function_names: BTreeSet<&str> = hir.functions.keys().map(String::as_str).collect();
    for q in hir.queries.values() {
        if !query_declares_goal(q) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E430,
                format!("query `{}` is missing a body or explicit goal", q.name),
            ));
        }
        if q.automatic {
            let mut effects = effect_set_from_names(&q.effects);
            let callees = collect_query_callees(q, &function_names);
            effects.extend(reachable_callee_effects(&callees, &hir.functions));
            if !effects.is_empty() {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::E420,
                    format!("automatic query `{}` must have an empty effect row", q.name),
                ));
            }
        }
        if query_has_bare_prop_guard(q, &propositions) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E310,
                "a proposition was used directly as a guard; use operative/determined/assumed",
            ));
        }
        if let Some(term) = query_result_term(q) {
            let locals = locals_from_params(&q.params);
            let cx = TypeCheck {
                functions: &hir.functions,
                propositions: &propositions,
                locals: &locals,
                owner: &q.name,
            };
            if let Some(actual) = cx.infer(term, diagnostics) {
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
    }
    for f in hir.functions.values() {
        if function_has_bare_prop_guard(f, &propositions) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E310,
                "a proposition was used directly as a guard; use operative/determined/assumed",
            ));
        }
        let Some(body) = &f.body else {
            continue;
        };
        let locals = locals_from_params(&f.params);
        let cx = TypeCheck {
            functions: &hir.functions,
            propositions: &propositions,
            locals: &locals,
            owner: &f.name,
        };
        let Some(actual) = cx.infer(body, diagnostics) else {
            continue;
        };
        let expected = parse_type_name(&f.result_type);
        if !types_compatible(&expected, &actual) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E210,
                format!(
                    "function `{}` returns {actual} but is declared to return {expected}",
                    f.name
                ),
            ));
        }
    }
}

fn query_declares_goal(q: &HirQuery) -> bool {
    if !q.has_goal {
        return false;
    }
    match &q.body {
        HirQueryBody::None => false,
        HirQueryBody::Goal {
            kind,
            expr,
            office,
            fields,
        } => !kind.is_empty() || expr.is_some() || office.is_some() || !fields.is_empty(),
        HirQueryBody::Return(_) => {
            last_brace_inner(&q.plan).is_some_and(|inner| !inner.trim().is_empty())
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

fn query_result_term(q: &HirQuery) -> Option<&Term> {
    match &q.body {
        HirQueryBody::Return(term) => Some(term),
        HirQueryBody::Goal { kind, expr, .. } if kind == "Evaluate" => expr.as_ref(),
        _ => None,
    }
}

struct TypeCheck<'a> {
    functions: &'a BTreeMap<String, HirFunction>,
    propositions: &'a BTreeSet<String>,
    locals: &'a BTreeMap<String, Type>,
    owner: &'a str,
}

impl TypeCheck<'_> {
    fn infer(&self, term: &Term, diagnostics: &mut Vec<Diagnostic>) -> Option<Type> {
        match term {
            Term::Bool(_) => Some(Type::bool()),
            Term::Int(_) => Some(Type::Primitive(PrimitiveType::Int)),
            Term::Decimal(_) => Some(Type::Primitive(PrimitiveType::Decimal)),
            Term::String(_) => Some(Type::Primitive(PrimitiveType::String)),
            Term::Ident(name) => self.infer_ident(name),
            Term::Binary { op, left, right } => self.infer_binary(*op, left, right, diagnostics),
            Term::If { cond, then, else_ } => {
                self.check_bool_condition("if", cond, diagnostics);
                self.join_inferred(then, else_, diagnostics)
            }
            Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => {
                self.infer_apply(ctor, args, diagnostics)
            }
            _ => None,
        }
    }

    fn infer_ident(&self, name: &str) -> Option<Type> {
        if let Some(ty) = self.locals.get(name) {
            return Some(ty.clone());
        }
        if self.propositions.contains(name) {
            return Some(Type::prop());
        }
        None
    }

    fn infer_apply(
        &self,
        ctor: &str,
        args: &[Term],
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Type> {
        if ctor.eq_ignore_ascii_case("seq") {
            return self.infer_seq(args, diagnostics);
        }
        if ctor.eq_ignore_ascii_case("require") {
            if let Some(cond) = args.first() {
                self.check_bool_condition("require", cond, diagnostics);
            }
            return None;
        }
        if ctor.eq_ignore_ascii_case("if") {
            if let Some(cond) = args.first() {
                self.check_bool_condition("if", cond, diagnostics);
            }
            return match (args.get(1), args.get(2)) {
                (Some(then), Some(else_)) => self.join_inferred(then, else_, diagnostics),
                (Some(then), None) => self.infer(then, diagnostics),
                _ => None,
            };
        }
        if ctor.eq_ignore_ascii_case("not") {
            if let Some(inner) = args.first() {
                self.check_bool_condition("not", inner, diagnostics);
            }
            return Some(Type::bool());
        }
        if is_modal_ctor(ctor) {
            for arg in args {
                let _ = self.infer(arg, diagnostics);
            }
            return Some(Type::bool());
        }
        if ctor.eq_ignore_ascii_case("for_all") || ctor.eq_ignore_ascii_case("exists") {
            for arg in args {
                let _ = self.infer(arg, diagnostics);
            }
            return Some(Type::bool());
        }
        if let Some(function) = self.functions.get(ctor) {
            for arg in args {
                let _ = self.infer(arg, diagnostics);
            }
            return Some(parse_type_name(&function.result_type));
        }
        if is_currency_ctor(ctor) {
            for arg in args {
                let _ = self.infer(arg, diagnostics);
            }
            return Some(Type::Primitive(PrimitiveType::Money {
                currency: ctor.to_owned(),
            }));
        }
        if self.propositions.contains(ctor) {
            for arg in args {
                let _ = self.infer(arg, diagnostics);
            }
            return Some(Type::prop());
        }
        for arg in args {
            let _ = self.infer(arg, diagnostics);
        }
        None
    }

    fn infer_seq(&self, args: &[Term], diagnostics: &mut Vec<Diagnostic>) -> Option<Type> {
        let mut result = None;
        for arg in args {
            if is_require_term(arg) {
                let _ = self.infer(arg, diagnostics);
                continue;
            }
            result = self.infer(arg, diagnostics);
        }
        result
    }

    fn infer_binary(
        &self,
        op: BinOp,
        left: &Term,
        right: &Term,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Type> {
        match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let _ = self.infer(left, diagnostics);
                let _ = self.infer(right, diagnostics);
                Some(Type::bool())
            }
            BinOp::And | BinOp::Or => {
                if let Some(ty) = self.infer(left, diagnostics) {
                    self.report_bool_operand("logical", &ty, diagnostics);
                }
                if let Some(ty) = self.infer(right, diagnostics) {
                    self.report_bool_operand("logical", &ty, diagnostics);
                }
                Some(Type::bool())
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                match (
                    self.infer(left, diagnostics),
                    self.infer(right, diagnostics),
                ) {
                    (Some(left_ty), Some(right_ty)) => join_types(&left_ty, &right_ty),
                    _ => None,
                }
            }
        }
    }

    fn join_inferred(
        &self,
        then: &Term,
        else_: &Term,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Type> {
        let then_ty = self.infer(then, diagnostics);
        let else_ty = match else_ {
            Term::Wildcard => None,
            other => self.infer(other, diagnostics),
        };
        match (then_ty, else_ty) {
            (Some(left), Some(right)) => join_types(&left, &right),
            (Some(ty), None) | (None, Some(ty)) => Some(ty),
            (None, None) => None,
        }
    }

    fn check_bool_condition(
        &self,
        construct: &str,
        cond: &Term,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(ty) = self.infer(cond, diagnostics) else {
            return;
        };
        self.report_bool_operand(construct, &ty, diagnostics);
    }

    fn report_bool_operand(&self, construct: &str, ty: &Type, diagnostics: &mut Vec<Diagnostic>) {
        if ty.is_bool() {
            return;
        }
        if ty.is_prop() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E310,
                "a proposition was used directly as a guard; use operative/determined/assumed",
            ));
            return;
        }
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::E210,
            format!(
                "{construct} in `{}` has type {ty} but must be Bool",
                self.owner
            ),
        ));
    }
}

fn locals_from_params(params: &[(String, String)]) -> BTreeMap<String, Type> {
    params
        .iter()
        .map(|(name, ty)| (name.clone(), parse_type_name(ty)))
        .collect()
}

fn is_require_term(term: &Term) -> bool {
    match term {
        Term::Apply { ctor, .. } | Term::Call { callee: ctor, .. } => {
            ctor.eq_ignore_ascii_case("require")
        }
        _ => false,
    }
}

fn is_modal_ctor(name: &str) -> bool {
    matches!(
        name,
        "operative" | "determined" | "assumed" | "observed" | "necessarily"
    )
}

fn is_currency_ctor(name: &str) -> bool {
    let len = name.len();
    (3..=4).contains(&len) && name.bytes().all(|b| b.is_ascii_uppercase())
}

fn join_types(left: &Type, right: &Type) -> Option<Type> {
    if left == right {
        return Some(left.clone());
    }
    if types_compatible(left, right) {
        return Some(prefer_money(left, right));
    }
    None
}

fn prefer_money(left: &Type, right: &Type) -> Type {
    match (left, right) {
        (Type::Primitive(PrimitiveType::Money { .. }), _) => left.clone(),
        (_, Type::Primitive(PrimitiveType::Money { .. })) => right.clone(),
        _ => left.clone(),
    }
}

fn types_compatible(expected: &Type, actual: &Type) -> bool {
    if expected == actual {
        return true;
    }
    // Tax closed forms may return Decimal for a Money formula. Distinct
    // currencies are not aliases: Money<USD> != Money<EUR> via PartialEq.
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

fn function_has_bare_prop_guard(f: &HirFunction, propositions: &BTreeSet<String>) -> bool {
    if source_has_bare_prop_if(&f.source, propositions) {
        return true;
    }
    f.body
        .as_ref()
        .is_some_and(|term| term_has_bare_prop_guard(term, propositions))
}

fn collect_query_callees(q: &HirQuery, functions: &BTreeSet<&str>) -> BTreeSet<String> {
    let mut callees = BTreeSet::new();
    match &q.body {
        HirQueryBody::Return(term) => collect_term_callees(term, functions, &mut callees),
        HirQueryBody::Goal {
            expr,
            office,
            fields,
            ..
        } => {
            if let Some(term) = expr {
                collect_term_callees(term, functions, &mut callees);
            }
            if let Some(term) = office {
                collect_term_callees(term, functions, &mut callees);
            }
            for term in fields.values() {
                collect_term_callees(term, functions, &mut callees);
            }
        }
        HirQueryBody::None => {}
    }
    callees.extend(collect_source_callees(&q.plan, functions));
    callees
}

fn reachable_callee_effects(
    start: &BTreeSet<String>,
    functions: &BTreeMap<String, HirFunction>,
) -> BTreeSet<EffectName> {
    let names: BTreeSet<&str> = functions.keys().map(String::as_str).collect();
    let mut stack: Vec<String> = start.iter().cloned().collect();
    let mut seen = BTreeSet::new();
    let mut effects = BTreeSet::new();
    while let Some(name) = stack.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(function) = functions.get(&name) else {
            continue;
        };
        effects.extend(function_effect_set(function));
        let mut callees = BTreeSet::new();
        if let Some(body) = &function.body {
            collect_term_callees(body, &names, &mut callees);
        }
        callees.extend(collect_source_callees(&function.source, &names));
        stack.extend(callees);
    }
    effects
}

fn parse_effect_name(name: &str) -> Option<EffectName> {
    match name {
        "Observe" => Some(EffectName::Observe),
        "Determine" => Some(EffectName::Determine),
        "Choose" => Some(EffectName::Choose),
        "Interpret" => Some(EffectName::Interpret),
        "ResolveNormConflict" => Some(EffectName::ResolveNormConflict),
        "SelectApplicableLaw" => Some(EffectName::SelectApplicableLaw),
        _ => None,
    }
}

fn effect_set_from_names<I>(names: I) -> BTreeSet<EffectName>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    names
        .into_iter()
        .filter_map(|name| parse_effect_name(name.as_ref()))
        .collect()
}

fn extract_effect_names(src: &str) -> Vec<String> {
    let mut search = src;
    while let Some(bang) = search.find('!') {
        let after = search[bang + 1..].trim_start();
        if let Some(inner) = after.strip_prefix('{')
            && let Some(close) = inner.find('}')
        {
            return inner[..close]
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
        }
        search = &search[bang + 1..];
    }
    Vec::new()
}

fn function_effect_names(function: &HirFunction) -> Vec<String> {
    extract_effect_names(&function.source)
}

fn function_effect_set(function: &HirFunction) -> BTreeSet<EffectName> {
    effect_set_from_names(function_effect_names(function))
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

fn check_imports(
    hir: &HirModule,
    manifest: &SourceManifest,
    source_root: Option<&Path>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for import in &hir.imports {
        if !import.digest_required {
            continue;
        }
        let matched = manifest.artifacts.iter().any(|artifact| {
            artifact_matches_import(artifact, import)
                && permits_import(authenticate_artifact(artifact, source_root))
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

fn artifact_matches_import(artifact: &ManifestArtifact, import: &HirImport) -> bool {
    if let Some(digest) = import
        .digest
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        && artifact.digest.trim().eq_ignore_ascii_case(digest)
    {
        return true;
    }
    path_matches_import(&artifact.path, &import.name)
}

fn path_matches_import(path: &str, import_name: &str) -> bool {
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

fn authenticate_artifact(artifact: &ManifestArtifact, source_root: Option<&Path>) -> TrustProfile {
    let digest = artifact.digest.trim();
    if digest.eq_ignore_ascii_case("fixture") {
        return TrustProfile::Fixture;
    }
    if !is_plausible_hex_digest(digest) {
        return TrustProfile::Unauthenticated;
    }
    let Some(root) = source_root else {
        return TrustProfile::Unauthenticated;
    };
    match read_artifact_bytes(root, &artifact.path) {
        Some(bytes) => authenticate_digest(digest, Some(&bytes)),
        None => TrustProfile::Unauthenticated,
    }
}

fn authenticate_digest(digest: &str, bytes: Option<&[u8]>) -> TrustProfile {
    let digest = digest.trim();
    if digest.eq_ignore_ascii_case("fixture") {
        return TrustProfile::Fixture;
    }
    let Some(bytes) = bytes else {
        return TrustProfile::Unauthenticated;
    };
    if is_plausible_hex_digest(digest) && digest_matches_bytes(digest, bytes) {
        TrustProfile::ByteVerified
    } else {
        TrustProfile::Unauthenticated
    }
}

fn read_artifact_bytes(source_root: &Path, artifact_path: &str) -> Option<Vec<u8>> {
    let path = Path::new(artifact_path);
    if path.is_absolute() {
        std::fs::read(path).ok()
    } else {
        std::fs::read(source_root.join(path)).ok()
    }
}

fn digest_matches_bytes(digest: &str, bytes: &[u8]) -> bool {
    let hash = *blake3::hash(bytes).as_bytes();
    match digest.len() {
        64 => digest.eq_ignore_ascii_case(&encode_hex(&hash)),
        32 => digest.eq_ignore_ascii_case(&encode_hex(&hash[..16])),
        _ => false,
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn is_plausible_hex_digest(digest: &str) -> bool {
    matches!(digest.len(), 32 | 64) && digest.bytes().all(|b| b.is_ascii_hexdigit())
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
    for (name, params) in &hir.propositions {
        declarations.push(CoreDecl::Proposition(CoreProposition {
            id: NodeId::of(name.as_bytes()),
            name: name.clone(),
            params: params
                .iter()
                .map(|(param, ty)| (param.clone(), parse_type_name(ty)))
                .collect(),
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
            effects: function_effect_set(f),
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
    for duty in &hir.duties {
        let mut content = duty.content.clone();
        if let Some(due) = &duty.due {
            content.push(Term::Apply {
                ctor: "due".into(),
                args: vec![due.clone()],
            });
        }
        declarations.push(CoreDecl::Duty(CoreDuty {
            id: NodeId::of(duty.name.as_bytes()),
            name: duty.name.clone(),
            bearer: Term::Ident(duty.bearer.clone()),
            claimant: duty.claimant.clone().map(Term::Ident),
            attaches: duty.attaches.clone().unwrap_or(Guard::Satisfied),
            content,
            meta: meta(&duty.name),
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
    let verifications = lower_verifications(hir, meta);
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

fn lower_verifications(hir: &HirModule, meta: impl Fn(&str) -> NodeMeta) -> Vec<CoreVerify> {
    let mut verifications = Vec::new();
    for item in &hir.verifications {
        verifications.push(CoreVerify {
            id: NodeId::of(item.name.as_bytes()),
            name: item.name.clone(),
            bounds: item.bounds.as_ref().map_or(
                VerificationBounds {
                    persons: 0,
                    events: 0,
                    time_points: 0,
                },
                |bounds| VerificationBounds {
                    persons: bounds.persons,
                    events: bounds.events,
                    time_points: bounds.time_points,
                },
            ),
            formula: item.formula.clone(),
            meta: meta(&item.name),
        });
    }
    for (i, q) in hir.quantifiers.iter().enumerate() {
        let name = format!("{}:{}:{i}", q.kind.as_str(), q.binder);
        verifications.push(CoreVerify {
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
        });
    }
    verifications
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
    let effects = effect_set_from_names(&q.effects);
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
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

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

    #[test]
    fn automatic_query_calling_determine_function_is_e420() {
        let src = r#"
module Examples.AutoDet version "0.1.0" {
    proposition P(x: LegalPerson)
    entity A : LegalPerson
    fn f() -> Bool ! {Determine} { determined(P(A)) }
    query automatic q() -> Bool { return f() }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E420),
            "{err:?}"
        );
    }

    #[test]
    fn evaluate_int_as_bool_query_is_e210() {
        let src = r#"
module Examples.EvalInt version "0.1.0" {
    query q() -> Bool { goal Evaluate { 7 } }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E210),
            "{err:?}"
        );
    }

    #[test]
    fn digest_abc_does_not_authenticate_required_import() {
        let src = r#"
module Examples.ImpAbc version "0.1.0" {
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
                path: "Other.Law".into(),
                digest: "abc".into(),
                kind: "text".into(),
                effective: "2026-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        let err = check(&hir, &manifest).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
    }

    #[test]
    fn fixture_digest_authenticates_required_import() {
        let src = r#"
module Examples.ImpFixture version "0.1.0" {
    import Other.Law version "1" { digest "fixture" }
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
                path: "Other.Law".into(),
                digest: "FIXTURE".into(),
                kind: "text".into(),
                effective: "2026-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        check(&hir, &manifest).expect("fixture digest authenticates");
    }

    #[test]
    fn determine_effect_row_is_preserved_on_core_function() {
        let src = r#"
module Examples.FnEff version "0.1.0" {
    proposition P(x: LegalPerson)
    entity A : LegalPerson
    fn f() -> Bool ! {Determine} { determined(P(A)) }
    query q() -> Bool ! {Determine} {
        goal Evaluate { true }
    }
}
"#;
        let module = check_src(src).expect("declared Determine should check");
        let function = module.declarations.iter().find_map(|d| match d {
            CoreDecl::Function(function) if function.name == "f" => Some(function),
            _ => None,
        });
        let function = function.expect("function f");
        assert!(
            function.effects.contains(&EffectName::Determine),
            "{:?}",
            function.effects
        );
    }

    #[test]
    fn digest_authenticates_fixture_not_hex() {
        assert_eq!(digest_authenticates("fixture"), TrustProfile::Fixture);
        assert_eq!(digest_authenticates("Fixture"), TrustProfile::Fixture);
        assert_ne!(digest_authenticates("fixture"), TrustProfile::ByteVerified);
        assert_eq!(
            digest_authenticates(&"ab".repeat(16)),
            TrustProfile::Unauthenticated
        );
        assert_eq!(
            digest_authenticates(&"ab".repeat(32)),
            TrustProfile::Unauthenticated
        );
        assert_eq!(digest_authenticates("abc"), TrustProfile::Unauthenticated);
        assert_eq!(
            digest_authenticates("deadbeef"),
            TrustProfile::Unauthenticated
        );
        assert_eq!(digest_authenticates(""), TrustProfile::Unauthenticated);
    }

    fn digest_import_src() -> &'static str {
        r#"
module Examples.ImpBytes version "0.1.0" {
    import Other.Law version "1" { digest "abc" }
    query ok() -> Bool {
        goal Evaluate { true }
    }
}
"#
    }

    fn artifact_manifest(path: &str, digest: &str) -> SourceManifest {
        SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: String::new(),
            jurisdiction: String::new(),
            artifacts: vec![ManifestArtifact {
                path: path.into(),
                digest: digest.into(),
                kind: "text".into(),
                effective: "2026-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        }
    }

    fn check_digest_import(
        manifest: &SourceManifest,
        source_root: Option<&Path>,
    ) -> Result<CoreModule, Vec<Diagnostic>> {
        let parsed = parse_file(digest_import_src());
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        check_with_sources(&hir, manifest, source_root)
    }

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("fidryn-check-{}-{n}", std::process::id()));
            fs::create_dir_all(&path).expect("source_root");
            Self(path)
        }

        fn write(&self, name: &str, bytes: &[u8]) {
            fs::write(self.0.join(name), bytes).expect("write artifact");
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn hex_digest_without_bytes_is_e200() {
        let bytes = b"fidryn-source-bytes";
        let digest = encode_hex(blake3::hash(bytes).as_bytes());
        let manifest = artifact_manifest("Other.Law", &digest);
        let err = check_digest_import(&manifest, None).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
    }

    #[test]
    fn matching_blake3_hex_authenticates_required_import() {
        let bytes = b"fidryn-source-bytes";
        let digest = encode_hex(blake3::hash(bytes).as_bytes());
        let root = TempRoot::new();
        root.write("Other.Law", bytes);
        let manifest = artifact_manifest("Other.Law", &digest);
        check_digest_import(&manifest, Some(&root.0)).expect("matching blake3 hex authenticates");
        assert_eq!(
            authenticate_artifact(&manifest.artifacts[0], Some(&root.0)),
            TrustProfile::ByteVerified
        );
    }

    #[test]
    fn matching_truncated_blake3_hex_authenticates_required_import() {
        let bytes = b"fidryn-source-bytes";
        let digest = encode_hex(&blake3::hash(bytes).as_bytes()[..16]);
        assert_eq!(digest.len(), 32);
        let root = TempRoot::new();
        root.write("Other.Law", bytes);
        let manifest = artifact_manifest("Other.Law", &digest);
        check_digest_import(&manifest, Some(&root.0)).expect("32-hex blake3 prefix authenticates");
    }

    #[test]
    fn mismatched_blake3_hex_is_e200() {
        let bytes = b"fidryn-source-bytes";
        let wrong = encode_hex(blake3::hash(b"tampered-bytes").as_bytes());
        let root = TempRoot::new();
        root.write("Other.Law", bytes);
        let manifest = artifact_manifest("Other.Law", &wrong);
        let err = check_digest_import(&manifest, Some(&root.0)).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
        assert_eq!(
            authenticate_artifact(&manifest.artifacts[0], Some(&root.0)),
            TrustProfile::Unauthenticated
        );
    }

    #[test]
    fn missing_artifact_file_for_hex_digest_is_e200() {
        let digest = encode_hex(blake3::hash(b"fidryn-source-bytes").as_bytes());
        let root = TempRoot::new();
        let manifest = artifact_manifest("Other.Law", &digest);
        let err = check_digest_import(&manifest, Some(&root.0)).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
    }

    #[test]
    fn source_integrity_requires_checked_hex_bytes() {
        assert_eq!(
            source_integrity(&SourceManifest::default(), None),
            TrustProfile::Unauthenticated
        );
        let fixture = artifact_manifest("Other.Law", "fixture");
        assert_eq!(source_integrity(&fixture, None), TrustProfile::Fixture);
        assert_ne!(source_integrity(&fixture, None), TrustProfile::ByteVerified);
        let bytes = b"fidryn-source-bytes";
        let digest = encode_hex(blake3::hash(bytes).as_bytes());
        let hex = artifact_manifest("never-created.txt", &digest);
        let root = TempRoot::new();
        assert_eq!(
            source_integrity(&hex, Some(&root.0)),
            TrustProfile::Unauthenticated
        );
        assert_ne!(
            source_integrity(&hex, Some(&root.0)),
            TrustProfile::ByteVerified
        );
        root.write("Other.Law", bytes);
        let matched = artifact_manifest("Other.Law", &digest);
        assert_eq!(
            source_integrity(&matched, Some(&root.0)),
            TrustProfile::ByteVerified
        );
        assert_eq!(
            source_integrity(&matched, None),
            TrustProfile::Unauthenticated
        );
    }

    fn digest_identity_src(digest: &str) -> String {
        format!(
            r#"
module Examples.ImpDigest version "0.1.0" {{
    import Other.Law version "1" {{ digest "{digest}" }}
    query ok() -> Bool {{
        goal Evaluate {{ true }}
    }}
}}
"#
        )
    }

    #[test]
    fn import_digest_identity_matches_unrelated_path() {
        let bytes = b"fidryn-source-bytes";
        let digest = encode_hex(blake3::hash(bytes).as_bytes());
        let root = TempRoot::new();
        root.write("unrelated.txt", bytes);
        let manifest = artifact_manifest("unrelated.txt", &digest);
        let parsed = parse_file(&digest_identity_src(&digest));
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(hir.imports[0].digest.as_deref(), Some(digest.as_str()));
        assert_eq!(hir.imports[0].version.as_deref(), Some("1"));
        check_with_sources(&hir, &manifest, Some(&root.0))
            .expect("exact digest identity authenticates without a path heuristic");
    }

    #[test]
    fn named_verify_trivial_lowers_to_core_verify() {
        let src = r#"
module Examples.Trivial version "0.1.0" {
    verify Trivial { assert true }
    query ok() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let module = check_src(src).expect("named verify should check");
        let verify = module
            .verifications
            .iter()
            .find(|v| v.name == "Trivial")
            .expect("CoreVerify Trivial");
        assert!(
            verify.formula == "assert true" || verify.formula == "true",
            "{:?}",
            verify.formula
        );
    }

    #[test]
    fn late_payment_style_duty_lowers_to_core_duty() {
        let src = r#"
module Programs.LatePayment version "0.1.0" {
    entity Payer : NaturalPerson
    entity Payee : NaturalPerson
    proposition InvoiceIssued(person: NaturalPerson)
    duty PayInvoice {
        bearer Payer
        claimant Payee
        attaches when operative InvoiceIssued(Payer)
        content USD(100.00)
        due 0 counted_days after invoice_date
    }
    query ok() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let module = check_src(src).expect("declared duty should check");
        let duty = module
            .declarations
            .iter()
            .find_map(|decl| match decl {
                CoreDecl::Duty(duty) if duty.name == "PayInvoice" => Some(duty),
                _ => None,
            })
            .expect("CoreDuty PayInvoice");
        assert_eq!(duty.bearer, Term::Ident("Payer".into()));
        assert_eq!(duty.claimant, Some(Term::Ident("Payee".into())));
        assert!(
            matches!(
                &duty.attaches,
                Guard::Operative(prop, _) if prop.predicate == "InvoiceIssued"
            ),
            "{:?}",
            duty.attaches
        );
        assert!(
            duty.content.iter().any(|term| matches!(
                term,
                Term::Apply { ctor, .. } if ctor == "USD"
            )),
            "{:?}",
            duty.content
        );
        let due = duty.content.last().expect("due content");
        assert!(
            matches!(due, Term::Apply { ctor, args } if ctor == "due" && args.len() == 1),
            "{due:?}"
        );
        let prop = module
            .declarations
            .iter()
            .find_map(|decl| match decl {
                CoreDecl::Proposition(prop) if prop.name == "InvoiceIssued" => Some(prop),
                _ => None,
            })
            .expect("proposition InvoiceIssued");
        assert_eq!(
            prop.params,
            vec![("person".to_owned(), Type::Sort(Sort::NaturalPerson))]
        );
    }

    #[test]
    fn sequencing_does_not_hide_a_wrong_result_type() {
        let src = r#"
module Review version "0.1.0" {
    query q() -> Bool { require true; return 7 }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E210),
            "{err:?}"
        );
    }

    #[test]
    fn a_declared_function_result_does_not_replace_checking_its_body() {
        let src = r#"
module Review version "0.1.0" {
    fn wrong() -> Bool { 7 }
    query q() -> Bool { return wrong() }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E210),
            "{err:?}"
        );
    }

    #[test]
    fn require_true_return_int_matches_int_query() {
        let src = r#"
module Examples.ReqInt version "0.1.0" {
    query q() -> Int { require true; return 7 }
}
"#;
        check_src(src).expect("int result after require should check");
    }

    #[test]
    fn tax_closed_form_money_decimal_still_checks() {
        let src = r#"
module Examples.Tax version "0.1.0" {
    calc ordinary_income_tax_formula(amount: Money<USD>) -> Money<USD> {
        if amount <= 11925.00 {
            amount * 0.10
        } else {
            if amount <= 48475.00 {
                11925.00 * 0.10 + (amount - 11925.00) * 0.12
            } else {
                if amount <= 103350.00 {
                    11925.00 * 0.10 + (48475.00 - 11925.00) * 0.12 + (amount - 48475.00) * 0.22
                } else {
                    17651.00 + (amount - 103350.00) * 0.24
                }
            }
        }
    }
    query automatic tax_on(amount: Money<USD>) -> Money<USD> {
        return ordinary_income_tax_formula(amount)
    }
}
"#;
        check_src(src).expect("tax closed form should check");
    }
}
