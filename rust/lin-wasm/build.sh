#!/bin/sh
# Builds the WebAssembly package into pkg/ (for web pages) and pkg-node/ (for Node, used by tests).
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.100
set -e
cd "$(dirname "$0")"
cargo build -p lin-wasm --target wasm32-unknown-unknown --release
WASM=../target/wasm32-unknown-unknown/release/lin_wasm.wasm
wasm-bindgen --target web --out-dir js/pkg "$WASM"
wasm-bindgen --target nodejs --out-dir pkg-node "$WASM"
echo "built js/pkg (web) and pkg-node (node)"
