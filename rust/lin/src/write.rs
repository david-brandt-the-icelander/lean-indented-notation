//! The LIN writer. Its output is byte-for-byte the same as the Python and TypeScript writers'.

use std::collections::{HashMap, HashSet};

use crate::json::quote;
use crate::schema::{bare_reads_as_string, Pos, Typer};
use crate::text::{bad_char, bare_key_ok, clen, seg_ok};
use crate::value::{is_json_number, Number, Value};
use crate::Error;

/// Writer settings. The defaults are the spec's recommended defaults (section 8).
#[derive(Clone, Debug)]
pub struct WriteOptions {
    /// Write the split form: long values move to a data section after `---`.
    pub split: bool,
    /// The header's schema id: a registered number, or a 39-digit provisional id.
    pub schema: Option<String>,
    /// An https:// location of the schema, written after the hash.
    pub url: Option<String>,
    /// Strings longer than this many characters move to the data section in split form.
    pub split_limit: usize,
    /// Lists of scalars go inline unless an item is longer than this (`None`: always inline).
    pub inline_limit: Option<usize>,
    /// Objects of scalars go inline when their fields fit in this many characters (0: never).
    pub object_limit: usize,
    /// Write the `LIN 1` header line (off for fragments printed by tools).
    pub header: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            split: false,
            schema: None,
            url: None,
            split_limit: 24,
            inline_limit: None,
            object_limit: 80,
            header: true,
        }
    }
}

const HEADLINE: [&str; 6] = ["id", "kind", "title", "name", "status", "type"];

#[derive(Clone, Debug)]
enum W {
    Null,
    Bool(bool),
    Num(Number),
    /// A string; the flag says it may go bare although it looks like a number or boolean.
    Str(String, bool),
    Arr(Vec<W>),
    Obj(Vec<(String, W)>),
    Ref(Option<Vec<String>>, Box<W>),
}

fn from_value<'a>(v: &Value, t: &Typer<'a>, pos: &Pos<'a>) -> W {
    match v {
        Value::Null => W::Null,
        Value::Bool(b) => W::Bool(*b),
        Value::Number(n) => W::Num(n.clone()),
        Value::String(s) => {
            let looks = is_json_number(s) || s == "true" || s == "false";
            W::Str(
                s.clone(),
                looks && t.has_schema() && bare_reads_as_string(s, t.types(pos)),
            )
        }
        Value::Array(a) => W::Arr(
            a.iter()
                .enumerate()
                .map(|(i, x)| from_value(x, t, &t.item(pos, i)))
                .collect(),
        ),
        Value::Object(m) => W::Obj(
            m.iter()
                .map(|(k, x)| (k.clone(), from_value(x, t, &t.prop(pos, k))))
                .collect(),
        ),
    }
}

fn is_container(w: &W) -> bool {
    matches!(w, W::Arr(_) | W::Obj(_))
}

/// A key as written: bare when it can be, else a JSON string.
pub fn quote_key(k: &str) -> String {
    if bare_key_ok(k) {
        k.to_string()
    } else {
        quote(k)
    }
}

/// Can `s` be written as an indented multi-line block?
pub fn block_ok(s: &str) -> bool {
    let lines: Vec<&str> = s.split('\n').collect();
    !s.ends_with('\n')
        && !lines[0].is_empty()
        && lines.iter().all(|l| !l.ends_with(' ') && !bad_char(l))
}

/// The one-line form of a string value, or `None` when it goes in a block.
pub fn string_form(s: &str, cell: bool, bare_number_ok: bool) -> Option<String> {
    if s.contains('\n') && !cell && block_ok(s) {
        return None;
    }
    let looks = (is_json_number(s) || s == "true" || s == "false") && !bare_number_ok;
    let need = s.is_empty()
        || s.starts_with(' ')
        || s.starts_with('"')
        || s.starts_with('@')
        || s.ends_with(' ')
        || matches!(s, "~" | "[]" | "{}" | "|")
        || bad_char(s)
        || looks
        || (cell && s.contains('|'));
    Some(if need { quote(s) } else { s.to_string() })
}

struct Writer {
    o: WriteOptions,
}

impl Writer {
    fn scalar(&self, w: &W, cell: bool) -> Option<String> {
        match w {
            W::Ref(None, _) => Some("@".into()),
            W::Ref(Some(n), _) => Some(format!("@{}", n.join("."))),
            W::Null => Some("~".into()),
            W::Bool(true) => Some("true".into()),
            W::Bool(false) => Some("false".into()),
            W::Num(n) => Some(n.as_str().to_string()),
            W::Str(s, ok) => string_form(s, cell, *ok),
            _ => unreachable!("containers have no scalar form"),
        }
    }

    fn cell(&self, w: &W) -> String {
        self.scalar(w, true)
            .expect("cells always have a one-line form")
    }

    fn cells<'w>(&self, ws: impl Iterator<Item = &'w W>) -> String {
        ws.map(|w| self.cell(w)).collect::<Vec<_>>().join("|")
    }

    fn inline_ok(&self, l: &[W]) -> bool {
        !l.is_empty()
            && l.iter().all(|x| match x {
                W::Arr(_) | W::Obj(_) | W::Ref(..) => false,
                W::Str(s, _) => self.o.inline_limit.map_or(true, |lim| clen(s) <= lim),
                _ => true,
            })
    }

    fn obj_cells(&self, o: &[(String, W)]) -> String {
        o.iter()
            .map(|(k, v)| format!("{k} {}", self.cell(v)))
            .collect::<Vec<_>>()
            .join("|")
    }

    fn inline_obj_ok(&self, o: &[(String, W)], ind: usize, item: bool) -> bool {
        if self.o.object_limit == 0 || ind < 1 || o.len() < if item { 2 } else { 1 } {
            return false;
        }
        if !o.iter().all(|(k, v)| !is_container(v) && bare_key_ok(k)) {
            return false;
        }
        clen(&self.obj_cells(o)) <= self.o.object_limit
    }

    fn table_cols(l: &[W]) -> Option<Vec<String>> {
        if l.len() < 2 {
            return None;
        }
        let first = match &l[0] {
            W::Obj(o) if !o.is_empty() => o,
            _ => return None,
        };
        let keys: Vec<&String> = first.iter().map(|(k, _)| k).collect();
        if !keys.iter().all(|k| bare_key_ok(k)) {
            return None;
        }
        for x in l {
            match x {
                W::Obj(o) => {
                    if o.len() != keys.len() || o.iter().zip(&keys).any(|((k, _), kk)| k != *kk) {
                        return None;
                    }
                    if o.iter().any(|(_, v)| is_container(v)) {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        Some(keys.into_iter().cloned().collect())
    }

    fn emit(&self, o: &[(String, W)], ind: usize, out: &mut Vec<String>) {
        let p = " ".repeat(ind);
        for (k, v) in o {
            let kk = quote_key(k);
            match v {
                W::Obj(m) => {
                    if m.is_empty() {
                        out.push(format!("{p}{kk} {{}}"));
                    } else if self.inline_obj_ok(m, ind, false) {
                        out.push(format!("{p}{kk}{{}} {}", self.obj_cells(m)));
                    } else {
                        out.push(format!("{p}{kk}"));
                        self.emit(m, ind + 1, out);
                    }
                }
                W::Arr(l) => {
                    if l.is_empty() {
                        out.push(format!("{p}{kk} []"));
                    } else if self.inline_ok(l) {
                        out.push(format!("{p}{kk}[] {}", self.cells(l.iter())));
                    } else if let Some(cols) = Self::table_cols(l) {
                        out.push(format!("{p}{kk}[{}]", cols.join("|")));
                        for x in l {
                            if let W::Obj(row) = x {
                                out.push(format!("{p} {}", self.cells(row.iter().map(|(_, v)| v))));
                            }
                        }
                    } else {
                        out.push(format!("{p}{kk}"));
                        self.emit_items(l, ind, out);
                    }
                }
                _ => match self.scalar(v, false) {
                    Some(s) => out.push(format!("{p}{kk} {s}")),
                    None => {
                        out.push(format!("{p}{kk} |"));
                        block_lines(str_of(v), ind + 1, out);
                    }
                },
            }
        }
    }

    fn emit_items(&self, l: &[W], ind: usize, out: &mut Vec<String>) {
        let p = " ".repeat(ind);
        for x in l {
            match x {
                W::Obj(o) => {
                    if o.is_empty() {
                        out.push(format!("{p}- {{}}"));
                    } else if self.inline_obj_ok(o, ind, true) {
                        out.push(format!("{p}-{{}} {}", self.obj_cells(o)));
                    } else {
                        let mut sub = Vec::new();
                        self.emit(o, ind + 1, &mut sub);
                        out.push(format!("{p}+ {}", &sub[0][ind + 1..]));
                        out.extend(sub.into_iter().skip(1));
                    }
                }
                W::Arr(a) => {
                    if a.is_empty() {
                        out.push(format!("{p}- []"));
                    } else if self.inline_ok(a) {
                        out.push(format!("{p}-[] {}", self.cells(a.iter())));
                    } else {
                        out.push(format!("{p}-"));
                        self.emit_items(a, ind + 1, out);
                    }
                }
                _ => match self.scalar(x, false) {
                    Some(s) => out.push(format!("{p}- {s}")),
                    None => {
                        out.push(format!("{p}- |"));
                        block_lines(str_of(x), ind + 1, out);
                    }
                },
            }
        }
    }

    fn body(&self, doc: &W, out: &mut Vec<String>) {
        match doc {
            W::Obj(o) => self.emit(o, 0, out),
            W::Arr(a) if a.is_empty() => out.push("[]".into()),
            W::Arr(a) if self.inline_ok(a) => out.push(format!("[] {}", self.cells(a.iter()))),
            W::Arr(a) => {
                if let Some(cols) = Self::table_cols(a) {
                    out.push(format!("[{}]", cols.join("|")));
                    for x in a {
                        if let W::Obj(row) = x {
                            out.push(format!(" {}", self.cells(row.iter().map(|(_, v)| v))));
                        }
                    }
                } else {
                    self.emit_items(a, 0, out);
                }
            }
            _ => unreachable!(),
        }
    }

    fn data_lines(&self, data: &[(Vec<String>, W)], out: &mut Vec<String>) {
        for (name, v) in data {
            let n = format!("@{}", name.join("."));
            match v {
                W::Arr(l) => {
                    out.push(n);
                    self.emit_items(l, 1, out);
                }
                _ => match self.scalar(v, false) {
                    Some(s) => out.push(format!("{n} {s}")),
                    None => {
                        out.push(format!("{n} |"));
                        block_lines(str_of(v), 1, out);
                    }
                },
            }
        }
    }

    fn split(&self, doc: W) -> (W, Vec<(Vec<String>, W)>) {
        let lim = self.o.split_limit;
        let long = |w: &W| matches!(w, W::Str(s, _) if clen(s) > lim);
        fn mv(w: W, key: Option<&str>, root: bool, long: &dyn Fn(&W) -> bool) -> W {
            match w {
                W::Obj(o) => W::Obj(
                    o.into_iter()
                        .map(|(k, v)| {
                            let nv = mv(v, Some(&k), false, long);
                            (k, nv)
                        })
                        .collect(),
                ),
                W::Arr(a) => {
                    if !root
                        && !a.is_empty()
                        && a.iter().all(|x| !is_container(x))
                        && a.iter().any(long)
                    {
                        W::Ref(None, Box::new(W::Arr(a)))
                    } else {
                        W::Arr(a.into_iter().map(|x| mv(x, None, false, long)).collect())
                    }
                }
                w if long(&w) && !key.is_some_and(|k| HEADLINE.contains(&k)) => {
                    W::Ref(None, Box::new(w))
                }
                w => w,
            }
        }
        let mut head = mv(doc, None, true, &long);
        let mut refs: Vec<(Vec<usize>, Vec<String>)> = Vec::new();
        collect_refs(&head, &[], &[], &mut vec![], &mut refs);
        let mut derived: HashMap<Vec<String>, usize> = HashMap::new();
        for (_, n) in &refs {
            *derived.entry(n.clone()).or_default() += 1;
        }
        let mut used: HashSet<Vec<String>> = HashSet::new();
        let mut data = Vec::new();
        for (loc, n) in refs {
            let node = at_mut(&mut head, &loc);
            let value = match node {
                W::Ref(_, v) => (**v).clone(),
                _ => unreachable!(),
            };
            if !n.iter().all(|s| seg_ok(s)) {
                *node = value;
                continue;
            }
            let name = if !used.contains(&n) && derived[&n] == 1 {
                *node = W::Ref(None, Box::new(value.clone()));
                n
            } else {
                let mut i = 2;
                let name = loop {
                    let mut c = n.clone();
                    let last = c.pop().unwrap();
                    c.push(format!("{last}~{i}"));
                    if !derived.contains_key(&c) && !used.contains(&c) {
                        break c;
                    }
                    i += 1;
                };
                *node = W::Ref(Some(name.clone()), Box::new(value.clone()));
                name
            };
            used.insert(name.clone());
            data.push((name, value));
        }
        (head, data)
    }
}

fn str_of(w: &W) -> &str {
    match w {
        W::Str(s, _) => s,
        _ => unreachable!(),
    }
}

fn block_lines(s: &str, ind: usize, out: &mut Vec<String>) {
    let p = " ".repeat(ind);
    for l in s.split('\n') {
        out.push(if l.is_empty() {
            String::new()
        } else {
            format!("{p}{l}")
        });
    }
}

fn w_id_segment(w: &W) -> Option<String> {
    let s = match w {
        W::Str(s, _) => s.clone(),
        W::Num(n) if n.is_integer_text() => n.as_str().to_string(),
        _ => return None,
    };
    if seg_ok(&s) {
        Some(s)
    } else {
        None
    }
}

fn collect_refs(
    w: &W,
    owner: &[String],
    path: &[String],
    loc: &mut Vec<usize>,
    out: &mut Vec<(Vec<usize>, Vec<String>)>,
) {
    match w {
        W::Obj(o) => {
            let (mut ow, mut pa) = (owner.to_vec(), path.to_vec());
            if let Some((_, idv)) = o.iter().find(|(k, _)| k == "id") {
                if let Some(s) = w_id_segment(idv) {
                    ow = vec![s];
                    pa = vec![];
                }
            }
            for (i, (k, v)) in o.iter().enumerate() {
                let mut sub = pa.clone();
                sub.push(k.clone());
                loc.push(i);
                if matches!(v, W::Ref(..)) {
                    out.push((loc.clone(), ow.iter().chain(sub.iter()).cloned().collect()));
                }
                collect_refs(v, &ow, &sub, loc, out);
                loc.pop();
            }
        }
        W::Arr(a) => {
            for (i, v) in a.iter().enumerate() {
                let mut sub = path.to_vec();
                sub.push(i.to_string());
                loc.push(i);
                if matches!(v, W::Ref(..)) {
                    out.push((
                        loc.clone(),
                        owner.iter().chain(sub.iter()).cloned().collect(),
                    ));
                }
                collect_refs(v, owner, &sub, loc, out);
                loc.pop();
            }
        }
        _ => {}
    }
}

fn at_mut<'w>(w: &'w mut W, loc: &[usize]) -> &'w mut W {
    let mut cur = w;
    for &i in loc {
        cur = match cur {
            W::Obj(o) => &mut o[i].1,
            W::Arr(a) => &mut a[i],
            _ => unreachable!(),
        };
    }
    cur
}

/// The header line for these options.
pub fn header_line(schema: Option<&str>, url: Option<&str>) -> Result<String, Error> {
    let mut h = String::from("LIN 1");
    match (schema, url) {
        (Some(s), u) => {
            let kind = crate::jcs::id_kind(s).ok_or_else(|| {
                Error::new("a schema id is a registered number (1 to 9 digits) or a 39-digit provisional id")
            })?;
            if u.is_some() && kind != crate::jcs::IdKind::Provisional {
                return Err(Error::new("a url may follow only a provisional schema id"));
            }
            h.push(' ');
            h.push_str(s);
            if let Some(u) = u {
                if !u.starts_with("https://") || u.contains(' ') {
                    return Err(Error::new(
                        "schema url must start with https:// and have no spaces",
                    ));
                }
                h.push(' ');
                h.push_str(u);
            }
        }
        (None, Some(_)) => return Err(Error::new("a url needs a schema id before it")),
        (None, None) => {}
    }
    Ok(h)
}

/// Write a value (object or array) as LIN. `schema_doc` is the schema itself: with it, strings
/// that look like numbers or booleans go bare where the schema says they are strings.
pub fn to_string_with(
    value: &Value,
    opts: &WriteOptions,
    schema_doc: Option<&Value>,
) -> Result<String, Error> {
    if !value.is_container() {
        return Err(Error::new("the root must be an object or an array"));
    }
    let typer = Typer::new(schema_doc);
    let mut doc = from_value(value, &typer, &typer.root());
    let w = Writer { o: opts.clone() };
    let mut data = Vec::new();
    if opts.split {
        let (h, d) = w.split(doc);
        doc = h;
        data = d;
    }
    let mut lines = Vec::new();
    if opts.header {
        lines.push(header_line(opts.schema.as_deref(), opts.url.as_deref())?);
    }
    w.body(&doc, &mut lines);
    if !data.is_empty() {
        lines.push("---".into());
        w.data_lines(&data, &mut lines);
    }
    let mut s = lines.join("\n");
    s.push('\n');
    Ok(s)
}

/// A scalar in its LIN line form (`~`, `true`, `12`, bare or quoted text).
pub fn scalar_text(v: &Value) -> String {
    match v {
        Value::Null => "~".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.as_str().to_string(),
        Value::String(s) => string_form(s, true, false).unwrap(),
        Value::Array(a) if a.is_empty() => "[]".into(),
        Value::Object(o) if o.is_empty() => "{}".into(),
        _ => json_text(v),
    }
}

fn json_text(v: &Value) -> String {
    crate::json::to_string(v)
}
