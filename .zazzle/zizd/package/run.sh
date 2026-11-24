#!/usr/bin/env bash
# The release artifacts of the legacy runner's release.sh, built for every commit and published by no one here: the
# release step publishes them in the release environment. Into /zazzle/output: the lin binary for this platform,
# the WebAssembly package, the npm package, the Python sdist and wheel, the lin-notation crate, the spec bundle,
# SHA256SUMS, and VERSION (the version every package carries).
set -euo pipefail
export HOME=/tmp CARGO_HOME=/tmp/cargo npm_config_cache=/tmp/.npm npm_config_update_notifier=false PYTHONDONTWRITEBYTECODE=1
srcs=(/zazzle/input/repos/*/lean-indented-notation); src=${srcs[0]}
commit=${ZAZZLE_COMMIT:-$(git -C "$src" rev-parse HEAD)}
git clone -q --shared --no-checkout "$src" /tmp/lin
git -C /tmp/lin checkout -q --detach "$commit"
cd /tmp/lin
OUT=/zazzle/output

run() { local name=$1 t0=$SECONDS; shift; echo; echo "==> $name"; "$@" || { echo "FAILED: $name" >&2; exit 1; }; echo "ok: $name ($((SECONDS - t0))s)"; }
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' rust/Cargo.toml | head -1)
TARGET=$(rustc -vV | sed -n 's/^host: //p')
ts_version=$(jq -r .version typescript/package.json)
py_version=$(sed -n 's/^version = "\(.*\)"/\1/p' python/pyproject.toml | head -1)
if [ "$ts_version" != "$VERSION" ] || [ "$py_version" != "$VERSION" ]; then
  echo "package: the versions disagree: rust/Cargo.toml $VERSION, typescript/package.json $ts_version, python/pyproject.toml $py_version" >&2
  exit 1
fi
echo "package: $VERSION for $TARGET"

lin_binary() {
  (cd rust && cargo build --release --locked -p lin-cli)
  local d="lin-$VERSION-$TARGET"
  mkdir -p "/tmp/$d"; cp rust/target/release/lin README.md LICENSE docs/cli.md "/tmp/$d/"
  tar -C /tmp -czf "$OUT/$d.tar.gz" "$d"
}
wasm_package() {
  sh rust/lin-wasm/build.sh
  local d="lin-wasm-$VERSION"
  mkdir -p "/tmp/$d"; cp -r rust/lin-wasm/js rust/lin-wasm/www rust/lin-wasm/README.md LICENSE "/tmp/$d/"
  tar -C /tmp -czf "$OUT/$d.tar.gz" "$d"
}
npm_package() { (cd typescript && npm ci --no-audit --no-fund && npm run build && npm pack --pack-destination "$OUT"); }
python_package() { python3 -m build --outdir "$OUT" python; }
crate_package() {
  (cd rust && cargo package --locked -p lin-notation --allow-dirty --no-verify)
  cp "rust/target/package/lin-notation-$VERSION.crate" "$OUT/"
}
spec_bundle() { tar -czf "$OUT/lin-spec-$VERSION.tar.gz" spec schemas registries.lin registries.json; }

run "lin tool binary" lin_binary
run "webassembly package" wasm_package
run "npm package" npm_package
run "python sdist and wheel" python_package
run "rust crate" crate_package
run "spec and schema registry bundle" spec_bundle
(cd "$OUT" && sha256sum -- * > /tmp/SHA256SUMS && mv /tmp/SHA256SUMS SHA256SUMS)
printf '%s\n' "$VERSION" > "$OUT/VERSION"
ls -l "$OUT"
echo "package: $VERSION built from $commit; nothing published"
