# LIN in Rust

| Crate | What it is |
| --- | --- |
| [`lin`](lin/) (`lin-notation`) | The library: reader, writer, schema typing, JSON, RFC 8785 hashing |
| [`lin-cli`](lin-cli/) | The `lin` command-line tool ([docs/cli.md](../docs/cli.md)) |
| [`lin-wasm`](lin-wasm/) | The library compiled to WebAssembly, for web pages |

```rust
use lin::{json, WriteOptions};

let v = json::parse(r#"{"name": "demo", "tags": ["a", "b"]}"#)?;
let text = lin::to_string(&v)?;                                  // "LIN 1\nname demo\ntags[] a|b\n"
let split = lin::to_string_with(&v, &WriteOptions { split: true, ..Default::default() }, None)?;
assert_eq!(lin::from_str(&text)?, v);
```

`lin::Value` keeps key order and exact numbers. Files whose header names a schema are read with
`lin::from_str_with_schema`, which checks the schema's hash; enable the `serde_json` feature for
conversions to and from `serde_json::Value`.

```bash
cargo test                      # library, conformance suite and CLI tests
cargo install --path lin-cli    # the lin tool
lin-wasm/build.sh               # the WebAssembly package
```
