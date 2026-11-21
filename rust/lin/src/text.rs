//! Character rules shared by the reader and writer.

use crate::value::Value;

/// Whitespace as Python's `str.isspace()` defines it; every implementation uses this set.
pub fn is_ws(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\u{1c}'..='\u{1f}' | ' ' | '\u{85}' | '\u{a0}'
        | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

/// Characters a bare string may not contain: controls, DEL, NEL and the Unicode line separators.
pub fn is_bad(c: char) -> bool {
    (c as u32) < 0x20 || matches!(c, '\u{7f}' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

pub fn bad_char(s: &str) -> bool {
    s.chars().any(is_bad)
}

fn key_char(c: char) -> bool {
    !is_ws(c) && !matches!(c, '"' | '[' | ']' | '{' | '}' | '|')
}

fn key_first(c: char) -> bool {
    key_char(c) && !matches!(c, '+' | '#' | '-')
}

/// The byte length of the bare key at the start of `s` (0 if there is none).
pub fn key_len_at(s: &str) -> usize {
    let mut it = s.char_indices();
    match it.next() {
        Some((_, c)) if key_first(c) => {}
        _ => return 0,
    }
    for (i, c) in it {
        if !key_char(c) {
            return i;
        }
    }
    s.len()
}

/// Can `k` be written as a bare key?
pub fn bare_key_ok(k: &str) -> bool {
    !k.is_empty() && key_len_at(k) == k.len() && !bad_char(k)
}

/// Characters that end a reference-name segment.
pub fn seg_stop(c: char) -> bool {
    c == '.' || is_ws(c) || c == '"' || c == '|' || c == '@'
}

pub const SEG_STOP: fn(char) -> bool = seg_stop;

/// Can `s` be a segment of a reference name?
pub fn seg_ok(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(seg_stop) && !bad_char(s)
}

/// The name segment an `id` value gives its object, if it makes the object an owner.
pub fn id_segment_of(v: &Value) -> Option<String> {
    let s = match v {
        Value::String(s) => s.clone(),
        Value::Number(n) if n.is_integer_text() => n.as_str().to_string(),
        _ => return None,
    };
    if seg_ok(&s) {
        Some(s)
    } else {
        None
    }
}

/// Length in characters (Unicode code points), as every limit counts it.
pub fn clen(s: &str) -> usize {
    s.chars().count()
}
