//! Type, effect, authority, time, and stratification checking.

use fidryn_core::ir::{
    ClauseSelector, Consequence, CoreDecl, CoreDuty, CoreEffect, CoreEffectDecl, CoreEffectOp,
    CoreEntity, CoreFunction, CoreModule, CoreOffice, CoreProposition, CoreQuery, CoreRule,
    CoreVerify, Guard, NodeMeta, QueryPlan, RuleKind, VerificationBounds,
};
use fidryn_core::patterns::{LegalStatusPattern, TermPattern};
use fidryn_core::time::Interval;
use fidryn_core::types::{PrimitiveType, Sort, Type, is_subtype};
use fidryn_core::value::{BinOp, PropTerm, Term};
use fidryn_core::{
    ClauseId, Diagnostic, DiagnosticCode, EffectId, EffectName, JurisdictionId, ManifestArtifact,
    ModuleId, NodeId, OriginId, PackageLock, SourceManifest, SourceManifestId, SourceSnapshotId,
    artifact_path_is_package, is_safe_package_name, package_dir_from_artifact_path,
    package_name_from_import, package_path_matches_import, packages_root,
};
use fidryn_hir::{
    HirEffect, HirFunction, HirImport, HirModule, HirQuery, HirQueryBody, collect_source_callees,
    collect_term_callees, flatten_term_list, last_brace_inner, parse_type_name,
    source_has_bare_prop_if, split_qname_type_args, term_as_name, term_has_bare_prop_guard,
};
use fidryn_syntax::ast::Item;
use fidryn_syntax::parse_file;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Nested package linking on path compile. Mill / `check_source` never
/// increment this: `source_root = None` does not walk `packages/`.
const PACKAGE_LINK_DEPTH_CAP: usize = 8;

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
/// digest-required imports. `source_root = None` never byte-verifies and
/// never reads `packages/` from the working directory.
///
/// When `source_root` is set and a package artifact authenticates, those
/// already-hashed bytes are parsed and checked. Nested `import`s in a
/// package are resolved against `source_root/packages` (depth cap 8;
/// a cycle or missing nested digest is E200) and unique declarations are
/// merged into the importer. `source_root = None` never walks `packages/`.
/// Unique names are kept as-is; collisions are E200.
pub fn check_with_sources(
    hir: &HirModule,
    manifest: &SourceManifest,
    source_root: Option<&Path>,
) -> Result<CoreModule, Vec<Diagnostic>> {
    let mut seen = BTreeSet::new();
    Ok(check_module(hir, manifest, source_root, true, &mut seen, 0)?.0)
}

fn check_module(
    hir: &HirModule,
    manifest: &SourceManifest,
    source_root: Option<&Path>,
    link_imports: bool,
    seen: &mut BTreeSet<String>,
    depth: usize,
) -> Result<(CoreModule, LinkedImports), Vec<Diagnostic>> {
    let mut diagnostics = hir.diagnostics.clone();
    check_nominations(hir, &mut diagnostics);
    check_imports(hir, manifest, source_root, &mut diagnostics);
    let linked = if link_imports {
        link_authenticated_packages(hir, manifest, source_root, seen, depth, &mut diagnostics)
    } else {
        LinkedImports::default()
    };
    check_queries(hir, &linked.functions, &mut diagnostics);
    check_doctrines(hir, &mut diagnostics);
    check_recursion(hir, &mut diagnostics);
    check_sources(hir, &mut diagnostics);
    check_instantiation_arity(hir, &mut diagnostics);
    if has_errors(&diagnostics) {
        return Err(diagnostics);
    }
    let mut core = lower(hir, manifest, &linked.functions);
    merge_linked_modules(&mut core, &linked.modules, &mut diagnostics);
    if has_errors(&diagnostics) {
        return Err(diagnostics);
    }
    let core = match instantiation_args(hir) {
        Some(args) => instantiate(&core, &args)?,
        None => core,
    };
    Ok((core, linked))
}

fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|d| d.code.severity() == fidryn_core::Severity::Error)
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

fn check_queries(
    hir: &HirModule,
    imported_functions: &BTreeMap<String, HirFunction>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let propositions: BTreeSet<String> = hir.propositions.keys().cloned().collect();
    let inferred = infer_module_effects(hir, imported_functions);
    let mut functions = hir.functions.clone();
    for (name, function) in imported_functions {
        functions.entry(name.clone()).or_insert(function.clone());
    }
    let entities: BTreeMap<String, Type> = hir
        .entities
        .iter()
        .map(|(name, ty)| (name.clone(), parse_type_name(ty)))
        .collect();
    for q in hir.queries.values() {
        if !query_declares_goal(q) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E430,
                format!("query `{}` is missing a body or explicit goal", q.name),
            ));
        }
        if q.automatic && !inferred.query(&q.name).is_empty() {
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
        if let Some(term) = query_result_term(q) {
            let locals = locals_from_params(&q.params);
            let cx = TypeCheck {
                functions: &functions,
                propositions: &propositions,
                entities: &entities,
                effects: &hir.effects,
                locals: &locals,
                owner: &q.name,
            };
            cx.check_result(
                term,
                &parse_type_name(&q.result_type),
                "query",
                true,
                diagnostics,
            );
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
            functions: &functions,
            propositions: &propositions,
            entities: &entities,
            effects: &hir.effects,
            locals: &locals,
            owner: &f.name,
        };
        cx.check_result(
            body,
            &parse_type_name(&f.result_type),
            "function",
            false,
            diagnostics,
        );
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
    entities: &'a BTreeMap<String, Type>,
    effects: &'a BTreeMap<String, HirEffect>,
    locals: &'a BTreeMap<String, Type>,
    owner: &'a str,
}

impl TypeCheck<'_> {
    fn check_result(
        &self,
        term: &Term,
        expected: &Type,
        item: &str,
        allow_case_bindings: bool,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let before = diagnostics.len();
        match self.infer(term, diagnostics) {
            Some(actual) => {
                if !types_compatible_in(self.owner, expected, &actual) {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::E210,
                        format!(
                            "{item} `{}` returns {actual} but is declared to return {expected}",
                            self.owner
                        ),
                    ));
                }
            }
            None if diagnostics.len() > before => {}
            None if allow_case_bindings && result_may_be_open(term) => {}
            None => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::E210,
                    format!(
                        "{item} `{}` has an untyped result but is declared to return {expected}",
                        self.owner
                    ),
                ));
            }
        }
    }

    fn infer(&self, term: &Term, diagnostics: &mut Vec<Diagnostic>) -> Option<Type> {
        match term {
            Term::Bool(_) => Some(Type::bool()),
            Term::Int(_) => Some(Type::Primitive(PrimitiveType::Int)),
            Term::Decimal(_) => Some(Type::Primitive(PrimitiveType::Decimal)),
            Term::String(_) => Some(Type::Primitive(PrimitiveType::String)),
            Term::Instant(_) => Some(Type::Primitive(PrimitiveType::Time)),
            Term::Duration(_) => Some(Type::Primitive(PrimitiveType::Duration {
                calendar: "counted_days".into(),
            })),
            Term::Ident(name) | Term::Binder(name) => self.infer_ident(name),
            Term::Binary { op, left, right } => self.infer_binary(*op, left, right, diagnostics),
            Term::If { cond, then, else_ } => {
                self.check_bool_condition("if", cond, diagnostics);
                self.join_inferred(then, else_, diagnostics)
            }
            Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => {
                self.infer_apply(ctor, args, diagnostics)
            }
            Term::Field { base, .. } => {
                let _ = self.infer(base, diagnostics);
                None
            }
            Term::Set(xs) => self.infer_set(xs, diagnostics),
            Term::Record(fields) => {
                for value in fields.values() {
                    let _ = self.infer(value, diagnostics);
                }
                None
            }
            Term::Wildcard => None,
        }
    }

    fn infer_ident(&self, name: &str) -> Option<Type> {
        if let Some(ty) = self.locals.get(name) {
            return Some(ty.clone());
        }
        if let Some(ty) = self.entities.get(name) {
            return Some(ty.clone());
        }
        if self.propositions.contains(name) {
            return Some(Type::prop());
        }
        if let Some(effect) = self.effects.get(name) {
            return Some(parse_type_name(&effect.result_type));
        }
        None
    }

    fn infer_apply(
        &self,
        ctor: &str,
        args: &[Term],
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Type> {
        if ctor.eq_ignore_ascii_case("seq") || ctor.eq_ignore_ascii_case("transaction") {
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
        if is_proposition_modal(ctor) {
            self.check_modal_proposition(ctor, args, diagnostics);
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
        if ctor == "." || ctor.eq_ignore_ascii_case("call") || ctor.eq_ignore_ascii_case("apply") {
            for arg in args {
                let _ = self.infer(arg, diagnostics);
            }
            return None;
        }
        if let Some(function) = self.functions.get(ctor) {
            self.check_call_args(function, args, diagnostics);
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
        if is_duration_unit(ctor) {
            for arg in args {
                let _ = self.infer(arg, diagnostics);
            }
            return Some(Type::Primitive(PrimitiveType::Duration {
                calendar: ctor.to_owned(),
            }));
        }
        if let Some(effect) = self.effects.get(ctor) {
            self.check_named_args(
                &effect.name,
                &source_typed_params(&effect.source),
                args,
                diagnostics,
            );
            return Some(parse_type_name(&effect.result_type));
        }
        if self.propositions.contains(ctor) {
            for arg in args {
                let _ = self.infer(arg, diagnostics);
            }
            return Some(Type::prop());
        }
        if looks_like_proposition(ctor) && args.is_empty() {
            return Some(Type::prop());
        }
        if looks_like_proposition(ctor) {
            for arg in args {
                let _ = self.infer(arg, diagnostics);
            }
            return Some(Type::Sort(fidryn_core::Sort::Nominal(ctor.to_owned())));
        }
        if is_residual_core_op(ctor) {
            for arg in args {
                let _ = self.infer(arg, diagnostics);
            }
            return None;
        }
        for arg in args {
            let _ = self.infer(arg, diagnostics);
        }
        // Unknown constructors in independently written instruments are
        // open observations, not Int/String mismatches on a resolved callee.
        // Known-function argument types and `if` joins still fail closed.
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

    fn infer_set(&self, xs: &[Term], diagnostics: &mut Vec<Diagnostic>) -> Option<Type> {
        let mut inner = None;
        for x in xs {
            match (inner.clone(), self.infer(x, diagnostics)) {
                (current, None) => inner = current,
                (None, Some(ty)) => inner = Some(ty),
                (Some(left), Some(right)) => inner = self.join_types(&left, &right),
            }
        }
        inner.map(|inner| {
            Type::Primitive(PrimitiveType::FiniteSet {
                inner: Box::new(inner),
            })
        })
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
                    (Some(left_ty), Some(right_ty)) => self.join_types(&left_ty, &right_ty),
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
            (Some(left), Some(right)) => match self.join_types(&left, &right) {
                Some(ty) => Some(ty),
                None => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::E210,
                        format!(
                            "if branches in `{}` have incompatible types {left} and {right}",
                            self.owner
                        ),
                    ));
                    None
                }
            },
            (Some(ty), None) | (None, Some(ty)) => Some(ty),
            (None, None) => None,
        }
    }

    fn join_types(&self, left: &Type, right: &Type) -> Option<Type> {
        if left == right {
            return Some(left.clone());
        }
        if types_compatible_in(self.owner, left, right)
            || types_compatible_in(self.owner, right, left)
        {
            return Some(prefer_join(left, right));
        }
        None
    }

    fn check_call_args(
        &self,
        function: &HirFunction,
        args: &[Term],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        self.check_named_args(
            &function.name,
            &params_as_types(&function.params),
            args,
            diagnostics,
        );
    }

    fn check_named_args(
        &self,
        callee: &str,
        params: &[(String, Type)],
        args: &[Term],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if args.len() != params.len() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E210,
                format!(
                    "`{callee}` expects {} argument(s), but {} were supplied",
                    params.len(),
                    args.len()
                ),
            ));
        }
        for (arg, (name, expected)) in args.iter().zip(params.iter()) {
            let before = diagnostics.len();
            match self.infer(arg, diagnostics) {
                Some(actual) if !types_compatible(expected, &actual) => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::E210,
                        format!(
                            "argument `{name}` of `{callee}` has type {actual} but is declared {expected}"
                        ),
                    ));
                }
                None if diagnostics.len() == before && !is_case_binding(arg) => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::E210,
                        format!("argument `{name}` of `{callee}` could not be typed as {expected}"),
                    ));
                }
                _ => {}
            }
        }
    }

    fn check_modal_proposition(
        &self,
        ctor: &str,
        args: &[Term],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(arg) = args.first() else {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E210,
                format!("`{ctor}` in `{}` expects a proposition", self.owner),
            ));
            return;
        };
        let before = diagnostics.len();
        match self.infer(arg, diagnostics) {
            Some(ty) if ty.is_prop() => {}
            Some(ty) => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::E210,
                    format!(
                        "`{ctor}` in `{}` requires a proposition, not {ty}",
                        self.owner
                    ),
                ));
            }
            None if diagnostics.len() == before => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::E210,
                    format!("`{ctor}` in `{}` requires a proposition", self.owner),
                ));
            }
            None => {}
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

fn is_proposition_modal(name: &str) -> bool {
    matches!(name, "operative" | "determined" | "assumed" | "necessarily")
}

fn is_currency_ctor(name: &str) -> bool {
    let len = name.len();
    (3..=4).contains(&len) && name.bytes().all(|b| b.is_ascii_uppercase())
}

fn is_duration_unit(name: &str) -> bool {
    matches!(
        name,
        "days"
            | "working_days"
            | "counted_days"
            | "calendar_days"
            | "hours"
            | "minutes"
            | "seconds"
    )
}

fn looks_like_proposition(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
}

fn is_case_binding(term: &Term) -> bool {
    match term {
        Term::Ident(_)
        | Term::Binder(_)
        | Term::Wildcard
        | Term::Field { .. }
        | Term::Record(_) => true,
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args }
            if ctor.eq_ignore_ascii_case("seq") || ctor.eq_ignore_ascii_case("transaction") =>
        {
            args.iter()
                .rev()
                .find(|step| !is_require_term(step))
                .is_some_and(is_case_binding)
        }
        _ => false,
    }
}

fn result_may_be_open(term: &Term) -> bool {
    is_case_binding(term) || is_open_core_term(term)
}

fn is_open_core_term(term: &Term) -> bool {
    match term {
        Term::Record(_) | Term::Field { .. } | Term::Set(_) => true,
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => {
            if ctor.eq_ignore_ascii_case("seq") || ctor.eq_ignore_ascii_case("transaction") {
                args.iter().all(|step| {
                    is_require_term(step) || is_open_core_term(step) || is_case_binding(step)
                })
            } else {
                is_residual_core_op(ctor) || !ctor.is_empty()
            }
        }
        _ => false,
    }
}

fn is_residual_core_op(name: &str) -> bool {
    matches!(
        name,
        "duty_step"
            | "duty_status"
            | "require_authority"
            | "due"
            | "field"
            | "Field"
            | "and"
            | "or"
            | "forall"
            | "every"
            | "is"
            | "has_llc_designator"
            | "nonempty"
            | "complete"
            | "not_blank"
            | "current_fee"
            | "Observe"
            | "Determine"
            | "Choose"
            | "Interpret"
            | "ResolveNormConflict"
            | "SelectApplicableLaw"
    ) || name.eq_ignore_ascii_case("duty_step")
        || name.eq_ignore_ascii_case("duty_status")
        || name.eq_ignore_ascii_case("require_authority")
}

fn params_as_types(params: &[(String, String)]) -> Vec<(String, Type)> {
    params
        .iter()
        .map(|(name, ty)| (name.clone(), parse_type_name(ty)))
        .collect()
}

fn source_typed_params(src: &str) -> Vec<(String, Type)> {
    let Some(start) = src.find('(') else {
        return Vec::new();
    };
    let mut depth = 0i32;
    let mut end = None;
    for (i, b) in src.as_bytes()[start..].iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end) = end else {
        return Vec::new();
    };
    let inner = src[start + 1..end].trim();
    if inner.is_empty() {
        return Vec::new();
    }
    inner
        .split(',')
        .filter_map(|part| {
            let (name, ty) = part.split_once(':')?;
            let name = name.trim();
            let ty = ty.trim();
            if name.is_empty() || ty.is_empty() {
                None
            } else {
                Some((name.to_owned(), parse_type_name(ty)))
            }
        })
        .collect()
}

fn effect_op_name(src: &str) -> String {
    let inner = last_brace_inner(src).unwrap_or(src).trim();
    let ident: String = inner
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if ident.is_empty() {
        "request".into()
    } else {
        ident
    }
}

fn prefer_join(left: &Type, right: &Type) -> Type {
    if is_subtype(left, right) {
        return right.clone();
    }
    if is_subtype(right, left) {
        return left.clone();
    }
    match (left, right) {
        (Type::Primitive(PrimitiveType::Money { .. }), _) => left.clone(),
        (_, Type::Primitive(PrimitiveType::Money { .. })) => right.clone(),
        _ => left.clone(),
    }
}

fn types_compatible(expected: &Type, actual: &Type) -> bool {
    expected == actual || is_subtype(actual, expected)
}

fn types_compatible_in(owner: &str, expected: &Type, actual: &Type) -> bool {
    if types_compatible(expected, actual) {
        return true;
    }
    is_tax_calc(owner) && money_decimal_pair(expected, actual)
}

fn is_tax_calc(name: &str) -> bool {
    name == "ordinary_income_tax" || name == "ordinary_income_tax_formula"
}

fn money_decimal_pair(left: &Type, right: &Type) -> bool {
    matches!(
        (left, right),
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

fn collect_query_callees(q: &HirQuery, names: &BTreeSet<&str>) -> BTreeSet<String> {
    let mut callees = BTreeSet::new();
    match &q.body {
        HirQueryBody::Return(term) => collect_term_callees(term, names, &mut callees),
        HirQueryBody::Goal {
            expr,
            office,
            fields,
            ..
        } => {
            if let Some(term) = expr {
                collect_term_callees(term, names, &mut callees);
            }
            if let Some(term) = office {
                collect_term_callees(term, names, &mut callees);
            }
            for term in fields.values() {
                collect_term_callees(term, names, &mut callees);
            }
        }
        HirQueryBody::None => {}
    }
    callees.extend(collect_source_callees(&q.plan, names));
    callees
}

struct InferredEffects {
    functions: BTreeMap<String, BTreeSet<EffectName>>,
    queries: BTreeMap<String, BTreeSet<EffectName>>,
}

impl InferredEffects {
    fn function(&self, name: &str) -> BTreeSet<EffectName> {
        self.functions.get(name).cloned().unwrap_or_default()
    }

    fn query(&self, name: &str) -> BTreeSet<EffectName> {
        self.queries.get(name).cloned().unwrap_or_default()
    }
}

/// Recursively infer effect rows from declarations, recognized Applies, and callees.
///
/// Unknown row idents are kept as [`EffectName::Unresolved`]. This is not a
/// full effect lattice or algebraic-effect worklist.
fn infer_module_effects(
    hir: &HirModule,
    imported_functions: &BTreeMap<String, HirFunction>,
) -> InferredEffects {
    let mut functions = hir.functions.clone();
    for (name, function) in imported_functions {
        functions.entry(name.clone()).or_insert(function.clone());
    }
    let function_names: BTreeSet<&str> = functions.keys().map(String::as_str).collect();
    let query_names: BTreeSet<&str> = hir.queries.keys().map(String::as_str).collect();
    let declared_custom: BTreeSet<&str> = hir.effects.keys().map(String::as_str).collect();

    let mut function_local = BTreeMap::new();
    let mut function_callees = BTreeMap::new();
    for function in functions.values() {
        let mut effects = effect_set_from_names(function_effect_names(function));
        if let Some(body) = &function.body {
            collect_term_effects(body, &declared_custom, &mut effects);
        }
        let mut callees = BTreeSet::new();
        if let Some(body) = &function.body {
            collect_term_callees(body, &function_names, &mut callees);
        }
        callees.extend(collect_source_callees(&function.source, &function_names));
        function_local.insert(function.name.clone(), effects);
        function_callees.insert(function.name.clone(), callees);
    }
    let functions = close_effect_sets(&function_local, &function_callees);

    let mut query_local = BTreeMap::new();
    let mut query_callees = BTreeMap::new();
    for query in hir.queries.values() {
        let mut effects = effect_set_from_names(&query.effects);
        collect_query_term_effects(query, &declared_custom, &mut effects);
        for callee in collect_query_callees(query, &function_names) {
            if let Some(callee_effects) = functions.get(&callee) {
                effects.extend(callee_effects.iter().cloned());
            }
        }
        query_local.insert(query.name.clone(), effects);
        query_callees.insert(
            query.name.clone(),
            collect_query_callees(query, &query_names),
        );
    }
    let queries = close_effect_sets(&query_local, &query_callees);
    InferredEffects { functions, queries }
}

fn close_effect_sets(
    local: &BTreeMap<String, BTreeSet<EffectName>>,
    callees: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, BTreeSet<EffectName>> {
    let mut inferred = local.clone();
    for name in callees.keys() {
        inferred.entry(name.clone()).or_default();
    }
    let names: Vec<String> = inferred.keys().cloned().collect();
    let mut changed = true;
    while changed {
        changed = false;
        for name in &names {
            let extra: BTreeSet<EffectName> = callees
                .get(name)
                .into_iter()
                .flatten()
                .filter_map(|callee| inferred.get(callee))
                .flatten()
                .cloned()
                .collect();
            if let Some(set) = inferred.get_mut(name) {
                let before = set.len();
                set.extend(extra);
                changed |= set.len() != before;
            }
        }
    }
    inferred
}

fn collect_query_term_effects(
    query: &HirQuery,
    declared_custom: &BTreeSet<&str>,
    out: &mut BTreeSet<EffectName>,
) {
    match &query.body {
        HirQueryBody::Return(term) => collect_term_effects(term, declared_custom, out),
        HirQueryBody::Goal {
            expr,
            office,
            fields,
            ..
        } => {
            if let Some(term) = expr {
                collect_term_effects(term, declared_custom, out);
            }
            if let Some(term) = office {
                collect_term_effects(term, declared_custom, out);
            }
            for term in fields.values() {
                collect_term_effects(term, declared_custom, out);
            }
        }
        HirQueryBody::None => {}
    }
}

fn collect_term_effects(
    term: &Term,
    declared_custom: &BTreeSet<&str>,
    out: &mut BTreeSet<EffectName>,
) {
    match term {
        Term::Call { callee, args } => {
            if let Some(effect) = effect_from_apply(callee, declared_custom) {
                out.insert(effect);
            }
            for arg in args {
                collect_term_effects(arg, declared_custom, out);
            }
        }
        Term::Apply { ctor, args } => {
            if let Some(effect) = effect_from_apply(ctor, declared_custom) {
                out.insert(effect);
            }
            for arg in args {
                collect_term_effects(arg, declared_custom, out);
            }
        }
        Term::Binary { left, right, .. } => {
            collect_term_effects(left, declared_custom, out);
            collect_term_effects(right, declared_custom, out);
        }
        Term::If { cond, then, else_ } => {
            collect_term_effects(cond, declared_custom, out);
            collect_term_effects(then, declared_custom, out);
            collect_term_effects(else_, declared_custom, out);
        }
        Term::Field { base, .. } => collect_term_effects(base, declared_custom, out),
        Term::Set(xs) => {
            for x in xs {
                collect_term_effects(x, declared_custom, out);
            }
        }
        Term::Record(fields) => {
            for x in fields.values() {
                collect_term_effects(x, declared_custom, out);
            }
        }
        _ => {}
    }
}

fn effect_from_apply(ctor: &str, declared_custom: &BTreeSet<&str>) -> Option<EffectName> {
    match ctor {
        "determined" | "operative" => Some(EffectName::Determine),
        "observed" => Some(EffectName::Observe),
        "Observe" => Some(EffectName::Observe),
        "Determine" => Some(EffectName::Determine),
        "Choose" => Some(EffectName::Choose),
        "Interpret" => Some(EffectName::Interpret),
        "ResolveNormConflict" => Some(EffectName::ResolveNormConflict),
        "SelectApplicableLaw" => Some(EffectName::SelectApplicableLaw),
        other if declared_custom.contains(other) => Some(EffectName::Unresolved(other.to_owned())),
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
        .map(|name| name.as_ref().trim().to_owned())
        .filter(|name| !name.is_empty())
        .map(|name| EffectName::from_ident(&name))
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
        let package_name = package_name_from_import(&import.name);
        let package_artifacts: Vec<&ManifestArtifact> = manifest
            .artifacts
            .iter()
            .filter(|artifact| {
                artifact_path_is_package(&artifact.path)
                    && package_dir_from_artifact_path(&artifact.path).as_deref()
                        == Some(package_name.as_str())
            })
            .collect();
        if !package_artifacts.is_empty() {
            let matched = package_artifacts.iter().any(|artifact| {
                package_artifact_satisfies(artifact, import)
                    && permits_import(authenticate_artifact(artifact, source_root))
            });
            if !matched {
                diagnostics.push(unresolved_import(import));
            }
            continue;
        }
        if !import.digest_required {
            continue;
        }
        let matched = manifest.artifacts.iter().any(|artifact| {
            artifact_matches_import(artifact, import)
                && permits_import(authenticate_artifact(artifact, source_root))
        });
        if !matched {
            diagnostics.push(unresolved_import(import));
        }
    }
}

fn unresolved_import(import: &HirImport) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::E200,
        format!(
            "import `{}` requires a digest in the authenticated source manifest",
            import.name
        ),
    )
}

fn package_artifact_satisfies(artifact: &ManifestArtifact, import: &HirImport) -> bool {
    if let Some(ver) = import
        .version
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let locked = artifact.effective.trim();
        if !locked.is_empty() && locked != ver {
            return false;
        }
    }
    if let Some(digest) = import
        .digest
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        && !artifact.digest.trim().eq_ignore_ascii_case(digest)
    {
        return false;
    }
    package_path_matches_import(&artifact.path, &import.name)
}

fn artifact_matches_import(artifact: &ManifestArtifact, import: &HirImport) -> bool {
    if artifact_path_is_package(&artifact.path) {
        return package_artifact_satisfies(artifact, import);
    }
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

#[derive(Default)]
struct LinkedImports {
    functions: BTreeMap<String, HirFunction>,
    modules: Vec<CoreModule>,
}

/// Parse and check authenticated package bytes on path compile.
///
/// Nested package imports are injected from `source_root/packages` and
/// linked with a depth/cycle guard. `source_root = None` (mill /
/// `check_source`) never enters here with a root, so it does not read
/// `packages/`.
fn link_authenticated_packages(
    hir: &HirModule,
    manifest: &SourceManifest,
    source_root: Option<&Path>,
    seen: &mut BTreeSet<String>,
    depth: usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> LinkedImports {
    let mut linked = LinkedImports::default();
    let Some(root) = source_root else {
        return linked;
    };
    let mut finished = BTreeSet::new();
    for import in &hir.imports {
        let Some(artifact) = authenticated_package_artifact(import, manifest, Some(root)) else {
            continue;
        };
        if !finished.insert(artifact.path.clone()) {
            continue;
        }
        if !seen.insert(artifact.path.clone()) {
            diagnostics.push(package_cycle(import, &artifact.path));
            continue;
        }
        let compiled = (|| {
            if depth >= PACKAGE_LINK_DEPTH_CAP {
                return Err(vec![package_depth(import)]);
            }
            let Some(bytes) = read_artifact_bytes(root, &artifact.path) else {
                return Ok(None);
            };
            if !permits_import(authenticate_digest(artifact.digest.trim(), Some(&bytes))) {
                return Ok(None);
            }
            compile_package_bytes(&bytes, Some(root), import, seen, depth + 1).map(Some)
        })();
        seen.remove(&artifact.path);
        match compiled {
            Ok(Some((_pkg_hir, pkg_core, nested))) => {
                merge_exported_functions(hir, nested, &mut linked);
                linked.modules.push(pkg_core);
            }
            Ok(None) => {}
            Err(err) => diagnostics.extend(err),
        }
    }
    linked
}

fn authenticated_package_artifact<'a>(
    import: &HirImport,
    manifest: &'a SourceManifest,
    source_root: Option<&Path>,
) -> Option<&'a ManifestArtifact> {
    let package_name = package_name_from_import(&import.name);
    manifest.artifacts.iter().find(|artifact| {
        artifact_path_is_package(&artifact.path)
            && package_dir_from_artifact_path(&artifact.path).as_deref()
                == Some(package_name.as_str())
            && package_artifact_satisfies(artifact, import)
            && permits_import(authenticate_artifact(artifact, source_root))
    })
}

fn compile_package_bytes(
    bytes: &[u8],
    source_root: Option<&Path>,
    import: &HirImport,
    seen: &mut BTreeSet<String>,
    depth: usize,
) -> Result<(HirModule, CoreModule, LinkedImports), Vec<Diagnostic>> {
    if depth > PACKAGE_LINK_DEPTH_CAP {
        return Err(vec![package_depth(import)]);
    }
    let source = std::str::from_utf8(bytes).map_err(|_| {
        vec![Diagnostic::new(
            DiagnosticCode::E100,
            format!("imported package `{}` is not utf-8", import.name),
        )]
    })?;
    let parsed = parse_file(source);
    if parsed.has_errors() {
        return Err(parsed.diagnostics);
    }
    let mut pkg_manifest = SourceManifest::default();
    if let Some(root) = source_root {
        inject_package_artifacts(&mut pkg_manifest, root, source)?;
    }
    let pkg_hir = fidryn_hir::elaborate(&parsed, &pkg_manifest)?;
    let (pkg_core, nested) = check_module(
        &pkg_hir,
        &pkg_manifest,
        source_root,
        source_root.is_some(),
        seen,
        depth,
    )?;
    let exported = export_package_functions(&pkg_hir, nested);
    Ok((pkg_hir, pkg_core, exported))
}

fn package_cycle(import: &HirImport, path: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::E200,
        format!("import `{}` forms a package cycle at `{path}`", import.name),
    )
}

fn package_depth(import: &HirImport) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::E200,
        format!(
            "import `{}` exceeds nested package depth {PACKAGE_LINK_DEPTH_CAP}",
            import.name
        ),
    )
}

/// Inject `source_root/packages/<name>` locks for imports in `src`.
///
/// Only path compile of an already-authenticated package calls this.
/// Missing nested lock/digest is E200. Mill / `check_source` never
/// pass a `source_root` into package compile.
fn inject_package_artifacts(
    manifest: &mut SourceManifest,
    source_root: &Path,
    src: &str,
) -> Result<(), Vec<Diagnostic>> {
    let packages_dir = packages_root(source_root);
    if !packages_dir.is_dir() {
        return Ok(());
    }
    let wanted = imported_package_names(src);
    if wanted.is_empty() {
        return Ok(());
    }
    let mut errors = Vec::new();
    for name in wanted {
        match load_nested_package_artifact(source_root, &name) {
            Ok(Some(artifact)) => {
                if !manifest
                    .artifacts
                    .iter()
                    .any(|existing| existing.path == artifact.path)
                {
                    manifest.artifacts.push(artifact);
                }
            }
            Ok(None) => errors.push(Diagnostic::new(
                DiagnosticCode::E200,
                format!("imported package `{name}` is missing a nested digest"),
            )),
            Err(ds) => errors.extend(ds),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn imported_package_names(src: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let parsed = parse_file(src);
    let Some(module) = parsed.module() else {
        return names;
    };
    for item in &module.items {
        if let Item::Import(decl) = item {
            let raw = decl.name.as_deref().unwrap_or("");
            let name = package_name_from_import(raw);
            if is_safe_package_name(&name) {
                names.insert(name);
            }
        }
    }
    names
}

fn load_nested_package_artifact(
    source_root: &Path,
    name: &str,
) -> Result<Option<ManifestArtifact>, Vec<Diagnostic>> {
    if !is_safe_package_name(name) {
        return Ok(None);
    }
    let dir = packages_root(source_root).join(name);
    if !dir.is_dir() {
        return Ok(None);
    }
    let lock_path = dir.join("manifest.json");
    let text = match fs::read_to_string(&lock_path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Err(vec![Diagnostic::new(
                DiagnosticCode::E200,
                format!("imported package `{name}` is missing a nested digest"),
            )]);
        }
        Err(err) => {
            return Err(vec![Diagnostic::new(
                DiagnosticCode::E200,
                format!("cannot read nested package `{name}` lock: {err}"),
            )]);
        }
    };
    let lock: PackageLock = serde_json::from_str(&text).map_err(|err| {
        vec![Diagnostic::new(
            DiagnosticCode::E200,
            format!(
                "malformed nested package lock {}: {err}",
                lock_path.display()
            ),
        )]
    })?;
    if lock.digest.trim().is_empty() {
        return Err(vec![Diagnostic::new(
            DiagnosticCode::E200,
            format!("imported package `{name}` is missing a nested digest"),
        )]);
    }
    if !lock.name.is_empty() && !lock.name.eq_ignore_ascii_case(name) {
        return Err(vec![Diagnostic::new(
            DiagnosticCode::E200,
            format!(
                "package directory `{name}` does not match lock name `{}`",
                lock.name
            ),
        )]);
    }
    let Some(module_path) = unique_package_module(&dir) else {
        return Err(vec![Diagnostic::new(
            DiagnosticCode::E200,
            format!("package `{name}` does not contain a unique .fr module"),
        )]);
    };
    let file_name = module_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            vec![Diagnostic::new(
                DiagnosticCode::E200,
                format!("package `{name}` module path is not utf-8"),
            )]
        })?;
    let rel = format!("packages/{name}/{file_name}");
    Ok(Some(lock.module_artifact(rel)))
}

fn unique_package_module(dir: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("fr") {
            found.push(path);
        }
    }
    if found.len() == 1 { found.pop() } else { None }
}

fn export_package_functions(pkg: &HirModule, nested: LinkedImports) -> LinkedImports {
    let mut exported = nested;
    for (name, function) in &pkg.functions {
        exported
            .functions
            .entry(name.clone())
            .or_insert(function.clone());
    }
    for (name, query) in &pkg.queries {
        exported
            .functions
            .entry(name.clone())
            .or_insert(hir_function_from_query(query));
    }
    exported
}

fn merge_exported_functions(
    importer: &HirModule,
    nested: LinkedImports,
    linked: &mut LinkedImports,
) {
    for (name, function) in nested.functions {
        if importer.functions.contains_key(&name) || linked.functions.contains_key(&name) {
            continue;
        }
        linked.functions.insert(name, function);
    }
}

fn hir_function_from_query(query: &HirQuery) -> HirFunction {
    HirFunction {
        name: query.name.clone(),
        is_calc: false,
        fuel: None,
        result_type: query.result_type.clone(),
        source: query.plan.clone(),
        params: query.params.clone(),
        body: query_result_term(query).cloned(),
    }
}

fn merge_linked_modules(
    into: &mut CoreModule,
    packages: &[CoreModule],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut decl_names: BTreeSet<String> = into
        .declarations
        .iter()
        .filter_map(core_decl_name)
        .map(str::to_owned)
        .collect();
    let mut query_names: BTreeSet<String> = into.queries.iter().map(|q| q.name.clone()).collect();
    for package in packages {
        for decl in &package.declarations {
            let Some(name) = core_decl_name(decl) else {
                into.declarations.push(decl.clone());
                continue;
            };
            if !decl_names.insert(name.to_owned()) {
                diagnostics.push(imported_name_collision(name, &package.name));
                continue;
            }
            into.declarations.push(decl.clone());
        }
        for query in &package.queries {
            if !query_names.insert(query.name.clone()) {
                diagnostics.push(imported_name_collision(&query.name, &package.name));
                continue;
            }
            into.queries.push(query.clone());
        }
    }
}

fn core_decl_name(decl: &CoreDecl) -> Option<&str> {
    match decl {
        CoreDecl::Source(d) => Some(d.name.as_str()),
        CoreDecl::Entity(d) => Some(d.name.as_str()),
        CoreDecl::RecordType(d) => Some(d.name.as_str()),
        CoreDecl::Office(d) => Some(d.name.as_str()),
        CoreDecl::Proposition(d) => Some(d.name.as_str()),
        CoreDecl::Observation(d) => Some(d.name.as_str()),
        CoreDecl::Fact(d) => Some(d.relation.as_str()),
        CoreDecl::Function(d) => Some(d.name.as_str()),
        CoreDecl::EffectDecl(d) => Some(d.name.as_str()),
        CoreDecl::Rule(d) => Some(d.name.as_str()),
        CoreDecl::Position(_) => None,
        CoreDecl::Power(d) => Some(d.name.as_str()),
        CoreDecl::Duty(d) => Some(d.name.as_str()),
        CoreDecl::Judgment(d) => Some(d.name.as_str()),
        CoreDecl::Decision(d) => Some(d.name.as_str()),
        CoreDecl::LegalAct(d) => Some(d.name.as_str()),
        CoreDecl::InterpretationFamily(d) => Some(d.name.as_str()),
        CoreDecl::ConflictDoctrine(d) => Some(d.name.as_str()),
        CoreDecl::Clause(d) => Some(d.name.as_str()),
    }
}

fn imported_name_collision(name: &str, from: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::E200,
        format!("imported `{name}` from `{from}` collides with an existing declaration"),
    )
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

fn lower(
    hir: &HirModule,
    manifest: &SourceManifest,
    imported_functions: &BTreeMap<String, HirFunction>,
) -> CoreModule {
    let inferred = infer_module_effects(hir, imported_functions);
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
            effects: inferred.function(&f.name),
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
                name: effect_op_name(&e.source),
                params: source_typed_params(&e.source),
                result: parse_type_name(&e.result_type),
            }],
            meta: meta(&e.name),
        }));
    }
    for rule in &hir.rules {
        let rule_meta = meta(&rule.name);
        let fallback = lower_rule_effects(&rule.name, "otherwise", &rule.fallback, &rule_meta);
        declarations.push(CoreDecl::Rule(CoreRule {
            id: NodeId::of(rule.name.as_bytes()),
            name: rule.name.clone(),
            kind: match rule.kind.as_str() {
                "constitutive" => RuleKind::Constitutive,
                "derive" => RuleKind::Derive,
                _ => RuleKind::Prescriptive,
            },
            binders: rule.binders.clone(),
            selection: None,
            guard: rule.guard.clone().unwrap_or(Guard::Satisfied),
            consequences: lower_rule_effects(&rule.name, "then", &rule.consequences, &rule_meta),
            fallback: (!fallback.is_empty()).then_some(fallback),
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
        .map(|q| lower_query(q, hir, jid, inferred.query(&q.name)))
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

fn lower_rule_effects(
    rule_name: &str,
    tag: &str,
    items: &[(String, PropTerm)],
    meta: &NodeMeta,
) -> Vec<CoreEffect> {
    items
        .iter()
        .enumerate()
        .map(|(i, (op, prop))| CoreEffect {
            id: EffectId::of(
                if tag == "then" {
                    format!("{rule_name}:{op}:{i}")
                } else {
                    format!("{rule_name}:{tag}:{op}:{i}")
                }
                .as_bytes(),
            ),
            consequence: consequence_from_op(op, prop),
            meta: meta.clone(),
        })
        .collect()
}

fn consequence_from_op(op: &str, prop: &PropTerm) -> Consequence {
    match op {
        "establish" | "constitute" => Consequence::Establish(prop.clone()),
        "terminate" => Consequence::Terminate(prop.clone()),
        "suspend" => Consequence::Suspend(prop.clone()),
        _ => Consequence::Derive(prop.clone()),
    }
}

fn lower_query(
    q: &HirQuery,
    hir: &HirModule,
    jid: JurisdictionId,
    effects: BTreeSet<EffectName>,
) -> CoreQuery {
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

    fn core_rule<'a>(module: &'a CoreModule, name: &str) -> &'a CoreRule {
        module
            .declarations
            .iter()
            .find_map(|decl| match decl {
                CoreDecl::Rule(rule) if rule.name == name => Some(rule),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing rule {name}"))
    }

    #[test]
    fn lowers_rule_guard_fallback_and_binders() {
        let module = check_src(
            r#"
module Review version "0.1.0" {
    proposition P()
    proposition Q()
    rule R(x: Person) : derive {
        when false
        require true
        then derive P()
        otherwise derive Q()
    }
    query q() -> Bool { return true }
}
"#,
        )
        .expect("rule program");
        let rule = core_rule(&module, "R");
        assert_eq!(rule.binders, vec!["x".to_owned()]);
        assert_eq!(rule.guard, Guard::Not(Box::new(Guard::Satisfied)));
        assert_eq!(rule.consequences.len(), 1);
        match &rule.consequences[0].consequence {
            Consequence::Derive(prop) => assert_eq!(prop.predicate, "P"),
            other => panic!("{other:?}"),
        }
        let fallback = rule.fallback.as_ref().expect("otherwise fallback");
        assert_eq!(fallback.len(), 1);
        match &fallback[0].consequence {
            Consequence::Derive(prop) => assert_eq!(prop.predicate, "Q"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn lowers_true_guard_without_otherwise() {
        let module = check_src(
            r#"
module Review version "0.1.0" {
    proposition P()
    rule R : derive { when true then derive P() }
    query q() -> Bool { return true }
}
"#,
        )
        .expect("rule program");
        let rule = core_rule(&module, "R");
        assert_eq!(rule.guard, Guard::Satisfied);
        assert!(rule.fallback.is_none());
        assert!(rule.binders.is_empty());
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
        match check_src(src) {
            Ok(_) => {}
            Err(err) => {
                assert!(
                    err.iter().all(|d| d.code != DiagnosticCode::E530),
                    "count calling counter is not self-recursion: {err:?}"
                );
                assert!(
                    err.iter().any(|d| d.code == DiagnosticCode::E210),
                    "unresolved `counter` must not fail open: {err:?}"
                );
            }
        }
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
    fn automatic_query_calling_custom_effect_function_is_e420() {
        let src = r#"
module Examples.AutoCustom version "0.1.0" {
    effect DocketLookup {
        request(docket_id: String) -> Bool
    }
    fn lookup() -> Bool ! {DocketLookup} { true }
    query automatic q() -> Bool { return lookup() }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E420),
            "{err:?}"
        );
    }

    #[test]
    fn custom_effect_names_are_kept_on_core_function_and_query() {
        let src = r#"
module Examples.CustomEff version "0.1.0" {
    effect DocketLookup {
        request(docket_id: String) -> Bool
    }
    fn lookup() -> Bool ! {DocketLookup} { true }
    query declared() -> Bool ! {DocketLookup} {
        goal Evaluate { true }
    }
    query inferred() -> Bool {
        goal Evaluate { lookup() }
    }
}
"#;
        let module = check_src(src).expect("non-automatic custom effects should check");
        let function = core_function(&module, "lookup");
        assert!(
            function
                .effects
                .contains(&EffectName::from_ident("DocketLookup")),
            "{:?}",
            function.effects
        );
        let declared = module.query("declared").expect("declared query");
        assert!(
            declared
                .effects
                .contains(&EffectName::from_ident("DocketLookup")),
            "{:?}",
            declared.effects
        );
        let inferred = module.query("inferred").expect("inferred query");
        assert!(
            inferred
                .effects
                .contains(&EffectName::from_ident("DocketLookup")),
            "{:?}",
            inferred.effects
        );
    }

    #[test]
    fn automatic_query_with_inferred_determine_apply_is_e420() {
        let src = r#"
module Examples.AutoDetBody version "0.1.0" {
    proposition P(x: LegalPerson)
    entity A : LegalPerson
    fn f() -> Bool { determined(P(A)) }
    query automatic q() -> Bool { return f() }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E420),
            "{err:?}"
        );
    }

    fn core_function<'a>(module: &'a CoreModule, name: &str) -> &'a CoreFunction {
        module
            .declarations
            .iter()
            .find_map(|d| match d {
                CoreDecl::Function(function) if function.name == name => Some(function),
                _ => None,
            })
            .unwrap_or_else(|| panic!("function `{name}`"))
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
    fn transaction_last_step_type_is_query_result() {
        let src = r#"
module Examples.TxInt version "0.1.0" {
    query q() -> Int {
        transaction {
            require true;
            7
        }
    }
}
"#;
        let module = check_src(src).expect("transaction last step Int should check");
        let q = module.query("q").expect("q");
        match &q.plan {
            QueryPlan::Evaluate(Term::Apply { ctor, args }) if ctor == "transaction" => {
                assert_eq!(args.len(), 2, "{args:?}");
                assert_eq!(args[1], Term::Int(7));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn transaction_does_not_hide_a_wrong_result_type() {
        let src = r#"
module Review version "0.1.0" {
    query q() -> Bool {
        transaction {
            require true;
            7
        }
    }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E210),
            "{err:?}"
        );
    }

    #[test]
    fn transaction_atomic_program_keeps_both_steps() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/programs/transaction-atomic.fr");
        let src = std::fs::read_to_string(&path).expect("transaction-atomic.fr");
        let module = check_src(&src).expect("transaction-atomic.fr should check");
        let q = module.query("q").expect("q");
        match &q.plan {
            QueryPlan::Evaluate(Term::Apply { ctor, args }) if ctor == "transaction" => {
                assert_eq!(args.len(), 2, "{args:?}");
                assert!(
                    !args
                        .iter()
                        .any(|arg| matches!(arg, Term::Apply { ctor, .. } if ctor == "seq")),
                    "{args:?}"
                );
            }
            other => panic!("{other:?}"),
        }
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

    #[test]
    fn declared_function_parameters_constrain_call_arguments() {
        let src = r#"
module Review version "0.1.0" {
    fn identity(x: Int) -> Int { x }
    query q() -> Int { return identity("wrong type") }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E210),
            "{err:?}"
        );
    }

    #[test]
    fn incompatible_branches_are_not_an_inference_escape_hatch() {
        let src = r#"
module Review version "0.1.0" {
    query q() -> Int { if false { 1 } else { "wrong" } }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E210),
            "{err:?}"
        );
    }

    #[test]
    fn determined_requires_a_proposition() {
        let src = r#"
module Review version "0.1.0" {
    query q() -> Bool { return determined(7) }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(
            err.iter()
                .any(|d| d.code == DiagnosticCode::E210 || d.code == DiagnosticCode::E310),
            "{err:?}"
        );
    }

    #[test]
    fn money_and_decimal_are_not_globally_compatible() {
        let src = r#"
module Review version "0.1.0" {
    fn not_tax() -> Money<USD> { 1.25 }
    query q() -> Money<USD> { return not_tax() }
}
"#;
        let err = check_src(src).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E210),
            "{err:?}"
        );
    }

    #[test]
    fn custom_effect_constructor_arguments_are_preserved() {
        let src = r#"
module Review version "0.1.0" {
    effect DocketLookup {
        request(docket_id: String) -> Bool
    }
    query a() -> Bool { DocketLookup("A") }
    query b() -> Bool { DocketLookup("B") }
}
"#;
        let module = check_src(src).expect("custom effect applications should check");
        let effect = module
            .declarations
            .iter()
            .find_map(|decl| match decl {
                CoreDecl::EffectDecl(effect) if effect.name == "DocketLookup" => Some(effect),
                _ => None,
            })
            .expect("DocketLookup");
        assert_eq!(effect.operations.len(), 1);
        assert_eq!(effect.operations[0].name, "request");
        assert_eq!(
            effect.operations[0].params,
            vec![(
                "docket_id".to_owned(),
                Type::Primitive(PrimitiveType::String)
            )]
        );
        let a = module.query("a").expect("a").plan.clone();
        let b = module.query("b").expect("b").plan.clone();
        assert_ne!(
            a, b,
            "DocketLookup(\"A\") must not equal DocketLookup(\"B\")"
        );
        match (&a, &b) {
            (
                QueryPlan::Evaluate(
                    Term::Apply {
                        ctor: ctor_a,
                        args: args_a,
                    }
                    | Term::Call {
                        callee: ctor_a,
                        args: args_a,
                    },
                ),
                QueryPlan::Evaluate(
                    Term::Apply {
                        ctor: ctor_b,
                        args: args_b,
                    }
                    | Term::Call {
                        callee: ctor_b,
                        args: args_b,
                    },
                ),
            ) => {
                assert_eq!(ctor_a, "DocketLookup");
                assert_eq!(ctor_b, "DocketLookup");
                assert_eq!(args_a, &vec![Term::String("A".into())]);
                assert_eq!(args_b, &vec![Term::String("B".into())]);
            }
            other => panic!("{other:?}"),
        }
    }

    fn std_core_import_src() -> &'static str {
        r#"
module Examples.UseStd version "0.1.0" {
    import Std.Core version "0.1.0"
    query ok() -> Bool {
        goal Evaluate { true }
    }
}
"#
    }

    fn std_core_call_src() -> &'static str {
        r#"
module Examples.UseStd version "0.1.0" {
    import Std.Core version "0.1.0"
    query q() -> Bool { return always_true() }
}
"#
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn std_core_package_bytes() -> Vec<u8> {
        fs::read(
            workspace_root()
                .join("packages")
                .join("std")
                .join("core.fr"),
        )
        .expect("core.fr")
    }

    fn install_workspace_packages(root: &Path) {
        for name in ["logic", "std"] {
            let src = workspace_root().join("packages").join(name);
            let dest = root.join("packages").join(name);
            fs::create_dir_all(&dest).expect("package dir");
            for entry in fs::read_dir(&src).expect("read package") {
                let entry = entry.expect("entry");
                let path = entry.path();
                if path.is_file() {
                    fs::copy(&path, dest.join(entry.file_name())).expect("copy package file");
                }
            }
        }
    }

    fn write_locked_package(
        root: &Path,
        name: &str,
        file: &str,
        source: &[u8],
        version: &str,
    ) -> String {
        let dir = root.join("packages").join(name);
        fs::create_dir_all(&dir).expect("package dir");
        fs::write(dir.join(file), source).expect("write module");
        let digest = encode_hex(blake3::hash(source).as_bytes());
        let lock = serde_json::json!({
            "schema": "fidryn.package-lock/v0.1",
            "name": name,
            "version": version,
            "digest": digest,
        });
        fs::write(dir.join("manifest.json"), lock.to_string()).expect("lock");
        digest
    }

    fn logic_true_src() -> &'static [u8] {
        br#"module Logic.True version "0.1.0" {
    fn always_true() -> Bool { true }
}
"#
    }

    fn std_core_reexport_src() -> &'static [u8] {
        br#"module Std.Core version "0.1.0" {
    import Logic.True version "0.1.0"
    query always_true() -> Bool {
        goal Evaluate { true }
    }
}
"#
    }

    fn has_core_function(module: &CoreModule, name: &str) -> bool {
        module
            .declarations
            .iter()
            .any(|d| matches!(d, CoreDecl::Function(function) if function.name == name))
    }

    fn check_std_core_call(
        manifest: &SourceManifest,
        source_root: Option<&Path>,
    ) -> Result<CoreModule, Vec<Diagnostic>> {
        let parsed = parse_file(std_core_call_src());
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        check_with_sources(&hir, manifest, source_root)
    }

    fn package_artifact_manifest(path: &str, digest: &str, version: &str) -> SourceManifest {
        SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: String::new(),
            jurisdiction: String::new(),
            artifacts: vec![ManifestArtifact {
                path: path.into(),
                digest: digest.into(),
                kind: "package_module".into(),
                effective: version.into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        }
    }

    fn check_std_core_import(
        manifest: &SourceManifest,
        source_root: Option<&Path>,
    ) -> Result<CoreModule, Vec<Diagnostic>> {
        let parsed = parse_file(std_core_import_src());
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        check_with_sources(&hir, manifest, source_root)
    }

    #[test]
    fn matching_package_digest_authenticates_std_core_import() {
        let bytes = br#"module Std.Core version "0.1.0" {
    query always_true() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let digest = encode_hex(blake3::hash(bytes).as_bytes());
        let root = TempRoot::new();
        fs::create_dir_all(root.0.join("packages").join("std")).expect("packages/std");
        root.write("packages/std/core.fr", bytes);
        let manifest = package_artifact_manifest("packages/std/core.fr", &digest, "0.1.0");
        check_std_core_import(&manifest, Some(&root.0))
            .expect("matching package digest authenticates");
        assert_eq!(
            source_integrity(&manifest, Some(&root.0)),
            TrustProfile::ByteVerified
        );
        assert_eq!(
            authenticate_artifact(&manifest.artifacts[0], Some(&root.0)),
            TrustProfile::ByteVerified
        );
    }

    #[test]
    fn mismatched_package_digest_is_e200() {
        let bytes = br#"module Std.Core version "0.1.0" {
    query always_true() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let wrong = encode_hex(blake3::hash(b"tampered-package-bytes").as_bytes());
        let root = TempRoot::new();
        fs::create_dir_all(root.0.join("packages").join("std")).expect("packages/std");
        root.write("packages/std/core.fr", bytes);
        let manifest = package_artifact_manifest("packages/std/core.fr", &wrong, "0.1.0");
        let err = check_std_core_import(&manifest, Some(&root.0)).unwrap_err();
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
    fn package_hex_without_source_root_is_e200() {
        let bytes = br#"module Std.Core version "0.1.0" {
    query always_true() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let digest = encode_hex(blake3::hash(bytes).as_bytes());
        let manifest = package_artifact_manifest("packages/std/core.fr", &digest, "0.1.0");
        let err = check_std_core_import(&manifest, None).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
        assert_ne!(
            source_integrity(&manifest, None),
            TrustProfile::ByteVerified
        );
        assert_eq!(
            source_integrity(&manifest, None),
            TrustProfile::Unauthenticated
        );
    }

    #[test]
    fn matching_package_links_always_true_so_query_type_checks() {
        let bytes = std_core_package_bytes();
        let digest = encode_hex(blake3::hash(&bytes).as_bytes());
        let root = TempRoot::new();
        install_workspace_packages(&root.0);
        let manifest = package_artifact_manifest("packages/std/core.fr", &digest, "0.1.0");
        let module =
            check_std_core_call(&manifest, Some(&root.0)).expect("path compile links always_true");
        assert!(
            has_core_function(&module, "always_true"),
            "linked CoreModule must contain always_true: {module:?}"
        );
        let q = module.query("q").unwrap_or_else(|| {
            panic!(
                "missing query q; queries={:?}",
                module.queries.iter().map(|q| &q.name).collect::<Vec<_>>()
            )
        });
        assert_eq!(q.result_type, Type::bool());
    }

    #[test]
    fn mismatched_package_digest_does_not_link_always_true() {
        let bytes = std_core_package_bytes();
        let wrong = encode_hex(blake3::hash(b"tampered-package-bytes").as_bytes());
        let root = TempRoot::new();
        fs::create_dir_all(root.0.join("packages").join("std")).expect("packages/std");
        root.write("packages/std/core.fr", &bytes);
        let manifest = package_artifact_manifest("packages/std/core.fr", &wrong, "0.1.0");
        let err = check_std_core_call(&manifest, Some(&root.0)).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
    }

    #[test]
    fn in_memory_compile_does_not_load_packages_from_cwd() {
        let bytes = std_core_package_bytes();
        let digest = encode_hex(blake3::hash(&bytes).as_bytes());
        let manifest = package_artifact_manifest("packages/std/core.fr", &digest, "0.1.0");
        let err = check_std_core_call(&manifest, None).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );

        let parsed = parse_file(std_core_call_src());
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        match check(&hir, &SourceManifest::default()) {
            Ok(module) => assert!(
                !has_core_function(&module, "always_true"),
                "check() must not mill packages/ from cwd: {module:?}"
            ),
            Err(err) => {
                assert!(
                    err.iter().any(|d| d.code == DiagnosticCode::E210),
                    "unlinked always_true must fail closed without milling: {err:?}"
                );
                assert!(
                    err.iter().all(|d| d.code != DiagnosticCode::E200),
                    "digest-free import must not mill packages/: {err:?}"
                );
            }
        }
    }

    #[test]
    fn imported_always_true_collides_with_importer_function() {
        let src = r#"
module Examples.UseStd version "0.1.0" {
    import Std.Core version "0.1.0"
    fn always_true() -> Bool { false }
    query q() -> Bool { return always_true() }
}
"#;
        let bytes = std_core_package_bytes();
        let digest = encode_hex(blake3::hash(&bytes).as_bytes());
        let root = TempRoot::new();
        install_workspace_packages(&root.0);
        let manifest = package_artifact_manifest("packages/std/core.fr", &digest, "0.1.0");
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let err = check_with_sources(&hir, &manifest, Some(&root.0)).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
    }

    #[test]
    fn nested_package_authenticates_and_links() {
        let root = TempRoot::new();
        write_locked_package(&root.0, "logic", "true.fr", logic_true_src(), "0.1.0");
        let std_digest =
            write_locked_package(&root.0, "std", "core.fr", std_core_reexport_src(), "0.1.0");
        let manifest = package_artifact_manifest("packages/std/core.fr", &std_digest, "0.1.0");
        let module = check_std_core_call(&manifest, Some(&root.0))
            .expect("nested Logic.True authenticates and links always_true");
        assert!(
            has_core_function(&module, "always_true"),
            "nested always_true must be merged into the importer: {module:?}"
        );
        assert_eq!(
            source_integrity(&manifest, Some(&root.0)),
            TrustProfile::ByteVerified
        );
    }

    #[test]
    fn nested_package_cycle_is_e200() {
        let ping = br#"module Ping.Mod version "0.1.0" {
    import Pong.Mod version "0.1.0"
    fn ping() -> Bool { true }
}
"#;
        let pong = br#"module Pong.Mod version "0.1.0" {
    import Ping.Mod version "0.1.0"
    fn pong() -> Bool { true }
}
"#;
        let root = TempRoot::new();
        let ping_digest = write_locked_package(&root.0, "ping", "mod.fr", ping, "0.1.0");
        write_locked_package(&root.0, "pong", "mod.fr", pong, "0.1.0");
        let src = r#"
module Examples.Cycle version "0.1.0" {
    import Ping.Mod version "0.1.0"
    query q() -> Bool { return true }
}
"#;
        let manifest = package_artifact_manifest("packages/ping/mod.fr", &ping_digest, "0.1.0");
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let err = check_with_sources(&hir, &manifest, Some(&root.0)).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
    }

    #[test]
    fn missing_nested_digest_is_e200() {
        let root = TempRoot::new();
        let logic_dir = root.0.join("packages").join("logic");
        fs::create_dir_all(&logic_dir).expect("packages/logic");
        fs::write(logic_dir.join("true.fr"), logic_true_src()).expect("true.fr");
        let std_digest =
            write_locked_package(&root.0, "std", "core.fr", std_core_reexport_src(), "0.1.0");
        let manifest = package_artifact_manifest("packages/std/core.fr", &std_digest, "0.1.0");
        let err = check_std_core_call(&manifest, Some(&root.0)).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
    }

    #[test]
    fn nested_package_wrong_digest_is_e200() {
        let root = TempRoot::new();
        write_locked_package(&root.0, "logic", "true.fr", logic_true_src(), "0.1.0");
        let lock_path = root.0.join("packages").join("logic").join("manifest.json");
        let mut lock: fidryn_core::PackageLock =
            serde_json::from_str(&fs::read_to_string(&lock_path).expect("lock")).expect("parse");
        lock.digest = encode_hex(blake3::hash(b"tampered-nested-bytes").as_bytes());
        fs::write(&lock_path, serde_json::to_string(&lock).expect("lock json"))
            .expect("write lock");
        let std_digest =
            write_locked_package(&root.0, "std", "core.fr", std_core_reexport_src(), "0.1.0");
        let manifest = package_artifact_manifest("packages/std/core.fr", &std_digest, "0.1.0");
        let err = check_std_core_call(&manifest, Some(&root.0)).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
    }

    #[test]
    fn check_source_does_not_read_nested_packages() {
        let parsed = parse_file(std_core_reexport_src_str());
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let module = check(&hir, &SourceManifest::default())
            .expect("in-memory package source must not mill packages/");
        assert!(
            !has_core_function(&module, "always_true"),
            "check() must not read nested packages/: {module:?}"
        );
    }

    fn std_core_reexport_src_str() -> &'static str {
        r#"module Std.Core version "0.1.0" {
    import Logic.True version "0.1.0"
    query always_true() -> Bool {
        goal Evaluate { true }
    }
}
"#
    }

    fn docket_lookup_src() -> &'static [u8] {
        br#"module Eff.Lib version "0.1.0" {
    effect DocketLookup {
        request(docket_id: String) -> Bool
    }
    fn lookup() -> Bool ! {DocketLookup} { true }
}
"#
    }

    #[test]
    fn imported_function_effect_rows_are_included() {
        let root = TempRoot::new();
        let digest = write_locked_package(&root.0, "eff", "lib.fr", docket_lookup_src(), "0.1.0");
        let src = r#"
module Examples.UseEff version "0.1.0" {
    import Eff.Lib version "0.1.0"
    query automatic q() -> Bool { return lookup() }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let manifest = package_artifact_manifest("packages/eff/lib.fr", &digest, "0.1.0");
        let err = check_with_sources(&hir, &manifest, Some(&root.0)).unwrap_err();
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E420),
            "{err:?}"
        );
    }
}
