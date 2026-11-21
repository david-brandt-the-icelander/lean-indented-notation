# LIN schema registries

A LIN header can name the file's schema in one of two ways:

| Form | Looks like | Tokens | Meaning |
| --- | --- | --- | --- |
| Registered number | `LIN 1 7` | 2 (1–999), 3 (to 999,999) | A number the official registry assigned to one schema, for good |
| Provisional id | `LIN 1 068856193987000667701029602040289804041` | 14 | The first 128 bits of the schema's SHA-256, as 39 decimal digits |

Anyone can compute a provisional id, offline, for any schema. A registered number is cheaper in
every file that uses it, so a schema that many files share is worth registering. A schema that is
never registered (a private one, say) can use its provisional id forever.

## Ids

- **Provisional id.** Serialize the schema's JSON value with the JSON Canonicalization Scheme
  (RFC 8785) and take the SHA-256 of the UTF-8 bytes. Take the first 16 bytes as a big-endian
  unsigned integer and write it in decimal, padded with leading zeros to 39 digits. Decimal is the
  cheapest text for tokenizers, which split digits three at a time: 39 digits are always 13 tokens.
- **Registered number.** 1 to 9 digits with no leading zero, assigned in order by the official
  registry and listed in its index with the schema's provisional id and full SHA-256. A number is
  never reassigned, and its schema never changes: a changed schema is a new schema.

The two forms cannot be confused: a provisional id has exactly 39 digits, a number at most 9.
A URL may follow a provisional id in the header (`LIN 1 <id> https://…`), and not a number, since a
number is looked up in the registry.

128 bits are not meant as a strong security boundary on their own. The registry refuses a schema
whose provisional id is already listed with other content, and records each schema's full SHA-256;
readers check what they fetch against both.

## Registries

[`registries.lin`](../registries.lin) lists the public registries in the order readers try them.
Its schema is registered as number 2. Each entry has:

| Field | Meaning |
| --- | --- |
| `name` | A short name |
| `prefix` | An `https://` URL ending in `/` |
| `official` | `true` for the one registry that assigns numbers; the others mirror it |
| `description` | Optional |

A registry serves, under its prefix:

- `<provisional id>.lin`: the schema in LIN, with its header naming the LIN meta-schema (`LIN 1 1`);
- `<provisional id>.json`: the same schema as JSON;
- `index.lin` and `index.json`: one row per schema with its `number` (or `~`), `provisional` id,
  full `sha256` in hex, `$id` and `title`. Its schema is registered as number 3.

The official registry is this repository's [`schemas/`](../schemas/) directory, served at
`https://raw.githubusercontent.com/david-brandt-the-icelander/lean-indented-notation/main/schemas/`.
Its index is the record of every number, kept in the repository whether or not a schema's own
files are. It is free to use and to mirror; a mirror copies the index and never assigns numbers.

Numbers assigned so far:

| Number | Schema |
| --- | --- |
| 1 | The LIN meta-schema: JSON Schema 2020-12 (`https://json-schema.org/draft/2020-12/schema`) |
| 2 | The schema of `registries.lin` |
| 3 | The schema of a registry's `index.lin` |

The seven JSON Schema vocabularies the meta-schema includes, and the spec's example schema, are in
the registry under their provisional ids only: no file's header names them.

## Lookup

A reader resolving a header's schema:

1. For a registered number, finds its row in an index: one beside the file or above it, then one
   built into the tool, then the official registry's, fetched and cached. The row gives the
   provisional id and full SHA-256.
2. Looks for `<provisional id>.lin` or `.json` in local stores (`LIN_SCHEMA_PATH`, a `schemas/`
   directory or registry directory beside the file or above it), then its cache, then the
   header's URL, then each registry.
3. Reads what it finds, and rejects it unless its provisional id matches and, for a number, its
   SHA-256 matches the index.

A reader needs the schema only to type the file's values (spec section 7). Tools that only show
data, such as `lin grep` and `lin outline`, read a file by the schema-less rules when its schema
cannot be found, and say so.

## Registering

```bash
lin schema publish my-schema.lin --dir schemas    # add it under its provisional id
lin schema register my-schema.lin --dir schemas   # ...and give it the next number
lin schema upgrade .                               # rewrite headers to use the number
```

To register a schema in the official registry, open a pull request to this repository with the
files `lin schema register` writes.
