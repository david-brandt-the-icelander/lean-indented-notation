# LIN: Lean Indented Notation

LIN (`.lin`) is a text format for structured data that AI agents read and write. It holds exactly
what JSON holds, in fewer tokens, and stays readable and editable as plain text: on a random sample
of 124 JSON files from GitHub it takes **58.9%** of the tokens of 2-space-indented JSON, against
69.9% for minified JSON and 84.5% for YAML.

```json
{
  "name": "example-registry",
  "plugins": [
    {
      "id": "spell-check",
      "enabled": true,
      "tags": ["text", "language"],
      "sources": [
        { "path": "src/spell.ts", "lines": 120 },
        { "path": "dict/en.txt", "lines": 5000 }
      ]
    }
  ]
}
```

```
LIN 1
name example-registry
plugins
+ id spell-check
 enabled true
 tags[] text|language
 sources[path|lines]
  src/spell.ts|120
  dict/en.txt|5000
```

## What is here

| Path | What it is |
| --- | --- |
| [`spec/lin-1.md`](spec/lin-1.md) | The specification, with the token-cost reasoning (Appendix A) and the benchmark (Appendix B) |
| [`spec/registry.md`](spec/registry.md) | How schemas are found by their content address |
| [`schemas/`](schemas/) | The default public schema registry: the LIN meta-schema and other published schemas, named by hash |
| [`registries.lin`](registries.lin) | The list of public schema registries (initially only `schemas/` above) |
| [`rust/`](rust/) | The Rust library (`lin-notation`), the `lin` command-line tool, and the WebAssembly build for web pages |
| [`typescript/`](typescript/) | The TypeScript library (`lean-indented-notation` on npm) |
| [`python/`](python/) | The Python library (`lean-indented-notation` on PyPI, module `lin`) |
| [`docs/cli.md`](docs/cli.md) | Every `lin` command, with examples |
| [`examples/`](examples/) | The spec's full example: a plugin registry, its schema, plain and split form |
| [`tests/conformance/`](tests/conformance/) | Shared test cases every implementation must pass |
| [`bench/`](bench/) | The benchmark sample (by reference) and the scripts that measure it |

Every `.lin` file in this repository has a `.json` file beside it holding the same data, so readers
who know JSON can see what the LIN says. The two are kept in step by `lin sync`, and CI fails if they
are not (see [AGENTS.md](AGENTS.md)).

CI and releases run in a Docker container defined in
[continuous_integration_and_delivery/lean-indented-notation](https://github.com/david-brandt-the-icelander/continuous_integration_and_delivery/tree/main/lean-indented-notation):
it watches this repository, checks every commit on `main` and releases every `v*` tag.

## The `lin` tool

```bash
cargo install --path rust/lin-cli
```

```bash
lin cat package.json                 # any JSON file, printed as LIN (fewer tokens to read)
lin outline data.lin                 # keys and nesting, no values
lin get data.lin spell-check.summary # one value by id or path
lin grep -i spell .                  # search keys and values in every .lin and .json file
lin query '.plugins[].id' data.lin   # jq filters (alias: lin jq)
lin set data.lin plugins.0.enabled false
lin check .                          # syntax, schemas, hashes, .lin/.json siblings
```

See [docs/cli.md](docs/cli.md) for the rest: `to-json`, `from-json`, `fmt`, `head`, `index`,
`flat`/`unflat`, `del`, `patch`, `merge`, `diff`, `validate`, `sync`, `schema
infer|get|publish|index|where`, `to-csv`/`from-csv`, `hash` and `tokens`.

## Libraries

All three read and write byte-for-byte the same LIN, checked by the same
[conformance suite](tests/conformance/).

```rust
let v = lin::json::parse(r#"{"name": "demo"}"#)?;
let text = lin::to_string(&v)?;            // "LIN 1\nname demo\n"
```

```ts
import * as lin from "lean-indented-notation";
lin.stringify({ name: "demo" });           // "LIN 1\nname demo\n"
lin.parse("LIN 1\nname demo\n");           // { name: "demo" }
```

```python
import lin
lin.dumps({"name": "demo"})                # 'LIN 1\nname demo\n'
lin.loads("LIN 1\nname demo\n")            # {'name': 'demo'}
```

In a web page, use the WebAssembly build instead of the TypeScript library:
[`rust/lin-wasm`](rust/lin-wasm/).

## Licence

MIT. The JSON Schema meta-schemas in `schemas/` are the JSON Schema project's; see
[schemas/README.md](schemas/README.md).
