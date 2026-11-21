//! Changing documents: JSON Patch (RFC 6902), JSON Merge Patch (RFC 7386), deep merge.

use lin::{Map, Value};

use crate::{paths, Res};

fn pointer(v: &Value, key: &str, i: usize) -> Res<Vec<String>> {
    let p = v
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("patch operation {i}: missing \"{key}\""))?;
    if !p.is_empty() && !p.starts_with('/') {
        return Err(format!("patch operation {i}: {key} must be a JSON Pointer"));
    }
    paths::parse(if p.is_empty() { "." } else { p })
}

/// Apply an RFC 6902 JSON Patch (a list of operations).
pub fn json_patch(doc: &mut Value, patch: &Value) -> Res<()> {
    let ops = patch
        .as_array()
        .ok_or("a JSON Patch is a list of operations")?;
    for (i, op) in ops.iter().enumerate() {
        let name = op
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("patch operation {i}: missing \"op\""))?;
        let path = pointer(op, "path", i)?;
        let value = || {
            op.get("value")
                .cloned()
                .ok_or_else(|| format!("patch operation {i}: missing \"value\""))
        };
        match name {
            "add" => {
                if path.is_empty() {
                    *doc = value()?;
                } else {
                    paths::insert(doc, &path, value()?)?
                }
            }
            "remove" => {
                paths::remove(doc, &path)?;
            }
            "replace" => {
                if paths::get(doc, &path).is_none() {
                    return Err(format!(
                        "patch operation {i}: nothing to replace at {}",
                        paths::fmt(&path)
                    ));
                }
                paths::set(doc, &path, value()?)?
            }
            "move" => {
                let from = pointer(op, "from", i)?;
                let v = paths::remove(doc, &from)?;
                paths::insert(doc, &path, v)?
            }
            "copy" => {
                let from = pointer(op, "from", i)?;
                let v = paths::get(doc, &from)
                    .cloned()
                    .ok_or_else(|| format!("patch operation {i}: nothing at from"))?;
                paths::insert(doc, &path, v)?
            }
            "test" => {
                if paths::get(doc, &path) != Some(&value()?) {
                    return Err(format!(
                        "patch operation {i}: test failed at {}",
                        paths::fmt(&path)
                    ));
                }
            }
            other => return Err(format!("patch operation {i}: unknown op {other:?}")),
        }
    }
    Ok(())
}

/// Apply an RFC 7386 merge patch: objects merge, null removes, anything else replaces.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    if let Value::Object(p) = patch {
        if !matches!(target, Value::Object(_)) {
            *target = Value::Object(Map::new());
        }
        if let Value::Object(t) = target {
            for (k, v) in p {
                if matches!(v, Value::Null) {
                    t.shift_remove(k);
                } else {
                    merge_patch(t.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
        }
    } else {
        *target = patch.clone();
    }
}

/// Deep merge: objects merge key by key, later documents win; lists are replaced, or
/// concatenated with `concat`.
pub fn deep_merge(a: &mut Value, b: &Value, concat: bool) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in y {
                match x.get_mut(k) {
                    Some(old) => deep_merge(old, v, concat),
                    None => {
                        x.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if concat => x.extend(y.iter().cloned()),
        (a, b) => *a = b.clone(),
    }
}
