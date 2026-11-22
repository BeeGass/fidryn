//! Source-path currency arithmetic and input admission regressions.

use fidryn_core::{
    CaseRecord, EngineError, Handler, HandlerResult, Instant, OpenRequest, Outcome, QueryName,
    RunContext, SourceManifest, SuspensionReason, Value,
};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet};

struct Refusing;
impl Handler for Refusing {
    fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
        HandlerResult::Suspend {
            requests: BTreeSet::from([request.clone()]),
            reason: SuspensionReason::MissingRecord,
            trace_fragment: String::new(),
        }
    }
    fn handle_determine(&mut self, request: &OpenRequest) -> HandlerResult {
        self.handle_observe(request)
    }
    fn handle_choose(&mut self, request: &OpenRequest) -> HandlerResult {
        self.handle_observe(request)
    }
    fn handle_interpret(&mut self, request: &OpenRequest) -> HandlerResult {
        self.handle_observe(request)
    }
}

fn run(body: &str, case: &CaseRecord) -> Result<Outcome<Value>, EngineError> {
    let parsed = fidryn_syntax::parse_file(&format!(
        "module Money.Source version \"0.1.0\" {{ {body} }}"
    ));
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let manifest = SourceManifest::default();
    let hir = fidryn_hir::elaborate(&parsed, &manifest).unwrap();
    let module = fidryn_check::check(&hir, &manifest).unwrap();
    let at = Instant::parse("2026-10-29T00:00:00Z").unwrap();
    fidryn_eval::evaluate(
        &module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &case.into_state(),
        &RunContext::new(at, at),
        &mut Refusing,
        case,
    )
}

fn money(currency: &str, amount: Decimal) -> Value {
    Value::Ctor {
        name: currency.into(),
        fields: BTreeMap::from([("_0".into(), Value::Decimal(amount))]),
    }
}

#[test]
fn source_money_arithmetic_preserves_currency_and_exact_decimal_amounts() {
    for (expression, expected) in [
        ("(USD(10.00) + USD(5.00)) * 0.10", "1.5"),
        ("2 * (USD(10.00) - USD(3.00))", "14"),
        ("USD(15.00) / 2", "7.5"),
    ] {
        let outcome = run(
            &format!("query q() -> Money<USD> {{ return {expression} }}"),
            &CaseRecord::default(),
        )
        .unwrap();
        assert!(
            matches!(outcome, Outcome::Determinate { value, .. } if value == money("USD", expected.parse().unwrap())),
            "{expression}"
        );
    }
    let comparison = run(
        "query q() -> Bool { return USD(10.00) < USD(11.00) }",
        &CaseRecord::default(),
    )
    .unwrap();
    assert!(matches!(
        comparison,
        Outcome::Determinate {
            value: Value::Bool(true),
            ..
        }
    ));
}

#[test]
fn runtime_money_admission_rejects_currency_and_string_substitution() {
    let body = "query q(x: Money<USD>) -> Money<USD> { return x + USD(1.00) }";
    for value in [
        money("EUR", Decimal::from(10)),
        Value::String("USD(10.00)".into()),
    ] {
        let case = CaseRecord {
            facts: BTreeMap::from([("x".into(), value)]),
            ..Default::default()
        };
        assert!(matches!(
            run(body, &case),
            Err(EngineError::InvalidInput(_))
        ));
    }
}

#[test]
fn monetary_overflow_and_division_by_zero_are_engine_errors() {
    let case = CaseRecord {
        facts: BTreeMap::from([("x".into(), money("USD", Decimal::MAX))]),
        ..Default::default()
    };
    assert!(matches!(
        run(
            "query q(x: Money<USD>) -> Money<USD> { return x + USD(1.00) }",
            &case
        ),
        Err(EngineError::InvalidInput(_))
    ));
    assert!(matches!(
        run(
            "query q() -> Money<USD> { return USD(1.00) / 0 }",
            &CaseRecord::default()
        ),
        Err(EngineError::InvalidInput(_))
    ));
}
