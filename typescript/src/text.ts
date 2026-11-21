// Character rules and JSON text, shared by the reader and writer.

import { LinError, Node, canonicalNumber, isJsonNumber } from "./node.js";

/** Whitespace as Python's str.isspace() defines it; every implementation uses this set. */
export function isWs(c: string): boolean {
  const x = c.codePointAt(0)!;
  return (
    (x >= 0x09 && x <= 0x0d) ||
    (x >= 0x1c && x <= 0x20) ||
    x === 0x85 ||
    x === 0xa0 ||
    x === 0x1680 ||
    (x >= 0x2000 && x <= 0x200a) ||
    x === 0x2028 ||
    x === 0x2029 ||
    x === 0x202f ||
    x === 0x205f ||
    x === 0x3000
  );
}

/** Controls, DEL, NEL and the Unicode line separators: never in a bare string. */
export function isBad(c: string): boolean {
  const x = c.codePointAt(0)!;
  return x < 0x20 || x === 0x7f || x === 0x85 || x === 0x2028 || x === 0x2029;
}

export function badChar(s: string): boolean {
  for (const c of s) if (isBad(c)) return true;
  return false;
}

function keyChar(c: string): boolean {
  return !isWs(c) && !'"[]{}|'.includes(c);
}

/** The length (UTF-16 units) of the bare key at s[i...], 0 if none. */
export function keyLenAt(s: string, i = 0): number {
  let j = i;
  let first = true;
  for (const c of s.slice(i)) {
    if (!keyChar(c) || (first && "+#-".includes(c))) break;
    first = false;
    j += c.length;
  }
  return j - i;
}

export function bareKeyOk(k: string): boolean {
  return k.length > 0 && keyLenAt(k) === k.length && !badChar(k);
}

export function segStop(c: string): boolean {
  return c === "." || c === '"' || c === "|" || c === "@" || isWs(c);
}

export function segOk(s: string): boolean {
  if (!s) return false;
  for (const c of s) if (segStop(c) || isBad(c)) return false;
  return true;
}

/** Length in characters (Unicode code points), as every limit counts it. */
export function clen(s: string): number {
  let n = 0;
  for (const _ of s) n++;
  return n;
}

/** A JSON string as JSON.stringify writes it. */
export function quote(s: string): string {
  let out = '"';
  for (const c of s) {
    switch (c) {
      case '"':
        out += '\\"';
        break;
      case "\\":
        out += "\\\\";
        break;
      case "\n":
        out += "\\n";
        break;
      case "\r":
        out += "\\r";
        break;
      case "\t":
        out += "\\t";
        break;
      case "\b":
        out += "\\b";
        break;
      case "\f":
        out += "\\f";
        break;
      default: {
        const x = c.codePointAt(0)!;
        out += x < 0x20 ? "\\u" + x.toString(16).padStart(4, "0") : c;
      }
    }
  }
  return out + '"';
}

/** Decode the JSON string starting at s[start] (a quote): [value, index after the closing quote]. */
export function decodeString(s: string, start: number): [string, number] {
  if (s[start] !== '"') throw new Error("expected a JSON string");
  let out = "";
  let i = start + 1;
  for (;;) {
    if (i >= s.length) throw new Error("unterminated string");
    const c = s[i];
    if (c === '"') return [out, i + 1];
    if (c === "\\") {
      const e = s[i + 1];
      i += 2;
      switch (e) {
        case '"':
          out += '"';
          break;
        case "\\":
          out += "\\";
          break;
        case "/":
          out += "/";
          break;
        case "b":
          out += "\b";
          break;
        case "f":
          out += "\f";
          break;
        case "n":
          out += "\n";
          break;
        case "r":
          out += "\r";
          break;
        case "t":
          out += "\t";
          break;
        case "u": {
          const hi = hex4(s, i);
          i += 4;
          if (hi >= 0xd800 && hi < 0xdc00) {
            if (s[i] === "\\" && s[i + 1] === "u") {
              const lo = hex4(s, i + 2);
              if (lo >= 0xdc00 && lo < 0xe000) {
                out += String.fromCharCode(hi, lo);
                i += 6;
                break;
              }
            }
            throw new Error("lone surrogate in \\u escape");
          }
          if (hi >= 0xdc00 && hi < 0xe000) throw new Error("lone surrogate in \\u escape");
          out += String.fromCharCode(hi);
          break;
        }
        default:
          throw new Error(`invalid escape \\${e ?? ""}`);
      }
      continue;
    }
    if (c.charCodeAt(0) < 0x20) throw new Error("control character in string");
    out += c;
    i++;
  }
}

function hex4(s: string, i: number): number {
  const h = s.slice(i, i + 4);
  if (!/^[0-9a-fA-F]{4}$/.test(h)) throw new Error("bad \\u escape");
  return parseInt(h, 16);
}

// ---------------------------------------------------------------- JSON

/** Parse JSON text strictly (duplicate keys are errors) into a node. */
export function parseJson(text: string): Node {
  let i = 0;
  const ws = () => {
    while (i < text.length && " \t\n\r".includes(text[i])) i++;
  };
  const fail = (m: string): never => {
    const before = text.slice(0, i);
    const line = before.split("\n").length;
    throw new LinError(`bad JSON: ${m} at line ${line}`);
  };
  const value = (depth: number): Node => {
    if (depth > 10000) fail("nested too deeply");
    const c = text[i];
    if (c === "{") {
      i++;
      const out: [string, Node][] = [];
      const seen = new Set<string>();
      ws();
      if (text[i] === "}") {
        i++;
        return { t: "obj", v: out };
      }
      for (;;) {
        ws();
        if (text[i] !== '"') fail("expected a string key");
        let k: string;
        try {
          [k, i] = decodeString(text, i);
        } catch (e) {
          return fail((e as Error).message);
        }
        ws();
        if (text[i] !== ":") fail("expected ':'");
        i++;
        ws();
        const v = value(depth + 1);
        if (seen.has(k)) throw new LinError(`duplicate key ${JSON.stringify(k)} in JSON`);
        seen.add(k);
        out.push([k, v]);
        ws();
        if (text[i] === ",") i++;
        else if (text[i] === "}") {
          i++;
          return { t: "obj", v: out };
        } else fail("expected ',' or '}'");
      }
    }
    if (c === "[") {
      i++;
      const out: Node[] = [];
      ws();
      if (text[i] === "]") {
        i++;
        return { t: "arr", v: out };
      }
      for (;;) {
        ws();
        out.push(value(depth + 1));
        ws();
        if (text[i] === ",") i++;
        else if (text[i] === "]") {
          i++;
          return { t: "arr", v: out };
        } else fail("expected ',' or ']'");
      }
    }
    if (c === '"') {
      try {
        const [s, e] = decodeString(text, i);
        i = e;
        return { t: "str", v: s };
      } catch (e) {
        return fail((e as Error).message);
      }
    }
    for (const [w, n] of [
      ["true", { t: "bool", v: true }],
      ["false", { t: "bool", v: false }],
      ["null", { t: "null" }],
    ] as [string, Node][]) {
      if (text.startsWith(w, i)) {
        i += w.length;
        return n;
      }
    }
    if (c === "-" || (c >= "0" && c <= "9")) {
      let j = i;
      while (j < text.length && "-+.eE0123456789".includes(text[j])) j++;
      const num = text.slice(i, j);
      if (!isJsonNumber(num)) fail(`not a JSON number: ${num}`);
      i = j;
      return { t: "num", v: canonicalNumber(num) };
    }
    return fail(i >= text.length ? "unexpected end of input" : "unexpected character");
  };
  ws();
  const v = value(0);
  ws();
  if (i !== text.length) fail("text after the JSON value");
  return v;
}

function scalarJson(n: Node): string {
  switch (n.t) {
    case "null":
      return "null";
    case "bool":
      return n.v ? "true" : "false";
    case "num":
      return n.v;
    case "str":
      return quote(n.v);
    default:
      throw new LinError("not a scalar");
  }
}

/** Compact JSON on one line. */
export function jsonCompact(n: Node): string {
  if (n.t === "obj") return "{" + n.v.map(([k, x]) => quote(k) + ":" + jsonCompact(x)).join(",") + "}";
  if (n.t === "arr") return "[" + n.v.map(jsonCompact).join(",") + "]";
  return scalarJson(n);
}

/** Indented JSON with a final newline: the layout of JSON.stringify(v, null, 2). */
export function jsonPretty(n: Node): string {
  const out: string[] = [];
  const walk = (x: Node, level: number) => {
    const pad = (l: number) => "  ".repeat(l);
    if (x.t === "obj" && x.v.length) {
      out.push("{");
      x.v.forEach(([k, y], i) => {
        out.push((i ? ",\n" : "\n") + pad(level + 1) + quote(k) + ": ");
        walk(y, level + 1);
      });
      out.push("\n" + pad(level) + "}");
    } else if (x.t === "arr" && x.v.length) {
      out.push("[");
      x.v.forEach((y, i) => {
        out.push((i ? ",\n" : "\n") + pad(level + 1));
        walk(y, level + 1);
      });
      out.push("\n" + pad(level) + "]");
    } else if (x.t === "obj") out.push("{}");
    else if (x.t === "arr") out.push("[]");
    else out.push(scalarJson(x));
  };
  walk(n, 0);
  return out.join("") + "\n";
}

export { isJsonNumber };
