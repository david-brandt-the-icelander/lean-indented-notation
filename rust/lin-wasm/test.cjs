// node rust/lin-wasm/test.cjs — runs the shared conformance suite against the WebAssembly build.
const fs = require("fs");
const path = require("path");
const lin = require("./pkg-node/lin_wasm.js");
const suite = path.join(__dirname, "../../tests/conformance");
const ls = (d, ext) => fs.readdirSync(path.join(suite, d)).filter((f) => f.endsWith(ext)).sort().map((f) => path.join(suite, d, f));
const read = (p) => fs.readFileSync(p, "utf8");
let n = 0;
function eq(a, b, what) {
  if (a !== b) { console.error("FAIL", what, "\n--- got\n" + a + "\n--- want\n" + b); process.exit(1); }
  n++;
}
for (const p of ls("write", ".json")) {
  const src = read(p);
  const lp = p.replace(/\.json$/, ".lin");
  const o = new lin.Options();
  o.split = lp.endsWith(".split.lin");
  eq(lin.jsonToLin(src, o), read(lp), lp);
  eq(lin.linToJson(read(lp)), src, lp + " (read)");
}
for (const p of ls("read", ".lin")) eq(lin.linToJson(read(p)), read(p.replace(/\.lin$/, ".json")), p);
for (const p of ls("invalid", ".lin")) {
  let ok = false;
  try { lin.linToJson(read(p)); } catch { ok = true; }
  eq(ok, true, p + " must be rejected");
}
const schema = read(ls("schemas", ".json")[0]);
const hash = lin.schemaHash(schema);
for (const p of ls("schema", ".lin")) {
  eq(lin.linToJson(read(p), schema), read(p.replace(/\.lin$/, ".json")), p);
  const o = new lin.Options();
  o.split = p.endsWith(".split.lin");
  if (o.split) o.splitLimit = 5;
  o.schema = hash;
  eq(lin.jsonToLin(read(p.replace(/\.lin$/, ".json")), o, schema), read(p), p + " (write)");
}
console.log(`wasm conformance: ${n} checks passed`);
