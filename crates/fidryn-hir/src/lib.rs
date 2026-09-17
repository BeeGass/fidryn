//! Name resolution, import authentication, and surface-to-HIR elaboration.

use fidryn_core::{Diagnostic, DiagnosticCode, SourceManifest};
use fidryn_syntax::Parse;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct HirModule {
    pub name: String,
    pub version: String,
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
    pub interpretation_families: BTreeMap<String, Vec<String>>,
    pub conflict_doctrines: Vec<HirDoctrine>,
    pub clauses: BTreeMap<String, String>,
    pub imports: Vec<HirImport>,
    pub sources: Vec<HirSource>,
    pub effects: BTreeMap<String, HirEffect>,
    pub functions: BTreeMap<String, HirFunction>,
    pub quantifiers: Vec<HirQuantifier>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
pub struct HirQuery {
    pub name: String,
    pub result_type: String,
    pub effects: Vec<String>,
    pub automatic: bool,
    pub has_goal: bool,
    pub plan: String,
}

#[derive(Clone, Debug)]
pub struct HirRule {
    pub name: String,
    pub kind: String,
    pub source: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HirNomination {
    pub candidate: String,
    pub office: String,
    pub rank: i64,
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

pub fn elaborate(parse: &Parse, manifest: &SourceManifest) -> Result<HirModule, Vec<Diagnostic>> {
    let Some(module) = parse.module() else {
        return Err(parse.diagnostics.clone());
    };
    let mut hir = HirModule {
        name: module.name.clone(),
        version: module.version.clone(),
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
        conflict_doctrines: Vec::new(),
        clauses: BTreeMap::new(),
        imports: Vec::new(),
        sources: Vec::new(),
        effects: BTreeMap::new(),
        functions: BTreeMap::new(),
        quantifiers: Vec::new(),
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
                hir.imports.push(HirImport {
                    name: d.name.clone().unwrap_or_default(),
                    digest_required: d.source.contains("digest"),
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
                let has_goal = src.contains('{')
                    && (contains_ident(src, "goal")
                        || contains_ident(src, "return")
                        || contains_ident(src, "require")
                        || contains_ident(src, "for_all")
                        || contains_ident(src, "exists"));
                let automatic = src.contains("automatic");
                let effects = extract_effects(src);
                hir.quantifiers.extend(extract_quantifiers(src));
                hir.queries.insert(
                    name.clone(),
                    HirQuery {
                        name,
                        result_type: extract_return_type(src),
                        effects,
                        automatic,
                        has_goal,
                        plan: src.clone(),
                    },
                );
            }
            fidryn_syntax::ast::Item::Function(d) => {
                let name = d.name.clone().unwrap_or_default();
                if !name.is_empty() {
                    hir.functions.insert(
                        name.clone(),
                        HirFunction {
                            is_calc: d.keyword == "calc",
                            fuel: extract_fuel(&d.source),
                            result_type: extract_return_type(&d.source),
                            source: d.source.clone(),
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
            }
            fidryn_syntax::ast::Item::Rule(d) => {
                hir.rules.push(HirRule {
                    name: d.name.clone().unwrap_or_default(),
                    kind: if d.source.contains(": constitutive") {
                        "constitutive".into()
                    } else if d.source.contains(": derive") {
                        "derive".into()
                    } else {
                        "prescriptive".into()
                    },
                    source: Some(d.source.clone()),
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
                let alts = extract_alternatives(&d.source);
                hir.interpretation_families.insert(name, alts);
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

fn extract_alternatives(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in src.split("alternative ") {
        if part == src {
            continue;
        }
        if let Some(name) = part.split_whitespace().next() {
            out.push(name.to_owned());
        }
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_syntax::parse_file;

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
    }
}
