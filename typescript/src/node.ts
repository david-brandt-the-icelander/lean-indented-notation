// The internal value tree. It keeps key order and exact numbers, which plain JavaScript values
// cannot (integer-like keys sort first; numbers are doubles).

export type Node =
  | { t: "null" }
  | { t: "bool"; v: boolean }
  | { t: "num"; v: string } // canonical number text
  | { t: "str"; v: string; bareOk?: boolean } // bareOk: may go bare although it looks like a number
  | { t: "arr"; v: Node[] }
  | { t: "obj"; v: [string, Node][] }
  | { t: "bare"; v: string } // reader only: number or boolean text not yet typed
  | { t: "ref"; name: string[] | null; value?: Node; pos?: unknown };

export class LinError extends Error {
  readonly line?: number;
  constructor(message: string, line?: number) {
    super(line ? `line ${line}: ${message}` : message);
    this.name = "LinError";
    this.line = line;
  }
}

/** The header names a schema that was not supplied. */
export class SchemaRequiredError extends LinError {
  constructor(message: string) {
    super(message);
    this.name = "SchemaRequiredError";
  }
}

const NUM_RE = /^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?$/;

export function isJsonNumber(s: string): boolean {
  return NUM_RE.test(s);
}

/** Canonical text of a JSON number: integers keep their digits, other numbers take the
 * shortest form JSON produces (ECMAScript Number::toString). */
export function canonicalNumber(text: string): string {
  if (/^-?[0-9]+$/.test(text)) {
    return /^-?0+$/.test(text) ? "0" : text;
  }
  const f = Number(text);
  if (!Number.isFinite(f)) throw new LinError(`number out of range: ${text}`);
  return f === 0 ? "0" : String(f);
}

export function isIntegerText(s: string): boolean {
  return /^-?[0-9]+$/.test(s);
}

export function typeName(n: Node): string {
  switch (n.t) {
    case "null":
      return "null";
    case "bool":
      return "boolean";
    case "num":
      return "number";
    case "str":
      return "string";
    case "arr":
      return "array";
    case "obj":
      return "object";
    default:
      return "string";
  }
}

/** A plain JavaScript value (as JSON.parse gives, bigint allowed) to a node. */
export function fromJs(v: unknown): Node {
  if (v === null) return { t: "null" };
  if (typeof v === "boolean") return { t: "bool", v };
  if (typeof v === "number") {
    if (!Number.isFinite(v)) throw new LinError("LIN numbers must be finite");
    return { t: "num", v: v === 0 ? "0" : String(v) };
  }
  if (typeof v === "bigint") return { t: "num", v: canonicalNumber(v.toString()) };
  if (typeof v === "string") return { t: "str", v };
  if (Array.isArray(v)) return { t: "arr", v: v.map(fromJs) };
  if (typeof v === "object") {
    const out: [string, Node][] = [];
    for (const [k, x] of Object.entries(v as Record<string, unknown>)) {
      if (x === undefined) continue;
      out.push([k, fromJs(x)]);
    }
    return { t: "obj", v: out };
  }
  throw new LinError(`not a JSON value: ${typeof v}`);
}

/** A node to a plain JavaScript value. Integers beyond 2^53 become bigint. */
export function toJs(n: Node): unknown {
  switch (n.t) {
    case "null":
      return null;
    case "bool":
      return n.v;
    case "num": {
      if (isIntegerText(n.v)) {
        const f = Number(n.v);
        return Number.isSafeInteger(f) ? f : BigInt(n.v);
      }
      return Number(n.v);
    }
    case "str":
      return n.v;
    case "arr":
      return n.v.map(toJs);
    case "obj": {
      const o: Record<string, unknown> = {};
      for (const [k, x] of n.v) {
        Object.defineProperty(o, k, { value: toJs(x), enumerable: true, writable: true, configurable: true });
      }
      return o;
    }
    default:
      throw new LinError("unresolved value");
  }
}

export function nodeEqual(a: Node, b: Node): boolean {
  if (a.t !== b.t) return false;
  switch (a.t) {
    case "arr": {
      const bv = (b as typeof a).v;
      return a.v.length === bv.length && a.v.every((x, i) => nodeEqual(x, bv[i]));
    }
    case "obj": {
      const bv = (b as typeof a).v;
      return a.v.length === bv.length && a.v.every(([k, x], i) => k === bv[i][0] && nodeEqual(x, bv[i][1]));
    }
    case "null":
      return true;
    default:
      return (a as { v: unknown }).v === (b as { v: unknown }).v;
  }
}
