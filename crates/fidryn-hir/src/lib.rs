//! Name resolution, import authentication, and surface-to-HIR elaboration.

mod body;

use fidryn_core::ir::Guard;
use fidryn_core::value::{PropTerm, Term};
use fidryn_core::{Diagnostic, DiagnosticCode, SourceManifest};
use fidryn_syntax::Parse;
use std::collections::BTreeMap;

pub use body::{
    collect_source_callees, collect_term_callees, flatten_term_list, last_brace_inner,
    parse_expr_src, parse_type_name, source_has_bare_prop_if, term_as_name, term_as_prop,
    term_from_decl, term_has_bare_prop_guard,
};

#[derive(Clone, Debug)]
pub struct HirModule {
    pub name: String,
    pub version: String,
    pub type_params: Vec<String>,
    pub jurisdiction: String,
    pub snapshot: String,
    pub manifest_path: String,
    pub outside_scope: Vec<String>,
    pub entities: BTreeMap<String, String>,
    pub propositions: BTreeMap<String, Vec<String>>,
    pub offices: BTreeMap<String, String>,
    pub queries: BTreeMap<String, HirQuery>,
    pub rules: Vec<HirRule>,
    pub nominations: Vec<HirNomination>,
    pub interpretation_families: BTreeMap<String, InterpretationAlts>,
    pub decisions: Vec<HirDecision>,
    pub conflict_doctrines: Vec<HirDoctrine>,
    pub clauses: BTreeMap<String, String>,
    pub imports: Vec<HirImport>,
    pub sources: Vec<HirSource>,
    pub effects: BTreeMap<String, HirEffect>,
    pub functions: BTreeMap<String, HirFunction>,
    pub quantifiers: Vec<HirQuantifier>,
    pub verifications: Vec<HirVerification>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirQuery {
    pub name: String,
    pub result_type: String,
    pub effects: Vec<String>,
    pub automatic: bool,
    pub has_goal: bool,
    /// Original surface text, retained for diagnostics and debug dumps.
    pub plan: String,
    pub params: Vec<(String, String)>,
    pub body: HirQueryBody,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HirQueryBody {
    Return(Term),
    Goal {
        kind: String,
        office: Option<Term>,
        expr: Option<Term>,
        fields: BTreeMap<String, Term>,
    },
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirRule {
    pub name: String,
    pub kind: String,
    pub source: Option<String>,
    pub guard: Option<Guard>,
    pub consequences: Vec<(String, PropTerm)>,
}

pub type EligibilityDef = (PropTerm, bool);
pub type InterpretationAlts = Vec<(String, Vec<EligibilityDef>)>;

#[derive(Clone, Debug)]
pub struct HirNomination {
    pub candidate: String,
    pub office: String,
    pub rank: i64,
}

#[derive(Clone, Debug)]
pub struct HirDecision {
    pub name: String,
    pub requirements: Vec<String>,
    pub result: Option<Term>,
}

#[derive(Clone, Debug)]
pub struct HirDoctrine {
    pub name: String,
    pub target: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirEffect {
    pub name: String,
    pub result_type: String,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirFunction {
    pub name: String,
    pub is_calc: bool,
    pub fuel: Option<u32>,
    pub result_type: String,
    pub source: String,
    pub params: Vec<(String, String)>,
    pub body: Option<Term>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantifierKind {
    ForAll,
    Exists,
}

impl QuantifierKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ForAll => "for_all",
            Self::Exists => "exists",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirImport {
    pub name: String,
    pub type_args: Vec<String>,
    pub digest_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirSource {
    pub name: String,
    pub artifact: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirQuantifier {
    pub kind: QuantifierKind,
    pub binder: String,
    pub domain: String,
    pub formula: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirVerification {
    pub name: String,
    pub formula: String,
    pub bounds: Option<HirVerificationBounds>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirVerificationBounds {
    pub persons: u32,
    pub events: u32,
    pub time_points: u32,
}

pub fn elaborate(parse: &Parse, manifest: &SourceManifest) -> Result<HirModule, Vec<Diagnostic>> {
    let Some(module) = parse.module() else {
        return Err(parse.diagnostics.clone());
    };
    let (name, type_params) = resolve_module_type_params(module, &parse.source);
    let mut hir = HirModule {
        name,
        version: module.version.clone(),
        type_params,
        jurisdiction: String::new(),
        snapshot: manifest.snapshot.clone(),
        manifest_path: String::new(),
        outside_scope: Vec::new(),
        entities: BTreeMap::new(),
        propositions: BTreeMap::new(),
        offices: BTreeMap::new(),
        queries: BTreeMap::new(),
        rules: Vec::new(),
        nominations: Vec::new(),
        interpretation_families: BTreeMap::new(),
        decisions: Vec::new(),
        conflict_doctrines: Vec::new(),
        clauses: BTreeMap::new(),
        imports: Vec::new(),
        sources: Vec::new(),
        effects: BTreeMap::new(),
        functions: BTreeMap::new(),
        quantifiers: Vec::new(),
        verifications: Vec::new(),
        diagnostics: parse.diagnostics.clone(),
    };
    for item in &module.items {
        match item {
            fidryn_syntax::ast::Item::Header(h) => match h.kind {
                fidryn_syntax::ast::HeaderKind::Jurisdiction => {
                    hir.jurisdiction = h.value.clone();
                }
                fidryn_syntax::ast::HeaderKind::SourceSnapshot => {
                    hir.snapshot = trim_quotes(&h.value).to_owned();
                }
                fidryn_syntax::ast::HeaderKind::SourceManifest => {
                    hir.manifest_path = trim_quotes(&h.value).to_owned();
                }
                fidryn_syntax::ast::HeaderKind::OutsideScope => {
                    hir.outside_scope = h
                        .value
                        .split(|c: char| c == '{' || c == '}' || c == ',' || c.is_whitespace())
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect();
                }
                _ => {}
            },
            fidryn_syntax::ast::Item::Import(d) => {
                let (name, type_args) = resolve_import_type_args(d);
                hir.imports.push(HirImport {
                    name,
                    type_args,
                    digest_required: body::import_requires_digest(&d.source),
                });
            }
            fidryn_syntax::ast::Item::Source(d) => {
                let artifact = d.source.split("artifact").nth(1).and_then(|s| {
                    let t = s.trim().trim_start_matches('"');
                    let end = t.find('"')?;
                    Some(t[..end].to_owned())
                });
                hir.sources.push(HirSource {
                    name: d.name.clone().unwrap_or_default(),
                    artifact,
                });
            }
            fidryn_syntax::ast::Item::Entity(d) => {
                if let Some(name) = &d.name {
                    let ty = d
                        .signature
                        .as_deref()
                        .unwrap_or("")
                        .trim_start_matches(':')
                        .trim()
                        .to_owned();
                    hir.entities.insert(name.clone(), ty);
                }
            }
            fidryn_syntax::ast::Item::Proposition(d) => {
                if let Some(name) = &d.name {
                    hir.propositions.insert(name.clone(), Vec::new());
                }
            }
            fidryn_syntax::ast::Item::Office(d) => {
                if let Some(name) = &d.name {
                    hir.offices
                        .insert(name.clone(), d.signature.clone().unwrap_or_default());
                }
            }
            fidryn_syntax::ast::Item::Query(d) => {
                let name = d.name.clone().unwrap_or_default();
                let src = &d.source;
                let body = body::query_body_from_decl(d);
                let has_goal = !matches!(body, HirQueryBody::None)
                    || query_body_has_quantifier_or_require(src);
                let automatic = d.automatic || body::query_is_automatic(src);
                let effects = if d.effects.is_empty() {
                    extract_effects(src)
                } else {
                    d.effects.clone()
                };
                let result_type = d
                    .result_type
                    .clone()
                    .unwrap_or_else(|| extract_return_type(src));
                let params = if d.params.is_empty() {
                    body::extract_params(src)
                } else {
                    d.params.clone()
                };
                hir.quantifiers.extend(extract_quantifiers(src));
                hir.queries.insert(
                    name.clone(),
                    HirQuery {
                        name,
                        result_type,
                        effects,
                        automatic,
                        has_goal,
                        plan: src.clone(),
                        params,
                        body,
                    },
                );
            }
            fidryn_syntax::ast::Item::Function(d) => {
                let name = d.name.clone().unwrap_or_default();
                if !name.is_empty() {
                    let (parsed_params, parsed_body) = body::parse_function_parts(&d.source);
                    let params = if d.params.is_empty() {
                        parsed_params
                    } else {
                        d.params.clone()
                    };
                    hir.functions.insert(
                        name.clone(),
                        HirFunction {
                            is_calc: d.keyword == "calc",
                            fuel: d.fuel.or_else(|| extract_fuel(&d.source)),
                            result_type: d
                                .result_type
                                .clone()
                                .unwrap_or_else(|| extract_return_type(&d.source)),
                            source: d.source.clone(),
                            params,
                            body: d
                                .expr
                                .as_ref()
                                .map(body::expr_to_term)
                                .or_else(|| term_from_decl(d))
                                .or(parsed_body),
                            name,
                        },
                    );
                }
            }
            fidryn_syntax::ast::Item::Effect(d) => {
                let name = d.name.clone().unwrap_or_default();
                if !name.is_empty() {
                    hir.effects.insert(
                        name.clone(),
                        HirEffect {
                            result_type: extract_effect_result(&d.source),
                            source: d.source.clone(),
                            name,
                        },
                    );
                }
            }
            fidryn_syntax::ast::Item::Verify(d) => {
                hir.quantifiers.extend(extract_quantifiers(&d.source));
                if let Some(verification) = verification_from_decl(d) {
                    hir.verifications.push(verification);
                }
            }
            fidryn_syntax::ast::Item::Rule(d) => {
                let (parsed_guard, parsed_consequences) = body::parse_rule_parts(&d.source);
                let guard = d.guard.as_ref().map(body::expr_to_guard).or(parsed_guard);
                let consequences = if d.consequences.is_empty() {
                    parsed_consequences
                } else {
                    body::consequences_from_ast(&d.consequences)
                };
                hir.rules.push(HirRule {
                    name: d.name.clone().unwrap_or_default(),
                    kind: d
                        .rule_kind
                        .clone()
                        .filter(|kind| {
                            matches!(kind.as_str(), "derive" | "constitutive" | "prescriptive")
                        })
                        .unwrap_or_else(|| body::parse_rule_kind(&d.source)),
                    source: Some(d.source.clone()),
                    guard,
                    consequences,
                });
            }
            fidryn_syntax::ast::Item::Nomination(d) => {
                let rank = extract_rank(&d.source);
                hir.nominations.push(HirNomination {
                    candidate: d.name.clone().unwrap_or_default(),
                    office: extract_office(&d.source),
                    rank,
                });
            }
            fidryn_syntax::ast::Item::Interpretation(d) => {
                let name = d.name.clone().unwrap_or_default();
                let alts = extract_alternative_defs(&d.source);
                hir.interpretation_families.insert(name, alts);
            }
            fidryn_syntax::ast::Item::Decision(d) => {
                hir.decisions.push(HirDecision {
                    name: d.name.clone().unwrap_or_default(),
                    requirements: extract_record_requires(&d.source),
                    result: extract_returns(&d.source),
                });
            }
            fidryn_syntax::ast::Item::ConflictDoctrine(d) => {
                hir.conflict_doctrines.push(HirDoctrine {
                    name: d.name.clone().unwrap_or_default(),
                    target: d.source.clone(),
                });
            }
            fidryn_syntax::ast::Item::Clause(d) => {
                if let Some(name) = &d.name {
                    hir.clauses.insert(name.clone(), d.source.clone());
                }
            }
            _ => {}
        }
    }
    if hir.diagnostics.iter().any(|d| {
        matches!(
            d.code,
            DiagnosticCode::E100
                | DiagnosticCode::E200
                | DiagnosticCode::E210
                | DiagnosticCode::E310
        )
    }) {
        return Err(hir.diagnostics);
    }
    Ok(hir)
}

fn trim_quotes(s: &str) -> &str {
    s.trim().trim_matches('"').trim()
}

fn query_body_has_quantifier_or_require(src: &str) -> bool {
    let inner = last_brace_inner(src).unwrap_or("");
    !inner.is_empty()
        && (contains_ident(inner, "require")
            || contains_ident(inner, "for_all")
            || contains_ident(inner, "exists")
            || contains_ident(inner, "goal")
            || contains_ident(inner, "return"))
}

fn extract_effects(src: &str) -> Vec<String> {
    let Some(start) = src.find('!') else {
        return Vec::new();
    };
    let rest = &src[start..];
    let Some(open) = rest.find('{') else {
        return Vec::new();
    };
    let Some(close) = rest.find('}') else {
        return Vec::new();
    };
    rest[open + 1..close]
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

fn extract_return_type(src: &str) -> String {
    let Some(after_arrow) = src.split("->").nth(1) else {
        return String::new();
    };
    let mut ty = after_arrow;
    if let Some(i) = ty.find('!') {
        ty = &ty[..i];
    }
    if let Some(i) = ty.find('{') {
        ty = &ty[..i];
    }
    if let Some(i) = find_ident(ty, "fuel") {
        ty = &ty[..i];
    }
    ty.trim().to_owned()
}

fn extract_rank(src: &str) -> i64 {
    src.split("rank")
        .nth(1)
        .and_then(|s| {
            s.split_whitespace()
                .next()
                .and_then(|n| n.trim_matches(';').parse().ok())
        })
        .unwrap_or(0)
}

fn extract_office(src: &str) -> String {
    src.split(" for ")
        .nth(1)
        .map(|s| s.split(" rank").next().unwrap_or(s).trim().to_owned())
        .unwrap_or_default()
}

#[allow(dead_code)]
fn extract_alternatives(src: &str) -> Vec<String> {
    extract_alternative_defs(src)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

fn extract_alternative_defs(src: &str) -> InterpretationAlts {
    let mut out = Vec::new();
    for (i, part) in src.split("alternative ").enumerate() {
        if i == 0 {
            continue;
        }
        let name = part
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_matches('{')
            .to_owned();
        if name.is_empty() {
            continue;
        }
        let mut defs = Vec::new();
        let mut rest = part;
        while let Some(idx) = rest.find("defines ") {
            rest = &rest[idx + "defines ".len()..];
            let stmt_end = rest.find("defines ").unwrap_or(rest.len());
            let stmt = &rest[..stmt_end];
            if let Some(end) = stmt.find(" as not_established") {
                let expr = stmt[..end].trim();
                if let Some(prop) = body::parse_expr_src(expr).and_then(|t| body::term_as_prop(&t))
                {
                    defs.push((prop, false));
                }
            } else if let Some(end) = stmt.find(" as established") {
                let expr = stmt[..end].trim();
                if let Some(prop) = body::parse_expr_src(expr).and_then(|t| body::term_as_prop(&t))
                {
                    defs.push((prop, true));
                }
            }
            rest = &rest[stmt_end..];
        }
        out.push((name, defs));
    }
    out
}

fn extract_record_requires(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(idx) = rest.find("record requires ") {
        rest = &rest[idx + "record requires ".len()..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

fn extract_returns(src: &str) -> Option<Term> {
    let rest = src.split("returns ").nth(1)?.trim();
    let token: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if token.is_empty() {
        None
    } else {
        Some(Term::Ident(token))
    }
}

fn extract_fuel(src: &str) -> Option<u32> {
    let mut start = 0;
    while let Some(rel) = src[start..].find("fuel") {
        let i = start + rel;
        if is_ident_boundary(src, i, 4) {
            let rest = src[i + 4..].trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim_start();
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                if let Ok(n) = digits.parse() {
                    return Some(n);
                }
            }
        }
        start = i + 1;
    }
    None
}

fn extract_effect_result(src: &str) -> String {
    src.split("->")
        .last()
        .map(|s| {
            s.trim()
                .trim_end_matches('}')
                .trim()
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_owned()
        })
        .unwrap_or_default()
}

fn verification_from_decl(d: &fidryn_syntax::ast::Decl) -> Option<HirVerification> {
    if d.keyword != "verify" {
        return None;
    }
    let name = d.name.as_ref().filter(|n| !n.is_empty())?.clone();
    Some(HirVerification {
        formula: verify_formula(d),
        bounds: verify_bounds(d),
        name,
    })
}

fn verify_formula(d: &fidryn_syntax::ast::Decl) -> String {
    if let Some(text) = d.fields.get("assert") {
        let trimmed = text.trim().trim_end_matches(';').trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    if let Some(text) = extract_assert_clause(&d.source) {
        return text;
    }
    match &d.expr {
        Some(fidryn_syntax::ast::Expr::Bool(true)) => "true".into(),
        Some(fidryn_syntax::ast::Expr::Bool(false)) => "false".into(),
        _ => last_brace_inner(&d.source).unwrap_or("").trim().to_owned(),
    }
}

fn verify_bounds(d: &fidryn_syntax::ast::Decl) -> Option<HirVerificationBounds> {
    let text = d
        .fields
        .get("bounds")
        .cloned()
        .or_else(|| extract_bounds_clause(&d.source))?;
    Some(HirVerificationBounds {
        persons: labeled_u32(&text, "persons").unwrap_or(0),
        events: labeled_u32(&text, "events").unwrap_or(0),
        time_points: labeled_u32(&text, "time_points").unwrap_or(0),
    })
}

fn extract_assert_clause(src: &str) -> Option<String> {
    let i = find_ident(src, "assert")?;
    Some(take_clause_to_end(&src[i..]))
}

fn extract_bounds_clause(src: &str) -> Option<String> {
    let i = find_ident(src, "bounds")?;
    let rest = src[i..].trim_start();
    if rest.len() < 6 {
        return None;
    }
    let after = rest[6..].trim_start();
    if after.starts_with('{') {
        let inner = brace_inner(after)?;
        Some(format!("bounds {{{inner}}}"))
    } else {
        Some(take_clause_to_end(&src[i..]))
    }
}

fn take_clause_to_end(s: &str) -> String {
    let mut depth_paren = 0i32;
    let mut depth_brack = 0i32;
    let mut depth_brace = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '[' => depth_brack += 1,
            ']' => depth_brack -= 1,
            '{' => depth_brace += 1,
            '}' => {
                if depth_paren == 0 && depth_brack == 0 && depth_brace == 0 {
                    return s[..i].trim().trim_end_matches(';').trim().to_owned();
                }
                depth_brace -= 1;
            }
            ';' if depth_paren == 0 && depth_brack == 0 && depth_brace == 0 => {
                return s[..i].trim().to_owned();
            }
            _ => {}
        }
    }
    s.trim().trim_end_matches(';').trim().to_owned()
}

fn labeled_u32(src: &str, label: &str) -> Option<u32> {
    let i = find_ident(src, label)?;
    let rest = src[i + label.len()..]
        .trim_start()
        .trim_start_matches(':')
        .trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn extract_quantifiers(src: &str) -> Vec<HirQuantifier> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < src.len() {
        if let Some((kind, kw_len)) = quantifier_at(src, i) {
            if let Some(q) = parse_quantifier_tail(kind, &src[i + kw_len..]) {
                out.push(q);
            }
            i += kw_len;
        } else {
            i += src[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        }
    }
    out
}

fn quantifier_at(src: &str, i: usize) -> Option<(QuantifierKind, usize)> {
    for (kind, kw) in [
        (QuantifierKind::ForAll, "for_all"),
        (QuantifierKind::Exists, "exists"),
    ] {
        if src[i..].starts_with(kw) && is_ident_boundary(src, i, kw.len()) {
            return Some((kind, kw.len()));
        }
    }
    None
}

fn parse_quantifier_tail(kind: QuantifierKind, rest: &str) -> Option<HirQuantifier> {
    let rest = rest.trim_start();
    let binder_len = ident_len(rest)?;
    let binder = rest[..binder_len].to_owned();
    let after_binder = rest[binder_len..].trim_start();
    if !after_binder.starts_with("in") || !is_ident_boundary(after_binder, 0, 2) {
        return None;
    }
    let after_in = after_binder[2..].trim_start();
    let (domain, formula) = split_domain_formula(after_in)?;
    Some(HirQuantifier {
        kind,
        binder,
        domain,
        formula,
    })
}

fn ident_len(s: &str) -> Option<usize> {
    let mut chars = s.char_indices();
    let (_, first) = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut end = first.len_utf8();
    for (i, c) in chars {
        if c.is_ascii_alphanumeric() || c == '_' {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    Some(end)
}

fn split_domain_formula(s: &str) -> Option<(String, String)> {
    let mut depth_paren = 0i32;
    let mut depth_brack = 0i32;
    let mut depth_brace = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '[' => depth_brack += 1,
            ']' => depth_brack -= 1,
            '{' => {
                if depth_paren == 0 && depth_brack == 0 && depth_brace == 0 {
                    let domain = s[..i].trim().to_owned();
                    let inner = brace_inner(&s[i..]).unwrap_or("").trim().to_owned();
                    return Some((domain, inner));
                }
                depth_brace += 1;
            }
            '}' => depth_brace -= 1,
            ':' if depth_paren == 0 && depth_brack == 0 && depth_brace == 0 => {
                let domain = s[..i].trim().to_owned();
                let formula = s[i + 1..]
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_end_matches(';')
                    .trim()
                    .to_owned();
                return Some((domain, formula));
            }
            _ => {}
        }
    }
    None
}

fn brace_inner(s: &str) -> Option<&str> {
    if !s.starts_with('{') {
        return None;
    }
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[1..i]);
                }
            }
            _ => {}
        }
    }
    None
}

fn contains_ident(src: &str, name: &str) -> bool {
    find_ident(src, name).is_some()
}

fn find_ident(src: &str, name: &str) -> Option<usize> {
    let mut start = 0;
    while let Some(rel) = src[start..].find(name) {
        let i = start + rel;
        if is_ident_boundary(src, i, name.len()) {
            return Some(i);
        }
        start = i + 1;
    }
    None
}

fn is_ident_boundary(src: &str, start: usize, len: usize) -> bool {
    let before = src[..start].chars().next_back();
    let after = src[start + len..].chars().next();
    let ident_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
    before.is_none_or(|c| !ident_char(c)) && after.is_none_or(|c| !ident_char(c))
}

/// Split `QName` / `QName<A, B>` into the base name and angle-bracket arguments.
pub fn split_qname_type_args(s: &str) -> (String, Vec<String>) {
    let s = s.trim();
    let Some(open) = s.find('<') else {
        return (s.to_owned(), Vec::new());
    };
    let name = s[..open].trim().to_owned();
    (name, parse_leading_angle_args(&s[open..]))
}

/// Parse the first `<...>` argument list in `s`, respecting nested angles.
pub fn parse_angle_args(s: &str) -> Vec<String> {
    let Some(open) = s.find('<') else {
        return Vec::new();
    };
    parse_leading_angle_args(&s[open..])
}

/// Type parameters written on a `module Name<T>` header, used when the AST
/// field is empty.
pub fn type_params_from_source(source: &str) -> Vec<String> {
    parse_module_header(source).1
}

fn resolve_module_type_params(
    module: &fidryn_syntax::ast::Module,
    source: &str,
) -> (String, Vec<String>) {
    let (base_from_name, args_from_name) = split_qname_type_args(&module.name);
    let name = if base_from_name.is_empty() {
        module.name.clone()
    } else {
        base_from_name
    };
    let params = if !module.type_params.is_empty() {
        module.type_params.clone()
    } else if !args_from_name.is_empty() {
        args_from_name
    } else {
        type_params_from_source(source)
    };
    (name, params)
}

fn resolve_import_type_args(d: &fidryn_syntax::ast::Decl) -> (String, Vec<String>) {
    let raw = d.name.clone().unwrap_or_default();
    let (base, args_from_name) = split_qname_type_args(&raw);
    let name = if base.is_empty() { raw } else { base };
    let args = if !d.type_args.is_empty() {
        d.type_args.clone()
    } else if !args_from_name.is_empty() {
        args_from_name
    } else {
        let mut args = type_args_from_import_text(&d.source);
        if args.is_empty()
            && let Some(sig) = &d.signature
        {
            args = parse_leading_angle_args(sig);
        }
        args
    };
    (name, args)
}

fn parse_module_header(source: &str) -> (String, Vec<String>) {
    let mut i = 0usize;
    skip_ws_and_comments(source, &mut i);
    if !source[i..].starts_with("module") || !is_ident_boundary(source, i, 6) {
        return (String::new(), Vec::new());
    }
    i += 6;
    skip_ws_and_comments(source, &mut i);
    let name_start = i;
    while i < source.len() {
        let Some(c) = source[i..].chars().next() else {
            break;
        };
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            i += c.len_utf8();
        } else {
            break;
        }
    }
    let name = source[name_start..i].trim_matches('.').to_owned();
    skip_ws_and_comments(source, &mut i);
    (name, parse_leading_angle_args(&source[i..]))
}

fn type_args_from_import_text(source: &str) -> Vec<String> {
    let s = source.trim();
    let rest = s.strip_prefix("import").unwrap_or(s);
    let mut i = 0usize;
    skip_ws_and_comments(rest, &mut i);
    while i < rest.len() {
        let Some(c) = rest[i..].chars().next() else {
            break;
        };
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            i += c.len_utf8();
        } else {
            break;
        }
    }
    parse_leading_angle_args(&rest[i..])
}

fn parse_leading_angle_args(s: &str) -> Vec<String> {
    let s = s.trim_start();
    if !s.starts_with('<') {
        return Vec::new();
    }
    let mut depth = 0i32;
    let mut end = None;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end) = end else {
        return Vec::new();
    };
    split_top_level_commas(&s[1..end])
}

fn split_top_level_commas(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth_paren = 0i32;
    let mut depth_angle = 0i32;
    let mut depth_brack = 0i32;
    let mut depth_brace = 0i32;
    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '[' => depth_brack += 1,
            ']' => depth_brack -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            ',' if depth_paren == 0 && depth_angle == 0 && depth_brack == 0 && depth_brace == 0 => {
                let part = inner[start..i].trim();
                if !part.is_empty() {
                    parts.push(part.to_owned());
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let part = inner[start..].trim();
    if !part.is_empty() {
        parts.push(part.to_owned());
    }
    parts
}

fn skip_ws_and_comments(s: &str, i: &mut usize) {
    loop {
        if *i >= s.len() {
            return;
        }
        let rest = &s[*i..];
        let Some(c) = rest.chars().next() else {
            return;
        };
        if c.is_whitespace() {
            *i += c.len_utf8();
            continue;
        }
        if rest.starts_with("//") {
            match rest.find('\n') {
                Some(nl) => *i += nl + 1,
                None => *i = s.len(),
            }
            continue;
        }
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_syntax::parse_file;
    use std::str::FromStr;

    #[test]
    fn elaborates_entities() {
        let src = r#"
module Examples.Mini version "0.1.0" {
    jurisdiction Massachusetts
    entity Bryan : NaturalPerson
    query acting_trustee() -> LegalPerson ! {Observe} {
        goal UniqueOccupant { office TrusteeOf(BRT) }
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(
            hir.entities.get("Bryan").map(String::as_str),
            Some("NaturalPerson")
        );
        assert!(hir.queries["acting_trustee"].has_goal);
    }

    #[test]
    fn extracts_effect_fuel_and_quantifiers() {
        let src = r#"
module Examples.Lang version "0.1.0" {
    effect DocketLookup {
        request(docket_id: String) -> Docket
    }
    fn countdown(n: Int) -> Int fuel = 4 {
        countdown(n - 1)
    }
    calc double(n: Int) -> Int {
        n + n
    }
    query all_ok() -> Bool {
        for_all x in People: Eligible(x)
        exists y in People: Trustee(y)
    }
    verify ClosedWorld {
        for_all p in People { Eligible(p) }
    }
    exists w in People: Trustee(w)
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let effect = &hir.effects["DocketLookup"];
        assert_eq!(effect.result_type, "Docket");
        let countdown = &hir.functions["countdown"];
        assert!(!countdown.is_calc);
        assert_eq!(countdown.fuel, Some(4));
        assert_eq!(countdown.result_type, "Int");
        let double = &hir.functions["double"];
        assert!(double.is_calc);
        assert_eq!(double.fuel, None);
        let kinds: Vec<_> = hir
            .quantifiers
            .iter()
            .map(|q| (q.kind.as_str(), q.binder.as_str(), q.domain.as_str()))
            .collect();
        assert!(kinds.contains(&("for_all", "x", "People")), "{kinds:?}");
        assert!(kinds.contains(&("exists", "y", "People")), "{kinds:?}");
        assert!(kinds.contains(&("for_all", "p", "People")), "{kinds:?}");
        assert!(kinds.contains(&("exists", "w", "People")), "{kinds:?}");
        assert!(hir.queries["all_ok"].has_goal);
        let countdown_params = &hir.functions["countdown"].params;
        assert_eq!(countdown_params, &[("n".to_owned(), "Int".to_owned())]);
        assert!(
            matches!(
                hir.functions["countdown"].body,
                Some(Term::Call { ref callee, .. }) if callee == "countdown"
            ) || matches!(
                hir.functions["countdown"].body,
                Some(Term::Apply { ref ctor, .. }) if ctor == "countdown"
            ),
            "{:?}",
            hir.functions["countdown"].body
        );
    }

    #[test]
    fn elaborates_evaluate_true_as_bool_term() {
        let src = r#"
module Examples.EvalTrue version "0.1.0" {
    query flag() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let q = &hir.queries["flag"];
        assert_eq!(q.result_type, "Bool");
        assert!(
            matches!(
                q.body,
                HirQueryBody::Goal {
                    ref kind,
                    expr: Some(Term::Bool(true)),
                    ..
                } if kind == "Evaluate"
            ),
            "{:?}",
            q.body
        );
    }

    #[test]
    fn elaborates_rule_consequences() {
        let src = r#"
module Examples.Rule version "0.1.0" {
    rule R : derive { when operative P() then derive Q() }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(hir.rules.len(), 1);
        assert_eq!(hir.rules[0].kind, "derive");
        assert!(!hir.rules[0].consequences.is_empty());
        assert_eq!(hir.rules[0].consequences[0].0, "derive");
        assert_eq!(hir.rules[0].consequences[0].1.predicate, "Q");
    }

    #[test]
    fn elaborates_interpretation_defines_not_just_labels() {
        let src = r#"
module Examples.Interp version "0.1.0" {
    interpretation_family SuccessorEligibility {
        alternative Both {
            defines Eligible(Alice, TrusteeOf(BRT)) as established
            defines Eligible(Bob, TrusteeOf(BRT)) as established
            defines Eligible(Carol, TrusteeOf(BRT)) as not_established
        }
        alternative BobOnly {
            defines Eligible(Alice, TrusteeOf(BRT)) as not_established
            defines Eligible(Bob, TrusteeOf(BRT)) as established
        }
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let alts = &hir.interpretation_families["SuccessorEligibility"];
        assert_eq!(alts.len(), 2, "{alts:?}");
        assert_eq!(alts[0].0, "Both");
        assert_eq!(alts[0].1.len(), 3, "{:?}", alts[0].1);
        assert!(
            alts[0].1.iter().any(|(prop, established)| *established
                && prop
                    .arguments
                    .iter()
                    .any(|a| matches!(a, Term::Ident(n) if n == "Alice"))),
            "{:?}",
            alts[0].1
        );
        assert!(
            alts[0].1.iter().any(|(prop, established)| !established
                && prop
                    .arguments
                    .iter()
                    .any(|a| matches!(a, Term::Ident(n) if n == "Carol"))),
            "{:?}",
            alts[0].1
        );
        assert_eq!(alts[1].0, "BobOnly");
        assert!(
            alts[1]
                .1
                .iter()
                .any(|(prop, established)| prop.predicate == "Eligible"
                    && !established
                    && prop
                        .arguments
                        .iter()
                        .any(|a| matches!(a, Term::Ident(n) if n == "Alice"))),
            "{:?}",
            alts[1].1
        );
    }

    #[test]
    fn elaborates_decimal_money_and_for_all() {
        let src = r#"
module Examples.Lower version "0.1.0" {
    calc floor() -> Decimal { 11925.00 }
    calc cash() -> Money<USD> { USD(11925.00) }
    calc all_ok() -> Bool { for_all x in People: Eligible(x) }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let expected = rust_decimal::Decimal::from_str("11925.00").expect("decimal");
        assert_eq!(hir.functions["floor"].body, Some(Term::Decimal(expected)));
        assert_eq!(
            hir.functions["cash"].body,
            Some(Term::Apply {
                ctor: "USD".into(),
                args: vec![Term::Decimal(expected)],
            })
        );
        match &hir.functions["all_ok"].body {
            Some(Term::Apply { ctor, args }) | Some(Term::Call { callee: ctor, args }) => {
                assert_eq!(ctor, "for_all");
                assert_eq!(args.len(), 3, "{args:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn elaborates_module_type_params() {
        let src = r#"
module Id<T> version "0.1.0" {
    entity X: T
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(hir.name, "Id");
        assert_eq!(hir.type_params, vec!["T".to_owned()]);
        assert_eq!(hir.entities.get("X").map(String::as_str), Some("T"));
        assert_eq!(
            type_params_from_source(src),
            vec!["T".to_owned()],
            "header fallback must also see module Id<T>"
        );
    }

    #[test]
    fn elaborates_import_type_args() {
        let src = r#"
module Host version "0.1.0" {
    import Id<NaturalPerson> version "0.1.0"
    entity X: NaturalPerson
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(hir.imports.len(), 1);
        assert_eq!(hir.imports[0].name, "Id");
        assert_eq!(hir.imports[0].type_args, vec!["NaturalPerson".to_owned()]);
        assert!(hir.type_params.is_empty());
    }

    #[test]
    fn type_params_from_source_reads_header_when_ast_empty() {
        let src = r#"module Examples.Box<T, U> version "0.1.0" { entity X: T }"#;
        assert_eq!(
            type_params_from_source(src),
            vec!["T".to_owned(), "U".to_owned()]
        );
        assert_eq!(
            split_qname_type_args("Examples.Box<T, U>"),
            (
                "Examples.Box".to_owned(),
                vec!["T".to_owned(), "U".to_owned()]
            )
        );
    }

    fn require_seq_is_not_bool_true(term: &Term) {
        assert_ne!(term, &Term::Bool(true), "{term:?}");
        match term {
            Term::Apply { ctor, args } if ctor == "seq" && args.len() == 2 => {
                assert_eq!(
                    args[0],
                    Term::Apply {
                        ctor: "require".into(),
                        args: vec![Term::Bool(false)],
                    }
                );
                assert_eq!(args[1], Term::Bool(true));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn require_false_return_true_is_not_just_bool_true() {
        let src = r#"
module Examples.Req version "0.1.0" {
    query q() -> Bool { require false; return true }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        match &hir.queries["q"].body {
            HirQueryBody::Return(term) => require_seq_is_not_bool_true(term),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn evaluate_require_false_return_true_is_not_just_bool_true() {
        let src = r#"
module Examples.ReqEval version "0.1.0" {
    query q() -> Bool { goal Evaluate { require false; return true } }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        match &hir.queries["q"].body {
            HirQueryBody::Goal {
                kind,
                expr: Some(term),
                ..
            } if kind == "Evaluate" => require_seq_is_not_bool_true(term),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn elaborates_verify_trivial_assert_true() {
        let src = r#"
module Examples.Trivial version "0.1.0" {
    verify Trivial { assert true }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(hir.verifications.len(), 1, "{:?}", hir.verifications);
        let v = &hir.verifications[0];
        assert_eq!(v.name, "Trivial");
        assert!(
            v.formula.contains("true"),
            "formula should contain true: {:?}",
            v.formula
        );
        assert!(
            v.formula == "assert true" || v.formula == "true",
            "formula should be a true literal: {:?}",
            v.formula
        );
        assert!(v.bounds.is_none(), "{:?}", v.bounds);
    }

    #[test]
    fn money_usd_and_eur_differ() {
        let src = r#"
module Examples.Fx version "0.1.0" {
    calc usd() -> Money<USD> { USD(1.00) }
    calc eur() -> Money<EUR> { EUR(1.00) }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let expected = rust_decimal::Decimal::from_str("1.00").expect("decimal");
        let usd = hir.functions["usd"].body.as_ref().expect("usd");
        let eur = hir.functions["eur"].body.as_ref().expect("eur");
        assert_eq!(
            usd,
            &Term::Apply {
                ctor: "USD".into(),
                args: vec![Term::Decimal(expected)],
            }
        );
        assert_eq!(
            eur,
            &Term::Apply {
                ctor: "EUR".into(),
                args: vec![Term::Decimal(expected)],
            }
        );
        assert_ne!(usd, eur);
    }
}
