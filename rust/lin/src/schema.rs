//! How a schema types bare scalars (spec section 7).
//!
//! Only the parts of JSON Schema that say which types a position allows are used: `type`,
//! `const`, `enum`, `allOf`, `anyOf`, `oneOf`, local `$ref`, `properties`, `patternProperties`,
//! `additionalProperties`, `prefixItems` and `items`. Anything else allows every type, which is
//! the schema-less rule. The Python and TypeScript libraries implement the same rules.

use regex_lite::Regex;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::value::Value;

/// A set of JSON types (`integer` counts as `number`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Types(u8);

impl Types {
    pub const NULL: u8 = 1;
    pub const BOOLEAN: u8 = 2;
    pub const NUMBER: u8 = 4;
    pub const STRING: u8 = 8;
    pub const ARRAY: u8 = 16;
    pub const OBJECT: u8 = 32;
    pub const ALL: Types = Types(63);
    pub const NONE: Types = Types(0);

    pub fn has(self, bit: u8) -> bool {
        self.0 & bit != 0
    }
    fn and(self, o: Types) -> Types {
        Types(self.0 & o.0)
    }
    fn or(self, o: Types) -> Types {
        Types(self.0 | o.0)
    }
    fn of_name(name: &str) -> u8 {
        match name {
            "null" => Self::NULL,
            "boolean" => Self::BOOLEAN,
            "number" | "integer" => Self::NUMBER,
            "string" => Self::STRING,
            "array" => Self::ARRAY,
            "object" => Self::OBJECT,
            _ => 0,
        }
    }
    fn of_value(v: &Value) -> u8 {
        Self::of_name(v.type_name())
    }
}

/// Would bare text `s`, at a position allowing `t`, read back as the string `s`?
pub fn bare_reads_as_string(s: &str, t: Types) -> bool {
    if crate::value::is_json_number(s) {
        return !t.has(Types::NUMBER) && t.has(Types::STRING);
    }
    if s == "true" || s == "false" {
        return !t.has(Types::BOOLEAN) && t.has(Types::STRING);
    }
    true
}

/// A position in a schema: one schema node, or a combination of them.
#[derive(Clone, Debug)]
pub enum Pos<'a> {
    /// Allows everything.
    True,
    Node(&'a Value),
    /// The node with its `$ref` already followed (its own other keywords still apply).
    NoRef(&'a Value),
    All(Vec<Pos<'a>>),
    Any(Vec<Pos<'a>>),
}

/// Typing context for one schema document. Without a schema, every position allows all types.
pub struct Typer<'a> {
    root: Option<&'a Value>,
    regexes: Mutex<HashMap<String, Option<Regex>>>,
}

impl<'a> Typer<'a> {
    pub fn new(schema: Option<&'a Value>) -> Typer<'a> {
        Typer {
            root: schema,
            regexes: Mutex::new(HashMap::new()),
        }
    }

    pub fn root(&self) -> Pos<'a> {
        match self.root {
            Some(r) => Pos::Node(r),
            None => Pos::True,
        }
    }

    pub fn has_schema(&self) -> bool {
        self.root.is_some()
    }

    fn pointer(&self, ptr: &str) -> Option<&'a Value> {
        let mut cur = self.root?;
        if ptr.is_empty() {
            return Some(cur);
        }
        let rest = ptr.strip_prefix('/')?;
        for part in rest.split('/') {
            let part = part.replace("~1", "/").replace("~0", "~");
            cur = match cur {
                Value::Object(m) => m.get(&part)?,
                Value::Array(a) => {
                    if !part.bytes().all(|b| b.is_ascii_digit()) || part.is_empty() {
                        return None;
                    }
                    a.get(part.parse::<usize>().ok()?)?
                }
                _ => return None,
            };
        }
        Some(cur)
    }

    /// Follow `$ref` the way the Python reference does.
    fn deref(&self, p: Pos<'a>) -> Pos<'a> {
        let mut node = match p {
            Pos::Node(v) => v,
            other => return other,
        };
        for _ in 0..64 {
            let m = match node {
                Value::Object(m) => m,
                _ => return Pos::Node(node),
            };
            let target = match m.get("$ref") {
                Some(Value::String(r)) if r.starts_with('#') => self.pointer(&r[1..]),
                _ => return Pos::Node(node),
            };
            let target = match target {
                Some(t) => t,
                None => return Pos::Node(node),
            };
            if m.len() > 1 {
                return Pos::All(vec![Pos::Node(target), Pos::NoRef(node)]);
            }
            node = target;
        }
        Pos::Node(node)
    }

    pub fn types(&self, p: &Pos<'a>) -> Types {
        match self.deref(p.clone()) {
            Pos::True => Types::ALL,
            Pos::All(ps) => ps.iter().fold(Types::ALL, |t, x| t.and(self.types(x))),
            Pos::Any(ps) => {
                if ps.is_empty() {
                    Types::ALL
                } else {
                    ps.iter().fold(Types::NONE, |t, x| t.or(self.types(x)))
                }
            }
            Pos::Node(v) | Pos::NoRef(v) => self.own_types(v),
        }
    }

    fn own_types(&self, v: &'a Value) -> Types {
        let m = match v {
            Value::Object(m) => m,
            _ => return Types::ALL,
        };
        let mut out = Types::ALL;
        match m.get("type") {
            Some(Value::String(s)) => out = out.and(Types(Types::of_name(s))),
            Some(Value::Array(a)) => {
                let bits = a
                    .iter()
                    .filter_map(|x| x.as_str())
                    .fold(0, |b, s| b | Types::of_name(s));
                out = out.and(Types(bits));
            }
            _ => {}
        }
        if let Some(c) = m.get("const") {
            out = out.and(Types(Types::of_value(c)));
        }
        if let Some(Value::Array(e)) = m.get("enum") {
            out = out.and(Types(e.iter().fold(0, |b, x| b | Types::of_value(x))));
        }
        if let Some(Value::Array(subs)) = m.get("allOf") {
            for s in subs {
                out = out.and(self.types(&Pos::Node(s)));
            }
        }
        for key in ["anyOf", "oneOf"] {
            if let Some(Value::Array(subs)) = m.get(key) {
                if !subs.is_empty() {
                    let u = subs
                        .iter()
                        .fold(Types::NONE, |t, s| t.or(self.types(&Pos::Node(s))));
                    out = out.and(u);
                }
            }
        }
        out
    }

    fn children(
        &self,
        p: Pos<'a>,
        pick: &dyn Fn(&Typer<'a>, &'a crate::value::Map) -> Pos<'a>,
    ) -> Vec<Pos<'a>> {
        match self.deref(p) {
            Pos::True => vec![Pos::True],
            Pos::All(ps) => {
                let mut out = vec![Pos::True];
                for x in ps {
                    out.extend(self.children(x, pick));
                }
                out
            }
            Pos::Any(ps) => vec![
                Pos::True,
                Pos::Any(
                    ps.into_iter()
                        .map(|x| Pos::All(self.children(x, pick)))
                        .collect(),
                ),
            ],
            Pos::Node(v) | Pos::NoRef(v) => {
                let m = match v {
                    Value::Object(m) => m,
                    _ => return vec![Pos::True],
                };
                let mut out = vec![pick(self, m)];
                if let Some(Value::Array(subs)) = m.get("allOf") {
                    for s in subs {
                        out.extend(self.children(Pos::Node(s), pick));
                    }
                }
                for key in ["anyOf", "oneOf"] {
                    if let Some(Value::Array(subs)) = m.get(key) {
                        if !subs.is_empty() {
                            out.push(Pos::Any(
                                subs.iter()
                                    .map(|s| Pos::All(self.children(Pos::Node(s), pick)))
                                    .collect(),
                            ));
                        }
                    }
                }
                out
            }
        }
    }

    fn search(&self, pattern: &str, s: &str) -> bool {
        let mut cache = self.regexes.lock().unwrap();
        let re = cache
            .entry(pattern.to_string())
            .or_insert_with(|| Regex::new(pattern).ok());
        re.as_ref().is_some_and(|r| r.is_match(s))
    }

    /// The position of property `key` of an object at `p`.
    pub fn prop(&self, p: &Pos<'a>, key: &str) -> Pos<'a> {
        if !self.has_schema() {
            return Pos::True;
        }
        let pick = |t: &Typer<'a>, m: &'a crate::value::Map| -> Pos<'a> {
            if let Some(Value::Object(props)) = m.get("properties") {
                if let Some(s) = props.get(key) {
                    return Pos::Node(s);
                }
            }
            if let Some(Value::Object(pats)) = m.get("patternProperties") {
                let hits: Vec<Pos<'a>> = pats
                    .iter()
                    .filter(|(pat, _)| t.search(pat, key))
                    .map(|(_, s)| Pos::Node(s))
                    .collect();
                if !hits.is_empty() {
                    return Pos::All(hits);
                }
            }
            if let Some(s) = m.get("additionalProperties") {
                return Pos::Node(s);
            }
            Pos::True
        };
        Pos::All(self.children(p.clone(), &pick))
    }

    /// The position of item `index` of an array at `p`.
    pub fn item(&self, p: &Pos<'a>, index: usize) -> Pos<'a> {
        if !self.has_schema() {
            return Pos::True;
        }
        let pick = |_: &Typer<'a>, m: &'a crate::value::Map| -> Pos<'a> {
            if let Some(Value::Array(pre)) = m.get("prefixItems") {
                if let Some(s) = pre.get(index) {
                    return Pos::Node(s);
                }
            }
            match m.get("items") {
                Some(Value::Array(_)) | None => Pos::True,
                Some(s) => Pos::Node(s),
            }
        };
        Pos::All(self.children(p.clone(), &pick))
    }
}
