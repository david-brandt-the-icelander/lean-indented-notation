# The official LIN schema registry

This directory is the official registry for LIN schemas (see
[spec/registry.md](../spec/registry.md)). Each schema is here twice, as
`<provisional id>.lin` and `<provisional id>.json`. [`index.lin`](index.lin) lists them all with
their provisional ids and full SHA-256, and records every registered number: it is the authority
for numbers.

| Number | Provisional id | Schema |
| --- | --- | --- |
| 1 | `257594055969365195939657195617727395759` | The LIN meta-schema: JSON Schema 2020-12 |
| 2 | `124342915454511122437553197701222413100` | The schema of [`registries.lin`](../registries.lin) |
| 3 | `269085630653286729508730126105558559404` | The schema of `index.lin` |
| ~ | `230631871630928134081358861136639409008` | 2020-12 core vocabulary |
| ~ | `204268784547655470004489683923042032228` | 2020-12 applicator vocabulary |
| ~ | `101973188649443879620232466638130441209` | 2020-12 unevaluated vocabulary |
| ~ | `207599909227210076967483512369187420124` | 2020-12 validation vocabulary |
| ~ | `278840150503937631720490556150644515685` | 2020-12 meta-data vocabulary |
| ~ | `172979957444070324120297600708435155332` | 2020-12 format-annotation vocabulary |
| ~ | `240226170551730644875985711096481518635` | 2020-12 content vocabulary |

Never edit a schema file here: a changed schema has a new provisional id, so it is a new file. Never
change or reuse a number. Add a schema with `lin schema publish <file> --dir schemas`, and give it a
number with `lin schema register <file> --dir schemas`; both update `index.lin`.

The eight JSON Schema 2020-12 meta-schemas were converted from json-schema.org and read back to
identical data; they remain the JSON Schema project's work, under its licence terms.
