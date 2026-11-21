//! LIN (Lean Indented Notation): JSON's data model in fewer tokens, for AI agents.
//!
//! ```
//! let v = lin::json::parse(r#"{"name": "demo", "tags": ["a", "b"]}"#).unwrap();
//! let text = lin::to_string(&v).unwrap();
//! assert_eq!(text, "LIN 1\nname demo\ntags[] a|b\n");
//! assert_eq!(lin::from_str(&text).unwrap(), v);
//! ```
//!
//! The spec is `spec/lin-1.md` in the repository. Files whose header names a schema are typed by
//! that schema; read them with [`from_str_with_schema`], or with [`ReadOptions::ignore_schema`].

pub mod jcs;
pub mod json;
pub mod read;
pub mod schema;
pub mod text;
pub mod value;
pub mod write;

pub use jcs::{canonicalize, id_kind, schema_id, schema_sha256, IdKind};
pub use read::{parse, resolve, Parsed, Raw};
pub use value::{Map, Number, Value};
pub use write::{to_string_with, WriteOptions};

use std::fmt;

/// A LIN or JSON text that breaks a rule. `line` is 1-based when known.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub message: String,
    pub line: Option<usize>,
    pub kind: ErrorKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Syntax,
    /// The header names a schema that was not supplied.
    SchemaRequired,
    /// The schema supplied is not the one the header names.
    SchemaMismatch,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Error {
        Error {
            message: message.into(),
            line: None,
            kind: ErrorKind::Syntax,
        }
    }
    pub fn at(message: impl Into<String>, line: usize) -> Error {
        Error {
            message: message.into(),
            line: Some(line),
            kind: ErrorKind::Syntax,
        }
    }
    fn kind(mut self, kind: ErrorKind) -> Error {
        self.kind = kind;
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(l) => write!(f, "line {l}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for Error {}

/// The header line: `LIN <version> [<schema> [<url>]]`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Header {
    pub version: u32,
    pub schema: Option<String>,
    pub url: Option<String>,
}

/// Read the header of a LIN text.
pub fn read_header(text: &str) -> Result<Header, Error> {
    read::read_header(text.split('\n').next().unwrap_or(""), 1)
}

/// How to read a file whose header names a schema.
#[derive(Clone, Debug, Default)]
pub struct ReadOptions {
    /// Read by the schema-less rules even though the header names a schema (an explicit
    /// choice, as spec section 7 requires).
    pub ignore_schema: bool,
    /// For a registered number: the SHA-256 (hex) the registry's index records for it. The
    /// schema given is checked against it. A provisional id is always checked.
    pub sha256: Option<String>,
}

/// Read a LIN text that names no schema (or ignore it with [`from_str_opts`]).
pub fn from_str(text: &str) -> Result<Value, Error> {
    from_str_with_schema(text, None, ReadOptions::default())
}

/// Read a LIN text by the schema-less rules even if its header names a schema.
pub fn from_str_opts(text: &str, opts: ReadOptions) -> Result<Value, Error> {
    from_str_with_schema(text, None, opts)
}

/// Read a LIN text typed by `schema` (a JSON Schema as a value). If the header names a schema,
/// `schema` must hash to it.
pub fn from_str_with_schema(
    text: &str,
    schema: Option<&Value>,
    opts: ReadOptions,
) -> Result<Value, Error> {
    let parsed = parse(text)?;
    let mut use_schema = schema;
    if let Some(h) = &parsed.header.schema {
        if opts.ignore_schema {
            use_schema = None;
        } else {
            match schema {
                None => {
                    return Err(Error::new(format!(
                        "the file names schema {h}; supply it, or read without it explicitly"
                    ))
                    .kind(ErrorKind::SchemaRequired))
                }
                Some(s) => {
                    if id_kind(h) == Some(IdKind::Provisional) {
                        let got = schema_id(s);
                        if &got != h {
                            return Err(Error::new(format!(
                                "schema mismatch: the header names {h}, the schema given has id {got}"
                            ))
                            .kind(ErrorKind::SchemaMismatch));
                        }
                    } else if let Some(want) = &opts.sha256 {
                        if &schema_sha256(s) != want {
                            return Err(Error::new(format!(
                                "schema mismatch: the schema given is not the one registered as {h}"
                            ))
                            .kind(ErrorKind::SchemaMismatch));
                        }
                    }
                }
            }
        }
    }
    resolve(parsed, use_schema)
}

/// Write a value as LIN with the default options.
pub fn to_string(value: &Value) -> Result<String, Error> {
    to_string_with(value, &WriteOptions::default(), None)
}

/// LIN text to indented JSON text (the format of the `.json` siblings in the repository).
pub fn lin_to_json(text: &str, schema: Option<&Value>, opts: ReadOptions) -> Result<String, Error> {
    Ok(json::to_string_pretty(&from_str_with_schema(
        text, schema, opts,
    )?))
}

/// JSON text to LIN text.
pub fn json_to_lin(
    text: &str,
    opts: &WriteOptions,
    schema_doc: Option<&Value>,
) -> Result<String, Error> {
    to_string_with(&json::parse(text)?, opts, schema_doc)
}

/// A value as LIN without a header: the body of an object or array, or a scalar's line form.
pub fn to_fragment(value: &Value, opts: &WriteOptions) -> String {
    if value.is_container() {
        let mut o = opts.clone();
        o.header = false;
        to_string_with(value, &o, None).unwrap_or_default()
    } else {
        format!("{}\n", write::scalar_text(value))
    }
}
