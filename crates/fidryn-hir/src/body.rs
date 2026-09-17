//! Dedicated parsers for declaration bodies.
//!
//! Syntax currently stores query/fn/rule payloads as [`Decl::source`] (and
//! [`Decl::signature`]). When `Decl.expr` / `Decl.goal` exist, prefer those
//! slices. Semantic decisions are made from tokens, not `str::contains`.

use crate::HirQueryBody;
use fidryn_core::ir::{CompareOp, Guard};
use fidryn_core::types::{PrimitiveType, Sort, Type};
use fidryn_core::value::{BinOp, PropTerm, Term};
use fidryn_syntax::ast::{BinOp as SynBinOp, ConsequenceAst, Decl, Expr, GoalAst};
use fidryn_syntax::lexer::{Token, TokenKind, lex};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

const MODALS: &[&str] = &[
    "operative",
    "determined",
    "assumed",
    "observed",
    "necessarily",
];

const GOAL_FIELDS: &[&str] = &[
    "office",
    "status",
    "when_present",
    "when_closed_absent",
    "decision",
    "arguments",
    "result",
    "clause",
    "context",
    "expr",
];

/// Parse a term from a declaration, preferring structured `expr` / `goal`.
pub fn term_from_decl(d: &Decl) -> Option<Term> {
    if let Some(expr) = &d.expr {
        return Some(expr_to_term(expr));
    }
    if let Some(goal) = &d.goal
        && let Some(expr) = &goal.expr
    {
        return Some(expr_to_term(expr));
    }
    for slice in source_slices(d) {
        if let Some(term) = parse_return_or_evaluate(slice) {
            return Some(term);
        }
    }
    for slice in source_slices(d) {
        if let Some(term) = parse_fn_body_from_source(slice) {
            return Some(term);
        }
    }
    None
}

pub fn query_body_from_decl(d: &Decl) -> HirQueryBody {
    if let Some(goal) = &d.goal {
        let mut body = goal_ast_to_body(goal);
        if let Some(expr) = &d.expr
            && let HirQueryBody::Goal {
                kind,
                expr: goal_expr,
                ..
            } = &mut body
            && kind == "Evaluate"
        {
            *goal_expr = Some(concat_seq(expr_to_term(expr), goal_expr.take()));
        }
        return body;
    }
    if let Some(expr) = &d.expr {
        return HirQueryBody::Return(expr_to_term(expr));
    }
    parse_query_body(&d.source)
}

pub fn expr_to_term(expr: &Expr) -> Term {
    match expr {
        Expr::Bool(value) => Term::Bool(*value),
        Expr::Int(value) => Term::Int(*value),
        Expr::Decimal(text) => decimal_term(text),
        Expr::String(text) => Term::String(text.clone()),
        Expr::Ident(name) => Term::Ident(name.clone()),
        Expr::Apply { callee, args } => apply_expr_to_term(callee, args),
        Expr::Binary { op, left, right } => Term::Binary {
            op: map_binop(*op),
            left: Box::new(expr_to_term(left)),
            right: Box::new(expr_to_term(right)),
        },
        Expr::Field { base, name } => Term::Field {
            base: Box::new(expr_to_term(base)),
            name: name.clone(),
        },
        Expr::Call { callee, args } if callee.eq_ignore_ascii_case("transaction") => {
            transaction_term(flatten_transaction_expr_args(args))
        }
        Expr::Call { callee, args } => Term::Call {
            callee: callee.clone(),
            args: args.iter().map(expr_to_term).collect(),
        },
        Expr::If { cond, then, else_ } => Term::If {
            cond: Box::new(expr_to_term(cond)),
            then: Box::new(expr_to_term(then)),
            else_: Box::new(else_.as_deref().map(expr_to_term).unwrap_or(Term::Wildcard)),
        },
        Expr::Money { amount, currency } => Term::Apply {
            ctor: currency.clone(),
            args: vec![decimal_term(amount)],
        },
        Expr::Duration { n, unit } => Term::Apply {
            ctor: unit.clone(),
            args: vec![Term::Int(*n)],
        },
        Expr::Require(inner) => require_term(expr_to_term(inner)),
        Expr::Block(items) => seq_term(items.iter().map(expr_to_term).collect()),
    }
}

pub fn expr_to_guard(expr: &Expr) -> Guard {
    term_to_guard(&expr_to_term(expr))
}

pub fn consequences_from_ast(items: &[ConsequenceAst]) -> Vec<(String, PropTerm)> {
    items
        .iter()
        .filter_map(|item| {
            term_as_prop(&expr_to_term(&item.expr)).map(|prop| (item.verb.clone(), prop))
        })
        .collect()
}

fn goal_ast_to_body(goal: &GoalAst) -> HirQueryBody {
    let fields: BTreeMap<String, Term> = goal
        .fields
        .iter()
        .map(|(name, expr)| (name.clone(), expr_to_term(expr)))
        .collect();
    HirQueryBody::Goal {
        kind: goal.kind.clone(),
        office: fields.get("office").cloned(),
        expr: goal.expr.as_ref().map(expr_to_term),
        fields,
    }
}

fn decimal_term(text: &str) -> Term {
    match Decimal::from_str(text.trim()) {
        Ok(value) => Term::Decimal(value),
        Err(_) => Term::Ident(text.to_owned()),
    }
}

fn apply_expr_to_term(callee: &Expr, args: &[Expr]) -> Term {
    match expr_to_term(callee) {
        Term::Ident(ctor) if ctor.eq_ignore_ascii_case("transaction") => {
            transaction_term(flatten_transaction_expr_args(args))
        }
        Term::Ident(ctor) => Term::Apply {
            ctor,
            args: args.iter().map(expr_to_term).collect(),
        },
        Term::Call { callee, args: prev } if prev.is_empty() => {
            if callee.eq_ignore_ascii_case("transaction") {
                transaction_term(flatten_transaction_expr_args(args))
            } else {
                Term::Apply {
                    ctor: callee,
                    args: args.iter().map(expr_to_term).collect(),
                }
            }
        }
        other => {
            let mut call_args = vec![other];
            call_args.extend(args.iter().map(expr_to_term));
            Term::Apply {
                ctor: "apply".into(),
                args: call_args,
            }
        }
    }
}

fn map_binop(op: SynBinOp) -> BinOp {
    match op {
        SynBinOp::Add => BinOp::Add,
        SynBinOp::Sub => BinOp::Sub,
        SynBinOp::Mul => BinOp::Mul,
        SynBinOp::Div => BinOp::Div,
        SynBinOp::Eq => BinOp::Eq,
        SynBinOp::Ne => BinOp::Ne,
        SynBinOp::Lt => BinOp::Lt,
        SynBinOp::Le => BinOp::Le,
        SynBinOp::Gt => BinOp::Gt,
        SynBinOp::Ge => BinOp::Ge,
        SynBinOp::And => BinOp::And,
        SynBinOp::Or => BinOp::Or,
    }
}

pub fn parse_query_body(src: &str) -> HirQueryBody {
    let inner = last_brace_inner(src).unwrap_or(src);
    let mut p = SliceParser::new(inner);
    let mut items = Vec::new();
    let mut goal = None;
    while !p.is_eof() {
        if p.eat_ident("return") {
            if let Some(term) = p.parse_expr() {
                items.push(term);
            }
            continue;
        }
        if p.eat_ident("goal") {
            let kind = if p.at_kind(TokenKind::Ident) {
                p.bump_text()
            } else {
                String::new()
            };
            if p.eat_kind(TokenKind::LBrace) {
                if kind == "Evaluate" {
                    let expr = parse_contents_until_rbrace(&mut p);
                    goal = Some(HirQueryBody::Goal {
                        kind,
                        office: None,
                        expr,
                        fields: BTreeMap::new(),
                    });
                } else {
                    let fields = parse_goal_fields(&mut p);
                    let office = fields.get("office").cloned();
                    let expr = fields.get("expr").cloned();
                    goal = Some(HirQueryBody::Goal {
                        kind,
                        office,
                        expr,
                        fields,
                    });
                }
            }
            continue;
        }
        match p.parse_expr() {
            Some(term) => items.push(term),
            None => {
                p.bump();
            }
        }
    }
    if let Some(HirQueryBody::Goal {
        kind,
        office,
        expr,
        fields,
    }) = goal
    {
        let expr = if kind == "Evaluate" && !items.is_empty() {
            Some(concat_seq(seq_term(items), expr))
        } else {
            expr
        };
        return HirQueryBody::Goal {
            kind,
            office,
            expr,
            fields,
        };
    }
    if items.is_empty() {
        HirQueryBody::None
    } else {
        HirQueryBody::Return(seq_term(items))
    }
}

pub fn parse_function_parts(src: &str) -> (Vec<(String, String)>, Option<Term>) {
    (extract_params(src), parse_fn_body_from_source(src))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleParts {
    pub guard: Option<Guard>,
    pub consequences: Vec<(String, PropTerm)>,
    pub fallback: Vec<(String, PropTerm)>,
    pub require: Option<Guard>,
}

pub fn parse_rule_parts(src: &str) -> RuleParts {
    let inner = last_brace_inner(src).unwrap_or(src);
    let mut p = SliceParser::new(inner);
    let mut guard = None;
    let mut consequences = Vec::new();
    let mut fallback = Vec::new();
    let mut require = None;
    while !p.is_eof() {
        if p.eat_ident("when") {
            if let Some(term) = p.parse_expr() {
                guard = Some(and_opt_guard(guard.take(), term_to_guard(&term)));
            }
            continue;
        }
        if p.eat_ident("then") {
            if let Some(item) = parse_rule_consequence_pair(&mut p) {
                consequences.push(item);
            }
            continue;
        }
        if p.eat_ident("otherwise") {
            if let Some(item) = parse_rule_consequence_pair(&mut p) {
                fallback.push(item);
            }
            continue;
        }
        if p.eat_ident("require") {
            if let Some(term) = p.parse_expr() {
                require = Some(and_opt_guard(require.take(), term_to_guard(&term)));
            }
            continue;
        }
        p.bump();
    }
    RuleParts {
        guard,
        consequences,
        fallback,
        require,
    }
}

fn parse_rule_consequence_pair(p: &mut SliceParser<'_>) -> Option<(String, PropTerm)> {
    let op = if p.at_kind(TokenKind::Ident) {
        p.bump_text()
    } else {
        "derive".into()
    };
    let term = p.parse_expr()?;
    term_as_prop(&term).map(|prop| (op, prop))
}

pub fn and_opt_guard(left: Option<Guard>, right: Guard) -> Guard {
    match left {
        None | Some(Guard::Satisfied) => right,
        Some(prev) => match right {
            Guard::Satisfied => prev,
            other => Guard::And(vec![prev, other]),
        },
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DutyParts {
    pub bearer: String,
    pub claimant: Option<String>,
    pub attaches: Option<Guard>,
    pub content: Vec<Term>,
    pub due: Option<Term>,
}

pub fn parse_duty_parts(src: &str) -> DutyParts {
    let inner = last_brace_inner(src).unwrap_or(src);
    let mut p = SliceParser::new(inner);
    let mut bearer = String::new();
    let mut claimant = None;
    let mut attaches = None;
    let mut content = Vec::new();
    let mut due = None;
    while !p.is_eof() {
        if p.eat_ident("bearer") {
            if p.at_kind(TokenKind::Ident) {
                bearer = p.bump_text();
            }
            continue;
        }
        if p.eat_ident("claimant") {
            if p.at_kind(TokenKind::Ident) {
                claimant = Some(p.bump_text());
            }
            continue;
        }
        if p.eat_ident("attaches") {
            let _ = p.eat_ident("when");
            if let Some(term) = p.parse_expr() {
                attaches = Some(term_to_guard(&term));
            }
            continue;
        }
        if p.eat_ident("content") {
            if let Some(term) = p.parse_expr() {
                content.push(term);
            }
            continue;
        }
        if p.eat_ident("due") {
            due = parse_due_expr(&mut p);
            continue;
        }
        p.bump();
    }
    DutyParts {
        bearer,
        claimant,
        attaches,
        content,
        due,
    }
}

fn parse_due_expr(p: &mut SliceParser<'_>) -> Option<Term> {
    let term = p.parse_expr()?;
    if p.eat_ident("after") {
        let event = p.parse_expr()?;
        Some(Term::Apply {
            ctor: "after".into(),
            args: vec![term, event],
        })
    } else {
        Some(term)
    }
}

pub fn parse_rule_kind(src: &str) -> String {
    let mut p = SliceParser::new(src);
    while !p.is_eof() {
        if p.eat_kind(TokenKind::Colon) && p.at_kind(TokenKind::Ident) {
            let kind = p.bump_text();
            if matches!(kind.as_str(), "derive" | "constitutive" | "prescriptive") {
                return kind;
            }
        } else {
            p.bump();
        }
    }
    "prescriptive".into()
}

pub fn parse_type_name(raw: &str) -> Type {
    let s = raw.trim();
    if s.is_empty() {
        return Type::Sort(Sort::Nominal(String::new()));
    }
    if let Some(open) = s.find('<')
        && s.ends_with('>')
        && open > 0
    {
        let ctor = s[..open].trim();
        let inner = &s[open + 1..s.len() - 1];
        return parse_applied_type(ctor, inner);
    }
    match s {
        "Bool" => Type::bool(),
        "Int" => Type::Primitive(PrimitiveType::Int),
        "Decimal" => Type::Primitive(PrimitiveType::Decimal),
        "String" => Type::Primitive(PrimitiveType::String),
        "Date" => Type::Primitive(PrimitiveType::Date),
        "Time" => Type::Primitive(PrimitiveType::Time),
        "Prop" => Type::prop(),
        "NaturalPerson" => Type::Sort(Sort::NaturalPerson),
        "LegalPerson" => Type::Sort(Sort::LegalPerson),
        "LegalEntity" => Type::Sort(Sort::LegalEntity),
        "Trust" => Type::Sort(Sort::Trust),
        "PremaritalAgreement" => Type::Sort(Sort::PremaritalAgreement),
        "ProposedLLC" | "ProposedLlc" => Type::Sort(Sort::ProposedLlc),
        "Asset" => Type::Sort(Sort::Asset),
        "Office" => Type::Sort(Sort::Office),
        "Authority" => Type::Sort(Sort::Authority),
        "Jurisdiction" => Type::Sort(Sort::Jurisdiction),
        "SourceVersion" => Type::Sort(Sort::SourceVersion),
        "LegalContext" => Type::Sort(Sort::LegalContext),
        "LegalAction" => Type::Sort(Sort::LegalAction),
        "LegalEffect" => Type::Sort(Sort::LegalEffect),
        "Proceeding" => Type::Sort(Sort::Proceeding),
        "RecordItem" => Type::Sort(Sort::RecordItem),
        "Determination" => Type::Sort(Sort::Determination),
        "Interpretation" => Type::Sort(Sort::Interpretation),
        "ClauseRef" => Type::Sort(Sort::ClauseRef),
        other => Type::Sort(Sort::Nominal(other.to_owned())),
    }
}

pub fn extract_params(src: &str) -> Vec<(String, String)> {
    let Some((start, end)) = first_paren_group(src) else {
        return Vec::new();
    };
    let inner = src[start + 1..end].trim();
    if inner.is_empty() {
        return Vec::new();
    }
    split_top_level(inner, ',')
        .filter_map(|part| {
            let (name, ty) = part.split_once(':')?;
            let name = name.trim();
            let ty = ty.trim();
            if name.is_empty() || ty.is_empty() {
                None
            } else {
                Some((name.to_owned(), ty.to_owned()))
            }
        })
        .collect()
}

pub fn last_brace_inner(src: &str) -> Option<&str> {
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

pub fn parse_expr_src(src: &str) -> Option<Term> {
    let mut p = SliceParser::new(src);
    let term = p.parse_expr()?;
    Some(term)
}

pub fn term_as_name(term: &Term) -> Option<String> {
    match term {
        Term::Ident(s) | Term::String(s) | Term::Binder(s) => Some(s.clone()),
        Term::Call { callee, .. } => Some(callee.clone()),
        Term::Apply { ctor, .. } => Some(ctor.clone()),
        _ => None,
    }
}

pub fn term_as_prop(term: &Term) -> Option<PropTerm> {
    match term {
        Term::Call { callee, args } if !is_modal(callee) && !is_operator(callee) => {
            Some(PropTerm {
                predicate: callee.clone(),
                arguments: args.clone(),
            })
        }
        Term::Apply { ctor, args } if !is_modal(ctor) && !is_operator(ctor) => Some(PropTerm {
            predicate: ctor.clone(),
            arguments: args.clone(),
        }),
        Term::Ident(name) => Some(PropTerm::new(name.clone(), Vec::new())),
        _ => None,
    }
}

pub fn flatten_term_list(term: &Term) -> Vec<Term> {
    match term {
        Term::Set(xs) => xs.clone(),
        Term::Record(fields) => fields.values().cloned().collect(),
        other => vec![other.clone()],
    }
}

pub fn collect_term_callees(term: &Term, functions: &BTreeSet<&str>, out: &mut BTreeSet<String>) {
    match term {
        Term::Ident(name) | Term::Binder(name) => {
            if functions.contains(name.as_str()) {
                out.insert(name.clone());
            }
        }
        Term::Call { callee, args } => {
            if functions.contains(callee.as_str()) {
                out.insert(callee.clone());
            }
            for arg in args {
                collect_term_callees(arg, functions, out);
            }
        }
        Term::Apply { ctor, args } => {
            if functions.contains(ctor.as_str()) {
                out.insert(ctor.clone());
            }
            for arg in args {
                collect_term_callees(arg, functions, out);
            }
        }
        Term::Binary { left, right, .. } => {
            collect_term_callees(left, functions, out);
            collect_term_callees(right, functions, out);
        }
        Term::If { cond, then, else_ } => {
            collect_term_callees(cond, functions, out);
            collect_term_callees(then, functions, out);
            collect_term_callees(else_, functions, out);
        }
        Term::Field { base, .. } => collect_term_callees(base, functions, out),
        Term::Set(xs) => {
            for x in xs {
                collect_term_callees(x, functions, out);
            }
        }
        Term::Record(fields) => {
            for x in fields.values() {
                collect_term_callees(x, functions, out);
            }
        }
        _ => {}
    }
}

pub fn collect_source_callees(src: &str, functions: &BTreeSet<&str>) -> BTreeSet<String> {
    let body = last_brace_inner(src).unwrap_or("");
    let mut out = BTreeSet::new();
    let p = SliceParser::new(body);
    for token in &p.tokens {
        if token.kind != TokenKind::Ident {
            continue;
        }
        let name = p.text(*token);
        if functions.contains(name) {
            out.insert(name.to_owned());
        }
    }
    out
}

pub fn term_has_bare_prop_guard(term: &Term, propositions: &BTreeSet<String>) -> bool {
    match term {
        Term::If { cond, then, else_ } => {
            is_bare_prop_cond(cond, propositions)
                || term_has_bare_prop_guard(then, propositions)
                || term_has_bare_prop_guard(else_, propositions)
        }
        Term::Apply { ctor, args } if ctor == "if" => {
            args.first()
                .is_some_and(|cond| is_bare_prop_cond(cond, propositions))
                || args
                    .iter()
                    .any(|arg| term_has_bare_prop_guard(arg, propositions))
        }
        Term::Call { args, .. } | Term::Apply { args, .. } => args
            .iter()
            .any(|arg| term_has_bare_prop_guard(arg, propositions)),
        Term::Binary { left, right, .. } => {
            term_has_bare_prop_guard(left, propositions)
                || term_has_bare_prop_guard(right, propositions)
        }
        Term::Field { base, .. } => term_has_bare_prop_guard(base, propositions),
        Term::Set(xs) => xs
            .iter()
            .any(|arg| term_has_bare_prop_guard(arg, propositions)),
        Term::Record(fields) => fields
            .values()
            .any(|arg| term_has_bare_prop_guard(arg, propositions)),
        _ => false,
    }
}

pub fn source_has_bare_prop_if(src: &str, propositions: &BTreeSet<String>) -> bool {
    let p = SliceParser::new(src);
    let tokens = &p.tokens;
    let mut i = 0;
    while i < tokens.len() {
        if p.is_ident_at(i, "if") {
            let mut j = i + 1;
            while j < tokens.len() && p.is_ident_at(j, "not") {
                j += 1;
            }
            if j < tokens.len() && tokens[j].kind == TokenKind::Ident {
                let name = p.text(tokens[j]);
                if !is_modal(name)
                    && j + 1 < tokens.len()
                    && tokens[j + 1].kind == TokenKind::LParen
                    && is_prop_ctor(name, propositions)
                {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

pub fn query_is_automatic(src: &str) -> bool {
    let mut p = SliceParser::new(src);
    let _ = p.eat_ident("query");
    p.at_ident("automatic")
}

pub fn import_requires_digest(src: &str) -> bool {
    ident_present(src, "digest")
}

/// Value of `digest "..."` on an import, if present.
pub fn import_digest(src: &str) -> Option<String> {
    field_after_ident(src, "digest")
}

/// Value of `version "..."` on an import, if present.
pub fn import_version(src: &str) -> Option<String> {
    field_after_ident(src, "version")
}

fn ident_present(src: &str, name: &str) -> bool {
    let p = SliceParser::new(src);
    p.tokens.iter().any(|t| {
        t.kind == TokenKind::Ident
            && p.text(*t) == name
            && ident_boundary(src, t.start as usize, name.len())
    })
}

fn field_after_ident(src: &str, name: &str) -> Option<String> {
    let p = SliceParser::new(src);
    for i in 0..p.tokens.len() {
        let token = p.tokens[i];
        if token.kind != TokenKind::Ident || p.text(token) != name {
            continue;
        }
        if !ident_boundary(src, token.start as usize, name.len()) {
            continue;
        }
        let next = p.tokens.get(i + 1)?;
        match next.kind {
            TokenKind::String => return Some(unquote_token(p.text(*next))),
            TokenKind::Ident | TokenKind::Int | TokenKind::Decimal => {
                return Some(p.text(*next).to_owned());
            }
            _ => return None,
        }
    }
    None
}

fn unquote_token(text: &str) -> String {
    let text = text.trim();
    let bytes = text.as_bytes();
    if bytes.len() >= 2 {
        let start = bytes[0];
        let end = bytes[bytes.len() - 1];
        if (start == b'"' && end == b'"') || (start == b'\'' && end == b'\'') {
            return text[1..text.len() - 1].to_owned();
        }
    }
    text.to_owned()
}

fn source_slices(d: &Decl) -> Vec<&str> {
    let mut out = Vec::new();
    if let Some(sig) = d.signature.as_deref() {
        out.push(sig);
    }
    out.push(d.source.as_str());
    out
}

fn parse_return_or_evaluate(src: &str) -> Option<Term> {
    match parse_query_body(src) {
        HirQueryBody::Return(term) => Some(term),
        HirQueryBody::Goal { kind, expr, .. } if kind == "Evaluate" => expr,
        _ => None,
    }
}

fn parse_fn_body_from_source(src: &str) -> Option<Term> {
    let inner = last_brace_inner(src)?;
    parse_block_contents(inner).or_else(|| parse_expr_src(inner))
}

fn parse_block_contents(src: &str) -> Option<Term> {
    let mut p = SliceParser::new(src);
    parse_contents_until_end(&mut p)
}

fn parse_contents_until_rbrace(p: &mut SliceParser<'_>) -> Option<Term> {
    let term = parse_contents_until_end(p);
    let _ = p.eat_kind(TokenKind::RBrace);
    term
}

fn parse_contents_until_end(p: &mut SliceParser<'_>) -> Option<Term> {
    let mut items = Vec::new();
    while !p.is_eof() && !p.at_kind(TokenKind::RBrace) {
        if p.eat_kind(TokenKind::Semicolon) {
            continue;
        }
        if p.eat_ident("return") {
            if let Some(term) = p.parse_expr() {
                items.push(term);
            }
            continue;
        }
        match p.parse_expr() {
            Some(term) => items.push(term),
            None => {
                p.bump();
            }
        }
    }
    if items.is_empty() {
        None
    } else {
        Some(seq_term(items))
    }
}

fn seq_term(args: Vec<Term>) -> Term {
    match args.len() {
        0 => Term::Wildcard,
        1 => args.into_iter().next().unwrap_or(Term::Wildcard),
        _ => Term::Apply {
            ctor: "seq".into(),
            args,
        },
    }
}

fn transaction_term(args: Vec<Term>) -> Term {
    Term::Apply {
        ctor: "transaction".into(),
        args,
    }
}

fn flatten_transaction_expr_args(args: &[Expr]) -> Vec<Term> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Expr::Block(items) => out.extend(items.iter().map(expr_to_term)),
            other => out.push(expr_to_term(other)),
        }
    }
    out
}

fn require_term(inner: Term) -> Term {
    Term::Apply {
        ctor: "require".into(),
        args: vec![inner],
    }
}

fn concat_seq(head: Term, tail: Option<Term>) -> Term {
    let mut args = match head {
        Term::Apply { ctor, args } if ctor == "seq" => args,
        other => vec![other],
    };
    match tail {
        Some(Term::Apply { ctor, args: more }) if ctor == "seq" => args.extend(more),
        Some(other) => args.push(other),
        None => {}
    }
    seq_term(args)
}

fn parse_goal_fields(p: &mut SliceParser<'_>) -> BTreeMap<String, Term> {
    let mut fields = BTreeMap::new();
    while !p.is_eof() && !p.at_kind(TokenKind::RBrace) {
        if p.eat_kind(TokenKind::Semicolon) || p.eat_kind(TokenKind::Comma) {
            continue;
        }
        if p.at_kind(TokenKind::Ident) && p.at_goal_field() {
            let name = p.bump_text();
            if let Some(term) = p.parse_expr() {
                fields.insert(name, term);
            }
            continue;
        }
        p.bump();
    }
    let _ = p.eat_kind(TokenKind::RBrace);
    fields
}

fn parse_applied_type(ctor: &str, inner: &str) -> Type {
    match ctor {
        "Money" => Type::Primitive(PrimitiveType::Money {
            currency: inner.trim().to_owned(),
        }),
        "Duration" => Type::Primitive(PrimitiveType::Duration {
            calendar: inner.trim().to_owned(),
        }),
        "Option" => Type::Primitive(PrimitiveType::Option {
            inner: Box::new(parse_type_name(inner)),
        }),
        "Interval" => Type::Primitive(PrimitiveType::Interval {
            inner: Box::new(parse_type_name(inner)),
        }),
        "FiniteSet" => Type::Primitive(PrimitiveType::FiniteSet {
            inner: Box::new(parse_type_name(inner)),
        }),
        "NonEmptySet" => Type::Primitive(PrimitiveType::NonEmptySet {
            inner: Box::new(parse_type_name(inner)),
        }),
        "Map" => {
            let mut args = split_top_level(inner, ',');
            let key = args.next().unwrap_or("");
            let value = args.next().unwrap_or("");
            Type::Primitive(PrimitiveType::Map {
                key: Box::new(parse_type_name(key)),
                value: Box::new(parse_type_name(value)),
            })
        }
        ctor => {
            let args = split_top_level(inner, ',').map(parse_type_name).collect();
            Type::Applied {
                ctor: ctor.to_owned(),
                args,
            }
        }
    }
}

fn first_paren_group(src: &str) -> Option<(usize, usize)> {
    let start = src.find('(')?;
    let mut depth = 0i32;
    for (i, c) in src[start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, start + i));
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level(src: &str, sep: char) -> impl Iterator<Item = &str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth_paren = 0i32;
    let mut depth_angle = 0i32;
    let mut depth_brack = 0i32;
    let mut depth_brace = 0i32;
    for (i, c) in src.char_indices() {
        match c {
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '[' => depth_brack += 1,
            ']' => depth_brack -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            ch if ch == sep
                && depth_paren == 0
                && depth_angle == 0
                && depth_brack == 0
                && depth_brace == 0 =>
            {
                parts.push(src[start..i].trim());
                start = i + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(src[start..].trim());
    parts.into_iter().filter(|s| !s.is_empty())
}

fn term_to_guard(term: &Term) -> Guard {
    match term {
        Term::Bool(true) => Guard::Satisfied,
        Term::Bool(false) => Guard::Not(Box::new(Guard::Satisfied)),
        Term::Apply { ctor, args } if ctor == "operative" => operative_guard(args),
        Term::Call { callee, args } if callee == "operative" => operative_guard(args),
        Term::Apply { ctor, args } if ctor == "determined" || ctor == "assumed" => args
            .first()
            .and_then(term_as_prop)
            .map(Guard::Derived)
            .unwrap_or(Guard::Satisfied),
        Term::Apply { ctor, args } if ctor == "observed" => Guard::Observed {
            schema: args.first().and_then(term_as_name).unwrap_or_default(),
            binder: args.get(1).and_then(term_as_name).unwrap_or_default(),
        },
        Term::Apply { ctor, args } if ctor == "and" => {
            Guard::And(args.iter().map(term_to_guard).collect())
        }
        Term::Apply { ctor, args } if ctor == "or" => {
            Guard::Or(args.iter().map(term_to_guard).collect())
        }
        Term::Binary {
            op: BinOp::And,
            left,
            right,
        } => Guard::And(vec![term_to_guard(left), term_to_guard(right)]),
        Term::Binary {
            op: BinOp::Or,
            left,
            right,
        } => Guard::Or(vec![term_to_guard(left), term_to_guard(right)]),
        Term::Apply { ctor, args } if ctor == "not" && args.len() == 1 => {
            Guard::Not(Box::new(term_to_guard(&args[0])))
        }
        Term::Binary { op, left, right } if binop_compare(*op).is_some() => Guard::Compare {
            op: binop_compare(*op).unwrap_or(CompareOp::Eq),
            left: *left.clone(),
            right: *right.clone(),
        },
        Term::Apply { ctor, args } if args.len() == 2 && compare_op(ctor).is_some() => {
            Guard::Compare {
                op: compare_op(ctor).unwrap_or(CompareOp::Eq),
                left: args[0].clone(),
                right: args[1].clone(),
            }
        }
        other => term_as_prop(other)
            .map(|prop| Guard::Operative(prop, String::new()))
            .unwrap_or(Guard::Satisfied),
    }
}

fn operative_guard(args: &[Term]) -> Guard {
    let prop = args
        .first()
        .and_then(term_as_prop)
        .unwrap_or_else(|| PropTerm::new("", Vec::new()));
    let ctx = args.get(1).and_then(term_as_name).unwrap_or_default();
    Guard::Operative(prop, ctx)
}

fn binop_compare(op: BinOp) -> Option<CompareOp> {
    Some(match op {
        BinOp::Eq => CompareOp::Eq,
        BinOp::Ne => CompareOp::Ne,
        BinOp::Lt => CompareOp::Lt,
        BinOp::Le => CompareOp::Le,
        BinOp::Gt => CompareOp::Gt,
        BinOp::Ge => CompareOp::Ge,
        _ => return None,
    })
}

fn compare_op(ctor: &str) -> Option<CompareOp> {
    Some(match ctor {
        "==" => CompareOp::Eq,
        "!=" => CompareOp::Ne,
        "<" => CompareOp::Lt,
        "<=" => CompareOp::Le,
        ">" => CompareOp::Gt,
        ">=" => CompareOp::Ge,
        _ => return None,
    })
}

fn is_bare_prop_cond(term: &Term, propositions: &BTreeSet<String>) -> bool {
    match term {
        Term::Apply { ctor, .. } if is_modal(ctor) => false,
        Term::Call { callee, .. } if is_modal(callee) => false,
        Term::Apply { ctor, args } if ctor == "not" => args
            .first()
            .is_some_and(|inner| is_bare_prop_cond(inner, propositions)),
        Term::Apply { ctor, args } if ctor == "and" || ctor == "or" => args
            .iter()
            .any(|inner| is_bare_prop_cond(inner, propositions)),
        Term::Binary {
            op: BinOp::And | BinOp::Or,
            left,
            right,
        } => is_bare_prop_cond(left, propositions) || is_bare_prop_cond(right, propositions),
        Term::Call { callee, .. } => is_prop_ctor(callee, propositions),
        Term::Apply { ctor, .. } => is_prop_ctor(ctor, propositions),
        _ => false,
    }
}

fn is_prop_ctor(name: &str, propositions: &BTreeSet<String>) -> bool {
    propositions.contains(name) || name.starts_with(|c: char| c.is_ascii_uppercase())
}

fn is_modal(name: &str) -> bool {
    MODALS.contains(&name)
}

fn is_operator(name: &str) -> bool {
    matches!(
        name,
        "if" | "and"
            | "or"
            | "not"
            | "+"
            | "-"
            | "*"
            | "/"
            | "=="
            | "!="
            | "<"
            | "<="
            | ">"
            | ">="
            | "."
            | "=>"
            | "call"
            | "seq"
            | "require"
            | "transaction"
    ) || is_modal(name)
}

fn ident_boundary(src: &str, start: usize, len: usize) -> bool {
    let before = src[..start].chars().next_back();
    let after = src.get(start + len..).and_then(|s| s.chars().next());
    let ident_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
    before.is_none_or(|c| !ident_char(c)) && after.is_none_or(|c| !ident_char(c))
}

struct SliceParser<'a> {
    src: &'a str,
    tokens: Vec<Token>,
    idx: usize,
}

impl<'a> SliceParser<'a> {
    fn new(src: &'a str) -> Self {
        let tokens = lex(src)
            .into_iter()
            .filter(|t| {
                !matches!(
                    t.kind,
                    TokenKind::Whitespace
                        | TokenKind::Comment
                        | TokenKind::DocComment
                        | TokenKind::Newline
                        | TokenKind::Eof
                )
            })
            .collect();
        Self {
            src,
            tokens,
            idx: 0,
        }
    }

    fn is_eof(&self) -> bool {
        self.idx >= self.tokens.len()
    }

    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.idx).copied()
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.peek().map(|t| t.kind)
    }

    fn peek_at(&self, n: usize) -> Option<Token> {
        self.tokens.get(self.idx + n).copied()
    }

    fn at_kind(&self, kind: TokenKind) -> bool {
        self.peek_kind() == Some(kind)
    }

    fn text(&self, token: Token) -> &'a str {
        &self.src[token.start as usize..token.end as usize]
    }

    fn bump(&mut self) -> Option<Token> {
        let token = self.peek()?;
        self.idx += 1;
        Some(token)
    }

    fn bump_text(&mut self) -> String {
        self.bump()
            .map(|t| self.text(t).to_owned())
            .unwrap_or_default()
    }

    fn eat_kind(&mut self, kind: TokenKind) -> bool {
        if self.at_kind(kind) {
            self.idx += 1;
            true
        } else {
            false
        }
    }

    fn at_ident(&self, name: &str) -> bool {
        self.peek()
            .is_some_and(|t| t.kind == TokenKind::Ident && self.text(t) == name)
    }

    fn eat_ident(&mut self, name: &str) -> bool {
        if self.at_ident(name) {
            self.idx += 1;
            true
        } else {
            false
        }
    }

    fn is_ident_at(&self, index: usize, name: &str) -> bool {
        self.tokens
            .get(index)
            .is_some_and(|t| t.kind == TokenKind::Ident && self.text(*t) == name)
    }

    fn at_goal_field(&self) -> bool {
        self.peek()
            .is_some_and(|t| t.kind == TokenKind::Ident && GOAL_FIELDS.contains(&self.text(t)))
    }

    fn parse_expr(&mut self) -> Option<Term> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Option<Term> {
        let mut left = self.parse_and()?;
        while self.eat_ident("or") {
            let right = self.parse_and()?;
            left = bin("or", left, right);
        }
        Some(left)
    }

    fn parse_and(&mut self) -> Option<Term> {
        let mut left = self.parse_compare()?;
        while self.eat_ident("and") {
            let right = self.parse_compare()?;
            left = bin("and", left, right);
        }
        Some(left)
    }

    fn parse_compare(&mut self) -> Option<Term> {
        let left = self.parse_add()?;
        let op = match self.peek_kind() {
            Some(TokenKind::EqEq) => "==",
            Some(TokenKind::Ne) => "!=",
            Some(TokenKind::Lt) => "<",
            Some(TokenKind::Le) => "<=",
            Some(TokenKind::Gt) => ">",
            Some(TokenKind::Ge) => ">=",
            Some(TokenKind::FatArrow) => "=>",
            _ => return Some(left),
        };
        self.bump();
        let right = self.parse_add()?;
        Some(bin(op, left, right))
    }

    fn parse_add(&mut self) -> Option<Term> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.peek_kind() {
                Some(TokenKind::Plus) => "+",
                Some(TokenKind::Minus) => "-",
                _ => break,
            };
            self.bump();
            let right = self.parse_mul()?;
            left = bin(op, left, right);
        }
        Some(left)
    }

    fn parse_mul(&mut self) -> Option<Term> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek_kind() {
                Some(TokenKind::Star) => "*",
                Some(TokenKind::Slash) => "/",
                _ => break,
            };
            self.bump();
            let right = self.parse_unary()?;
            left = bin(op, left, right);
        }
        Some(left)
    }

    fn parse_unary(&mut self) -> Option<Term> {
        if self.eat_ident("not") {
            let inner = self.parse_unary()?;
            return Some(Term::Apply {
                ctor: "not".into(),
                args: vec![inner],
            });
        }
        if self.eat_kind(TokenKind::Minus) {
            let inner = self.parse_unary()?;
            return Some(match inner {
                Term::Int(n) => Term::Int(-n),
                other => Term::Apply {
                    ctor: "-".into(),
                    args: vec![Term::Int(0), other],
                },
            });
        }
        for modal in MODALS {
            if self.eat_ident(modal) {
                let inner = self.parse_unary()?;
                let mut args = vec![inner];
                if self.eat_ident("in")
                    && let Some(ctx) = self.parse_unary()
                {
                    args.push(ctx);
                }
                return Some(Term::Apply {
                    ctor: (*modal).to_owned(),
                    args,
                });
            }
        }
        if self.at_ident("if") {
            return self.parse_if();
        }
        if self.eat_ident("require") {
            let inner = self.parse_expr()?;
            if self.eat_ident("using") && self.at_kind(TokenKind::Ident) {
                self.bump();
            }
            return Some(require_term(inner));
        }
        if self.at_ident("transaction")
            && self.peek_at(1).map(|t| t.kind) == Some(TokenKind::LBrace)
        {
            return self.parse_transaction_block_term();
        }
        if self.at_ident("for_all") || self.at_ident("exists") {
            return self.parse_quantifier();
        }
        self.parse_postfix()
    }

    fn parse_transaction_block_term(&mut self) -> Option<Term> {
        self.eat_ident("transaction");
        if !self.eat_kind(TokenKind::LBrace) {
            return Some(Term::Ident("transaction".into()));
        }
        let mut items = Vec::new();
        while !self.is_eof() && !self.at_kind(TokenKind::RBrace) {
            if self.eat_kind(TokenKind::Semicolon) || self.eat_kind(TokenKind::Comma) {
                continue;
            }
            if self.eat_ident("return") {
                if let Some(term) = self.parse_expr() {
                    items.push(term);
                }
                continue;
            }
            match self.parse_expr() {
                Some(term) => items.push(term),
                None => {
                    self.bump();
                }
            }
        }
        let _ = self.eat_kind(TokenKind::RBrace);
        Some(transaction_term(items))
    }

    fn parse_quantifier(&mut self) -> Option<Term> {
        let kind = self.bump_text();
        if !self.at_kind(TokenKind::Ident) {
            return Some(Term::Ident(kind));
        }
        let binder = self.bump_text();
        if !self.eat_ident("in") {
            return Some(Term::Apply {
                ctor: kind,
                args: vec![Term::Ident(binder)],
            });
        }
        let domain = self.parse_expr()?;
        let body = if self.eat_kind(TokenKind::Colon) {
            self.parse_expr()?
        } else if self.at_kind(TokenKind::LBrace) {
            self.parse_block()?
        } else {
            Term::Wildcard
        };
        Some(Term::Apply {
            ctor: kind,
            args: vec![Term::Ident(binder), domain, body],
        })
    }

    fn parse_if(&mut self) -> Option<Term> {
        self.eat_ident("if");
        let cond = self.parse_expr()?;
        let then_branch = if self.at_kind(TokenKind::LBrace) {
            self.parse_block()?
        } else {
            self.parse_expr()?
        };
        let mut args = vec![cond, then_branch];
        if self.eat_ident("else") {
            let else_branch = if self.at_ident("if") {
                self.parse_if()?
            } else if self.at_kind(TokenKind::LBrace) {
                self.parse_block()?
            } else {
                self.parse_expr()?
            };
            args.push(else_branch);
        }
        Some(Term::Apply {
            ctor: "if".into(),
            args,
        })
    }

    fn parse_block(&mut self) -> Option<Term> {
        if !self.eat_kind(TokenKind::LBrace) {
            return None;
        }
        let term = parse_contents_until_end(self);
        let _ = self.eat_kind(TokenKind::RBrace);
        Some(term.unwrap_or(Term::Wildcard))
    }

    fn parse_postfix(&mut self) -> Option<Term> {
        let mut term = self.parse_primary()?;
        loop {
            if self.at_kind(TokenKind::Lt) && !self.try_skip_type_args() {
                break;
            }
            if self.eat_kind(TokenKind::LParen) {
                let args = self.parse_arg_list();
                let _ = self.eat_kind(TokenKind::RParen);
                term = match term {
                    Term::Ident(name) => Term::Apply { ctor: name, args },
                    Term::Apply {
                        ctor, args: prev, ..
                    } if prev.is_empty() => Term::Apply { ctor, args },
                    other => {
                        let mut call_args = vec![other];
                        call_args.extend(args);
                        Term::Apply {
                            ctor: "call".into(),
                            args: call_args,
                        }
                    }
                };
                continue;
            }
            if self.eat_kind(TokenKind::Dot) && self.at_kind(TokenKind::Ident) {
                let name = self.bump_text();
                term = Term::Apply {
                    ctor: ".".into(),
                    args: vec![term, Term::Ident(name)],
                };
                continue;
            }
            break;
        }
        Some(term)
    }

    fn parse_primary(&mut self) -> Option<Term> {
        let token = self.peek()?;
        match token.kind {
            TokenKind::Ident => {
                if self.at_ident("true") {
                    self.bump();
                    return Some(Term::Bool(true));
                }
                if self.at_ident("false") {
                    self.bump();
                    return Some(Term::Bool(false));
                }
                Some(Term::Ident(self.parse_qname()))
            }
            TokenKind::Int => {
                let text = self.bump_text();
                let n = text.parse().ok().map(Term::Int)?;
                if self.at_kind(TokenKind::DurationUnit) {
                    let unit = self.bump_text();
                    return Some(Term::Apply {
                        ctor: unit,
                        args: vec![n],
                    });
                }
                Some(n)
            }
            TokenKind::Decimal => Some(decimal_term(&self.bump_text())),
            TokenKind::Date | TokenKind::DateTime => Some(Term::Ident(self.bump_text())),
            TokenKind::String => {
                let raw = self.bump_text();
                Some(Term::String(trim_quotes(&raw).to_owned()))
            }
            TokenKind::LParen => {
                self.bump();
                let inner = self.parse_expr();
                let _ = self.eat_kind(TokenKind::RParen);
                inner
            }
            TokenKind::LBrace => self.parse_brace_term(),
            TokenKind::PlusInf => {
                self.bump();
                Some(Term::Ident("+inf".into()))
            }
            TokenKind::MinusInf => {
                self.bump();
                Some(Term::Ident("-inf".into()))
            }
            TokenKind::Star => {
                self.bump();
                Some(Term::Wildcard)
            }
            _ => None,
        }
    }

    fn parse_qname(&mut self) -> String {
        let mut name = self.bump_text();
        while self.eat_kind(TokenKind::Dot) && self.at_kind(TokenKind::Ident) {
            name.push('.');
            name.push_str(&self.bump_text());
        }
        name
    }

    fn parse_brace_term(&mut self) -> Option<Term> {
        if !self.eat_kind(TokenKind::LBrace) {
            return None;
        }
        if self.eat_kind(TokenKind::RBrace) {
            return Some(Term::Set(Vec::new()));
        }
        let record = self.at_kind(TokenKind::Ident)
            && matches!(
                self.peek_at(1).map(|t| t.kind),
                Some(TokenKind::Colon | TokenKind::Eq)
            );
        if record {
            let mut fields = BTreeMap::new();
            while !self.is_eof() && !self.at_kind(TokenKind::RBrace) {
                if self.eat_kind(TokenKind::Comma) || self.eat_kind(TokenKind::Semicolon) {
                    continue;
                }
                if !self.at_kind(TokenKind::Ident) {
                    self.bump();
                    continue;
                }
                let key = self.bump_text();
                let _ = self.eat_kind(TokenKind::Colon) || self.eat_kind(TokenKind::Eq);
                if let Some(value) = self.parse_expr() {
                    fields.insert(key, value);
                }
            }
            let _ = self.eat_kind(TokenKind::RBrace);
            return Some(Term::Record(fields));
        }
        let mut items = Vec::new();
        while !self.is_eof() && !self.at_kind(TokenKind::RBrace) {
            if self.eat_kind(TokenKind::Comma) || self.eat_kind(TokenKind::Semicolon) {
                continue;
            }
            match self.parse_expr() {
                Some(term) => items.push(term),
                None => {
                    self.bump();
                }
            }
        }
        let _ = self.eat_kind(TokenKind::RBrace);
        Some(Term::Set(items))
    }

    fn parse_arg_list(&mut self) -> Vec<Term> {
        let mut args = Vec::new();
        while !self.is_eof() && !self.at_kind(TokenKind::RParen) {
            if self.eat_kind(TokenKind::Comma) {
                continue;
            }
            if self.at_kind(TokenKind::Ident)
                && self.peek_at(1).map(|t| t.kind) == Some(TokenKind::Eq)
            {
                self.bump();
                self.bump();
            }
            match self.parse_expr() {
                Some(term) => args.push(term),
                None => {
                    self.bump();
                }
            }
        }
        args
    }

    fn try_skip_type_args(&mut self) -> bool {
        if !self.at_kind(TokenKind::Lt) {
            return false;
        }
        let save = self.idx;
        self.bump();
        let mut depth = 1i32;
        while depth > 0 && !self.is_eof() {
            match self.peek_kind() {
                Some(TokenKind::Lt) => {
                    depth += 1;
                    self.bump();
                }
                Some(TokenKind::Gt) => {
                    depth -= 1;
                    self.bump();
                }
                Some(
                    TokenKind::Plus
                    | TokenKind::Minus
                    | TokenKind::Star
                    | TokenKind::Slash
                    | TokenKind::EqEq
                    | TokenKind::Ne
                    | TokenKind::Le
                    | TokenKind::Ge
                    | TokenKind::FatArrow,
                ) => {
                    self.idx = save;
                    return false;
                }
                _ => {
                    self.bump();
                }
            }
        }
        if depth != 0 {
            self.idx = save;
            false
        } else {
            true
        }
    }
}

fn bin(op: &str, left: Term, right: Term) -> Term {
    Term::Apply {
        ctor: op.to_owned(),
        args: vec![left, right],
    }
}

fn trim_quotes(s: &str) -> &str {
    s.trim().trim_matches('"')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_expr_true() {
        assert_eq!(parse_expr_src("true"), Some(Term::Bool(true)));
        assert_eq!(parse_expr_src("false"), Some(Term::Bool(false)));
    }

    #[test]
    fn parse_expr_counted_days_duration() {
        assert_eq!(
            parse_expr_src("0 counted_days"),
            Some(Term::Apply {
                ctor: "counted_days".into(),
                args: vec![Term::Int(0)],
            })
        );
        assert_eq!(parse_expr_src("15"), Some(Term::Int(15)));
        assert_eq!(
            parse_expr_src("GracePeriod"),
            Some(Term::Ident("GracePeriod".into()))
        );
    }

    #[test]
    fn parse_duty_late_payment_fields() {
        let src = r#"
duty PayInvoice {
    bearer Payer
    claimant Payee
    attaches when operative InvoiceIssued(Payer)
    content USD(100.00)
    due 0 counted_days after invoice_date
}
"#;
        let parts = parse_duty_parts(src);
        assert_eq!(parts.bearer, "Payer");
        assert_eq!(parts.claimant.as_deref(), Some("Payee"));
        assert!(
            matches!(
                &parts.attaches,
                Some(Guard::Operative(prop, _)) if prop.predicate == "InvoiceIssued"
            ),
            "{:?}",
            parts.attaches
        );
        assert_eq!(parts.content.len(), 1, "{:?}", parts.content);
        match parts.due {
            Some(Term::Apply { ref ctor, ref args }) if ctor == "after" => {
                assert_eq!(args.len(), 2, "{args:?}");
            }
            Some(Term::Apply { ref ctor, .. }) if ctor == "counted_days" => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parse_expr_mul_tighter_than_add() {
        match parse_expr_src("1 + 2 * 3") {
            Some(Term::Apply { ctor, args }) if ctor == "+" => {
                assert_eq!(args[0], Term::Int(1));
                match &args[1] {
                    Term::Apply { ctor, args } if ctor == "*" => {
                        assert_eq!(args[0], Term::Int(2));
                        assert_eq!(args[1], Term::Int(3));
                    }
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parse_expr_subtraction_is_left_associative() {
        match parse_expr_src("1 - 2 - 3") {
            Some(Term::Apply { ctor, args }) if ctor == "-" => {
                match &args[0] {
                    Term::Apply { ctor, args: inner } if ctor == "-" => {
                        assert_eq!(inner[0], Term::Int(1));
                        assert_eq!(inner[1], Term::Int(2));
                    }
                    other => panic!("{other:?}"),
                }
                assert_eq!(args[1], Term::Int(3));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parse_expr_malformed_add_is_none() {
        assert_eq!(parse_expr_src("1 +"), None);
    }

    #[test]
    fn parse_evaluate_true() {
        let body = parse_query_body("query q() -> Bool { goal Evaluate { true } }");
        assert!(
            matches!(
                body,
                HirQueryBody::Goal {
                    ref kind,
                    expr: Some(Term::Bool(true)),
                    ..
                } if kind == "Evaluate"
            ),
            "{body:?}"
        );
    }

    #[test]
    fn parse_rule_operative_derive() {
        let src = "rule R : derive { when operative P() then derive Q() }";
        let parts = parse_rule_parts(src);
        assert!(
            matches!(
                parts.guard,
                Some(Guard::Operative(ref p, _)) if p.predicate == "P"
            ),
            "{:?}",
            parts.guard
        );
        assert_eq!(parts.consequences.len(), 1);
        assert_eq!(parts.consequences[0].0, "derive");
        assert_eq!(parts.consequences[0].1.predicate, "Q");
        assert!(parts.fallback.is_empty());
        assert!(parts.require.is_none());
    }

    #[test]
    fn term_to_guard_bool_false_is_not_satisfied() {
        assert_eq!(expr_to_guard(&Expr::Bool(true)), Guard::Satisfied);
        assert_eq!(
            expr_to_guard(&Expr::Bool(false)),
            Guard::Not(Box::new(Guard::Satisfied))
        );
        assert_ne!(expr_to_guard(&Expr::Bool(false)), Guard::Satisfied);
        let parts = parse_rule_parts("rule R : derive { when false then derive P() }");
        assert_eq!(
            parts.guard,
            Some(Guard::Not(Box::new(Guard::Satisfied))),
            "{:?}",
            parts.guard
        );
    }

    #[test]
    fn parse_rule_otherwise_is_fallback() {
        let parts =
            parse_rule_parts("rule R : derive { when true then derive P() otherwise derive Q() }");
        assert_eq!(parts.guard, Some(Guard::Satisfied));
        assert_eq!(parts.consequences.len(), 1);
        assert_eq!(parts.consequences[0].1.predicate, "P");
        assert_eq!(parts.fallback.len(), 1);
        assert_eq!(parts.fallback[0].1.predicate, "Q");
    }

    #[test]
    fn parse_rule_require_false_is_not_dropped() {
        let parts = parse_rule_parts("rule R : derive { when true require false then derive P() }");
        assert_eq!(parts.guard, Some(Guard::Satisfied));
        assert_eq!(
            parts.require,
            Some(Guard::Not(Box::new(Guard::Satisfied))),
            "{:?}",
            parts.require
        );
    }

    #[test]
    fn parse_fn_call_body() {
        let (_, body) = parse_function_parts("fn countdown(n: Int) -> Int { countdown(n - 1) }");
        assert!(
            matches!(
                body,
                Some(Term::Call { ref callee, .. }) if callee == "countdown"
            ) || matches!(
                body,
                Some(Term::Apply { ref ctor, .. }) if ctor == "countdown"
            ),
            "{body:?}"
        );
    }

    #[test]
    fn money_type_is_primitive() {
        assert_eq!(
            parse_type_name("Money<USD>"),
            Type::Primitive(PrimitiveType::Money {
                currency: "USD".into()
            })
        );
        assert_eq!(parse_type_name("Bool"), Type::bool());
    }

    #[test]
    fn decimal_expr_lowers_to_term_decimal() {
        let expected = Decimal::from_str("11925.00").expect("decimal");
        assert_eq!(
            expr_to_term(&Expr::Decimal("11925.00".into())),
            Term::Decimal(expected)
        );
        assert_eq!(parse_expr_src("11925.00"), Some(Term::Decimal(expected)));
        assert_eq!(
            expr_to_term(&Expr::Decimal("not-a-number".into())),
            Term::Ident("not-a-number".into())
        );
    }

    #[test]
    fn money_expr_lowers_to_currency_apply() {
        let expected = Decimal::from_str("11925.00").expect("decimal");
        let usd = expr_to_term(&Expr::Money {
            currency: "USD".into(),
            amount: "11925.00".into(),
        });
        let eur = expr_to_term(&Expr::Money {
            currency: "EUR".into(),
            amount: "11925.00".into(),
        });
        assert_eq!(
            usd,
            Term::Apply {
                ctor: "USD".into(),
                args: vec![Term::Decimal(expected)],
            }
        );
        assert_eq!(
            eur,
            Term::Apply {
                ctor: "EUR".into(),
                args: vec![Term::Decimal(expected)],
            }
        );
        assert_ne!(usd, eur);
    }

    #[test]
    fn block_of_two_exprs_lowers_to_seq() {
        let term = expr_to_term(&Expr::Block(vec![Expr::Bool(false), Expr::Bool(true)]));
        match term {
            Term::Apply { ctor, args } if ctor == "seq" => {
                assert_eq!(args, vec![Term::Bool(false), Term::Bool(true)]);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            expr_to_term(&Expr::Block(vec![Expr::Bool(true)])),
            Term::Bool(true)
        );
    }

    #[test]
    fn require_false_return_true_slice_is_seq_not_bool_true() {
        let body = parse_query_body("query q() -> Bool { require false; return true }");
        match body {
            HirQueryBody::Return(term) => {
                assert_ne!(term, Term::Bool(true), "{term:?}");
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
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn evaluate_require_false_return_true_slice_keeps_both() {
        let body =
            parse_query_body("query q() -> Bool { goal Evaluate { require false; return true } }");
        match body {
            HirQueryBody::Goal {
                ref kind,
                expr: Some(term),
                ..
            } if kind == "Evaluate" => {
                assert_ne!(term, Term::Bool(true), "{term:?}");
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
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn for_all_apply_keeps_three_args() {
        let expr = Expr::Apply {
            callee: Box::new(Expr::Ident("for_all".into())),
            args: vec![
                Expr::Ident("x".into()),
                Expr::Ident("People".into()),
                Expr::Call {
                    callee: "Eligible".into(),
                    args: vec![Expr::Ident("x".into())],
                },
            ],
        };
        match expr_to_term(&expr) {
            Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => {
                assert_eq!(ctor, "for_all");
                assert_eq!(args.len(), 3, "{args:?}");
                assert_eq!(args[0], Term::Ident("x".into()));
                assert_eq!(args[1], Term::Ident("People".into()));
            }
            other => panic!("{other:?}"),
        }

        let exists = Expr::Apply {
            callee: Box::new(Expr::Ident("exists".into())),
            args: vec![
                Expr::Ident("y".into()),
                Expr::Ident("People".into()),
                Expr::Call {
                    callee: "Trustee".into(),
                    args: vec![Expr::Ident("y".into())],
                },
            ],
        };
        match expr_to_term(&exists) {
            Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => {
                assert_eq!(ctor, "exists");
                assert_eq!(args.len(), 3, "{args:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    fn duty_step_expr(name: &str, action: &str) -> Expr {
        Expr::Call {
            callee: "duty_step".into(),
            args: vec![Expr::Ident(name.into()), Expr::Ident(action.into())],
        }
    }

    fn duty_step_call(name: &str, action: &str) -> Term {
        Term::Call {
            callee: "duty_step".into(),
            args: vec![Term::Ident(name.into()), Term::Ident(action.into())],
        }
    }

    fn duty_step_apply(name: &str, action: &str) -> Term {
        Term::Apply {
            ctor: "duty_step".into(),
            args: vec![Term::Ident(name.into()), Term::Ident(action.into())],
        }
    }

    fn expect_transaction_args(term: &Term) -> &[Term] {
        match term {
            Term::Apply { ctor, args } if ctor == "transaction" => args,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn transaction_block_lowers_to_apply_not_seq() {
        let expr = Expr::Apply {
            callee: Box::new(Expr::Ident("transaction".into())),
            args: vec![
                duty_step_expr("pay", "attach"),
                duty_step_expr("pay", "discharge"),
            ],
        };
        let term = expr_to_term(&expr);
        let args = expect_transaction_args(&term);
        assert_eq!(args.len(), 2, "{args:?}");
        assert_eq!(args[0], duty_step_call("pay", "attach"));
        assert_eq!(args[1], duty_step_call("pay", "discharge"));
        assert!(!args.iter().any(|arg| matches!(
            arg,
            Term::Apply { ctor, .. } if ctor == "seq"
        )));
    }

    #[test]
    fn transaction_block_wrapped_as_one_block_arg_is_flattened() {
        let expr = Expr::Apply {
            callee: Box::new(Expr::Ident("transaction".into())),
            args: vec![Expr::Block(vec![
                duty_step_expr("pay", "attach"),
                duty_step_expr("pay", "discharge"),
            ])],
        };
        let term = expr_to_term(&expr);
        let args = expect_transaction_args(&term);
        assert_eq!(args.len(), 2, "{args:?}");
        assert_eq!(args[0], duty_step_call("pay", "attach"));
        assert_eq!(args[1], duty_step_call("pay", "discharge"));
    }

    #[test]
    fn single_step_transaction_is_not_unwrapped() {
        let expr = Expr::Apply {
            callee: Box::new(Expr::Ident("transaction".into())),
            args: vec![duty_step_expr("pay", "attach")],
        };
        let term = expr_to_term(&expr);
        let args = expect_transaction_args(&term);
        assert_eq!(args, &[duty_step_call("pay", "attach")]);
        assert_ne!(term, duty_step_call("pay", "attach"));
    }

    #[test]
    fn transaction_call_lowers_to_apply() {
        let expr = Expr::Call {
            callee: "transaction".into(),
            args: vec![
                duty_step_expr("pay", "attach"),
                duty_step_expr("pay", "discharge"),
            ],
        };
        let term = expr_to_term(&expr);
        let args = expect_transaction_args(&term);
        assert_eq!(args.len(), 2, "{args:?}");
        assert_eq!(args[0], duty_step_call("pay", "attach"));
        assert_eq!(args[1], duty_step_call("pay", "discharge"));
    }

    #[test]
    fn parse_expr_src_transaction_block_keeps_both_steps() {
        let term =
            parse_expr_src("transaction { duty_step(pay, attach); duty_step(pay, discharge) }")
                .expect("transaction block");
        let args = expect_transaction_args(&term);
        assert_eq!(args.len(), 2, "{args:?}");
        assert_eq!(args[0], duty_step_apply("pay", "attach"));
        assert_eq!(args[1], duty_step_apply("pay", "discharge"));
    }

    #[test]
    fn parse_query_body_transaction_block_is_not_seq() {
        let body = parse_query_body(
            "query q() -> Bool { transaction { duty_step(pay, attach); duty_step(pay, discharge) } }",
        );
        match body {
            HirQueryBody::Return(term) => {
                let args = expect_transaction_args(&term);
                assert_eq!(args.len(), 2, "{args:?}");
            }
            other => panic!("{other:?}"),
        }
    }
}
