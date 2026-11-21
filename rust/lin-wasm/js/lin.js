// LIN for JavaScript, backed by the Rust library compiled to WebAssembly.
//
//   import init, * as lin from "./lin.js";
//   await init();                       // in a browser; loads lin_wasm_bg.wasm
//   lin.stringify({ name: "demo" })     // "LIN 1\nname demo\n"
//   lin.parse("LIN 1\nname demo\n")     // { name: "demo" }
//
// parse/stringify use plain JavaScript values, so they share JSON.parse's limits (integer-like
// keys come first, numbers are doubles). linToJson/jsonToLin work on text and keep everything.
import wasmInit, * as raw from "./pkg/lin_wasm.js";

export const { linToJson, jsonToLin, readHeader, schemaHash, canonicalize, version, Options } = raw;

export default function init(input) {
  return wasmInit(input);
}

function options(o = {}) {
  const opt = new raw.Options();
  if (o.split) opt.split = true;
  if (o.splitLimit !== undefined) opt.splitLimit = o.splitLimit;
  if (o.inlineLimit !== undefined) opt.inlineLimit = o.inlineLimit;
  if (o.objectLimit !== undefined) opt.objectLimit = o.objectLimit;
  if (o.schemaHash) opt.schema = o.schemaHash;
  if (o.url) opt.url = o.url;
  return opt;
}

/** Read LIN text into a JavaScript value. `schema` is the schema object the header names. */
export function parse(text, { schema, ignoreSchema = false } = {}) {
  return JSON.parse(raw.linToJson(text, schema ? JSON.stringify(schema) : undefined, ignoreSchema, true));
}

/** Write a JavaScript value as LIN. Options: split, splitLimit, inlineLimit, objectLimit,
 * schemaHash, url, schema (the schema object, for schema-aware quoting). */
export function stringify(value, o = {}) {
  return raw.jsonToLin(JSON.stringify(value), options(o), o.schema ? JSON.stringify(o.schema) : undefined);
}
