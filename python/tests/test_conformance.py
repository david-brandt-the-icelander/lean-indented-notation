"""Runs the shared conformance suite in tests/conformance against the Python library."""
import glob
import json
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'src'))
import lin  # noqa: E402

SUITE = os.path.join(HERE, '..', '..', 'tests', 'conformance')
SCHEMAS = os.path.join(HERE, '..', '..', 'schemas')


def read(p):
    with open(p, encoding='utf-8', newline='') as f:
        return f.read()


def files(sub, ext):
    return sorted(glob.glob(os.path.join(SUITE, sub, '*' + ext)))


class Conformance(unittest.TestCase):
    def test_write(self):
        for p in files('write', '.json'):
            src = read(p)
            lp = p[:-5] + '.lin'
            with self.subTest(lp):
                self.assertEqual(lin.json_to_lin(src, split=lp.endswith('.split.lin')), read(lp))
                self.assertEqual(lin.lin_to_json(read(lp)), src)

    def test_read(self):
        for p in files('read', '.lin'):
            with self.subTest(p):
                self.assertEqual(lin.lin_to_json(read(p)), read(p[:-4] + '.json'))

    def test_invalid(self):
        for p in files('invalid', '.lin'):
            with self.subTest(p), self.assertRaises(lin.LinError):
                lin.loads(read(p))

    def test_schema(self):
        schema = lin.from_json(read(files('schemas', '.json')[0]))
        h = lin.schema_hash(schema)
        for p in files('schema', '.lin'):
            want = read(p[:-4] + '.json')
            with self.subTest(p):
                self.assertEqual(lin.lin_to_json(read(p), schema), want)
                with self.assertRaises(lin.SchemaRequired):
                    lin.loads(read(p))
                split = p.endswith('.split.lin')
                got = lin.json_to_lin(want, schema=h, schema_doc=schema, split=split, split_limit=5 if split else 24)
                self.assertEqual(got, read(p))

    def test_published_schemas(self):
        for p in glob.glob(os.path.join(SCHEMAS, 'sha256-*.json')):
            self.assertEqual(lin.schema_hash(lin.from_json(read(p))) + '.json', os.path.basename(p))


class Api(unittest.TestCase):
    def test_round_trip(self):
        v = {'name': 'demo', 'tags': ['a', 'b'], 'files': [{'p': 'a', 'n': 1}, {'p': 'b', 'n': 2}]}
        text = lin.dumps(v)
        self.assertEqual(text, 'LIN 1\nname demo\ntags[] a|b\nfiles[p|n]\n a|1\n b|2\n')
        self.assertEqual(lin.loads(text), v)

    def test_errors_have_lines(self):
        with self.assertRaises(lin.LinError) as cm:
            lin.loads('LIN 1\na 1\na 2\n')
        self.assertEqual(cm.exception.line, 3)

    def test_numbers(self):
        self.assertEqual(lin.num(6.0), '6')
        self.assertEqual(lin.num(1e-05), '0.00001')
        self.assertEqual(lin.num(1e21), '1e+21')
        self.assertEqual(lin.to_json({'a': 1.0}, None), '{"a":1}')

    def test_fragment(self):
        self.assertEqual(lin.dumps_fragment({'a': 1}), 'a 1\n')
        self.assertEqual(lin.dumps_fragment('1'), '"1"\n')

    def test_ignore_schema(self):
        h = lin.schema_hash({'type': 'object'})
        text = f'LIN 1 {h}\na 1\n'
        self.assertEqual(lin.loads(text, ignore_schema=True), {'a': 1})
        self.assertEqual(lin.loads(text, {'type': 'object'}), {'a': 1})


if __name__ == '__main__':
    unittest.main()
