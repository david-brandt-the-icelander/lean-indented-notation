//! RFC 8785 JSON Canonicalization and LIN schema hashes (spec section 7).

use base64::Engine;
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

/// `sha256-` and the base64url (no padding) SHA-256 of the schema's canonical JSON.
pub fn schema_hash(schema: &Value) -> String {
    let digest = Sha256::digest(canonicalize(schema).as_bytes());
    format!(
        "sha256-{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
    )
}

/// Is `s` a schema hash as the header writes it?
pub fn is_hash(s: &str) -> bool {
    s.len() == 50
        && s.starts_with("sha256-")
        && s[7..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
