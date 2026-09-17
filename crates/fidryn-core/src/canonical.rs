//! RFC 8785-style canonical JSON: sorted keys, no insignificant whitespace.

use serde::Serialize;
use serde_json::Value;

pub fn canonical_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let v = serde_json::to_value(value)?;
    Ok(canonicalize(&v))
}

pub fn canonical_to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    Ok(canonical_json(value)?.into_bytes())
}

fn canonicalize(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(true) => "true".into(),
        Value::Bool(false) => "false".into(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => serde_json::to_string(s).expect("string encodes"),
        Value::Array(items) => {
            let mut out = String::from("[");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&canonicalize(item));
            }
            out.push(']');
            out
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(*key).expect("key encodes"));
                out.push(':');
                out.push_str(&canonicalize(&map[*key]));
            }
            out.push('}');
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn object_keys_are_sorted() {
        let v = json!({"b": 1, "a": 2});
        assert_eq!(canonicalize(&v), r#"{"a":2,"b":1}"#);
    }

    #[test]
    fn replay_is_byte_identical() {
        let v = json!({"z":[true,null],"m":{"k":"v"}});
        assert_eq!(canonicalize(&v), canonicalize(&v));
    }
}
