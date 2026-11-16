//! Name resolution, import authentication, and surface-to-HIR elaboration.

mod body;
#[cfg(test)]
mod comment_tests;
mod text;

use fidryn_core::ir::Guard;
use fidryn_core::value::{PropTerm, Term};
use fidryn_core::{Diagnostic, DiagnosticCode, SourceManifest};
use fidryn_syntax::{Parse, TokenKind, lex};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Range;

pub use body::{
    collect_source_callees, collect_term_callees, flatten_term_list, last_brace_inner,
    parse_expr_src, parse_type_name, source_has_bare_prop_if, term_as_name, term_as_prop,
    term_from_decl, term_has_bare_prop_guard, term_to_guard,
};
pub use text::{structure_of, without_comments};

pub type HirRecordTypes = BTreeMap<String, (bool, Vec<(String, String)>)>;
pub type HirTypes = fidryn_core::types::TypeEnvironment;
pub type HirCallableSignatures = BTreeMap<String, (Vec<(String, String)>, String)>;

#[derive(Clone, Debug)]
pub struct HirModule {
    pub name: String,
    pub version: String,
    pub type_params: Vec<String>,
    pub jurisdiction: String,
    pub snapshot: String,
    pub manifest_path: String,
    pub effective_at: String,
    pub recorded_at: String,
    pub outside_scope: Vec<String>,
    pub entities: BTreeMap<String, String>,
    pub propositions: BTreeMap<String, Vec<(String, String)>>,
    pub record_types: HirRecordTypes,
    pub types: HirTypes,
    pub offices: BTreeMap<String, String>,
    pub callable_signatures: HirCallableSignatures,
    pub queries: BTreeMap<String, HirQuery>,
    pub rules: Vec<HirRule>,
    pub duties: Vec<HirDuty>,
    pub nominations: Vec<HirNomination>,
    pub interpretation_families: BTreeMap<String, InterpretationAlts>,
    pub decisions: Vec<HirDecision>,
    pub judgments: Vec<HirJudgment>,
    pub observations: Vec<HirObservation>,
    pub conflict_doctrines: Vec<HirDoctrine>,
    pub clauses: BTreeMap<String, String>,
    pub imports: Vec<HirImport>,
    pub sources: Vec<HirSource>,
    pub effects: BTreeMap<String, HirEffect>,
    pub functions: BTreeMap<String, HirFunction>,
    pub quantifiers: Vec<HirQuantifier>,
    pub verifications: Vec<HirVerification>,
    /// Parsed surface constructs that are not yet executable Core.
    pub retained: Vec<HirRetainedDecl>,
    pub diagnostics: Vec<Diagnostic>,
}

/// A parsed declaration kept for conservation rather than silently dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirRetainedDecl {
    pub kind: String,
    pub name: String,
    pub source: String,
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
    pub binders: Vec<String>,
    pub typed_binders: Vec<(String, String)>,
    pub guard_terms: Vec<Term>,
    pub guard: Option<Guard>,
    pub selection: Option<HirSelection>,
    pub consequences: Vec<(String, PropTerm)>,
    pub fallback: Vec<(String, PropTerm)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirDuty {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub bearer: String,
    pub bearer_term: Term,
    pub claimant: Option<String>,
    pub claimant_term: Option<Term>,
    pub attaches: Option<Guard>,
    pub attaches_term: Option<Term>,
    pub content: Vec<Term>,
    pub due: Option<Term>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirSelection {
    pub binder: String,
    pub finite_domain: Term,
    pub predicate: Term,
    pub ordering: Vec<String>,
    pub ascending: Vec<bool>,
    pub require_unique_keys: bool,
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
    pub params: Vec<(String, String)>,
    pub result_type: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HirJudgment {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub decides: PropTerm,
    pub source: Option<Term>,
    pub authority: fidryn_core::ir::JudgmentAuthority,
    pub records: Vec<(String, String)>,
    pub record_patterns: Vec<Term>,
    pub constraints: Vec<Term>,
    pub establish_when: Option<Term>,
    pub reject_when: Option<Term>,
    pub otherwise: Option<fidryn_core::ir::JudgmentFallback>,
    pub on_established: Vec<fidryn_core::ir::JudgmentAction>,
    pub standard: Option<Term>,
    pub review: Option<fidryn_core::ir::JudgmentReview>,
    pub unsupported_clauses: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct HirObservation {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub establishes: PropTerm,
    pub validators: Vec<Term>,
    pub source: Option<Term>,
    pub unsupported_clauses: Vec<String>,
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
    pub effects: Vec<String>,
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
    /// Expected artifact digest from `digest "..."` on the import, if present.
    pub digest: Option<String>,
    /// Requested import version from `version "..."`, if present.
    pub version: Option<String>,
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
    pub assertions: Vec<Term>,
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
        effective_at: String::new(),
        recorded_at: String::new(),
        outside_scope: Vec::new(),
        entities: BTreeMap::new(),
        propositions: BTreeMap::new(),
        record_types: standard_type_environment().1.clone(),
        types: standard_type_environment().0.clone(),
        offices: BTreeMap::new(),
        callable_signatures: BTreeMap::new(),
        queries: BTreeMap::new(),
        rules: Vec::new(),
        duties: Vec::new(),
        nominations: Vec::new(),
        interpretation_families: BTreeMap::new(),
        decisions: Vec::new(),
        judgments: Vec::new(),
        observations: Vec::new(),
        conflict_doctrines: Vec::new(),
        clauses: BTreeMap::new(),
        imports: Vec::new(),
        sources: Vec::new(),
        effects: BTreeMap::new(),
        functions: BTreeMap::new(),
        quantifiers: Vec::new(),
        verifications: Vec::new(),
        retained: Vec::new(),
        diagnostics: parse.diagnostics.clone(),
    };
    let mut declared = BTreeMap::new();
    for item in &module.items {
        let decl = match item {
            fidryn_syntax::ast::Item::Header(_)
            | fidryn_syntax::ast::Item::Import(_)
            | fidryn_syntax::ast::Item::Nomination(_) => continue,
            other => other.decl(),
        };
        let Some(decl) = decl else {
            continue;
        };
        let Some(name) = decl.name.as_ref() else {
            continue;
        };
        let namespace = if matches!(
            item,
            fidryn_syntax::ast::Item::Type(_) | fidryn_syntax::ast::Item::RecordType(_)
        ) {
            "type"
        } else {
            "value"
        };
        if let Some(first) = declared.insert((namespace, name.clone()), decl.span) {
            let mut diagnostic = Diagnostic::new(
                DiagnosticCode::E200,
                format!("duplicate {namespace} declaration `{name}`"),
            )
            .with_span(decl.span);
            diagnostic.related_spans.push(first);
            hir.diagnostics.push(diagnostic);
        }
    }
    for item in &module.items {
        match item {
            fidryn_syntax::ast::Item::Header(h) => match h.kind {
                fidryn_syntax::ast::HeaderKind::Jurisdiction => {
                    hir.jurisdiction = h.value.clone();
                }
                fidryn_syntax::ast::HeaderKind::SourceSnapshot => {
                    hir.snapshot =
                        fidryn_syntax::decode_string_literal(&h.value).unwrap_or_default();
                }
                fidryn_syntax::ast::HeaderKind::SourceManifest => {
                    hir.manifest_path =
                        fidryn_syntax::decode_string_literal(&h.value).unwrap_or_default();
                }
                fidryn_syntax::ast::HeaderKind::OutsideScope => {
                    hir.outside_scope = fidryn_syntax::lexer::lex(&h.value)
                        .iter()
                        .filter_map(|token| {
                            let raw = &h.value[token.start as usize..token.end as usize];
                            match token.kind {
                                fidryn_syntax::lexer::TokenKind::Ident => Some(raw.to_owned()),
                                fidryn_syntax::lexer::TokenKind::String => {
                                    fidryn_syntax::decode_string_literal(raw).ok()
                                }
                                _ => None,
                            }
                        })
                        .collect();
                }
                fidryn_syntax::ast::HeaderKind::EffectiveAt => {
                    hir.effective_at = h.value.clone();
                }
                fidryn_syntax::ast::HeaderKind::RecordedAt => {
                    hir.recorded_at = h.value.clone();
                }
            },
            fidryn_syntax::ast::Item::Import(d) => {
                let (name, type_args) = resolve_import_type_args(d);
                hir.imports.push(HirImport {
                    name,
                    type_args,
                    digest_required: body::import_requires_digest(&d.source),
                    digest: body::import_digest(&d.source),
                    version: body::import_version(&d.source),
                });
            }
            fidryn_syntax::ast::Item::Source(d) => {
                hir.sources.push(HirSource {
                    name: d.name.clone().unwrap_or_default(),
                    artifact: source_artifact(&d.source),
                });
            }
            fidryn_syntax::ast::Item::Entity(d) => {
                if let Some(name) = &d.name {
                    let ty = without_comments(d.signature.as_deref().unwrap_or(""))
                        .trim_start_matches(':')
                        .trim()
                        .trim_end_matches(';')
                        .trim()
                        .to_owned();
                    hir.entities.insert(name.clone(), ty);
                }
            }
            fidryn_syntax::ast::Item::Proposition(d) => {
                if let Some(name) = &d.name {
                    hir.propositions.insert(name.clone(), decl_params(d));
                }
            }
            fidryn_syntax::ast::Item::Office(d) => {
                if let Some(name) = &d.name {
                    hir.callable_signatures
                        .insert(name.clone(), (decl_params(d), "Office".into()));
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
                let automatic = d.automatic;
                let effects = d.effects.clone();
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
                            fuel: d.fuel,
                            result_type: d
                                .result_type
                                .clone()
                                .unwrap_or_else(|| extract_return_type(&d.source)),
                            source: d.source.clone(),
                            effects: d.effects.clone(),
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
                let parsed = body::parse_rule_parts(&d.source);
                let mut guard = d.guard.as_ref().map(body::expr_to_guard).or(parsed.guard);
                if let Some(req) = d.require.as_ref() {
                    guard = Some(body::and_opt_guard(guard, body::expr_to_guard(req)));
                } else if let Some(req) = parsed.require {
                    guard = Some(body::and_opt_guard(guard, req));
                }
                let (consequences, consequences_not_propositions) = if d.consequences.is_empty() {
                    (parsed.consequences, parsed.consequences_not_propositions)
                } else {
                    body::consequences_from_ast(&d.consequences)
                };
                let (fallback, fallback_not_propositions) = if d.fallback.is_empty() {
                    (parsed.fallback, parsed.fallback_not_propositions)
                } else {
                    body::consequences_from_ast(&d.fallback)
                };
                // A consequence whose expression is not a proposition is in
                // neither list. It is reported: a rule that lost it would
                // establish less than its text says.
                let rule_name = d.name.as_deref().unwrap_or_default();
                for (word, texts) in [
                    ("then", &consequences_not_propositions),
                    ("otherwise", &fallback_not_propositions),
                ] {
                    for text in texts {
                        let text = without_comments(text)
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ");
                        hir.diagnostics.push(
                            Diagnostic::new(
                                DiagnosticCode::E210,
                                format!(
                                    "`{word} {text}` in rule `{rule_name}` requires a proposition"
                                ),
                            )
                            .with_span(d.span),
                        );
                    }
                }
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
                    binders: d.params.iter().map(|(name, _)| name.clone()).collect(),
                    typed_binders: d.params.clone(),
                    guard_terms: d
                        .guard
                        .iter()
                        .chain(d.require.iter())
                        .map(body::expr_to_term)
                        .collect(),
                    guard,
                    selection: d.selection.as_ref().map(|selection| HirSelection {
                        binder: selection.binder.clone(),
                        finite_domain: body::expr_to_term(&selection.finite_domain),
                        predicate: body::expr_to_term(&selection.predicate),
                        ordering: selection.ordering.clone(),
                        ascending: selection.ascending.clone(),
                        require_unique_keys: selection.require_unique_keys,
                    }),
                    consequences,
                    fallback,
                });
            }
            fidryn_syntax::ast::Item::Duty(d) => {
                let name = d.name.clone().unwrap_or_default();
                if !name.is_empty() {
                    let parts = body::parse_duty_parts(&d.source);
                    if parts.invalid {
                        hir.diagnostics.push(Diagnostic::new(DiagnosticCode::E100, format!("duty `{name}` contains an invalid or over-budget attachment/deadline expression")).with_span(d.span));
                    }
                    hir.duties.push(HirDuty {
                        name,
                        params: decl_params(d),
                        bearer: parts.bearer,
                        bearer_term: parts.bearer_term,
                        claimant: parts.claimant,
                        claimant_term: parts.claimant_term,
                        attaches: parts.attaches,
                        attaches_term: parts.attaches_term,
                        content: parts.content,
                        due: parts.due,
                    });
                }
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
                let result = extract_returns(&d.source).unwrap_or_else(|()| {
                    hir.diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::E100,
                            "decision return expression is invalid or exceeds resource limits",
                        )
                        .with_span(d.span),
                    );
                    None
                });
                hir.decisions.push(HirDecision {
                    name: d.name.clone().unwrap_or_default(),
                    requirements: extract_record_requires(&d.source),
                    result,
                    params: d.params.clone(),
                    result_type: d.result_type.clone(),
                });
            }
            fidryn_syntax::ast::Item::Judgment(d) => {
                if let Some(judgment) = judgment_from_decl(d, &mut hir.diagnostics) {
                    hir.judgments.push(judgment);
                }
                hir.retained.push(HirRetainedDecl {
                    kind: d.keyword.clone(),
                    name: d.name.clone().unwrap_or_default(),
                    source: d.source.clone(),
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
                    hir.callable_signatures
                        .insert(name.clone(), (d.params.clone(), "ClauseRef".into()));
                    hir.clauses.insert(name.clone(), d.source.clone());
                }
            }
            fidryn_syntax::ast::Item::RecordType(d) => {
                hir.types.remove(d.name.as_deref().unwrap_or_default());
                hir.record_types.insert(
                    d.name.clone().unwrap_or_default(),
                    (
                        d.keyword == "evidence_type",
                        d.fields
                            .iter()
                            .map(|(name, ty)| (name.clone(), ty.clone()))
                            .collect(),
                    ),
                );
                hir.retained.push(HirRetainedDecl {
                    kind: d.keyword.clone(),
                    name: d.name.clone().unwrap_or_default(),
                    source: d.source.clone(),
                });
            }
            fidryn_syntax::ast::Item::Type(d) => {
                let name = d.name.clone().unwrap_or_default();
                hir.record_types.remove(&name);
                if let Some(definition) = type_definition_from_decl(d) {
                    let parameters = d
                        .type_args
                        .iter()
                        .map(|parameter| without_comments(parameter).into_owned())
                        .collect();
                    hir.types.insert(name.clone(), (parameters, definition));
                }
                if let Some(fidryn_syntax::ast::TypeDefinitionAst::Sum(variants)) =
                    &d.type_definition
                {
                    for variant in variants {
                        if !variant.fields.is_empty() {
                            hir.record_types
                                .insert(variant.name.clone(), (false, variant.fields.clone()));
                        }
                    }
                }
                hir.retained.push(HirRetainedDecl {
                    kind: d.keyword.clone(),
                    name,
                    source: d.source.clone(),
                });
            }
            fidryn_syntax::ast::Item::LegalAct(d) => {
                let name = d.name.clone().unwrap_or_default();
                hir.callable_signatures
                    .insert(name.clone(), (decl_params(d), "LegalAction".into()));
                hir.retained.push(HirRetainedDecl {
                    kind: d.keyword.clone(),
                    name,
                    source: d.source.clone(),
                });
            }
            fidryn_syntax::ast::Item::Observation(d) => {
                let name = d.name.clone().unwrap_or_default();
                if let Some(observation) = &d.observation {
                    if let Some(establishes) =
                        body::term_as_prop(&body::expr_to_term(&observation.establishes))
                    {
                        hir.observations.push(HirObservation {
                            name: name.clone(),
                            params: d.params.clone(),
                            establishes,
                            validators: observation
                                .validators
                                .iter()
                                .map(body::expr_to_term)
                                .collect(),
                            source: d.source_basis.as_ref().map(body::expr_to_term),
                            unsupported_clauses: observation.unsupported_clauses.clone(),
                        });
                    } else {
                        hir.diagnostics.push(
                            Diagnostic::new(
                                DiagnosticCode::E210,
                                "observation must establish a proposition",
                            )
                            .with_span(d.span),
                        );
                    }
                }
                hir.retained.push(HirRetainedDecl {
                    kind: d.keyword.clone(),
                    name,
                    source: d.source.clone(),
                });
            }
            fidryn_syntax::ast::Item::Power(d) | fidryn_syntax::ast::Item::Scenario(d) => {
                hir.retained.push(HirRetainedDecl {
                    kind: d.keyword.clone(),
                    name: d.name.clone().unwrap_or_default(),
                    source: d.source.clone(),
                });
            }
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

fn type_definition_from_decl(
    d: &fidryn_syntax::ast::Decl,
) -> Option<fidryn_core::ir::TypeDefinition> {
    use fidryn_core::ir::{CoreConstructor, TypeDefinition};
    Some(match d.type_definition.as_ref()? {
        fidryn_syntax::ast::TypeDefinitionAst::Alias(ty) => {
            TypeDefinition::Alias(parse_type_name(ty))
        }
        fidryn_syntax::ast::TypeDefinitionAst::Sum(variants) => TypeDefinition::Sum(
            variants
                .iter()
                .map(|variant| CoreConstructor {
                    name: variant.name.clone(),
                    arguments: variant
                        .arguments
                        .iter()
                        .map(|ty| parse_type_name(ty))
                        .collect(),
                    fields: variant
                        .fields
                        .iter()
                        .map(|(name, ty)| (name.clone(), parse_type_name(ty)))
                        .collect(),
                })
                .collect(),
        ),
    })
}

fn standard_type_environment() -> &'static (HirTypes, HirRecordTypes) {
    static STANDARD: std::sync::OnceLock<(HirTypes, HirRecordTypes)> = std::sync::OnceLock::new();
    STANDARD.get_or_init(|| {
        let parsed = fidryn_syntax::parse_file(include_str!("../../../prelude/prelude.fr"));
        assert!(
            !parsed.has_errors(),
            "invalid standard type environment: {:?}",
            parsed.diagnostics
        );
        let mut types = BTreeMap::new();
        let mut records = BTreeMap::new();
        for item in &parsed.module().expect("standard module").items {
            match item {
                fidryn_syntax::ast::Item::Type(d) => {
                    if let Some(definition) = type_definition_from_decl(d) {
                        types.insert(
                            d.name.clone().unwrap_or_default(),
                            (d.type_args.clone(), definition),
                        );
                    }
                }
                fidryn_syntax::ast::Item::RecordType(d) => {
                    records.insert(
                        d.name.clone().unwrap_or_default(),
                        (
                            d.keyword == "evidence_type",
                            d.fields
                                .iter()
                                .map(|(name, ty)| (name.clone(), ty.clone()))
                                .collect(),
                        ),
                    );
                }
                _ => {}
            }
        }
        (types, records)
    })
}

fn judgment_from_decl(
    d: &fidryn_syntax::ast::Decl,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<HirJudgment> {
    use fidryn_core::ir::{JudgmentAction, JudgmentAuthority, JudgmentFallback, JudgmentReview};
    use fidryn_syntax::ast::JudgmentAuthorityAst;
    let parsed = d.judgment.as_ref()?;
    let Some(decides) = term_as_prop(&body::expr_to_term(&parsed.decides)) else {
        diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::E210,
                "judgment decides requires a proposition",
            )
            .with_span(d.span),
        );
        return None;
    };
    let authority = match parsed.authority.as_ref()? {
        JudgmentAuthorityAst::Actor(actor) => JudgmentAuthority::Actor(body::expr_to_term(actor)),
        JudgmentAuthorityAst::Concurrence {
            minimum,
            distinct,
            issuer_type,
        } => JudgmentAuthority::Concurrence {
            minimum: *minimum,
            distinct: *distinct,
            issuer_type: parse_type_name(issuer_type),
        },
    };
    let otherwise = if let Some((issue, protocol)) = &parsed.otherwise {
        let Some(issue) = term_as_prop(&body::expr_to_term(issue)) else {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::E210,
                    "judgment suspension issue requires a proposition",
                )
                .with_span(d.span),
            );
            return None;
        };
        Some(JudgmentFallback::Suspend {
            issue,
            protocol: body::expr_to_term(protocol),
        })
    } else {
        None
    };
    Some(HirJudgment {
        name: d.name.clone().unwrap_or_default(),
        params: d.params.clone(),
        decides,
        source: d.source_basis.as_ref().map(body::expr_to_term),
        authority,
        records: parsed.records.clone(),
        record_patterns: parsed
            .record_patterns
            .iter()
            .map(body::expr_to_term)
            .collect(),
        constraints: parsed.constraints.iter().map(body::expr_to_term).collect(),
        establish_when: parsed.establish_when.as_ref().map(body::expr_to_term),
        reject_when: parsed.reject_when.as_ref().map(body::expr_to_term),
        otherwise,
        on_established: parsed
            .on_established
            .iter()
            .map(|(issue, context)| JudgmentAction::MakeOperative {
                issue: body::expr_to_term(issue),
                context: body::expr_to_term(context),
            })
            .collect(),
        standard: parsed.standard.as_ref().map(body::expr_to_term),
        review: parsed.review.as_ref().map(|(by, standard)| JudgmentReview {
            by: body::expr_to_term(by),
            standard: body::expr_to_term(standard),
        }),
        unsupported_clauses: parsed.unsupported_clauses.clone(),
    })
}

fn decl_params(d: &fidryn_syntax::ast::Decl) -> Vec<(String, String)> {
    if d.params.is_empty() {
        body::extract_params(&d.source)
    } else {
        d.params.clone()
    }
}

/// The text of a declaration as its readers take it: the text with its
/// comments blank, which they cut what they read out of, and its structure,
/// which they search for the place. A position in one is the same position
/// in the other.
fn views(src: &str) -> (Cow<'_, str>, Cow<'_, str>) {
    (without_comments(src), structure_of(src))
}

/// The stretch of `structure` after the first `word` in it, up to the next
/// `word` or the end. Splitting the text on the word gives this stretch as
/// its second piece.
fn after_first(structure: &str, word: &str) -> Option<Range<usize>> {
    let start = structure.find(word)? + word.len();
    let end = structure[start..]
        .find(word)
        .map_or(structure.len(), |next| start + next);
    Some(start..end)
}

/// The value of the `artifact` field of a `source`: what stands after the
/// word, from the quote that follows it to the next quote. The word is not
/// found in a comment or inside a string literal.
fn source_artifact(src: &str) -> Option<String> {
    let (code, structure) = views(src);
    let value = code[after_first(&structure, "artifact")?]
        .trim()
        .trim_start_matches('"');
    let end = value.find('"')?;
    Some(value[..end].to_owned())
}

fn query_body_has_quantifier_or_require(src: &str) -> bool {
    let inner = last_brace_inner(src).unwrap_or("");
    let structure = structure_of(inner);
    !inner.is_empty()
        && (contains_ident(&structure, "require")
            || contains_ident(&structure, "for_all")
            || contains_ident(&structure, "exists")
            || contains_ident(&structure, "goal")
            || contains_ident(&structure, "return"))
}

fn extract_return_type(src: &str) -> String {
    let (code, structure) = views(src);
    let Some(after_arrow) = after_first(&structure, "->") else {
        return String::new();
    };
    let mut ty = &structure[after_arrow.clone()];
    if let Some(i) = ty.find('!') {
        ty = &ty[..i];
    }
    if let Some(i) = ty.find('{') {
        ty = &ty[..i];
    }
    if let Some(i) = find_ident(ty, "fuel") {
        ty = &ty[..i];
    }
    code[after_arrow.start..after_arrow.start + ty.len()]
        .trim()
        .to_owned()
}

fn extract_rank(src: &str) -> i64 {
    let (code, structure) = views(src);
    after_first(&structure, "rank")
        .and_then(|after_rank| {
            code[after_rank]
                .split_whitespace()
                .next()
                .and_then(|n| n.trim_matches(';').parse().ok())
        })
        .unwrap_or(0)
}

fn extract_office(src: &str) -> String {
    let (code, structure) = views(src);
    let Some(after_for) = after_first(&structure, " for ") else {
        return String::new();
    };
    let end = structure[after_for.clone()]
        .find(" rank")
        .map_or(after_for.end, |rank| after_for.start + rank);
    code[after_for.start..end].trim().to_owned()
}

#[allow(dead_code)]
fn extract_alternatives(src: &str) -> Vec<String> {
    extract_alternative_defs(src)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

fn extract_alternative_defs(src: &str) -> InterpretationAlts {
    const ALTERNATIVE: &str = "alternative ";
    const DEFINES: &str = "defines ";
    let (code, structure) = views(src);
    let mut out = Vec::new();
    // An alternative is the text after the word, up to the next one.
    let starts: Vec<usize> = structure
        .match_indices(ALTERNATIVE)
        .map(|(word, _)| word + ALTERNATIVE.len())
        .collect();
    for (index, &start) in starts.iter().enumerate() {
        let end = starts
            .get(index + 1)
            .map_or(structure.len(), |next| next - ALTERNATIVE.len());
        let name = code[start..end]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_matches('{')
            .to_owned();
        if name.is_empty() {
            continue;
        }
        let mut defs = Vec::new();
        let mut rest = start;
        while let Some(idx) = structure[rest..end].find(DEFINES) {
            // A definition is the text after the word, up to the next one.
            let stmt_start = rest + idx + DEFINES.len();
            let stmt_end = structure[stmt_start..end]
                .find(DEFINES)
                .map_or(end, |next| stmt_start + next);
            let stmt = &structure[stmt_start..stmt_end];
            let defined = match stmt.find(" as not_established") {
                Some(expr_end) => Some((expr_end, false)),
                None => stmt
                    .find(" as established")
                    .map(|expr_end| (expr_end, true)),
            };
            if let Some((expr_end, established)) = defined {
                let expr = code[stmt_start..stmt_start + expr_end].trim();
                if let Some(prop) = body::parse_expr_src(expr).and_then(|t| body::term_as_prop(&t))
                {
                    defs.push((prop, established));
                }
            }
            rest = stmt_end;
        }
        out.push((name, defs));
    }
    out
}

fn extract_record_requires(src: &str) -> Vec<String> {
    body::parse_record_requirements(src)
}

fn extract_returns(src: &str) -> Result<Option<Term>, ()> {
    body::parse_decl_result(src)
}

fn extract_effect_result(src: &str) -> String {
    let (code, structure) = views(src);
    let after_arrow = structure.rfind("->").map_or(0, |arrow| arrow + "->".len());
    code[after_arrow..]
        .trim()
        .trim_end_matches('}')
        .trim()
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_owned()
}

fn verification_from_decl(d: &fidryn_syntax::ast::Decl) -> Option<HirVerification> {
    if d.keyword != "verify" {
        return None;
    }
    let name = d.name.as_ref().filter(|n| !n.is_empty())?.clone();
    Some(HirVerification {
        // A block the parser read no assertion from gets no formula. Its
        // text is not searched for one: the search would find the word
        // `assert` in a comment and take the comment for the assertion.
        formula: if d.assertions.is_empty() {
            String::new()
        } else {
            verify_formula(d)
        },
        assertions: d.assertions.iter().map(body::expr_to_term).collect(),
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

/// The bounds of a verify block: the three numbers in the `bounds` field the
/// parser recorded. That record is the text of the field's tokens, and it is
/// read token by token, so no comment and no string sets a number.
fn verify_bounds(d: &fidryn_syntax::ast::Decl) -> Option<HirVerificationBounds> {
    let text = d.fields.get("bounds")?;
    let tokens: Vec<(TokenKind, &str)> = lex(text)
        .iter()
        .filter(|token| {
            !matches!(
                token.kind,
                TokenKind::Whitespace
                    | TokenKind::Comment
                    | TokenKind::DocComment
                    | TokenKind::Newline
                    | TokenKind::Eof
            )
        })
        .map(|token| (token.kind, &text[token.start as usize..token.end as usize]))
        .collect();
    Some(HirVerificationBounds {
        persons: bound_in(&tokens, "persons"),
        events: bound_in(&tokens, "events"),
        time_points: bound_in(&tokens, "time_points"),
    })
}

/// The number the tokens of a `bounds` field give for `word`. The first
/// token that is the word decides: the number is the integer token that
/// follows it, directly or after a `:`. It is zero when no integer token
/// follows there, when the integer does not fit in 32 bits and when the word
/// is not among the tokens.
fn bound_in(tokens: &[(TokenKind, &str)], word: &str) -> u32 {
    let mut after = tokens
        .iter()
        .skip_while(|(kind, text)| *kind != TokenKind::Ident || *text != word)
        .skip(1);
    let number = match after.next() {
        Some((TokenKind::Colon, _)) => after.next(),
        token => token,
    };
    match number {
        Some((TokenKind::Int, digits)) => digits.parse().unwrap_or(0),
        _ => 0,
    }
}

/// The text of a declaration from the word `assert` to the end of that
/// clause, as it is written. The word and the end of the clause are looked
/// for in the structure of the text, so neither is found in a comment or
/// inside a string literal.
fn extract_assert_clause(src: &str) -> Option<String> {
    let structure = structure_of(src);
    let i = find_ident(&structure, "assert")?;
    Some(take_clause_to_end(&src[i..], &structure[i..]))
}

/// `s` up to the end of its first clause: a `;` or a `}` that closes
/// nothing, outside every bracket of `structure`, which is the structure of
/// `s`.
fn take_clause_to_end(s: &str, structure: &str) -> String {
    let mut depth_paren = 0i32;
    let mut depth_brack = 0i32;
    let mut depth_brace = 0i32;
    for (i, c) in structure.char_indices() {
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

/// The quantifiers written in the text of a declaration. A quantifier in a
/// comment or inside a string literal is none.
fn extract_quantifiers(src: &str) -> Vec<HirQuantifier> {
    let (code, structure) = views(src);
    let mut out = Vec::new();
    let mut i = 0;
    while i < structure.len() {
        if let Some((kind, kw_len)) = quantifier_at(&structure, i) {
            let after = i + kw_len;
            if let Some(q) = parse_quantifier_tail(kind, &code[after..], &structure[after..]) {
                out.push(q);
            }
            i += kw_len;
        } else {
            i += structure[i..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
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

/// The quantifier whose text after its keyword is `code`, with the
/// structure of that text beside it. The variable and the word `in` hold no
/// string, so they are read from the structure, where they stand as they
/// are written.
fn parse_quantifier_tail(
    kind: QuantifierKind,
    code: &str,
    structure: &str,
) -> Option<HirQuantifier> {
    let rest = structure.trim_start();
    let binder_len = ident_len(rest)?;
    let binder = rest[..binder_len].to_owned();
    let after_binder = rest[binder_len..].trim_start();
    if !after_binder.starts_with("in") || !is_ident_boundary(after_binder, 0, 2) {
        return None;
    }
    let after_in = after_binder[2..].trim_start();
    let (domain, formula) =
        split_domain_formula(&code[structure.len() - after_in.len()..], after_in)?;
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

/// The domain and the formula of a quantifier, cut out of `s`, the text
/// after its word `in`. The `:` or the `{` between the two is looked for in
/// `structure`, the structure of `s`.
fn split_domain_formula(s: &str, structure: &str) -> Option<(String, String)> {
    let mut depth_paren = 0i32;
    let mut depth_brack = 0i32;
    let mut depth_brace = 0i32;
    for (i, c) in structure.char_indices() {
        match c {
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '[' => depth_brack += 1,
            ']' => depth_brack -= 1,
            '{' => {
                if depth_paren == 0 && depth_brack == 0 && depth_brace == 0 {
                    let domain = s[..i].trim().to_owned();
                    let inner = brace_inner(&s[i..], &structure[i..])
                        .unwrap_or("")
                        .trim()
                        .to_owned();
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

/// What stands between the first `{` of `s` and the `}` that closes it,
/// where `structure` is the structure of `s`.
fn brace_inner<'a>(s: &'a str, structure: &str) -> Option<&'a str> {
    if !structure.starts_with('{') {
        return None;
    }
    let mut depth = 0i32;
    for (i, c) in structure.char_indices() {
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
///
/// A comment in the text is read as white space, and a `<`, a `>` or a `,`
/// inside a string literal is not counted.
pub fn split_qname_type_args(s: &str) -> (String, Vec<String>) {
    let code = without_comments(s);
    let s = code.trim();
    let Some(open) = structure_of(s).find('<') else {
        return (s.to_owned(), Vec::new());
    };
    let name = s[..open].trim().to_owned();
    (name, parse_leading_angle_args(&s[open..]))
}

/// Parse the first `<...>` argument list in `s`, respecting nested angles.
///
/// A comment in the text is read as white space, and a `<`, a `>` or a `,`
/// inside a string literal is not counted.
pub fn parse_angle_args(s: &str) -> Vec<String> {
    let code = without_comments(s);
    let Some(open) = structure_of(&code).find('<') else {
        return Vec::new();
    };
    parse_leading_angle_args(&code[open..])
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
    let source = without_comments(source);
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

/// The arguments between the `<` that `s` begins with and the `>` that
/// closes it. A comment in between is read as white space, and a `<`, a `>`
/// or a `,` inside a string literal is not counted.
fn parse_leading_angle_args(s: &str) -> Vec<String> {
    let s = s.trim_start();
    if !s.starts_with('<') {
        return Vec::new();
    }
    let (code, structure) = views(s);
    let mut depth = 0i32;
    let mut end = None;
    for (i, c) in structure.char_indices() {
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
    split_top_level_commas(&code[1..end])
}

/// The parts of `inner` between the commas that stand inside no bracket. A
/// bracket or a comma inside a string literal is not counted.
fn split_top_level_commas(inner: &str) -> Vec<String> {
    let structure = structure_of(inner);
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth_paren = 0i32;
    let mut depth_angle = 0i32;
    let mut depth_brack = 0i32;
    let mut depth_brace = 0i32;
    for (i, c) in structure.char_indices() {
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
        assert!(hir.rules[0].fallback.is_empty());
        assert!(hir.rules[0].binders.is_empty());
    }

    #[test]
    fn elaborates_false_guard_require_otherwise_and_binders() {
        let src = r#"
module Examples.RuleGuards version "0.1.0" {
    rule R(x: Person) : derive {
        when false
        require true
        then derive P()
        otherwise derive Q()
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(hir.rules.len(), 1);
        let rule = &hir.rules[0];
        assert_eq!(rule.binders, vec!["x".to_owned()]);
        assert_eq!(
            rule.guard,
            Some(Guard::Not(Box::new(Guard::Satisfied))),
            "{:?}",
            rule.guard
        );
        assert_eq!(rule.consequences.len(), 1);
        assert_eq!(rule.consequences[0].1.predicate, "P");
        assert_eq!(rule.fallback.len(), 1);
        assert_eq!(rule.fallback[0].1.predicate, "Q");
    }

    #[test]
    fn elaborates_require_false_into_guard() {
        let src = r#"
module Examples.RuleRequire version "0.1.0" {
    rule R : derive { when true require false then derive P() }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(
            hir.rules[0].guard,
            Some(Guard::Not(Box::new(Guard::Satisfied))),
            "{:?}",
            hir.rules[0].guard
        );
        assert_eq!(hir.rules[0].consequences[0].1.predicate, "P");
        assert!(hir.rules[0].fallback.is_empty());
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
        assert_eq!(hir.imports[0].version.as_deref(), Some("0.1.0"));
        assert_eq!(hir.imports[0].digest, None);
        assert!(!hir.imports[0].digest_required);
        assert!(hir.type_params.is_empty());
    }

    #[test]
    fn elaborates_import_digest_and_version() {
        let src = r#"
module Host version "0.1.0" {
    import Other.Law version "1" { digest "fixture" }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(hir.imports.len(), 1);
        assert_eq!(hir.imports[0].name, "Other.Law");
        assert_eq!(hir.imports[0].version.as_deref(), Some("1"));
        assert_eq!(hir.imports[0].digest.as_deref(), Some("fixture"));
        assert!(hir.imports[0].digest_required);
        assert!(hir.imports[0].type_args.is_empty());
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

    fn expect_transaction_args(term: &Term) -> &[Term] {
        match term {
            Term::Apply { ctor, args } if ctor == "transaction" => args,
            other => panic!("{other:?}"),
        }
    }

    fn query_result_term(body: &HirQueryBody) -> &Term {
        match body {
            HirQueryBody::Return(term) => term,
            HirQueryBody::Goal {
                expr: Some(term), ..
            } => term,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn elaborates_transaction_block_as_apply_steps() {
        let src = r#"
module Examples.Tx version "0.1.0" {
    query q() -> Bool {
        transaction {
            duty_step(pay, attach);
            duty_step(pay, discharge)
        }
    }
}
"#;
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let args = expect_transaction_args(query_result_term(&hir.queries["q"].body));
        assert_eq!(args.len(), 2, "{args:?}");
        assert_eq!(
            args[0],
            Term::Call {
                callee: "duty_step".into(),
                args: vec![Term::Ident("pay".into()), Term::Ident("attach".into())],
            }
        );
        assert_eq!(
            args[1],
            Term::Call {
                callee: "duty_step".into(),
                args: vec![Term::Ident("pay".into()), Term::Ident("discharge".into())],
            }
        );
        assert!(
            !args
                .iter()
                .any(|arg| matches!(arg, Term::Apply { ctor, .. } if ctor == "seq"))
        );
    }

    #[test]
    fn elaborates_transaction_atomic_program() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/programs/transaction-atomic.fr");
        let src = std::fs::read_to_string(&path).expect("transaction-atomic.fr");
        let parsed = parse_file(&src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let args = expect_transaction_args(query_result_term(&hir.queries["q"].body));
        assert_eq!(args.len(), 2, "{args:?}");
        assert_eq!(
            args[0],
            Term::Call {
                callee: "duty_step".into(),
                args: vec![Term::Ident("pay".into()), Term::Ident("attach".into())],
            }
        );
        assert_eq!(
            args[1],
            Term::Call {
                callee: "duty_step".into(),
                args: vec![Term::Ident("pay".into()), Term::Ident("discharge".into())],
            }
        );
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
    fn a_verify_block_with_no_assertion_has_no_formula() {
        let src = r#"
module Examples.Comment version "0.1.0" {
    verify OnlyAComment {
        bounds { persons: 2 }
        assuming fair
        // assert true
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(hir.verifications.len(), 1, "{:?}", hir.verifications);
        let v = &hir.verifications[0];
        assert!(v.assertions.is_empty(), "{:?}", v.assertions);
        // The text of the block is not searched for an assertion.
        assert_eq!(v.formula, "");
        assert_eq!(v.bounds.as_ref().map(|bounds| bounds.persons), Some(2));
    }

    #[test]
    fn the_bounds_of_a_verify_block_come_only_from_a_bounds_field() {
        let src = r#"
module Examples.Bounds version "0.1.0" {
    verify InAComment {
        // bounds { persons: 2 }
        assert true
    }
    verify InAnotherField {
        assuming bounds persons 3
        assert true
    }
    verify Braces {
        bounds { persons: 2 }
        assert true
    }
    verify Words {
        bounds persons 2 events 1 time_points 1
        assert true
    }
    verify BraceOnTheNextLine {
        bounds
        { persons: 3 }
        assert true
    }
    verify TwoFields {
        bounds { persons: 2 }
        bounds { events: 1 }
        assert true
    }
    verify CommentBelow {
        bounds { events: 1 }
        // bounds { persons: 2 }
        assert true
    }
    verify CommentBelowTheLastField {
        assert true
        bounds { events: 1 }
        // note: persons 2 is enough
    }
    verify CommentOnTheLine {
        bounds events 1 // persons: 2
        assert true
    }
    verify CommentOnTheNextLine {
        bounds events 1
        // persons: 2
        assert true
    }
    verify CommentBelowABlankLine {
        bounds { events: 1 }

        // persons: 2

        assert true
    }
    verify CommentAboveAnotherField {
        bounds { events: 1 }
        // persons: 2
        assuming fair
        assert true
    }
    verify CommentInsideTheBraces {
        bounds {
            // persons: 9
            persons: 2
        }
        assert true
    }
    verify CommentAfterAComma {
        bounds { events: 1, // persons: 5
            persons: 2 }
        assert true
    }
    verify InAString {
        bounds { label: "persons: 2" }
        assert true
    }
    verify Decimal {
        bounds { persons: 2.5 }
        assert true
    }
    verify SecondOccurrence {
        bounds persons x persons 2
        assert true
    }
    verify TooLarge {
        bounds { persons: 99999999999 }
        assert true
    }
    verify OneColon {
        bounds persons : 2
        assert true
    }
    verify TwoColons {
        bounds persons :: 2
        assert true
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        let bounds: Vec<_> = hir
            .verifications
            .iter()
            .map(|verification| {
                (
                    verification.name.as_str(),
                    verification
                        .bounds
                        .as_ref()
                        .map(|bounds| (bounds.persons, bounds.events, bounds.time_points)),
                )
            })
            .collect();
        assert_eq!(
            bounds,
            [
                ("InAComment", None),
                ("InAnotherField", None),
                ("Braces", Some((2, 0, 0))),
                ("Words", Some((2, 1, 1))),
                ("BraceOnTheNextLine", Some((3, 0, 0))),
                // The last `bounds` field counts, and a number it does not
                // give is zero.
                ("TwoFields", Some((0, 1, 0))),
                // A comment sets nothing: not below the field, not on its
                // line and not inside it.
                ("CommentBelow", Some((0, 1, 0))),
                ("CommentBelowTheLastField", Some((0, 1, 0))),
                ("CommentOnTheLine", Some((0, 1, 0))),
                ("CommentOnTheNextLine", Some((0, 1, 0))),
                ("CommentBelowABlankLine", Some((0, 1, 0))),
                ("CommentAboveAnotherField", Some((0, 1, 0))),
                ("CommentInsideTheBraces", Some((2, 0, 0))),
                ("CommentAfterAComma", Some((2, 1, 0))),
                // The contents of a string are not read, and a number is an
                // integer token.
                ("InAString", Some((0, 0, 0))),
                ("Decimal", Some((0, 0, 0))),
                // The first place where a word stands decides, and an
                // integer that does not fit in 32 bits gives zero.
                ("SecondOccurrence", Some((0, 0, 0))),
                ("TooLarge", Some((0, 0, 0))),
                // The integer follows its word directly or after one `:`.
                ("OneColon", Some((2, 0, 0))),
                ("TwoColons", Some((0, 0, 0))),
            ]
        );
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

    #[test]
    fn elaborates_late_payment_style_duty() {
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
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(
            hir.propositions.get("InvoiceIssued").map(Vec::as_slice),
            Some([("person".to_owned(), "NaturalPerson".to_owned())].as_slice())
        );
        assert_eq!(hir.duties.len(), 1, "{:?}", hir.duties);
        let duty = &hir.duties[0];
        assert_eq!(duty.name, "PayInvoice");
        assert_eq!(duty.bearer, "Payer");
        assert_eq!(duty.claimant.as_deref(), Some("Payee"));
        assert!(
            matches!(
                &duty.attaches,
                Some(Guard::Operative(prop, _)) if prop.predicate == "InvoiceIssued"
            ),
            "{:?}",
            duty.attaches
        );
        assert_eq!(
            duty.content,
            vec![Term::Apply {
                ctor: "USD".into(),
                args: vec![Term::Decimal(
                    rust_decimal::Decimal::from_str("100.00").expect("decimal")
                )],
            }]
        );
        match &duty.due {
            Some(Term::Apply { ctor, args }) if ctor == "after" && args.len() == 2 => {
                match &args[0] {
                    Term::Apply { ctor, args: unit } if ctor == "counted_days" => {
                        assert_eq!(unit.as_slice(), &[Term::Int(0)]);
                    }
                    other => panic!("{other:?}"),
                }
                assert_eq!(args[1], Term::Ident("invoice_date".into()));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn retains_parsed_surface_constructs() {
        let src = r#"
module Examples.Retain version "0.1.0" {
    effective_at 2026-01-01
    recorded_at 2026-01-01T00:00:00Z
    type TaxYear = Int
    record_type TaxableIncome { amount: Money<USD> }
    observation Filing(record: TaxableIncome) establishes FilingReceived(record) { require true }
    power Amend { holder Settlor }
    judgment Capacity(person: NaturalPerson) decides Issued(person) { authority Court }
    legal_act Sign(doc: TaxableIncome)
    scenario HappyPath { assume true }
    proposition Issued(person: NaturalPerson)
    query q() -> Bool { return true }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
        assert_eq!(hir.effective_at, "2026-01-01");
        assert_eq!(hir.recorded_at, "2026-01-01T00:00:00Z");
        assert_eq!(
            hir.propositions.get("Issued").map(Vec::as_slice),
            Some([("person".to_owned(), "NaturalPerson".to_owned())].as_slice())
        );
        let kinds: Vec<&str> = hir.retained.iter().map(|d| d.kind.as_str()).collect();
        for kind in [
            "type",
            "record_type",
            "observation",
            "power",
            "judgment",
            "legal_act",
            "scenario",
        ] {
            assert!(
                kinds.contains(&kind),
                "expected retained `{kind}`, got {kinds:?}"
            );
        }
        assert!(
            hir.retained
                .iter()
                .any(|d| d.kind == "type" && d.name == "TaxYear"),
            "{:?}",
            hir.retained
        );
        assert!(
            hir.retained
                .iter()
                .any(|d| d.kind == "scenario" && d.name == "HappyPath"),
            "{:?}",
            hir.retained
        );
    }

    /// The codes and messages the lowering rejects a module with. The
    /// module has no parse error.
    fn rejected(src: &str) -> Vec<(DiagnosticCode, String)> {
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        elaborate(&parsed, &SourceManifest::default())
            .expect_err("the module is rejected")
            .into_iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message))
            .collect()
    }

    /// A module with one rule, `Exemption`, that has these statements.
    fn rule_module(statements: &str) -> String {
        format!(
            "module Examples.Consequence version \"0.1.0\" {{\n    entity Acme : LegalPerson\n    proposition Exempt(who: LegalPerson)\n    rule Exemption : constitutive {{\n        when true\n        {statements}\n    }}\n}}\n"
        )
    }

    #[test]
    fn a_consequence_that_is_not_a_proposition_is_e210_and_names_the_rule_and_the_consequence() {
        // The expression of the consequence is a number, a string, a sum, a
        // proposition under a modal word, a set, a field of a name, or a
        // number in parentheses. None of these is a proposition. Such a
        // consequence was dropped without a diagnostic, and the rule
        // established less than its text says.
        for written in [
            "5",
            "\"x\"",
            "1 + 2",
            "operative Exempt(Acme)",
            "{Acme}",
            "Acme.name",
            "(5)",
        ] {
            // In the `then` list and in the `otherwise` list.
            for (word, statements) in [
                ("then", format!("then establish {written}")),
                (
                    "otherwise",
                    format!("then establish Exempt(Acme)\n        otherwise establish {written}"),
                ),
            ] {
                assert_eq!(
                    rejected(&rule_module(&statements)),
                    [(
                        DiagnosticCode::E210,
                        format!(
                            "`{word} establish {written}` in rule `Exemption` requires a proposition"
                        )
                    )],
                    "{word} establish {written}"
                );
            }
        }
        // Each such consequence of a rule is reported, in the order of the
        // rule's text within its list.
        assert_eq!(
            rejected(&rule_module(
                "then establish 5\n        then derive Exempt(Acme)\n        then derive \"x\"\n        otherwise derive 7"
            )),
            [
                (
                    DiagnosticCode::E210,
                    "`then establish 5` in rule `Exemption` requires a proposition".to_owned()
                ),
                (
                    DiagnosticCode::E210,
                    "`then derive \"x\"` in rule `Exemption` requires a proposition".to_owned()
                ),
                (
                    DiagnosticCode::E210,
                    "`otherwise derive 7` in rule `Exemption` requires a proposition".to_owned()
                ),
            ]
        );
        // The consequence is quoted as it is written, on one line, with its
        // `as_to` part and without the `;` that ends it.
        assert_eq!(
            rejected(&rule_module(
                "then establish 1 +\n            2 as_to Acme;"
            )),
            [(
                DiagnosticCode::E210,
                "`then establish 1 + 2 as_to Acme` in rule `Exemption` requires a proposition"
                    .to_owned()
            )]
        );
    }

    #[test]
    fn a_consequence_that_is_a_proposition_is_kept_with_no_diagnostic() {
        let parsed = parse_file(&rule_module(
            "then establish Exempt(Acme)\n        then derive Flag\n        otherwise derive (Exempt(Acme))",
        ));
        let hir = elaborate(&parsed, &SourceManifest::default()).expect("the module is lowered");
        let rule = &hir.rules[0];
        let predicates = |list: &[(String, PropTerm)]| -> Vec<(String, String)> {
            list.iter()
                .map(|(verb, prop)| (verb.clone(), prop.predicate.clone()))
                .collect()
        };
        assert_eq!(
            predicates(&rule.consequences),
            [
                ("establish".to_owned(), "Exempt".to_owned()),
                ("derive".to_owned(), "Flag".to_owned())
            ]
        );
        assert_eq!(
            predicates(&rule.fallback),
            [("derive".to_owned(), "Exempt".to_owned())]
        );
    }
}
