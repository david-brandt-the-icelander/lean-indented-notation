import assert from "node:assert/strict";
import { test } from "node:test";
import * as lin from "../src/index.js";

test("parse and stringify", () => {
  const v = { name: "demo", tags: ["a", "b"], size: { w: 1, h: 2 }, files: [{ p: "a", n: 1 }, { p: "b", n: 2 }] };
  const text = lin.stringify(v);
  assert.equal(text, "LIN 1\nname demo\ntags[] a|b\nsize\n w 1\n h 2\nfiles[p|n]\n a|1\n b|2\n");
  assert.deepEqual(lin.parse(text), v);
});

test("big integers come back as bigint", () => {
  const v = lin.parse("LIN 1\nn 123456789012345678901234567890\nm 12\n");
  assert.equal(v.n, 123456789012345678901234567890n);
  assert.equal(v.m, 12);
  assert.equal(lin.stringify(v), "LIN 1\nn 123456789012345678901234567890\nm 12\n");
});

test("text conversion keeps key order that objects cannot", () => {
  const json = '{"b": 1, "200": 2}';
  assert.equal(lin.jsonToLin(json), "LIN 1\nb 1\n200 2\n");
  assert.equal(lin.linToJson("LIN 1\nb 1\n200 2\n", { compact: true }), '{"b":1,"200":2}');
});

test("errors carry line numbers", () => {
  try {
    lin.parse("LIN 1\na 1\na 2\n");
    assert.fail("accepted");
  } catch (e) {
    assert.ok(e instanceof lin.LinError);
    assert.equal((e as lin.LinError).line, 3);
  }
});

test("a schema'd file needs its schema", () => {
  const schema = { type: "object", properties: { v: { type: "string" } } };
  const h = lin.schemaHash(schema);
  const text = lin.stringify({ v: "1.0" }, { schema, schemaHash: h });
  assert.equal(text, `LIN 1 ${h}\nv 1.0\n`);
  assert.throws(() => lin.parse(text), lin.SchemaRequiredError);
  assert.deepEqual(lin.parse(text, { schema }), { v: "1.0" });
  assert.deepEqual(lin.parse(text, { ignoreSchema: true }), { v: 1 });
  assert.throws(() => lin.parse(text, { schema: { type: "object" } }), /hash mismatch/);
});
