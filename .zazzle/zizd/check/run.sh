#!/usr/bin/env bash
# Every check the legacy runner's ci.sh runs, in the order a failure is cheapest to find: rustfmt, clippy, the Rust
# tests (library, conformance, lin tool), the lin tool's release build, .lin/.json twins in step and every file valid,
# the WebAssembly build and its conformance run in Node, the TypeScript build and tests, the Python tests, and the
# conformance files as the Python implementation generates them.
set -euo pipefail
export HOME=/tmp CARGO_HOME=/tmp/cargo npm_config_cache=/tmp/.npm npm_config_update_notifier=false PYTHONDONTWRITEBYTECODE=1
srcs=(/zazzle/input/repos/*/lean-indented-notation); src=${srcs[0]}
commit=${ZAZZLE_COMMIT:-$(git -C "$src" rev-parse HEAD)}
# A clone, not a copy, so `git diff` below compares with the commit; it shares the checkout's objects.
git clone -q --shared --no-checkout "$src" /tmp/lin
git -C /tmp/lin checkout -q --detach "$commit"
cd /tmp/lin

run() { local name=$1 t0=$SECONDS; shift; echo; echo "==> $name"; "$@" || { echo "FAILED: $name" >&2; exit 1; }; echo "ok: $name ($((SECONDS - t0))s)"; }
rust_fmt() { (cd rust && cargo fmt --all -- --check); }
rust_clippy() { (cd rust && cargo clippy --workspace --all-targets --locked -- -D warnings); }
rust_test() { (cd rust && cargo test --workspace --locked); }
lin_build() { (cd rust && cargo build --release --locked -p lin-cli); }
lockstep() { rust/target/release/lin --offline sync --check . && rust/target/release/lin --offline check --require-siblings .; }
wasm() { sh rust/lin-wasm/build.sh && node rust/lin-wasm/test.cjs; }
typescript() { (cd typescript && npm ci --no-audit --no-fund && npm test); }
python() { python3 -m unittest discover -s python/tests; }
conformance_current() { python3 tests/conformance/generate.py && git diff --exit-code -- tests/conformance; }

rustc --version; node --version; python3 --version
run "rust: format" rust_fmt
run "rust: clippy" rust_clippy
run "rust: tests (library, conformance, lin tool)" rust_test
run "rust: build the lin tool" lin_build
run ".lin and .json twins in step; every file valid" lockstep
run "webassembly: build and conformance" wasm
run "typescript: build and tests" typescript
run "python: tests" python
run "conformance files are current" conformance_current
echo "check: all checks passed on $(git rev-parse --short HEAD)"
