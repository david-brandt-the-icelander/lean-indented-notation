# The package step of lean-indented-notation's zizd pipeline (see ../README.md).
# Toolchain, the same in the check, package and release images so Docker builds it once: Rust 1.100.0 with rustfmt,
# clippy, the wasm32 target and wasm-bindgen-cli 0.2.100 (the version rust/lin-wasm/Cargo.toml pins); Node 22.23.5;
# Debian 12's Python 3.11 with build 1.2.2 and twine 6.1.0; git, jq and curl. As the legacy runner's image, pinned.
FROM node:22.23.5-bookworm-slim@sha256:be979e5523c76fa4d5ca4ebd3099078f220aa47ec39c48ad5b5df9314debd6eb AS node
FROM rust:1.100.0-bookworm@sha256:1b9dc70890ec494e466c87f5ad41ad544bfb30ea6acc99a3e068c47a4ae3f23e
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
 && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx
RUN apt-get update \
 && apt-get install -y --no-install-recommends python3 python3-venv git jq ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && python3 -m venv /opt/py \
 && /opt/py/bin/pip install --no-cache-dir build==1.2.2 twine==6.1.0
ENV PATH=/opt/py/bin:$PATH
RUN rustup component add rustfmt clippy \
 && rustup target add wasm32-unknown-unknown \
 && cargo install --locked wasm-bindgen-cli --version 0.2.100 \
 && rm -rf /usr/local/cargo/registry /usr/local/cargo/git
COPY run.sh /run.sh
CMD ["bash", "/run.sh"]
