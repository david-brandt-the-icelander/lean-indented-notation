# Instructions for agents

This repository holds the LIN specification, its schema registry, and the Rust, TypeScript and
Python implementations. These rules apply to every agent working here.

## Every .lin file has a .json twin

Each `.lin` file has a `.json` file beside it with the same name (`registries.lin` and
`registries.json`) holding the same data as 2-space-indented JSON. The JSON is there so readers who
know JSON can see what the LIN says. **The two must always hold the same data, byte for byte in the
canonical layout.**

- Edit the `.lin` file, then run `lin sync` (or edit with `lin set`, `lin del` or `lin patch`, which
  update the twin themselves).
- If you edited the `.json` file instead, run `lin sync --prefer json <file.lin>` to carry the change
  back into the `.lin` file.
- A new `.lin` file needs its `.json`: `lin sync` creates it.
- Before committing, run `lin sync --check .` and `lin check .`. Both must pass; CI (a Docker container in the
  continuous_integration_and_delivery repository) runs them, and the
  pre-commit hook in `.githooks/` runs them if you enable it (`git config core.hooksPath .githooks`).
- Files that must not have a twin (invalid test cases) are listed in `.linignore`.

Build the tool with `cargo build --release -p lin-cli --manifest-path rust/Cargo.toml`; the binary
is `rust/target/release/lin`.

## The three implementations agree

The Rust, TypeScript and Python libraries must produce identical LIN and JSON text. Change all three
together, then:

1. If the change alters output, regenerate the expected files with
   `python3 tests/conformance/generate.py` and review the diff.
2. Run every test suite:
   - `cargo test --manifest-path rust/Cargo.toml`
   - `cd typescript && npm install && npm test`
   - `cd python && python3 -m unittest discover -s tests`
   - `rust/lin-wasm/build.sh && node rust/lin-wasm/test.cjs`
3. Keep `spec/lin-1.md` in step with any rule you change.

## Schemas

`schemas/` is the default public registry. Files there are named by their content address
(`sha256-….lin` and `.json`); never edit one in place, because its name is its hash. Publish a new
or changed schema with `lin schema publish <file> --dir schemas`, which also rewrites
`schemas/index.lin`. `lin check` fails if a file's data does not hash to its name.

## Style

- Rust: `cargo fmt` and `cargo clippy` clean.
- Python: standard library only in `python/src`.
- TypeScript: no runtime dependencies.
