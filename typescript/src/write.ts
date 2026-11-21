// The LIN writer. Its output is byte-for-byte the same as the Python and Rust writers'.

import { idKind } from "./jcs.js";
import { LinError, Node, isIntegerText, isJsonNumber } from "./node.js";
import { Pos, Typer, bareReadsAsString } from "./schema.js";
import { badChar, bareKeyOk, clen, quote, segOk } from "./text.js";

export interface WriteOptions {
  /** Write the split form: long values move to a data section after `---`. */
  split?: boolean;
  /** The header's schema id: a registered number, or a 39-digit provisional id. */
  schemaId?: string;
  /** An https:// location of the schema, written after its hash. */
  url?: string;
  /** The schema itself: strings that look like numbers go bare where it says they are strings. */
  schema?: unknown;
  /** Strings longer than this move to the data section in split form (default 24). */
  splitLimit?: number;
  /** Lists of scalars go inline unless an item is longer than this (default: always inline). */
  inlineLimit?: number;
  /** Objects of scalars go inline when their fields fit in this many characters (default 80; 0 never). */
  objectLimit?: number;
  /** Write the `LIN 1` header (default true). */
  header?: boolean;
}

const HEADLINE = new Set(["id", "kind", "title", "name", "status", "type"]);

const isContainer = (n: Node) => n.t === "arr" || n.t === "obj";

function mark(n: Node, t: Typer, pos: Pos): Node {
  switch (n.t) {
    case "arr":
      return { t: "arr", v: n.v.map((x, i) => mark(x, t, t.item(pos, i))) };
    case "obj":
      return { t: "obj", v: n.v.map(([k, x]) => [k, mark(x, t, t.prop(pos, k))] as [string, Node]) };
    case "str": {
      const looks = isJsonNumber(n.v) || n.v === "true" || n.v === "false";
      return { t: "str", v: n.v, bareOk: looks && t.hasSchema && bareReadsAsString(n.v, t.types(pos)) };
    }
    default:
      return n;
  }
}

export function quoteKey(k: string): string {
  return bareKeyOk(k) ? k : quote(k);
}

export function blockOk(s: string): boolean {
  const lines = s.split("\n");
  return !s.endsWith("\n") && lines[0] !== "" && lines.every((l) => !l.endsWith(" ") && !badChar(l));
}

/** The one-line form of a string, or null when it goes in a block. */
export function stringForm(s: string, cell: boolean, bareOk = false): string | null {
  if (s.includes("\n") && !cell && blockOk(s)) return null;
  const looks = (isJsonNumber(s) || s === "true" || s === "false") && !bareOk;
  const need =
    s === "" ||
    s[0] === " " ||
    s[0] === '"' ||
    s[0] === "@" ||
    s.endsWith(" ") ||
    s === "~" ||
    s === "[]" ||
    s === "{}" ||
    s === "|" ||
    badChar(s) ||
    looks ||
    (cell && s.includes("|"));
  return need ? quote(s) : s;
}

class Writer {
  constructor(
    readonly splitLimit: number,
    readonly inlineLimit: number | undefined,
    readonly objectLimit: number,
  ) {}

  scalar(n: Node, cell: boolean): string | null {
    switch (n.t) {
      case "ref":
        return n.name === null ? "@" : "@" + n.name.join(".");
      case "null":
        return "~";
      case "bool":
        return n.v ? "true" : "false";
      case "num":
        return n.v;
      case "str":
        return stringForm(n.v, cell, n.bareOk);
      default:
        throw new LinError("containers have no scalar form");
    }
  }
  cell(n: Node): string {
    return this.scalar(n, true)!;
  }
  cells(ns: Node[]): string {
    return ns.map((x) => this.cell(x)).join("|");
  }
  inlineOk(l: Node[]): boolean {
    return (
      l.length > 0 &&
      l.every((x) => {
        if (x.t === "arr" || x.t === "obj" || x.t === "ref") return false;
        if (x.t === "str" && this.inlineLimit !== undefined) return clen(x.v) <= this.inlineLimit;
        return true;
      })
    );
  }
  objCells(o: [string, Node][]): string {
    return o.map(([k, v]) => `${k} ${this.cell(v)}`).join("|");
  }
  inlineObjOk(o: [string, Node][], ind: number, item: boolean): boolean {
    if (!this.objectLimit || ind < 1 || o.length < (item ? 2 : 1)) return false;
    if (!o.every(([k, v]) => !isContainer(v) && bareKeyOk(k))) return false;
    return clen(this.objCells(o)) <= this.objectLimit;
  }
  static tableCols(l: Node[]): string[] | null {
    if (l.length < 2) return null;
    const first = l[0];
    if (first.t !== "obj" || !first.v.length) return null;
    const keys = first.v.map(([k]) => k);
    if (!keys.every(bareKeyOk)) return null;
    for (const x of l) {
      if (x.t !== "obj" || x.v.length !== keys.length) return null;
      if (x.v.some(([k, v], i) => k !== keys[i] || isContainer(v))) return null;
    }
    return keys;
  }
  emit(o: [string, Node][], ind: number, out: string[]): void {
    const p = " ".repeat(ind);
    for (const [k, v] of o) {
      const kk = quoteKey(k);
      if (v.t === "obj") {
        if (!v.v.length) out.push(`${p}${kk} {}`);
        else if (this.inlineObjOk(v.v, ind, false)) out.push(`${p}${kk}{} ${this.objCells(v.v)}`);
        else {
          out.push(p + kk);
          this.emit(v.v, ind + 1, out);
        }
      } else if (v.t === "arr") {
        const cols = Writer.tableCols(v.v);
        if (!v.v.length) out.push(`${p}${kk} []`);
        else if (this.inlineOk(v.v)) out.push(`${p}${kk}[] ${this.cells(v.v)}`);
        else if (cols) {
          out.push(`${p}${kk}[${cols.join("|")}]`);
          for (const row of v.v) out.push(`${p} ${this.cells((row as { v: [string, Node][] }).v.map(([, x]) => x))}`);
        } else {
          out.push(p + kk);
          this.emitItems(v.v, ind, out);
        }
      } else {
        const s = this.scalar(v, false);
        if (s === null) {
          out.push(`${p}${kk} |`);
          blockLines((v as { v: string }).v, ind + 1, out);
        } else out.push(`${p}${kk} ${s}`);
      }
    }
  }
  emitItems(l: Node[], ind: number, out: string[]): void {
    const p = " ".repeat(ind);
    for (const x of l) {
      if (x.t === "obj") {
        if (!x.v.length) out.push(p + "- {}");
        else if (this.inlineObjOk(x.v, ind, true)) out.push(`${p}-{} ${this.objCells(x.v)}`);
        else {
          const sub: string[] = [];
          this.emit(x.v, ind + 1, sub);
          out.push(p + "+ " + sub[0].slice(ind + 1));
          out.push(...sub.slice(1));
        }
      } else if (x.t === "arr") {
        if (!x.v.length) out.push(p + "- []");
        else if (this.inlineOk(x.v)) out.push(`${p}-[] ${this.cells(x.v)}`);
        else {
          out.push(p + "-");
          this.emitItems(x.v, ind + 1, out);
        }
      } else {
        const s = this.scalar(x, false);
        if (s === null) {
          out.push(p + "- |");
          blockLines((x as { v: string }).v, ind + 1, out);
        } else out.push(p + "- " + s);
      }
    }
  }
  body(doc: Node, out: string[]): void {
    if (doc.t === "obj") return this.emit(doc.v, 0, out);
    if (doc.t !== "arr") throw new LinError("the root must be an object or an array");
    if (!doc.v.length) out.push("[]");
    else if (this.inlineOk(doc.v)) out.push("[] " + this.cells(doc.v));
    else {
      const cols = Writer.tableCols(doc.v);
      if (cols) {
        out.push(`[${cols.join("|")}]`);
        for (const row of doc.v) out.push(" " + this.cells((row as { v: [string, Node][] }).v.map(([, x]) => x)));
      } else this.emitItems(doc.v, 0, out);
    }
  }
  dataLines(data: [string[], Node][], out: string[]): void {
    for (const [name, v] of data) {
      const n = "@" + name.join(".");
      if (v.t === "arr") {
        out.push(n);
        this.emitItems(v.v, 1, out);
      } else {
        const s = this.scalar(v, false);
        if (s === null) {
          out.push(n + " |");
          blockLines((v as { v: string }).v, 1, out);
        } else out.push(`${n} ${s}`);
      }
    }
  }
  split(doc: Node): [Node, [string[], Node][]] {
    const long = (n: Node) => n.t === "str" && clen(n.v) > this.splitLimit;
    const mv = (n: Node, key: string | null, root: boolean): Node => {
      if (n.t === "obj") return { t: "obj", v: n.v.map(([k, v]) => [k, mv(v, k, false)] as [string, Node]) };
      if (n.t === "arr") {
        if (!root && n.v.length && n.v.every((x) => !isContainer(x)) && n.v.some(long)) return { t: "ref", name: null, value: n };
        return { t: "arr", v: n.v.map((x) => mv(x, null, false)) };
      }
      if (long(n) && !(key !== null && HEADLINE.has(key))) return { t: "ref", name: null, value: n };
      return n;
    };
    const head = mv(doc, null, true);
    // every reference with its container, slot and derived name, in document order
    const refs: [Node[] | [string, Node][], number, string[]][] = [];
    const idSeg = (n: Node): string | null => {
      let s: string;
      if (n.t === "str") s = n.v;
      else if (n.t === "num" && isIntegerText(n.v)) s = n.v;
      else return null;
      return segOk(s) ? s : null;
    };
    const collect = (n: Node, owner: string[], path: string[]) => {
      if (n.t === "obj") {
        let ow = owner, pa = path;
        const id = n.v.find(([k]) => k === "id");
        const seg = id ? idSeg(id[1]) : null;
        if (seg !== null) {
          ow = [seg];
          pa = [];
        }
        n.v.forEach(([k, v], i) => {
          if (v.t === "ref") refs.push([n.v, i, [...ow, ...pa, k]]);
          collect(v, ow, [...pa, k]);
        });
      } else if (n.t === "arr") {
        n.v.forEach((v, i) => {
          if (v.t === "ref") refs.push([n.v, i, [...owner, ...path, String(i)]]);
          collect(v, owner, [...path, String(i)]);
        });
      }
    };
    collect(head, [], []);
    const derived = new Map<string, number>();
    for (const [, , n] of refs) derived.set(n.join("\u0000"), (derived.get(n.join("\u0000")) ?? 0) + 1);
    const used = new Set<string>();
    const data: [string[], Node][] = [];
    const get = (c: Node[] | [string, Node][], i: number): Node => (Array.isArray(c[i]) ? (c[i] as [string, Node])[1] : (c[i] as Node));
    const put = (c: Node[] | [string, Node][], i: number, v: Node) => {
      if (Array.isArray(c[i])) (c[i] as [string, Node])[1] = v;
      else (c as Node[])[i] = v;
    };
    for (const [c, i, n] of refs) {
      const ref = get(c, i) as { t: "ref"; value: Node };
      const value = ref.value;
      if (!n.every(segOk)) {
        put(c, i, value);
        continue;
      }
      const key = n.join("\u0000");
      let name: string[];
      if (!used.has(key) && derived.get(key) === 1) {
        name = n;
        put(c, i, { t: "ref", name: null, value });
      } else {
        let k = 2;
        for (;;) {
          name = [...n.slice(0, -1), `${n[n.length - 1]}~${k}`];
          const nk = name.join("\u0000");
          if (!derived.has(nk) && !used.has(nk)) break;
          k++;
        }
        put(c, i, { t: "ref", name, value });
      }
      used.add(name.join("\u0000"));
      data.push([name, value]);
    }
    return [head, data];
  }
}

function blockLines(s: string, ind: number, out: string[]): void {
  const p = " ".repeat(ind);
  for (const l of s.split("\n")) out.push(l ? p + l : "");
}

export function headerLine(schema?: string, url?: string): string {
  let h = "LIN 1";
  if (schema) {
    const kind = idKind(schema);
    if (kind === null) throw new LinError("a schema id is a registered number (1 to 9 digits) or a 39-digit provisional id");
    if (url && kind !== "provisional") throw new LinError("a url may follow only a provisional schema id");
    h += " " + schema;
    if (url) {
      if (!url.startsWith("https://") || url.includes(" ")) throw new LinError("schema url must start with https:// and have no spaces");
      h += " " + url;
    }
  } else if (url) throw new LinError("a url needs a schema id before it");
  return h;
}

/** Write a node (object or array) as LIN. */
export function writeNode(value: Node, o: WriteOptions = {}): string {
  if (!isContainer(value)) throw new LinError("the root must be an object or an array");
  const typer = new Typer(o.schema);
  let doc = o.schema !== undefined ? mark(value, typer, typer.root()) : value;
  const w = new Writer(o.splitLimit ?? 24, o.inlineLimit, o.objectLimit ?? 80);
  let data: [string[], Node][] = [];
  if (o.split) [doc, data] = w.split(structuredCopy(doc));
  const lines: string[] = [];
  if (o.header !== false) lines.push(headerLine(o.schemaId, o.url));
  w.body(doc, lines);
  if (data.length) {
    lines.push("---");
    w.dataLines(data, lines);
  }
  return lines.join("\n") + "\n";
}

function structuredCopy(n: Node): Node {
  if (n.t === "arr") return { t: "arr", v: n.v.map(structuredCopy) };
  if (n.t === "obj") return { t: "obj", v: n.v.map(([k, x]) => [k, structuredCopy(x)] as [string, Node]) };
  return n;
}
