# LIN for Python

`pip install lean-indented-notation` (or `pip install ./python` from this repository). Standard
library only; Python 3.10+.

```python
import lin

text = lin.dumps({"name": "demo", "tags": ["a", "b"]})   # 'LIN 1\nname demo\ntags[] a|b\n'
lin.loads(text)                                           # {'name': 'demo', 'tags': ['a', 'b']}

lin.dumps(doc, split=True)              # split form: long values after ---
lin.lin_to_json(text)                   # LIN text to 2-space-indented JSON text
lin.json_to_lin(json_text)              # JSON text to LIN text
lin.schema_hash(schema)                 # 'sha256-…', the content address for a header
```

A file whose header names a schema is typed by it: `lin.loads(text, schema)` checks the schema's hash
against the header; `lin.loads(text, ignore_schema=True)` reads it by the schema-less rules instead.
Write with `lin.dumps(doc, schema=hash, schema_doc=schema)` to name the schema and let strings that
look like numbers go bare where the schema says they are strings.

Errors are `lin.LinError`, with `.line` set when the line is known.

Tests: `python3 -m unittest discover -s tests`.
