#!/usr/bin/env bash
# The release step: the deploy phase of the pipeline, run by a deployer in the `release` environment (the `release`
# branch). It reads the version from the repository, publishes the package step's artifacts (crates.io: the library
# crate then the lin tool; npm; PyPI), then creates the tag vVERSION at the commit and a GitHub release with the
# artifacts attached. Nothing here pushes a tag from git: the tag is made through GitHub's API, after publishing.
#
# It refuses, before publishing anything:
#   - outside the release environment (ZAZZLE_ENV must be `release`), or outside the deploy phase;
#   - without GITHUB_RELEASE_TOKEN, or without any registry token;
#   - when the package step's artifacts are missing or do not match their SHA256SUMS;
#   - when rust/Cargo.toml, typescript/package.json, python/pyproject.toml and the artifacts disagree on the version;
#   - when the tag vVERSION already exists at another commit (the version was not bumped);
#   - when a package name on a registry belongs to another project;
#   - when a registry's own dry run fails.
# A rerun of the same commit is safe: what is already published, tagged or attached is left alone.
# A registry whose token is absent (only possible when the pipeline marks it `KEY?`) is skipped, with a note.
#
# DEPLOY_DRY_RUN=1 (by hand only; no pipeline names it) needs no credentials and no environment: it runs every check
# and every registry's dry run, prints what it would publish, and changes nothing anywhere.
set -euo pipefail
export HOME=/tmp CARGO_HOME=/tmp/cargo npm_config_cache=/tmp/.npm npm_config_update_notifier=false PYTHONDONTWRITEBYTECODE=1
dry=${DEPLOY_DRY_RUN:-0}
say() { echo "release: $*"; }
refuse() { echo "release: REFUSED: $*" >&2; exit 1; }
halt_check() { # exit 75 when the deployer asks this step to halt (only between sub-steps; each is safe to redo)
  if [ -n "${ZAZZLE_HALT_FILE:-}" ] && [ -f "$ZAZZLE_HALT_FILE" ]; then say "halting on request: $(cat "$ZAZZLE_HALT_FILE")"; exit 75; fi
}

# ── Where and with what this may run ──────────────────────────────────────────────────────────────────────────────
slug=${ZAZZLE_PIPELINE:?ZAZZLE_PIPELINE is not set}; slug=${slug%%:*}
if [ "$dry" = 1 ]; then
  say "DRY RUN for $ZAZZLE_PIPELINE: nothing will be published, tagged or released"
else
  [ "${ZAZZLE_STAGE:-}" = deploy ] || refuse "runs only in the deploy phase (ZAZZLE_STAGE is '${ZAZZLE_STAGE:-}')"
  [ "${ZAZZLE_ENV:-}" = release ] || refuse "runs only in the release environment (this is $ZAZZLE_PIPELINE)"
  [ -n "${GITHUB_RELEASE_TOKEN:-}" ] || refuse "no GITHUB_RELEASE_TOKEN: the tag and the GitHub release cannot be made"
  [ -n "${CARGO_REGISTRY_TOKEN:-}${NPM_TOKEN:-}${PYPI_TOKEN:-}" ] || refuse "no registry token (CARGO_REGISTRY_TOKEN, NPM_TOKEN, PYPI_TOKEN)"
fi
halt_check

# ── The artifacts and the version ─────────────────────────────────────────────────────────────────────────────────
A=${ZAZZLE_ARTIFACTS:-/zazzle/input/artifacts}/package
{ [ -f "$A/VERSION" ] && [ -f "$A/SHA256SUMS" ]; } || refuse "no package artifacts in $A"
(cd "$A" && sha256sum --quiet -c SHA256SUMS) || refuse "the package artifacts do not match their SHA256SUMS"

srcs=(/zazzle/input/repos/*/lean-indented-notation); src=${srcs[0]}
commit=${ZAZZLE_COMMIT:-$(git -C "$src" rev-parse HEAD)}
git clone -q --shared --no-checkout "$src" /tmp/lin
git -C /tmp/lin checkout -q --detach "$commit"
cd /tmp/lin
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' rust/Cargo.toml | head -1)
ts_version=$(jq -r .version typescript/package.json)
py_version=$(sed -n 's/^version = "\(.*\)"/\1/p' python/pyproject.toml | head -1)
built_version=$(cat "$A/VERSION")
if [ "$ts_version" != "$VERSION" ] || [ "$py_version" != "$VERSION" ] || [ "$built_version" != "$VERSION" ]; then
  refuse "the versions disagree: rust/Cargo.toml $VERSION, typescript/package.json $ts_version, python/pyproject.toml $py_version, artifacts $built_version"
fi
[[ $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]] || refuse "'$VERSION' is not a version"
tag=v$VERSION
crates=("$(sed -n 's/^name = "\(.*\)"/\1/p' rust/lin/Cargo.toml | head -1)" "$(sed -n 's/^name = "\(.*\)"/\1/p' rust/lin-cli/Cargo.toml | head -1)")
npm_name=$(jq -r .name typescript/package.json)
py_name=$(sed -n 's/^name = "\(.*\)"/\1/p' python/pyproject.toml | head -1)
npm_tgz=$A/$npm_name-$VERSION.tgz
py_files=("$A"/*.whl)
for f in "$A"/*-"$VERSION".tar.gz; do case $(basename "$f") in lin-*) ;; *) py_files+=("$f") ;; esac; done
say "$slug $VERSION at $commit: crates ${crates[*]}, npm $npm_name, PyPI $py_name, tag $tag"

# ── Preflight: what exists already, and whose it is ───────────────────────────────────────────────────────────────
problems=()
problem() { problems+=("$*"); echo "release: PROBLEM: $*" >&2; }
ua="zizd-release ($slug)"
ours() { grep -qiE "github\.com[/:]$slug(\.git)?/?\$" <<<"$1"; } # ours URL: is URL this repository?
fetch() { curl -sS -A "$ua" -o /tmp/resp.json -w '%{http_code}' "$1" || echo 000; } # fetch URL: HTTP status, body in /tmp/resp.json

tag_line=$(git ls-remote --tags "https://github.com/$slug.git" "refs/tags/$tag" "refs/tags/$tag^{}" | sort -k2 | tail -1)
tag_commit=${tag_line%%[[:space:]]*}
tag_exists=false
if [ -n "$tag_line" ]; then
  if [ "$tag_commit" = "$commit" ]; then tag_exists=true; say "tag $tag is already at this commit: a rerun, finishing what is missing"
  else problem "tag $tag already exists at $tag_commit, not $commit: bump the version in rust/Cargo.toml, typescript/package.json and python/pyproject.toml"; fi
fi

pending_crates=()
for c in "${crates[@]}"; do
  code=$(fetch "https://crates.io/api/v1/crates/$c")
  case $code in
    404) pending_crates+=("$c"); say "crates.io: $c is new" ;;
    200) repo=$(jq -r '.crate.repository // ""' /tmp/resp.json)
         if ! ours "$repo"; then problem "crates.io: the name $c belongs to another project (${repo:-no repository}); rename the crate"
         elif jq -e --arg v "$VERSION" '.versions | map(.num) | index($v)' /tmp/resp.json >/dev/null; then say "crates.io: $c $VERSION is already published"
         else pending_crates+=("$c"); fi ;;
    *) problem "crates.io: could not look up $c (HTTP $code)" ;;
  esac
done

npm_pending=false
code=$(fetch "https://registry.npmjs.org/$npm_name")
case $code in
  404) npm_pending=true; say "npm: $npm_name is new" ;;
  200) repo=$(jq -r '.repository.url // .repository // ""' /tmp/resp.json)
       if ! ours "$repo"; then problem "npm: the name $npm_name belongs to another project (${repo:-no repository})"
       elif jq -e --arg v "$VERSION" '.versions[$v]' /tmp/resp.json >/dev/null; then say "npm: $npm_name $VERSION is already published"
       else npm_pending=true; fi ;;
  *) problem "npm: could not look up $npm_name (HTTP $code)" ;;
esac

py_pending=false
code=$(fetch "https://pypi.org/pypi/$py_name/json")
case $code in
  404) py_pending=true; say "PyPI: $py_name is new" ;;
  200) urls=$(jq -r '[.info.home_page // empty, (.info.project_urls // {} | .[])] | .[]' /tmp/resp.json)
       if ! grep -qiE "github\.com[/:]$slug(\.git)?/?\$" <<<"$urls"; then problem "PyPI: the name $py_name belongs to another project"
       elif jq -e --arg v "$VERSION" '.releases[$v] | length > 0' /tmp/resp.json >/dev/null; then say "PyPI: $py_name $VERSION is already published"
       else py_pending=true; fi ;;
  *) problem "PyPI: could not look up $py_name (HTTP $code)" ;;
esac

# ── Each registry's own dry run ───────────────────────────────────────────────────────────────────────────────────
# The published lin tool carries the schemas and the registry list in embedded/, as the legacy runner's release did.
mkdir -p rust/lin-cli/embedded/schemas
cp schemas/*.lin rust/lin-cli/embedded/schemas/
cp registries.lin rust/lin-cli/embedded/
if [ ${#pending_crates[@]} -gt 0 ]; then
  pkgs=(); for c in "${pending_crates[@]}"; do pkgs+=(-p "$c"); done
  (cd rust && cargo publish --locked --allow-dirty --dry-run "${pkgs[@]}") || problem "crates.io: cargo publish --dry-run failed"
fi
if $npm_pending; then npm publish --dry-run --access public "$npm_tgz" || problem "npm: npm publish --dry-run failed"; fi
if $py_pending; then twine check --strict "${py_files[@]}" || problem "PyPI: twine check failed"; fi

if [ ${#problems[@]} -gt 0 ]; then
  echo "release: $([ "$dry" = 1 ] && echo "the release would be REFUSED" || echo "REFUSED, nothing published"):" >&2
  printf '  - %s\n' "${problems[@]}" >&2
  exit 1
fi

plan="crates.io: ${pending_crates[*]:-nothing}; npm: $($npm_pending && echo "$npm_name" || echo nothing); PyPI: $($py_pending && echo "$py_name" || echo nothing); tag: $($tag_exists && echo "exists" || echo "$tag at $commit"); GitHub release $tag with $(find "$A" -maxdepth 1 -type f ! -name VERSION | wc -l | tr -d ' ') files"
if [ "$dry" = 1 ]; then say "DRY RUN passed; a release would publish $plan"; exit 0; fi
say "publishing $plan"

# ── Publish ───────────────────────────────────────────────────────────────────────────────────────────────────────
done_list=()
halt_check
if [ ${#pending_crates[@]} -gt 0 ]; then
  if [ -n "${CARGO_REGISTRY_TOKEN:-}" ]; then
    (cd rust && cargo publish --locked --allow-dirty "${pkgs[@]}")   # cargo reads CARGO_REGISTRY_TOKEN from the environment
    done_list+=("crates.io ${pending_crates[*]}")
  else say "crates.io: no CARGO_REGISTRY_TOKEN, skipped"; fi
fi
halt_check
if $npm_pending; then
  if [ -n "${NPM_TOKEN:-}" ]; then
    # npm expands ${NPM_TOKEN} in its config itself, so the value is never written anywhere.
    # shellcheck disable=SC2016
    printf '//registry.npmjs.org/:_authToken=${NPM_TOKEN}\n' > /tmp/npmrc
    npm publish --access public --userconfig /tmp/npmrc "$npm_tgz"
    done_list+=("npm $npm_name")
  else say "npm: no NPM_TOKEN, skipped"; fi
fi
halt_check
if $py_pending; then
  if [ -n "${PYPI_TOKEN:-}" ]; then
    TWINE_USERNAME=__token__ TWINE_PASSWORD=$PYPI_TOKEN twine upload --non-interactive "${py_files[@]}"
    done_list+=("PyPI $py_name")
  else say "PyPI: no PYPI_TOKEN, skipped"; fi
fi

halt_check
api=https://api.github.com/repos/$slug
gh_api() { # gh_api METHOD URL [curl args…]: the token reaches curl as a header on stdin, never on a command line
  local method=$1 url=$2; shift 2
  printf 'Authorization: Bearer %s\n' "$GITHUB_RELEASE_TOKEN" |
    curl -fsS -H @- -H 'Accept: application/vnd.github+json' -H 'X-GitHub-Api-Version: 2022-11-28' -X "$method" "$url" "$@"
}
if ! $tag_exists; then
  jq -n --arg t "$tag" --arg c "$commit" --arg m "lean-indented-notation $VERSION" \
    '{tag: $t, message: $m, object: $c, type: "commit"}' > /tmp/tag.json
  tag_object=$(gh_api POST "$api/git/tags" --data-binary @/tmp/tag.json | jq -r .sha)
  jq -n --arg r "refs/tags/$tag" --arg s "$tag_object" '{ref: $r, sha: $s}' > /tmp/ref.json
  gh_api POST "$api/git/refs" --data-binary @/tmp/ref.json >/dev/null
  done_list+=("tag $tag")
  say "tagged $tag at $commit"
fi

halt_check
release_id=$(gh_api GET "$api/releases/tags/$tag" 2>/dev/null | jq -r '.id // empty' || true)
if [ -z "$release_id" ]; then
  jq -n --arg t "$tag" --arg c "$commit" '{tag_name: $t, target_commitish: $c, name: $t, generate_release_notes: true}' > /tmp/release.json
  release_id=$(gh_api POST "$api/releases" --data-binary @/tmp/release.json | jq -r .id)
fi
attached=$(gh_api GET "$api/releases/$release_id/assets?per_page=100" | jq -r '.[].name')
for f in "$A"/*; do
  name=$(basename "$f")
  [ "$name" != VERSION ] || continue
  grep -qxF "$name" <<<"$attached" && continue
  gh_api POST "https://uploads.github.com/repos/$slug/releases/$release_id/assets?name=$name" \
    -H 'Content-Type: application/octet-stream' --data-binary @"$f" >/dev/null
  say "attached $name"
done
done_list+=("GitHub release $tag")

jq -n --arg v "$VERSION" --arg t "$tag" --arg c "$commit" --args '{version: $v, tag: $t, commit: $c, done: $ARGS.positional}' \
  "${done_list[@]}" > /zazzle/output/release.json
say "released $VERSION: ${done_list[*]}"
