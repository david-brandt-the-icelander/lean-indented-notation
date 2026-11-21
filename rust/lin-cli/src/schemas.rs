//! Finding schemas by their content address (spec section 7).
//!
//! Lookup order: directories in `LIN_SCHEMA_PATH`, then a `schemas/` directory in the file's
//! directory or any directory above it, then the user cache, then the schemas built into this
//! tool, then the header's URL, then each public registry in `registries.lin` (or
//! `LIN_REGISTRIES`). Whatever is found is hashed and rejected if the hash differs.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use lin::{jcs::is_hash, schema_hash, ReadOptions, Value};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

pub struct Store {
    pub offline: bool,
    path_dirs: Vec<PathBuf>,
    pub cache: Option<PathBuf>,
    loaded: RefCell<HashMap<String, Value>>,
    loading: RefCell<HashSet<String>>,
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

/// The registry prefixes: `LIN_REGISTRIES` (comma-separated), else the built-in registries.lin.
pub fn registries() -> Vec<(String, String)> {
    if let Ok(r) = std::env::var("LIN_REGISTRIES") {
        return r
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| (s.to_string(), s.to_string()))
            .collect();
    }
    let v = match lin::from_str_opts(
        embedded::REGISTRIES,
        ReadOptions {
            ignore_schema: true,
        },
    ) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    v.get("registries")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    Some((
                        r.get("name")?.as_str()?.to_string(),
                        r.get("prefix")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn embedded_schemas() -> &'static [(&'static str, &'static str)] {
    embedded::SCHEMAS
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
        }
    }

    /// Directories searched for `<hash>.lin` / `<hash>.json`, nearest first.
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
                if s.is_dir() {
                    out.push(s);
                }
                if d.file_name().is_some_and(|n| n == "schemas") {
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

    /// The schema with this hash, verified.
    pub fn get(&self, hash: &str, url: Option<&str>, near: Option<&Path>) -> Result<Value, String> {
        if !is_hash(hash) {
            return Err(format!("{hash} is not a schema hash"));
        }
        if let Some(v) = self.loaded.borrow().get(hash) {
            return Ok(v.clone());
        }
        let mut tried = Vec::new();
        for d in self.local_dirs(near) {
            for ext in ["lin", "json"] {
                let p = d.join(format!("{hash}.{ext}"));
                if let Ok(text) = fs::read_to_string(&p) {
                    tried.push(p.display().to_string());
                    let v = self.decode(hash, &text, Some(&p))?;
                    self.loaded.borrow_mut().insert(hash.to_string(), v.clone());
                    return Ok(v);
                }
            }
        }
        if let Some((_, text)) = embedded::SCHEMAS.iter().find(|(h, _)| *h == hash) {
            let v = self.decode(hash, text, None)?;
            self.loaded.borrow_mut().insert(hash.to_string(), v.clone());
            return Ok(v);
        }
        if self.offline {
            return Err("not found locally, and --offline is set".into());
        }
        let mut urls: Vec<String> = Vec::new();
        if let Some(u) = url {
            urls.push(u.to_string());
        }
        for (_, prefix) in registries() {
            urls.push(format!("{prefix}{hash}.lin"));
            urls.push(format!("{prefix}{hash}.json"));
        }
        let mut errs = Vec::new();
        for u in urls {
            match fetch(&u) {
                Ok(text) => match self.decode(hash, &text, None) {
                    Ok(v) => {
                        self.save_cache(hash, &text);
                        self.loaded.borrow_mut().insert(hash.to_string(), v.clone());
                        return Ok(v);
                    }
                    Err(e) => errs.push(format!("{u}: {e}")),
                },
                Err(e) => errs.push(format!("{u}: {e}")),
            }
        }
        Err(if errs.is_empty() {
            "not found".into()
        } else {
            errs.join("; ")
        })
    }

    fn save_cache(&self, hash: &str, text: &str) {
        if let Some(c) = &self.cache {
            let _ = fs::create_dir_all(c);
            let ext = if text.starts_with("LIN ") {
                "lin"
            } else {
                "json"
            };
            let _ = fs::write(c.join(format!("{hash}.{ext}")), text);
        }
    }

    /// Read a schema file (LIN or JSON) and check that its data hashes to `hash`.
    pub fn decode(&self, hash: &str, text: &str, path: Option<&Path>) -> Result<Value, String> {
        let v = if text.starts_with("LIN ") {
            let header = lin::read_header(text).map_err(|e| e.to_string())?;
            match header.schema {
                None => lin::from_str(text).map_err(|e| e.to_string())?,
                Some(meta) => {
                    if self.loading.borrow().contains(&meta) || meta == hash {
                        // a schema that names itself (the meta-schema): read it by the schema-less
                        // rules, check the hash, then read it again typed by itself
                        let first = lin::from_str_opts(
                            text,
                            ReadOptions {
                                ignore_schema: true,
                            },
                        )
                        .map_err(|e| e.to_string())?;
                        if schema_hash(&first) == meta {
                            lin::from_str_with_schema(text, Some(&first), ReadOptions::default())
                                .map_err(|e| e.to_string())?
                        } else {
                            first
                        }
                    } else {
                        self.loading.borrow_mut().insert(hash.to_string());
                        let m = self.get(&meta, header.url.as_deref(), path);
                        self.loading.borrow_mut().remove(hash);
                        let m = m?;
                        lin::from_str_with_schema(text, Some(&m), ReadOptions::default())
                            .map_err(|e| e.to_string())?
                    }
                }
            }
        } else {
            lin::json::parse(text).map_err(|e| e.to_string())?
        };
        let got = schema_hash(&v);
        if got != hash {
            return Err(format!("its data hashes to {got}, not {hash}"));
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
