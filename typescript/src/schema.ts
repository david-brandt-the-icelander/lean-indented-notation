// How a schema types bare scalars (spec section 7). The same rules as the Python and Rust
// libraries: type, const, enum, allOf, anyOf, oneOf, local $ref, properties,
// patternProperties, additionalProperties, prefixItems and items. Anything else allows every type.

import { isJsonNumber } from "./node.js";

export const NULL = 1, BOOLEAN = 2, NUMBER = 4, STRING = 8, ARRAY = 16, OBJECT = 32, ALL = 63;

type J = unknown;
export type Pos =
  | { k: "true" }
  | { k: "node"; v: J }
  | { k: "noref"; v: J }
  | { k: "all"; ps: Pos[] }
  | { k: "any"; ps: Pos[] };

const TRUE: Pos = { k: "true" };

function bitOfName(n: unknown): number {
  switch (n) {
    case "null":
      return NULL;
    case "boolean":
      return BOOLEAN;
    case "number":
    case "integer":
      return NUMBER;
    case "string":
      return STRING;
    case "array":
      return ARRAY;
    case "object":
      return OBJECT;
    default:
      return 0;
  }
}

function bitOfValue(v: unknown): number {
  if (v === null) return NULL;
  if (typeof v === "boolean") return BOOLEAN;
  if (typeof v === "number" || typeof v === "bigint") return NUMBER;
  if (typeof v === "string") return STRING;
  if (Array.isArray(v)) return ARRAY;
  return OBJECT;
}

function isObj(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function has(o: Record<string, unknown>, k: string): boolean {
  return Object.prototype.hasOwnProperty.call(o, k);
}

/** Would bare text s, at a position allowing `types`, read back as the string s? */
export function bareReadsAsString(s: string, types: number): boolean {
  if (isJsonNumber(s)) return !(types & NUMBER) && !!(types & STRING);
  if (s === "true" || s === "false") return !(types & BOOLEAN) && !!(types & STRING);
  return true;
}

export class Typer {
  private regexes = new Map<string, RegExp | null>();
  constructor(private readonly schema: unknown | undefined) {}

  get hasSchema(): boolean {
    return this.schema !== undefined;
  }

  root(): Pos {
    return this.schema === undefined ? TRUE : { k: "node", v: this.schema };
  }

  private pointer(ptr: string): unknown | undefined {
    let cur: unknown = this.schema;
    if (ptr === "") return cur;
    if (!ptr.startsWith("/")) return undefined;
    for (let part of ptr.slice(1).split("/")) {
      part = part.replace(/~1/g, "/").replace(/~0/g, "~");
      if (isObj(cur) && has(cur, part)) cur = cur[part];
      else if (Array.isArray(cur) && /^[0-9]+$/.test(part) && Number(part) < cur.length) cur = cur[Number(part)];
      else return undefined;
    }
    return cur;
  }

  private deref(p: Pos): Pos {
    if (p.k !== "node") return p;
    let node = p.v;
    for (let n = 0; n < 64; n++) {
      if (!isObj(node)) return { k: "node", v: node };
      const r = node["$ref"];
      if (typeof r !== "string" || !r.startsWith("#")) return { k: "node", v: node };
      const target = this.pointer(r.slice(1));
      if (target === undefined) return { k: "node", v: node };
      if (Object.keys(node).length > 1) return { k: "all", ps: [{ k: "node", v: target }, { k: "noref", v: node }] };
      node = target;
    }
    return { k: "node", v: node };
  }

  types(p: Pos): number {
    const d = this.deref(p);
    switch (d.k) {
      case "true":
        return ALL;
      case "all":
        return d.ps.reduce((t, x) => t & this.types(x), ALL);
      case "any":
        return d.ps.length ? d.ps.reduce((t, x) => t | this.types(x), 0) : ALL;
      default:
        return this.ownTypes(d.v);
    }
  }

  private ownTypes(v: unknown): number {
    if (!isObj(v)) return ALL;
    let out = ALL;
    const t = v["type"];
    if (typeof t === "string") out &= bitOfName(t);
    else if (Array.isArray(t)) out &= t.filter((x) => typeof x === "string").reduce((b: number, x) => b | bitOfName(x), 0);
    if (has(v, "const")) out &= bitOfValue(v["const"]);
    if (Array.isArray(v["enum"])) out &= (v["enum"] as unknown[]).reduce((b: number, x) => b | bitOfValue(x), 0);
    if (Array.isArray(v["allOf"])) for (const s of v["allOf"] as unknown[]) out &= this.types({ k: "node", v: s });
    for (const key of ["anyOf", "oneOf"]) {
      const subs = v[key];
      if (Array.isArray(subs) && subs.length) out &= subs.reduce((t: number, s) => t | this.types({ k: "node", v: s }), 0);
    }
    return out;
  }

  private children(p: Pos, pick: (m: Record<string, unknown>) => Pos): Pos[] {
    const d = this.deref(p);
    switch (d.k) {
      case "true":
        return [TRUE];
      case "all":
        return [TRUE, ...d.ps.flatMap((x) => this.children(x, pick))];
      case "any":
        return [TRUE, { k: "any", ps: d.ps.map((x) => ({ k: "all", ps: this.children(x, pick) }) as Pos) }];
      default: {
        const m = d.v;
        if (!isObj(m)) return [TRUE];
        const out: Pos[] = [pick(m)];
        if (Array.isArray(m["allOf"])) for (const s of m["allOf"] as unknown[]) out.push(...this.children({ k: "node", v: s }, pick));
        for (const key of ["anyOf", "oneOf"]) {
          const subs = m[key];
          if (Array.isArray(subs) && subs.length)
            out.push({ k: "any", ps: subs.map((s) => ({ k: "all", ps: this.children({ k: "node", v: s }, pick) }) as Pos) });
        }
        return out;
      }
    }
  }

  private search(pattern: string, s: string): boolean {
    if (!this.regexes.has(pattern)) {
      let re: RegExp | null = null;
      try {
        re = new RegExp(pattern, "u");
      } catch {
        re = null;
      }
      this.regexes.set(pattern, re);
    }
    const re = this.regexes.get(pattern);
    return re ? re.test(s) : false;
  }

  prop(p: Pos, key: string): Pos {
    if (!this.hasSchema) return TRUE;
    return {
      k: "all",
      ps: this.children(p, (m) => {
        const props = m["properties"];
        if (isObj(props) && has(props, key)) return { k: "node", v: props[key] };
        const pats = m["patternProperties"];
        if (isObj(pats)) {
          const hits = Object.entries(pats)
            .filter(([pat]) => this.search(pat, key))
            .map(([, s]) => ({ k: "node", v: s }) as Pos);
          if (hits.length) return { k: "all", ps: hits };
        }
        if (has(m, "additionalProperties")) return { k: "node", v: m["additionalProperties"] };
        return TRUE;
      }),
    };
  }

  item(p: Pos, index: number): Pos {
    if (!this.hasSchema) return TRUE;
    return {
      k: "all",
      ps: this.children(p, (m) => {
        const pre = m["prefixItems"];
        if (Array.isArray(pre) && index < pre.length) return { k: "node", v: pre[index] };
        if (has(m, "items") && !Array.isArray(m["items"])) return { k: "node", v: m["items"] };
        return TRUE;
      }),
    };
  }
}
