//! Paths into documents.
//!
//! A path is keys and list positions joined by dots: `plugins.0.title`. A key with a dot,
//! whitespace or `"` in it is written as a JSON string: `files."a.json".size`. A path starting
//! with `/` is a JSON Pointer (RFC 6901). The empty path, or `.`, is the root.
//! Tools print paths in this form, and every command that takes a path accepts it.

use lin::{json, Value};

use crate::Res;

pub fn parse(s: &str) -> Res<Vec<String>> {
    if s.is_empty() || s == "." {
        return Ok(vec![]);
    }
    if let Some(p) = s.strip_prefix('/') {
        return Ok(p
            .split('/')
            .map(|x| x.replace("~1", "/").replace("~0", "~"))
            .collect());
    }
    let mut out = Vec::new();
    let mut i = 0;
    let b = s.as_bytes();
    loop {
        if b.get(i) == Some(&b'"') {
            let (k, e) = json::decode_string(s, i).map_err(|m| format!("bad path {s:?}: {m}"))?;
            out.push(k);
            i = e;
        } else {
            let j = s[i..].find('.').map_or(s.len(), |p| i + p);
            if j == i {
                return Err(format!("bad path {s:?}: empty segment"));
            }
            out.push(s[i..j].to_string());
            i = j;
        }
        if i == s.len() {
            return Ok(out);
        }
        if b[i] != b'.' {
            return Err(format!("bad path {s:?}: expected . after a quoted key"));
        }
        i += 1;
        if i == s.len() {
            return Err(format!("bad path {s:?}: ends with ."));
        }
    }
}

fn plain_seg(k: &str) -> bool {
    !k.is_empty()
        && !k
            .chars()
            .any(|c| c == '.' || c == '"' || lin::text::is_ws(c) || lin::text::is_bad(c))
        && !k.starts_with('/')
}

pub fn fmt(path: &[String]) -> String {
    if path.is_empty() {
        return ".".into();
    }
    path.iter()
        .map(|k| {
            if plain_seg(k) {
                k.clone()
            } else {
                json::quote(k)
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn index(seg: &str, len: usize) -> Option<usize> {
    if seg.is_empty() || !seg.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    seg.parse::<usize>().ok().filter(|i| *i < len)
}

pub fn get<'v>(v: &'v Value, path: &[String]) -> Option<&'v Value> {
    let mut cur = v;
    for seg in path {
        cur = match cur {
            Value::Object(m) => m.get(seg)?,
            Value::Array(a) => &a[index(seg, a.len())?],
            _ => return None,
        };
    }
    Some(cur)
}

pub fn get_mut<'v>(v: &'v mut Value, path: &[String]) -> Option<&'v mut Value> {
    let mut cur = v;
    for seg in path {
        cur = match cur {
            Value::Object(m) => m.get_mut(seg)?,
            Value::Array(a) => {
                let i = index(seg, a.len())?;
                &mut a[i]
            }
            _ => return None,
        };
    }
    Some(cur)
}

/// Set the value at `path`. The parent must exist; a missing last key is added at the end of
/// its object, and `-` (or the list's length) appends to a list.
pub fn set(doc: &mut Value, path: &[String], value: Value) -> Res<()> {
    let (last, parent) = match path.split_last() {
        None => {
            *doc = value;
            return Ok(());
        }
        Some(x) => x,
    };
    let p = get_mut(doc, parent).ok_or_else(|| format!("no value at {}", fmt(parent)))?;
    match p {
        Value::Object(m) => {
            m.insert(last.clone(), value);
            Ok(())
        }
        Value::Array(a) => {
            if last == "-" || *last == a.len().to_string() {
                a.push(value);
                Ok(())
            } else {
                let i = index(last, a.len()).ok_or_else(|| {
                    format!("{} is not a position in the list at {}", last, fmt(parent))
                })?;
                a[i] = value;
                Ok(())
            }
        }
        _ => Err(format!(
            "the value at {} is not an object or a list",
            fmt(parent)
        )),
    }
}

/// Insert into a list before position `last` (JSON Patch `add` semantics for lists).
pub fn insert(doc: &mut Value, path: &[String], value: Value) -> Res<()> {
    let (last, parent) = path.split_last().ok_or("cannot insert at the root")?;
    match get_mut(doc, parent).ok_or_else(|| format!("no value at {}", fmt(parent)))? {
        Value::Array(a) => {
            if last == "-" {
                a.push(value);
            } else {
                let i: usize = last
                    .parse()
                    .map_err(|_| format!("bad list position {last}"))?;
                if i > a.len() {
                    return Err(format!(
                        "position {i} is past the end of the list at {}",
                        fmt(parent)
                    ));
                }
                a.insert(i, value);
            }
            Ok(())
        }
        Value::Object(m) => {
            m.insert(last.clone(), value);
            Ok(())
        }
        _ => Err(format!(
            "the value at {} is not an object or a list",
            fmt(parent)
        )),
    }
}

pub fn remove(doc: &mut Value, path: &[String]) -> Res<Value> {
    let (last, parent) = path.split_last().ok_or("cannot remove the root")?;
    match get_mut(doc, parent).ok_or_else(|| format!("no value at {}", fmt(parent)))? {
        Value::Object(m) => m
            .shift_remove(last)
            .ok_or_else(|| format!("no key {last:?} at {}", fmt(parent))),
        Value::Array(a) => {
            let i = index(last, a.len())
                .ok_or_else(|| format!("no position {last} in the list at {}", fmt(parent)))?;
            Ok(a.remove(i))
        }
        _ => Err(format!(
            "the value at {} is not an object or a list",
            fmt(parent)
        )),
    }
}

/// Visit every value with its path, in document order (containers before their contents).
pub fn walk<'v>(v: &'v Value, path: &mut Vec<String>, f: &mut dyn FnMut(&[String], &'v Value)) {
    f(path, v);
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                path.push(k.clone());
                walk(x, path, f);
                path.pop();
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                path.push(i.to_string());
                walk(x, path, f);
                path.pop();
            }
        }
        _ => {}
    }
}

/// Every object with an id that can name an owner: (path, id).
pub fn owners(doc: &Value) -> Vec<(Vec<String>, String)> {
    let mut out = Vec::new();
    walk(doc, &mut vec![], &mut |p, v| {
        if let Some(id) = v.get("id") {
            if let Some(seg) = lin::text::id_segment_of(id) {
                out.push((p.to_vec(), seg));
            }
        }
    });
    out
}

/// Find a value the way `lin get` does: an object by its id, a reference name
/// (`<owner id>.<path>`), or a path from the root.
pub fn lookup(doc: &Value, name: &str) -> Res<(Vec<String>, Value)> {
    let path = parse(name)?;
    if !name.starts_with('/') && !path.is_empty() {
        let owners = owners(doc);
        if let Some((p, _)) = owners.iter().find(|(_, id)| *id == path[0]) {
            let mut full = p.clone();
            full.extend(path[1..].iter().cloned());
            if let Some(v) = get(doc, &full) {
                return Ok((full, v.clone()));
            }
        }
    }
    get(doc, &path)
        .map(|v| (path.clone(), v.clone()))
        .ok_or_else(|| format!("nothing at {name:?}: not an id, a reference name or a path"))
}
