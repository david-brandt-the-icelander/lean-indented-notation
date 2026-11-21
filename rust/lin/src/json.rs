//! Strict JSON in, canonical JSON out (the same text as the Python and TypeScript libraries).

use crate::value::{Map, Number, Value};
use crate::Error;

/// Parse JSON text. Duplicate keys are errors, as in LIN; numbers keep their exact value.
pub fn parse(text: &str) -> Result<Value, Error> {
    let mut p = P {
        s: text.as_bytes(),
        i: 0,
        text,
    };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err(p.err("text after the JSON value"));
    }
    Ok(v)
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
    text: &'a str,
}

impl<'a> P<'a> {
    fn err(&self, msg: &str) -> Error {
        let before = &self.text[..self.i.min(self.text.len())];
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        Error::new(format!("bad JSON: {msg} at line {line} column {col}"))
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.s.get(self.i) {
            self.i += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, Error> {
        if depth > 10_000 {
            return Err(self.err("nested too deeply"));
        }
        match self.s.get(self.i) {
            Some(b'{') => {
                self.i += 1;
                let mut m = Map::new();
                self.ws();
                if self.s.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Value::Object(m));
                }
                loop {
                    self.ws();
                    if self.s.get(self.i) != Some(&b'"') {
                        return Err(self.err("expected a string key"));
                    }
                    let k = self.string()?;
                    self.ws();
                    if self.s.get(self.i) != Some(&b':') {
                        return Err(self.err("expected ':'"));
                    }
                    self.i += 1;
                    self.ws();
                    let v = self.value(depth + 1)?;
                    if m.contains_key(&k) {
                        return Err(Error::new(format!("duplicate key {k:?} in JSON")));
                    }
                    m.insert(k, v);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Value::Object(m));
                        }
                        _ => return Err(self.err("expected ',' or '}'")),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Value::Array(a));
                }
                loop {
                    self.ws();
                    a.push(self.value(depth + 1)?);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Value::Array(a));
                        }
                        _ => return Err(self.err("expected ',' or ']'")),
                    }
                }
            }
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') => self.lit("true", Value::Bool(true)),
            Some(b'f') => self.lit("false", Value::Bool(false)),
            Some(b'n') => self.lit("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => {
                let start = self.i;
                while let Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') = self.s.get(self.i)
                {
                    self.i += 1;
                }
                let t = &self.text[start..self.i];
                Number::from_json_text(t).map(Value::Number).map_err(|e| {
                    self.i = start;
                    self.err(&e.message)
                })
            }
            Some(_) => Err(self.err("unexpected character")),
            None => Err(self.err("unexpected end of input")),
        }
    }

    fn lit(&mut self, word: &str, v: Value) -> Result<Value, Error> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(self.err("unexpected word"))
        }
    }

    fn string(&mut self) -> Result<String, Error> {
        let (s, end) = decode_string(self.text, self.i).map_err(|m| self.err(&m))?;
        self.i = end;
        Ok(s)
    }
}

/// Decode the JSON string starting at byte `start` (a `"`). Returns the string and the byte
/// offset just past its closing quote.
pub fn decode_string(text: &str, start: usize) -> Result<(String, usize), String> {
    let b = text.as_bytes();
    if b.get(start) != Some(&b'"') {
        return Err("expected a JSON string".into());
    }
    let mut out = String::new();
    let mut i = start + 1;
    loop {
        let rest = &text[i..];
        let c = match rest.chars().next() {
            None => return Err("unterminated string".into()),
            Some(c) => c,
        };
        match c {
            '"' => return Ok((out, i + 1)),
            '\\' => {
                let e = *b.get(i + 1).ok_or("unterminated string")?;
                i += 2;
                match e {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{8}'),
                    b'f' => out.push('\u{c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hi = hex4(text, i)?;
                        i += 4;
                        if (0xD800..0xDC00).contains(&hi) {
                            if b.get(i) == Some(&b'\\') && b.get(i + 1) == Some(&b'u') {
                                let lo = hex4(text, i + 2)?;
                                if (0xDC00..0xE000).contains(&lo) {
                                    i += 6;
                                    let cp = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                                    out.push(char::from_u32(cp).ok_or("bad surrogate pair")?);
                                    continue;
                                }
                            }
                            return Err("lone surrogate in \\u escape".into());
                        }
                        if (0xDC00..0xE000).contains(&hi) {
                            return Err("lone surrogate in \\u escape".into());
                        }
                        out.push(char::from_u32(hi).ok_or("bad \\u escape")?);
                    }
                    _ => return Err(format!("invalid escape \\{}", e as char)),
                }
            }
            c if (c as u32) < 0x20 => return Err("control character in string".into()),
            c => {
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
}

fn hex4(text: &str, i: usize) -> Result<u32, String> {
    let h = text.get(i..i + 4).ok_or("bad \\u escape")?;
    if !h.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("bad \\u escape".into());
    }
    u32::from_str_radix(h, 16).map_err(|_| "bad \\u escape".into())
}

/// A JSON string as `JSON.stringify` and Python's `json.dumps(ensure_ascii=False)` write it.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn scalar(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(n.as_str()),
        Value::String(s) => out.push_str(&quote(s)),
        _ => unreachable!(),
    }
}

/// Compact JSON on one line, no final newline.
pub fn to_string(v: &Value) -> String {
    let mut out = String::new();
    compact(v, &mut out);
    out
}

fn compact(v: &Value, out: &mut String) {
    match v {
        Value::Object(m) => {
            out.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&quote(k));
                out.push(':');
                compact(x, out);
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                compact(x, out);
            }
            out.push(']');
        }
        _ => scalar(v, out),
    }
}

/// Indented JSON with a final newline: the layout of `JSON.stringify(v, null, 2)`.
pub fn to_string_pretty(v: &Value) -> String {
    let mut out = String::new();
    pretty(v, 0, &mut out);
    out.push('\n');
    out
}

fn pretty(v: &Value, level: usize, out: &mut String) {
    match v {
        Value::Object(m) if !m.is_empty() => {
            out.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                out.push_str(if i == 0 { "\n" } else { ",\n" });
                out.push_str(&"  ".repeat(level + 1));
                out.push_str(&quote(k));
                out.push_str(": ");
                pretty(x, level + 1, out);
            }
            out.push('\n');
            out.push_str(&"  ".repeat(level));
            out.push('}');
        }
        Value::Array(a) if !a.is_empty() => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                out.push_str(if i == 0 { "\n" } else { ",\n" });
                out.push_str(&"  ".repeat(level + 1));
                pretty(x, level + 1, out);
            }
            out.push('\n');
            out.push_str(&"  ".repeat(level));
            out.push(']');
        }
        Value::Object(_) => out.push_str("{}"),
        Value::Array(_) => out.push_str("[]"),
        _ => scalar(v, out),
    }
}
