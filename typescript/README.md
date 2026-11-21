# LIN for TypeScript and JavaScript

`npm install lean-indented-notation`. No runtime dependencies; runs in Node and browsers. (In a web
page you can also use the WebAssembly build of the Rust library: `rust/lin-wasm`.)

```ts
import * as lin from "lean-indented-notation";

lin.stringify({ name: "demo", tags: ["a", "b"] }); // "LIN 1\nname demo\ntags[] a|b\n"
lin.parse("LIN 1\nname demo\n");                    // { name: "demo" }
lin.stringify(doc, { split: true });               // split form
lin.linToJson(text);                                // LIN text to JSON text, keeping key order and digits
lin.jsonToLin(jsonText);                            // JSON text to LIN text
lin.schemaHash(schema);                             // "sha256-…"
```

`parse` and `stringify` use plain JavaScript values and so share `JSON.parse`'s limits: integer-like
keys such as `"200"` come first in an object, and numbers are doubles (integers beyond 2^53 come back
as `bigint`). `linToJson` and `jsonToLin` work on text and keep everything.

A file whose header names a schema needs it: `lin.parse(text, { schema })`, or
`lin.parse(text, { ignoreSchema: true })` to read it by the schema-less rules. Errors are `LinError`
(with `.line`) and `SchemaRequiredError`.

Tests: `npm install && npm test`.
