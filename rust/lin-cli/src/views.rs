//! Read-only views: outline, index, flat, grep, diff.

use lin::{Map, Value};
use regex::Regex;

use crate::paths;

/// Keys and nesting with list lengths, no values. For a list of objects the outline shows the
/// keys of all its items, in the order they first appear.
pub fn outline(v: &Value, max_depth: Option<usize>, types: bool) -> String {
    let mut out = Vec::new();
    match v {
        Value::Array(a) => {
            out.push(format!("[{}]", a.len()));
            outline_items(a, 1, max_depth, types, &mut out);
        }
        Value::Object(m) => outline_obj(m, 0, max_depth, types, &mut out),
        _ => {}
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

fn type_word(v: &Value) -> &'static str {
    v.type_name()
}

fn outline_obj(m: &Map, depth: usize, max: Option<usize>, types: bool, out: &mut Vec<String>) {
    let p = " ".repeat(depth);
    for (k, v) in m {
        let kk = lin::write::quote_key(k);
        match v {
            Value::Object(o) if !o.is_empty() => {
                out.push(format!("{p}{kk}"));
                if max.map_or(true, |m| depth + 1 < m) {
                    outline_obj(o, depth + 1, max, types, out);
                }
            }
            Value::Array(a) => {
                out.push(format!("{p}{kk}[{}]", a.len()));
                if max.map_or(true, |m| depth + 1 < m) {
                    outline_items(a, depth + 1, max, types, out);
                }
            }
            _ if types => out.push(format!("{p}{kk} {}", type_word(v))),
            _ => out.push(format!("{p}{kk}")),
        }
    }
}

fn outline_items(
    a: &[Value],
    depth: usize,
    max: Option<usize>,
    types: bool,
    out: &mut Vec<String>,
) {
    // union of object items' keys; first item of a nested list
    let mut merged = Map::new();
    let mut any_obj = false;
    for x in a {
        if let Value::Object(o) = x {
            any_obj = true;
            for (k, v) in o {
                let e = merged.entry(k.clone()).or_insert(Value::Null);
                if matches!(e, Value::Null) {
                    *e = v.clone();
                }
            }
        }
    }
    if any_obj {
        outline_obj(&merged, depth, max, types, out);
    } else if let Some(Value::Array(inner)) = a.iter().find(|x| matches!(x, Value::Array(_))) {
        out.push(format!("{}-[{}]", " ".repeat(depth), inner.len()));
        outline_items(inner, depth + 1, max, types, out);
    } else if types {
        let mut ts: Vec<&str> = a.iter().map(type_word).collect();
        ts.dedup();
        if !ts.is_empty() {
            out.push(format!("{}- {}", " ".repeat(depth), ts.join("|")));
        }
    }
}

/// `path value` for every scalar and empty container: the document flattened, one line each.
pub fn flat(v: &Value) -> String {
    let mut out = String::new();
    paths::walk(v, &mut vec![], &mut |p, x| {
        let leaf = match x {
            Value::Array(a) => a.is_empty(),
            Value::Object(o) => o.is_empty(),
            _ => true,
        };
        if leaf && !p.is_empty() {
            out.push_str(&paths::fmt(p));
            out.push(' ');
            out.push_str(&lin::write::scalar_text(x));
            out.push('\n');
        }
    });
    out
}

/// Rebuild a document from `flat` lines.
pub fn unflat(text: &str) -> Result<Value, String> {
    let mut root: Option<Value> = None;
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (p, val) = split_flat_line(line)
            .ok_or_else(|| format!("line {}: expected `path value`", n + 1))?;
        let path = paths::parse(p).map_err(|e| format!("line {}: {e}", n + 1))?;
        let value = parse_scalar_arg(val).map_err(|e| format!("line {}: {e}", n + 1))?;
        if root.is_none() {
            root = Some(
                if path
                    .first()
                    .is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit()))
                {
                    Value::Array(vec![])
                } else {
                    Value::Object(Map::new())
                },
            );
        }
        let r = root.as_mut().unwrap();
        // create containers along the path
        for i in 1..path.len() {
            let next_is_index = path[i].bytes().all(|b| b.is_ascii_digit());
            if paths::get(r, &path[..i]).is_none() {
                let c = if next_is_index {
                    Value::Array(vec![])
                } else {
                    Value::Object(Map::new())
                };
                paths::set(r, &path[..i], c).map_err(|e| format!("line {}: {e}", n + 1))?;
            }
        }
        paths::set(r, &path, value).map_err(|e| format!("line {}: {e}", n + 1))?;
    }
    Ok(root.unwrap_or(Value::Object(Map::new())))
}

fn split_flat_line(line: &str) -> Option<(&str, &str)> {
    // the path may contain quoted keys with spaces
    let b = line.as_bytes();
    let mut i = 0;
    let mut in_str = false;
    while i < b.len() {
        match b[i] {
            b'\\' if in_str => i += 1,
            b'"' => in_str = !in_str,
            b' ' if !in_str => return Some((&line[..i], &line[i + 1..])),
            _ => {}
        }
        i += 1;
    }
    None
}

/// A scalar written as in LIN: `~`, `true`, `12`, `[]`, `{}`, a JSON string, or bare text.
pub fn parse_scalar_arg(s: &str) -> Result<Value, String> {
    let text = format!("LIN 1\nv {s}\n");
    lin::from_str(&text)
        .map(|v| v.get("v").cloned().unwrap_or(Value::Null))
        .map_err(|e| e.message)
}

pub struct GrepOpts {
    pub re: Regex,
    pub keys_only: bool,
    pub values_only: bool,
}

/// Matches as (path, the value or key line that matched).
pub fn grep(v: &Value, o: &GrepOpts) -> Vec<String> {
    let mut out = Vec::new();
    paths::walk(v, &mut vec![], &mut |p, x| {
        let key_hit = !o.values_only && p.last().is_some_and(|k| o.re.is_match(k));
        let val_hit = !o.keys_only
            && match x {
                Value::Array(_) | Value::Object(_) => false,
                Value::String(s) => o.re.is_match(s),
                other => o.re.is_match(&lin::write::scalar_text(other)),
            };
        if key_hit || val_hit {
            let shown = match x {
                Value::Array(a) if !a.is_empty() => format!("[{}]", a.len()),
                Value::Object(m) if !m.is_empty() => format!("{{{}}}", m.len()),
                other => lin::write::scalar_text(other),
            };
            out.push(format!("{} {}", paths::fmt(p), shown));
        }
    });
    out
}

/// One line per object with an id: path, id, and its title or name.
pub fn index(v: &Value) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    paths::walk(v, &mut vec![], &mut |p, x| {
        if let Value::Object(m) = x {
            if let Some(id) = m.get("id") {
                if matches!(id, Value::String(_) | Value::Number(_)) {
                    let title = ["title", "name"]
                        .iter()
                        .find_map(|k| m.get(*k).and_then(Value::as_str))
                        .unwrap_or("")
                        .to_string();
                    out.push((paths::fmt(p), lin::write::scalar_text(id), title));
                }
            }
        }
    });
    out
}

// ------------------------------------------------------------------ diff

pub fn diff(a: &Value, b: &Value) -> Vec<String> {
    let mut out = Vec::new();
    diff_at(a, b, &mut vec![], &mut out);
    out
}

fn show(v: &Value) -> String {
    match v {
        Value::Array(x) if !x.is_empty() => lin::json::to_string(v),
        Value::Object(x) if !x.is_empty() => lin::json::to_string(v),
        _ => lin::write::scalar_text(v),
    }
}

fn diff_at(a: &Value, b: &Value, path: &mut Vec<String>, out: &mut Vec<String>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, va) in x {
                path.push(k.clone());
                match y.get(k) {
                    Some(vb) => diff_at(va, vb, path, out),
                    None => out.push(format!("- {} {}", paths::fmt(path), show(va))),
                }
                path.pop();
            }
            for (k, vb) in y {
                if !x.contains_key(k) {
                    path.push(k.clone());
                    out.push(format!("+ {} {}", paths::fmt(path), show(vb)));
                    path.pop();
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => diff_lists(x, y, path, out),
        _ => out.push(format!("~ {} {} -> {}", paths::fmt(path), show(a), show(b))),
    }
}

/// Align lists by their longest common subsequence, so an insertion shows as one `+`.
fn diff_lists(x: &[Value], y: &[Value], path: &mut Vec<String>, out: &mut Vec<String>) {
    let (n, m) = (x.len(), y.len());
    if n * m > 4_000_000 {
        for i in 0..n.max(m) {
            path.push(i.to_string());
            match (x.get(i), y.get(i)) {
                (Some(a), Some(b)) => diff_at(a, b, path, out),
                (Some(a), None) => out.push(format!("- {} {}", paths::fmt(path), show(a))),
                (None, Some(b)) => out.push(format!("+ {} {}", paths::fmt(path), show(b))),
                _ => {}
            }
            path.pop();
        }
        return;
    }
    let mut l = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            l[i][j] = if x[i] == y[j] {
                l[i + 1][j + 1] + 1
            } else {
                l[i + 1][j].max(l[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut dels: Vec<usize> = Vec::new();
    let mut adds: Vec<usize> = Vec::new();
    let flush = |dels: &mut Vec<usize>,
                 adds: &mut Vec<usize>,
                 path: &mut Vec<String>,
                 out: &mut Vec<String>| {
        // pair a removed item with an added one at the same place: show what changed inside
        let k = dels.len().min(adds.len());
        for t in 0..k {
            path.push(adds[t].to_string());
            diff_at(&x[dels[t]], &y[adds[t]], path, out);
            path.pop();
        }
        for &d in &dels[k..] {
            path.push(d.to_string());
            out.push(format!("- {} {}", paths::fmt(path), show(&x[d])));
            path.pop();
        }
        for &a in &adds[k..] {
            path.push(a.to_string());
            out.push(format!("+ {} {}", paths::fmt(path), show(&y[a])));
            path.pop();
        }
        dels.clear();
        adds.clear();
    };
    while i < n || j < m {
        if i < n && j < m && x[i] == y[j] {
            flush(&mut dels, &mut adds, path, out);
            i += 1;
            j += 1;
        } else if j < m && (i == n || l[i][j + 1] >= l[i + 1][j]) {
            adds.push(j);
            j += 1;
        } else {
            dels.push(i);
            i += 1;
        }
    }
    flush(&mut dels, &mut adds, path, out);
}
