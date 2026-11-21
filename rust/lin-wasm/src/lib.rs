//! LIN for web pages: the Rust library compiled to WebAssembly.
//!
//! Values cross the boundary as JSON text, so key order and exact numbers survive; the
//! JavaScript wrapper (`js/lin.js`) adds `parse` and `stringify` on top.

use lin::{json, ReadOptions, Value, WriteOptions};
use wasm_bindgen::prelude::*;

fn err(e: lin::Error) -> JsError {
    JsError::new(&e.to_string())
}

fn schema_arg(schema_json: Option<String>) -> Result<Option<Value>, JsError> {
    match schema_json {
        Some(s) if !s.is_empty() => Ok(Some(json::parse(&s).map_err(err)?)),
        _ => Ok(None),
    }
}

/// LIN text to JSON text. `schema_json` is the schema the header names, as JSON text.
#[wasm_bindgen(js_name = linToJson)]
pub fn lin_to_json(
    text: &str,
    schema_json: Option<String>,
    ignore_schema: Option<bool>,
    compact: Option<bool>,
) -> Result<String, JsError> {
    let schema = schema_arg(schema_json)?;
    let v = lin::from_str_with_schema(
        text,
        schema.as_ref(),
        ReadOptions {
            ignore_schema: ignore_schema.unwrap_or(false),
        },
    )
    .map_err(err)?;
    Ok(if compact.unwrap_or(false) {
        json::to_string(&v)
    } else {
        json::to_string_pretty(&v)
    })
}

/// Writer options, all optional.
#[wasm_bindgen]
#[derive(Default)]
pub struct Options {
    pub split: bool,
    #[wasm_bindgen(js_name = splitLimit)]
    pub split_limit: Option<u32>,
    #[wasm_bindgen(js_name = inlineLimit)]
    pub inline_limit: Option<u32>,
    #[wasm_bindgen(js_name = objectLimit)]
    pub object_limit: Option<u32>,
    schema: Option<String>,
    url: Option<String>,
}

#[wasm_bindgen]
impl Options {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Options {
        Options::default()
    }
    #[wasm_bindgen(setter)]
    pub fn set_schema(&mut self, s: Option<String>) {
        self.schema = s;
    }
    #[wasm_bindgen(getter)]
    pub fn schema(&self) -> Option<String> {
        self.schema.clone()
    }
    #[wasm_bindgen(setter)]
    pub fn set_url(&mut self, s: Option<String>) {
        self.url = s;
    }
    #[wasm_bindgen(getter)]
    pub fn url(&self) -> Option<String> {
        self.url.clone()
    }
}

/// JSON text to LIN text. `schema_json` (optional) is the schema itself, so strings that look
/// like numbers can go bare where it says they are strings.
#[wasm_bindgen(js_name = jsonToLin)]
pub fn json_to_lin(
    text: &str,
    options: Option<Options>,
    schema_json: Option<String>,
) -> Result<String, JsError> {
    let o = options.unwrap_or_default();
    let d = WriteOptions::default();
    let opts = WriteOptions {
        split: o.split,
        schema: o.schema.clone(),
        url: o.url.clone(),
        split_limit: o.split_limit.map_or(d.split_limit, |x| x as usize),
        inline_limit: o.inline_limit.map(|x| x as usize),
        object_limit: o.object_limit.map_or(d.object_limit, |x| x as usize),
        header: true,
    };
    let schema = schema_arg(schema_json)?;
    lin::json_to_lin(text, &opts, schema.as_ref()).map_err(err)
}

/// The header of a LIN text as JSON: {"version":1,"schema":…,"url":…}.
#[wasm_bindgen(js_name = readHeader)]
pub fn read_header(text: &str) -> Result<String, JsError> {
    let h = lin::read_header(text).map_err(err)?;
    let mut m = lin::Map::new();
    m.insert("version".into(), Value::from(h.version as i64));
    m.insert("schema".into(), h.schema.map_or(Value::Null, Value::String));
    m.insert("url".into(), h.url.map_or(Value::Null, Value::String));
    Ok(json::to_string(&Value::Object(m)))
}

/// The normalized SHA-256 (`sha256-…`) of a schema given as JSON text.
#[wasm_bindgen(js_name = schemaHash)]
pub fn schema_hash(schema_json: &str) -> Result<String, JsError> {
    Ok(lin::schema_hash(&json::parse(schema_json).map_err(err)?))
}

/// JSON text in canonical (RFC 8785) form.
#[wasm_bindgen]
pub fn canonicalize(json_text: &str) -> Result<String, JsError> {
    Ok(lin::canonicalize(&json::parse(json_text).map_err(err)?))
}

/// The library's version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
