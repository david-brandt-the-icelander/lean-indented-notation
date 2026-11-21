// Runs the shared conformance suite in tests/conformance against this library.
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { LinError, jsonToLin, linToJson, schemaHash } from "../src/index.js";
const lin_error = (e: unknown) => e instanceof LinError;

const suite = join(process.cwd(), "..", "tests", "conformance");
const ls = (d: string, ext: string) =>
  readdirSync(join(suite, d)).filter((f) => f.endsWith(ext)).sort().map((f) => join(suite, d, f));
const read = (p: string) => readFileSync(p, "utf8");

test("writer output and round trips", () => {
  for (const p of ls("write", ".json")) {
    const lp = p.replace(/\.json$/, ".lin");
    const split = lp.endsWith(".split.lin");
    assert.equal(jsonToLin(read(p), { split }), read(lp), lp);
    assert.equal(linToJson(read(lp)), read(p), lp + " reads back");
  }
});

test("hand-written LIN reads as expected", () => {
  for (const p of ls("read", ".lin")) assert.equal(linToJson(read(p)), read(p.replace(/\.lin$/, ".json")), p);
});

test("invalid LIN is rejected", () => {
  for (const p of ls("invalid", ".lin")) assert.throws(() => linToJson(read(p)), lin_error, p);
});

test("schema typing", () => {
  const schemaText = read(ls("schemas", ".json")[0]);
  const schema = JSON.parse(schemaText);
  const hash = schemaHash(schema);
  assert.ok(ls("schemas", ".json")[0].includes(hash));
  for (const p of ls("schema", ".lin")) {
    const want = read(p.replace(/\.lin$/, ".json"));
    assert.equal(linToJson(read(p), { schema }), want, p);
    assert.throws(() => linToJson(read(p)), /names schema/);
    const split = p.endsWith(".split.lin");
    assert.equal(jsonToLin(want, { schema, schemaHash: hash, split, splitLimit: split ? 5 : 24 }), read(p), p + " (write)");
  }
});

test("published schemas hash to their names", () => {
  const dir = join(process.cwd(), "..", "schemas");
  for (const f of readdirSync(dir).filter((f) => f.startsWith("sha256-") && f.endsWith(".json"))) {
    assert.equal(schemaHash(JSON.parse(read(join(dir, f)))) + ".json", f);
  }
});
