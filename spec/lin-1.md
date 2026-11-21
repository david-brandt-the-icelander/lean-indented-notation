# LIN: Lean Indented Notation

## 1. Purpose and scope

LIN (Lean Indented Notation, `.lin`) is a text format for structured data that AI agents read and write. It holds exactly what JSON holds, in fewer tokens, and stays readable and editable as plain text.

- **In scope:** the text syntax, an optional split form that moves long values after a separator, the mapping to and from JSON, and the tools agents use to read only part of a file.
- **Out of scope:** binary and compressed encodings, which cost agents more tokens rather than fewer (Appendix A).
- **Data model:** every LIN file maps to exactly one JSON document, and JSON Schema validates it unchanged.

Keywords MUST, SHOULD and MAY are used as in RFC 2119.

## 2. Design goals

On a random sample of 124 permissively licensed JSON files from 87 GitHub projects, LIN takes 58.9% of the tokens of the same data as 2-space-indented JSON, against 69.9% for minified JSON and 84.5% for YAML (Appendix B). LIN is smaller than minified JSON in 110 of the 124 files, and gains most on large files of same-shaped records, which become tables. LIN is written for agents, not people: any rule that saves agent tokens is preferred over one that is easier for people to read.

1. **Fewest tokens.** No brackets around objects and lists, no colons or commas; quotes only where a value would be misread; keys written once per table.
2. **Line-oriented.** Each key starts its own line, except the fields of a short inline object, so diffs show what changed and agents can edit with exact-match text tools, `grep`, `sed` and `awk`.
3. **Readable without tools.** No encoding, compression or numeric references.
4. **No guessing of types.** The schema named in the header, not the look of a value, decides whether `no` is a string or a boolean.
5. **Read only what is needed.** A split form whose head omits long values (51% of the JSON tokens on the sample), and generated outlines (8%), so agents rarely load whole files.
6. **Merge-safe.** Nothing in a file counts lines, bytes or positions, so two branches can add entries without conflicting.

The figures use OpenAI's `o200k_base` tokenizer as a stand-in; Claude's tokenizer will give different absolute numbers.

## 3. File layout

A LIN file is a header line, a head, and an optional data section after a `---` line. A file with no data section is in **plain form**; one with a data section is in **split form**.

```
LIN 1 sha256-M804…yHos https://schemas.example.org/plugin-registry.lin   ← header
name example-app           ← head: structure and short values
description @              ← value lives in the data section
---                        ← separator, alone on its line
@description A small app that shows how LIN stores…
```

**The header line** is `LIN <version> [<schema> [<url>]]`, with single spaces between fields:

- `LIN` and `<version>`, the format's major version (`1`), are required. Only the major version is written: a minor version MUST NOT change how an existing file reads, so a reader never needs it.
- `<schema>` is the SHA-256 of the file's schema, written as sha256- followed by 43 base64url characters; section 7 says how it is computed. It identifies both the kind of document and its version, so the data needs no `$schema` or version key. A file without it has no schema.
- `<url>` is an `https://` location the schema can be fetched from. It is optional and only allowed after `<schema>`; the hash, not the URL, identifies the schema.
- The header costs 4 tokens without a schema and about 36 with one, plus about 10 for a typical URL.

**Other layout rules**

- The file MUST be UTF-8 without a byte-order mark, with LF line ends and a final newline. The extension is `.lin`.
- There is no length or offset header. A reader that wants only the head stops at the first line that is exactly `---` (`sed '/^---$/q' file`). No line of the head can be exactly `---`: no bare key starts with `-` (section 4), and a line that starts with `-` is a list item, which is `-` alone or `-` followed by a space, `[` or `{`.

## 4. Core syntax

Each line is a key, optionally followed by one space and a value; indentation shows nesting.

```
title Repository backlog      ← key, one space, value to end of line
origin                        ← key alone: its children follow, indented
 kind conversation
 date 2026-11-10
tags []                       ← empty list
extra {}                      ← empty object
replacedBy ~                  ← null
```

- **Indentation** is exactly 1 space per level. Tabs MUST NOT be used for indentation.
- **Keys** are written bare when they are not empty, contain no whitespace, no control character and none of `"` `[` `]` `{` `}` `|`, and do not start with `-`, `+` or `#`. So `name`, `200`, `*.yml`, `@types/node` and `src/main.ts` are bare keys. Any other key is written as a JSON string, for example `"two words" value` or `"#tag" value`. The excluded characters are LIN's own: `-` and `+` start list items, `[` and `{` start inline lists, tables and inline objects, `|` separates cells, and a line starting with `#` would look like a comment.
- **Values** run from after the single space to the end of the line, taken literally. Section 6 says when a value must be quoted.
- **A key with no value** introduces children on the following lines, indented one level. If the first child line starts with ` -  `, the key holds a list; otherwise it holds an object. An object of scalars can instead go on the key's own line, as `size{} w 640|h 480` (section 5).
- **Literals:** `~` is null, `[]` an empty list, `{}` an empty object. A string that is exactly one of these is quoted.
- **Key order** is kept as written and maps to JSON key order. Duplicate keys in one object MUST be rejected.
- **Blank lines** outside multi-line values are ignored. Writers SHOULD NOT emit them.
- **Comments** are not part of LIN 1, because JSON cannot carry them through a round trip.

## 5. Lists, tables and inline objects

A list of scalars can go on one line as `key[] a|b|c`; a list of flat, same-shaped objects is a table, `key[col|col]`; any other list has one item per line, starting at the key's indentation with ` -  ` for a scalar, ` +  ` for an object, or `-` alone for a nested list. An object of scalars can also go on one line, as `key{} a 1|b 2`.

```
lib[] dom|dom.iterable|esnext  ← inline list
criteria                       ← the same kind of list, one item per line
- Every sample file converts to LIN and back unchanged.
- 2
authors                        ← list of objects
+ name Ada                     ← first key on the + line
 email ada@example.org         ← further keys one level deeper than the +
 roles[] maintainer
matrix                         ← list of lists
-[] 1|2|3                      ← a nested list, inline
-
 - A nested list written one item per line.
files[path|size]               ← table: header names the columns
 src/main.ts|2048              ← one row per object, one level deeper
 README.md|512
window                         ← object
 size{} width 640|height 480   ← inline object: key, space and value per field
```

- **Inline lists:** a list of scalars MAY be written as `key[] item|item|…`, or `-[] item|…` as a list item. Which lists go inline is the writer's choice (section 8, Writer's limits); the recommended default is every list of scalars, because one item per line costs about 3 tokens an item and inline costs about 1. Writers MAY keep long items on their own lines for readable diffs.
- **Inline objects:** an object whose values are all scalars MAY be written as `key{} key value|key value|…`, or `-{} key value|…` as a list item. Each field is a key (bare or a JSON string), one space and a value, written as a cell (below); a field's value MUST NOT be empty, and in split form it can be `@` (section 8). `key {}`, with a space, is still the empty object. Which objects go inline is the writer's choice (section 8, Writer's limits). Inlining saves a token for each field that would otherwise have its own line at depth 2 or more, where indentation costs a token, and saves nothing at depth 0 or 1.
- **Tables:** a list MUST be written as `key[col|col|…]` followed by rows indented one level when it has at least 2 items, every item is a non-empty object with the same keys in the same order, every key is a bare key, and every value is a scalar.
- **Root lists:** a root list can be a table, with the header `[col|col|…]` as the first line after the header line and rows indented one level, or an inline list, `[] item|item|…`. `[]` alone is an empty root list.
- **Item forms** for all other lists: `- value` is a scalar (section 6); `- |` starts a multi-line scalar; `+ key …` starts an object, whose first line is written as it would be inside the object and whose other keys are indented one level deeper than the `+`; `-{} key value|…` is an inline object; `-` alone starts a nested list on the following lines, indented one level; `- []` and `- {}` are empty containers.
- **Why two item markers:** with one marker, `- A sentence` could be a string or an object with the key `A`. ` -  ` always starts a scalar and ` +  ` always starts an object, so the reader never guesses. Both cost one token.
- **Cells, inline items and inline-object fields** are separated by `|`, and their values are written by the value rules of section 6. They are also quoted when they contain `|`, and a multi-line string in a cell or field is written as a JSON string with `\n` escapes.
- **A row** MUST have exactly as many cells as the header has columns, and a cell MUST NOT be empty (an empty string is `""`).

## 6. Strings

Values are bare text unless they could be misread; then they are written as a JSON string, and multi-line text goes in an indented block.

A string MUST be written as a JSON string (with JSON escapes) when it:

- is empty, starts with a space, `"` or `@`, or ends with a space;
- is exactly `~`, `[]`, `{}` or `|`;
- contains a control character (U+0000–U+001F, U+007F, U+0085) or U+2028 or U+2029, other than newlines in a block;
- reads as a number, `true` or `false` where that type is allowed (always, in a file without a schema; section 7);
- is a cell (a table cell, an inline list item or the value of an inline-object field) containing `|`.

Everything else, including colons, `#`, `-`, `+`, quotes inside the text and non-ASCII characters, is written bare.

**Multi-line values** are written as `key |` (or `- |` in a list), followed by the lines indented one level past the key or dash:

```
notes |
 First line.

 Third line, after a blank one.
```

- The value is the lines with that indentation removed, joined by LF. Empty lines in the value are written as empty lines.
- The block ends at the first non-empty line indented less than the block. Empty lines at its end belong to what follows, not to the value.
- A value that starts or ends with a newline, or has a line ending in a space, is written as a JSON string instead.

## 7. Types and schemas

A scalar's type comes from the schema named in the header, never from how the value looks. `status no` is the string `no` unless the schema says `status` is a boolean.

| Schema type at that position | Bare text accepted | Result |
| --- | --- | --- |
| `string` | anything | the text |
| `integer`, `number` | JSON number syntax, such as `3` or `-1.5e3` | the number |
| `boolean` | `true` or `false` | the boolean |
| any type | `~` | null |
| a union, such as `string` or `number` | JSON number or boolean syntax | the number or boolean; a string that looks like one MUST be quoted |

- **Files without a schema** use the same rule as a union of all types: bare `true`, `false` and JSON numbers are read as those types, and writers MUST quote strings that look like them.
- **Numbers** are written in the shortest form JSON produces for them, which is the form ECMAScript's number-to-string conversion gives and RFC 8785 uses: `1.10` is written `1.1`, `6.0` is written `6`, and `1e-05` is written `0.00001`. Integers keep all their digits. A value whose exact digits matter belongs in a string.

**Schemas**

- A schema is a JSON Schema (draft 2020-12) document written in LIN. The meta-schema that schemas follow is published with this spec in LIN (Appendix C); a schema's header names its hash.
- **Normalizing:** the schema's hash is computed from its data, not its text. Read the schema (LIN or JSON) into its JSON value, serialize that value with the JSON Canonicalization Scheme (RFC 8785), and take the SHA-256 of the resulting UTF-8 bytes. Write it as `sha256-` followed by the 32-byte digest in base64url without padding (RFC 4648 section 5), 43 characters. The header line is not part of the data, so it is not hashed, and a schema can name its own hash, as the meta-schema does. The same schema has the same hash whether written in LIN or JSON, plain or split, or with any writer's choices.
- **Finding a schema:** the hash is the schema's content address in a public content-addressed registry, reached at an `https://` prefix followed by the content address (`https://<registry>/sha256-…`). The registry is defined separately from this spec; like a package registry, it is meant to be free to use. A reader looks the hash up in its local cache, then at the header's URL if one is given, then in the registry.
- **Checking a schema:** a reader MUST normalize and hash whatever it fetches, and MUST reject it if the hash differs from the header. A reader that cannot obtain the schema MUST NOT read the file without it, unless its application has explicitly chosen to read without a schema.

## 8. Split form: data section and references

In split form, long values move to the data section and the head holds `@` in their place; each moved value is found by a name built from its owner's `id` and its path, never by a number. Appendix B gives the measured cost of a head.

```
plugins
+ id spell-check
 title Spell check
 summary @
---
@spell-check.summary Underlines misspelt words as you type and offers corrections.
```

**References**

- In the head, a value of bare `@` means "in the data section". Its name is `<owner>.<path>`: `<owner>` is the `id` of the nearest enclosing object whose `id` is a string or integer that can be a name segment, and `<path>` is the keys and list positions from that object down to the value (`spell-check.summary`, `spell-check.sources.0.path`).
- If no enclosing object has such an `id`, the name is the path from the root (`description`, `plugins.3.summary`). Positions change when items are inserted, so data meant to be split SHOULD give its list items an `id`.
- **Segments MUST NOT contain dots**, spaces or other whitespace, `"`, `|` or `@`, and there is no quoting. An `id` containing one of these does not make its object an owner, and a value whose name would need one stays in the head.
- A head MAY name a reference explicitly (`summary @spell-check.summary~2`); writers do this only when two values would get the same name, such as two objects with the same `id`.
- Every name MUST be unique, every `@` in the head MUST have exactly one data entry, and every data entry exactly one `@`.

**Data entries**

- An entry is a line starting with `@<name>`, followed by a space and a value (section 6), by `  | ` and a multi-line block, or by nothing and a list indented one level.
- Entries appear in the order their references appear in the head.

**Writer's limits**

Which values move to the data section, and which lists and objects go inline (section 5), is the writer's choice. A reader needs no setting to read either, so the choice MAY differ from value to value within one file.

- A writer SHOULD choose the limits that lead to the fewest tokens likely to be consumed unnecessarily: tokens an agent reads without needing them. How it judges that, for example from how its files are read, is up to the writer.
- **Recommended default for split form:** move any string longer than 24 characters, and any list of scalars with a string item longer than 24 characters. Keep in the head the values under `id`, `kind`, `title`, `name`, `status` and `type`, and all numbers, booleans and nulls.
- **Recommended default for inline lists:** write every list of scalars inline (section 5). On the Appendix B sample this saves 1.0 points over inlining only lists whose items are 24 characters or less.
- **Recommended default for inline objects:** write an object inline when all its values are scalars, its fields fit in 80 characters, and inlining takes at least one field off a line of its own at depth 2 or more. On the Appendix B sample this saves 0.7 points. At depth 0 and 1 inlining saves nothing, and the 80-character cap keeps grep output and diffs short.
- Objects, and lists containing objects or lists, are never moved. Their short fields stay in the head, so the head always shows the whole structure.

## 9. Mapping to and from JSON

Every LIN file reads as exactly one JSON value, and converting JSON → LIN → JSON returns the same document. Writers may lay out the same data in more than one valid way (plain or split form, and the limits of section 8), but every layout reads back to the same value.

- **Reading:** parse the head, replace each `@` with its data entry's value, and apply the schema's types (section 7).
- **Writing:** key order as in the source; tables wherever section 5 requires them; quotes only where section 6 requires them; inline lists, inline objects and moved values by the writer's limits (section 8).
- **Root:** the root MUST be an object or an array. A root array is written as items at indentation 0.
- **Not representable:** duplicate keys (rejected, as in strict JSON) and comments (JSON has none).

## 10. Tooling

One command, `lin`, gives agents cheap partial views; views are generated on demand and never stored, so they cannot go stale or conflict in a merge.

| Command | Prints |
| --- | --- |
| `lin index [paths…]` | one line per object that has an `id`: file, `id`, and its `title` or `name` |
| `lin outline <file>` | keys and nesting, with list lengths, and no values |
| `lin head <file>` | the head of a split-form file |
| `lin get <file> <name>` | one object (by `id`) or one value (by reference name), fully resolved |
| `lin to-json <file>` / `lin from-json <file>` | the conversion in section 9, plain or split |
| `lin check [paths…]` | syntax errors, broken MUST rules, schema errors, and schemas whose hash does not match the header |
| `lin hash <schema>` | the normalized SHA-256 of a schema file, for the header (section 7) |

- Agents SHOULD find things with `lin index` or `lin outline`, read them with `lin get`, and open whole files only to edit them.
- A file that fails `lin check` MUST NOT be committed.
- A reference reader and writer (about 450 lines of Python) round-trips every file in the Appendix B sample.

## 11. Editing rules and git

Agents edit LIN as ordinary text, one line at a time, and run `lin check` before committing.

1. **Change a short value** by replacing its line, quoting the whole line as the match text, for example `  status open ` → `  status done `.
2. **Change a moved value** by replacing its `@<name>` line in the data section; the head does not change.
3. **Add an entry** by inserting its lines in the head and its data entries in the order section 8 gives. Because references are named, nothing else in the file changes.
4. **Move a value between head and data** by hand or with `lin from-json`; any placement allowed by the writer's limits is valid (section 8).
5. **Run `lin check`** on every changed file, then the repository's validators.

In git, a change shows as the changed lines only, and every data line names its owner. Two branches that add different entries conflict only if they touch neighbouring lines, as in any text file.

## 12. Full example

A plugin registry, its schema, and the registry in both LIN forms. Every LIN text here was produced by the reference writer and read back to identical JSON, and the hashes were computed as section 7 describes.

**The data, as JSON:**

```
{"name": "example-registry",
 "version": "2.3.0",
 "description": "Plugins the example editor loads at start-up, in load order.",
 "plugins": [
  {"id": "spell-check", "title": "Spell check", "enabled": true, "priority": 10,
   "summary": "Underlines misspelt words as you type and offers corrections.",
   "tags": ["text", "language"],
   "sources": [{"path": "src/spell.ts", "lines": 120}, {"path": "dict/en.txt", "lines": 5000}]},
  {"id": "word-count", "title": "Word count", "enabled": false, "priority": "20",
   "summary": "Shows the number of words in the document and the selection.",
   "notes": "Off by default.\nTurn it on in Settings.",
   "tags": []}
 ]}
```

**The schema, in LIN** (`plugin-registry.lin`). Its header names the JSON Schema 2020-12 meta-schema, whose normalized hash is `sha256-wcrP…H4Ars`; the schema's own normalized hash is `sha256-M804…yHos`.

```
LIN 1 sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars
$schema https://json-schema.org/draft/2020-12/schema
title Plugin registry
type object
required[] name|plugins
properties
 name{} type string
 version{} type string
 description{} type string
 plugins
  type array
  items
   type object
   required[] id|title
   properties
    id{} type string|pattern ^[a-z0-9-]+$
    title{} type string
    enabled{} type boolean
    priority
     type[] integer|string
    summary{} type string
    notes{} type string
    tags
     type array
     items{} type string
    sources
     type array
     items
      type object
      properties
       path{} type string
       lines{} type integer
```

**The data in plain form:**

```
LIN 1 sha256-M804eExtUC7S2al_N82_CXtgd3ip0TT4ppYaSQtyHos https://schemas.example.org/plugin-registry.lin
name example-registry
version 2.3.0
description Plugins the example editor loads at start-up, in load order.
plugins
+ id spell-check
 title Spell check
 enabled true
 priority 10
 summary Underlines misspelt words as you type and offers corrections.
 tags[] text|language
 sources[path|lines]
  src/spell.ts|120
  dict/en.txt|5000
+ id word-count
 title Word count
 enabled false
 priority "20"
 summary Shows the number of words in the document and the selection.
 notes |
  Off by default.
  Turn it on in Settings.
 tags []
```

**The data in split form,** with the recommended default limits:

```
LIN 1 sha256-M804eExtUC7S2al_N82_CXtgd3ip0TT4ppYaSQtyHos https://schemas.example.org/plugin-registry.lin
name example-registry
version 2.3.0
description @
plugins
+ id spell-check
 title Spell check
 enabled true
 priority 10
 summary @
 tags[] text|language
 sources[path|lines]
  src/spell.ts|120
  dict/en.txt|5000
+ id word-count
 title Word count
 enabled false
 priority "20"
 summary @
 notes @
 tags []
---
@description Plugins the example editor loads at start-up, in load order.
@spell-check.summary Underlines misspelt words as you type and offers corrections.
@word-count.summary Shows the number of words in the document and the selection.
@word-count.notes |
 Off by default.
 Turn it on in Settings.
```

- `priority "20"` is quoted because the schema allows an integer or a string there, and bare `20` would read as the integer.
- `version 2.3.0` is bare: the schema says it is a string, and it is not number syntax anyway.
- `$schema` in the schema is JSON Schema's own keyword, so it stays in the data; the registry itself needs no `$schema` key, because its header names its schema.

## 13. Grammar summary

The line-level grammar, informally in ABNF; indentation (`IND` = 1 space per level) and the rules in sections 5–8 carry the rest.

```
file          = header LF head [ "---" LF data ]
header        = "LIN " version [ SP schema-hash [ SP url ] ]
version       = 1*DIGIT                     ; the major version, "1"
schema-hash   = "sha256-" 43( ALPHA / DIGIT / "-" / "_" )  ; base64url, no padding
url           = "https://" 1*( any char but space )
head          = [ root-list ] *line
root-list     = "[]" LF                     ; empty root list
              / "[] " cell *( "|" cell ) LF  ; inline root list
              / "[" bare-key *( "|" bare-key ) "]" LF *row  ; root table
line          = IND ( pair / key-only / table-head / inline-list / inline-object / item ) LF
pair          = key SP value
key-only      = key                         ; children follow
table-head    = key "[" bare-key *( "|" bare-key ) "]"  ; rows follow
inline-list   = key "[] " cell *( "|" cell )
inline-object = key "{} " field *( "|" field )
field         = key SP cell                 ; a field of an inline object
row           = IND cell *( "|" cell ) LF
item          = "- " value                  ; scalar, "|" block, [] or {}
              / "+ " ( pair / key-only / table-head / inline-list / inline-object )  ; object
              / "-[] " cell *( "|" cell )   ; nested list of scalars
              / "-{} " field *( "|" field )  ; object of scalars, on one line
              / "-"                         ; nested list follows, one level deeper
value         = bare / json-string / "~" / "[]" / "{}" / "|" / "@" [ name ]
key           = bare-key / json-string
bare-key      = key-first *key-char
key-first     = key-char but not "-", "+" or "#"
key-char      = any char but whitespace, a control char, '"', "[", "]", "{", "}" or "|"
data          = *( "@" name [ SP value ] LF [ block / items ] )
name          = segment *( "." segment )    ; owner id, then keys and positions
segment       = 1*( any char but ".", whitespace, '"', "|", "@" )  ; no quoting
```

## 14. Deferred

These are decided in principle and left to be specified later.

- **The registry.** Schemas are fetched from an `https://` prefix followed by the content address. Which registry, and how it is run so that it can be free to use, like a package registry, will be defined separately.
- **Confirming the figures with Claude's tokenizer.** All figures use `o200k_base` as a stand-in.
- **Licence of the published meta-schema.** Appendix C carries JSON Schema 2020-12's meta-schemas in LIN; check the JSON Schema project's licence terms before publishing them with the spec.

## Appendix A. Why the syntax looks like this: token costs

Each rule in LIN was chosen by counting the tokens of the alternatives. Tokenizers are trained mostly on prose and code, so a common word with its leading space is one token, while punctuation is usually one token per cluster. A format is cheap when it has few punctuation clusters per value, and when those clusters merge with the words or line ends next to them.

| Construct | JSON | Tokens | LIN | Tokens | Why LIN's form |
| --- | --- | --- | --- | --- | --- |
| Key and value, nested | `     "status": "open", ` | 7 | `  status open ` | 3 | Quotes, colon and comma are 4 tokens. A space separator is free: the value's first token carries it (`  open `). At depth 1 the one-space indent merges into the key (`  status `). |
| Key with punctuation | `     "@types/node": "^20", ` | 8 | `  @types/node ^20 ` | 6 | Any key without whitespace or LIN's own punctuation is bare. Quoting this one would cost 1 more token; on the sample, bare keys like it save 0.3%. |
| Separator choice | `description: The owner asked` | 6 | `description The owner asked` | 5 | A colon is a token of its own; a space is not. |
| Same pair, minified JSON | `"status":"open",` | 5 | `status open` | 3 | Minified JSON merges `":"` and `",` into single tokens, which is why it comes close to LIN. |
| Closing brackets | `     }, ` on its own line | 2 | nothing | 0 | Indentation already ends the object. |
| Indentation | `      ` | 1 | 1 space per level | 0–1 | A run of spaces is one token at any width, and a single space merges into the next word's token. So depth 1 costs nothing and every deeper line costs 1. Tabs behave the same, and inline objects exist to take fields off those deeper lines. On the sample, 1 space per level saves 2.6% over 2 spaces. |
| Row of a flat list of objects | `{"path": "src/spell.ts", "lines": 120},` | 16 | `src/spell.ts\|120` | 8 | The table states the keys once, in the header. |
| Small object, nested | `     "size": {"w": 640, "h": 480}, ` | 16 | `  size{} w 640\|h 480 ` | 10 | One field per line costs 12, because each line at depth 2 or more pays a token for its indentation. Inline objects save 1.1% on the sample. |
| Table separator | `,` or tab | 1 | `\|` | 1 | A comma or tab often merges with a short word after it (`,b` is one token), which would save 0.8% on the sample. `\|` is kept because commas are common in text, so many cells would need quotes, and tabs are invisible and often changed by editors and edit tools. |
| Null | `"summary": null,` | 6 | `summary ~` | 4 | `~` is one token, and no quotes or comma. |
| Number with a zero fraction | `     "width": 6.0, ` | 9 | `  width 6 ` | 4 | 6.0 and 6 are the same JSON number, and LIN writes the shortest form, as RFC 8785 does. This saves 0.8% on the sample, nearly all in one file of measurements. |
| Multi-line text | `"Off by default.\nTurn it on…"` | 16 | `notes \|` and indented lines | 15 | `\n` costs 2 tokens as an escape; a real line end often merges with the punctuation before it (`.` + line end is one token). |
| Non-ASCII text | `"été"` (escaped) | 11 | `été` | 1 | Writing characters as themselves avoids escapes, which tokenize as several tokens each. |
| List item markers | `"text",` | 5 | `- text` / `+ id …` | 5 | One item per line costs 3 tokens an item (indent, marker, line end), so short items go inline as key\[\] a\|b\|c, at 1 token an item, as in minified JSON. Long items keep their own lines, so a diff shows which one changed. |
| Version line | — | — | `LIN 1` | 4 | The version alone would cost 2 (`1` and a line end), and `1.0.2` costs 6 because each digit and dot is a token. `LIN 1` costs 4 and makes the file recognizable. A schema hash adds about 32 tokens as sha256- and base64url, against about 37 in hex. |
| Head separator | — | — | `---` | 1 | `---` plus its line end is a single token. |
| Reference | — | — | `summary @` | 4 | `@` is one token; a name (`@root-views.summary`) would repeat what the position already says. |

Token counts include the line end, and use OpenAI's `o200k_base` tokenizer as a stand-in for Claude's.

**What was rejected, and why**

- **YAML's `key: value`.** The colon adds a token per line, and YAML guesses types from how values look (`no` becomes `false`). LIN takes types from the schema.
- **Base64 or compressed values.** Measured at 2.5 times the tokens of plain text on the Appendix B sample: the encoded characters look random, so they split into tokens of 1–2 characters. Compressing a whole file and then encoding it does save tokens, but the agent cannot read the result, and decoding it puts the plain text in its context anyway.
- **Numeric references into the data section.** They cost about the same as names but break when values are inserted or removed, and two branches adding values conflict.
- **A length header for the head.** It must be updated on every edit; a `---` line costs 1 token and never goes stale.
- **Dropping list markers.** A bare `   text ` line saves nothing over `   - text ` in most cases and would make items indistinguishable from keys.
- **Other hash encodings.** Hex costs about 37 tokens, base32 about 32, and `sha256-` with base64url about 32 (bare base64url about 30). Base85 and similar alphabets are shorter but contain characters that `grep`, `sed`, `awk` and the shell treat as special. The `sha256-` prefix costs 2 tokens and is kept: it names the algorithm, so another can be added later, and it stops a hash that starts with `-` from being read as a command-line option. Hashes cut to 128 or 192 bits would save 8–15 tokens but would no longer match the full SHA-256 that content-addressed registries index by.
- **Folded paths (`a.b.c value`).** Writing an object that has one key on its parent's line would save 0.5% on the sample. But keys with dots, such as `package.json` or `eslint.enable`, would then need quotes, and an agent adding a key beside a folded one would create a duplicate.
- **Sparse tables.** Empty cells for missing keys would save 0.04%: lists whose items have different keys usually hold nested values too, so they could not be tables anyway.
- **Tables with nested fields under each row.** Moving the shared scalar keys of mixed objects into a header would save 0.4%, at the cost of a third kind of list for agents to learn.
- **Tables of records keyed by name**, for an object whose values are same-shaped objects. They would save 249 tokens on the whole sample.
- **Unindented rows in root tables.** They would save 0.5%, nearly all in one file, and give root tables an indentation rule of their own.

## Appendix B. Benchmark

LIN was measured on 124 JSON files drawn at random from 87 GitHub projects under permissive licences, 1.48 MB in all. Every file converted to LIN and back to identical JSON, in plain and split form.

**Method**

1. Pick projects at random with GitHub's repository search: a random licence among MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unlicense and CC0-1.0, a random star range from 20 to 30,000, a random results page, and up to 3 projects per page.
2. In each project, list `.json` files of 200 bytes to 300 KB with GitHub's code search, skip lock files, `node_modules` and minified files, and take 1 or 2 at random that parse as strict JSON.
3. Convert each file with the reference writer (1-space indentation, inline lists for every list of scalars, inline objects by the 80-character default, the 24-character split default) and count tokens with `o200k_base`. Percentages compare totals over all files with the same data as 2-space-indented JSON.

The files were downloaded for measurement only and are not redistributed; the list below links each one.

**Results**

| Form of the same data | Tokens | vs. 2-space JSON |
| --- | --- | --- |
| JSON, 2-space indent | 537,563 | 100% |
| JSON as found in the projects | 505,326 | 94.0% |
| YAML | 454,113 | 84.5% |
| JSON, minified | 375,780 | 69.9% |
| LIN, plain form, 2-space indent | 325,073 | 60.5% |
| **LIN, plain form** | **316,592** | **58.9%** |
| LIN, split form: head only | 276,114 | 51.4% |
| `lin outline` view | 43,856 | 8.2% |
| The values alone, with no keys or syntax | 261,519 | 48.6% |

**By file size** (2-space JSON tokens per file)

| File size | Files | Minified JSON | LIN, plain form |
| --- | --- | --- | --- |
| Under 1,000 tokens | 90 | 70.6% | 65.1% |
| 1,000–10,000 tokens | 23 | 73.2% | 69.5% |
| Over 10,000 tokens | 11 | 69.3% | 56.6% |

- LIN is smaller than minified JSON in 110 of the 124 files (median: 91% of minified). It gains most on large files, which are mostly lists of same-shaped records that become tables.
- Root-level tables and inline lists came from this sample. Before them, plain LIN cost 77.5% on most of these files, more than minified JSON, because two large files are root-level lists. One-space indentation took it from 61.8% to 60.2%, and inline objects, looser bare keys and numbers in their shortest form took it to 58.9%.
- Encoding each value in base64 cost 2.5 times its plain text.
- The sample is small and depends on what GitHub search returns. Formats rich in prose would split better; numeric data gains least from LIN.

**Sources** (repository, file at the time of sampling, and its licence as GitHub reports it)

| # | Repository | File | License |
| --- | --- | --- | --- |
| 1 | [71/stadiacontroller](https://github.com/71/stadiacontroller) | [.vscode/launch.json](https://github.com/71/stadiacontroller/blob/master/.vscode/launch.json) | ISC |
| 2 | [adobe/lagrange](https://github.com/adobe/lagrange) | [cmake/recipes/external/MKL.json](https://github.com/adobe/lagrange/blob/main/cmake/recipes/external/MKL.json) | Apache-2.0 |
| 3 | [affinipay/react-bootstrap-autosuggest](https://github.com/affinipay/react-bootstrap-autosuggest) | [demo/manifest.json](https://github.com/affinipay/react-bootstrap-autosuggest/blob/master/demo/manifest.json) | ISC |
| 4 | [affinipay/react-bootstrap-autosuggest](https://github.com/affinipay/react-bootstrap-autosuggest) | [package.json](https://github.com/affinipay/react-bootstrap-autosuggest/blob/master/package.json) | ISC |
| 5 | [albertomosconi/foss-apps](https://github.com/albertomosconi/foss-apps) | [apps/programming.json](https://github.com/albertomosconi/foss-apps/blob/main/apps/programming.json) | CC0-1.0 |
| 6 | [albertomosconi/foss-apps](https://github.com/albertomosconi/foss-apps) | [apps/utilities.json](https://github.com/albertomosconi/foss-apps/blob/main/apps/utilities.json) | CC0-1.0 |
| 7 | [Anjaxs/WebStack-vue](https://github.com/Anjaxs/WebStack-vue) | [package.json](https://github.com/Anjaxs/WebStack-vue/blob/master/package.json) | MIT |
| 8 | [Anjaxs/WebStack-vue](https://github.com/Anjaxs/WebStack-vue) | [src/assets/data.json](https://github.com/Anjaxs/WebStack-vue/blob/master/src/assets/data.json) | MIT |
| 9 | [apiture/openapi-down-convert](https://github.com/apiture/openapi-down-convert) | [package.json](https://github.com/apiture/openapi-down-convert/blob/main/package.json) | ISC |
| 10 | [apiture/openapi-down-convert](https://github.com/apiture/openapi-down-convert) | [tsconfig.json](https://github.com/apiture/openapi-down-convert/blob/main/tsconfig.json) | ISC |
| 11 | [arpanghosh8453/garmin-grafana](https://github.com/arpanghosh8453/garmin-grafana) | [Grafana\_Dashboard/Garmin-Strength-Training-Dashboard.json](https://github.com/arpanghosh8453/garmin-grafana/blob/main/Grafana_Dashboard/Garmin-Strength-Training-Dashboard.json) | BSD-3-Clause |
| 12 | [aws-actions/aws-codebuild-run-build](https://github.com/aws-actions/aws-codebuild-run-build) | [.eslintrc.json](https://github.com/aws-actions/aws-codebuild-run-build/blob/main/.eslintrc.json) | Apache-2.0 |
| 13 | [awslabs/aws-config-rules](https://github.com/awslabs/aws-config-rules) | [python/IAM\_IP\_RESTRICTION/parameters.json](https://github.com/awslabs/aws-config-rules/blob/master/python/IAM_IP_RESTRICTION/parameters.json) | CC0-1.0 |
| 14 | [BenoitZugmeyer/eslint-plugin-html](https://github.com/BenoitZugmeyer/eslint-plugin-html) | [package.json](https://github.com/BenoitZugmeyer/eslint-plugin-html/blob/main/package.json) | ISC |
| 15 | [beto-rodriguez/Manuela](https://github.com/beto-rodriguez/Manuela) | [templates/content/HybridMauiTemplate/.template.config/template.json](https://github.com/beto-rodriguez/Manuela/blob/master/templates/content/HybridMauiTemplate/.template.config/template.json) | MIT |
| 16 | [bitwalker/distillery](https://github.com/bitwalker/distillery) | [test/fixtures/umbrella\_app/apps/web/assets/package.json](https://github.com/bitwalker/distillery/blob/master/test/fixtures/umbrella_app/apps/web/assets/package.json) | MIT |
| 17 | [ciromattia/kcc](https://github.com/ciromattia/kcc) | [kcc.json](https://github.com/ciromattia/kcc/blob/master/kcc.json) | ISC |
| 18 | [codigoencasa/builderbot](https://github.com/codigoencasa/builderbot) | [packages/bot/tsconfig.json](https://github.com/codigoencasa/builderbot/blob/builderbot/packages/bot/tsconfig.json) | MIT |
| 19 | [codigoencasa/builderbot](https://github.com/codigoencasa/builderbot) | [packages/provider-tiktok/tsconfig.json](https://github.com/codigoencasa/builderbot/blob/builderbot/packages/provider-tiktok/tsconfig.json) | MIT |
| 20 | [CrimsonForge-io/king-phisher](https://github.com/CrimsonForge-io/king-phisher) | [data/client/king\_phisher/client\_config.json](https://github.com/CrimsonForge-io/king-phisher/blob/master/data/client/king_phisher/client_config.json) | BSD-3-Clause |
| 21 | [CrimsonForge-io/king-phisher](https://github.com/CrimsonForge-io/king-phisher) | [data/client/king\_phisher/completion/jinja.json](https://github.com/CrimsonForge-io/king-phisher/blob/master/data/client/king_phisher/completion/jinja.json) | BSD-3-Clause |
| 22 | [danielsaidi/DeckKit](https://github.com/danielsaidi/DeckKit) | [Demo/Demo/Resources/AppIcon.icon/icon.json](https://github.com/danielsaidi/DeckKit/blob/main/Demo/Demo/Resources/AppIcon.icon/icon.json) | MIT |
| 23 | [dankeboy36/esp-exception-decoder](https://github.com/dankeboy36/esp-exception-decoder) | [.vscode/settings.json](https://github.com/dankeboy36/esp-exception-decoder/blob/main/.vscode/settings.json) | MIT |
| 24 | [dankeboy36/esp-exception-decoder](https://github.com/dankeboy36/esp-exception-decoder) | [package.json](https://github.com/dankeboy36/esp-exception-decoder/blob/main/package.json) | MIT |
| 25 | [darkain/php-tracking-urls](https://github.com/darkain/php-tracking-urls) | [composer.json](https://github.com/darkain/php-tracking-urls/blob/main/composer.json) | BSD-2-Clause |
| 26 | [DarkaOnLine/L5-Swagger](https://github.com/DarkaOnLine/L5-Swagger) | [composer.json](https://github.com/DarkaOnLine/L5-Swagger/blob/master/composer.json) | MIT |
| 27 | [DarkaOnLine/L5-Swagger](https://github.com/DarkaOnLine/L5-Swagger) | [tests/Unit/Fixtures/api-docs.json](https://github.com/DarkaOnLine/L5-Swagger/blob/master/tests/Unit/Fixtures/api-docs.json) | MIT |
| 28 | [davidmerfield/randomColor](https://github.com/davidmerfield/randomColor) | [package.json](https://github.com/davidmerfield/randomColor/blob/master/package.json) | CC0-1.0 |
| 29 | [dbt-labs/jaffle\_shop\_duckdb](https://github.com/dbt-labs/jaffle_shop_duckdb) | [.devcontainer.json](https://github.com/dbt-labs/jaffle_shop_duckdb/blob/duckdb/.devcontainer.json) | Apache-2.0 |
| 30 | [deldersveld/PowerBI-ThemeTemplates](https://github.com/deldersveld/PowerBI-ThemeTemplates) | [GlobalLevelTemplate.json](https://github.com/deldersveld/PowerBI-ThemeTemplates/blob/master/GlobalLevelTemplate.json) | Unlicense |
| 31 | [deldersveld/PowerBI-ThemeTemplates](https://github.com/deldersveld/PowerBI-ThemeTemplates) | [LineStackedColumnCombo.json](https://github.com/deldersveld/PowerBI-ThemeTemplates/blob/master/LineStackedColumnCombo.json) | Unlicense |
| 32 | [django-commons/django-cookie-consent](https://github.com/django-commons/django-cookie-consent) | [cookie\_consent/fixtures/common\_cookies.json](https://github.com/django-commons/django-cookie-consent/blob/main/cookie_consent/fixtures/common_cookies.json) | BSD-2-Clause |
| 33 | [django-commons/django-cookie-consent](https://github.com/django-commons/django-cookie-consent) | [js/package.json](https://github.com/django-commons/django-cookie-consent/blob/main/js/package.json) | BSD-2-Clause |
| 34 | [DLLXW/baby-llama2-chinese](https://github.com/DLLXW/baby-llama2-chinese) | [chatglm\_tokenizer/tokenizer\_config.json](https://github.com/DLLXW/baby-llama2-chinese/blob/main/chatglm_tokenizer/tokenizer_config.json) | MIT |
| 35 | [dominictobias/react-image-crop](https://github.com/dominictobias/react-image-crop) | [package.json](https://github.com/dominictobias/react-image-crop/blob/master/package.json) | ISC |
| 36 | [dominictobias/react-image-crop](https://github.com/dominictobias/react-image-crop) | [tsconfig.json](https://github.com/dominictobias/react-image-crop/blob/master/tsconfig.json) | ISC |
| 37 | [dynamoose/dynamoose](https://github.com/dynamoose/dynamoose) | [packages/dynamoose-utils/package.json](https://github.com/dynamoose/dynamoose/blob/main/packages/dynamoose-utils/package.json) | Unlicense |
| 38 | [dynamoose/dynamoose](https://github.com/dynamoose/dynamoose) | [publish/package.json](https://github.com/dynamoose/dynamoose/blob/main/publish/package.json) | Unlicense |
| 39 | [ErlichLiu/DeepClaude](https://github.com/ErlichLiu/DeepClaude) | [app/model\_manager/model\_configs.json](https://github.com/ErlichLiu/DeepClaude/blob/main/app/model_manager/model_configs.json) | MIT |
| 40 | [fasttime/JScrewIt](https://github.com/fasttime/JScrewIt) | [package.json](https://github.com/fasttime/JScrewIt/blob/main/package.json) | ISC |
| 41 | [fasttime/JScrewIt](https://github.com/fasttime/JScrewIt) | [packages/\~result-format/package.json](https://github.com/fasttime/JScrewIt/blob/main/packages/~result-format/package.json) | ISC |
| 42 | [firtoz/GPT-Shell](https://github.com/firtoz/GPT-Shell) | [.changeset/config.json](https://github.com/firtoz/GPT-Shell/blob/main/.changeset/config.json) | MIT |
| 43 | [firtoz/GPT-Shell](https://github.com/firtoz/GPT-Shell) | [package.json](https://github.com/firtoz/GPT-Shell/blob/main/package.json) | MIT |
| 44 | [for-GET/know-your-http-well](https://github.com/for-GET/know-your-http-well) | [json/relations.json](https://github.com/for-GET/know-your-http-well/blob/master/json/relations.json) | Unlicense |
| 45 | [for-GET/know-your-http-well](https://github.com/for-GET/know-your-http-well) | [json/status-codes.json](https://github.com/for-GET/know-your-http-well/blob/master/json/status-codes.json) | Unlicense |
| 46 | [fqborges/webpack-fix-style-only-entries](https://github.com/fqborges/webpack-fix-style-only-entries) | [package.json](https://github.com/fqborges/webpack-fix-style-only-entries/blob/master/package.json) | ISC |
| 47 | [gokrazy/gokrazy](https://github.com/gokrazy/gokrazy) | [website/themes/hugo-theme-relearn/assets/js/mathjax/sre/mathmaps/base.json](https://github.com/gokrazy/gokrazy/blob/main/website/themes/hugo-theme-relearn/assets/js/mathjax/sre/mathmaps/base.json) | BSD-3-Clause |
| 48 | [gokrazy/gokrazy](https://github.com/gokrazy/gokrazy) | [website/themes/hugo-theme-relearn/docs/content/shortcodes/openapi/petstore.json](https://github.com/gokrazy/gokrazy/blob/main/website/themes/hugo-theme-relearn/docs/content/shortcodes/openapi/petstore.json) | BSD-3-Clause |
| 49 | [hgaiser/moonshine](https://github.com/hgaiser/moonshine) | [.vscode/launch.json](https://github.com/hgaiser/moonshine/blob/main/.vscode/launch.json) | BSD-2-Clause |
| 50 | [holzschu/a-shell](https://github.com/holzschu/a-shell) | [Resources/Themes/catppuccin-macchiato.json](https://github.com/holzschu/a-shell/blob/master/Resources/Themes/catppuccin-macchiato.json) | BSD-3-Clause |
| 51 | [holzschu/a-shell](https://github.com/holzschu/a-shell) | [Resources/Themes/catppuccin-mocha.json](https://github.com/holzschu/a-shell/blob/master/Resources/Themes/catppuccin-mocha.json) | BSD-3-Clause |
| 52 | [hpc-maths/samurai](https://github.com/hpc-maths/samurai) | [docs/.impeccable/design.json](https://github.com/hpc-maths/samurai/blob/main/docs/.impeccable/design.json) | BSD-3-Clause |
| 53 | [ImVexed/muon](https://github.com/ImVexed/muon) | [examples/create-react-app/public/package.json](https://github.com/ImVexed/muon/blob/master/examples/create-react-app/public/package.json) | MIT |
| 54 | [ImVexed/muon](https://github.com/ImVexed/muon) | [examples/create-react-app/public/public/manifest.json](https://github.com/ImVexed/muon/blob/master/examples/create-react-app/public/public/manifest.json) | MIT |
| 55 | [InitialDet/AutoHook](https://github.com/InitialDet/AutoHook) | [AutoHook/AutoHook.json](https://github.com/InitialDet/AutoHook/blob/main/AutoHook/AutoHook.json) | BSD-3-Clause |
| 56 | [InitialDet/AutoHook](https://github.com/InitialDet/AutoHook) | [AutoHook/Data/FishData/bitetimers.json](https://github.com/InitialDet/AutoHook/blob/main/AutoHook/Data/FishData/bitetimers.json) | BSD-3-Clause |
| 57 | [JakubOnderka/PHP-Console-Color](https://github.com/JakubOnderka/PHP-Console-Color) | [composer.json](https://github.com/JakubOnderka/PHP-Console-Color/blob/master/composer.json) | BSD-2-Clause |
| 58 | [JarryShaw/PyPCAPKit](https://github.com/JarryShaw/PyPCAPKit) | [tests/project/ci\_timing\_fixtures/jobs-37323913186.json](https://github.com/JarryShaw/PyPCAPKit/blob/main/tests/project/ci_timing_fixtures/jobs-37323913186.json) | BSD-3-Clause |
| 59 | [joseluisq/gimage](https://github.com/joseluisq/gimage) | [composer.json](https://github.com/joseluisq/gimage/blob/master/composer.json) | MIT |
| 60 | [jupyterlab-contrib/jupyterlab-spreadsheet-editor](https://github.com/jupyterlab-contrib/jupyterlab-spreadsheet-editor) | [install.json](https://github.com/jupyterlab-contrib/jupyterlab-spreadsheet-editor/blob/main/install.json) | BSD-3-Clause |
| 61 | [jupyterlab-contrib/jupyterlab-spreadsheet-editor](https://github.com/jupyterlab-contrib/jupyterlab-spreadsheet-editor) | [package.json](https://github.com/jupyterlab-contrib/jupyterlab-spreadsheet-editor/blob/main/package.json) | BSD-3-Clause |
| 62 | [Kkevsterrr/geneva](https://github.com/Kkevsterrr/geneva) | [workers/example/worker.json](https://github.com/Kkevsterrr/geneva/blob/master/workers/example/worker.json) | BSD-3-Clause |
| 63 | [leetal/ios-cmake](https://github.com/leetal/ios-cmake) | [example/example-app/example-app/Assets.xcassets/AppIcon.appiconset/Contents.json](https://github.com/leetal/ios-cmake/blob/master/example/example-app/example-app/Assets.xcassets/AppIcon.appiconset/Contents.json) | BSD-3-Clause |
| 64 | [lefticus/cpp\_weekly](https://github.com/lefticus/cpp_weekly) | [PMR/data/apache\_builds.json](https://github.com/lefticus/cpp_weekly/blob/master/PMR/data/apache_builds.json) | Unlicense |
| 65 | [lefticus/cpp\_weekly](https://github.com/lefticus/cpp_weekly) | [PMR/data/numbers.json](https://github.com/lefticus/cpp_weekly/blob/master/PMR/data/numbers.json) | Unlicense |
| 66 | [LemonOSProject/LemonOS](https://github.com/LemonOSProject/LemonOS) | [Base/lemon/resources/themes/default.json](https://github.com/LemonOSProject/LemonOS/blob/master/Base/lemon/resources/themes/default.json) | BSD-2-Clause |
| 67 | [leptos-rs/start-axum-0.6](https://github.com/leptos-rs/start-axum-0.6) | [end2end/package.json](https://github.com/leptos-rs/start-axum-0.6/blob/main/end2end/package.json) | Unlicense |
| 68 | [lloyd/node-compute-cluster](https://github.com/lloyd/node-compute-cluster) | [package.json](https://github.com/lloyd/node-compute-cluster/blob/master/package.json) | ISC |
| 69 | [lmco/dart](https://github.com/lmco/dart) | [missions/fixtures/common\_bas.json](https://github.com/lmco/dart/blob/master/missions/fixtures/common_bas.json) | Apache-2.0 |
| 70 | [lmco/dart](https://github.com/lmco/dart) | [missions/fixtures/common\_classifications.json](https://github.com/lmco/dart/blob/master/missions/fixtures/common_classifications.json) | Apache-2.0 |
| 71 | [lpongetti/flutter\_map\_marker\_cluster](https://github.com/lpongetti/flutter_map_marker_cluster) | [example/ios/Runner/Assets.xcassets/LaunchImage.imageset/Contents.json](https://github.com/lpongetti/flutter_map_marker_cluster/blob/master/example/ios/Runner/Assets.xcassets/LaunchImage.imageset/Contents.json) | BSD-3-Clause |
| 72 | [magnusoverli/opencode](https://github.com/magnusoverli/opencode) | [ha\_opencode/rootfs/opt/openchamber/security-overrides.json](https://github.com/magnusoverli/opencode/blob/main/ha_opencode/rootfs/opt/openchamber/security-overrides.json) | Unlicense |
| 73 | [manosim/django-rest-framework-docs](https://github.com/manosim/django-rest-framework-docs) | [rest\_framework\_docs/static/package.json](https://github.com/manosim/django-rest-framework-docs/blob/master/rest_framework_docs/static/package.json) | BSD-2-Clause |
| 74 | [mapbox/mapbox-gl-directions](https://github.com/mapbox/mapbox-gl-directions) | [package.json](https://github.com/mapbox/mapbox-gl-directions/blob/main/package.json) | ISC |
| 75 | [meta-pytorch/torchrec](https://github.com/meta-pytorch/torchrec) | [torchrec/metrics/tests/metric\_fqn\_golden\_snapshot.json](https://github.com/meta-pytorch/torchrec/blob/main/torchrec/metrics/tests/metric_fqn_golden_snapshot.json) | BSD-3-Clause |
| 76 | [michelegera/tailwindcss-open-template](https://github.com/michelegera/tailwindcss-open-template) | [package.json](https://github.com/michelegera/tailwindcss-open-template/blob/main/package.json) | MIT |
| 77 | [microsoft/CCF](https://github.com/microsoft/CCF) | [samples/config/recover\_config.json](https://github.com/microsoft/CCF/blob/main/samples/config/recover_config.json) | Apache-2.0 |
| 78 | [microsoft/CCF](https://github.com/microsoft/CCF) | [scripts/source-dependencies.json](https://github.com/microsoft/CCF/blob/main/scripts/source-dependencies.json) | Apache-2.0 |
| 79 | [MieMieeeee/ComfyUI-Mie-Package-Launcher](https://github.com/MieMieeeee/ComfyUI-Mie-Package-Launcher) | [config.json](https://github.com/MieMieeeee/ComfyUI-Mie-Package-Launcher/blob/main/config.json) | Apache-2.0 |
| 80 | [MieMieeeee/ComfyUI-Mie-Package-Launcher](https://github.com/MieMieeeee/ComfyUI-Mie-Package-Launcher) | [launcher/config.json](https://github.com/MieMieeeee/ComfyUI-Mie-Package-Launcher/blob/main/launcher/config.json) | Apache-2.0 |
| 81 | [mint-lang/mint](https://github.com/mint-lang/mint) | [runtime/package.json](https://github.com/mint-lang/mint/blob/master/runtime/package.json) | BSD-3-Clause |
| 82 | [mskelton/yarn-plugin-outdated](https://github.com/mskelton/yarn-plugin-outdated) | [.oxfmtrc.json](https://github.com/mskelton/yarn-plugin-outdated/blob/main/.oxfmtrc.json) | ISC |
| 83 | [nginx/nginx-asg-sync](https://github.com/nginx/nginx-asg-sync) | [renovate.json](https://github.com/nginx/nginx-asg-sync/blob/main/renovate.json) | BSD-2-Clause |
| 84 | [ngx-formly/ngx-formly](https://github.com/ngx-formly/ngx-formly) | [demo/src/app/app.menu.json](https://github.com/ngx-formly/ngx-formly/blob/main/demo/src/app/app.menu.json) | MIT |
| 85 | [ngx-formly/ngx-formly](https://github.com/ngx-formly/ngx-formly) | [src/ui/kendo/package.json](https://github.com/ngx-formly/ngx-formly/blob/main/src/ui/kendo/package.json) | MIT |
| 86 | [niyouqianma/workbuddy2api-hub](https://github.com/niyouqianma/workbuddy2api-hub) | [pricing/pricing.json](https://github.com/niyouqianma/workbuddy2api-hub/blob/main/pricing/pricing.json) | MIT |
| 87 | [o-o-overflow/dcf-game-infrastructure-public](https://github.com/o-o-overflow/dcf-game-infrastructure-public) | [ooogame/database/frontend/public/manifest.json](https://github.com/o-o-overflow/dcf-game-infrastructure-public/blob/main/ooogame/database/frontend/public/manifest.json) | BSD-2-Clause |
| 88 | [o-o-overflow/dcf-game-infrastructure-public](https://github.com/o-o-overflow/dcf-game-infrastructure-public) | [ooogame/team\_interface/frontend/public/manifest.json](https://github.com/o-o-overflow/dcf-game-infrastructure-public/blob/main/ooogame/team_interface/frontend/public/manifest.json) | BSD-2-Clause |
| 89 | [OpenMobileAlliance/OMA-LWM2M-DevKit---DECOMMISSIONED](https://github.com/OpenMobileAlliance/OMA-LWM2M-DevKit---DECOMMISSIONED) | [clients/example-client.json](https://github.com/OpenMobileAlliance/OMA-LWM2M-DevKit---DECOMMISSIONED/blob/master/clients/example-client.json) | ISC |
| 90 | [outflanknl/RedELK](https://github.com/outflanknl/RedELK) | [elkserver/docker/redelk-base/redelkinstalldata/templates/diff/redelk\_kibana\_index-pattern\_implantsdb.json](https://github.com/outflanknl/RedELK/blob/master/elkserver/docker/redelk-base/redelkinstalldata/templates/diff/redelk_kibana_index-pattern_implantsdb.json) | BSD-3-Clause |
| 91 | [outflanknl/RedELK](https://github.com/outflanknl/RedELK) | [elkserver/docker/redelk-base/redelkinstalldata/templates/redelk\_kibana\_settings.json](https://github.com/outflanknl/RedELK/blob/master/elkserver/docker/redelk-base/redelkinstalldata/templates/redelk_kibana_settings.json) | BSD-3-Clause |
| 92 | [pakastin/frzr](https://github.com/pakastin/frzr) | [package.json](https://github.com/pakastin/frzr/blob/master/package.json) | ISC |
| 93 | [pawamoy/website](https://github.com/pawamoy/website) | [sponsors-logo.json](https://github.com/pawamoy/website/blob/main/sponsors-logo.json) | ISC |
| 94 | [perplexityai/wandr](https://github.com/perplexityai/wandr) | [datasets/wandr/new-grad-rn-residencies/tests/manifest.json](https://github.com/perplexityai/wandr/blob/main/datasets/wandr/new-grad-rn-residencies/tests/manifest.json) | Apache-2.0 |
| 95 | [perplexityai/wandr](https://github.com/perplexityai/wandr) | [datasets/wandr/sony-label-artist-affiliations/tests/manifest.json](https://github.com/perplexityai/wandr/blob/main/datasets/wandr/sony-label-artist-affiliations/tests/manifest.json) | Apache-2.0 |
| 96 | [PlakarKorp/go-cdc-chunkers](https://github.com/PlakarKorp/go-cdc-chunkers) | [tests/testdata/golden.json](https://github.com/PlakarKorp/go-cdc-chunkers/blob/main/tests/testdata/golden.json) | ISC |
| 97 | [PlakarKorp/go-cdc-chunkers](https://github.com/PlakarKorp/go-cdc-chunkers) | [tests/vectors/vectors.json](https://github.com/PlakarKorp/go-cdc-chunkers/blob/main/tests/vectors/vectors.json) | ISC |
| 98 | [rcaferati/react-awesome-slider](https://github.com/rcaferati/react-awesome-slider) | [.eslintrc.json](https://github.com/rcaferati/react-awesome-slider/blob/master/.eslintrc.json) | MIT |
| 99 | [rcaferati/react-awesome-slider](https://github.com/rcaferati/react-awesome-slider) | [package.json](https://github.com/rcaferati/react-awesome-slider/blob/master/package.json) | MIT |
| 100 | [reframe-hpc/reframe](https://github.com/reframe-hpc/reframe) | [reframe/schemas/config.json](https://github.com/reframe-hpc/reframe/blob/develop/reframe/schemas/config.json) | BSD-3-Clause |
| 101 | [reframe-hpc/reframe](https://github.com/reframe-hpc/reframe) | [reframe/schemas/runreport.json](https://github.com/reframe-hpc/reframe/blob/develop/reframe/schemas/runreport.json) | BSD-3-Clause |
| 102 | [richgel999/miniz](https://github.com/richgel999/miniz) | [CMakePresets.json](https://github.com/richgel999/miniz/blob/master/CMakePresets.json) | MIT |
| 103 | [riophae/vue-treeselect](https://github.com/riophae/vue-treeselect) | [package.json](https://github.com/riophae/vue-treeselect/blob/master/package.json) | MIT |
| 104 | [sarvjeets/lakshmi](https://github.com/sarvjeets/lakshmi) | [tests/data/profile.json](https://github.com/sarvjeets/lakshmi/blob/develop/tests/data/profile.json) | MIT |
| 105 | [souporserious/react-media-player](https://github.com/souporserious/react-media-player) | [package.json](https://github.com/souporserious/react-media-player/blob/master/package.json) | ISC |
| 106 | [step-security/wait-for-secrets](https://github.com/step-security/wait-for-secrets) | [package.json](https://github.com/step-security/wait-for-secrets/blob/main/package.json) | Apache-2.0 |
| 107 | [step-security/wait-for-secrets](https://github.com/step-security/wait-for-secrets) | [tsconfig.json](https://github.com/step-security/wait-for-secrets/blob/main/tsconfig.json) | Apache-2.0 |
| 108 | [stephenmcd/mezzanine](https://github.com/stephenmcd/mezzanine) | [mezzanine/pages/fixtures/mezzanine\_optional.json](https://github.com/stephenmcd/mezzanine/blob/master/mezzanine/pages/fixtures/mezzanine_optional.json) | BSD-2-Clause |
| 109 | [stephenmcd/mezzanine](https://github.com/stephenmcd/mezzanine) | [mezzanine/pages/fixtures/mezzanine\_required.json](https://github.com/stephenmcd/mezzanine/blob/master/mezzanine/pages/fixtures/mezzanine_required.json) | BSD-2-Clause |
| 110 | [sumitchatterjee13/nuke-nodes-comfyui](https://github.com/sumitchatterjee13/nuke-nodes-comfyui) | [tools/interface\_manifest.json](https://github.com/sumitchatterjee13/nuke-nodes-comfyui/blob/main/tools/interface_manifest.json) | MIT |
| 111 | [swagger-api/swagger-codegen-generators](https://github.com/swagger-api/swagger-codegen-generators) | [src/test/resources/3\_0\_0/datePropertyTest.json](https://github.com/swagger-api/swagger-codegen-generators/blob/master/src/test/resources/3_0_0/datePropertyTest.json) | Apache-2.0 |
| 112 | [swagger-api/swagger-codegen-generators](https://github.com/swagger-api/swagger-codegen-generators) | [src/test/resources/3\_0\_0/requestBodyRefTest.json](https://github.com/swagger-api/swagger-codegen-generators/blob/master/src/test/resources/3_0_0/requestBodyRefTest.json) | Apache-2.0 |
| 113 | [thomasballinger/observable-jupyter](https://github.com/thomasballinger/observable-jupyter) | [js/package.json](https://github.com/thomasballinger/observable-jupyter/blob/main/js/package.json) | ISC |
| 114 | [thorgate/django-project-template](https://github.com/thorgate/django-project-template) | [{{cookiecutter.repo\_name}}/app/tsconfig.json](https://github.com/thorgate/django-project-template/blob/master/%7B%7Bcookiecutter.repo_name%7D%7D/app/tsconfig.json) | ISC |
| 115 | [tlaplus/tlapm](https://github.com/tlaplus/tlapm) | [.vscode/cspell.json](https://github.com/tlaplus/tlapm/blob/main/.vscode/cspell.json) | BSD-2-Clause |
| 116 | [tlaplus/tlapm](https://github.com/tlaplus/tlapm) | [.vscode/launch.json](https://github.com/tlaplus/tlapm/blob/main/.vscode/launch.json) | BSD-2-Clause |
| 117 | [valkey-io/valkey-search](https://github.com/valkey-io/valkey-search) | [src/commands/ft.create.json](https://github.com/valkey-io/valkey-search/blob/main/src/commands/ft.create.json) | BSD-3-Clause |
| 118 | [vuejs/devtools](https://github.com/vuejs/devtools) | [packages/chrome/package.json](https://github.com/vuejs/devtools/blob/main/packages/chrome/package.json) | MIT |
| 119 | [vuejs/devtools](https://github.com/vuejs/devtools) | [packages/client/package.json](https://github.com/vuejs/devtools/blob/main/packages/client/package.json) | MIT |
| 120 | [webpro-nl/unbash](https://github.com/webpro-nl/unbash) | [.oxfmtrc.json](https://github.com/webpro-nl/unbash/blob/main/.oxfmtrc.json) | ISC |
| 121 | [webpro-nl/unbash](https://github.com/webpro-nl/unbash) | [package.json](https://github.com/webpro-nl/unbash/blob/main/package.json) | ISC |
| 122 | [XRPLF/xrpl4j](https://github.com/XRPLF/xrpl4j) | [xrpl4j-core/src/main/resources/definitions.json](https://github.com/XRPLF/xrpl4j/blob/main/xrpl4j-core/src/main/resources/definitions.json) | ISC |
| 123 | [zigcc/awesome-zig](https://github.com/zigcc/awesome-zig) | [package.json](https://github.com/zigcc/awesome-zig/blob/main/package.json) | CC0-1.0 |
| 124 | [Zulko/eagle.js](https://github.com/Zulko/eagle.js) | [package.json](https://github.com/Zulko/eagle.js/blob/master/package.json) | ISC |

## Appendix C. The LIN meta-schema

The meta-schema that LIN schemas follow is JSON Schema 2020-12, published with this spec in LIN. It is eight documents: the main meta-schema and the seven vocabulary meta-schemas it includes with `allOf`. Each was converted from json-schema.org's JSON and reads back to identical data, so its hash is the same as the JSON original's.

| `$id` (under `https://json-schema.org/draft/2020-12/`) | Hash |
| --- | --- |
| `schema` (main) | `sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars` |
| `meta/core` | `sha256-rYIVMHECYTpbxewIO3m7cFkmdJ164g7AapUNzu_3tz4` |
| `meta/applicator` | `sha256-may8pJYoFfev-pvC-3PaZNeciE0ilp0gMjmzAtarwVs` |
| `meta/unevaluated` | `sha256-TLdSXoAYb-3G_UCkjHTr-SG6ULKAmHpRxT-zcDgJWBw` |
| `meta/validation` | `sha256-nC5JxRGEktjjr7tWjih33KajTKPKi99pPu4ONQco-DM` |
| `meta/meta-data` | `sha256-0cao2J2xjwekmN8WdCyjZf2mdr44gIgw8Lc42LncE2A` |
| `meta/format-annotation` | `sha256-giK6XQfewAlGPLo0Xo6ZhAAcU5LCa15lcEbwuF2MgrU` |
| `meta/content` | `sha256-tLngpb9Ypy0MzWztLD0UK0VgfinH3QTSRRVVvFLYZYg` |

- Every one of the eight has the main meta-schema's hash in its header. The main meta-schema names its own hash, which works because the header is not part of the hashed data.
- The documents refer to each other by `$id` URI, as JSON Schema does; the table maps each `$id` to its hash, so a reader can resolve them from the registry.

**Main meta-schema** (`schema.lin`)

```
LIN 1 sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars
$schema https://json-schema.org/draft/2020-12/schema
$id https://json-schema.org/draft/2020-12/schema
$vocabulary
 https://json-schema.org/draft/2020-12/vocab/core true
 https://json-schema.org/draft/2020-12/vocab/applicator true
 https://json-schema.org/draft/2020-12/vocab/unevaluated true
 https://json-schema.org/draft/2020-12/vocab/validation true
 https://json-schema.org/draft/2020-12/vocab/meta-data true
 https://json-schema.org/draft/2020-12/vocab/format-annotation true
 https://json-schema.org/draft/2020-12/vocab/content true
$dynamicAnchor meta
title Core and Validation specifications meta-schema
allOf[$ref]
 meta/core
 meta/applicator
 meta/unevaluated
 meta/validation
 meta/meta-data
 meta/format-annotation
 meta/content
type[] object|boolean
$comment This meta-schema also defines keywords that have appeared in previous drafts in order to prevent incompatible extensions as they remain in common use.
properties
 definitions
  $comment "\"definitions\" has been replaced by \"$defs\"."
  type object
  additionalProperties{} $dynamicRef #meta
  deprecated true
  default {}
 dependencies
  $comment "\"dependencies\" has been split and replaced by \"dependentSchemas\" and \"dependentRequired\" in order to serve their differing semantics."
  type object
  additionalProperties
   anyOf
   + $dynamicRef #meta
   + $ref meta/validation#/$defs/stringArray
  deprecated true
  default {}
 $recursiveAnchor
  $comment "\"$recursiveAnchor\" has been replaced by \"$dynamicAnchor\"."
  $ref meta/core#/$defs/anchorString
  deprecated true
 $recursiveRef
  $comment "\"$recursiveRef\" has been replaced by \"$dynamicRef\"."
  $ref meta/core#/$defs/uriReferenceString
  deprecated true
```

**Core vocabulary** (`meta/core.lin`)

```
LIN 1 sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars
$schema https://json-schema.org/draft/2020-12/schema
$id https://json-schema.org/draft/2020-12/meta/core
$dynamicAnchor meta
title Core vocabulary meta-schema
type[] object|boolean
properties
 $id
  $ref #/$defs/uriReferenceString
  $comment Non-empty fragments not allowed.
  pattern ^[^#]*#?$
 $schema{} $ref #/$defs/uriString
 $ref{} $ref #/$defs/uriReferenceString
 $anchor{} $ref #/$defs/anchorString
 $dynamicRef{} $ref #/$defs/uriReferenceString
 $dynamicAnchor{} $ref #/$defs/anchorString
 $vocabulary
  type object
  propertyNames{} $ref #/$defs/uriString
  additionalProperties{} type boolean
 $comment{} type string
 $defs
  type object
  additionalProperties{} $dynamicRef #meta
$defs
 anchorString{} type string|pattern ^[A-Za-z_][-A-Za-z0-9._]*$
 uriString{} type string|format uri
 uriReferenceString{} type string|format uri-reference
```

**Applicator vocabulary** (`meta/applicator.lin`)

```
LIN 1 sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars
$schema https://json-schema.org/draft/2020-12/schema
$id https://json-schema.org/draft/2020-12/meta/applicator
$dynamicAnchor meta
title Applicator vocabulary meta-schema
type[] object|boolean
properties
 prefixItems{} $ref #/$defs/schemaArray
 items{} $dynamicRef #meta
 contains{} $dynamicRef #meta
 additionalProperties{} $dynamicRef #meta
 properties
  type object
  additionalProperties{} $dynamicRef #meta
  default {}
 patternProperties
  type object
  additionalProperties{} $dynamicRef #meta
  propertyNames{} format regex
  default {}
 dependentSchemas
  type object
  additionalProperties{} $dynamicRef #meta
  default {}
 propertyNames{} $dynamicRef #meta
 if{} $dynamicRef #meta
 then{} $dynamicRef #meta
 else{} $dynamicRef #meta
 allOf{} $ref #/$defs/schemaArray
 anyOf{} $ref #/$defs/schemaArray
 oneOf{} $ref #/$defs/schemaArray
 not{} $dynamicRef #meta
$defs
 schemaArray
  type array
  minItems 1
  items{} $dynamicRef #meta
```

**Unevaluated vocabulary** (`meta/unevaluated.lin`)

```
LIN 1 sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars
$schema https://json-schema.org/draft/2020-12/schema
$id https://json-schema.org/draft/2020-12/meta/unevaluated
$dynamicAnchor meta
title Unevaluated applicator vocabulary meta-schema
type[] object|boolean
properties
 unevaluatedItems{} $dynamicRef #meta
 unevaluatedProperties{} $dynamicRef #meta
```

**Validation vocabulary** (`meta/validation.lin`)

```
LIN 1 sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars
$schema https://json-schema.org/draft/2020-12/schema
$id https://json-schema.org/draft/2020-12/meta/validation
$dynamicAnchor meta
title Validation vocabulary meta-schema
type[] object|boolean
properties
 type
  anyOf
  + $ref #/$defs/simpleTypes
  + type array
   items{} $ref #/$defs/simpleTypes
   minItems 1
   uniqueItems true
 const true
 enum{} type array|items true
 multipleOf{} type number|exclusiveMinimum 0
 maximum{} type number
 exclusiveMaximum{} type number
 minimum{} type number
 exclusiveMinimum{} type number
 maxLength{} $ref #/$defs/nonNegativeInteger
 minLength{} $ref #/$defs/nonNegativeIntegerDefault0
 pattern{} type string|format regex
 maxItems{} $ref #/$defs/nonNegativeInteger
 minItems{} $ref #/$defs/nonNegativeIntegerDefault0
 uniqueItems{} type boolean|default false
 maxContains{} $ref #/$defs/nonNegativeInteger
 minContains{} $ref #/$defs/nonNegativeInteger|default 1
 maxProperties{} $ref #/$defs/nonNegativeInteger
 minProperties{} $ref #/$defs/nonNegativeIntegerDefault0
 required{} $ref #/$defs/stringArray
 dependentRequired
  type object
  additionalProperties{} $ref #/$defs/stringArray
$defs
 nonNegativeInteger{} type integer|minimum 0
 nonNegativeIntegerDefault0{} $ref #/$defs/nonNegativeInteger|default 0
 simpleTypes
  enum[] array|boolean|integer|null|number|object|string
 stringArray
  type array
  items{} type string
  uniqueItems true
  default []
```

**Meta-data vocabulary** (`meta/meta-data.lin`)

```
LIN 1 sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars
$schema https://json-schema.org/draft/2020-12/schema
$id https://json-schema.org/draft/2020-12/meta/meta-data
$dynamicAnchor meta
title Meta-data vocabulary meta-schema
type[] object|boolean
properties
 title{} type string
 description{} type string
 default true
 deprecated{} type boolean|default false
 readOnly{} type boolean|default false
 writeOnly{} type boolean|default false
 examples{} type array|items true
```

**Format-annotation vocabulary** (`meta/format-annotation.lin`)

```
LIN 1 sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars
$schema https://json-schema.org/draft/2020-12/schema
$id https://json-schema.org/draft/2020-12/meta/format-annotation
$dynamicAnchor meta
title Format vocabulary meta-schema for annotation results
type[] object|boolean
properties
 format{} type string
```

**Content vocabulary** (`meta/content.lin`)

```
LIN 1 sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars
$schema https://json-schema.org/draft/2020-12/schema
$id https://json-schema.org/draft/2020-12/meta/content
$dynamicAnchor meta
title Content vocabulary meta-schema
type[] object|boolean
properties
 contentEncoding{} type string
 contentMediaType{} type string
 contentSchema{} $dynamicRef #meta
```
