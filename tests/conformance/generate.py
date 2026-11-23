"""Regenerate the conformance suite from the Python implementation.

    python3 tests/conformance/generate.py

Every implementation must give the same results on these files; see README.md here.
Review the diff before committing: the files are the expected outputs.
"""
import json
import os
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', '..', 'python', 'src'))
import lin  # noqa: E402

LONG = 'This sentence is longer than twenty-four characters.'

# name -> JSON text of a document. Writer cases: plain and split form must match exactly.
CASES = {
    'empty-object': '{}',
    'empty-array': '[]',
    'scalars': '{"s": "text", "i": 42, "neg": -7, "f": 1.5, "t": true, "f2": false, "n": null}',
    'numbers': '{"a": 1.0, "b": 1.10, "c": 1e-05, "d": 1e-7, "e": 1e21, "f": 1.5e300, "g": -0, "h": -0.0, "i": 123456789012345678901234567890, "j": 0.1, "k": 100.0, "l": 2.5E3, "m": 5e-324, "n": 1.00000000000000000001, "o": 1E400, "p": 0.10, "q": -1.5e-400}',
    'number-like-strings': '{"a": "1", "b": "1.0", "c": "-2", "d": "true", "e": "false", "f": "1e5", "g": "01", "h": "1.", "i": "+1", "j": ".5", "k": "null", "l": "True"}',
    'strings-that-need-quotes': '{"empty": "", "lead": " x", "trail": "x ", "quote": "\\"q", "at": "@x", "tilde": "~", "list": "[]", "obj": "{}", "bar": "|", "tab": "a\\tb", "cr": "a\\rb", "del": "a\\u007fb", "nel": "a\\u0085b", "ls": "a\\u2028b", "ps": "a\\u2029b", "nul": "a\\u0000b"}',
    'strings-that-stay-bare': '{"colon": "a: b", "hash": "# not a comment", "dash": "- x", "plus": "+ y", "inner": "say \\"hi\\"", "pipe": "a|b", "unicode": "été 日本語 🙂", "url": "https://example.com/a?b=c#d", "at-inside": "a@b", "tilde-inside": "~x", "brackets": "[x]"}',
    'keys': '{"plain": 1, "200": 2, "*.yml": 3, "@types/node": 4, "src/main.ts": 5, "a.b": 6, "$ref": 7, "~": 8, "true": 9, "x-y": 10, "two words": 11, "#tag": 12, "-dash": 13, "+plus": 14, "a|b": 15, "a[b]": 16, "a{b}": 17, "": 18, "q\\"": 19, "---": 20, "tab\\there": 21, "é": 22}',
    'multiline': '{"ok": "line one\\nline two\\n\\nline four", "indented": "a\\n  b\\n    c", "trailing-newline": "a\\nb\\n", "leading-newline": "\\na", "trailing-space": "a \\nb", "only-newline": "\\n", "in-list": ["x\\ny", "z"], "in-cell": [{"a": "x\\ny", "b": 1}, {"a": "z", "b": 2}]}',
    'inline-lists': '{"words": ["a", "b", "c"], "nums": [1, 2.5, -3], "mixed": ["x", 1, true, null, "", "a|b", "1"], "long": ["' + LONG + '", "short"], "nested": [[1, 2], [3, [4, 5]], [], {}]}',
    'tables': '{"files": [{"path": "src/main.ts", "size": 2048}, {"path": "README.md", "size": 512}], "with-nulls": [{"a": null, "b": "x|y"}, {"a": 1, "b": ""}], "other-order": [{"a": 1, "b": 2}, {"b": 3, "a": 4}], "nested-value": [{"a": 1, "b": [1]}, {"a": 2, "b": [2]}], "quoted-key": [{"a b": 1}, {"a b": 2}], "one-row": [{"a": 1}]}',
    'root-table': '[{"id": 1, "name": "one"}, {"id": 2, "name": "two"}]',
    'root-inline-list': '["a", "b", 3]',
    'root-items': '[{"a": 1}, "text", [1, 2], [], {}, [{"x": 1}], "multi\\nline"]',
    'items': '{"list": [{"name": "Ada", "email": "ada@example.org", "roles": ["maintainer"]}, {"files": [{"p": 1}, {"p": 2}], "after": true}, {"sub": {"deep": 1}}, {"first": [{"a": 1, "b": {"c": 2}}]}, {}]}',
    'inline-objects': '{"top": {"x": 1, "y": 2}, "deeper": {"size": {"w": 640, "h": 480}, "one": {"k": "v"}, "quoted": {"a": "x|y", "b": "1"}, "long": {"text": "' + LONG + ' ' + LONG + '"}, "empty": {}, "nested": {"inner": {"leaf": 1}}, "badkey": {"a b": 1}}, "list": [{"a": 1, "b": 2}, [{"a": 1}, {"a": 1, "b": 2}, {"a": 1, "b": 2, "c": 3}]]}',
    'split-basic': '{"title": "' + LONG + '", "description": "' + LONG + '", "short": "ok", "n": 12345678901234567890123456789, "tags": ["short", "' + LONG + '"], "notes": "line one of a long note\\nline two of a long note"}',
    'split-owners': '{"plugins": [{"id": "spell-check", "summary": "' + LONG + '", "sources": [{"path": "' + LONG + '", "lines": 1}, {"path": "b", "lines": 2}]}, {"id": 7, "summary": "' + LONG + '"}, {"id": "has.dot", "summary": "' + LONG + '"}, {"id": 1.5, "summary": "' + LONG + '"}, {"id": true, "summary": "' + LONG + '"}, {"summary": "' + LONG + '"}]}',
    'split-collisions': '{"a": [{"id": "x", "d": "' + LONG + '"}, {"id": "x", "d": "' + LONG + ' 2"}, {"id": "x", "d": "' + LONG + ' 3"}], "x": {"d~2": "' + LONG + ' 4"}}',
    'split-unnamable': '{"a.b": "' + LONG + '", "c d": {"e": "' + LONG + '"}, "ok": "' + LONG + '"}',
    'split-root-list': '["' + LONG + '", "b"]',
    'split-headline': '{"id": "' + LONG + '", "kind": "' + LONG + '", "title": "' + LONG + '", "name": "' + LONG + '", "status": "' + LONG + '", "type": "' + LONG + '", "other": "' + LONG + '"}',
    'split-items': '{"steps": ["' + LONG + '", {"k": 1}, "' + LONG + '\\nsecond line"], "deep": {"inline": {"a": "' + LONG + '", "b": 1}}}',
    'unicode-lengths': '{"limit": "ééééééééééééééééééééééééé", "under": "éééééééééééééééééééééééé", "emoji": "🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂"}',
}

# Hand-written LIN that is valid but not what the writer produces: name -> (LIN, JSON value)
READ = {
    'blank-lines': ('LIN 1\na 1\n\nb\n\n c 2\n', {'a': 1, 'b': {'c': 2}}),
    'quoted-anyway': ('LIN 1\n"a" "x"\nb "1"\nc[] "p"|q\n', {'a': 'x', 'b': '1', 'c': ['p', 'q']}),
    'number-forms': ('LIN 1\na 1.0\nb 1E2\nc -0\nd 0.50\ne 12345678901234567890.000000000000000001\n',
                     lin.from_json('{"a": 1.0, "b": 1E2, "c": -0, "d": 0.50, "e": 12345678901234567890.000000000000000001}')),
    'explicit-ref-name': ('LIN 1\na @a\nb @other\n---\n@a one\n@other two\n', {'a': 'one', 'b': 'two'}),
    'data-out-of-order': ('LIN 1\na @\nb @\n---\n@b two\n@a one\n', {'a': 'one', 'b': 'two'}),
    'data-block-and-list': ('LIN 1\na @\nb @\n---\n@a |\n x\n  y\n@b\n - p\n - q\n', {'a': 'x\n y', 'b': ['p', 'q']}),
    'ref-in-cells': ('LIN 1\nt[a|b]\n @|1\n x|2\n---\n@t.0.a long\n', {'t': [{'a': 'long', 'b': 1}, {'a': 'x', 'b': 2}]}),
    'inline-object-forms': ('LIN 1\na{} "k k" 1|b "x|y"|c ~\nl\n-{} p 1\n', {'a': {'k k': 1, 'b': 'x|y', 'c': None}, 'l': [{'p': 1}]}),
    'empty-literals-in-cells': ('LIN 1\na[] []|{}|~\n', {'a': [[], {}, None]}),
    'block-with-blank-lines': ('LIN 1\na |\n x\n\n y\nb 1\n', {'a': 'x\n\ny', 'b': 1}),
    'no-final-newline': ('LIN 1\na 1', {'a': 1}),
    'header-only': ('LIN 1\n', {}),
    'escapes': ('LIN 1\na "\\u00e9\\n\\t\\"\\\\\\/"\n', {'a': 'é\n\t"\\/'}),
}

# LIN that every reader MUST reject: name -> text
INVALID = {
    'no-header': 'a 1\n',
    'wrong-version': 'LIN 2\na 1\n',
    'header-junk': 'LIN 1 x\n',
    'header-id-length': 'LIN 1 12345678901234567890\n',
    'header-id-leading-zero': 'LIN 1 0123\n',
    'header-id-letters': 'LIN 1 4a2\n',
    'header-old-sha256-form': 'LIN 1 sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars\n',
    'header-url-after-number': 'LIN 1 7 https://x.org/s\n',
    'header-http-url': 'LIN 1 000123456789012345678901234567890123456 http://x.org/s\n',
    'header-url-without-hash': 'LIN 1 https://x.org/s\n',
    'empty-file': '',
    'bom': '\ufeffLIN 1\na 1\n',
    'crlf': 'LIN 1\r\na 1\r\n',
    'tab-indent': 'LIN 1\na\n\tb 1\n',
    'over-indented': 'LIN 1\na 1\n  b 2\n',
    'indented-root': 'LIN 1\n a 1\n',
    'duplicate-key': 'LIN 1\na 1\na 2\n',
    'duplicate-key-inline': 'LIN 1\na{} x 1|x 2\n',
    'duplicate-column': 'LIN 1\nt[a|a]\n 1|2\n 3|4\n',
    'empty-value': 'LIN 1\na \n',
    'value-trailing-space': 'LIN 1\na x \n',
    'value-two-spaces': 'LIN 1\na  x\n',
    'empty-cell': 'LIN 1\na[] x||y\n',
    'cell-space': 'LIN 1\na[] x | y\n',
    'row-width': 'LIN 1\nt[a|b]\n 1|2\n 3\n',
    'table-no-rows': 'LIN 1\nt[a|b]\n',
    'bad-column': 'LIN 1\nt[a b|c]\n 1|2\n',
    'empty-brackets': 'LIN 1\na[]\n',
    'empty-braces': 'LIN 1\na{}\n',
    'no-children': 'LIN 1\na\n',
    'bad-item': 'LIN 1\nl\n-x\n',
    'bad-json-string': 'LIN 1\na "\\x"\n',
    'unterminated-string': 'LIN 1\na "abc\n',
    'text-after-string': 'LIN 1\na "x" y\n',
    'raw-control-char': 'LIN 1\na x\u0001y\n',
    'missing-data-entry': 'LIN 1\na @\n---\n',
    'unused-data-entry': 'LIN 1\na x\n---\n@b y\n',
    'duplicate-data-entry': 'LIN 1\na @\n---\n@a x\n@a y\n',
    'ref-used-twice': 'LIN 1\na @c\nb @c\n---\n@c x\n',
    'bad-data-line': 'LIN 1\na @\n---\na x\n',
    'bare-at-value': 'LIN 1\na @types/node\n',
    'inline-object-no-value': 'LIN 1\na{} x\n',
    'inline-object-space-before-bar': 'LIN 1\na{} x 1 |y 2\n',
    'empty-block': 'LIN 1\na |\nb 1\n',
    'nested-marker-empty': 'LIN 1\nl\n-\nb 1\n',
    'key-control-char': 'LIN 1\na\u0085b 1\n',
}

def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(text)


def main():
    for sub in ('write', 'read', 'invalid', 'schema', 'schemas'):
        shutil.rmtree(os.path.join(HERE, sub), ignore_errors=True)
    for name, src in CASES.items():
        doc = lin.from_json(src)
        j = lin.to_json(doc)
        base = os.path.join(HERE, 'write', name)
        write(base + '.json', j)
        write(base + '.lin', lin.dumps(doc))
        write(base + '.split.json', j)
        write(base + '.split.lin', lin.dumps(doc, split=True))
        assert lin.to_json(lin.loads(lin.dumps(doc))) == j, name
        assert lin.to_json(lin.loads(lin.dumps(doc, split=True))) == j, name
    for name, (text, value) in READ.items():
        assert lin.to_json(lin.loads(text)) == lin.to_json(value), name
        write(os.path.join(HERE, 'read', name + '.lin'), text)
        write(os.path.join(HERE, 'read', name + '.json'), lin.to_json(value))
    for name, text in INVALID.items():
        try:
            lin.loads(text)
        except lin.LinError:
            pass
        else:
            raise AssertionError(f'invalid case {name} was accepted')
        write(os.path.join(HERE, 'invalid', name + '.lin'), text)
    # schema typing: the schema is in schemas/<hash>.lin beside it, as in a content-addressed store
    schema = {
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'type': 'object',
        'properties': {
            'version': {'type': 'string'},
            'count': {'type': 'integer'},
            'flag': {'type': 'boolean'},
            'either': {'type': ['integer', 'string']},
            'label': {'$ref': '#/$defs/label'},
            'items': {'type': 'array', 'items': {'type': 'string'}},
            'pair': {'type': 'array', 'prefixItems': [{'type': 'string'}, {'type': 'number'}]},
            'rows': {'type': 'array', 'items': {'type': 'object', 'properties': {'code': {'type': 'string'}, 'n': {'type': 'number'}}}},
            'enumed': {'enum': ['1', '2']},
            'any': {},
            'all': {'allOf': [{'type': 'string'}, {'minLength': 1}]},
            'one': {'oneOf': [{'type': 'string'}, {'type': 'null'}]},
            'map': {'type': 'object', 'additionalProperties': {'type': 'string'}},
            'pat': {'type': 'object', 'patternProperties': {'^s_': {'type': 'string'}}},
            'note': {'type': 'string'},
        },
        '$defs': {'label': {'type': 'string'}},
    }
    h = lin.schema_id(schema)
    write(os.path.join(HERE, 'schemas', h + '.lin'), lin.dumps(schema, schema=META))
    write(os.path.join(HERE, 'schemas', h + '.json'), lin.to_json(schema))
    doc = {'version': '1.0', 'count': 3, 'flag': True, 'either': '20', 'label': 'true', 'items': ['1', 'x', 'false'],
           'pair': ['2', 3], 'rows': [{'code': '007', 'n': 1}, {'code': '1.5', 'n': 2.5}], 'enumed': '1', 'any': '5',
           'all': '12', 'one': '3', 'map': {'a': '1', 'b': 'true'}, 'pat': {'s_x': '9', 'n_x': 9},
           'note': '12345678901234567890123456789'}
    text = lin.dumps(doc, schema=h, schema_doc=schema)
    assert lin.loads(text, schema) == doc
    write(os.path.join(HERE, 'schema', 'typed.lin'), text)
    write(os.path.join(HERE, 'schema', 'typed.json'), lin.to_json(doc))
    split = lin.dumps(doc, schema=h, schema_doc=schema, split=True, split_limit=5)
    assert lin.loads(split, schema) == doc
    write(os.path.join(HERE, 'schema', 'typed.split.lin'), split)
    write(os.path.join(HERE, 'schema', 'typed.split.json'), lin.to_json(doc))
    # the same data under a registered number, which this store's index registers as a fixture
    reg_index = json.load(open(os.path.join(HERE, '..', '..', 'schemas', 'index.json')))
    index_schema_id = next(r['provisional'] for r in reg_index['schemas'] if r['number'] == 3)
    index_schema = json.load(open(os.path.join(HERE, '..', '..', 'schemas', index_schema_id + '.json')))
    idx = {'schemas': [{'number': int(REGISTERED), 'provisional': h, 'sha256': lin.schema_sha256(schema),
                        'id': None, 'title': 'Conformance-suite test schema (a fixture, not a real registration)'}]}
    write(os.path.join(HERE, 'schemas', 'index.lin'), lin.dumps(idx, schema='3', schema_doc=index_schema))
    write(os.path.join(HERE, 'schemas', 'index.json'), lin.to_json(idx))
    reg = lin.dumps(doc, schema=REGISTERED, schema_doc=schema)
    assert lin.loads(reg, schema) == doc
    write(os.path.join(HERE, 'schema', 'typed.registered.lin'), reg)
    write(os.path.join(HERE, 'schema', 'typed.registered.json'), lin.to_json(doc))
    print(f'{len(CASES)} write cases, {len(READ)} read cases, {len(INVALID)} invalid cases, 3 schema cases')


META = '1'  # the LIN meta-schema's registered number
REGISTERED = '4242'

if __name__ == '__main__':
    main()
