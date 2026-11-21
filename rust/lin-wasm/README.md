# LIN for web pages (WebAssembly)

The Rust library compiled to WebAssembly, with a small JavaScript wrapper. Use it in a page instead
of the TypeScript library when you want the exact Rust implementation.

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.100
./build.sh          # writes js/pkg/ (web) and pkg-node/ (Node, for the tests)
node test.cjs       # the conformance suite against the WebAssembly build
```

```html
<script type="module">
  import init, * as lin from "./js/lin.js";
  await init();
  lin.stringify({ name: "demo" });             // "LIN 1\nname demo\n"
  lin.parse("LIN 1\nname demo\n");             // { name: "demo" }
  lin.jsonToLin('{"b":1,"200":2}');            // text in, text out: keeps key order and digits
</script>
```

[`www/index.html`](www/index.html) is a JSON ⇄ LIN converter page built on it; serve `rust/` over
HTTP (for example `python3 -m http.server` in `rust/`) and open `lin-wasm/www/`.
