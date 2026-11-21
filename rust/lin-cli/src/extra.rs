//! Schema inference and validation, CSV, token counts, jq queries.

use lin::{json, Map, Number, Value};

use crate::Res;

pub const META: &str = "sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars";

// ------------------------------------------------------------ validation

/// Validate `doc` against `schema`; each error as `path: message`.
pub fn validate(doc: &Value, schema: &Value) -> Res<Vec<String>> {
    let s: serde_json::Value = schema.into();
    let d: serde_json::Value = doc.into();
    let v = jsonschema::validator_for(&s).map_err(|e| format!("the schema is not valid: {e}"))?;
    Ok(v.iter_errors(&d)
        .map(|e| {
            let p = e.instance_path().to_string();
            let path = if p.is_empty() {
                ".".to_string()
            } else {
                crate::paths::fmt(&crate::paths::parse(&p).unwrap_or_default())
            };
            format!("{path}: {e}")
        })
        .collect())
}

// ------------------------------------------------------------- inference

/// A JSON Schema (2020-12) that the given documents all satisfy.
pub fn infer(docs: &[Value]) -> Value {
    let mut s = infer_many(&docs.iter().collect::<Vec<_>>());
    if let Value::Object(m) = &mut s {
        let mut out = Map::new();
        out.insert(
            "$schema".into(),
            "https://json-schema.org/draft/2020-12/schema".into(),
        );
        for (k, v) in m.iter() {
            out.insert(k.clone(), v.clone());
        }
        *m = out;
    }
    s
}

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Number(n) if n.is_integer_text() => "integer",
        other => other.type_name(),
    }
}

fn infer_many(vs: &[&Value]) -> Value {
    let mut kinds: Vec<&str> = Vec::new();
    for v in vs {
        let k = kind(v);
        if !kinds.contains(&k) {
            kinds.push(k);
        }
    }
    if kinds.contains(&"integer") && kinds.contains(&"number") {
        kinds.retain(|k| *k != "integer");
    }
    let mut m = Map::new();
    if kinds.len() == 1 {
        m.insert("type".into(), kinds[0].into());
    } else if !kinds.is_empty() {
        m.insert(
            "type".into(),
            Value::Array(kinds.iter().map(|k| (*k).into()).collect()),
        );
    }
    let objs: Vec<&Map> = vs.iter().filter_map(|v| v.as_object()).collect();
    if !objs.is_empty() {
        let mut props = Map::new();
        let mut keys: Vec<&String> = Vec::new();
        for o in &objs {
            for k in o.keys() {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
        }
        for k in &keys {
            let vals: Vec<&Value> = objs.iter().filter_map(|o| o.get(*k)).collect();
            props.insert((*k).clone(), infer_many(&vals));
        }
        let required: Vec<Value> = keys
            .iter()
            .filter(|k| objs.iter().all(|o| o.contains_key(**k)))
            .map(|k| Value::String((*k).clone()))
            .collect();
        m.insert("properties".into(), Value::Object(props));
        if !required.is_empty() {
            m.insert("required".into(), Value::Array(required));
        }
    }
    let items: Vec<&Value> = vs.iter().filter_map(|v| v.as_array()).flatten().collect();
    if vs.iter().any(|v| v.as_array().is_some()) && !items.is_empty() {
        m.insert("items".into(), infer_many(&items));
    }
    Value::Object(m)
}

// ------------------------------------------------------------------ CSV

/// A list of flat objects as CSV (RFC 4180), columns in order of first appearance.
pub fn to_csv(v: &Value) -> Res<String> {
    let rows = v
        .as_array()
        .ok_or("CSV needs a list of objects (a table)")?;
    let mut cols: Vec<String> = Vec::new();
    for r in rows {
        let o = r
            .as_object()
            .ok_or("CSV needs a list of objects (a table)")?;
        for k in o.keys() {
            if !cols.contains(k) {
                cols.push(k.clone());
            }
        }
    }
    let mut out = String::new();
    out.push_str(
        &cols
            .iter()
            .map(|c| csv_field(c))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');
    for r in rows {
        let o = r.as_object().unwrap();
        let line: Vec<String> = cols
            .iter()
            .map(|c| match o.get(c) {
                None | Some(Value::Null) => String::new(),
                Some(Value::String(s)) => csv_field(s),
                Some(x @ (Value::Array(_) | Value::Object(_))) => csv_field(&json::to_string(x)),
                Some(x) => lin::write::scalar_text(x),
            })
            .collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    Ok(out)
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// CSV with a header row to a list of objects. Number and boolean text becomes numbers and
/// booleans unless `strings`; empty fields become null.
pub fn from_csv(text: &str, strings: bool) -> Res<Value> {
    let rows = parse_csv(text)?;
    let mut it = rows.into_iter();
    let header = it.next().ok_or("empty CSV")?;
    let mut out = Vec::new();
    for (n, r) in it.enumerate() {
        if r.len() == 1 && r[0].is_empty() {
            continue;
        }
        if r.len() != header.len() {
            return Err(format!(
                "CSV row {} has {} fields, the header has {}",
                n + 2,
                r.len(),
                header.len()
            ));
        }
        let mut o = Map::new();
        for (k, f) in header.iter().zip(r) {
            let v = if f.is_empty() {
                Value::Null
            } else if !strings && (f == "true" || f == "false") {
                Value::Bool(f == "true")
            } else if !strings && lin::value::is_json_number(&f) {
                Value::Number(Number::from_json_text(&f).map_err(|e| e.message)?)
            } else {
                Value::String(f)
            };
            o.insert(k.clone(), v);
        }
        out.push(Value::Object(o));
    }
    Ok(Value::Array(out))
}

fn parse_csv(text: &str) -> Res<Vec<Vec<String>>> {
    let mut rows = vec![];
    let mut row = vec![];
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    let mut any = false;
    while let Some(c) = chars.next() {
        any = true;
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else {
            match c {
                '"' if field.is_empty() => quoted = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\r' => {}
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                    any = false;
                }
                c => field.push(c),
            }
        }
    }
    if quoted {
        return Err("unterminated quoted CSV field".into());
    }
    if any {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

// --------------------------------------------------------------- tokens

#[cfg(feature = "tokens")]
pub fn token_counter() -> Res<impl Fn(&str) -> usize> {
    let bpe = tiktoken_rs::o200k_base().map_err(|e| e.to_string())?;
    Ok(move |s: &str| bpe.encode_with_special_tokens(s).len())
}

// ---------------------------------------------------------------- query

pub struct Query {
    pub filter: String,
    pub vars: Vec<(String, Value)>,
}

/// Run a jq filter (the jaq implementation) on a value; every output value.
pub fn run_query(q: &Query, input: &Value) -> Res<Vec<Value>> {
    use jaq_core::load::{Arena, File, Loader};
    use jaq_core::{data, unwrap_valr, Compiler, Ctx, Vars};
    use jaq_json::Val;

    let defs = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(jaq_json::defs());
    let funs = jaq_core::funs()
        .chain(jaq_std::funs())
        .chain(jaq_json::funs());
    let loader = Loader::new(defs);
    let arena = Arena::default();
    let program = File {
        code: q.filter.as_str(),
        path: (),
    };
    let modules = loader
        .load(&arena, program)
        .map_err(|errs| format!("cannot parse the filter: {errs:?}"))?;
    let names: Vec<String> = q.vars.iter().map(|(k, _)| format!("${k}")).collect();
    let filter = Compiler::default()
        .with_funs(funs)
        .with_global_vars(names.iter().map(String::as_str))
        .compile(modules)
        .map_err(|errs| format!("cannot compile the filter: {errs:?}"))?;
    let to_val = |v: &Value| -> Res<Val> {
        jaq_json::read::parse_single(json::to_string(v).as_bytes()).map_err(|e| e.to_string())
    };
    let vars: Vec<Val> = q.vars.iter().map(|(_, v)| to_val(v)).collect::<Res<_>>()?;
    let ctx = Ctx::<data::JustLut<Val>>::new(&filter.lut, Vars::new(vars));
    let mut out = Vec::new();
    for r in filter.id.run((ctx, to_val(input)?)).map(unwrap_valr) {
        let v = r.map_err(|e| format!("query error: {e}"))?;
        let text = v.to_string();
        out.push(json::parse(&text).unwrap_or(Value::String(text)));
    }
    Ok(out)
}
