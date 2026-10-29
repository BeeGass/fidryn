use fidryn_core::{Value, canonical_json};
use std::collections::BTreeMap;

#[test]
fn finite_sets_ignore_order_and_duplicate_occurrences() {
    let a = Value::Set(vec![Value::Int(1), Value::Int(2)]);
    let b = Value::Set(vec![Value::Int(2), Value::Int(1), Value::Int(2)]);
    assert_eq!(a, b);
    assert_eq!(canonical_json(&a).unwrap(), canonical_json(&b).unwrap());
    assert_ne!(a, Value::Set(vec![Value::Int(1)]));
}

#[test]
fn sets_preserve_variant_identity() {
    let a = Value::Set(vec![Value::Bool(true), Value::String("true".into())]);
    let b = Value::Set(vec![Value::Bool(true), Value::Entity("true".into())]);
    assert_ne!(a, b);
    assert_ne!(canonical_json(&a).unwrap(), canonical_json(&b).unwrap());
}

#[test]
fn nested_sets_and_decimal_scale_have_stable_set_identity() {
    let row = |decimal, values| {
        Value::Map(BTreeMap::from([
            ("amount".into(), Value::Decimal(decimal)),
            ("values".into(), Value::Set(values)),
        ]))
    };
    let a = Value::Set(vec![row(
        "1.0".parse().unwrap(),
        vec![Value::Int(1), Value::Int(2)],
    )]);
    let b = Value::Set(vec![row(
        "1.00".parse().unwrap(),
        vec![Value::Int(2), Value::Int(1), Value::Int(1)],
    )]);
    assert_eq!(a, b);
    assert_eq!(canonical_json(&a).unwrap(), canonical_json(&b).unwrap());
    let round_trip: Value = serde_json::from_str(&canonical_json(&a).unwrap()).unwrap();
    assert_eq!(round_trip, a);
}
