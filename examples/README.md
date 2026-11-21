# Examples

The full example of spec section 12: a plugin registry and its schema.

| File | What it is |
| --- | --- |
| `plugin-registry.lin` | The registry in plain form; its header names the schema's provisional id |
| `plugin-registry.split.lin` | The same data in split form: long values after `---` |
| `plugin-registry.schema.lin` | The schema, in LIN |
| `schemas/` | The schema again, named by its provisional id, so `lin` finds it locally |

Each `.lin` file has a `.json` twin holding the same data.
