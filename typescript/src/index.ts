/**
 * LIN (Lean Indented Notation): JSON's data model in fewer tokens, for AI agents.
 *
 * ```ts
 * import * as lin from "lean-indented-notation";
 * lin.stringify({ name: "demo", tags: ["a", "b"] }); // "LIN 1\nname demo\ntags[] a|b\n"
 * lin.parse("LIN 1\nname demo\n");                    // { name: "demo" }
 * ```
 *
 * `parse` and `stringify` use plain JavaScript values and so share JSON.parse's limits:
 * integer-like keys come first, and numbers are doubles (integers beyond 2^53 come back as
 * bigint). `linToJson` and `jsonToLin` work on text and keep key order and every digit.
 */
import { canonicalize, isHash, schemaHash } from "./jcs.js";
import { LinError, Node, SchemaRequiredError, fromJs, toJs } from "./node.js";
import { Header, parse as parseRaw, readHeaderLine, resolve } from "./read.js";
import { jsonCompact, jsonPretty, parseJson } from "./text.js";
import { WriteOptions, writeNode } from "./write.js";

export { LinError, SchemaRequiredError, canonicalize, schemaHash, isHash };
export type { Header, WriteOptions };

export interface ReadOptions {
  /** The schema the header names (a JavaScript value); its hash must match the header. */
  schema?: unknown;
  /** Read a file that names a schema by the schema-less rules (an explicit choice, spec section 7). */
  ignoreSchema?: boolean;
}

/** The header line of a LIN text. */
export function readHeader(text: string): Header {
  return readHeaderLine(text.split("\n", 1)[0]);
}

function readNode(text: string, o: ReadOptions = {}): Node {
  const parsed = parseRaw(text);
  let schema = o.schema;
  const h = parsed.header.schema;
  if (h) {
    if (o.ignoreSchema) schema = undefined;
    else if (schema === undefined) throw new SchemaRequiredError(`the file names schema ${h}; pass { schema } or { ignoreSchema: true }`);
    else {
      const got = schemaHash(schema);
      if (got !== h) throw new LinError(`schema hash mismatch: the header names ${h}, the schema given hashes to ${got}`);
    }
  }
  return resolve(parsed, schema);
}

/** Read LIN text into a JavaScript value. */
export function parse(text: string, o: ReadOptions = {}): any {
  return toJs(readNode(text, o));
}

/** Write a JavaScript value (an object or array) as LIN. */
export function stringify(value: unknown, o: WriteOptions = {}): string {
  return writeNode(fromJs(value), o);
}

/** LIN text to JSON text (2-space indented with a final newline, or compact). Lossless. */
export function linToJson(text: string, o: ReadOptions & { compact?: boolean } = {}): string {
  const n = readNode(text, o);
  return o.compact ? jsonCompact(n) : jsonPretty(n);
}

/** JSON text to LIN text. Lossless. */
export function jsonToLin(json: string, o: WriteOptions = {}): string {
  return writeNode(parseJson(json), o);
}

/** A value as JSON text in the layout of the .json files beside .lin files. */
export function toJson(value: unknown, compact = false): string {
  const n = fromJs(value);
  return compact ? jsonCompact(n) : jsonPretty(n);
}

/** Parse JSON strictly (duplicate keys are errors). */
export function fromJson(text: string): any {
  return toJs(parseJson(text));
}

export const VERSION = "1.0.0";
