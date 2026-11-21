//! RFC 8785 JSON Canonicalization and LIN schema ids (spec section 7).

use sha2::{Digest, Sha256};

use crate::json::quote;
use crate::value::{Number, Value};

/// The JCS text of a value: keys sorted by UTF-16 code units, numbers as the IEEE double they
/// denote in ECMAScript form, no whitespace.
pub fn canonicalize(v: &Value) -> String {
    let mut out = String::new();
    canon(v, &mut out);
    out
}

fn canon(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            let d = Number::from_f64(n.as_f64()).unwrap_or_else(|| n.clone());
            out.push_str(d.as_str())
        }
        Value::String(s) => out.push_str(&quote(s)),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canon(x, out);
            }
            out.push(']');
        }
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&quote(k));
                out.push(':');
                canon(&m[*k], out);
            }
            out.push('}');
        }
    }
}

/// The SHA-256 of the schema's canonical JSON, in hex: what a registry records.
pub fn schema_sha256(schema: &Value) -> String {
    Sha256::digest(canonicalize(schema).as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The schema's provisional id: the first 128 bits of its SHA-256 as a 39-digit decimal number.
/// Digits are the cheapest text for tokenizers, which split them three at a time.
pub fn schema_id(schema: &Value) -> String {
    let d = Sha256::digest(canonicalize(schema).as_bytes());
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    format!("{:039}", u128::from_be_bytes(b))
}

/// The two forms of schema id a header can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdKind {
    /// A number (1 to 9 digits, no leading zero) assigned by the official registry.
    Registered,
    /// 39 digits: 128 bits of the schema's SHA-256.
    Provisional,
}

/// What kind of schema id `s` is, if it is one.
pub fn id_kind(s: &str) -> Option<IdKind> {
    let digits = !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if digits && (1..=9).contains(&s.len()) && !s.starts_with('0') {
        Some(IdKind::Registered)
    } else if digits && s.len() == 39 {
        Some(IdKind::Provisional)
    } else {
        None
    }
}
