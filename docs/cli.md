# The `lin` tool

`lin` works on LIN and JSON files alike: a file that starts with `LIN ` (or ends in `.lin`) is LIN,
anything else is read as JSON. Commands that print data print LIN, the cheaper form for an agent to
read, unless given `--json`.

Install: `cargo install --path rust/lin-cli` (or build with `cargo build --release -p lin-cli` in
`rust/`). Every command takes `--help`.

## Paths and names

A path is keys and list positions joined by dots: `plugins.0.title`. Keys holding a dot, a space or a
quote are written as JSON strings: `files."a.json".size`. A path starting with `/` is a JSON Pointer:
`/plugins/0/title`. `.` is the root. `lin flat`, `lin grep` and `lin diff` print paths in this form,
and every command that takes a path accepts it.

`lin get` also takes the names the split form uses: an object's `id` (`spell-check`), or an id
followed by a path inside that object (`spell-check.summary`).

## Reading

| Command | What it prints |
| --- | --- |
| `lin cat FILE` | The file as LIN. Read JSON files this way: it costs fewer tokens. `--split` for the split form |
| `lin outline FILE` | Keys and nesting with list lengths, no values. `-t` adds scalar types, `-d N` stops at depth N |
| `lin head FILE` | The head of a split-form file: the structure and short values, with long values as `@` |
| `lin get FILE NAME` | One value, by id, reference name or path. `-r` prints a string raw, `--json` as JSON |
| `lin index [PATHS]` | One row per object with an `id`, in every file: file, path, id, title or name |
| `lin flat FILE` | Every scalar as one `path value` line; `lin unflat` turns such lines back into a document |
| `lin grep PATTERN [PATHS]` | Keys and values matching a regular expression, as `file:path value`. `-i`, `-F` (fixed text), `-k` keys only, `-V` values only, `-l` file names, `-c` counts |
| `lin query FILTER [FILES]` | A jq filter (jq syntax, run by jaq). Alias `lin jq`. `-r`, `--json`, `-c`, `--arg NAME VALUE`, `--argjson NAME JSON`, `-s` (slurp), `-n` (null input) |
| `lin diff A B` | One line per difference in the data: `+ path value`, `- path value`, `~ path old -> new`. Lists are aligned, so an insertion shows as one line. Exits 1 if they differ |
| `lin tokens FILES` | Token counts (o200k_base) as pretty JSON, minified JSON, LIN, split head and outline |

```bash
$ lin outline examples/plugin-registry.lin
name
version
description
plugins[2]
 id
 title
 ...
$ lin get examples/plugin-registry.lin word-count.notes -r
Off by default.
Turn it on in Settings.
$ lin grep -k '^lines$' examples
examples/plugin-registry.lin:plugins.0.sources.0.lines 120
examples/plugin-registry.lin:plugins.0.sources.1.lines 5000
$ lin jq '.plugins | map({id, n: (.tags | length)})' examples/plugin-registry.lin
[id|n]
 spell-check|2
 word-count|0
```

## Converting

| Command | What it does |
| --- | --- |
| `lin to-json [FILE]` | LIN (or JSON) to 2-space-indented JSON. `-c` for one line, `-o FILE` to write a file |
| `lin from-json [FILE]` | JSON (or LIN) to LIN. `--split`, `--split-limit N`, `--inline-limit N`, `--object-limit N`, `--schema FILE|HASH`, `--url URL`, `-o FILE` |
| `lin fmt FILES` | Rewrite LIN files in the writer's layout, keeping each file's header and form. `--check` changes nothing and exits 1 if a file would change |
| `lin to-csv FILE [-p PATH]` | A table (list of flat objects) as CSV |
| `lin from-csv FILE` | CSV with a header row as a LIN table. `--strings` keeps every field a string |

## Editing

Edits write the file back in its own format (LIN or JSON) and update its `.lin`/`.json` twin if
there is one. If the file has a schema, `lin` warns when the result breaks it. `--stdout` prints the
result instead of writing.

| Command | What it does |
| --- | --- |
| `lin set FILE PATH VALUE` | Set or add a value. VALUE is a LIN scalar (`text`, `12`, `true`, `~`, `[]`, `{}`, `"a JSON string"`); `--json` takes any JSON value, `--string` takes the text as is. `-` as the last list position appends |
| `lin del FILE PATHS` | Delete values |
| `lin patch FILE PATCH` | Apply a JSON Patch (RFC 6902); `--merge` applies a merge patch (RFC 7386). The patch may be LIN or JSON |
| `lin merge FILES` | Deep-merge files, later ones winning, and print the result. `--concat` joins lists |

## Checking and keeping files in step

| Command | What it does |
| --- | --- |
| `lin check [PATHS]` | Syntax and every MUST rule, the final newline, validation against the file's schema, hashes of `sha256-…` files, and whether each `.lin` file's `.json` twin holds the same data. `--layout` also requires the writer's layout; `--require-siblings` requires a twin for every `.lin` |
| `lin validate FILE` | Validate against the header's schema, or `--schema FILE` |
| `lin sync [PATHS]` | Write each `.lin` file's `.json` twin. If both exist and differ, the newer one wins (`--prefer lin|json` to choose). `--check` changes nothing and exits 1 if any pair is out of step or missing |

`check` and `sync` skip paths listed in `.linignore` (one glob per line).

## Schemas

| Command | What it does |
| --- | --- |
| `lin hash FILE` | The schema's content address, `sha256-` and 43 base64url characters |
| `lin schema infer FILES` | A JSON Schema that the example files all satisfy, in LIN |
| `lin schema get HASH` | Find a schema by hash and print it |
| `lin schema publish FILE [--dir schemas]` | Write the schema into a registry directory as `<hash>.lin` and `<hash>.json`, and update its `index.lin` |
| `lin schema index [--dir schemas]` | Rewrite a registry directory's `index.lin` |
| `lin schema where [FILE]` | The places a schema is looked for, in order |

A file whose header names a schema is read with that schema, which decides whether bare `20` is a
number or a string (spec section 7). `lin` looks for it in `LIN_SCHEMA_PATH`, in a `schemas/`
directory beside the file or above it, in its cache (`LIN_CACHE_DIR`, by default
`~/.cache/lin/schemas`), among the schemas built into it (the LIN meta-schema), at the header's URL,
and in the public registries (`registries.lin`, or `LIN_REGISTRIES` as comma-separated prefixes). See
[spec/registry.md](../spec/registry.md). `--offline` stops network lookups; `--no-schema` reads such
files by the schema-less rules.

## Exit codes

0: success. 1: the command ran and found something (a difference, a problem, no grep match).
2: an error (a file that cannot be read or parsed, a bad argument).
