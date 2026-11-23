"""RFC 8785 JSON Canonicalization and LIN schema ids (spec section 7)."""
import hashlib

from .core import is_number, json_string, num


def _num(x):
    # RFC 8785 serializes every number as the IEEE double it denotes, in ECMAScript form.
    return num(float(x))


def canonicalize(v):
    """The JCS text of a JSON value."""
    if v is None:
        return 'null'
    if v is True:
        return 'true'
    if v is False:
        return 'false'
    if is_number(v):
        return _num(v)
    if isinstance(v, str):
        return json_string(v)
    if isinstance(v, list):
        return '[' + ','.join(canonicalize(x) for x in v) + ']'
    if isinstance(v, dict):
        keys = sorted(v, key=lambda k: k.encode('utf-16-be'))
        return '{' + ','.join(json_string(k) + ':' + canonicalize(v[k]) for k in keys) + '}'
    raise TypeError(f'not a JSON value: {type(v).__name__}')


def schema_sha256(schema):
    """The SHA-256 of the schema's canonical JSON, in hex: what a registry records."""
    return hashlib.sha256(canonicalize(schema).encode('utf-8')).hexdigest()


def schema_id(schema):
    """The schema's provisional id: the first 128 bits of its SHA-256 as a 39-digit decimal number.
    Digits are the cheapest text for tokenizers, which split them three at a time."""
    d = hashlib.sha256(canonicalize(schema).encode('utf-8')).digest()
    return str(int.from_bytes(d[:16], 'big')).zfill(39)
