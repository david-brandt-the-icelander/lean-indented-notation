// RFC 8785 JSON Canonicalization and LIN schema hashes (spec section 7).

import { Node, fromJs } from "./node.js";
import { quote } from "./text.js";

/** The JCS text of a value (a node or a plain JavaScript value). */
export function canonicalize(value: unknown): string {
  return canon(isNode(value) ? value : fromJs(value));
}

function isNode(v: unknown): v is Node {
  return typeof v === "object" && v !== null && "t" in v && typeof (v as { t: unknown }).t === "string" && !Array.isArray(v);
}

function canon(n: Node): string {
  switch (n.t) {
    case "null":
      return "null";
    case "bool":
      return n.v ? "true" : "false";
    case "num": {
      const f = Number(n.v);
      return f === 0 ? "0" : String(f);
    }
    case "str":
      return quote(n.v);
    case "arr":
      return "[" + n.v.map(canon).join(",") + "]";
    case "obj": {
      const entries = [...n.v].sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
      return "{" + entries.map(([k, x]) => quote(k) + ":" + canon(x)).join(",") + "}";
    }
    default:
      throw new Error("not a JSON value");
  }
}

/** sha256- and the base64url SHA-256 of the schema's canonical JSON. */
export function schemaHash(schema: unknown): string {
  return "sha256-" + base64url(sha256(utf8(canonicalize(schema))));
}

export function isHash(s: string): boolean {
  return /^sha256-[A-Za-z0-9_-]{43}$/.test(s);
}

function utf8(s: string): Uint8Array {
  const out: number[] = [];
  for (const c of s) {
    let x = c.codePointAt(0)!;
    if (x < 0x80) out.push(x);
    else if (x < 0x800) out.push(0xc0 | (x >> 6), 0x80 | (x & 63));
    else if (x < 0x10000) out.push(0xe0 | (x >> 12), 0x80 | ((x >> 6) & 63), 0x80 | (x & 63));
    else out.push(0xf0 | (x >> 18), 0x80 | ((x >> 12) & 63), 0x80 | ((x >> 6) & 63), 0x80 | (x & 63));
  }
  return new Uint8Array(out);
}

function base64url(b: Uint8Array): string {
  const A = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  let out = "";
  for (let i = 0; i < b.length; i += 3) {
    const n = (b[i] << 16) | ((b[i + 1] ?? 0) << 8) | (b[i + 2] ?? 0);
    out += A[(n >> 18) & 63] + A[(n >> 12) & 63];
    if (i + 1 < b.length) out += A[(n >> 6) & 63];
    if (i + 2 < b.length) out += A[n & 63];
  }
  return out;
}

const K = new Uint32Array([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be,
  0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa,
  0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85,
  0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
  0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f,
  0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);

/** SHA-256 (FIPS 180-4), small and dependency-free so the library runs anywhere. */
export function sha256(msg: Uint8Array): Uint8Array {
  const len = msg.length;
  const total = Math.ceil((len + 9) / 64) * 64;
  const m = new Uint8Array(total);
  m.set(msg);
  m[len] = 0x80;
  const bits = len * 8;
  const dv = new DataView(m.buffer);
  dv.setUint32(total - 4, bits >>> 0);
  dv.setUint32(total - 8, Math.floor(bits / 0x100000000));
  const h = new Uint32Array([0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19]);
  const w = new Uint32Array(64);
  const r = (x: number, n: number) => (x >>> n) | (x << (32 - n));
  for (let o = 0; o < total; o += 64) {
    for (let i = 0; i < 16; i++) w[i] = dv.getUint32(o + i * 4);
    for (let i = 16; i < 64; i++) {
      const s0 = r(w[i - 15], 7) ^ r(w[i - 15], 18) ^ (w[i - 15] >>> 3);
      const s1 = r(w[i - 2], 17) ^ r(w[i - 2], 19) ^ (w[i - 2] >>> 10);
      w[i] = (w[i - 16] + s0 + w[i - 7] + s1) >>> 0;
    }
    let [a, b, c, d, e, f, g, hh] = h;
    for (let i = 0; i < 64; i++) {
      const t1 = (hh + (r(e, 6) ^ r(e, 11) ^ r(e, 25)) + ((e & f) ^ (~e & g)) + K[i] + w[i]) >>> 0;
      const t2 = ((r(a, 2) ^ r(a, 13) ^ r(a, 22)) + ((a & b) ^ (a & c) ^ (b & c))) >>> 0;
      hh = g;
      g = f;
      f = e;
      e = (d + t1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (t1 + t2) >>> 0;
    }
    h[0] += a;
    h[1] += b;
    h[2] += c;
    h[3] += d;
    h[4] += e;
    h[5] += f;
    h[6] += g;
    h[7] += hh;
  }
  const out = new Uint8Array(32);
  const odv = new DataView(out.buffer);
  for (let i = 0; i < 8; i++) odv.setUint32(i * 4, h[i]);
  return out;
}
