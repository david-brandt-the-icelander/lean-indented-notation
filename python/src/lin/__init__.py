"""LIN (Lean Indented Notation): JSON's data model in fewer tokens.

    >>> import lin
    >>> text = lin.dumps({"name": "demo", "tags": ["a", "b"]})
    >>> lin.loads(text)
    {'name': 'demo', 'tags': ['a', 'b']}
"""
from .core import (DEFAULT_OBJECT_LIMIT, DEFAULT_SPLIT_LIMIT, VERSION, LinError, SchemaRequired,
                   dumps, dumps_fragment, loads, num, parse, read_header_of, resolve, schema_id_kind)
from .jcs import canonicalize, schema_id, schema_sha256
from .jsonio import from_json, to_json

__all__ = ['VERSION', 'LinError', 'SchemaRequired', 'loads', 'dumps', 'dumps_fragment', 'parse', 'resolve',
           'read_header_of', 'num', 'canonicalize', 'schema_id', 'schema_sha256', 'schema_id_kind', 'from_json', 'to_json',
           'lin_to_json', 'json_to_lin', 'DEFAULT_SPLIT_LIMIT', 'DEFAULT_OBJECT_LIMIT']
__version__ = '1.0.0'


def lin_to_json(text, schema=None, *, ignore_schema=False, indent=2):
    """LIN text to JSON text (2-space indented by default, as the .json siblings in this repo)."""
    return to_json(loads(text, schema, ignore_schema=ignore_schema), indent)


def json_to_lin(text, **kw):
    """JSON text to LIN text; keyword arguments as for dumps()."""
    return dumps(from_json(text), **kw)
