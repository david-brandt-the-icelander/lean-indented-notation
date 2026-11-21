//! Loading and saving documents: LIN or JSON, from files or stdin, with their schemas.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use lin::{json, Header, ReadOptions, Value, WriteOptions};

use crate::schemas::Store;
use crate::{fail, Res};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Lin,
    Json,
}

/// A loaded document and how it was stored.
pub struct Doc {
    pub path: Option<PathBuf>,
    pub format: Format,
    pub header: Header,
    pub split: bool,
    pub value: Value,
    /// The schema the file names, when it was found.
    pub schema: Option<Value>,
}

pub fn read_text(path: Option<&Path>) -> Res<String> {
    match path {
        None => {
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| format!("stdin: {e}"))?;
            Ok(s)
        }
        Some(p) if p.as_os_str() == "-" => read_text(None),
        Some(p) => fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display())),
    }
}

/// LIN files start with the header `LIN `; everything else is read as JSON.
pub fn detect(text: &str) -> Format {
    if text.starts_with("LIN ") || text.starts_with("LIN\n") || text == "LIN" {
        Format::Lin
    } else {
        Format::Json
    }
}

pub struct LoadOptions<'a> {
    pub store: &'a Store,
    /// Read by the schema-less rules even when the header names a schema.
    pub no_schema: bool,
}

pub fn load(path: Option<&Path>, o: &LoadOptions) -> Res<Doc> {
    let text = read_text(path)?;
    load_text(text, path, o)
}

pub fn load_text(text: String, path: Option<&Path>, o: &LoadOptions) -> Res<Doc> {
    let name = path.map_or("<stdin>".to_string(), |p| p.display().to_string());
    let format = if path.is_some_and(|p| p.extension().is_some_and(|e| e == "lin")) {
        Format::Lin
    } else {
        detect(&text)
    };
    match format {
        Format::Json => {
            let value = json::parse(&text).map_err(|e| format!("{name}: {e}"))?;
            Ok(Doc {
                path: path.map(Path::to_path_buf),
                format: Format::Json,
                header: Header::default(),
                split: false,
                value,
                schema: None,
            })
        }
        Format::Lin => {
            let header = lin::parse(&text)
                .map_err(|e| format!("{name}: {e}"))?
                .header;
            let split = text.split('\n').any(|l| l == "---");
            let mut schema = None;
            if let Some(h) = &header.schema {
                if !o.no_schema {
                    match o.store.get(h, header.url.as_deref(), path) {
                        Ok(s) => schema = Some(s),
                        Err(e) => {
                            return fail(format!(
                                "{name}: the header names schema {h}, which could not be found ({e}). \
                                 Put it in a schemas/ directory, set LIN_SCHEMA_PATH, or pass --no-schema to read it by the schema-less rules."
                            ))
                        }
                    }
                }
            }
            let ro = ReadOptions {
                ignore_schema: o.no_schema,
            };
            let value = lin::from_str_with_schema(&text, schema.as_ref(), ro)
                .map_err(|e| format!("{name}: {e}"))?;
            Ok(Doc {
                path: path.map(Path::to_path_buf),
                format: Format::Lin,
                header,
                split,
                value,
                schema,
            })
        }
    }
}

/// Writer settings shared by every command that writes LIN.
#[derive(clap::Args, Clone, Debug, Default)]
pub struct Layout {
    /// Write the split form (long values after a --- line)
    #[arg(long, conflicts_with = "plain")]
    pub split: bool,
    /// Write the plain form
    #[arg(long)]
    pub plain: bool,
    /// Split form: move strings longer than this many characters [default: 24]
    #[arg(long, value_name = "N")]
    pub split_limit: Option<usize>,
    /// Put a list of scalars inline only if no item is longer than N characters [default: always]
    #[arg(long, value_name = "N")]
    pub inline_limit: Option<usize>,
    /// Put an object of scalars inline when its fields fit in N characters; 0 never [default: 80]
    #[arg(long, value_name = "N")]
    pub object_limit: Option<usize>,
}

impl Layout {
    pub fn options(&self, keep_split: bool, header: &Header) -> WriteOptions {
        let d = WriteOptions::default();
        WriteOptions {
            split: if self.split {
                true
            } else if self.plain {
                false
            } else {
                keep_split
            },
            schema: header.schema.clone(),
            url: header.url.clone(),
            split_limit: self.split_limit.unwrap_or(d.split_limit),
            inline_limit: self.inline_limit.or(d.inline_limit),
            object_limit: self.object_limit.unwrap_or(d.object_limit),
            header: true,
        }
    }
}

/// The text of a document in its own format, with a new value.
pub fn render(doc: &Doc, value: &Value, layout: &Layout) -> Res<String> {
    match doc.format {
        Format::Json => Ok(json::to_string_pretty(value)),
        Format::Lin => {
            let opts = layout.options(doc.split, &doc.header);
            lin::to_string_with(value, &opts, doc.schema.as_ref()).map_err(|e| e.to_string())
        }
    }
}

pub fn sibling(path: &Path) -> Option<PathBuf> {
    let s = path.to_string_lossy();
    if let Some(b) = s.strip_suffix(".lin") {
        Some(PathBuf::from(format!("{b}.json")))
    } else {
        s.strip_suffix(".json")
            .map(|b| PathBuf::from(format!("{b}.lin")))
    }
}

/// Write a document back to its file, and keep a .lin/.json sibling in lockstep if one exists.
pub fn save(doc: &Doc, value: &Value, layout: &Layout, store: &Store) -> Res<Vec<PathBuf>> {
    let path = doc
        .path
        .as_ref()
        .ok_or("no file to write to (input came from stdin); use --stdout")?;
    let text = render(doc, value, layout)?;
    write_file(path, &text)?;
    let mut written = vec![path.clone()];
    if let Some(sib) = sibling(path) {
        if sib.exists() {
            let other = match doc.format {
                Format::Lin => json::to_string_pretty(value),
                Format::Json => {
                    // keep the sibling's own header and layout
                    let old = fs::read_to_string(&sib).unwrap_or_default();
                    let header = lin::read_header(&old).unwrap_or_default();
                    let split = old.split('\n').any(|l| l == "---");
                    let opts = layout.options(split, &header);
                    let schema = header
                        .schema
                        .as_ref()
                        .and_then(|h| store.get(h, header.url.as_deref(), Some(&sib)).ok());
                    lin::to_string_with(value, &opts, schema.as_ref()).map_err(|e| e.to_string())?
                }
            };
            write_file(&sib, &other)?;
            written.push(sib);
        }
    }
    Ok(written)
}

pub fn write_file(path: &Path, text: &str) -> Res<()> {
    let tmp = path.with_extension(format!(
        "{}.tmp-lin",
        path.extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default()
    ));
    fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}
