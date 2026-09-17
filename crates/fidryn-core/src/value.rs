//! Runtime values and proposition terms. No `From<PropTerm> for bool`.

use crate::ids::NodeId;
use crate::time::{FidrynDuration, Instant};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Term {
    Bool(bool),
    Int(i64),
    Decimal(Decimal),
    String(String),
    Instant(Instant),
    Duration(FidrynDuration),
    Ident(String),
    Apply {
        ctor: String,
        args: Vec<Term>,
    },
    Set(Vec<Term>),
    Record(BTreeMap<String, Term>),
    Wildcard,
    Binder(String),
    Binary {
        op: BinOp,
        left: Box<Term>,
        right: Box<Term>,
    },
    Call {
        callee: String,
        args: Vec<Term>,
    },
    If {
        cond: Box<Term>,
        then: Box<Term>,
        #[serde(rename = "else")]
        else_: Box<Term>,
    },
    Field {
        base: Box<Term>,
        name: String,
    },
}

impl Term {
    pub fn unit_ctor(name: impl Into<String>) -> Self {
        Term::Apply {
            ctor: name.into(),
            args: Vec::new(),
        }
    }
}

/// A ground proposition application. Never a Boolean.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PropTerm {
    pub predicate: String,
    pub arguments: Vec<Term>,
}

impl PropTerm {
    pub fn new(predicate: impl Into<String>, arguments: Vec<Term>) -> Self {
        Self {
            predicate: predicate.into(),
            arguments,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Decimal(Decimal),
    String(String),
    Instant(Instant),
    Duration(FidrynDuration),
    Prop(PropTerm),
    Entity(String),
    Ctor {
        name: String,
        fields: BTreeMap<String, Value>,
    },
    Set(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Option(Option<Box<Value>>),
    ClauseRef {
        module: String,
        clause: String,
        arguments: Vec<Value>,
        digest: NodeId,
    },
}

impl Value {
    pub fn display_label(&self) -> String {
        match self {
            Value::Entity(n) | Value::String(n) => n.clone(),
            Value::Ctor { name, .. } => name.clone(),
            Value::Prop(p) => p.predicate.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            other => format!("{other:?}"),
        }
    }
}

/// Case JSON: bare literals for primitives, tagged objects for Entity and the rest.
pub(crate) fn value_from_case_json(raw: serde_json::Value) -> Result<Value, String> {
    match raw {
        serde_json::Value::Null => Ok(Value::Unit),
        serde_json::Value::Bool(b) => Ok(Value::Bool(b)),
        serde_json::Value::Number(n) => number_to_value(&n),
        serde_json::Value::String(s) => Ok(Value::String(s)),
        serde_json::Value::Array(items) => {
            let values = items
                .into_iter()
                .map(value_from_case_json)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Set(values))
        }
        serde_json::Value::Object(map) => {
            let obj = serde_json::Value::Object(map);
            if obj.get("kind").is_some() {
                serde_json::from_value(obj).map_err(|e| e.to_string())
            } else {
                map_from_case_json(obj)
            }
        }
    }
}

fn number_to_value(n: &serde_json::Number) -> Result<Value, String> {
    if let Some(i) = n.as_i64() {
        return Ok(Value::Int(i));
    }
    if let Some(u) = n.as_u64()
        && u <= i64::MAX as u64
    {
        return Ok(Value::Int(u as i64));
    }
    match Decimal::from_str(&n.to_string()) {
        Ok(d) => Ok(Value::Decimal(d)),
        Err(_) => Ok(Value::String(n.to_string())),
    }
}

fn map_from_case_json(obj: serde_json::Value) -> Result<Value, String> {
    let serde_json::Value::Object(map) = obj else {
        return Err("expected object".into());
    };
    let mut out = BTreeMap::new();
    for (k, v) in map {
        out.insert(k, value_from_case_json(v)?);
    }
    Ok(Value::Map(out))
}

pub(crate) mod case_value {
    use super::*;

    pub fn serialize<S: Serializer>(value: &Value, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Int(i) => serializer.serialize_i64(*i),
            Value::String(s) => serializer.serialize_str(s),
            Value::Decimal(d) => serializer.serialize_str(&d.to_string()),
            Value::Unit => serializer.serialize_unit(),
            other => other.serialize(serializer),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
        let raw = serde_json::Value::deserialize(deserializer)?;
        value_from_case_json(raw).map_err(serde::de::Error::custom)
    }
}

pub(crate) mod case_value_map {
    use super::*;
    use serde::ser::SerializeMap;

    struct CaseWire<'a>(&'a Value);

    impl Serialize for CaseWire<'_> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            case_value::serialize(self.0, serializer)
        }
    }

    pub fn serialize<S: Serializer>(
        map: &BTreeMap<String, Value>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut ser = serializer.serialize_map(Some(map.len()))?;
        for (k, v) in map {
            ser.serialize_entry(k, &CaseWire(v))?;
        }
        ser.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<String, Value>, D::Error> {
        let raw = BTreeMap::<String, serde_json::Value>::deserialize(deserializer)?;
        raw.into_iter()
            .map(|(k, v)| {
                value_from_case_json(v)
                    .map(|val| (k, val))
                    .map_err(serde::de::Error::custom)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prop_term_is_not_bool() {
        let p = PropTerm::new("Incapacitated", vec![Term::Ident("Bryan".into())]);
        assert_ne!(Value::Prop(p), Value::Bool(true));
    }

    #[test]
    fn entity_value_round_trips_as_entity_not_string() {
        let entity = Value::Entity("Alice".into());
        let json = serde_json::to_value(&entity).unwrap();
        assert_eq!(json["kind"], "entity");
        assert_eq!(json["data"], "Alice");
        let back: Value = serde_json::from_value(json).unwrap();
        assert_eq!(back, entity);

        let string = Value::String("Alice".into());
        let string_json = serde_json::to_value(&string).unwrap();
        assert_eq!(string_json["kind"], "string");
        let string_back: Value = serde_json::from_value(string_json).unwrap();
        assert_eq!(string_back, string);
        assert_ne!(entity, string);
    }

    #[test]
    fn case_json_bare_string_is_not_entity() {
        let raw = serde_json::json!("Alice");
        assert_eq!(
            value_from_case_json(raw).unwrap(),
            Value::String("Alice".into())
        );
        let tagged = serde_json::json!({"kind": "entity", "data": "Alice"});
        assert_eq!(
            value_from_case_json(tagged).unwrap(),
            Value::Entity("Alice".into())
        );
    }
}
