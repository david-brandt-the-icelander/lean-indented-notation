# `.zazzle/zizd/`: lean-indented-notation's zizd pipeline

The steps zazzle's builders and deployers run for this repository, written against zazzle's step-pipeline spec
(`src/macos/zazzle-pipeline/pipeline-spec.md`, sections 1–5). One pipeline, `default`, so each step is a folder
`.zazzle/zizd/STEP/` with a `Dockerfile` and a `run.sh`. The pipeline file itself lives in the orchestrator
(`continuous_integration_and_delivery`, `.zazzle/zizd/pipelines/david-brandt-the-icelander/lean-indented-notation/default.json`);
the copy below is what goes there.

It replaces the legacy runner `continuous_integration_and_delivery/lean-indented-notation/`, which checks every commit on
`main` and releases every `v*` tag. Here **releases come from a release branch instead of tags**: the `release`
environment builds the branch `release`, and its deploy step reads the version from the repository, publishes, and then
creates the tag itself.

| Step | Phase | What it does | Leaves in `/zazzle/output` |
|---|---|---|---|
| `check` | build | every check of the legacy `scripts/ci.sh`: rustfmt, clippy `-D warnings`, the Rust tests, the `lin` tool's release build, `lin sync --check` and `lin check --require-siblings`, the WebAssembly build and its conformance run in Node, the TypeScript build and tests, the Python tests, and the conformance files as `tests/conformance/generate.py` writes them | nothing |
| `package` | build | the artifacts of the legacy `scripts/release.sh`, built for every commit: `lin-VERSION-TARGET.tar.gz` (the `lin` binary for the builder's platform, README, licence, CLI docs), `lin-wasm-VERSION.tar.gz`, the npm package `lean-indented-notation-VERSION.tgz`, the Python wheel and sdist, `lin-notation-VERSION.crate`, `lin-spec-VERSION.tar.gz`; refuses when `rust/Cargo.toml`, `typescript/package.json` and `python/pyproject.toml` disagree on the version | those files, `SHA256SUMS`, `VERSION` |
| `release` | deploy (`deploys: true`) | in the `release` environment only: publishes the `package` step's artifacts and creates the tag and the GitHub release (below) | `release.json`: version, tag, commit and what was done |

All three images share one toolchain, pinned: `rust:1.100.0-bookworm` with rustfmt, clippy, the `wasm32-unknown-unknown`
target and `wasm-bindgen-cli` 0.2.100 (the version `rust/lin-wasm/Cargo.toml` pins); Node 22.23.5 from
`node:22.23.5-bookworm-slim`; Debian 12's Python 3.11 with `build` 1.2.2 and `twine` 6.1.0 (the legacy image's pins);
git, jq and curl. Every build tree is in the container's `/tmp`, in a clone of the read-only checkout that shares its
objects.

Every `FROM` names its image by tag and by digest (the multi-arch index), so a rebuilt image starts from the same
bytes on any builder.

## The pipeline file

```json
{
  "id": "david-brandt-the-icelander/lean-indented-notation:default",
  "repos": ["https://github.com/david-brandt-the-icelander/lean-indented-notation.git"],
  "steps": [
    { "name": "check", "path": ".zazzle/zizd/check" },
    { "name": "package", "path": ".zazzle/zizd/package" },
    { "name": "release", "path": ".zazzle/zizd/release", "deploys": true }
  ],
  "environments": {
    "release": {
      "branch": "release",
      "config": { "release": ["GITHUB_RELEASE_TOKEN", "CARGO_REGISTRY_TOKEN", "NPM_TOKEN", "PYPI_TOKEN"] }
    }
  },
  "supersede": true,
  "halt": { "deployment": "none", "build": "none" }
}
```

- **The base pipeline** (`…:default`) builds every commit on `main`: `check` and `package`. Its deploy phase never runs,
  because no deployer serves it: the deployer is registered with
  `--serves 'david-brandt-the-icelander/lean-indented-notation:default@release'` only. Should one serve the base by
  mistake, the `release` step refuses (it runs only when `ZAZZLE_ENV` is `release`) and gets no keys there anyway.
- **The `release` environment** (`…:default@release`) builds the branch `release` the same way, and a deployer then runs
  `release` on the newest finished build.
- **`halt` is `none`** for both: a release that has started publishing finishes. Since only the newest finished build is
  deployed, a release halted part-way would leave a version half published with nothing coming back to finish it.
- No `paths`: every commit is checked, as today.

### Releasing

1. On `main`, bump the version in `rust/Cargo.toml` (the workspace version), `typescript/package.json` (and its lock
   file) and `python/pyproject.toml`, together.
2. Move `release` to that commit (`git push origin main:release`).
3. The builder checks and packages the commit as `…:default@release`; the deployer then runs `release`, which:
   - refuses, before publishing anything, when: it is not in the `release` environment or the deploy phase; there is no
     `GITHUB_RELEASE_TOKEN` or no registry token; the `package` artifacts are missing or fail `SHA256SUMS`; the versions
     disagree; the tag `vVERSION` already exists at another commit (the version was not bumped); a package name on a
     registry belongs to another project; or a registry's own dry run (`cargo publish --dry-run`, `npm publish --dry-run`,
     `twine check --strict`) fails;
   - publishes `lin-notation` and then `lin-cli` to crates.io (one `cargo publish -p … -p …`, with the schemas and
     `registries.lin` copied into `rust/lin-cli/embedded/` as the legacy release did), the npm package, and the wheel and
     sdist to PyPI;
   - creates the annotated tag `vVERSION` at the commit through GitHub's API, then a GitHub release for it with every
     artifact attached.
   
   A rerun of the same commit finishes what is missing and leaves alone what is already published, tagged or attached.
   No git tag is ever pushed by hand.

The legacy runner releases every `v*` tag; here a tag is the result of a release, not its trigger.

## Config keys

Names only; the values go in the orchestrator's config files (later zeecrets), never here.

| Key | Step | File and section | What it is |
|---|---|---|---|
| `GITHUB_RELEASE_TOKEN` | `release` | `config/david-brandt-the-icelander/lean-indented-notation/default@release.json`, `deploy_keys` | a fine-grained token for this repository with *Contents: read and write* (creates the tag and the GitHub release) |
| `CARGO_REGISTRY_TOKEN` | `release` | same file, `deploy_keys` | crates.io: publish `lin-notation` and `lin-cli` |
| `NPM_TOKEN` | `release` | same file, `deploy_keys` | npm: publish `lean-indented-notation` |
| `PYPI_TOKEN` | `release` | same file, `deploy_keys` | PyPI: publish `lean-indented-notation` |

All four are deploy-only, named only by the `release` environment's `config`, so the base pipeline and every build step
get none. Marking one optional (`"PYPI_TOKEN?"`) makes the release skip that registry, with a note, as the legacy runner
did for a missing token. The build steps need no key: the repository is public. The builder's own `GITHUB_TOKEN` and z3
keys (`shared.json`) are zazzle's and reach no step.

## Before the first release

Nothing has been published to any registry yet (2026-11-24): `lin-notation` is not on crates.io and
`lean-indented-notation` is on neither npm nor PyPI. **The crates.io name `lin-cli` belongs to an unrelated project**
(a CLI for Linear, at `github.com/sprouted-dev/lin`), so the `release` step refuses until `rust/lin-cli` is published
under another name. The legacy runner would have failed at that `cargo publish`.

## Trying it by hand

Each step was run with the spec's "Trying a step by hand" recipe (`--cpus 2 --memory 4g --pids-limit 4096`, the host's
uid, the repository read-only; the deploy step also with a read-only `/zazzle/control` and `ZAZZLE_HALT_FILE`), never with
a credential:

- `check` and `package` on this branch;
- `release` in the `release` environment without credentials: refuses;
- `release` in the base pipeline with stand-in token values: refuses (not the release environment);
- `release` with a halt file present: exits 75 before doing anything;
- `release` with `DEPLOY_DRY_RUN=1` (by hand only; no pipeline names it), which needs no credentials and changes nothing:
  on this branch (version 1.1.0, whose tag `v1.1.0` is at another commit) and on a scratch commit with the version
  bumped to 1.2.0 (never pushed).

## What goes at cutover

Not yet removed: the legacy runner `continuous_integration_and_delivery/lean-indented-notation/` (`scripts/ci.sh` is
`check`, the build half of `scripts/release.sh` is `package`, its publishing half and `scripts/github-release.sh` are
`release`; `scripts/serve.sh`, `scripts/render.py` and the status page are replaced by zizd). The repository has no
GitHub workflows. The branch `release` does not exist yet; it is created at cutover, at the first commit to release.
