# The default LIN schema registry

This directory is the default public content-addressed registry for LIN schemas (see
[spec/registry.md](../spec/registry.md)). Each schema is here twice, as `<hash>.lin` and
`<hash>.json`, named by its content address; [`index.lin`](index.lin) lists them.

| Hash | Schema |
| --- | --- |
| `sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars` | The LIN meta-schema: JSON Schema 2020-12 (`https://json-schema.org/draft/2020-12/schema`) |
| `sha256-rYIVMHECYTpbxewIO3m7cFkmdJ164g7AapUNzu_3tz4` | 2020-12 core vocabulary |
| `sha256-may8pJYoFfev-pvC-3PaZNeciE0ilp0gMjmzAtarwVs` | 2020-12 applicator vocabulary |
| `sha256-TLdSXoAYb-3G_UCkjHTr-SG6ULKAmHpRxT-zcDgJWBw` | 2020-12 unevaluated vocabulary |
| `sha256-nC5JxRGEktjjr7tWjih33KajTKPKi99pPu4ONQco-DM` | 2020-12 validation vocabulary |
| `sha256-0cao2J2xjwekmN8WdCyjZf2mdr44gIgw8Lc42LncE2A` | 2020-12 meta-data vocabulary |
| `sha256-giK6XQfewAlGPLo0Xo6ZhAAcU5LCa15lcEbwuF2MgrU` | 2020-12 format-annotation vocabulary |
| `sha256-tLngpb9Ypy0MzWztLD0UK0VgfinH3QTSRRVVvFLYZYg` | 2020-12 content vocabulary |
| `sha256-GNvQTexeyzKBidPglSpH44StGNk1FBL1sWQD2cKVZA0` | The schema of [`registries.lin`](../registries.lin) |

Readers fetch `https://raw.githubusercontent.com/david-brandt-the-icelander/lean-indented-notation/main/schemas/<hash>.lin`
and check the hash of what they get. Never edit a file here: a changed schema has a new hash. Add one
with `lin schema publish <file> --dir schemas`.

The eight JSON Schema 2020-12 meta-schemas were converted from json-schema.org and read back to
identical data; they remain the JSON Schema project's work, under its licence terms.
