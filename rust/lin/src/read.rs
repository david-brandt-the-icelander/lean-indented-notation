//! The LIN reader.

use std::collections::{HashMap, HashSet};

use crate::json::decode_string;
use crate::schema::{bare_reads_as_string, Pos, Typer};
use crate::text::{bad_char, bare_key_ok, id_segment_of, key_len_at, seg_ok, SEG_STOP};
use crate::value::{is_json_number, Map, Number, Value};
use crate::{Error, Header};

/// A parsed but not yet typed value: bare number and boolean text keeps its text until the
/// schema says what it is, and `@` stays a reference until the data section fills it in.
#[derive(Clone, Debug)]
pub enum Raw {
    Null,
    Bare(String),
    Str(String),
    Arr(Vec<Raw>),
    Obj(Vec<(String, Raw)>),
    Ref(Option<Vec<String>>),
}

/// The result of [`parse`]: the header, the head, and the data entries by name.
#[derive(Clone, Debug)]
pub struct Parsed {
    pub header: Header,
    pub head: Raw,
    pub data: Vec<(Vec<String>, Raw)>,
}

fn err(msg: impl Into<String>, line: usize) -> Error {
    Error::at(msg, line)
}

pub fn read_header(line: &str, no: usize) -> Result<Header, Error> {
    let f: Vec<&str> = line.split(' ').collect();
    if f.len() < 2 || f[0] != "LIN" {
        return Err(err(
            format!("the first line must be the header \"LIN 1 [schema [url]]\", found {line:?}"),
            no,
        ));
    }
    if f[1] != "1" {
        return Err(err(format!("unsupported LIN version {:?}", f[1]), no));
    }
    if f.len() > 4 {
        return Err(err("too many fields in the header", no));
    }
    if f.len() >= 3 && !crate::jcs::is_hash(f[2]) {
        return Err(err(
            "the schema must be sha256- followed by 43 base64url characters",
            no,
        ));
    }
    if f.len() == 4 && (!f[3].starts_with("https://") || f[3].len() <= 8) {
        return Err(err("the schema url must start with https://", no));
    }
    Ok(Header {
        version: 1,
        schema: f.get(2).map(|s| s.to_string()),
        url: f.get(3).map(|s| s.to_string()),
    })
}

fn json_str_at(s: &str, i: usize, line: usize) -> Result<(String, usize), Error> {
    decode_string(s, i).map_err(|m| err(format!("bad JSON string: {m}"), line))
}

fn bare_value(s: &str, line: usize, cell: bool) -> Result<Raw, Error> {
    if s.is_empty() {
        return Err(err(
            if cell {
                "empty cell; an empty string is written \"\""
            } else {
                "empty value; an empty string is written \"\""
            },
            line,
        ));
    }
    if s.starts_with(' ') || s.ends_with(' ') {
        return Err(err(format!("value {s:?} starts or ends with a space; such a string is written as a JSON string"), line));
    }
    if bad_char(s) {
        return Err(err(
            "a control character in a bare value; write it as a JSON string",
            line,
        ));
    }
    Ok(match s {
        "~" => Raw::Null,
        "[]" => Raw::Arr(vec![]),
        "{}" => Raw::Obj(vec![]),
        "true" | "false" => Raw::Bare(s.to_string()),
        _ if is_json_number(s) => Raw::Bare(s.to_string()),
        _ if s.starts_with('@') => Raw::Ref(if s.len() > 1 {
            Some(parse_name(&s[1..], line)?)
        } else {
            None
        }),
        _ => Raw::Str(s.to_string()),
    })
}

fn parse_scalar(s: &str, line: usize) -> Result<Raw, Error> {
    if s.starts_with('"') {
        let (v, end) = json_str_at(s, 0, line)?;
        if end != s.len() {
            return Err(err(
                format!("text after a JSON string: {:?}", &s[end..]),
                line,
            ));
        }
        return Ok(Raw::Str(v));
    }
    bare_value(s, line, false)
}

/// Parse a reference name (`owner.path.0.key`).
pub fn parse_name(s: &str, line: usize) -> Result<Vec<String>, Error> {
    let mut segs = Vec::new();
    for part in s.split('.') {
        if part.is_empty() || part.chars().any(SEG_STOP) {
            return Err(err(format!("bad reference name {s:?}"), line));
        }
        segs.push(part.to_string());
    }
    Ok(segs)
}

fn split_cells(s: &str, line: usize) -> Result<Vec<Raw>, Error> {
    let mut cells = Vec::new();
    let mut i = 0;
    loop {
        let j;
        if s[i..].starts_with('"') {
            let (v, e) = json_str_at(s, i, line)?;
            cells.push(Raw::Str(v));
            j = e;
        } else {
            j = s[i..].find('|').map_or(s.len(), |p| i + p);
            cells.push(bare_value(&s[i..j], line, true)?);
        }
        if j == s.len() {
            return Ok(cells);
        }
        if !s[j..].starts_with('|') {
            return Err(err(
                format!("expected | after a cell, found {:?}", &s[j..]),
                line,
            ));
        }
        i = j + 1;
    }
}

fn obj_from_cells(s: &str, line: usize) -> Result<Raw, Error> {
    let mut o: Vec<(String, Raw)> = Vec::new();
    let mut keys = HashSet::new();
    let mut i = 0;
    loop {
        let k;
        if s[i..].starts_with('"') {
            let (kk, e) = json_str_at(s, i, line)?;
            k = kk;
            i = e;
        } else {
            let n = key_len_at(&s[i..]);
            if n == 0 {
                return Err(err(
                    format!("bad key in inline object: {:?}", &s[i..]),
                    line,
                ));
            }
            k = s[i..i + n].to_string();
            i += n;
        }
        if !s[i..].starts_with(' ') {
            return Err(err(
                format!("inline object field {k:?} needs one space and a value"),
                line,
            ));
        }
        i += 1;
        let (v, j) = if s[i..].starts_with('"') {
            let (v, e) = json_str_at(s, i, line)?;
            (Raw::Str(v), e)
        } else {
            let j = s[i..].find('|').map_or(s.len(), |p| i + p);
            (bare_value(&s[i..j], line, true)?, j)
        };
        if !keys.insert(k.clone()) {
            return Err(err(format!("duplicate key {k:?}"), line));
        }
        o.push((k, v));
        if j == s.len() {
            return Ok(Raw::Obj(o));
        }
        if !s[j..].starts_with('|') {
            return Err(err(
                format!("expected | after a field, found {:?}", &s[j..]),
                line,
            ));
        }
        i = j + 1;
    }
}

fn columns(spec: &str, line: usize) -> Result<Vec<String>, Error> {
    let cols: Vec<String> = spec.split('|').map(str::to_string).collect();
    for c in &cols {
        if !bare_key_ok(c) {
            return Err(err(
                format!("bad column name {c:?}: columns are bare keys"),
                line,
            ));
        }
    }
    let uniq: HashSet<&String> = cols.iter().collect();
    if uniq.len() != cols.len() {
        return Err(err("duplicate column name", line));
    }
    Ok(cols)
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

struct Reader<'t> {
    l: Vec<&'t str>,
    i: usize,
    base: usize,
}

impl<'t> Reader<'t> {
    fn no(&self) -> usize {
        self.base + self.i
    }

    fn peek(&mut self) -> Option<&'t str> {
        while self.i < self.l.len() && self.l[self.i].trim_matches(' ').is_empty() {
            self.i += 1;
        }
        self.l.get(self.i).copied()
    }

    fn block(&mut self, ind: usize) -> Result<String, Error> {
        let p = " ".repeat(ind);
        let mut out: Vec<&str> = Vec::new();
        while self.i < self.l.len() {
            let line = self.l[self.i];
            if line.is_empty() {
                out.push("");
            } else if let Some(rest) = line.strip_prefix(p.as_str()) {
                out.push(rest);
            } else {
                break;
            }
            self.i += 1;
        }
        while out.last() == Some(&"") {
            out.pop();
            self.i -= 1;
        }
        if out.is_empty() {
            return Err(err("empty multi-line value", self.no()));
        }
        Ok(out.join("\n"))
    }

    fn value_after(&mut self, rest: &str, ind: usize, line: usize) -> Result<Raw, Error> {
        if rest == "|" {
            return Ok(Raw::Str(self.block(ind + 1)?));
        }
        parse_scalar(rest, line)
    }

    fn table_rows(&mut self, cols: &[String], ind: usize, line: usize) -> Result<Raw, Error> {
        let mut rows = Vec::new();
        while let Some(nxt) = self.peek() {
            if indent(nxt) != ind {
                break;
            }
            let cells = split_cells(&nxt[ind..], self.no())?;
            if cells.len() != cols.len() {
                return Err(err(
                    format!(
                        "row has {} cells, the header has {} columns",
                        cells.len(),
                        cols.len()
                    ),
                    self.no(),
                ));
            }
            self.i += 1;
            rows.push(Raw::Obj(cols.iter().cloned().zip(cells).collect()));
        }
        if rows.is_empty() {
            return Err(err("table with no rows", line));
        }
        Ok(Raw::Arr(rows))
    }

    fn obj(&mut self, ind: usize, mut first: Option<(&'t str, usize)>) -> Result<Raw, Error> {
        let mut o: Vec<(String, Raw)> = Vec::new();
        let mut keys = HashSet::new();
        loop {
            let (line, no) = match first.take() {
                Some(f) => f,
                None => {
                    let raw = match self.peek() {
                        None => return Ok(Raw::Obj(o)),
                        Some(r) => r,
                    };
                    let n = indent(raw);
                    if n != ind {
                        if n > ind {
                            return Err(err("unexpected indentation", self.no()));
                        }
                        return Ok(Raw::Obj(o));
                    }
                    let no = self.no();
                    self.i += 1;
                    (&raw[ind..], no)
                }
            };
            let (k, rest): (String, &str) = if line.starts_with('"') {
                let (k, j) = json_str_at(line, 0, no)?;
                (k, &line[j..])
            } else {
                let n = key_len_at(line);
                if n == 0 {
                    return Err(err(format!("expected a key, found {line:?}"), no));
                }
                let k = &line[..n];
                if bad_char(k) {
                    return Err(err("a control character in a bare key", no));
                }
                (k.to_string(), &line[n..])
            };
            if keys.contains(&k) {
                return Err(err(format!("duplicate key {k:?}"), no));
            }
            let v = if rest.is_empty() {
                let nxt = self.peek();
                match nxt {
                    Some(n)
                        if indent(n) == ind
                            && (n[ind..].starts_with('-') || n[ind..].starts_with('+')) =>
                    {
                        self.items(ind)?
                    }
                    Some(n) if indent(n) == ind + 1 => self.obj(ind + 1, None)?,
                    _ => return Err(err(format!("key {k:?} has no value and no children"), no)),
                }
            } else if let Some(r) = rest.strip_prefix("[] ") {
                Raw::Arr(split_cells(r, no)?)
            } else if let Some(r) = rest.strip_prefix("{} ") {
                obj_from_cells(r, no)?
            } else if rest.starts_with('[') && rest.ends_with(']') && rest.len() > 2 {
                let cols = columns(&rest[1..rest.len() - 1], no)?;
                self.table_rows(&cols, ind + 1, no)?
            } else if let Some(r) = rest.strip_prefix(' ') {
                self.value_after(r, ind, no)?
            } else {
                return Err(err(
                    format!("expected a space, [] , {{}} or a table header after key {k:?}"),
                    no,
                ));
            };
            keys.insert(k.clone());
            o.push((k, v));
        }
    }

    fn items(&mut self, ind: usize) -> Result<Raw, Error> {
        let mut out = Vec::new();
        loop {
            let raw = match self.peek() {
                Some(r) if indent(r) == ind => r,
                _ => return Ok(Raw::Arr(out)),
            };
            let body = &raw[ind..];
            let no = self.no();
            if let Some(r) = body.strip_prefix("-[] ") {
                self.i += 1;
                out.push(Raw::Arr(split_cells(r, no)?));
            } else if let Some(r) = body.strip_prefix("-{} ") {
                self.i += 1;
                out.push(obj_from_cells(r, no)?);
            } else if body == "-" {
                self.i += 1;
                let inner = self.items(ind + 1)?;
                if matches!(&inner, Raw::Arr(a) if a.is_empty()) {
                    return Err(err("a nested list marker with no items", no));
                }
                out.push(inner);
            } else if let Some(r) = body.strip_prefix("- ") {
                self.i += 1;
                out.push(self.value_after(r, ind, no)?);
            } else if let Some(r) = body.strip_prefix("+ ") {
                self.i += 1;
                out.push(self.obj(ind + 1, Some((r, no)))?);
            } else if body.starts_with('-') || body.starts_with('+') {
                return Err(err(format!("bad list item {body:?}"), no));
            } else {
                return Ok(Raw::Arr(out));
            }
        }
    }
}

/// Parse LIN text without typing or resolving references.
pub fn parse(text: &str) -> Result<Parsed, Error> {
    if text.starts_with('\u{feff}') {
        return Err(err("a LIN file must not start with a byte-order mark", 1));
    }
    if let Some(p) = text.find('\r') {
        return Err(err(
            "LIN uses LF line ends, not CR LF",
            text[..p].matches('\n').count() + 1,
        ));
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    if lines.is_empty() {
        return Err(Error::new("empty file"));
    }
    let header = read_header(lines[0], 1)?;
    let sep = lines
        .iter()
        .position(|l| *l == "---")
        .unwrap_or(lines.len());
    let mut head = Reader {
        l: lines[1..sep].to_vec(),
        i: 0,
        base: 2,
    };
    let doc = match head.peek() {
        None => Raw::Obj(vec![]),
        Some("[]") => {
            head.i += 1;
            Raw::Arr(vec![])
        }
        Some(f) if f.starts_with("[] ") => {
            let no = head.no();
            head.i += 1;
            Raw::Arr(split_cells(&f[3..], no)?)
        }
        Some(f) if f.starts_with('[') => {
            let no = head.no();
            if !f.ends_with(']') || f.len() < 3 {
                return Err(err(format!("bad root table header {f:?}"), no));
            }
            let cols = columns(&f[1..f.len() - 1], no)?;
            head.i += 1;
            head.table_rows(&cols, 1, no)?
        }
        Some(f) if (f.starts_with('-') || f.starts_with('+')) => head.items(0)?,
        Some(f) => {
            if indent(f) > 0 {
                return Err(err("unexpected indentation", head.no()));
            }
            head.obj(0, None)?
        }
    };
    if let Some(l) = head.peek() {
        return Err(err(format!("unexpected line {l:?}"), head.no()));
    }
    let mut data: Vec<(Vec<String>, Raw)> = Vec::new();
    let mut names = HashSet::new();
    let mut r = Reader {
        l: if sep < lines.len() {
            lines[sep + 1..].to_vec()
        } else {
            vec![]
        },
        i: 0,
        base: sep + 2,
    };
    while let Some(line) = r.peek() {
        let no = r.no();
        if !line.starts_with('@') {
            return Err(err(
                format!("a data line must start with @name, found {line:?}"),
                no,
            ));
        }
        r.i += 1;
        let sp = line.find(' ');
        let name = parse_name(&line[1..sp.unwrap_or(line.len())], no)?;
        if !names.insert(name.clone()) {
            return Err(err(format!("duplicate data entry @{}", name.join(".")), no));
        }
        let v = match sp {
            None => {
                let items = r.items(1)?;
                if matches!(&items, Raw::Arr(a) if a.is_empty()) {
                    return Err(err(
                        format!("data entry @{} has no value", name.join(".")),
                        no,
                    ));
                }
                items
            }
            Some(sp) => r.value_after(&line[sp + 1..], 0, no)?,
        };
        data.push((name, v));
    }
    Ok(Parsed {
        header,
        head: doc,
        data,
    })
}

/// Type the head by the schema and put each data entry in place of its `@`.
pub fn resolve(parsed: Parsed, schema: Option<&Value>) -> Result<Value, Error> {
    let typer = Typer::new(schema);
    let mut data: HashMap<Vec<String>, Raw> = parsed.data.into_iter().collect();
    let total = data.len();
    let mut cx = Cx {
        typer: &typer,
        data: &mut data,
        used: HashSet::new(),
    };
    let v = cx.walk(parsed.head, &typer.root(), &[], &[])?;
    if cx.used.len() != total {
        let mut extra: Vec<String> = cx
            .data
            .keys()
            .filter(|k| !cx.used.contains(*k))
            .map(|k| format!("@{}", k.join(".")))
            .collect();
        extra.sort();
        extra.truncate(3);
        return Err(Error::new(format!(
            "data entries with no @ in the head: {}",
            extra.join(", ")
        )));
    }
    Ok(v)
}

struct Cx<'a, 'b> {
    typer: &'b Typer<'a>,
    data: &'b mut HashMap<Vec<String>, Raw>,
    used: HashSet<Vec<String>>,
}

impl<'a, 'b> Cx<'a, 'b> {
    fn scalar(&self, r: Raw, pos: &Pos<'a>) -> Result<Value, Error> {
        Ok(match r {
            Raw::Null => Value::Null,
            Raw::Str(s) => Value::String(s),
            Raw::Bare(s) => {
                if bare_reads_as_string(&s, self.typer.types(pos)) {
                    Value::String(s)
                } else if s == "true" || s == "false" {
                    Value::Bool(s == "true")
                } else {
                    Value::Number(Number::from_json_text(&s)?)
                }
            }
            _ => unreachable!(),
        })
    }

    /// Type a data entry's value (it holds no references).
    fn plain(&self, r: Raw, pos: &Pos<'a>) -> Result<Value, Error> {
        match r {
            Raw::Arr(a) => Ok(Value::Array(
                a.into_iter()
                    .enumerate()
                    .map(|(i, x)| self.plain(x, &self.typer.item(pos, i)))
                    .collect::<Result<_, _>>()?,
            )),
            Raw::Obj(o) => {
                let mut m = Map::new();
                for (k, x) in o {
                    let p = self.typer.prop(pos, &k);
                    m.insert(k, self.plain(x, &p)?);
                }
                Ok(Value::Object(m))
            }
            Raw::Ref(_) => Err(Error::new("a data entry cannot hold a reference")),
            other => self.scalar(other, pos),
        }
    }

    fn walk(
        &mut self,
        r: Raw,
        pos: &Pos<'a>,
        owner: &[String],
        path: &[String],
    ) -> Result<Value, Error> {
        match r {
            Raw::Obj(o) => {
                // the owner comes from the object's own id, typed first
                let mut owner_v = owner.to_vec();
                let mut path_v = path.to_vec();
                if let Some((_, idr)) = o.iter().find(|(k, _)| k == "id") {
                    let seg = match idr {
                        Raw::Ref(_) | Raw::Arr(_) | Raw::Obj(_) => None,
                        other => {
                            id_segment_of(&self.scalar(other.clone(), &self.typer.prop(pos, "id"))?)
                        }
                    };
                    if let Some(s) = seg {
                        owner_v = vec![s];
                        path_v = vec![];
                    }
                }
                let mut m = Map::new();
                for (k, x) in o {
                    let p = self.typer.prop(pos, &k);
                    let mut sub = path_v.clone();
                    sub.push(k.clone());
                    let v = self.walk(x, &p, &owner_v, &sub)?;
                    m.insert(k, v);
                }
                Ok(Value::Object(m))
            }
            Raw::Arr(a) => {
                let mut out = Vec::with_capacity(a.len());
                for (i, x) in a.into_iter().enumerate() {
                    let p = self.typer.item(pos, i);
                    let mut sub = path.to_vec();
                    sub.push(i.to_string());
                    out.push(self.walk(x, &p, owner, &sub)?);
                }
                Ok(Value::Array(out))
            }
            Raw::Ref(name) => {
                let name =
                    name.unwrap_or_else(|| owner.iter().chain(path.iter()).cloned().collect());
                if self.used.contains(&name) {
                    return Err(Error::new(format!(
                        "data entry @{} is referenced twice",
                        name.join(".")
                    )));
                }
                let raw =
                    self.data.get(&name).cloned().ok_or_else(|| {
                        Error::new(format!("no data entry for @{}", name.join(".")))
                    })?;
                self.used.insert(name);
                self.plain(raw, pos)
            }
            other => self.scalar(other, pos),
        }
    }
}

/// Is every name segment usable (no dots, whitespace, `"`, `|` or `@`)?
pub fn names_ok(n: &[String]) -> bool {
    n.iter().all(|s| seg_ok(s))
}
