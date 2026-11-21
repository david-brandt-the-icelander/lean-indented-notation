//! Working on many files: `lin check` and `lin sync`.

use std::fs;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use lin::{json, Value};

use crate::doc::{self, Format, Layout, LoadOptions};
use crate::schemas::Store;

const SKIP_DIRS: [&str; 5] = [".git", "target", "node_modules", "dist", "pkg"];

/// Patterns from `.linignore` files (in the current directory and the repository root), one
/// glob per line, `#` comments. A pattern without `/` matches at any depth.
pub struct Ignore {
    sets: Vec<(PathBuf, GlobSet)>,
}

impl Ignore {
    pub fn load() -> Ignore {
        let mut sets = Vec::new();
        let mut dirs = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            let mut d = Some(cwd.as_path());
            while let Some(x) = d {
                dirs.push(x.to_path_buf());
                if x.join(".git").exists() {
                    break;
                }
                d = x.parent();
            }
        }
        for d in dirs {
            let f = d.join(".linignore");
            if let Ok(text) = fs::read_to_string(&f) {
                let mut b = GlobSetBuilder::new();
                for line in text
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                {
                    let pat = line.trim_start_matches('/');
                    let pat = if line.contains('/') {
                        pat.to_string()
                    } else {
                        format!("**/{pat}")
                    };
                    if let Ok(g) = Glob::new(&pat) {
                        b.add(g);
                    }
                    if let Ok(g) = Glob::new(&format!("{}/**", pat.trim_end_matches('/'))) {
                        b.add(g);
                    }
                }
                if let Ok(s) = b.build() {
                    sets.push((d, s));
                }
            }
        }
        Ignore { sets }
    }

    pub fn ignored(&self, p: &Path) -> bool {
        let abs = fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        self.sets
            .iter()
            .any(|(root, set)| abs.strip_prefix(root).is_ok_and(|rel| set.is_match(rel)))
    }
}

/// Every file under `roots` with one of the extensions, skipping ignored paths.
pub fn files(roots: &[PathBuf], exts: &[&str], ignore: &Ignore) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for r in roots {
        walk(r, exts, ignore, &mut out, true);
    }
    out.sort();
    out.dedup();
    out
}

fn walk(p: &Path, exts: &[&str], ignore: &Ignore, out: &mut Vec<PathBuf>, top: bool) {
    if !top && ignore.ignored(p) {
        return;
    }
    if p.is_dir() {
        if !top
            && p.file_name()
                .is_some_and(|n| SKIP_DIRS.contains(&n.to_string_lossy().as_ref()))
        {
            return;
        }
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                walk(&e.path(), exts, ignore, out, false);
            }
        }
    } else if exts.iter().any(|e| p.to_string_lossy().ends_with(e)) && (top || !ignore.ignored(p)) {
        out.push(p.to_path_buf());
    }
}

pub struct CheckOptions {
    pub no_schema: bool,
    pub layout: bool,
    pub require_siblings: bool,
}

/// Problems found in one file, as `file:line: message` lines.
pub fn check_file(p: &Path, store: &Store, o: &CheckOptions) -> Vec<String> {
    let name = p.display().to_string();
    let mut errs = Vec::new();
    let text = match fs::read_to_string(p) {
        Ok(t) => t,
        Err(e) => return vec![format!("{name}: {e}")],
    };
    let format = doc::detect(&text);
    if format == Format::Lin && !text.ends_with('\n') {
        errs.push(format!("{name}: the file must end with a newline"));
    }
    let lo = LoadOptions {
        store,
        no_schema: o.no_schema,
    };
    let d = match doc::load_text(text.clone(), Some(p), &lo) {
        Ok(d) => d,
        Err(e) => {
            errs.push(e);
            return errs;
        }
    };
    if let Some(schema) = &d.schema {
        match crate::extra::validate(&d.value, schema) {
            Ok(v) => errs.extend(v.into_iter().map(|e| format!("{name}: schema: {e}"))),
            Err(e) => errs.push(format!("{name}: {e}")),
        }
    }
    let file = p
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    if file.starts_with("sha256-") {
        let stem = file.trim_end_matches(".lin").trim_end_matches(".json");
        let h = lin::schema_hash(&d.value);
        if h != stem {
            errs.push(format!(
                "{name}: the file is named for {stem}, but its data hashes to {h}"
            ));
        }
    }
    if format == Format::Lin {
        if o.layout {
            if let Ok(want) = doc::render(&d, &d.value, &Layout::default()) {
                if want != text {
                    errs.push(format!("{name}: not in the writer's layout (run lin fmt)"));
                }
            }
        }
        if let Some(sib) = doc::sibling(p) {
            match fs::read_to_string(&sib) {
                Ok(j) => {
                    if j != json::to_string_pretty(&d.value) {
                        let why = match json::parse(&j) {
                            Ok(v) if v == d.value => {
                                "same data, but not in the canonical JSON layout"
                            }
                            Ok(_) => "different data",
                            Err(_) => "not valid JSON",
                        };
                        errs.push(format!(
                            "{name}: {} is out of step ({why}); run lin sync",
                            sib.display()
                        ));
                    }
                }
                Err(_) if o.require_siblings => errs.push(format!(
                    "{name}: no {} beside it; run lin sync",
                    sib.display()
                )),
                Err(_) => {}
            }
        }
    }
    errs
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Prefer {
    /// Whichever file was modified last
    Newer,
    Lin,
    Json,
}

/// Bring a .lin file and its .json sibling into step. Returns a description of what changed,
/// or None if they already agree. With `dry`, nothing is written.
pub fn sync_file(
    p: &Path,
    store: &Store,
    prefer: Prefer,
    dry: bool,
) -> Result<Option<String>, String> {
    let sib = doc::sibling(p).ok_or("not a .lin file")?;
    let lo = LoadOptions {
        store,
        no_schema: false,
    };
    let lin_doc = doc::load(Some(p), &lo)?;
    let want = json::to_string_pretty(&lin_doc.value);
    let current = fs::read_to_string(&sib).ok();
    if current.as_deref() == Some(want.as_str()) {
        return Ok(None);
    }
    let json_newer = || -> bool {
        let m = |x: &Path| fs::metadata(x).and_then(|m| m.modified()).ok();
        matches!((m(p), m(&sib)), (Some(a), Some(b)) if b > a)
    };
    let from_json = match (&current, prefer) {
        (None, _) => false,
        (Some(_), Prefer::Lin) => false,
        (Some(_), Prefer::Json) => true,
        (Some(_), Prefer::Newer) => json_newer(),
    };
    if from_json {
        let j = current.unwrap();
        let v: Value = json::parse(&j).map_err(|e| format!("{}: {e}", sib.display()))?;
        if v == lin_doc.value {
            // same data, the JSON just isn't in canonical layout
            if !dry {
                doc::write_file(&sib, &want)?;
            }
            return Ok(Some(format!(
                "{}: rewrote in canonical JSON layout",
                sib.display()
            )));
        }
        if !dry {
            let text = doc::render(&lin_doc, &v, &Layout::default())?;
            doc::write_file(p, &text)?;
            doc::write_file(&sib, &json::to_string_pretty(&v))?;
        }
        Ok(Some(format!(
            "{}: updated from {}",
            p.display(),
            sib.display()
        )))
    } else {
        if !dry {
            doc::write_file(&sib, &want)?;
        }
        Ok(Some(format!(
            "{}: {} from {}",
            sib.display(),
            if current.is_some() {
                "updated"
            } else {
                "created"
            },
            p.display()
        )))
    }
}
