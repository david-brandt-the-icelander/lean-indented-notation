//! JSON values with ordered objects and exact numbers.

use indexmap::IndexMap;
use std::fmt;

use crate::Error;

/// An object: keys in the order they were written.
pub type Map = IndexMap<String, Value>;

/// A JSON value, as LIN holds it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(Map),
}

/// A number, kept as its canonical text: integers with all their digits, other numbers in the
/// shortest form JSON produces (ECMAScript `Number::toString`). `1.0` and `1` are the same number.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Number(String);

impl Number {
    /// Parse JSON number syntax (`-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?`).
    pub fn from_json_text(text: &str) -> Result<Number, Error> {
        if !is_json_number(text) {
            return Err(Error::new(format!("not a JSON number: {text}")));
        }
        Number::canonical(text)
    }

    fn canonical(text: &str) -> Result<Number, Error> {
        if text.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
            let digits = text.trim_start_matches('-');
            if digits.bytes().all(|b| b == b'0') {
                return Ok(Number("0".into()));
            }
            return Ok(Number(text.to_string()));
        }
        let f: f64 = text
            .parse()
            .map_err(|_| Error::new(format!("bad number: {text}")))?;
        Number::from_f64(f).ok_or_else(|| Error::new(format!("number out of range: {text}")))
    }

    /// A finite float in shortest ECMAScript form; `None` for NaN and infinities.
    pub fn from_f64(f: f64) -> Option<Number> {
        if !f.is_finite() {
            return None;
        }
        if f == 0.0 {
            return Some(Number("0".into()));
        }
        let mut buf = ryu_js::Buffer::new();
        Some(Number(buf.format_finite(f).to_string()))
    }

    pub fn from_i64(i: i64) -> Number {
        Number(i.to_string())
    }

    pub fn from_u64(i: u64) -> Number {
        Number(i.to_string())
    }

    /// The canonical text, as LIN and the reference JSON writers print it.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True when the canonical text is an integer (`-?[0-9]+`).
    pub fn is_integer_text(&self) -> bool {
        let t = self.0.strip_prefix('-').unwrap_or(&self.0);
        !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())
    }

    pub fn as_f64(&self) -> f64 {
        self.0.parse().unwrap_or(f64::NAN)
    }

    pub fn as_i64(&self) -> Option<i64> {
        if self.is_integer_text() {
            self.0.parse().ok()
        } else {
            None
        }
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Is `s` JSON number syntax?
pub fn is_json_number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    if b.get(i) == Some(&b'-') {
        i += 1;
    }
    match b.get(i) {
        Some(b'0') => i += 1,
        Some(c) if c.is_ascii_digit() => {
            while b.get(i).is_some_and(|c| c.is_ascii_digit()) {
                i += 1;
            }
        }
        _ => return false,
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while b.get(i).is_some_and(|c| c.is_ascii_digit()) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    if matches!(b.get(i), Some(b'e') | Some(b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+') | Some(b'-')) {
            i += 1;
        }
        let start = i;
        while b.get(i).is_some_and(|c| c.is_ascii_digit()) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    i == b.len()
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    pub fn as_object(&self) -> Option<&Map> {
        match self {
            Value::Object(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object().and_then(|m| m.get(key))
    }

    pub fn is_container(&self) -> bool {
        matches!(self, Value::Array(_) | Value::Object(_))
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::String(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::String(s)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}
impl From<i64> for Value {
    fn from(i: i64) -> Value {
        Value::Number(Number::from_i64(i))
    }
}

#[cfg(feature = "serde_json")]
mod serde_conv {
    use super::*;

    impl From<&Value> for serde_json::Value {
        fn from(v: &Value) -> serde_json::Value {
            match v {
                Value::Null => serde_json::Value::Null,
                Value::Bool(b) => serde_json::Value::Bool(*b),
                Value::Number(n) => serde_json::from_str(n.as_str())
                    .or_else(|_| {
                        serde_json::Number::from_f64(n.as_f64())
                            .map(serde_json::Value::Number)
                            .ok_or(())
                    })
                    .unwrap_or(serde_json::Value::Null),
                Value::String(s) => serde_json::Value::String(s.clone()),
                Value::Array(a) => serde_json::Value::Array(a.iter().map(Into::into).collect()),
                Value::Object(m) => serde_json::Value::Object(
                    m.iter().map(|(k, v)| (k.clone(), v.into())).collect(),
                ),
            }
        }
    }

    impl From<&serde_json::Value> for Value {
        fn from(v: &serde_json::Value) -> Value {
            match v {
                serde_json::Value::Null => Value::Null,
                serde_json::Value::Bool(b) => Value::Bool(*b),
                serde_json::Value::Number(n) => Value::Number(
                    Number::from_json_text(&n.to_string()).unwrap_or_else(|_| Number("0".into())),
                ),
                serde_json::Value::String(s) => Value::String(s.clone()),
                serde_json::Value::Array(a) => Value::Array(a.iter().map(Into::into).collect()),
                serde_json::Value::Object(m) => {
                    Value::Object(m.iter().map(|(k, v)| (k.clone(), v.into())).collect())
                }
            }
        }
    }
}
