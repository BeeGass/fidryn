//! Fidryn canonical JSON encoding `fidryn.canonical/v0.1`.
//!
//! Deterministic JSON used by covering hashes, driver run keys, module
//! fingerprints, and CLI snapshots. Replay is byte-identical under this
//! encoding.
//!
//! This is **not** RFC 8785 (JSON Canonicalization Scheme):
//! - object keys are sorted by Rust `str` / Unicode scalar-value order,
//!   not UTF-16 code units
//! - numbers are `serde_json::Number::to_string()`, not JCS number format
//! - strings use `serde_json` quoting, not JCS hex escapes
//!
//! Covering hashes and cache keys bind this encoding. Changing the
//! algorithm in place would silently invalidate them; a new
//! `fidryn.canonical/v0.x` id is required.

use serde::Serialize;
use serde_json::Value;

/// Schema id for this encoding. Not RFC 8785.
pub const CANONICAL_SCHEMA: &str = "fidryn.canonical/v0.1";

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

    fn assert_canonical(value: &Value, expected: &str) {
        assert_eq!(
            canonicalize(value),
            expected,
            "{CANONICAL_SCHEMA} vector failed for {value}"
        );
        assert_eq!(canonicalize(value), canonicalize(value));
        assert!(
            !canonicalize(value).contains(' ') || expected.contains(' '),
            "v0.1 has no insignificant whitespace: {}",
            canonicalize(value)
        );
    }

    #[test]
    fn schema_id_is_fidryn_canonical_v0_1() {
        assert_eq!(CANONICAL_SCHEMA, "fidryn.canonical/v0.1");
    }

    #[test]
    fn object_keys_are_sorted() {
        assert_canonical(&json!({"b": 1, "a": 2}), r#"{"a":2,"b":1}"#);
    }

    #[test]
    fn nested_objects_sort_each_level() {
        assert_canonical(
            &json!({"z":{"b":1,"a":2},"a":{"d":0,"c":true}}),
            r#"{"a":{"c":true,"d":0},"z":{"a":2,"b":1}}"#,
        );
    }

    #[test]
    fn arrays_preserve_order_and_sort_nested_objects() {
        assert_canonical(
            &json!([{"b":1,"a":2},{"d":3,"c":4}]),
            r#"[{"a":2,"b":1},{"c":4,"d":3}]"#,
        );
        assert_canonical(&json!([3, 1, 2]), "[3,1,2]");
        assert_canonical(&json!([]), "[]");
    }

    #[test]
    fn empty_object_is_braces() {
        assert_canonical(&json!({}), "{}");
    }

    #[test]
    fn literals_have_no_whitespace() {
        assert_canonical(&json!(null), "null");
        assert_canonical(&json!(true), "true");
        assert_canonical(&json!(false), "false");
        assert_canonical(
            &json!({"a": [1, 2], "b": {"c": true}}),
            r#"{"a":[1,2],"b":{"c":true}}"#,
        );
    }

    #[test]
    fn integers_are_decimal_without_exponent() {
        assert_canonical(&json!(0), "0");
        assert_canonical(&json!(1), "1");
        assert_canonical(&json!(-7), "-7");
        assert_canonical(&json!(1000), "1000");
        assert_canonical(&json!(i64::MAX), "9223372036854775807");
        assert_canonical(&json!(i64::MIN), "-9223372036854775808");
    }

    #[test]
    fn numbers_use_serde_json_spelling() {
        assert_canonical(&json!(1.5), "1.5");
    }

    #[test]
    fn strings_use_serde_json_quoting() {
        assert_canonical(&json!("plain"), r#""plain""#);
        assert_canonical(&json!("a\"b"), r#""a\"b""#);
        assert_canonical(&json!("a\\b"), r#""a\\b""#);
        assert_canonical(&json!("line\nfeed"), r#""line\nfeed""#);
        assert_canonical(&json!({"k\"ey": "v"}), r#"{"k\"ey":"v"}"#);
    }

    #[test]
    fn unicode_keys_sort_by_scalar_value() {
        assert_canonical(&json!({"é": 1, "a": 2, "z": 3}), r#"{"a":2,"z":3,"é":1}"#);
        let nested = json!({"é":[{"ö":1,"a":0}],"b":true});
        assert_canonical(&nested, r#"{"b":true,"é":[{"a":0,"ö":1}]}"#);
    }

    #[test]
    fn unicode_key_order_is_not_rfc8785_utf16() {
        // U+E000 is a BMP private-use scalar (UTF-16 unit E000).
        // U+10000 encodes as UTF-16 D800 DC00. RFC 8785 would sort U+10000
        // first because D800 < E000. v0.1 uses Rust `str` / scalar order,
        // so U+E000 sorts first (0xE000 < 0x10000).
        let bmp_high = "\u{e000}";
        let supplementary = "\u{10000}";
        assert!(
            bmp_high < supplementary,
            "v0.1 key order is Unicode scalar / Rust str Ord"
        );
        let value = json!({ supplementary: 1, bmp_high: 2 });
        let encoded = canonicalize(&value);
        let key_bmp = serde_json::to_string(bmp_high).expect("key");
        let key_supp = serde_json::to_string(supplementary).expect("key");
        let expected = format!("{{{key_bmp}:2,{key_supp}:1}}");
        assert_eq!(encoded, expected);
        assert!(
            encoded.find(&key_bmp).expect("bmp") < encoded.find(&key_supp).expect("supp"),
            "RFC 8785 UTF-16 order would reverse these keys: {encoded}"
        );
    }

    #[test]
    fn replay_is_byte_identical() {
        let v = json!({"z":[true,null],"m":{"k":"v"}});
        assert_eq!(canonicalize(&v), canonicalize(&v));
        assert_eq!(
            canonical_json(&v).unwrap(),
            r#"{"m":{"k":"v"},"z":[true,null]}"#
        );
        assert_eq!(
            canonical_to_vec(&v).unwrap(),
            canonical_json(&v).unwrap().into_bytes()
        );
    }

    #[test]
    fn mixed_nested_vector() {
        let v = json!({
            "z": [true, {"b": 1, "a": null}],
            "m": {"k": "v", "j": [2, 3]},
            "a": false
        });
        assert_canonical(
            &v,
            r#"{"a":false,"m":{"j":[2,3],"k":"v"},"z":[true,{"a":null,"b":1}]}"#,
        );
    }
}
