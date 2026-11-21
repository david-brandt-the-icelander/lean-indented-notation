//! Finding schemas by their id (spec section 7).
//!
//! A header names a schema by a registered number or a provisional id. A registered number is
//! looked up in the official registry's index, which gives its provisional id and full SHA-256.
//! The schema itself is then looked for, by provisional id, in: directories in `LIN_SCHEMA_PATH`,
//! a `schemas/` directory beside the file or above it, the user cache, the schemas built into this
//! tool, the header's URL, and each public registry in `registries.lin` (or `LIN_REGISTRIES`).
//! Whatever is found is checked against the provisional id, and against the SHA-256 when the
//! index records one.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use lin::{id_kind, schema_id, schema_sha256, IdKind, ReadOptions, Value};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

/// The LIN meta-schema's registered number.
pub const META: &str = "1";
/// The registered number of the schema of a registry's index.lin.
pub const INDEX_SCHEMA: &str = "3";

/// One row of a registry's index.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub number: Option<String>,
    pub provisional: String,
    pub sha256: String,
    pub id: Option<String>,
    pub title: Option<String>,
}

pub struct Registry {
    pub name: String,
    pub prefix: String,
    pub official: bool,
}

pub struct Store {
    pub offline: bool,
    path_dirs: Vec<PathBuf>,
    pub cache: Option<PathBuf>,
    loaded: RefCell<HashMap<String, Value>>,
    loading: RefCell<HashSet<String>>,
    fetched_index: RefCell<Option<Vec<Entry>>>,
}

pub fn cache_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("LIN_CACHE_DIR") {
        return Some(PathBuf::from(d));
    }
    if let Ok(d) = std::env::var("XDG_CACHE_HOME") {
        return Some(PathBuf::from(d).join("lin/schemas"));
    }
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".cache/lin/schemas"))
}

fn schemaless(text: &str) -> Option<Value> {
    lin::from_str_opts(
        text,
        ReadOptions {
            ignore_schema: true,
            ..Default::default()
        },
    )
    .ok()
}

/// The public registries: `LIN_REGISTRIES` (comma-separated prefixes; the first is official),
/// else the built-in registries.lin.
pub fn registries() -> Vec<Registry> {
    if let Ok(r) = std::env::var("LIN_REGISTRIES") {
        return r
            .split(',')
            .filter(|s| !s.is_empty())
            .enumerate()
            .map(|(i, s)| Registry {
                name: s.to_string(),
                prefix: s.to_string(),
                official: i == 0,
            })
            .collect();
    }
    let v = match schemaless(embedded::REGISTRIES) {
        Some(v) => v,
        None => return vec![],
    };
    v.get("registries")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    Some(Registry {
                        name: r.get("name")?.as_str()?.to_string(),
                        prefix: r.get("prefix")?.as_str()?.to_string(),
                        official: matches!(r.get("official"), Some(Value::Bool(true))),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn embedded_schemas() -> &'static [(&'static str, &'static str)] {
    embedded::SCHEMAS
}

fn text_of(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.as_str().to_string()),
        _ => None,
    }
}

/// The rows of an index.lin (read by the schema-less rules, so it needs no schema to bootstrap).
pub fn parse_index(text: &str) -> Vec<Entry> {
    let v = match schemaless(text) {
        Some(v) => v,
        None => return vec![],
    };
    v.get("schemas")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    let mut provisional = text_of(r.get("provisional"))?;
                    // read without its schema, a provisional id may come back as a number
                    while provisional.len() < 39 {
                        provisional.insert(0, '0');
                    }
                    Some(Entry {
                        number: text_of(r.get("number")),
                        provisional,
                        sha256: text_of(r.get("sha256"))?,
                        id: text_of(r.get("id")),
                        title: text_of(r.get("title")),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

impl Store {
    pub fn new(offline: bool) -> Store {
        let path_dirs = std::env::var("LIN_SCHEMA_PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        Store {
            offline,
            path_dirs,
            cache: cache_dir(),
            loaded: RefCell::new(HashMap::new()),
            loading: RefCell::new(HashSet::new()),
            fetched_index: RefCell::new(None),
        }
    }

    /// Directories searched for schemas and index.lin, nearest first.
    pub fn local_dirs(&self, near: Option<&Path>) -> Vec<PathBuf> {
        let mut out = self.path_dirs.clone();
        if let Some(p) = near {
            let start = fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            let mut dir = if start.is_dir() {
                Some(start.as_path())
            } else {
                start.parent()
            };
            while let Some(d) = dir {
                let s = d.join("schemas");
                if s.is_dir() && !out.contains(&s) {
                    out.push(s);
                }
                // a directory named schemas, or any registry directory (one with an index.lin)
                let is_store =
                    d.file_name().is_some_and(|n| n == "schemas") || d.join("index.lin").is_file();
                if is_store && !out.contains(&d.to_path_buf()) {
                    out.push(d.to_path_buf());
                }
                dir = d.parent();
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            let s = cwd.join("schemas");
            if s.is_dir() && !out.contains(&s) {
                out.push(s);
            }
        }
        if let Some(c) = &self.cache {
            out.push(c.clone());
        }
        out
    }

    /// Every index this tool can see: local index.lin files, then the built-in one, then (when a
    /// number is unknown) the official registry's, fetched and cached.
    fn indexes(&self, near: Option<&Path>) -> Vec<Entry> {
        let mut out = Vec::new();
        for d in self.local_dirs(near) {
            if let Ok(t) = fs::read_to_string(d.join("index.lin")) {
                out.extend(parse_index(&t));
            }
        }
        out.extend(parse_index(embedded::INDEX));
        if let Some(f) = self.fetched_index.borrow().as_ref() {
            out.extend(f.iter().cloned());
        }
        out
    }

    /// The index row for a registered number, fetching the official index if it is unknown here.
    pub fn lookup_number(&self, number: &str, near: Option<&Path>) -> Result<Entry, String> {
        let find = |rows: Vec<Entry>| {
            rows.into_iter()
                .find(|e| e.number.as_deref() == Some(number))
        };
        if let Some(e) = find(self.indexes(near)) {
            return Ok(e);
        }
        if self.offline {
            return Err(format!(
                "schema number {number} is not in a local index, and --offline is set"
            ));
        }
        let official = registries().into_iter().find(|r| r.official);
        let r = official.ok_or("no official registry is configured")?;
        let url = format!("{}index.lin", r.prefix);
        let text = fetch(&url).map_err(|e| format!("{url}: {e}"))?;
        if let Some(c) = &self.cache {
            let _ = fs::create_dir_all(c);
            let _ = fs::write(c.join("index.lin"), &text);
        }
        *self.fetched_index.borrow_mut() = Some(parse_index(&text));
        find(self.indexes(near)).ok_or_else(|| format!("schema number {number} is not registered"))
    }

    /// The index row for a provisional id, if any index lists it.
    pub fn lookup_provisional(&self, provisional: &str, near: Option<&Path>) -> Option<Entry> {
        self.indexes(near)
            .into_iter()
            .find(|e| e.provisional == provisional)
    }

    /// The schema a header names, verified.
    pub fn get(&self, id: &str, url: Option<&str>, near: Option<&Path>) -> Result<Value, String> {
        let (provisional, sha256) = match id_kind(id) {
            Some(IdKind::Provisional) => (id.to_string(), None),
            Some(IdKind::Registered) => {
                let e = self.lookup_number(id, near)?;
                (e.provisional, Some(e.sha256))
            }
            None => return Err(format!("{id} is not a schema id")),
        };
        if let Some(v) = self.loaded.borrow().get(&provisional) {
            return Ok(v.clone());
        }
        let v = self.find(&provisional, sha256.as_deref(), url, near)?;
        self.loaded.borrow_mut().insert(provisional, v.clone());
        Ok(v)
    }

    fn find(
        &self,
        provisional: &str,
        sha256: Option<&str>,
        url: Option<&str>,
        near: Option<&Path>,
    ) -> Result<Value, String> {
        for d in self.local_dirs(near) {
            for ext in ["lin", "json"] {
                let p = d.join(format!("{provisional}.{ext}"));
                if let Ok(text) = fs::read_to_string(&p) {
                    return self.decode(provisional, sha256, &text, Some(&p));
                }
            }
        }
        if let Some((_, text)) = embedded::SCHEMAS.iter().find(|(h, _)| *h == provisional) {
            return self.decode(provisional, sha256, text, None);
        }
        if self.offline {
            return Err("not found locally, and --offline is set".into());
        }
        let mut urls: Vec<String> = Vec::new();
        if let Some(u) = url {
            urls.push(u.to_string());
        }
        for r in registries() {
            urls.push(format!("{}{provisional}.lin", r.prefix));
            urls.push(format!("{}{provisional}.json", r.prefix));
        }
        let mut errs = Vec::new();
        for u in urls {
            match fetch(&u).and_then(|text| {
                self.decode(provisional, sha256, &text, None)
                    .map(|v| (v, text))
            }) {
                Ok((v, text)) => {
                    self.save_cache(provisional, &text);
                    return Ok(v);
                }
                Err(e) => errs.push(format!("{u}: {e}")),
            }
        }
        Err(if errs.is_empty() {
            "not found".into()
        } else {
            errs.join("; ")
        })
    }

    fn save_cache(&self, provisional: &str, text: &str) {
        if let Some(c) = &self.cache {
            let _ = fs::create_dir_all(c);
            let ext = if text.starts_with("LIN ") {
                "lin"
            } else {
                "json"
            };
            let _ = fs::write(c.join(format!("{provisional}.{ext}")), text);
        }
    }

    /// Read a schema file (LIN or JSON) and check it against its provisional id and, when known,
    /// its full SHA-256.
    pub fn decode(
        &self,
        provisional: &str,
        sha256: Option<&str>,
        text: &str,
        path: Option<&Path>,
    ) -> Result<Value, String> {
        let v = if text.starts_with("LIN ") {
            let header = lin::read_header(text).map_err(|e| e.to_string())?;
            match header.schema {
                None => lin::from_str(text).map_err(|e| e.to_string())?,
                Some(meta) => {
                    let meta_prov = match id_kind(&meta) {
                        Some(IdKind::Registered) => {
                            self.lookup_number(&meta, path).ok().map(|e| e.provisional)
                        }
                        _ => Some(meta.clone()),
                    };
                    let self_describing = meta_prov.as_deref() == Some(provisional)
                        || meta_prov
                            .as_ref()
                            .is_some_and(|m| self.loading.borrow().contains(m));
                    if self_describing {
                        // a schema that names itself (the meta-schema): read it by the schema-less
                        // rules, check it, then read it again typed by itself
                        let first = schemaless(text).ok_or("the schema does not parse")?;
                        if Some(schema_id(&first)) == meta_prov {
                            lin::from_str_with_schema(text, Some(&first), ReadOptions::default())
                                .map_err(|e| e.to_string())?
                        } else {
                            first
                        }
                    } else {
                        self.loading.borrow_mut().insert(provisional.to_string());
                        let m = self.get(&meta, header.url.as_deref(), path);
                        self.loading.borrow_mut().remove(provisional);
                        let m = m?;
                        lin::from_str_with_schema(text, Some(&m), ReadOptions::default())
                            .map_err(|e| e.to_string())?
                    }
                }
            }
        } else {
            lin::json::parse(text).map_err(|e| e.to_string())?
        };
        let got = schema_id(&v);
        if got != provisional {
            return Err(format!("its data has id {got}, not {provisional}"));
        }
        if let Some(want) = sha256 {
            if schema_sha256(&v) != want {
                return Err("its SHA-256 differs from the one the registry records".into());
            }
        }
        Ok(v)
    }
}

#[cfg(feature = "net")]
pub fn fetch(url: &str) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err("only https:// URLs are fetched".into());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .build()
        .into();
    let mut resp = agent.get(url).call().map_err(|e| e.to_string())?;
    resp.body_mut()
        .with_config()
        .limit(16 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| e.to_string())
}

#[cfg(not(feature = "net"))]
pub fn fetch(_url: &str) -> Result<String, String> {
    Err("this build of lin has no network support".into())
}
