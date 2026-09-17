//! Fidryn canonical JSON encoding `fidryn.canonical/v0.2+rfc8785`.
//!
//! Deterministic JSON used by covering hashes, driver run keys, module
//! fingerprints, and CLI snapshots. Replay is byte-identical under this
//! encoding.
//!
//! This is RFC 8785 (JSON Canonicalization Scheme):
//! - no insignificant whitespace
//! - object keys sorted by UTF-16 code units, not UTF-8 / Unicode scalars
//! - numbers follow ECMA-262 `NumberToString` (Note 2 / Ryu) except that
//!   integers `serde_json` stores as `i64`/`u64` are emitted as decimal
//!   without exponent (`1` not `1.0`; `i64::MAX` is not rounded through
//!   IEEE-754)
//! - strings use `serde_json` quoting, which matches JCS for ASCII and
//!   for the RFC 8785 string sample
//!
//! ASCII-only covering hashes match `fidryn.canonical/v0.1` because ASCII
//! key order equals UTF-16 order and i64/string spelling is unchanged.
//! The schema id still changes: Unicode key order and non-integer number
//! spelling now follow RFC 8785.

use serde::Serialize;
use serde_json::{Number, Value};

/// Schema id for this encoding (RFC 8785 JCS, with integer spelling above).
pub const CANONICAL_SCHEMA: &str = "fidryn.canonical/v0.2+rfc8785";

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
        Value::Number(n) => format_number(n),
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
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
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

/// Integers `serde_json` stores as `i64`/`u64` stay decimal without exponent.
/// Other numbers use RFC 8785 / ECMA-262 `NumberToString`.
fn format_number(n: &Number) -> String {
    if n.is_i64() || n.is_u64() {
        return n.to_string();
    }
    let Some(float) = n.as_f64() else {
        return n.to_string();
    };
    format_jcs_f64(float)
}

fn format_jcs_f64(value: f64) -> String {
    debug_assert!(
        value.is_finite(),
        "RFC 8785 forbids NaN and Infinity in JSON numbers"
    );
    ryu_js::Buffer::new().format_finite(value).to_owned()
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
            "JCS has no insignificant whitespace: {}",
            canonicalize(value)
        );
    }

    #[test]
    fn schema_id_is_fidryn_canonical_v0_2_rfc8785() {
        assert_eq!(CANONICAL_SCHEMA, "fidryn.canonical/v0.2+rfc8785");
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
        assert_canonical(&json!(u64::MAX), "18446744073709551615");
    }

    #[test]
    fn floats_use_rfc8785_number_format() {
        assert_canonical(&json!(1.5), "1.5");
        assert_canonical(&json!(4.5), "4.5");
        assert_canonical(&json!(0.1), "0.1");
        // Integer-valued f64 uses JCS "1", not serde_json's "1.0".
        let one = Number::from_f64(1.0).expect("finite");
        assert_canonical(&Value::Number(one), "1");
        let bits = |u: u64, expected: &str| {
            let n = Number::from_f64(f64::from_bits(u)).expect("finite");
            assert_canonical(&Value::Number(n), expected);
        };
        // RFC 8785 Appendix B.
        bits(0x0000_0000_0000_0000, "0");
        bits(0x8000_0000_0000_0000, "0");
        bits(0x0000_0000_0000_0001, "5e-324");
        bits(0x8000_0000_0000_0001, "-5e-324");
        bits(0x7fefffffffffffff, "1.7976931348623157e+308");
        bits(0xffefffffffffffff, "-1.7976931348623157e+308");
        bits(0x4340_0000_0000_0000, "9007199254740992");
        bits(0xc340_0000_0000_0000, "-9007199254740992");
        bits(0x4430_0000_0000_0000, "295147905179352830000");
        bits(0x44b5_2d02_c7e1_4af5, "9.999999999999997e+22");
        bits(0x44b5_2d02_c7e1_4af6, "1e+23");
        bits(0x44b5_2d02_c7e1_4af7, "1.0000000000000001e+23");
        bits(0x444b_1ae4_d6e2_ef4e, "999999999999999700000");
        bits(0x444b_1ae4_d6e2_ef4f, "999999999999999900000");
        bits(0x444b_1ae4_d6e2_ef50, "1e+21");
        bits(0x3eb0_c6f7_a0b5_ed8c, "9.999999999999997e-7");
        bits(0x3eb0_c6f7_a0b5_ed8d, "0.000001");
        bits(0x41b3_de43_5555_5553, "333333333.3333332");
        bits(0x41b3_de43_5555_5554, "333333333.33333325");
        bits(0x41b3_de43_5555_5555, "333333333.3333333");
        bits(0x41b3_de43_5555_5556, "333333333.3333334");
        bits(0x41b3_de43_5555_5557, "333333333.33333343");
        bits(0xbecb_f647_612f_3696, "-0.0000033333333333333333");
        bits(0x4314_3ff3_c1cb_0959, "1424953923781206.2");
    }

    #[test]
    fn strings_use_serde_json_quoting() {
        assert_canonical(&json!("plain"), r#""plain""#);
        assert_canonical(&json!("a\"b"), r#""a\"b""#);
        assert_canonical(&json!("a\\b"), r#""a\\b""#);
        assert_canonical(&json!("line\nfeed"), r#""line\nfeed""#);
        assert_canonical(&json!({"k\"ey": "v"}), r#"{"k\"ey":"v"}"#);
        // RFC 8785 §3.2.2 sample after parse: serde_json matches JCS here.
        let rfc = "\u{20ac}$\u{000f}\nA'B\"\\\\\"/";
        assert_eq!(
            canonicalize(&Value::String(rfc.into())),
            r#""€$\u000f\nA'B\"\\\\\"/""#
        );
        assert_canonical(&json!("a/b"), r#""a/b""#);
        assert_canonical(&json!("\u{0008}"), r#""\b""#);
        assert_canonical(&json!("\u{0000}"), r#""\u0000""#);
    }

    #[test]
    fn unicode_keys_sort_by_utf16_code_units() {
        assert_canonical(&json!({"é": 1, "a": 2, "z": 3}), r#"{"a":2,"z":3,"é":1}"#);
        let nested = json!({"é":[{"ö":1,"a":0}],"b":true});
        assert_canonical(&nested, r#"{"b":true,"é":[{"a":0,"ö":1}]}"#);
    }

    #[test]
    fn combining_character_keys_sort_by_utf16_units() {
        // Precomposed U+00E9 vs combining U+0065 U+0301.
        // UTF-16: [0x00E9] vs [0x0065, 0x0301]. 0x0065 < 0x00E9, so the
        // combining sequence precedes the precomposed letter. UTF-8 / scalar
        // order agrees on this pair (U+0065 < U+00E9); it locks combining
        // keys, not a UTF-16-vs-byte divergence.
        let precomposed = "\u{00e9}";
        let combining = "e\u{0301}";
        assert_ne!(precomposed, combining);
        let value = json!({ combining: 1, precomposed: 2 });
        let key_combining = serde_json::to_string(combining).expect("key");
        let key_precomposed = serde_json::to_string(precomposed).expect("key");
        let expected = format!("{{{key_combining}:1,{key_precomposed}:2}}");
        assert_eq!(canonicalize(&value), expected);
    }

    #[test]
    fn supplementary_plane_key_sorts_before_high_bmp_under_utf16() {
        // U+E000 is a BMP private-use scalar (UTF-16 unit E000).
        // U+10000 encodes as UTF-16 D800 DC00. RFC 8785 sorts U+10000
        // first because D800 < E000. Scalar / UTF-8 order is the reverse
        // (0xE000 < 0x10000).
        let bmp_high = "\u{e000}";
        let supplementary = "\u{10000}";
        assert!(
            bmp_high < supplementary,
            "Rust str / Unicode scalar order puts U+E000 first"
        );
        let value = json!({ supplementary: 1, bmp_high: 2 });
        let encoded = canonicalize(&value);
        let key_bmp = serde_json::to_string(bmp_high).expect("key");
        let key_supp = serde_json::to_string(supplementary).expect("key");
        let expected = format!("{{{key_supp}:1,{key_bmp}:2}}");
        assert_eq!(encoded, expected);
        assert!(
            encoded.find(&key_supp).expect("supp") < encoded.find(&key_bmp).expect("bmp"),
            "RFC 8785 UTF-16 order puts U+10000 before U+E000: {encoded}"
        );
    }

    #[test]
    fn rfc8785_section_3_2_3_property_order() {
        let value = json!({
            "\u{20ac}": "Euro Sign",
            "\r": "Carriage Return",
            "\u{fb33}": "Hebrew Letter Dalet With Dagesh",
            "1": "One",
            "\u{1F600}": "Emoji: Grinning Face",
            "\u{0080}": "Control",
            "\u{00f6}": "Latin Small Letter O With Diaeresis"
        });
        let order = [
            "\r",
            "1",
            "\u{0080}",
            "\u{00f6}",
            "\u{20ac}",
            "\u{1F600}",
            "\u{fb33}",
        ];
        let mut expected = String::from("{");
        for (i, key) in order.iter().enumerate() {
            if i > 0 {
                expected.push(',');
            }
            expected.push_str(&serde_json::to_string(key).expect("key"));
            expected.push(':');
            expected.push_str(
                &serde_json::to_string(value[*key].as_str().expect("string value")).expect("value"),
            );
        }
        expected.push('}');
        assert_canonical(&value, &expected);
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

    #[test]
    fn ascii_fixture_bytes_match_v0_1() {
        // ASCII key order equals UTF-16 order; i64 and ASCII strings are
        // unchanged from fidryn.canonical/v0.1. Schema id is v0.2+rfc8785
        // because Unicode key order and non-integer numbers follow JCS.
        let v = json!({
            "z": [true, {"b": 1, "a": null}],
            "m": {"k": "v", "j": [2, 3]},
            "a": false
        });
        assert_eq!(
            canonicalize(&v),
            r#"{"a":false,"m":{"j":[2,3],"k":"v"},"z":[true,{"a":null,"b":1}]}"#
        );
        assert_eq!(canonicalize(&json!({"b": 1, "a": 2})), r#"{"a":2,"b":1}"#);
        assert_eq!(canonicalize(&json!(1)), "1");
        assert_eq!(canonicalize(&json!(i64::MAX)), "9223372036854775807");
    }
}
