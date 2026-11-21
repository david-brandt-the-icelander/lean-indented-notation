// The LIN reader.

import { idKind } from "./jcs.js";
import { LinError, Node, canonicalNumber, isIntegerText, isJsonNumber } from "./node.js";
import { Pos, Typer, bareReadsAsString } from "./schema.js";
import { badChar, bareKeyOk, decodeString, keyLenAt, segOk, segStop } from "./text.js";

export interface Header {
  version: number;
  schema: string | null;
  url: string | null;
}

export interface Parsed {
  header: Header;
  head: Node;
  data: Map<string, Node>; // name joined by "." -> raw value
}

const err = (m: string, line?: number) => new LinError(m, line);

export function readHeaderLine(line: string, no = 1): Header {
  const f = line.split(" ");
  if (f.length < 2 || f[0] !== "LIN") throw err(`the first line must be the header "LIN 1 [schema [url]]", found ${JSON.stringify(line)}`, no);
  if (f[1] !== "1") throw err(`unsupported LIN version ${JSON.stringify(f[1])}`, no);
  if (f.length > 4) throw err("too many fields in the header", no);
  const kind = f.length >= 3 ? idKind(f[2]) : null;
  if (f.length >= 3 && kind === null)
    throw err("the schema id must be a registered number (1 to 9 digits, no leading zero) or a 39-digit provisional id", no);
  if (f.length === 4 && kind !== "provisional") throw err("a url may follow only a provisional schema id", no);
  if (f.length === 4 && (!f[3].startsWith("https://") || f[3].length <= 8)) throw err("the schema url must start with https://", no);
  return { version: 1, schema: f[2] ?? null, url: f[3] ?? null };
}

function jsonStrAt(s: string, i: number, line: number): [string, number] {
  try {
    return decodeString(s, i);
  } catch (e) {
    throw err(`bad JSON string: ${(e as Error).message}`, line);
  }
}

function bareValue(s: string, line: number, cell: boolean): Node {
  if (s === "") throw err(cell ? 'empty cell; an empty string is written ""' : 'empty value; an empty string is written ""', line);
  if (s.startsWith(" ") || s.endsWith(" "))
    throw err(`value ${JSON.stringify(s)} starts or ends with a space; such a string is written as a JSON string`, line);
  if (badChar(s)) throw err("a control character in a bare value; write it as a JSON string", line);
  if (s === "~") return { t: "null" };
  if (s === "[]") return { t: "arr", v: [] };
  if (s === "{}") return { t: "obj", v: [] };
  if (s === "true" || s === "false" || isJsonNumber(s)) return { t: "bare", v: s };
  if (s.startsWith("@")) return { t: "ref", name: s.length > 1 ? parseName(s.slice(1), line) : null };
  return { t: "str", v: s };
}

function parseScalar(s: string, line: number): Node {
  if (s.startsWith('"')) {
    const [v, end] = jsonStrAt(s, 0, line);
    if (end !== s.length) throw err(`text after a JSON string: ${JSON.stringify(s.slice(end))}`, line);
    return { t: "str", v };
  }
  return bareValue(s, line, false);
}

export function parseName(s: string, line?: number): string[] {
  const segs = s.split(".");
  for (const p of segs) {
    if (!p || [...p].some(segStop)) throw err(`bad reference name ${JSON.stringify(s)}`, line);
  }
  return segs;
}

function splitCells(s: string, line: number): Node[] {
  const cells: Node[] = [];
  let i = 0;
  for (;;) {
    let j: number;
    if (s[i] === '"') {
      const [v, e] = jsonStrAt(s, i, line);
      cells.push({ t: "str", v });
      j = e;
    } else {
      j = s.indexOf("|", i);
      if (j < 0) j = s.length;
      cells.push(bareValue(s.slice(i, j), line, true));
    }
    if (j === s.length) return cells;
    if (s[j] !== "|") throw err(`expected | after a cell, found ${JSON.stringify(s.slice(j))}`, line);
    i = j + 1;
  }
}

function objFromCells(s: string, line: number): Node {
  const o: [string, Node][] = [];
  const keys = new Set<string>();
  let i = 0;
  for (;;) {
    let k: string;
    if (s[i] === '"') {
      [k, i] = jsonStrAt(s, i, line);
    } else {
      const n = keyLenAt(s, i);
      if (!n) throw err(`bad key in inline object: ${JSON.stringify(s.slice(i))}`, line);
      k = s.slice(i, i + n);
      i += n;
    }
    if (s[i] !== " ") throw err(`inline object field ${JSON.stringify(k)} needs one space and a value`, line);
    i++;
    let v: Node, j: number;
    if (s[i] === '"') {
      const [x, e] = jsonStrAt(s, i, line);
      v = { t: "str", v: x };
      j = e;
    } else {
      j = s.indexOf("|", i);
      if (j < 0) j = s.length;
      v = bareValue(s.slice(i, j), line, true);
    }
    if (keys.has(k)) throw err(`duplicate key ${JSON.stringify(k)}`, line);
    keys.add(k);
    o.push([k, v]);
    if (j === s.length) return { t: "obj", v: o };
    if (s[j] !== "|") throw err(`expected | after a field, found ${JSON.stringify(s.slice(j))}`, line);
    i = j + 1;
  }
}

function columns(spec: string, line: number): string[] {
  const cols = spec.split("|");
  for (const c of cols) if (!bareKeyOk(c)) throw err(`bad column name ${JSON.stringify(c)}: columns are bare keys`, line);
  if (new Set(cols).size !== cols.length) throw err("duplicate column name", line);
  return cols;
}

const indent = (l: string) => l.length - l.replace(/^ +/, "").length;
const blank = (l: string) => /^ *$/.test(l);

class Reader {
  i = 0;
  constructor(readonly l: string[], readonly base: number) {}
  no(): number {
    return this.base + this.i;
  }
  peek(): string | undefined {
    while (this.i < this.l.length && blank(this.l[this.i])) this.i++;
    return this.l[this.i];
  }
  block(ind: number): string {
    const p = " ".repeat(ind);
    const out: string[] = [];
    while (this.i < this.l.length) {
      const line = this.l[this.i];
      if (line === "") out.push("");
      else if (line.startsWith(p)) out.push(line.slice(p.length));
      else break;
      this.i++;
    }
    while (out.length && out[out.length - 1] === "") {
      out.pop();
      this.i--;
    }
    if (!out.length) throw err("empty multi-line value", this.no());
    return out.join("\n");
  }
  valueAfter(rest: string, ind: number, line: number): Node {
    if (rest === "|") return { t: "str", v: this.block(ind + 1) };
    return parseScalar(rest, line);
  }
  tableRows(cols: string[], ind: number, line: number): Node {
    const rows: Node[] = [];
    for (;;) {
      const nxt = this.peek();
      if (nxt === undefined || indent(nxt) !== ind) break;
      const cells = splitCells(nxt.slice(ind), this.no());
      if (cells.length !== cols.length) throw err(`row has ${cells.length} cells, the header has ${cols.length} columns`, this.no());
      this.i++;
      rows.push({ t: "obj", v: cols.map((c, n) => [c, cells[n]] as [string, Node]) });
    }
    if (!rows.length) throw err("table with no rows", line);
    return { t: "arr", v: rows };
  }
  obj(ind: number, first?: [string, number]): Node {
    const o: [string, Node][] = [];
    const keys = new Set<string>();
    for (;;) {
      let line: string, no: number;
      if (first) {
        [line, no] = first;
        first = undefined;
      } else {
        const raw = this.peek();
        if (raw === undefined) return { t: "obj", v: o };
        const n = indent(raw);
        if (n !== ind) {
          if (n > ind) throw err("unexpected indentation", this.no());
          return { t: "obj", v: o };
        }
        no = this.no();
        this.i++;
        line = raw.slice(ind);
      }
      let k: string, rest: string;
      if (line.startsWith('"')) {
        let j: number;
        [k, j] = jsonStrAt(line, 0, no);
        rest = line.slice(j);
      } else {
        const n = keyLenAt(line);
        if (!n) throw err(`expected a key, found ${JSON.stringify(line)}`, no);
        k = line.slice(0, n);
        if (badChar(k)) throw err("a control character in a bare key", no);
        rest = line.slice(n);
      }
      if (keys.has(k)) throw err(`duplicate key ${JSON.stringify(k)}`, no);
      let v: Node;
      if (rest === "") {
        const nxt = this.peek();
        if (nxt !== undefined && indent(nxt) === ind && (nxt[ind] === "-" || nxt[ind] === "+")) v = this.items(ind);
        else if (nxt !== undefined && indent(nxt) === ind + 1) v = this.obj(ind + 1);
        else throw err(`key ${JSON.stringify(k)} has no value and no children`, no);
      } else if (rest.startsWith("[] ")) v = { t: "arr", v: splitCells(rest.slice(3), no) };
      else if (rest.startsWith("{} ")) v = objFromCells(rest.slice(3), no);
      else if (rest.startsWith("[") && rest.endsWith("]") && rest.length > 2) v = this.tableRows(columns(rest.slice(1, -1), no), ind + 1, no);
      else if (rest.startsWith(" ")) v = this.valueAfter(rest.slice(1), ind, no);
      else throw err(`expected a space, [] , {} or a table header after key ${JSON.stringify(k)}`, no);
      keys.add(k);
      o.push([k, v]);
    }
  }
  items(ind: number): Node {
    const out: Node[] = [];
    for (;;) {
      const raw = this.peek();
      if (raw === undefined || indent(raw) !== ind) return { t: "arr", v: out };
      const body = raw.slice(ind);
      const no = this.no();
      if (body.startsWith("-[] ")) {
        this.i++;
        out.push({ t: "arr", v: splitCells(body.slice(4), no) });
      } else if (body.startsWith("-{} ")) {
        this.i++;
        out.push(objFromCells(body.slice(4), no));
      } else if (body === "-") {
        this.i++;
        const inner = this.items(ind + 1);
        if (inner.t === "arr" && !inner.v.length) throw err("a nested list marker with no items", no);
        out.push(inner);
      } else if (body.startsWith("- ")) {
        this.i++;
        out.push(this.valueAfter(body.slice(2), ind, no));
      } else if (body.startsWith("+ ")) {
        this.i++;
        out.push(this.obj(ind + 1, [body.slice(2), no]));
      } else if (body.startsWith("-") || body.startsWith("+")) throw err(`bad list item ${JSON.stringify(body)}`, no);
      else return { t: "arr", v: out };
    }
  }
}

/** Parse LIN text without typing or resolving references. */
export function parse(text: string): Parsed {
  if (text.startsWith("﻿")) throw err("a LIN file must not start with a byte-order mark", 1);
  const cr = text.indexOf("\r");
  if (cr >= 0) throw err("LIN uses LF line ends, not CR LF", text.slice(0, cr).split("\n").length);
  const lines = text.split("\n");
  if (lines.length && lines[lines.length - 1] === "") lines.pop();
  if (!lines.length) throw err("empty file");
  const header = readHeaderLine(lines[0], 1);
  let sep = lines.indexOf("---");
  if (sep < 0) sep = lines.length;
  const head = new Reader(lines.slice(1, sep), 2);
  const f = head.peek();
  let doc: Node;
  if (f === undefined) doc = { t: "obj", v: [] };
  else if (f === "[]") {
    head.i++;
    doc = { t: "arr", v: [] };
  } else if (f.startsWith("[] ")) {
    const no = head.no();
    head.i++;
    doc = { t: "arr", v: splitCells(f.slice(3), no) };
  } else if (f.startsWith("[")) {
    const no = head.no();
    if (!f.endsWith("]") || f.length < 3) throw err(`bad root table header ${JSON.stringify(f)}`, no);
    const cols = columns(f.slice(1, -1), no);
    head.i++;
    doc = head.tableRows(cols, 1, no);
  } else if (f.startsWith("-") || f.startsWith("+")) doc = head.items(0);
  else {
    if (indent(f) > 0) throw err("unexpected indentation", head.no());
    doc = head.obj(0);
  }
  const left = head.peek();
  if (left !== undefined) throw err(`unexpected line ${JSON.stringify(left)}`, head.no());
  const data = new Map<string, Node>();
  const r = new Reader(sep < lines.length ? lines.slice(sep + 1) : [], sep + 2);
  for (let line = r.peek(); line !== undefined; line = r.peek()) {
    const no = r.no();
    if (!line.startsWith("@")) throw err(`a data line must start with @name, found ${JSON.stringify(line)}`, no);
    r.i++;
    const sp = line.indexOf(" ");
    const name = parseName(line.slice(1, sp < 0 ? line.length : sp), no).join(".");
    if (data.has(name)) throw err(`duplicate data entry @${name}`, no);
    if (sp < 0) {
      const items = r.items(1);
      if (items.t === "arr" && !items.v.length) throw err(`data entry @${name} has no value`, no);
      data.set(name, items);
    } else data.set(name, r.valueAfter(line.slice(sp + 1), 0, no));
  }
  return { header, head: doc, data };
}

function idSegment(n: Node): string | null {
  let s: string;
  if (n.t === "str") s = n.v;
  else if (n.t === "num" && isIntegerText(n.v)) s = n.v;
  else return null;
  return segOk(s) ? s : null;
}

/** Type the head by the schema and put each data entry in place of its @. */
export function resolve(parsed: Parsed, schema?: unknown): Node {
  const typer = new Typer(schema);
  const used = new Set<string>();
  const scalar = (n: Node, pos: Pos): Node => {
    if (n.t !== "bare") return n;
    if (bareReadsAsString(n.v, typer.types(pos))) return { t: "str", v: n.v };
    if (n.v === "true" || n.v === "false") return { t: "bool", v: n.v === "true" };
    return { t: "num", v: canonicalNumber(n.v) };
  };
  const plain = (n: Node, pos: Pos): Node => {
    if (n.t === "arr") return { t: "arr", v: n.v.map((x, i) => plain(x, typer.item(pos, i))) };
    if (n.t === "obj") return { t: "obj", v: n.v.map(([k, x]) => [k, plain(x, typer.prop(pos, k))] as [string, Node]) };
    if (n.t === "ref") throw err("a data entry cannot hold a reference");
    return scalar(n, pos);
  };
  const walk = (n: Node, pos: Pos, owner: string[], path: string[]): Node => {
    if (n.t === "obj") {
      let ow = owner, pa = path;
      const id = n.v.find(([k]) => k === "id");
      if (id && id[1].t !== "ref" && id[1].t !== "arr" && id[1].t !== "obj") {
        const seg = idSegment(scalar(id[1], typer.prop(pos, "id")));
        if (seg !== null) {
          ow = [seg];
          pa = [];
        }
      }
      return { t: "obj", v: n.v.map(([k, x]) => [k, walk(x, typer.prop(pos, k), ow, [...pa, k])] as [string, Node]) };
    }
    if (n.t === "arr") return { t: "arr", v: n.v.map((x, i) => walk(x, typer.item(pos, i), owner, [...path, String(i)])) };
    if (n.t === "ref") {
      const name = (n.name ?? [...owner, ...path]).join(".");
      if (used.has(name)) throw err(`data entry @${name} is referenced twice`);
      const raw = parsed.data.get(name);
      if (raw === undefined) throw err(`no data entry for @${name}`);
      used.add(name);
      return plain(raw, pos);
    }
    return scalar(n, pos);
  };
  const out = walk(parsed.head, typer.root(), [], []);
  if (used.size !== parsed.data.size) {
    const extra = [...parsed.data.keys()].filter((k) => !used.has(k)).sort().slice(0, 3);
    throw err(`data entries with no @ in the head: ${extra.map((x) => "@" + x).join(", ")}`);
  }
  return out;
}
