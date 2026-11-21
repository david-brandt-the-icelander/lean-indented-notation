# LIN schema registries

A LIN header can name a schema by its hash: `LIN 1 sha256-… [url]`. The hash is the schema's content
address (spec section 7). This page defines where readers look it up.

## Lookup order

A reader resolving a hash `H`:

1. looks in local stores: directories it was configured with (`LIN_SCHEMA_PATH` for the `lin` tool),
   then a `schemas/` directory beside the file or in any directory above it, then its cache;
2. fetches the header's URL, if the header has one;
3. fetches `<prefix>H.lin`, then `<prefix>H.json`, from each public registry in turn.

Whatever it finds, it reads into its JSON value, hashes as section 7 describes, and rejects if the hash
is not `H`. Because of that check, a registry needs no trust: any copy that hashes right is the schema.

## Registries

[`registries.lin`](../registries.lin) lists the public registries, in the order readers try them.
Its schema is published in the default registry. Each entry has:

| Field | Meaning |
| --- | --- |
| `name` | A short name |
| `prefix` | An `https://` URL ending in `/`; the schema with hash `H` is at `<prefix>H.lin` and `<prefix>H.json` |
| `description` | Optional |

The list starts with one registry, this repository's [`schemas/`](../schemas/) directory, served by
GitHub at `https://raw.githubusercontent.com/david-brandt-the-icelander/lean-indented-notation/main/schemas/`.
It is free to use and to mirror: copying the files anywhere makes another registry, since every reader
checks hashes.

## A registry's layout

- `<hash>.lin`: the schema in LIN. Its header names the LIN meta-schema
  (`sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars`), which names itself.
- `<hash>.json`: the same schema as JSON.
- `index.lin` and `index.json`: one row per schema, with its hash, `$id` and `title`.

Files are never changed in place: a changed schema has a new hash, so it is a new file.

## Publishing

`lin schema publish my-schema.lin --dir schemas` writes both files and updates the index. To add a
schema to the default registry, open a pull request to this repository with those files.
