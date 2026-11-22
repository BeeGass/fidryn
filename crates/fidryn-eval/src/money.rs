//! Currency-preserving arithmetic for explicit monetary constructors.

use fidryn_core::{EngineError, Value};
use rust_decimal::Decimal;
use std::collections::BTreeMap;

pub(crate) fn try_binop(
    op: &str,
    left: &Value,
    right: &Value,
) -> Result<Option<Value>, EngineError> {
    let a = amount(left)?;
    let b = amount(right)?;
    let result = match (a, b) {
        (None, None) => return Ok(None),
        (Some((currency, a)), Some((other, b))) => {
            if currency != other {
                return Err(EngineError::InvalidInput(format!(
                    "currency mismatch: {currency} and {other}"
                )));
            }
            match op {
                "+" => monetary(currency, a.checked_add(b), op)?,
                "-" => monetary(currency, a.checked_sub(b), op)?,
                "==" => Value::Bool(a == b),
                "!=" => Value::Bool(a != b),
                "<" => Value::Bool(a < b),
                "<=" => Value::Bool(a <= b),
                ">" => Value::Bool(a > b),
                ">=" => Value::Bool(a >= b),
                _ => {
                    return Err(EngineError::Unsupported(format!(
                        "operator `{op}` requires a scalar operand for money"
                    )));
                }
            }
        }
        (Some((currency, a)), None) => {
            let b = scalar(right)?;
            match op {
                "*" => monetary(currency, a.checked_mul(b), op)?,
                "/" if b.is_zero() => {
                    return Err(EngineError::InvalidInput("division by zero".into()));
                }
                "/" => monetary(currency, a.checked_div(b), op)?,
                _ => {
                    return Err(EngineError::Unsupported(format!(
                        "operator `{op}` is not defined between money and a scalar"
                    )));
                }
            }
        }
        (None, Some((currency, b))) => {
            let a = scalar(left)?;
            match op {
                "*" => monetary(currency, a.checked_mul(b), op)?,
                _ => {
                    return Err(EngineError::Unsupported(format!(
                        "operator `{op}` is not defined between a scalar and money"
                    )));
                }
            }
        }
    };
    Ok(Some(result))
}

fn amount(value: &Value) -> Result<Option<(&str, Decimal)>, EngineError> {
    let Value::Ctor { name, fields } = value else {
        return Ok(None);
    };
    if !(3..=4).contains(&name.len()) || !name.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Ok(None);
    }
    if fields.len() != 1 || !fields.contains_key("_0") {
        return Err(EngineError::InvalidInput(format!(
            "currency constructor `{name}` requires exactly one numeric amount"
        )));
    }
    Ok(Some((name, scalar(&fields["_0"])?)))
}

fn scalar(value: &Value) -> Result<Decimal, EngineError> {
    match value {
        Value::Int(value) => Ok(Decimal::from(*value)),
        Value::Decimal(value) => Ok(*value),
        _ => Err(EngineError::Unsupported(
            "monetary amount and scale must be numeric values".into(),
        )),
    }
}

fn monetary(currency: &str, value: Option<Decimal>, op: &str) -> Result<Value, EngineError> {
    let value =
        value.ok_or_else(|| EngineError::InvalidInput(format!("monetary overflow in `{op}`")))?;
    Ok(Value::Ctor {
        name: currency.into(),
        fields: BTreeMap::from([("_0".into(), Value::Decimal(value))]),
    })
}
